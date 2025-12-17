fn main() -> std::io::Result<()> {
    use crossterm::{execute, {cursor::MoveTo, terminal::{Clear, ClearType}}};
    use std::io::Write;

    execute_terminal(|stdout| {
        execute!(
            stdout,
            Clear(ClearType::All),
            MoveTo(0, 0),
        )?;

        writeln!(stdout, "Hello World!")?;

        execute!(stdout, MoveTo(0, 0))?;

        std::thread::sleep(std::time::Duration::from_secs(5));

        Ok(())
    })
}

/// Sets up terminal, executes editor, and automatically cleans up afterward
fn execute_terminal<T>(f: impl FnOnce(&mut std::io::Stdout) -> std::io::Result<T>) -> std::io::Result<T> {
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
