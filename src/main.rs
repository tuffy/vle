fn main() -> std::io::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read};
    use std::fs::File;
    use std::io::BufReader;

    let mut editor = Editor {
        line: 0,
        buffer: ropey::Rope::from_reader(BufReader::new(File::open(
            std::env::args_os().skip(1).next().unwrap(),
        )?))?,
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

struct Editor {
    // TODO - support multiple buffers, each with its own path
    buffer: ropey::Rope,
    // TODO - support both line and column cursor position
    line: usize,
}

impl Editor {
    fn display(&self, term: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        use ratatui::{layout::Position, text::Line, widgets::Paragraph};
        use std::borrow::Cow;

        fn tabs_to_spaces<'s, S: Into<Cow<'s, str>> + AsRef<str>>(s: S) -> Cow<'s, str> {
            if s.as_ref().contains('\t') {
                s.as_ref().replace('\t', "    ").into()
            } else {
                s.into()
            }
        }

        term.draw(|frame| {
            let area = frame.area();
            frame.render_widget(
                Paragraph::new(
                    self.buffer
                        .lines_at(self.line)
                        .map(|line| Line::from(tabs_to_spaces(Cow::from(line)).into_owned()))
                        .take(area.height.into())
                        .collect::<Vec<_>>(),
                ),
                area,
            );
            frame.set_cursor_position(Position { x: 0, y: 0 });
        })
        .map(|_| ())

        // TODO - support horizontal scrolling
        // TODO - draw vertical scrollbar
        // TODO - draw status bar
        // TODO - draw help messages, by default
    }

    fn previous_line(&mut self) {
        if self.line > 0 {
            self.line -= 1;
        }
    }

    fn next_line(&mut self) {
        self.line = (self.line + 1).min(self.buffer.len_lines())
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
