use ratatui::widgets::StatefulWidget;
use std::borrow::Cow;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    buffer: Arc<Mutex<Buffer>>,
    viewport_line: usize,     // viewport's start line (should be <= cursor)
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
            (self.viewport_line + lines).min(self.buffer.lock().unwrap().total_lines());
    }

    /// Returns cursor position in rope as (row, col), if possible
    ///
    /// Both indexes start from 0
    ///
    /// This position is independent of the viewport position
    fn cursor_position(&self) -> Option<(usize, usize)> {
        let rope = &self.buffer.lock().unwrap().rope;
        let line = rope.try_char_to_line(self.cursor).ok()?;
        let line_start = rope.try_line_to_char(line).ok()?;

        Some((line, self.cursor.checked_sub(line_start)?))
    }
}

impl From<Buffer> for BufferContext {
    fn from(buffer: Buffer) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(buffer)),
            viewport_line: 0,
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
                .map(|p| Buffer::open(p).map(|b| BufferContext::from(b)))
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
        if self.buffers.len() > 0 {
            self.current = (self.current + 1) % self.buffers.len()
        }
    }

    pub fn previous_buffer(&mut self) {
        if self.buffers.len() > 0 {
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

        let buffer = state.buffer.lock().unwrap();

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
