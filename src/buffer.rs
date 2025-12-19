use ratatui::widgets::StatefulWidget;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A buffer corresponding to a file on disk (either local or remote)
pub struct Buffer {
    // TODO - support buffer's source as Source enum (file on disk, ssh target, etc.)
    rope: ropey::Rope,
    // TODO - support undo stack
    // TODO - support redo stack
}

impl Buffer {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        use std::fs::File;
        use std::io::BufReader;

        // TODO - if file doesn't exist, create new one
        Ok(Self {
            rope: ropey::Rope::from_reader(BufReader::new(File::open(path)?))?,
        })
    }

    pub fn total_lines(&self) -> usize {
        self.rope.len_lines()
    }
}

/// A buffer with additional context on a per-view basis
#[derive(Clone)]
pub struct BufferContext {
    buffer: Arc<Mutex<Buffer>>,
    viewport_line: usize,
    // TODO - support cursor's character position in rope
    // TODO - support optional text selection
}

impl BufferContext {
    fn viewport_up(&mut self, lines: usize) {
        self.viewport_line = self.viewport_line.saturating_sub(lines)
    }

    fn viewport_down(&mut self, lines: usize) {
        self.viewport_line =
            (self.viewport_line + lines).min(self.buffer.lock().unwrap().total_lines());
    }
}

impl From<Buffer> for BufferContext {
    fn from(buffer: Buffer) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(buffer)),
            viewport_line: 0,
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
    pub fn new<P: AsRef<Path>>(paths: impl IntoIterator<Item = P>) -> std::io::Result<Self> {
        Ok(Self {
            buffers: paths
                .into_iter()
                .map(|p| Buffer::open(p).map(|b| BufferContext::from(b)))
                .collect::<Result<_, _>>()?,
            current: 0,
        })
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
            text::Line,
            widgets::{Paragraph, Widget},
        };
        use std::borrow::Cow;

        fn tabs_to_spaces<'s, S: Into<Cow<'s, str>> + AsRef<str>>(s: S) -> Cow<'s, str> {
            if s.as_ref().contains('\t') {
                s.as_ref().replace('\t', "    ").into()
            } else {
                s.into()
            }
        }

        Paragraph::new(
            state
                .buffer
                .lock()
                .unwrap()
                .rope
                .lines_at(state.viewport_line)
                .map(|line| Line::from(tabs_to_spaces(Cow::from(line)).into_owned()))
                .take(area.height.into())
                .collect::<Vec<_>>(),
        )
        .render(area, buf)

        // TODO - support horizontal scrolling
        // TODO - draw vertical scrollbar at right
        // TODO - draw status bar at bottom
    }
}
