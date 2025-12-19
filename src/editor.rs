use crate::buffer::{Buffer, BufferPosition};
use ratatui::widgets::StatefulWidget;
use std::path::Path;

pub struct Editor {
    buffers: Vec<Buffer>,
    layout: Layout,
}

impl Editor {
    pub fn new<P: AsRef<Path>>(buffers: impl IntoIterator<Item = P>) -> std::io::Result<Self> {
        Ok(Self {
            buffers: buffers
                .into_iter()
                .map(Buffer::open)
                .collect::<Result<_, _>>()?,
            layout: Layout::default(),
        })
    }

    pub fn display(&mut self, term: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        use ratatui::layout::Position;

        term.draw(|frame| {
            let area = frame.area();
            frame.render_stateful_widget(&self.layout, area, &mut self.buffers);
            frame.set_cursor_position(Position { x: 0, y: 0 });
        })
        .map(|_| ())

        // TODO - place cursor in appropriate position in buffer
        // TODO - draw help messages, by default
    }

    pub fn previous_line(&mut self) {
        self.layout.decrement_lines(&self.buffers, 1);
    }

    pub fn next_line(&mut self) {
        self.layout.increment_lines(&self.buffers, 1);
    }

    pub fn previous_buffer(&mut self) {
        self.layout.previous_buffer(self.buffers.len());
    }

    pub fn next_buffer(&mut self) {
        self.layout.next_buffer(self.buffers.len());
    }

    pub fn single_layout(&mut self) {
        self.layout.single_layout()
    }

    pub fn horizontal_layout(&mut self) {
        self.layout.horizontal_layout()
    }

    pub fn vertical_layout(&mut self) {
        self.layout.vertical_layout()
    }

    pub fn swap_cursor_pane(&mut self) {
        self.layout.swap_cursor()
    }

    pub fn swap_pane_positions(&mut self) {
        self.layout.swap_panes()
    }
}

#[derive(Default)]
enum HorizontalPos {
    #[default]
    Top,
    Bottom,
}

enum Layout {
    Single {
        // which buffer our single screen is pointing at
        buffer: BufferPosition,
    },
    Horizontal {
        top: BufferPosition,
        bottom: BufferPosition,
        which: HorizontalPos,
    },
}

impl Layout {
    fn decrement_lines(&mut self, buffers: &[Buffer], lines: usize) {
        match self {
            Self::Single { buffer }
            | Self::Horizontal {
                top: buffer,
                which: HorizontalPos::Top,
                ..
            }
            | Self::Horizontal {
                bottom: buffer,
                which: HorizontalPos::Bottom,
                ..
            } => {
                if buffer.get_buffer(buffers).is_some() {
                    buffer.decrement_lines(lines);
                }
            }
        }
    }

    fn increment_lines(&mut self, buffers: &[Buffer], lines: usize) {
        match self {
            Self::Single { buffer }
            | Self::Horizontal {
                top: buffer,
                which: HorizontalPos::Top,
                ..
            }
            | Self::Horizontal {
                bottom: buffer,
                which: HorizontalPos::Bottom,
                ..
            } => {
                if let Some(b) = buffer.get_buffer(buffers) {
                    buffer.increment_lines(lines, b.total_lines());
                }
            }
        }
    }

    fn previous_buffer(&mut self, total_buffers: usize) {
        fn wrapping_dec(value: usize, max: usize) -> usize {
            if max > 0 {
                value.checked_sub(1).unwrap_or(max - 1)
            } else {
                0
            }
        }

        match self {
            Self::Single {
                buffer: BufferPosition { index: buffer, .. },
            }
            | Self::Horizontal {
                top: BufferPosition { index: buffer, .. },
                which: HorizontalPos::Top,
                ..
            }
            | Self::Horizontal {
                bottom: BufferPosition { index: buffer, .. },
                which: HorizontalPos::Bottom,
                ..
            } => {
                *buffer = wrapping_dec(*buffer, total_buffers);
            }
        }
    }

    fn next_buffer(&mut self, total_buffers: usize) {
        fn wrapping_inc(value: usize, max: usize) -> usize {
            if max > 0 { (value + 1) % max } else { 0 }
        }

        match self {
            Self::Single {
                buffer: BufferPosition { index: buffer, .. },
            }
            | Self::Horizontal {
                top: BufferPosition { index: buffer, .. },
                which: HorizontalPos::Top,
                ..
            }
            | Self::Horizontal {
                bottom: BufferPosition { index: buffer, .. },
                which: HorizontalPos::Bottom,
                ..
            } => {
                *buffer = wrapping_inc(*buffer, total_buffers);
            }
        }
    }

    fn single_layout(&mut self) {
        match self {
            Self::Single { .. } => { /* do nothing */ }
            Self::Horizontal { top, .. } => {
                *self = Self::Single { buffer: *top };
            }
        }
    }

    fn vertical_layout(&mut self) {
        // TODO - implement this
    }

    fn horizontal_layout(&mut self) {
        match self {
            Self::Single { buffer } => {
                *self = Self::Horizontal {
                    top: *buffer,
                    bottom: *buffer,
                    which: HorizontalPos::default(),
                }
            }
            Self::Horizontal { .. } => { /* do nothing */ }
        }
    }

    fn swap_panes(&mut self) {
        match self {
            Self::Single { .. } => { /* do nothing */ }
            Self::Horizontal { top, bottom, .. } => {
                std::mem::swap(top, bottom);
            }
        }
    }

    fn swap_cursor(&mut self) {
        match self {
            Self::Single { .. } => { /* do nothing */ }
            Self::Horizontal { which, .. } => {
                *which = match which {
                    HorizontalPos::Top => HorizontalPos::Bottom,
                    HorizontalPos::Bottom => HorizontalPos::Top,
                }
            }
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::Single {
            buffer: BufferPosition::default(),
        }
    }
}

impl StatefulWidget for &Layout {
    type State = Vec<Buffer>;

    fn render(
        self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        buffers: &mut Vec<Buffer>,
    ) {
        use crate::buffer::{BufferPosition, BufferWidget};

        match self {
            Layout::Single {
                buffer: BufferPosition { index, line },
            } => {
                if let Some(buffer) = buffers.get_mut(*index) {
                    BufferWidget { line: *line }.render(area, buf, buffer);
                }
            }
            Layout::Horizontal { top, bottom, .. } => {
                use ratatui::layout::{Constraint, Layout};

                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);
                if let Some(buffer) = buffers.get_mut(top.index) {
                    BufferWidget { line: top.line }.render(top_area, buf, buffer);
                }
                if let Some(buffer) = buffers.get_mut(bottom.index) {
                    BufferWidget { line: bottom.line }.render(bottom_area, buf, buffer);
                }
            }
        }
    }
}
