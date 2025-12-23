//! A Nano-like editor with an emphasis on modern features

#![forbid(unsafe_code)]

mod buffer;
mod editor;

fn main() -> std::io::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, read};
    use editor::Editor;

    let mut editor = Editor::new(std::env::args_os().skip(1))?;

    execute_terminal(|terminal| {
        loop {
            editor.display(terminal)?;

            // TODO - filter out Mouse motion events in a sub-loop?
            // TODO - exit when only a single buffer remains
            match read()? {
                Event::Key(KeyEvent {
                    code: KeyCode::Char('q'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => break,
                event => editor.process_event(event),
            }
        }

        Ok(())
    })
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
