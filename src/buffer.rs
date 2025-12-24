use crate::editor::EditorMode;
use ratatui::widgets::StatefulWidget;
use std::borrow::Cow;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

enum Source {
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

    fn read_data(&self) -> std::io::Result<ropey::Rope> {
        use std::fs::File;
        use std::io::BufReader;

        match self {
            Self::File(path) => {
                // TODO - if file doesn't exist, create new rope
                File::open(path).and_then(|f| ropey::Rope::from_reader(BufReader::new(f)))
            }
        }
    }

    // TODO - implement save_data() method
}

/// A buffer corresponding to a file on disk (either local or remote)
struct Buffer {
    source: Source,
    rope: ropey::Rope,
    modified: bool, // whether buffer has been modified since last save
                    // TODO - support undo stack
                    // TODO - support redo stack
}

impl Buffer {
    fn open(path: OsString) -> std::io::Result<Self> {
        let source = Source::from(path);

        Ok(Self {
            rope: source.read_data()?,
            source,
            modified: false,
        })
    }

    fn total_lines(&self) -> usize {
        self.rope.len_lines()
    }
}

#[derive(Clone)]
pub struct BufferId(Arc<RwLock<Buffer>>);

/// A buffer with additional context on a per-view basis
#[derive(Clone)]
pub struct BufferContext {
    buffer: Arc<RwLock<Buffer>>,
    viewport_line: usize,     // viewport's start line (should be <= cursor)
    viewport_height: usize,   // viewport's current height in lines
    cursor: usize,            // cursor's absolute position in rope, in characters
    cursor_column: usize,     // cursor's desired column, in characters
    selection: Option<usize>, // cursor's text selection anchor
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

    fn viewport_up(&mut self, lines: usize) {
        self.viewport_line = self.viewport_line.saturating_sub(lines)
    }

    fn viewport_down(&mut self, lines: usize) {
        self.viewport_line =
            (self.viewport_line + lines).min(self.buffer.try_read().unwrap().total_lines());
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

        Some((line, self.cursor.checked_sub(line_start)?))
    }

    pub fn cursor_up(&mut self, lines: usize, selecting: bool) {
        let rope = &self.buffer.try_read().unwrap().rope;
        if let Ok(current_line) = rope.try_char_to_line(self.cursor) {
            let previous_line = current_line.saturating_sub(lines);
            if let Some((prev_start, prev_end)) = line_char_range(rope, previous_line) {
                update_selection(&mut self.selection, self.cursor, selecting);
                self.cursor = (prev_start + self.cursor_column).min(prev_end);
                viewport_follow_cursor(
                    previous_line,
                    &mut self.viewport_line,
                    self.viewport_height,
                );
            }
        }
    }

    pub fn cursor_down(&mut self, lines: usize, selecting: bool) {
        let rope = &self.buffer.try_read().unwrap().rope;
        if let Ok(current_line) = rope.try_char_to_line(self.cursor) {
            let next_line = (current_line + lines).min(rope.len_lines());
            if let Some((next_start, next_end)) = line_char_range(rope, next_line) {
                update_selection(&mut self.selection, self.cursor, selecting);
                self.cursor = (next_start + self.cursor_column).min(next_end);
                viewport_follow_cursor(next_line, &mut self.viewport_line, self.viewport_height);
            }
        }
    }

    pub fn cursor_back(&mut self, selecting: bool) {
        let rope = &self.buffer.try_read().unwrap().rope;
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = self.cursor.saturating_sub(1);
        self.cursor_column = cursor_column(rope, self.cursor);
        if let Ok(current_line) = rope.try_char_to_line(self.cursor) {
            viewport_follow_cursor(current_line, &mut self.viewport_line, self.viewport_height);
        }
    }

    pub fn cursor_forward(&mut self, selecting: bool) {
        let rope = &self.buffer.try_read().unwrap().rope;
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = (self.cursor + 1).min(rope.len_chars());
        self.cursor_column = cursor_column(rope, self.cursor);
        if let Ok(current_line) = rope.try_char_to_line(self.cursor) {
            viewport_follow_cursor(current_line, &mut self.viewport_line, self.viewport_height);
        }
    }

    pub fn cursor_home(&mut self, selecting: bool) {
        let rope = &self.buffer.try_read().unwrap().rope;
        if let Ok(current_line) = rope.try_char_to_line(self.cursor)
            && let Some((home, _)) = line_char_range(rope, current_line)
        {
            update_selection(&mut self.selection, self.cursor, selecting);
            self.cursor = home;
            self.cursor_column = 0;
        }
    }

    pub fn cursor_end(&mut self, selecting: bool) {
        let rope = &self.buffer.try_read().unwrap().rope;
        if let Ok(current_line) = rope.try_char_to_line(self.cursor)
            && let Some((_, end)) = line_char_range(rope, current_line)
        {
            update_selection(&mut self.selection, self.cursor, selecting);
            self.cursor_column += end - self.cursor;
            self.cursor = end;
        }
    }

    pub fn insert_char(&mut self, c: char) {
        // TODO - perform auto-pairing if char is pair-able
        // TODO - update undo list with current state
        let mut buf = self.buffer.try_write().unwrap();
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
        // TODO - update undo list with current state
        let mut buf = self.buffer.try_write().unwrap();
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

            if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
                viewport_follow_cursor(current_line, &mut self.viewport_line, self.viewport_height);
            }
            buf.modified = true;
        }
    }

    pub fn newline(&mut self) {
        // TODO - update undo list with current state
        // TODO - zap selection before inserting newline
        let mut buf = self.buffer.try_write().unwrap();

        let indent = line_start_to_cursor(&buf.rope, self.cursor)
            .map(|i| i.take_while(|c| *c == ' ').count())
            .unwrap_or(0);

        buf.rope.insert_char(self.cursor, '\n');
        buf.modified = true;
        self.cursor += 1;
        self.cursor_column = 0;
        for _ in 0..indent {
            buf.rope.insert_char(self.cursor, ' ');
            self.cursor += 1;
            self.cursor_column += 1;
        }
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
            viewport_follow_cursor(current_line, &mut self.viewport_line, self.viewport_height);
        }
    }

    pub fn backspace(&mut self) {
        let mut buf = self.buffer.try_write().unwrap();

        match self.selection.take() {
            None => {
                if let Some(prev) = self.cursor.checked_sub(1)
                    && buf.rope.try_remove(prev..self.cursor).is_ok()
                {
                    // TODO - remove auto-pairing if pair is together (like "{}")
                    // TODO - update undo list with current state

                    self.cursor -= 1;
                    // we need to recalculate the cursor column altogether
                    // in case a newline has been removed
                    self.cursor_column = cursor_column(&buf.rope, self.cursor);

                    if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
                        viewport_follow_cursor(
                            current_line,
                            &mut self.viewport_line,
                            self.viewport_height,
                        );
                    }

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

                if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
                    viewport_follow_cursor(
                        current_line,
                        &mut self.viewport_line,
                        self.viewport_height,
                    );
                }

                buf.modified = true;
            }
        }
    }

    pub fn delete(&mut self) {
        let buf = &mut self.buffer.try_write().unwrap();

        match self.selection.take() {
            None => {
                if buf.rope.try_remove(self.cursor..(self.cursor + 1)).is_ok() {
                    // TODO - remove auto-pairing if pair is together (like "{}")
                    // TODO - update undo list with current state
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

                if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
                    viewport_follow_cursor(
                        current_line,
                        &mut self.viewport_line,
                        self.viewport_height,
                    );
                }

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
                buf.rope.remove(selection_start..selection_end);
                buf.modified = true;
                self.cursor = selection_start;
                self.cursor_column = cursor_column(&buf.rope, self.cursor);

                if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
                    viewport_follow_cursor(
                        current_line,
                        &mut self.viewport_line,
                        self.viewport_height,
                    );
                }
            })
    }
}

// Given line in rope, returns (start, end) of that line in characters from start of rope
fn line_char_range(rope: &ropey::Rope, line: usize) -> Option<(usize, usize)> {
    Some((
        rope.try_line_to_char(line).ok()?,
        rope.try_line_to_char(line + 1).ok()? - 1,
    ))
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

fn viewport_follow_cursor(current_line: usize, viewport_line: &mut usize, viewport_height: usize) {
    if *viewport_line > current_line {
        *viewport_line = current_line;
    } else if let Some(max) = current_line.checked_sub(viewport_height - 1)
        && *viewport_line < max
    {
        *viewport_line = max;
    }
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

impl From<Buffer> for BufferContext {
    fn from(buffer: Buffer) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(buffer)),
            viewport_line: 0,
            viewport_height: 0,
            cursor: 0,
            cursor_column: 0,
            selection: None,
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

    pub fn viewport_up(&mut self, lines: usize) {
        if let Some(buf) = self.current_mut() {
            buf.viewport_up(lines);
        }
    }

    pub fn viewport_down(&mut self, lines: usize) {
        if let Some(buf) = self.current_mut() {
            buf.viewport_down(lines);
        }
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

    pub fn cursor_viewport_position(&self) -> Option<(usize, usize)> {
        let buf = self.current()?;
        buf.cursor_position()
            .and_then(|(row, col)| Some((row.checked_sub(buf.viewport_line)?, col)))
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
        use ratatui::{
            layout::{
                Constraint::{Length, Min},
                Layout,
            },
            style::{Modifier, Style},
            text::Line,
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

        // returns selection to be highlighted along with any
        // non-highlighted prefix or suffix
        fn highlight(
            line: Cow<'_, str>,
            (line_start, line_end): (usize, usize),
            (selection_start, selection_end): (usize, usize),
        ) -> Line<'_> {
            use ratatui::text::Span;

            fn pop_chars_front<'s>(s: &mut &'s str, chars: usize) -> &'s str {
                use unicode_truncate::UnicodeTruncateStr;

                let (split, _) = s.unicode_truncate(chars);
                *s = s.split_at(split.len()).1;
                split
            }

            if selection_end <= line_start || selection_start >= line_end {
                Line::from(line)
            } else {
                let mut s = line.as_ref();
                let mut line = vec![];
                let prefix = pop_chars_front(&mut s, selection_start.saturating_sub(line_start));
                line.extend((!prefix.is_empty()).then_some(Span::raw(prefix.to_string())));
                line.push(Span::styled(
                    pop_chars_front(&mut s, selection_end - selection_start.max(line_start))
                        .to_string(),
                    REVERSED,
                ));
                line.extend((!s.is_empty()).then_some(Span::raw(s.to_string())));
                Line::from(line)
            }
        }

        const REVERSED: Style = Style::new().add_modifier(Modifier::REVERSED);

        let [text_area, status_area] = Layout::vertical([Min(0), Length(1)]).areas(area);
        let [text_area, scrollbar_area] = Layout::horizontal([Min(0), Length(1)]).areas(text_area);

        state.viewport_height = text_area.height.into();

        let buffer = state.buffer.try_read().unwrap();
        let rope = &buffer.rope;

        Paragraph::new(match state.selection {
            // no selection, so nothing to highlight
            None => rope
                .lines_at(state.viewport_line)
                .map(|line| Line::from(tabs_to_spaces(Cow::from(line)).into_owned()))
                .take(area.height.into())
                .collect::<Vec<_>>(),
            // highlight whole line, no line, or part of the line
            Some(selection) => {
                let (selection_start, selection_end) = reorder(state.cursor, selection);

                rope.lines_at(state.viewport_line)
                    .zip(state.viewport_line..)
                    .map(
                        |(line, line_number)| match line_char_range(rope, line_number) {
                            None => Line::from(tabs_to_spaces(Cow::from(line)).into_owned()),
                            Some((line_start, line_end)) => highlight(
                                tabs_to_spaces(Cow::from(line)),
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
                .position(state.viewport_line),
        );

        match self.mode {
            None | Some(EditorMode::Editing) => {
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

                        let [source_area, line_area] =
                            Layout::horizontal([Min(0), Length(digits.try_into().unwrap())])
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
            Some(EditorMode::ConfirmClose { .. }) => {
                Paragraph::new("Unsaved changes. Really quit?")
                    .style(REVERSED)
                    .render(status_area, buf);
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

fn reorder<T: Ord>(x: T, y: T) -> (T, T) {
    if x <= y { (x, y) } else { (y, x) }
}
