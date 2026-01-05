// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A Nano-like editor with an emphasis on modern features

#![forbid(unsafe_code)]

mod buffer;
mod editor;
mod help;
mod prompt;
mod syntax;

fn main() -> std::io::Result<()> {
    use clap::Parser;
    use crossterm::event::read;
    use editor::Editor;
    use std::ffi::OsString;

    #[derive(Parser)]
    #[command(version)]
    #[command(about = "Very Little Editor")]
    struct Opt {
        files: Vec<OsString>,
    }

    let mut editor = Editor::new(Opt::parse().files)?;

    execute_terminal(|terminal| {
        while editor.has_open_buffers() {
            editor.display(terminal)?;
            editor.process_event(read()?);
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
