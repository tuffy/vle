use crate::buffer::BufferList;
use ratatui::widgets::StatefulWidget;
use std::path::Path;

pub struct Editor {
    layout: Layout,
}

impl Editor {
    pub fn new<P: AsRef<Path>>(buffers: impl IntoIterator<Item = P>) -> std::io::Result<Self> {
        Ok(Self {
            layout: Layout::Single(BufferList::new(buffers)?),
        })
    }

    pub fn display(&mut self, term: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        use ratatui::layout::Position;

        term.draw(|frame| {
            let area = frame.area();
            frame.render_stateful_widget(LayoutWidget, area, &mut self.layout);
            frame.set_cursor_position(Position { x: 0, y: 0 });
        })
        .map(|_| ())

        // TODO - place cursor in appropriate position in buffer
        // TODO - draw help messages, by default
    }

    pub fn viewport_up(&mut self, lines: usize) {
        self.layout.viewport_up(lines);
    }

    pub fn viewport_down(&mut self, lines: usize) {
        self.layout.viewport_down(lines)
    }

    pub fn previous_buffer(&mut self) {
        self.layout.previous_buffer();
    }

    pub fn next_buffer(&mut self) {
        self.layout.next_buffer();
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
    Single(BufferList),
    Horizontal {
        top: BufferList,
        bottom: BufferList,
        which: HorizontalPos,
    },
}

impl Layout {
    fn selected_buffer_list_mut(&mut self) -> &mut BufferList {
        match self {
            Self::Single(buffer)
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

    fn viewport_up(&mut self, lines: usize) {
        self.selected_buffer_list_mut().viewport_up(lines);
    }

    fn viewport_down(&mut self, lines: usize) {
        self.selected_buffer_list_mut().viewport_down(lines);
    }

    fn previous_buffer(&mut self) {
        self.selected_buffer_list_mut().previous_buffer()
    }

    fn next_buffer(&mut self) {
        self.selected_buffer_list_mut().next_buffer()
    }

    fn single_layout(&mut self) {
        match self {
            Self::Single(_) => { /* do nothing */ }
            Self::Horizontal {
                top,
                which: HorizontalPos::Top,
                ..
            } => {
                *self = Self::Single(std::mem::take(top));
            }
            Self::Horizontal {
                bottom,
                which: HorizontalPos::Bottom,
                ..
            } => {
                *self = Self::Single(std::mem::take(bottom));
            }
        }
    }

    fn vertical_layout(&mut self) {
        // TODO - implement this
    }

    fn horizontal_layout(&mut self) {
        match self {
            Self::Single(buffer) => {
                *self = Self::Horizontal {
                    top: buffer.clone(),
                    bottom: std::mem::take(buffer),
                    which: HorizontalPos::default(),
                }
            }
            Self::Horizontal { .. } => { /* do nothing */ }
        }
    }

    fn swap_panes(&mut self) {
        match self {
            Self::Single(_) => { /* do nothing */ }
            Self::Horizontal { top, bottom, .. } => {
                std::mem::swap(top, bottom);
            }
        }
    }

    fn swap_cursor(&mut self) {
        match self {
            Self::Single(_) => { /* do nothing */ }
            Self::Horizontal { which, .. } => {
                *which = match which {
                    HorizontalPos::Top => HorizontalPos::Bottom,
                    HorizontalPos::Bottom => HorizontalPos::Top,
                }
            }
        }
    }
}

struct LayoutWidget;

impl StatefulWidget for LayoutWidget {
    type State = Layout;

    fn render(
        self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        layout: &mut Layout,
    ) {
        use crate::buffer::BufferWidget;

        match layout {
            Layout::Single(single) => {
                if let Some(buffer) = single.current_mut() {
                    BufferWidget.render(area, buf, buffer);
                }
            }
            Layout::Horizontal { top, bottom, .. } => {
                use ratatui::layout::{Constraint, Layout};

                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);
                if let Some(buffer) = top.current_mut() {
                    BufferWidget.render(top_area, buf, buffer);
                }
                if let Some(buffer) = bottom.current_mut() {
                    BufferWidget.render(bottom_area, buf, buffer);
                }
            }
        }
    }
}
