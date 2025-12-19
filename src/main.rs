mod buffer;
mod editor;

fn main() -> std::io::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, read};
    use editor::Editor;

    // const ALT_SHIFT: KeyModifiers = KeyModifiers::ALT.union(KeyModifiers::SHIFT);

    let mut editor = Editor::new(std::env::args_os().skip(1))?;

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
                }) => editor.single_layout(),
                Event::Key(KeyEvent {
                    code: KeyCode::F(2),
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.horizontal_layout(),
                Event::Key(KeyEvent {
                    code: KeyCode::F(3),
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                }) => editor.vertical_layout(),
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

/// Sets up terminal, executes editor, and automatically cleans up afterward
fn execute_terminal<T>(
    f: impl FnOnce(&mut ratatui::DefaultTerminal) -> std::io::Result<T>,
) -> std::io::Result<T> {
    let mut term = ratatui::init();
    let result = f(&mut term);
    ratatui::restore();
    result
}
