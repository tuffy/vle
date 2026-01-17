// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An exercise in minimalist text editing

#![forbid(unsafe_code)]

mod buffer;
mod editor;
mod endings;
mod files;
mod help;
mod prompt;
mod syntax;

use editor::Editor;

fn main() {
    use crossterm::event::read;

    let mut editor = match open_editor() {
        Ok(editor) => editor,
        Err(err) => {
            eprintln!("* {err}");
            return;
        }
    };

    if let Err(err) = execute_terminal(|terminal| {
        while editor.has_open_buffers() {
            editor.display(terminal)?;
            editor.process_event(read()?);
        }

        Ok(())
    }) {
        eprintln!("{err}");
    }
}

#[cfg(not(feature = "ssh"))]
fn open_editor() -> Result<Editor, Box<dyn std::error::Error>> {
    use clap::Parser;
    use std::path::PathBuf;

    #[derive(Debug, Parser)]
    #[command(version)]
    #[command(about = "Very Little Editor")]
    struct Opt {
        files: Vec<PathBuf>,
    }

    Ok(Editor::new(
        Opt::parse().files.into_iter().map(buffer::Source::from),
    )?)
}

#[cfg(feature = "ssh")]
fn open_editor() -> Result<Editor, Box<dyn std::error::Error>> {
    use clap::Parser;
    use inquire::{Password, PasswordDisplayMode, Text};
    use ssh2::Session;
    use std::net::TcpStream;
    use std::path::PathBuf;

    #[derive(Debug, Parser)]
    #[command(version)]
    #[command(about = "Very Little Editor")]
    struct Opt {
        files: Vec<PathBuf>,
        #[clap(short = 's', long = "ssh", help = "remote SSH host")]
        host: Option<String>,
        #[clap(
            short = 'u',
            long = "user",
            help = "remote username",
            requires = "host"
        )]
        username: Option<String>,
        #[clap(short = 'P', long = "private", help = "private key", requires = "host")]
        private_key: Option<PathBuf>,
        #[clap(
            short = 'p',
            long = "public",
            help = "public key",
            requires = "private_key"
        )]
        public_key: Option<PathBuf>,
        #[clap(long = "port", help = "remote port number", default_value = "22")]
        port: String,
    }

    match Opt::parse() {
        Opt {
            files, host: None, ..
        } => Ok(Editor::new(files.into_iter().map(buffer::Source::from))?),
        Opt {
            files,
            host: Some(host),
            username,
            private_key,
            public_key,
            port,
        } => {
            let username = match username {
                Some(username) => username,
                None => Text::new("Username").prompt()?,
            };

            Ok(Editor::new_remote(
                files.into_iter().map(buffer::Source::from),
                match private_key {
                    Some(private_key) => {
                        let password = Password::new("Private Key Password")
                            .with_display_mode(PasswordDisplayMode::Masked)
                            .without_confirmation()
                            .prompt_skippable()?;

                        let tcp = TcpStream::connect(&format!("{host}:{port}"))?;
                        let mut sess = Session::new()?;
                        sess.set_tcp_stream(tcp);
                        sess.handshake()?;
                        sess.userauth_pubkey_file(
                            &username,
                            public_key.as_deref(),
                            &private_key,
                            password.as_deref(),
                        )?;
                        sess
                    }
                    None => {
                        let password = Password::new("Password")
                            .with_display_mode(PasswordDisplayMode::Masked)
                            .without_confirmation()
                            .prompt()?;

                        let tcp = TcpStream::connect(&format!("{host}:{port}"))?;
                        let mut sess = Session::new()?;
                        sess.set_tcp_stream(tcp);
                        sess.handshake()?;
                        sess.userauth_password(&username, &password)?;
                        sess
                    }
                },
            )?)
        }
    }
}

/// Sets up terminal, executes editor, and automatically cleans up afterward
fn execute_terminal<T>(
    f: impl FnOnce(&mut ratatui::DefaultTerminal) -> std::io::Result<T>,
) -> std::io::Result<T> {
    use crossterm::{event::EnableBracketedPaste, execute};

    let mut term = ratatui::init();
    execute!(std::io::stdout(), EnableBracketedPaste)?;
    let result = f(&mut term);
    execute!(std::io::stdout(), EnableBracketedPaste)?;
    ratatui::restore();
    result
}
