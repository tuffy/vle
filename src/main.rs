fn main() -> std::io::Result<()> {
    use std::fs::File;
    use std::io::BufReader;

    let editor = Editor {
        line: 0,
        buffer: ropey::Rope::from_reader(BufReader::new(File::open(
            std::env::args_os().skip(1).next().unwrap(),
        )?))?,
    };

    execute_terminal(|stdout| {
        editor.display(stdout)?;
        std::thread::sleep(std::time::Duration::from_secs(5));

        Ok(())
    })
}

struct Editor {
    // FIXME - support multiple buffers, each with its own path
    buffer: ropey::Rope,
    // FIXME - support both line and column cursor position
    line: usize,
}

impl Editor {
    fn display<W: std::io::Write>(&self, mut w: W) -> std::io::Result<()> {
        use crossterm::{
            execute,
            {
                cursor::{MoveTo, MoveToNextLine},
                terminal::{Clear, ClearType, size},
            },
        };
        use std::borrow::Cow;
        use unicode_truncate::UnicodeTruncateStr;

        execute!(w, Clear(ClearType::All), MoveTo(0, 0))?;

        let (cols, rows) = size()?;

        for line in self.buffer.lines_at(self.line).take(rows.into()) {
            write!(
                w,
                "{}",
                Cow::from(line).trim_end().unicode_truncate(cols.into()).0
            )?;
            execute!(w, MoveToNextLine(1))?;
        }

        execute!(w, MoveTo(0, 0))?;

        Ok(())
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
