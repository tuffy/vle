fn main() -> std::io::Result<()> {
    use crossterm::{
        execute,
        {
            cursor::{MoveTo, MoveToNextLine},
            terminal::{Clear, ClearType, size},
        },
    };
    use std::borrow::Cow;
    use std::fs::File;
    use std::io::{BufReader, Write};

    let file = ropey::Rope::from_reader(BufReader::new(File::open(
        std::env::args_os().skip(1).next().unwrap(),
    )?))?;

    execute_terminal(|stdout| {
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0),)?;

        let (_cols, rows) = size()?;

        for line in file.lines().take(rows.into()) {
            write!(stdout, "{}", Cow::from(line).trim_end())?;
            execute!(stdout, MoveToNextLine(1))?;
        }

        execute!(stdout, MoveTo(0, 0))?;

        std::thread::sleep(std::time::Duration::from_secs(5));

        Ok(())
    })
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
