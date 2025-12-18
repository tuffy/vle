use ratatui::widgets::StatefulWidget;
use std::path::Path;

fn main() -> std::io::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read};

    // const ALT_SHIFT: KeyModifiers = KeyModifiers::ALT.union(KeyModifiers::SHIFT);

    let mut editor = Editor {
        buffers: std::env::args_os()
            .skip(1)
            .map(Buffer::open)
            .collect::<Result<_, _>>()?,
        layout: Layout::default(),
    };

    execute_terminal(|terminal| {
        loop {
            editor.display(terminal)?;

            // TODO - filter out Mouse motion events in a sub-loop?
            match read()? {
                Event::Key(KeyEvent {
                    code: KeyCode::Up,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.previous_line(),
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.next_line(),
                Event::Key(KeyEvent {
                    code: KeyCode::Left,
                    modifiers: KeyModifiers::ALT,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.previous_buffer(),
                Event::Key(KeyEvent {
                    code: KeyCode::Right,
                    modifiers: KeyModifiers::ALT,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.next_buffer(),
                Event::Key(KeyEvent {
                    code: KeyCode::F(1),
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.to_single_layout(),
                Event::Key(KeyEvent {
                    code: KeyCode::F(2),
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.to_horizontal_layout(),
                Event::Key(KeyEvent {
                    code: KeyCode::F(3),
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.to_vertical_layout(),
                Event::Key(KeyEvent {
                    code: KeyCode::Tab,
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.swap_cursor_pane(),
                Event::Key(KeyEvent {
                    code: KeyCode::Char('='),
                    modifiers: KeyModifiers::ALT,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.swap_pane_positions(),
                Event::Key(KeyEvent {
                    code: KeyCode::Esc, ..
                }) => break,
                _ => { /* ignore other events */ }
            }
        }

        Ok(())
    })
}

struct Buffer {
    // TODO - support buffer's source as Source enum (file on disk, ssh target, etc.)
    rope: ropey::Rope,
    // TODO - support cursor's column
    // TODO - support optional text selection
    // TODO - support undo stack
    // TODO - support redo stack
}

impl Buffer {
    fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        use std::fs::File;
        use std::io::BufReader;

        Ok(Self {
            rope: ropey::Rope::from_reader(BufReader::new(File::open(path)?))?,
        })
    }

    fn total_lines(&self) -> usize {
        self.rope.len_lines()
    }
}

struct BufferWidget {
    line: usize,
}

impl StatefulWidget for BufferWidget {
    type State = Buffer;

    fn render(
        self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        state: &mut Buffer,
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
                .rope
                .lines_at(self.line)
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

type BufIndex = usize;
type LineNum = usize;

#[derive(Copy, Clone, Default)]
struct BufferPosition {
    index: BufIndex,
    line: LineNum,
}

impl BufferPosition {
    fn get_buffer<'b>(&self, buffers: &'b [Buffer]) -> Option<&'b Buffer> {
        buffers.get(self.index)
    }

    fn decrement_lines(&mut self, lines: usize) {
        self.line = self.line.checked_sub(lines).unwrap_or(0)
    }

    fn increment_lines(&mut self, lines: usize, max_lines: usize) {
        self.line = (self.line + lines).min(max_lines)
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

    fn to_single(&mut self) {
        match self {
            Self::Single { .. } => { /* do nothing */ }
            Self::Horizontal { top, .. } => {
                *self = Self::Single { buffer: *top };
            }
        }
    }

    fn to_vertical(&mut self) {
        // TODO - implement this
    }

    fn to_horizontal(&mut self) {
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

struct Editor {
    buffers: Vec<Buffer>,
    layout: Layout,
}

impl Editor {
    fn display(&mut self, term: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
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

    fn previous_line(&mut self) {
        self.layout.decrement_lines(&self.buffers, 1);
    }

    fn next_line(&mut self) {
        self.layout.increment_lines(&self.buffers, 1);
    }

    fn previous_buffer(&mut self) {
        self.layout.previous_buffer(self.buffers.len());
    }

    fn next_buffer(&mut self) {
        self.layout.next_buffer(self.buffers.len());
    }

    fn to_single_layout(&mut self) {
        self.layout.to_single()
    }

    fn to_horizontal_layout(&mut self) {
        self.layout.to_horizontal()
    }

    fn to_vertical_layout(&mut self) {
        self.layout.to_vertical()
    }

    fn swap_cursor_pane(&mut self) {
        self.layout.swap_cursor()
    }

    fn swap_pane_positions(&mut self) {
        self.layout.swap_panes()
    }
}

/// Sets up terminal, executes editor, and automatically cleans up afterward
fn execute_terminal<T>(
    f: impl FnOnce(&mut ratatui::DefaultTerminal) -> std::io::Result<T>,
) -> std::io::Result<T> {
    let mut term = ratatui::init();
    let result = f(&mut term);
    ratatui::restore();
    result
}
