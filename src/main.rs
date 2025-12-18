use ratatui::widgets::StatefulWidget;
use std::path::Path;

fn main() -> std::io::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read};

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
    line: usize,
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
            line: 0,
        })
    }

    fn decrement_lines(&mut self, lines: usize) {
        self.line = self.line.checked_sub(lines).unwrap_or(0);
    }

    fn increment_lines(&mut self, lines: usize) {
        self.line = (self.line + lines).min(self.rope.len_lines());
    }
}

struct BufferWidget;

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
                .lines_at(state.line)
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

enum Layout {
    Single {
        // which buffer our single screen is pointing at
        buffer: usize,
    },
}

impl Layout {
    fn buffer<'b>(&self, buffers: &'b mut [Buffer]) -> Option<&'b mut Buffer> {
        match &self {
            Self::Single { buffer } => buffers.get_mut(*buffer),
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
            Self::Single { buffer } => {
                *buffer = wrapping_dec(*buffer, total_buffers);
            }
        }
    }

    fn next_buffer(&mut self, total_buffers: usize) {
        fn wrapping_inc(value: usize, max: usize) -> usize {
            if max > 0 { (value + 1) % max } else { 0 }
        }

        match self {
            Self::Single { buffer } => {
                *buffer = wrapping_inc(*buffer, total_buffers);
            }
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::Single { buffer: 0 }
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
            if let Some(buffer) = self.layout.buffer(&mut self.buffers) {
                frame.render_stateful_widget(BufferWidget, area, buffer);
            }
            frame.set_cursor_position(Position { x: 0, y: 0 });
        })
        .map(|_| ())

        // TODO - place cursor in appropriate position in buffer
        // TODO - draw help messages, by default
    }

    fn previous_line(&mut self) {
        if let Some(buffer) = self.layout.buffer(&mut self.buffers) {
            buffer.decrement_lines(1)
        }
    }

    fn next_line(&mut self) {
        if let Some(buffer) = self.layout.buffer(&mut self.buffers) {
            buffer.increment_lines(1)
        }
    }

    fn previous_buffer(&mut self) {
        self.layout.previous_buffer(self.buffers.len());
    }

    fn next_buffer(&mut self) {
        self.layout.next_buffer(self.buffers.len());
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
