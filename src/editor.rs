use crate::buffer::BufferList;
use crossterm::event::Event;
use ratatui::{
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
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
        term.draw(|frame| {
            let area = frame.area();
            frame.render_stateful_widget(LayoutWidget, area, &mut self.layout);
            frame.set_cursor_position(self.layout.cursor_position(area).unwrap_or_default());
        })
        .map(|_| ())

        // TODO - place cursor in appropriate position in buffer
        // TODO - draw help messages, by default
    }

    pub fn process_event(&mut self, event: Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.viewport_up(1),
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.viewport_down(1),
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.viewport_up(25),
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.viewport_down(25),
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.previous_buffer(),
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.next_buffer(),
            Event::Key(KeyEvent {
                code: KeyCode::F(1),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.single_layout(),
            Event::Key(KeyEvent {
                code: KeyCode::F(2),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.horizontal_layout(),
            Event::Key(KeyEvent {
                code: KeyCode::F(3),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.vertical_layout(),
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if let Layout::Vertical { which, .. } = &mut self.layout {
                    *which = VerticalPos::Left;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if let Layout::Vertical { which, .. } = &mut self.layout {
                    *which = VerticalPos::Right;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if let Layout::Horizontal { which, .. } = &mut self.layout {
                    *which = HorizontalPos::Top;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if let Layout::Horizontal { which, .. } = &mut self.layout {
                    *which = HorizontalPos::Bottom;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('='),
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.swap_panes(),
            _ => { /* ignore other events */ }
        }
    }
}

#[derive(Default)]
enum HorizontalPos {
    #[default]
    Top,
    Bottom,
}

#[derive(Default)]
enum VerticalPos {
    #[default]
    Left,
    Right,
}

enum Layout {
    Single(BufferList),
    Horizontal {
        top: BufferList,
        bottom: BufferList,
        which: HorizontalPos,
    },
    Vertical {
        left: BufferList,
        right: BufferList,
        which: VerticalPos,
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
            }
            | Self::Vertical {
                left: buffer,
                which: VerticalPos::Left,
                ..
            }
            | Self::Vertical {
                right: buffer,
                which: VerticalPos::Right,
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
            Self::Vertical {
                left,
                which: VerticalPos::Left,
                ..
            } => {
                *self = Self::Single(std::mem::take(left));
            }
            Self::Vertical {
                right,
                which: VerticalPos::Right,
                ..
            } => {
                *self = Self::Single(std::mem::take(right));
            }
        }
    }

    fn vertical_layout(&mut self) {
        match self {
            Self::Single(buffer) => {
                *self = Self::Vertical {
                    left: buffer.clone(),
                    right: std::mem::take(buffer),
                    which: VerticalPos::default(),
                }
            }
            Self::Horizontal { top, bottom, which } => {
                *self = Self::Vertical {
                    left: std::mem::take(top),
                    right: std::mem::take(bottom),
                    which: match which {
                        HorizontalPos::Top => VerticalPos::Left,
                        HorizontalPos::Bottom => VerticalPos::Right,
                    },
                }
            }
            Self::Vertical { .. } => { /* do nothing */ }
        }
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
            Self::Vertical { left, right, which } => {
                *self = Self::Horizontal {
                    top: std::mem::take(left),
                    bottom: std::mem::take(right),
                    which: match which {
                        VerticalPos::Left => HorizontalPos::Top,
                        VerticalPos::Right => HorizontalPos::Bottom,
                    },
                }
            }
        }
    }

    fn swap_panes(&mut self) {
        match self {
            Self::Single(_) => { /* do nothing */ }
            Self::Horizontal { top, bottom, .. } => {
                std::mem::swap(top, bottom);
            }
            Self::Vertical { left, right, .. } => {
                std::mem::swap(left, right);
            }
        }
    }

    fn cursor_position(&self, area: Rect) -> Option<Position> {
        use ratatui::layout::{Constraint, Layout};

        match self {
            Self::Single(_) => Some(Position {
                x: area.x,
                y: area.y,
            }),
            Self::Horizontal { which, .. } => {
                let [top, bottom] = Layout::vertical(Constraint::from_fills([1, 1])).areas(area);

                match which {
                    HorizontalPos::Top => Some(Position { x: top.x, y: top.y }),
                    HorizontalPos::Bottom => Some(Position {
                        x: bottom.x,
                        y: bottom.y,
                    }),
                }
            }
            Self::Vertical { which, .. } => {
                let [left, right] = Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);

                match which {
                    VerticalPos::Left => Some(Position {
                        x: left.x,
                        y: left.y,
                    }),
                    VerticalPos::Right => Some(Position {
                        x: right.x,
                        y: right.y,
                    }),
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
            Layout::Vertical { left, right, .. } => {
                use ratatui::layout::{Constraint, Layout};

                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);
                if let Some(buffer) = left.current_mut() {
                    BufferWidget.render(left_area, buf, buffer);
                }
                if let Some(buffer) = right.current_mut() {
                    BufferWidget.render(right_area, buf, buffer);
                }
            }
        }
    }
}
