use ratatui::widgets::StatefulWidget;
use std::path::Path;

fn main() -> std::io::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read};

    let mut editor = Editor {
        buffer: Buffer::open(std::env::args_os().skip(1).next().unwrap())?,
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

struct Editor {
    // TODO - support multiple buffers, each with its own path
    buffer: Buffer,
}

impl Editor {
    fn display(&mut self, term: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        use ratatui::layout::Position;

        term.draw(|frame| {
            let area = frame.area();
            frame.render_stateful_widget(BufferWidget, area, &mut self.buffer);
            frame.set_cursor_position(Position { x: 0, y: 0 });
        })
        .map(|_| ())

        // TODO - draw help messages, by default
    }

    fn previous_line(&mut self) {
        self.buffer.decrement_lines(1)
    }

    fn next_line(&mut self) {
        self.buffer.increment_lines(1)
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
