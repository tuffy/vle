use crate::editor::EditorMode;
use crate::help::Keybinding;
use crate::syntax::Syntax;
use ratatui::widgets::StatefulWidget;
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::rc::Rc;

pub enum Source {
    File(PathBuf),
}

impl From<OsString> for Source {
    fn from(s: OsString) -> Self {
        Self::File(s.into())
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::File(pb) => pb.display().fmt(f),
        }
    }
}

impl Source {
    fn source_str(&self) -> &OsStr {
        match self {
            Self::File(pb) => pb.as_os_str(),
        }
    }

    fn name(&self) -> Cow<'_, str> {
        match self {
            Self::File(path) => path.to_string_lossy(),
        }
    }

    pub fn file_name(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::File(path) => path.file_name().map(|s| s.to_string_lossy()),
        }
    }

    pub fn extension(&self) -> Option<&str> {
        match self {
            Self::File(path) => path.extension().and_then(|s| s.to_str()),
        }
    }

    fn read_data(&self) -> std::io::Result<ropey::Rope> {
        use std::fs::File;
        use std::io::BufReader;

        match self {
            Self::File(path) => match File::open(path) {
                Ok(f) => ropey::Rope::from_reader(BufReader::new(f)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ropey::Rope::default()),
                Err(e) => Err(e),
            },
        }
    }

    fn save_data(&self, data: &ropey::Rope) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::BufWriter;

        match self {
            Self::File(path) => File::create(path)
                .map(BufWriter::new)
                .and_then(|f| data.write_to(f)),
        }
    }
}

mod private {
    pub struct Rope {
        rope: ropey::Rope,  // the primary data rope
        saved: ropey::Rope, // the rope's contents on disk
        modified: bool,     // whether the rope has been modified
    }

    impl From<ropey::Rope> for Rope {
        fn from(rope: ropey::Rope) -> Self {
            Self {
                saved: rope.clone(),
                rope,
                modified: false,
            }
        }
    }

    impl Rope {
        /// Whether the rope has been modified
        pub fn modified(&self) -> bool {
            self.modified
        }

        /// Tag rope as having been saved successfully
        pub fn save(&mut self) {
            self.saved = self.rope.clone();
            self.modified = false;
        }

        pub fn get_mut(&mut self) -> RopeHandle<'_> {
            RopeHandle {
                rope: &mut self.rope,
                saved: &mut self.saved,
                modified: &mut self.modified,
            }
        }
    }

    impl std::ops::Deref for Rope {
        type Target = ropey::Rope;

        fn deref(&self) -> &ropey::Rope {
            &self.rope
        }
    }

    pub struct RopeHandle<'r> {
        rope: &'r mut ropey::Rope,
        saved: &'r mut ropey::Rope,
        modified: &'r mut bool,
    }

    impl std::ops::Deref for RopeHandle<'_> {
        type Target = ropey::Rope;

        fn deref(&self) -> &ropey::Rope {
            self.rope
        }
    }

    impl std::ops::DerefMut for RopeHandle<'_> {
        fn deref_mut(&mut self) -> &mut ropey::Rope {
            self.rope
        }
    }

    impl std::ops::Drop for RopeHandle<'_> {
        fn drop(&mut self) {
            // log whether the rope value has been changed
            // from the version that exists on disk
            *self.modified = self.rope != self.saved;
        }
    }
}

/// A buffer corresponding to a file on disk (either local or remote)
struct Buffer {
    source: Source,         // the source file
    rope: private::Rope,    // the data rope
    undo: Vec<Undo>,        // the undo stack
    redo: Vec<BufferState>, // the redo stack
    syntax: Syntax,         // the syntax highlighting to use
}

impl Buffer {
    fn source_str(&self) -> &OsStr {
        self.source.source_str()
    }

    fn open(path: OsString) -> std::io::Result<Self> {
        let source = Source::from(path);

        Ok(Self {
            rope: source.read_data()?.into(),
            syntax: Syntax::new(&source),
            source,
            undo: vec![],
            redo: vec![],
        })
    }

    fn save(&mut self, message: &mut Option<BufferMessage>) {
        match self.source.save_data(&self.rope) {
            Ok(()) => {
                self.rope.save();
                log_movement(&mut self.undo);
            }
            Err(err) => {
                *message = Some(BufferMessage::Error(err.to_string().into()));
            }
        }
    }

    fn total_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn modified(&self) -> bool {
        self.rope.modified()
    }

    fn log_undo(&mut self, cursor: usize, cursor_column: usize) {
        if let None | Some(Undo { finished: true, .. }) = self.undo.last() {
            self.undo.push(Undo {
                state: BufferState {
                    rope: self.rope.clone(),
                    cursor,
                    cursor_column,
                },
                finished: false,
            });
            self.redo.clear();
        }
    }
}

#[derive(Clone)]
pub struct BufferId(Rc<RefCell<Buffer>>);

/// A buffer with additional context on a per-view basis
#[derive(Clone)]
pub struct BufferContext {
    buffer: Rc<RefCell<Buffer>>,
    tabs_required: bool,            // whether the format demands actual tabs
    tab_substitution: String,       // spaces to substitute for tabs
    viewport_height: usize,         // viewport's current height in lines
    cursor: usize,                  // cursor's absolute position in rope, in characters
    cursor_column: usize,           // cursor's desired column, in characters
    selection: Option<usize>,       // cursor's text selection anchor
    message: Option<BufferMessage>, // some user-facing message
}

// moving the cursor vertically should preserve the cursor column
// even if the intervening lines are shorter
// (moving down then back up should always round-trip back to the same
// column, even if the next line is shorter)
// while horizontal movement or adding text updates the column
// to the current position

impl BufferContext {
    pub fn id(&self) -> BufferId {
        BufferId(Rc::clone(&self.buffer))
    }

    pub fn modified(&self) -> bool {
        self.buffer.borrow().modified()
    }

    pub fn open(path: OsString) -> std::io::Result<Self> {
        Buffer::open(path).map(|b| b.into())
    }

    pub fn save(&mut self) {
        self.buffer.borrow_mut().save(&mut self.message);
    }

    pub fn get_cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_selection(&mut self, start: usize, end: usize) {
        assert!(end >= start);
        let buf = self.buffer.borrow();
        self.cursor = start;
        self.selection = Some(end);
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
    }

    /// Returns cursor position in rope as (row, col), if possible
    ///
    /// Both indexes start from 0
    ///
    /// This position is independent of the viewport position
    fn cursor_position(&self) -> Option<(usize, usize)> {
        let rope = &self.buffer.borrow().rope;
        let line = rope.try_char_to_line(self.cursor).ok()?;
        let line_start = rope.try_line_to_char(line).ok()?;
        let spaces_per_tab = self.tab_substitution.len();

        Some((
            line,
            rope.chars_at(line_start)
                .take(self.cursor.checked_sub(line_start)?)
                .map(|c| match c {
                    '\t' => spaces_per_tab,
                    _ => 1,
                })
                .sum(),
        ))
    }

    pub fn cursor_up(&mut self, lines: usize, selecting: bool) {
        let mut buf = self.buffer.borrow_mut();
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
            let previous_line = current_line.saturating_sub(lines);
            if let Some((prev_start, prev_end)) = line_char_range(&buf.rope, previous_line) {
                log_movement(&mut buf.undo);
                update_selection(&mut self.selection, self.cursor, selecting);
                self.cursor = (prev_start + self.cursor_column).min(prev_end);
            }
        }
    }

    pub fn cursor_down(&mut self, lines: usize, selecting: bool) {
        let mut buf = self.buffer.borrow_mut();
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
            let next_line = (current_line + lines).min(buf.rope.len_lines().saturating_sub(1));
            if let Some((next_start, next_end)) = line_char_range(&buf.rope, next_line) {
                log_movement(&mut buf.undo);
                update_selection(&mut self.selection, self.cursor, selecting);
                self.cursor = (next_start + self.cursor_column).min(next_end);
            }
        }
    }

    pub fn cursor_back(&mut self, selecting: bool) {
        let mut buf = self.buffer.borrow_mut();
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = self.cursor.saturating_sub(1);
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
        log_movement(&mut buf.undo);
    }

    pub fn cursor_forward(&mut self, selecting: bool) {
        let mut buf = self.buffer.borrow_mut();
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = (self.cursor + 1).min(buf.rope.len_chars());
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
        log_movement(&mut buf.undo);
    }

    pub fn cursor_home(&mut self, selecting: bool) {
        let mut buf = self.buffer.borrow_mut();
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor)
            && let Some((home, _)) = line_char_range(&buf.rope, current_line)
        {
            log_movement(&mut buf.undo);
            update_selection(&mut self.selection, self.cursor, selecting);
            self.cursor = home;
            self.cursor_column = 0;
        }
    }

    pub fn cursor_end(&mut self, selecting: bool) {
        let mut buf = self.buffer.borrow_mut();
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor)
            && let Some((_, end)) = line_char_range(&buf.rope, current_line)
        {
            log_movement(&mut buf.undo);
            update_selection(&mut self.selection, self.cursor, selecting);
            self.cursor_column += end - self.cursor;
            self.cursor = end;
        }
    }

    pub fn last_line(&self) -> usize {
        self.buffer.borrow_mut().rope.len_lines().saturating_sub(1)
    }

    pub fn select_line(&mut self, line: usize) {
        let mut buf = self.buffer.borrow_mut();
        match buf.rope.try_line_to_char(line) {
            Ok(cursor) => {
                log_movement(&mut buf.undo);
                self.cursor_column = 0;
                self.cursor = cursor;
                self.selection = None;
            }
            Err(_) => {
                self.message = Some(BufferMessage::Error("invalid line".into()));
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let mut buf = self.buffer.borrow_mut();
        buf.log_undo(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        if let Some(selection) = self.selection.take() {
            zap_selection(
                &mut rope,
                &mut self.cursor,
                &mut self.cursor_column,
                selection,
            );
        }
        rope.insert_char(self.cursor, c);
        self.cursor += 1;
        self.cursor_column += 1;
    }

    pub fn paste(&mut self, pasted: &CutBuffer) {
        let mut buf = self.buffer.borrow_mut();
        buf.log_undo(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        if let Some(selection) = self.selection.take() {
            zap_selection(
                &mut rope,
                &mut self.cursor,
                &mut self.cursor_column,
                selection,
            );
        }
        if rope.try_insert(self.cursor, &pasted.data).is_ok() {
            self.cursor += pasted.chars_len;
            self.cursor_column = cursor_column(&rope, self.cursor);
        }
    }

    pub fn newline(&mut self) {
        let mut buf = self.buffer.borrow_mut();

        let (indent, all_indent) = match line_start_to_cursor(&buf.rope, self.cursor) {
            Some(iter) => {
                let mut iter = iter.peekable();
                let mut indent = 0;
                while iter.next_if(|c| *c == ' ').is_some() {
                    indent += 1;
                }
                (indent, iter.next().is_none())
            }
            None => (0, false),
        };

        buf.log_undo(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();

        // if the whole line is indent, insert newline *before* indent
        // instead of adding a fresh indentation
        if all_indent {
            rope.insert_char(self.cursor - indent, '\n');
            self.cursor += 1;
        } else {
            rope.insert_char(self.cursor, '\n');
            self.cursor += 1;
            self.cursor_column = 0;
            for _ in 0..indent {
                rope.insert_char(self.cursor, ' ');
                self.cursor += 1;
                self.cursor_column += 1;
            }
        }
    }

    pub fn backspace(&mut self) {
        let mut buf = self.buffer.borrow_mut();

        buf.log_undo(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();

        match self.selection.take() {
            None => {
                if let Some(prev) = self.cursor.checked_sub(1)
                    && rope.try_remove(prev..self.cursor).is_ok()
                {
                    self.cursor -= 1;
                    // we need to recalculate the cursor column
                    // in case a newline has been removed
                    self.cursor_column = cursor_column(&rope, self.cursor);
                }
            }
            Some(current_selection) => {
                zap_selection(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    current_selection,
                );
            }
        }
    }

    pub fn delete(&mut self) {
        let buf = &mut self.buffer.borrow_mut();
        buf.log_undo(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();

        match self.selection.take() {
            None => {
                let _ = rope.try_remove(self.cursor..(self.cursor + 1));
                // leave cursor position and current column unchanged
            }
            Some(current_selection) => {
                zap_selection(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    current_selection,
                );
            }
        }
    }

    pub fn get_selection(&mut self) -> Option<CutBuffer> {
        let selection = self.selection.take()?;
        let (selection_start, selection_end) = reorder(self.cursor, selection);
        self.buffer
            .borrow()
            .rope
            .get_slice(selection_start..selection_end)
            .map(|r| r.into())
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn take_selection(&mut self) -> Option<CutBuffer> {
        let selection = self.selection.take()?;
        let (selection_start, selection_end) = reorder(self.cursor, selection);
        let mut buf = self.buffer.borrow_mut();
        buf.log_undo(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();

        rope.get_slice(selection_start..selection_end)
            .map(|r| r.into())
            .inspect(|_| {
                rope.remove(selection_start..selection_end);
                self.cursor = selection_start;
                self.cursor_column = cursor_column(&rope, self.cursor);
            })
    }

    pub fn perform_undo(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        match buf.undo.pop() {
            Some(Undo { mut state, .. }) => {
                use std::ops::DerefMut;
                std::mem::swap(buf.rope.get_mut().deref_mut(), &mut state.rope);
                std::mem::swap(&mut self.cursor, &mut state.cursor);
                std::mem::swap(&mut self.cursor_column, &mut state.cursor_column);
                buf.redo.push(state);
                self.selection = None;
            }
            None => {
                self.message = Some(BufferMessage::Notice("nothing left to undo".into()));
            }
        }
    }

    pub fn perform_redo(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        match buf.redo.pop() {
            Some(mut state) => {
                use std::ops::DerefMut;
                std::mem::swap(buf.rope.get_mut().deref_mut(), &mut state.rope);
                std::mem::swap(&mut self.cursor, &mut state.cursor);
                std::mem::swap(&mut self.cursor_column, &mut state.cursor_column);
                buf.undo.push(Undo {
                    state,
                    finished: true,
                });
                self.selection = None;
            }
            None => {
                self.message = Some(BufferMessage::Notice("nothing left to redo".into()));
            }
        }
    }

    pub fn indent(&mut self) {
        let indent = match self.tabs_required {
            false => self.tab_substitution.as_str(),
            true => "\t",
        };
        let mut buf = self.buffer.borrow_mut();

        buf.log_undo(self.cursor, self.cursor_column);

        match self.selection {
            None => {
                let mut rope = buf.rope.get_mut();
                if let Ok(line_start) = rope
                    .try_char_to_line(self.cursor)
                    .and_then(|line| rope.try_line_to_char(line))
                {
                    rope.insert(line_start, indent);
                    self.cursor += indent.len();
                }
            }
            selection @ Some(_) => {
                let mut rope = buf.rope.get_mut();
                let indent_lines = selected_lines(&rope, self.cursor, selection)
                    .filter(|l| l.end > l.start)
                    .collect::<Vec<_>>();

                for SelectedLine { start, .. } in indent_lines.iter().rev() {
                    rope.insert(*start, indent);
                }

                self.selection = indent_lines.first().map(|l| l.start);
                self.cursor = indent_lines
                    .last()
                    .map(|l| l.end + (indent.len() * indent_lines.len()))
                    .unwrap_or(0);
            }
        }
    }

    pub fn un_indent(&mut self) {
        let indent = match self.tabs_required {
            false => self.tab_substitution.as_str(),
            true => "\t",
        };
        let mut buf = self.buffer.borrow_mut();

        match self.selection {
            None => {
                if let Some(line_start) = buf
                    .rope
                    .try_char_to_line(self.cursor)
                    .ok()
                    .and_then(|line| buf.rope.try_line_to_char(line).ok())
                    && buf
                        .rope
                        .chars_at(line_start)
                        .take(indent.len())
                        .eq(indent.chars())
                {
                    buf.log_undo(self.cursor, self.cursor_column);
                    let mut rope = buf.rope.get_mut();
                    rope.remove(line_start..line_start + indent.len());
                    self.cursor = line_start;
                    self.cursor_column = 0;
                }
            }
            selection @ Some(_) => {
                let unindent_lines = selected_lines(&buf.rope, self.cursor, selection)
                    .filter(|l| l.end > l.start)
                    .collect::<Vec<_>>();

                // un-indent whole selection as a unit
                // so long as each non-empty line has the proper amount
                // of prefixed spaces
                if unindent_lines.iter().all(|SelectedLine { start, .. }| {
                    buf.rope
                        .chars_at(*start)
                        .take(indent.len())
                        .eq(indent.chars())
                }) {
                    buf.log_undo(self.cursor, self.cursor_column);
                    let mut rope = buf.rope.get_mut();

                    for line in unindent_lines.iter().rev() {
                        rope.remove(line.start..line.start + indent.len());
                    }

                    self.selection = unindent_lines.first().map(|l| l.start);
                    self.cursor = unindent_lines
                        .last()
                        .map(|l| l.end - (unindent_lines.len() * indent.len()))
                        .unwrap_or(0);
                }
            }
        }
    }

    pub fn select_inside(&mut self, (start, end): (char, char), stack: Option<(char, char)>) {
        let buf = self.buffer.borrow();
        let (stack_back, stack_forward) = match stack {
            Some((back, forward)) => (Some(back), Some(forward)),
            None => (None, None),
        };
        if let (Some(start), Some(end)) = (
            select_next_char::<false>(&buf.rope, self.cursor, start, stack_back),
            select_next_char::<true>(&buf.rope, self.cursor, end, stack_forward),
        ) {
            self.selection = Some(start);
            self.cursor = end;
        }
    }

    pub fn cursor_to_selection_start(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        if let Some(selection) = &mut self.selection
            && self.cursor > *selection
        {
            std::mem::swap(selection, &mut self.cursor);
            self.cursor_column = cursor_column(&buf.rope, self.cursor);
            log_movement(&mut buf.undo);
        }
    }

    pub fn cursor_to_selection_end(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        if let Some(selection) = &mut self.selection
            && self.cursor < *selection
        {
            std::mem::swap(selection, &mut self.cursor);
            self.cursor_column = cursor_column(&buf.rope, self.cursor);
            log_movement(&mut buf.undo);
        }
    }

    pub fn select_matching_paren(&mut self) {
        let mut buf = self.buffer.borrow_mut();

        if let Some(new_pos) = buf.rope.get_char(self.cursor).and_then(|c| match c {
            '(' => select_next_char::<true>(&buf.rope, self.cursor + 1, ')', Some('(')),
            ')' => select_next_char::<false>(&buf.rope, self.cursor, '(', Some(')'))
                .map(|c| c.saturating_sub(1)),
            '{' => select_next_char::<true>(&buf.rope, self.cursor + 1, '}', Some('{')),
            '}' => select_next_char::<false>(&buf.rope, self.cursor, '{', Some('}'))
                .map(|c| c.saturating_sub(1)),
            '[' => select_next_char::<true>(&buf.rope, self.cursor + 1, ']', Some('[')),
            ']' => select_next_char::<false>(&buf.rope, self.cursor, '[', Some(']'))
                .map(|c| c.saturating_sub(1)),
            '<' => select_next_char::<true>(&buf.rope, self.cursor + 1, '>', Some('<')),
            '>' => select_next_char::<false>(&buf.rope, self.cursor, '<', Some('>'))
                .map(|c| c.saturating_sub(1)),
            _ => None,
        }) {
            log_movement(&mut buf.undo);
            self.cursor = new_pos;
            self.selection = None;
        }
    }

    pub fn select_whole_lines(&mut self) {
        let buf = &mut self.buffer.borrow_mut();
        let rope = &buf.rope;

        match self.selection {
            None => {
                // no selection, so select current line instead

                if let Some((start, end)) = rope
                    .try_char_to_line(self.cursor)
                    .ok()
                    .and_then(|line| line_char_range(rope, line))
                {
                    self.selection = Some(start);
                    self.cursor = end;
                    self.cursor_column = cursor_column(rope, self.cursor);
                    log_movement(&mut buf.undo);
                }
            }
            Some(selection) => {
                // widen start and end of selection to line boundaries
                if selection < self.cursor {
                    // selection to start of line, cursor to end of line
                    if let Some(start) = rope
                        .try_char_to_line(selection)
                        .ok()
                        .and_then(|line| rope.try_line_to_char(line).ok())
                        && let Some(end) = rope
                            .try_char_to_line(self.cursor)
                            .ok()
                            .and_then(|line| rope.try_line_to_char(line + 1).ok())
                    {
                        self.selection = Some(start);
                        self.cursor = end - 1;
                    }
                } else {
                    // cursor to start of line, selection to end of line
                    if let Some(start) = rope
                        .try_char_to_line(self.cursor)
                        .ok()
                        .and_then(|line| rope.try_line_to_char(line).ok())
                        && let Some(end) = rope
                            .try_char_to_line(selection)
                            .ok()
                            .and_then(|line| rope.try_line_to_char(line + 1).ok())
                    {
                        self.cursor = start;
                        self.selection = Some(end - 1);
                    }
                }
            }
        }
    }

    /// Given search term, returns all match ranges as characters
    /// If selection is active, matches are restricted to selection
    pub fn matches(&self, term: &str) -> Vec<(usize, usize)> {
        let rope = &self.buffer.borrow().rope;

        // combine rope or rope slice into unified String
        let (whole, byte_offset) = match self.selection {
            None => (rope.chunks().collect::<String>(), 0),
            Some(selection) => {
                let (start, end) = reorder(self.cursor, selection);
                (
                    rope.slice(start..end).chunks().collect(),
                    rope.char_to_byte(start),
                )
            }
        };

        // get byte ranges of matches and convert them to character offsets
        whole
            .match_indices(term)
            .map(|(start_byte, s)| (byte_offset + start_byte, byte_offset + start_byte + s.len()))
            .filter_map(|(s, e)| {
                Some((
                    rope.try_byte_to_char(s).ok()?,
                    rope.try_byte_to_char(e).ok()?,
                ))
            })
            .collect()
    }

    pub fn replace(&mut self, ranges: &[(usize, usize)], to: &str) {
        let mut buf = self.buffer.borrow_mut();
        buf.log_undo(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        for (s, e) in ranges.iter().rev() {
            let _ = rope.try_remove(s..e);
            let _ = rope.try_insert(*s, to);
        }
        self.selection = None;
    }

    pub fn set_error<S: Into<Cow<'static, str>>>(&mut self, err: S) {
        self.message = Some(BufferMessage::Error(err.into()))
    }
}

// Given line in rope, returns (start, end) of that line in characters from start of rope
fn line_char_range(rope: &ropey::Rope, line: usize) -> Option<(usize, usize)> {
    Some((
        rope.try_line_to_char(line).ok()?,
        rope.try_line_to_char(line + 1).ok()? - 1,
    ))
}

struct SelectedLine {
    start: usize,
    end: usize,
}

// Iterates over position ranges of all selected lines
//
// If no selection, yields current line's position ranges
fn selected_lines(
    rope: &ropey::Rope,
    cursor: usize,
    selection: Option<usize>,
) -> Box<dyn DoubleEndedIterator<Item = SelectedLine> + '_> {
    match selection {
        // select current line
        None => match rope.try_char_to_line(cursor) {
            Ok(line) => Box::new(
                line_char_range(rope, line)
                    .map(|(start, end)| SelectedLine { start, end })
                    .into_iter(),
            ),
            Err(_) => Box::new(std::iter::empty()),
        },
        Some(selection) => {
            let (start, end) = reorder(cursor, selection);
            if let Ok(start_line) = rope.try_char_to_line(start)
                && let Ok(end_line) = rope.try_char_to_line(end)
            {
                Box::new((start_line..=end_line).filter_map(move |line| {
                    line_char_range(rope, line).map(|(start, end)| SelectedLine { start, end })
                }))
            } else {
                Box::new(std::iter::empty())
            }
        }
    }
}

// Given cursor position from start of rope,
// return that cursor's column in line
fn cursor_column(rope: &ropey::Rope, cursor: usize) -> usize {
    rope.try_char_to_line(cursor)
        .ok()
        .and_then(|line| rope.try_line_to_char(line).ok())
        .and_then(|line_start| cursor.checked_sub(line_start))
        .unwrap_or(0)
}

// Returns characters from the cursor's line start
// up to (not not including) the cursor itself
fn line_start_to_cursor(rope: &ropey::Rope, cursor: usize) -> Option<impl Iterator<Item = char>> {
    let line = rope.try_char_to_line(cursor).ok()?;
    let start = rope.try_line_to_char(line).ok()?;
    rope.get_chars_at(start)
        .map(|iter| iter.take(cursor - start))
}

// If we move the cursor without performing a selection, clear the selection
// If we move the cursor while performing a selection, set the selection if necessary
fn update_selection(selection: &mut Option<usize>, cursor: usize, selecting: bool) {
    if selecting && selection.is_none() {
        *selection = Some(cursor);
    } else if !selecting && selection.is_some() {
        *selection = None
    }
}

fn zap_selection(rope: &mut ropey::Rope, cursor: &mut usize, column: &mut usize, selection: usize) {
    let (selection_start, selection_end) = reorder(*cursor, selection);
    if rope.try_remove(selection_start..selection_end).is_ok() {
        *cursor = selection_start;
        *column = cursor_column(rope, *cursor);
    }
}

fn select_next_char<const FORWARD: bool>(
    rope: &ropey::Rope,
    cursor: usize,
    target: char,
    stack: Option<char>,
) -> Option<usize> {
    let mut chars = rope.chars_at(cursor);
    if !FORWARD {
        chars.reverse();
    }
    match stack {
        None => chars
            .position(|c| c == target)
            .map(|pos| if FORWARD { cursor + pos } else { cursor - pos }),
        Some(stack) => {
            let mut stacked = 0;
            chars
                .enumerate()
                .filter(|(_, c)| {
                    if *c == target {
                        if stacked > 0 {
                            stacked -= 1;
                            false
                        } else {
                            true
                        }
                    } else if *c == stack {
                        stacked += 1;
                        true
                    } else {
                        true
                    }
                })
                .find_map(|(idx, c)| (c == target).then_some(idx))
                .map(|pos| if FORWARD { cursor + pos } else { cursor - pos })
        }
    }
}

impl From<Buffer> for BufferContext {
    fn from(buffer: Buffer) -> Self {
        use crate::syntax::Highlighter;

        let spaces_per_tab: usize = std::env::var("VLE_SPACES_PER_TAB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);

        Self {
            tab_substitution: std::iter::repeat_n(' ', spaces_per_tab).collect(),
            tabs_required: buffer.syntax.tabs_required(),
            buffer: Rc::new(RefCell::new(buffer)),
            viewport_height: 0,
            cursor: 0,
            cursor_column: 0,
            selection: None,
            message: None,
        }
    }
}

/// A set of buffer contexts on a per-view basis
#[derive(Clone, Default)]
pub struct BufferList {
    buffers: Vec<BufferContext>,
    // if we have any buffers at all,
    // must be a valid index pointing to one of our buffers
    current: usize,
}

impl BufferList {
    pub fn new(paths: impl IntoIterator<Item = OsString>) -> std::io::Result<Self> {
        // TODO - if buffers are empty, open an unnamed scratch buffer
        Ok(Self {
            buffers: paths
                .into_iter()
                .map(|p| Buffer::open(p).map(BufferContext::from))
                .collect::<Result<_, _>>()?,
            current: 0,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    pub fn push(&mut self, buffer: BufferContext, select: bool) {
        self.buffers.push(buffer);
        if select {
            // must always be at least one buffer present,
            // so this cannot fail
            self.current = self.buffers.len() - 1;
        }
    }

    pub fn remove(&mut self, buffer: &BufferId) {
        self.buffers
            .retain(|buf| !Rc::ptr_eq(&buf.buffer, &buffer.0));
        self.current = self.current.min(self.buffers.len()).saturating_sub(1);
    }

    pub fn current(&self) -> Option<&BufferContext> {
        self.buffers.get(self.current)
    }

    pub fn current_mut(&mut self) -> Option<&mut BufferContext> {
        self.buffers.get_mut(self.current)
    }

    pub fn next_buffer(&mut self) {
        if !self.buffers.is_empty() {
            self.current = (self.current + 1) % self.buffers.len()
        }
    }

    pub fn previous_buffer(&mut self) {
        if !self.buffers.is_empty() {
            self.current = self
                .current
                .checked_sub(1)
                .unwrap_or(self.buffers.len() - 1);
        }
    }

    /// Returns cursor's position relative to the viewport as (row, col)
    ///
    /// The cursor should be centered in the viewport unless
    /// at the very beginning of the file.
    pub fn cursor_viewport_position(&self) -> Option<(usize, usize)> {
        let buf = self.current()?;
        buf.cursor_position()
            .map(|(row, col)| ((buf.viewport_height / 2).min(row), col))
    }

    pub fn update_buf(&mut self, f: impl FnOnce(&mut BufferContext)) {
        if let Some(buf) = self.current_mut() {
            f(buf);
        }
    }

    /// Attempts to select buffer by file name
    /// Returns Ok on success, Err on failure
    pub fn select_by_name(&mut self, name: &OsStr) -> Result<(), ()> {
        match self
            .buffers
            .iter()
            .position(|buf| buf.buffer.borrow().source_str() == name)
        {
            Some(idx) => {
                self.current = idx;
                Ok(())
            }
            None => Err(()),
        }
    }
}

pub struct BufferWidget<'e> {
    pub mode: Option<&'e EditorMode>,
}

impl StatefulWidget for BufferWidget<'_> {
    type State = BufferContext;

    fn render(
        self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        state: &mut BufferContext,
    ) {
        use crate::help::{
            CONFIRM_CLOSE, FIND, OPEN_FILE, REPLACE, REPLACE_WITH, SELECT_INSIDE, SELECT_LINE,
            SELECT_MATCHES,
        };
        use crate::syntax::Highlighter;
        use ratatui::{
            layout::{
                Constraint::{Length, Min},
                Layout,
            },
            style::{Color, Modifier, Style},
            text::{Line, Span},
            widgets::{
                Block, BorderType, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
                Widget,
            },
        };
        use std::borrow::Cow;
        use std::collections::VecDeque;

        const EDITING: Style = Style::new().add_modifier(Modifier::REVERSED);
        const HIGHLIGHTED: Style = Style::new()
            .fg(Color::Yellow)
            .add_modifier(Modifier::REVERSED);

        fn widen_tabs<'l>(mut input: Line<'l>, tab_substitution: &str) -> Line<'l> {
            fn tabs_to_spaces(s: &mut Cow<'_, str>, tab_substitution: &str) {
                if s.as_ref().contains('\t') {
                    *s = Cow::Owned(s.as_ref().replace('\t', tab_substitution));
                }
            }

            input
                .spans
                .iter_mut()
                .for_each(|s| tabs_to_spaces(&mut s.content, tab_substitution));
            input
        }

        // Colorize syntax of the given text
        fn colorize<'s, S: Highlighter>(syntax: &S, text: Cow<'s, str>) -> Vec<Span<'s>> {
            fn colorize_str<'s, S: Highlighter>(syntax: &S, text: &'s str) -> Vec<Span<'s>> {
                let mut elements = vec![];
                let mut idx = 0;
                for (color, range) in syntax.highlight(text) {
                    if idx < range.start {
                        elements.push(Span::raw(&text[idx..range.start]));
                    }
                    elements.push(Span::styled(
                        &text[range.clone()],
                        Style::default().fg(color),
                    ));
                    idx = range.end;
                }
                let last = &text[idx..];
                if !last.is_empty() {
                    elements.push(Span::raw(last));
                }
                elements
            }

            fn colorize_string<S: Highlighter>(syntax: &S, text: String) -> Vec<Span<'static>> {
                let mut elements = vec![];
                let mut idx = 0;
                for (color, range) in syntax.highlight(&text) {
                    if idx < range.start {
                        elements.push(Span::raw(text[idx..range.start].to_string()));
                    }
                    elements.push(Span::styled(
                        text[range.clone()].to_string(),
                        Style::default().fg(color),
                    ));
                    idx = range.end;
                }
                let last = text[idx..].to_string();
                if !last.is_empty() {
                    elements.push(Span::raw(last));
                }
                elements
            }

            match text {
                Cow::Borrowed(s) => colorize_str(syntax, s),
                Cow::Owned(s) => colorize_string(syntax, s),
            }
        }

        fn extract<'s>(
            colorized: &mut VecDeque<Span<'s>>,
            mut characters: usize,
            output: &mut Vec<Span<'s>>,
            map: impl Fn(Span<'s>) -> Span<'s>,
        ) {
            use unicode_truncate::UnicodeTruncateStr;

            while characters > 0 {
                let Some(span) = colorized.pop_front() else {
                    return;
                };
                let span_width = span.width();
                if span_width <= characters {
                    characters -= span_width;
                    output.push(map(span));
                } else {
                    let mut s = span.content.into_owned();
                    let (split, _) = s.unicode_truncate(characters);
                    let suffix = s.split_off(split.len());
                    colorized.push_front(Span {
                        style: span.style,
                        content: suffix.into(),
                    });
                    output.push(map(Span {
                        style: span.style,
                        content: s.into(),
                    }));
                    return;
                }
            }
        }

        // Takes syntaxed-colorized line of text along with
        // highlighted match ranges (in ascending order)
        // and returns text in those ranges highlighted in blue
        fn highlight_matches<'s>(
            colorized: Vec<Span<'s>>,
            (line_start, line_end): (usize, usize),
            matches: &mut VecDeque<(usize, usize)>,
        ) -> Vec<Span<'s>> {
            // A trivial abstraction to make working
            // simultaneously with both line and matche ranges
            // more intuitive.
            struct IntRange {
                start: usize,
                end: usize,
            }

            impl From<(usize, usize)> for IntRange {
                #[inline]
                fn from((start, end): (usize, usize)) -> Self {
                    Self { start, end }
                }
            }

            impl From<IntRange> for (usize, usize) {
                #[inline]
                fn from(IntRange { start, end }: IntRange) -> Self {
                    (start, end)
                }
            }

            impl IntRange {
                #[inline]
                fn is_empty(&self) -> bool {
                    self.start == self.end
                }

                #[inline]
                fn remaining(&self) -> usize {
                    self.end - self.start
                }

                #[inline]
                fn take(&mut self, requested: usize) -> usize {
                    let to_extract = requested.min(self.remaining());
                    self.start += to_extract;
                    to_extract
                }

                #[inline]
                fn take_both(&mut self, other: &mut Self, requested: usize) -> usize {
                    let to_extract = requested.min(self.remaining().min(other.remaining()));
                    self.start += to_extract;
                    other.start += to_extract;
                    to_extract
                }
            }

            let mut colorized = VecDeque::from(colorized);
            let mut highlighted = Vec::with_capacity(colorized.len());
            let mut line_range = IntRange {
                start: line_start,
                end: line_end,
            };

            while !line_range.is_empty() {
                let Some(mut match_range) = matches.pop_front().map(IntRange::from) else {
                    // if there's no remaining matches,
                    // there's nothing left to highlight
                    highlighted.extend(colorized);
                    return highlighted;
                };

                // if match ending is before start of line, just drop it
                if match_range.end < line_range.start {
                    continue;
                }
                // if match starts before start of line,
                // bump match range start up accordingly
                if match_range.start < line_range.start {
                    match_range.start = line_range.start;
                }

                // output line_start to match_start verbatim
                extract(
                    &mut colorized,
                    line_range.take(match_range.start - line_range.start),
                    &mut highlighted,
                    |span| span,
                );

                // output as much of highlighted match as possible
                extract(
                    &mut colorized,
                    match_range.take_both(&mut line_range, match_range.remaining()),
                    &mut highlighted,
                    |span| span.style(HIGHLIGHTED),
                );

                // push any remaining partial match back into VecDeque
                if !match_range.is_empty() {
                    matches.push_front(match_range.into());
                }
            }

            highlighted.extend(colorized);
            highlighted
        }

        // Takes syntax-colorized line of text and returns
        // portion highlighted, if necessary
        fn highlight_selection<'s>(
            colorized: Vec<Span<'s>>,
            (line_start, line_end): (usize, usize),
            (selection_start, selection_end): (usize, usize),
        ) -> Line<'s> {
            if selection_end <= line_start || selection_start >= line_end {
                colorized.into()
            } else {
                let mut colorized = VecDeque::from(colorized);
                let mut highlighted = Vec::with_capacity(colorized.len());

                // output line_start to selection_start characters verbatim
                extract(
                    &mut colorized,
                    selection_start.saturating_sub(line_start),
                    &mut highlighted,
                    |span| span,
                );

                // output selection_start to selection_end characters highlighted
                extract(
                    &mut colorized,
                    selection_end - selection_start.max(line_start),
                    &mut highlighted,
                    |span| span.style(EDITING),
                );

                // output any remaining characters verbatim
                highlighted.extend(colorized);

                highlighted.into()
            }
        }

        let buffer = state.buffer.borrow();
        let rope = &buffer.rope;
        let syntax = &buffer.syntax;

        let block = Block::bordered()
            .border_type(if self.mode.is_some() {
                BorderType::Thick
            } else {
                BorderType::Plain
            })
            .title_top(if buffer.modified() {
                Line::from(buffer.source.name()).style(Style::default().fg(Color::Blue))
            } else {
                Line::from(buffer.source.name())
            })
            .title_bottom(
                Line::from(match buffer.rope.try_char_to_line(state.cursor) {
                    Ok(line) => (line + 1).to_string(),
                    Err(_) => "???".to_string(),
                })
                .centered(),
            );

        let [text_area, scrollbar_area] =
            Layout::horizontal([Min(0), Length(1)]).areas(block.inner(area));

        block.render(area, buf);

        let viewport_line: usize = rope
            .try_char_to_line(state.cursor)
            .map(|line| line.saturating_sub(state.viewport_height / 2))
            .unwrap_or(0);

        state.viewport_height = text_area.height.into();

        Paragraph::new(match self.mode {
            Some(
                EditorMode::ReplaceWith { matches, .. } | EditorMode::SelectMatches { matches, .. },
            ) => {
                let mut matches = matches.iter().copied().collect();
                match state.selection {
                    // no selection, so highlight matches only
                    None => rope
                        .lines_at(viewport_line)
                        .zip(viewport_line..)
                        .map(
                            |(line, line_number)| match line_char_range(rope, line_number) {
                                None => Line::from(colorize(syntax, Cow::from(line))),
                                Some((line_start, line_end)) => highlight_matches(
                                    colorize(syntax, Cow::from(line)),
                                    (line_start, line_end),
                                    &mut matches,
                                )
                                .into(),
                            },
                        )
                        .map(|line| widen_tabs(line, &state.tab_substitution))
                        .take(area.height.into())
                        .collect::<Vec<_>>(),
                    // highlight both matches *and* selection
                    Some(selection) => {
                        let (selection_start, selection_end) = reorder(state.cursor, selection);

                        rope.lines_at(viewport_line)
                            .zip(viewport_line..)
                            .map(
                                |(line, line_number)| match line_char_range(rope, line_number) {
                                    None => Line::from(colorize(syntax, Cow::from(line))),
                                    Some((line_start, line_end)) => highlight_selection(
                                        highlight_matches(
                                            colorize(syntax, Cow::from(line)),
                                            (line_start, line_end),
                                            &mut matches,
                                        ),
                                        (line_start, line_end),
                                        (selection_start, selection_end),
                                    ),
                                },
                            )
                            .map(|line| widen_tabs(line, &state.tab_substitution))
                            .take(area.height.into())
                            .collect::<Vec<_>>()
                    }
                }
            }
            _ => {
                match state.selection {
                    // no selection, so nothing to highlight
                    None => rope
                        .lines_at(viewport_line)
                        .map(|line| Line::from(colorize(syntax, Cow::from(line))))
                        .map(|line| widen_tabs(line, &state.tab_substitution))
                        .take(area.height.into())
                        .collect::<Vec<_>>(),
                    // highlight whole line, no line, or part of the line
                    Some(selection) => {
                        let (selection_start, selection_end) = reorder(state.cursor, selection);

                        rope.lines_at(viewport_line)
                            .zip(viewport_line..)
                            .map(
                                |(line, line_number)| match line_char_range(rope, line_number) {
                                    None => Line::from(colorize(syntax, Cow::from(line))),
                                    Some((line_start, line_end)) => highlight_selection(
                                        colorize(syntax, Cow::from(line)),
                                        (line_start, line_end),
                                        (selection_start, selection_end),
                                    ),
                                },
                            )
                            .map(|line| widen_tabs(line, &state.tab_substitution))
                            .take(area.height.into())
                            .collect::<Vec<_>>()
                    }
                }
            }
        })
        .scroll((
            0,
            cursor_column(rope, state.cursor)
                .saturating_sub(text_area.width.into())
                .try_into()
                .unwrap_or(0),
        ))
        .render(text_area, buf);

        Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
            scrollbar_area,
            buf,
            &mut ScrollbarState::new(buffer.total_lines())
                .viewport_content_length(text_area.height.into())
                .position(rope.try_char_to_line(state.cursor).unwrap_or(viewport_line)),
        );

        match self.mode {
            None | Some(EditorMode::Editing) => { /* no dialog to display */ }
            Some(EditorMode::ConfirmClose { .. }) => {
                render_confirmation(
                    text_area,
                    buf,
                    "Unsaved changes. Really quit?",
                    CONFIRM_CLOSE,
                );
            }
            Some(EditorMode::SelectInside) => {
                render_confirmation(text_area, buf, "Select Inside", SELECT_INSIDE);
            }
            Some(EditorMode::SelectLine { prompt }) => {
                render_prompt(text_area, buf, "Line", prompt, SELECT_LINE);
            }
            Some(EditorMode::Open { prompt }) => {
                render_prompt(text_area, buf, "Open File", prompt, OPEN_FILE);
            }
            Some(EditorMode::Find { prompt, .. }) => {
                render_prompt(text_area, buf, "Find", prompt, FIND);
            }
            Some(EditorMode::SelectMatches { .. }) => {
                render_confirmation(text_area, buf, "Select Match", SELECT_MATCHES);
            }
            Some(EditorMode::Replace { replace, .. }) => {
                render_prompt(text_area, buf, "Replace", replace, REPLACE);
            }
            Some(EditorMode::ReplaceWith { with, .. }) => {
                render_prompt(text_area, buf, "Replace With", with, REPLACE_WITH);
            }
        }

        if let Some(message) = state.message.take() {
            render_message(text_area, buf, message);
        }
    }
}

// Given whole outer area and width of dialog in characters,
// returns sub-area for dialog box - including border
pub fn dialog_area(area: ratatui::layout::Rect, width: u16, height: u16) -> ratatui::layout::Rect {
    use ratatui::layout::{
        Constraint::{Length, Min, Ratio},
        Layout,
    };

    let [_, dialog, _] = Layout::horizontal([Min(0), Length(width + 2), Min(0)]).areas(area);
    let [_, dialog] = Layout::vertical([Ratio(2, 3), Ratio(1, 3)]).areas(dialog);
    let [dialog, _] = Layout::vertical([Length(height + 2), Min(0)]).areas(dialog);
    dialog
}

fn render_confirmation(
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
    label: &str,
    keybindings: &[Keybinding],
) {
    use crate::help::{field_widths, help_message};
    use ratatui::{
        layout::{
            Constraint::{Length, Min},
            Layout,
        },
        widgets::{Block, BorderType, Paragraph, Widget},
    };
    use unicode_width::UnicodeWidthStr;

    let dialog_area = dialog_area(
        area,
        label
            .width()
            .max(field_widths(keybindings).into_iter().sum()) as u16,
        (keybindings.len() + 1) as u16,
    );
    let block = Block::bordered().border_type(BorderType::Rounded);
    let [prompt_area, keybindings_area] =
        Layout::vertical([Length(1), Min(0)]).areas(block.inner(dialog_area));
    ratatui::widgets::Clear.render(dialog_area, buf);
    block.render(dialog_area, buf);
    Paragraph::new(label).render(prompt_area, buf);
    help_message(keybindings).render(keybindings_area, buf);
}

fn render_prompt<P: std::fmt::Display>(
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
    label: &str,
    prompt: P,
    keybindings: &[Keybinding],
) {
    render_confirmation(area, buf, &format!("{} : {}", label, prompt), keybindings);
}

fn render_message(
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
    message: BufferMessage,
) {
    use ratatui::{
        layout::{
            Constraint::{Length, Min},
            Layout,
        },
        style::{Color, Style},
        widgets::{Block, BorderType, Paragraph, Widget},
    };
    use unicode_width::UnicodeWidthStr;

    let width = message.as_str().width().try_into().unwrap_or(u16::MAX);
    let [_, dialog_area, _] = Layout::horizontal([Min(0), Length(width + 2), Min(0)]).areas(area);
    let [_, dialog_area, _] = Layout::vertical([Min(0), Length(3), Min(0)]).areas(dialog_area);

    Paragraph::new(message.as_str())
        .style(match message {
            BufferMessage::Notice(_) => Style::default(),
            BufferMessage::Error(_) => Style::default().fg(Color::Red),
        })
        .block(Block::bordered().border_type(BorderType::Rounded))
        .render(dialog_area, buf);
}

pub struct CutBuffer {
    data: String,
    chars_len: usize,
}

impl From<ropey::RopeSlice<'_>> for CutBuffer {
    fn from(slice: ropey::RopeSlice<'_>) -> Self {
        Self {
            data: slice.chunks().collect(),
            chars_len: slice.len_chars(),
        }
    }
}

/// Buffer's undo/redo state
struct BufferState {
    rope: ropey::Rope,
    cursor: usize,
    cursor_column: usize,
}

struct Undo {
    state: BufferState,
    finished: bool, // whether we've done any movement since undo added
}

fn log_movement(undo: &mut [Undo]) {
    if let Some(last) = undo.last_mut() {
        last.finished = true;
    }
}

#[derive(Clone)]
enum BufferMessage {
    Notice(Cow<'static, str>),
    Error(Cow<'static, str>),
}

impl BufferMessage {
    fn as_str(&self) -> &str {
        match self {
            Self::Notice(s) | Self::Error(s) => s.as_ref(),
        }
    }
}

fn reorder<T: Ord>(x: T, y: T) -> (T, T) {
    if x <= y { (x, y) } else { (y, x) }
}
