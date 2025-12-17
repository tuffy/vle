fn main() -> std::io::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read};
    use std::fs::File;
    use std::io::{BufReader, Write};

    let mut editor = Editor {
        line: 0,
        buffer: ropey::Rope::from_reader(BufReader::new(File::open(
            std::env::args_os().skip(1).next().unwrap(),
        )?))?,
    };

    execute_terminal(|stdout| {
        loop {
            editor.display(stdout.by_ref())?;
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
    fn display<W: std::io::Write>(&self, mut w: W) -> std::io::Result<()> {
        use crossterm::{
            queue,
            {
                cursor::{MoveTo, MoveToNextLine},
                terminal::{Clear, ClearType, size},
            },
        };
        use std::borrow::Cow;
        use unicode_truncate::UnicodeTruncateStr;

        fn tabs_to_spaces(s: &str) -> Cow<'_, str> {
            if s.contains('\t') {
                s.replace('\t', "    ").into()
            } else {
                s.into()
            }
        }

        queue!(w, Clear(ClearType::All), MoveTo(0, 0))?;

        let (cols, rows) = size()?;

        for line in self.buffer.lines_at(self.line).take(rows.into()) {
            write!(
                w,
                "{}",
                tabs_to_spaces(Cow::from(line).trim_end())
                    .unicode_truncate(cols.into())
                    .0
            )?;
            queue!(w, MoveToNextLine(1))?;
        }

        queue!(w, MoveTo(0, 0))?;

        w.flush()?;

        // TODO - support horizontal scrolling
        // TODO - draw vertical scrollbar
        // TODO - draw status bar
        // TODO - draw help messages, by default

        Ok(())
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
    f: impl FnOnce(&mut std::io::Stdout) -> std::io::Result<T>,
) -> std::io::Result<T> {
    use crossterm::{
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    };

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(&mut stdout, EnterAlternateScreen)?;
    let result = f(&mut stdout)?;
    execute!(&mut stdout, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(result)
}
