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
    // TODO - indicate whether rope has been edited since last save
    // TODO - support undo stack
    // TODO - support redo stack
}

impl Buffer {
    fn open(path: OsString) -> std::io::Result<Self> {
        let source = Source::from(path);

        Ok(Self {
            rope: source.read_data()?,
            source,
        })
    }

    fn total_lines(&self) -> usize {
        self.rope.len_lines()
    }
}

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

    pub fn cursor_up(&mut self, lines: usize) {
        let rope = &self.buffer.try_read().unwrap().rope;
        if let Ok(current_line) = rope.try_char_to_line(self.cursor) {
            let previous_line = current_line.saturating_sub(lines);
            if let Some((prev_start, prev_end)) = line_char_range(rope, previous_line) {
                self.cursor = (prev_start + self.cursor_column).min(prev_end);
                viewport_follow_cursor(
                    previous_line,
                    &mut self.viewport_line,
                    self.viewport_height,
                );
            }
        }
    }

    pub fn cursor_down(&mut self, lines: usize) {
        let rope = &self.buffer.try_read().unwrap().rope;
        if let Ok(current_line) = rope.try_char_to_line(self.cursor) {
            let next_line = (current_line + lines).min(rope.len_lines());
            if let Some((next_start, next_end)) = line_char_range(rope, next_line) {
                self.cursor = (next_start + self.cursor_column).min(next_end);
                viewport_follow_cursor(next_line, &mut self.viewport_line, self.viewport_height);
            }
        }
    }

    pub fn cursor_back(&mut self) {
        let rope = &self.buffer.try_read().unwrap().rope;
        self.cursor = self.cursor.saturating_sub(1);
        self.cursor_column = cursor_column(&rope, self.cursor);
        if let Ok(current_line) = rope.try_char_to_line(self.cursor) {
            viewport_follow_cursor(current_line, &mut self.viewport_line, self.viewport_height);
        }
    }

    pub fn cursor_forward(&mut self) {
        let rope = &self.buffer.try_read().unwrap().rope;
        self.cursor = (self.cursor + 1).min(rope.len_chars());
        self.cursor_column = cursor_column(&rope, self.cursor);
        if let Ok(current_line) = rope.try_char_to_line(self.cursor) {
            viewport_follow_cursor(current_line, &mut self.viewport_line, self.viewport_height);
        }
    }

    pub fn cursor_home(&mut self) {
        let rope = &self.buffer.try_read().unwrap().rope;
        if let Ok(current_line) = rope.try_char_to_line(self.cursor)
            && let Some((home, _)) = line_char_range(&rope, current_line)
        {
            self.cursor = home;
            self.cursor_column = 0;
        }
    }

    pub fn cursor_end(&mut self) {
        let rope = &self.buffer.try_read().unwrap().rope;
        if let Ok(current_line) = rope.try_char_to_line(self.cursor)
            && let Some((_, end)) = line_char_range(&rope, current_line)
        {
            self.cursor_column += end - self.cursor;
            self.cursor = end;
        }
    }

    pub fn insert_char(&mut self, c: char) {
        // TODO - perform auto-pairing if char is pair-able
        // TODO - update undo list with current state
        // TODO - zap selection before performing insert
        let rope = &mut self.buffer.try_write().unwrap().rope;
        rope.insert_char(self.cursor, c);
        self.cursor += 1;
        self.cursor_column += 1;
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
    } else if let Some(max) = current_line.checked_sub(viewport_height - 1) {
        if *viewport_line < max {
            *viewport_line = max;
        }
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

pub struct BufferWidget;

impl StatefulWidget for BufferWidget {
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

        let [text_area, status_area] = Layout::vertical([Min(0), Length(1)]).areas(area);
        let [text_area, scrollbar_area] = Layout::horizontal([Min(0), Length(1)]).areas(text_area);

        state.viewport_height = text_area.height.into();

        let buffer = state.buffer.try_read().unwrap();

        Paragraph::new(
            buffer
                .rope
                .lines_at(state.viewport_line)
                .map(|line| Line::from(tabs_to_spaces(Cow::from(line)).into_owned()))
                .take(area.height.into())
                .collect::<Vec<_>>(),
        )
        .render(text_area, buf);
        // TODO - support horizontal scrolling if necessary

        Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
            scrollbar_area,
            buf,
            &mut ScrollbarState::new(buffer.total_lines())
                .viewport_content_length(text_area.height.into())
                .position(state.viewport_line),
        );

        // TODO - display different status messages if necessary
        // TODO - display whether source needs to be edited
        Paragraph::new(buffer.source.name())
            .style(Style::default().add_modifier(Modifier::REVERSED))
            .render(status_area, buf);
    }
}
