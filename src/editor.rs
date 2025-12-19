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

    pub fn viewport_up(&mut self, lines: usize) {
        self.layout.viewport_up(&self.buffers, lines);
    }

    pub fn viewport_down(&mut self, lines: usize) {
        self.layout.viewport_down(&self.buffers, lines);
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
    fn selected_buffer_pos_mut(&mut self) -> &mut BufferPosition {
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
            } => buffer,
        }
    }

    fn viewport_up(&mut self, buffers: &[Buffer], lines: usize) {
        let pos = self.selected_buffer_pos_mut();
        if pos.get_buffer(buffers).is_some() {
            pos.viewport_up(lines);
        }
    }

    fn viewport_down(&mut self, buffers: &[Buffer], lines: usize) {
        let pos = self.selected_buffer_pos_mut();
        if let Some(b) = pos.get_buffer(buffers) {
            pos.viewport_down(lines, b.total_lines());
        }
    }

    fn previous_buffer(&mut self, total_buffers: usize) {
        self.selected_buffer_pos_mut()
            .previous_buffer(total_buffers)
    }

    fn next_buffer(&mut self, total_buffers: usize) {
        self.selected_buffer_pos_mut().next_buffer(total_buffers)
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
        use crate::buffer::BufferWidget;

        match self {
            Layout::Single { buffer: single } => {
                if let Some(buffer) = single.get_buffer_mut(buffers) {
                    BufferWidget {
                        line: single.viewport_line(),
                    }
                    .render(area, buf, buffer);
                }
            }
            Layout::Horizontal { top, bottom, .. } => {
                use ratatui::layout::{Constraint, Layout};

                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);
                if let Some(buffer) = top.get_buffer_mut(buffers) {
                    BufferWidget {
                        line: top.viewport_line(),
                    }
                    .render(top_area, buf, buffer);
                }
                if let Some(buffer) = bottom.get_buffer_mut(buffers) {
                    BufferWidget {
                        line: bottom.viewport_line(),
                    }
                    .render(bottom_area, buf, buffer);
                }
            }
        }
    }
}
