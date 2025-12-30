use crate::editor::EditorMode;
use crate::syntax::Syntax;
use ratatui::widgets::StatefulWidget;
use std::borrow::Cow;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

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
    fn name(&self) -> Cow<'_, str> {
        match self {
            Self::File(path) => path.to_string_lossy(),
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

/// A buffer corresponding to a file on disk (either local or remote)
struct Buffer {
    source: Source,         // the source file
    rope: ropey::Rope,      // the data rope
    modified: bool,         // whether buffer has been modified since last save
    undo: Vec<Undo>,        // the undo stack
    redo: Vec<BufferState>, // the redo stack
    syntax: Syntax,         // the syntax highlighting to use
}

impl Buffer {
    fn open(path: OsString) -> std::io::Result<Self> {
        let source = Source::from(path);

        Ok(Self {
            rope: source.read_data()?,
            syntax: Syntax::new(&source),
            source,
            modified: false,
            undo: vec![],
            redo: vec![],
        })
    }

    fn save(&mut self, message: &mut Option<BufferMessage>) {
        match self.source.save_data(&self.rope) {
            Ok(()) => {
                self.modified = false;
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

    fn log_undo(&mut self, cursor: usize, cursor_column: usize) {
        log_undo(
            &mut self.undo,
            &mut self.redo,
            &self.rope,
            self.modified,
            cursor,
            cursor_column,
        );
    }
}

#[derive(Clone)]
pub struct BufferId(Arc<RwLock<Buffer>>);

/// A buffer with additional context on a per-view basis
#[derive(Clone)]
pub struct BufferContext {
    buffer: Arc<RwLock<Buffer>>,
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
        BufferId(Arc::clone(&self.buffer))
    }

    pub fn modified(&self) -> bool {
        self.buffer.try_read().unwrap().modified
    }

    pub fn save(&mut self) {
        self.buffer.try_write().unwrap().save(&mut self.message);
    }

    pub fn set_selection(&mut self, start: usize, end: usize) {
        assert!(end >= start);
        let buf = self.buffer.try_read().unwrap();
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
        let rope = &self.buffer.try_read().unwrap().rope;
        let line = rope.try_char_to_line(self.cursor).ok()?;
        let line_start = rope.try_line_to_char(line).ok()?;

        // TODO - account for tabs between line_start and cursor
        Some((line, self.cursor.checked_sub(line_start)?))
    }

    pub fn cursor_up(&mut self, lines: usize, selecting: bool) {
        let mut buf = self.buffer.try_write().unwrap();
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
        let mut buf = self.buffer.try_write().unwrap();
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
        let mut buf = self.buffer.try_write().unwrap();
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = self.cursor.saturating_sub(1);
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
        log_movement(&mut buf.undo);
    }

    pub fn cursor_forward(&mut self, selecting: bool) {
        let mut buf = self.buffer.try_write().unwrap();
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = (self.cursor + 1).min(buf.rope.len_chars());
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
        log_movement(&mut buf.undo);
    }

    pub fn cursor_home(&mut self, selecting: bool) {
        let mut buf = self.buffer.try_write().unwrap();
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
        let mut buf = self.buffer.try_write().unwrap();
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
        self.buffer
            .try_write()
            .unwrap()
            .rope
            .len_lines()
            .saturating_sub(1)
    }

    pub fn select_line(&mut self, line: usize) {
        let mut buf = self.buffer.try_write().unwrap();
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
        let mut buf = self.buffer.try_write().unwrap();
        buf.log_undo(self.cursor, self.cursor_column);
        if let Some(selection) = self.selection.take() {
            zap_selection(
                &mut buf.rope,
                &mut self.cursor,
                &mut self.cursor_column,
                selection,
            );
        }
        buf.rope.insert_char(self.cursor, c);
        buf.modified = true;
        self.cursor += 1;
        self.cursor_column += 1;
    }

    pub fn paste(&mut self, pasted: &CutBuffer) {
        let mut buf = self.buffer.try_write().unwrap();
        buf.log_undo(self.cursor, self.cursor_column);
        if let Some(selection) = self.selection.take() {
            zap_selection(
                &mut buf.rope,
                &mut self.cursor,
                &mut self.cursor_column,
                selection,
            );
            buf.modified = true;
        }
        if buf.rope.try_insert(self.cursor, &pasted.data).is_ok() {
            self.cursor += pasted.chars_len;
            self.cursor_column = cursor_column(&buf.rope, self.cursor);
            buf.modified = true;
        }
    }

    pub fn newline(&mut self) {
        // TODO - zap selection before inserting newline
        let mut buf = self.buffer.try_write().unwrap();

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

        // if the whole line is indent, insert newline *before* indent
        // instead of adding a fresh indentation
        if all_indent {
            buf.rope.insert_char(self.cursor - indent, '\n');
            self.cursor += 1;
        } else {
            buf.rope.insert_char(self.cursor, '\n');
            self.cursor += 1;
            self.cursor_column = 0;
            for _ in 0..indent {
                buf.rope.insert_char(self.cursor, ' ');
                self.cursor += 1;
                self.cursor_column += 1;
            }
        }
        buf.modified = true;
    }

    pub fn backspace(&mut self) {
        let mut buf = self.buffer.try_write().unwrap();

        buf.log_undo(self.cursor, self.cursor_column);

        match self.selection.take() {
            None => {
                if let Some(prev) = self.cursor.checked_sub(1)
                    && buf.rope.try_remove(prev..self.cursor).is_ok()
                {
                    self.cursor -= 1;
                    // we need to recalculate the cursor column altogether
                    // in case a newline has been removed
                    self.cursor_column = cursor_column(&buf.rope, self.cursor);
                    buf.modified = true;
                }
            }
            Some(current_selection) => {
                zap_selection(
                    &mut buf.rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    current_selection,
                );
                buf.modified = true;
            }
        }
    }

    pub fn delete(&mut self) {
        let buf = &mut self.buffer.try_write().unwrap();
        buf.log_undo(self.cursor, self.cursor_column);

        match self.selection.take() {
            None => {
                if buf.rope.try_remove(self.cursor..(self.cursor + 1)).is_ok() {
                    // leave cursor position and current column unchanged
                    buf.modified = true;
                }
            }
            Some(current_selection) => {
                zap_selection(
                    &mut buf.rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    current_selection,
                );
                buf.modified = true;
            }
        }
    }

    pub fn get_selection(&mut self) -> Option<CutBuffer> {
        let selection = self.selection.take()?;
        let (selection_start, selection_end) = reorder(self.cursor, selection);
        self.buffer
            .try_read()
            .unwrap()
            .rope
            .get_slice(selection_start..selection_end)
            .map(|r| r.into())
    }

    pub fn take_selection(&mut self) -> Option<CutBuffer> {
        let selection = self.selection.take()?;
        let (selection_start, selection_end) = reorder(self.cursor, selection);
        let mut buf = self.buffer.try_write().unwrap();

        buf.rope
            .get_slice(selection_start..selection_end)
            .map(|r| r.into())
            .inspect(|_| {
                buf.log_undo(self.cursor, self.cursor_column);
                buf.rope.remove(selection_start..selection_end);
                buf.modified = true;
                self.cursor = selection_start;
                self.cursor_column = cursor_column(&buf.rope, self.cursor);
            })
    }

    pub fn perform_undo(&mut self) {
        let mut buf = self.buffer.try_write().unwrap();
        match buf.undo.pop() {
            Some(Undo { mut state, .. }) => {
                std::mem::swap(&mut buf.rope, &mut state.rope);
                std::mem::swap(&mut buf.modified, &mut state.modified);
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
        let mut buf = self.buffer.try_write().unwrap();
        match buf.redo.pop() {
            Some(mut state) => {
                std::mem::swap(&mut buf.rope, &mut state.rope);
                std::mem::swap(&mut buf.modified, &mut state.modified);
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

    pub fn indent(&mut self, spaces: usize) {
        let indent = std::iter::repeat_n(' ', spaces).collect::<String>();
        let mut buf = self.buffer.try_write().unwrap();

        buf.log_undo(self.cursor, self.cursor_column);
        buf.modified = true;

        match self.selection {
            None => {
                if let Ok(line_start) = buf
                    .rope
                    .try_char_to_line(self.cursor)
                    .and_then(|line| buf.rope.try_line_to_char(line))
                {
                    buf.rope.insert(line_start, &indent);
                    self.cursor += spaces;
                }
            }
            selection @ Some(_) => {
                for (start, _) in selected_lines(&buf.rope, self.cursor, selection)
                    .rev()
                    .filter(|(s, e)| e > s)
                    .collect::<Vec<_>>()
                    .into_iter()
                {
                    buf.rope.insert(start, &indent);
                    match &mut self.selection {
                        Some(selection) => {
                            *selection.max(&mut self.cursor) += spaces;
                        }
                        None => {
                            self.cursor += spaces;
                        }
                    }
                }
            }
        }
    }

    pub fn un_indent(&mut self, spaces: usize) {
        let mut buf = self.buffer.try_write().unwrap();

        let selected = selected_lines(&buf.rope, self.cursor, self.selection)
            .filter(|(s, e)| e > s)
            .collect::<Vec<_>>();

        // un-indent whole selection as a unit
        // so long as each has the proper amount of prefixed spaces
        if selected.iter().all(|(start, _)| {
            buf.rope
                .chars_at(*start)
                .take(spaces)
                .eq(std::iter::repeat_n(' ', spaces))
        }) {
            buf.log_undo(self.cursor, self.cursor_column);
            buf.modified = true;

            for (start, _) in selected.into_iter().rev() {
                buf.rope.remove(start..start + spaces);

                match &mut self.selection {
                    Some(selection) => {
                        *selection.max(&mut self.cursor) -= spaces;
                    }
                    None => {
                        self.cursor -= spaces;
                    }
                }
            }
        }
    }

    pub fn select_inside(&mut self, (start, end): (char, char), stack: Option<(char, char)>) {
        let buf = self.buffer.try_read().unwrap();
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

    pub fn select_matching_paren(&mut self) {
        let mut buf = self.buffer.try_write().unwrap();

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
            _ => None,
        }) {
            log_movement(&mut buf.undo);
            self.cursor = new_pos;
            self.selection = None;
        }
    }

    // returns true if search term found
    pub fn search(&mut self, forward: bool, term: &str, cache: &mut String) -> bool {
        let buf = &mut self.buffer.try_write().unwrap();
        if cache.len() != buf.rope.len_bytes() {
            *cache = buf.rope.chunks().collect();
        }

        (self.cursor, self.selection) = match forward {
            true => {
                let Ok(byte_start) = buf.rope.try_char_to_byte(self.cursor + 1) else {
                    return false;
                };
                match cache[byte_start..].find(term) {
                    Some(found_offset) => (
                        buf.rope.byte_to_char(byte_start + found_offset),
                        Some(
                            buf.rope
                                .byte_to_char(byte_start + found_offset + term.len()),
                        ),
                    ),
                    None => return false,
                }
            }
            false => {
                let Ok(byte_start) = buf.rope.try_char_to_byte(self.cursor) else {
                    return false;
                };
                match cache[0..byte_start].rfind(term) {
                    Some(found_offset) => (
                        buf.rope.byte_to_char(found_offset),
                        Some(buf.rope.byte_to_char(found_offset + term.len())),
                    ),
                    None => return false,
                }
            }
        };

        self.cursor_column = cursor_column(&buf.rope, self.cursor);
        log_movement(&mut buf.undo);

        true
    }

    /// Given search term, returns all match ranges as characters
    /// If selection is active, matches are restricted to selection
    pub fn matches(&self, term: &str) -> Vec<(usize, usize)> {
        let rope = &self.buffer.try_read().unwrap().rope;

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
                    rope.try_char_to_byte(s).ok()?,
                    rope.try_char_to_byte(e).ok()?,
                ))
            })
            .collect()
    }

    pub fn replace(&mut self, ranges: &[(usize, usize)], to: &str) {
        let mut buf = self.buffer.try_write().unwrap();
        buf.log_undo(self.cursor, self.cursor_column);
        buf.modified = true;
        for (s, e) in ranges.iter().rev() {
            let _ = buf.rope.try_remove(s..e);
            let _ = buf.rope.try_insert(*s, to);
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

// Iterates over position ranges of all selected lines
//
// If no selection, yields current line's position ranges
fn selected_lines(
    rope: &ropey::Rope,
    cursor: usize,
    selection: Option<usize>,
) -> Box<dyn DoubleEndedIterator<Item = (usize, usize)> + '_> {
    match selection {
        // select current line
        None => match rope.try_char_to_line(cursor) {
            Ok(line) => Box::new(line_char_range(rope, line).into_iter()),
            Err(_) => Box::new(std::iter::empty()),
        },
        Some(selection) => {
            let (start, end) = reorder(cursor, selection);
            if let Ok(start_line) = rope.try_char_to_line(start)
                && let Ok(end_line) = rope.try_char_to_line(end)
            {
                Box::new((start_line..=end_line).filter_map(|l| line_char_range(rope, l)))
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
        Self {
            buffer: Arc::new(RwLock::new(buffer)),
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

    pub fn remove(&mut self, buffer: &BufferId) {
        self.buffers
            .retain(|buf| !Arc::ptr_eq(&buf.buffer, &buffer.0));
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
        use crate::{prompt::PromptWidget, syntax::Highlighter};
        use ratatui::{
            layout::{
                Constraint::{Length, Min},
                Layout,
            },
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
        };
        use std::borrow::Cow;

        fn tabs_to_spaces<'s, S: Into<Cow<'s, str>> + AsRef<str>>(s: S) -> Cow<'s, str> {
            if s.as_ref().contains('\t') {
                s.as_ref().replace('\t', "    ").into()
            } else {
                s.into()
            }
        }

        // Colorize syntax of the given text
        fn colorize<S: Highlighter>(syntax: &S, text: &str) -> Vec<Span<'static>> {
            let mut elements = vec![];
            let mut idx = 0;
            for (color, range) in syntax.highlight(text) {
                if idx < range.start {
                    elements.push(Span::raw(text[idx..range.start].to_string()));
                }
                elements.push(Span::styled(
                    text[range.clone()].to_string(),
                    Style::default().fg(color),
                ));
                idx = range.end;
            }
            let last = &text[idx..];
            if !last.is_empty() {
                elements.push(Span::raw(last.to_string()));
            }
            elements
        }

        // Takes syntax-colorized line of text and returns
        // portion highlighted, if necessary
        fn highlight_selection(
            colorized: Vec<Span<'static>>,
            (line_start, line_end): (usize, usize),
            (selection_start, selection_end): (usize, usize),
        ) -> Line<'static> {
            if selection_end <= line_start || selection_start >= line_end {
                colorized.into()
            } else {
                use std::collections::VecDeque;

                fn extract(
                    colorized: &mut VecDeque<Span<'static>>,
                    mut characters: usize,
                    output: &mut Vec<Span<'static>>,
                    map: impl Fn(Span<'static>) -> Span<'static>,
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

                let mut colorized = VecDeque::from(colorized);
                let mut highlighted = Vec::with_capacity(colorized.len());

                // output selection_start - line_start characters verbatim
                extract(
                    &mut colorized,
                    selection_start.saturating_sub(line_start),
                    &mut highlighted,
                    |span| span,
                );

                // output selection_end - selection_start characters highlighted
                extract(
                    &mut colorized,
                    selection_end - selection_start.max(line_start),
                    &mut highlighted,
                    |span| span.style(REVERSED),
                );

                // output the remaining characters verbatim
                highlighted.extend(colorized);

                highlighted.into()
            }
        }

        const REVERSED: Style = Style::new().add_modifier(Modifier::REVERSED);

        let [text_area, status_area] = Layout::vertical([Min(0), Length(1)]).areas(area);
        let [text_area, scrollbar_area] = Layout::horizontal([Min(0), Length(1)]).areas(text_area);

        state.viewport_height = text_area.height.into();

        let buffer = state.buffer.try_read().unwrap();
        let rope = &buffer.rope;
        let syntax = &buffer.syntax;

        // ensure cursor hasn't been shifted outside of rope
        // (which might occur if the rope is shrunk in another buffer)
        if state.cursor > rope.len_chars() {
            state.cursor = rope.len_chars().saturating_sub(1);
            state.selection = None;
        }

        let viewport_line: usize = rope
            .try_char_to_line(state.cursor)
            .map(|line| line.saturating_sub(state.viewport_height / 2))
            .unwrap_or(0);

        Paragraph::new(match state.selection {
            // no selection, so nothing to highlight
            None => rope
                .lines_at(viewport_line)
                .map(|line| Line::from(colorize(syntax, &tabs_to_spaces(Cow::from(line)))))
                .take(area.height.into())
                .collect::<Vec<_>>(),
            // highlight whole line, no line, or part of the line
            Some(selection) => {
                let (selection_start, selection_end) = reorder(state.cursor, selection);

                rope.lines_at(viewport_line)
                    .zip(viewport_line..)
                    .map(
                        |(line, line_number)| match line_char_range(rope, line_number) {
                            None => Line::from(colorize(syntax, &tabs_to_spaces(Cow::from(line)))),
                            Some((line_start, line_end)) => highlight_selection(
                                colorize(syntax, &tabs_to_spaces(Cow::from(line))),
                                (line_start, line_end),
                                (selection_start, selection_end),
                            ),
                        },
                    )
                    .take(area.height.into())
                    .collect::<Vec<_>>()
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
            None | Some(EditorMode::Editing) => match state.message.take() {
                None => {
                    let source = Paragraph::new(format!(
                        "{} {}",
                        match buffer.modified {
                            true => '*',
                            false => ' ',
                        },
                        buffer.source.name()
                    ))
                    .style(REVERSED);

                    match buffer.rope.try_char_to_line(state.cursor) {
                        Ok(line) => {
                            let line = std::num::NonZero::new(line + 1).unwrap();
                            let digits = line.ilog10() + 1;

                            let [source_area, line_area] = Layout::horizontal([
                                Min(0),
                                Length((digits + 1).try_into().unwrap()),
                            ])
                            .areas(status_area);

                            source.render(source_area, buf);

                            Paragraph::new(line.to_string())
                                .style(REVERSED)
                                .render(line_area, buf);
                        }
                        Err(_) => {
                            source.render(status_area, buf);
                        }
                    }
                }
                Some(BufferMessage::Notice(msg)) => {
                    Paragraph::new(msg.into_owned())
                        .style(REVERSED)
                        .render(status_area, buf);
                }
                Some(BufferMessage::Error(msg)) => {
                    Paragraph::new(msg.into_owned())
                        .style(REVERSED.fg(ratatui::style::Color::Red))
                        .render(status_area, buf);
                }
            },
            Some(EditorMode::ConfirmClose { .. }) => {
                Paragraph::new("Unsaved changes. Really quit?")
                    .style(REVERSED)
                    .render(status_area, buf);
            }
            Some(EditorMode::SelectInside) => {
                Paragraph::new("Select Inside")
                    .style(REVERSED)
                    .render(status_area, buf);
            }
            Some(EditorMode::SelectLine { prompt }) => {
                let [label_area, prompt_area] =
                    Layout::horizontal([Length(7), Min(0)]).areas(status_area);

                Paragraph::new("Line : ")
                    .style(REVERSED)
                    .render(label_area, buf);

                PromptWidget { prompt }.render(prompt_area, buf);
            }
            Some(EditorMode::Find { prompt, .. }) => {
                let [label_area, prompt_area] =
                    Layout::horizontal([Length(7), Min(0)]).areas(status_area);

                Paragraph::new("Find : ")
                    .style(REVERSED)
                    .render(label_area, buf);

                PromptWidget { prompt }.render(prompt_area, buf);
            }
            Some(EditorMode::Replace { replace, .. }) => {
                let [label_area, prompt_area] =
                    Layout::horizontal([Length(10), Min(0)]).areas(status_area);

                Paragraph::new("Replace : ")
                    .style(REVERSED)
                    .render(label_area, buf);

                PromptWidget { prompt: replace }.render(prompt_area, buf);
            }
            Some(EditorMode::ReplaceWith { with, matches, .. }) => {
                let matches = match matches.len() {
                    1 => format!("(1 match) "),
                    matches => format!("({matches} matches) "),
                };

                // our labal is ASCII, so its width is easy to calculate
                let [label_area, prompt_area, matches_area] = Layout::horizontal([
                    Length(15),
                    Min(0),
                    Length(matches.len().try_into().unwrap()),
                ])
                .areas(status_area);

                Paragraph::new("Replace With : ")
                    .style(REVERSED)
                    .render(label_area, buf);

                PromptWidget { prompt: with }.render(prompt_area, buf);

                Paragraph::new(matches)
                    .style(REVERSED)
                    .render(matches_area, buf);
            }
        }
    }
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
    modified: bool,
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

fn log_undo(
    undo: &mut Vec<Undo>,
    redo: &mut Vec<BufferState>,
    rope: &ropey::Rope,
    modified: bool,
    cursor: usize,
    cursor_column: usize,
) {
    if let None | Some(Undo { finished: true, .. }) = undo.last() {
        undo.push(Undo {
            state: BufferState {
                rope: rope.clone(),
                modified,
                cursor,
                cursor_column,
            },
            finished: false,
        });
        redo.clear();
    }
}

#[derive(Clone)]
enum BufferMessage {
    Notice(Cow<'static, str>),
    Error(Cow<'static, str>),
}

fn reorder<T: Ord>(x: T, y: T) -> (T, T) {
    if x <= y { (x, y) } else { (y, x) }
}
