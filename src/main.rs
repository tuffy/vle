// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An Exercise in Minimalist Text Editing

#![forbid(unsafe_code)]

mod buffer;
mod editor;
mod endings;
mod files;
mod help;
mod key;
mod prompt;
mod scrollbar;
mod syntax;

use editor::{Editor, LineNumber};

fn main() {
    use crossterm::event::{Event, MouseEvent, MouseEventKind, poll, read};
    use std::time::Duration;

    let mut editor = match open_editor() {
        Ok(editor) => editor,
        Err(err) => {
            eprintln!("* {err}");
            return;
        }
    };

    if let Err(err) = execute_terminal(|terminal| {
        match std::env::var("VLE_AUTO_SAVE")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(Duration::from_secs)
            .filter(|d| *d > Duration::ZERO)
        {
            None => {
                while editor.has_open_buffers() {
                    let area = editor.display(terminal)?;
                    editor.process_event(
                        area,
                        loop {
                            match read()? {
                                Event::Mouse(MouseEvent {
                                    kind: MouseEventKind::Moved | MouseEventKind::Up(_),
                                    ..
                                }) => { /* ignore mouse movement events */ }
                                event => break event,
                            }
                        },
                    )
                }
            }
            Some(max_wait) => {
                while editor.has_open_buffers() {
                    let area = editor.display(terminal)?;
                    loop {
                        match poll(max_wait)? {
                            true => match read()? {
                                Event::Mouse(MouseEvent {
                                    kind: MouseEventKind::Moved | MouseEventKind::Up(_),
                                    ..
                                }) => { /* ignore mouse movement events */ }
                                event => break editor.process_event(area, event),
                            },
                            false => {
                                if editor.auto_save() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
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
        #[clap(short = 'l', long = "line", help = "starting line number")]
        line: Option<LineNumber>,
        #[clap(long = "test", help = "display terminal test")]
        test: bool,
        files: Vec<PathBuf>,
    }

    let Opt { line, files, test } = Opt::parse();
    let editor = Editor::new(
        files
            .into_iter()
            .map(buffer::Source::from)
            .chain(test.then_some(buffer::Source::Test)),
    )?;
    Ok(match line {
        None => editor,
        Some(line) => editor.at_line(line),
    })
}

#[cfg(feature = "ssh")]
fn open_editor() -> Result<Editor, Box<dyn std::error::Error>> {
    use clap::Parser;
    use ssh2::Session;
    use std::net::TcpStream;
    use std::path::PathBuf;

    #[derive(Debug, Parser)]
    #[command(version)]
    #[command(about = "Very Little Editor")]
    struct Opt {
        #[clap(short = 'l', long = "line", help = "starting line number")]
        line: Option<LineNumber>,
        #[clap(long = "test", help = "display terminal test")]
        test: bool,
        files: Vec<PathBuf>,
        #[clap(
            short = 's',
            long = "ssh",
            help = "remote SSH host",
            requires = "username"
        )]
        host: Option<String>,
        #[clap(
            short = 'u',
            long = "user",
            help = "remote username",
            requires = "host"
        )]
        username: Option<String>,
        #[clap(long = "no-passwd", help = "don't use password")]
        no_password: bool,
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
            files,
            test,
            line,
            host: None,
            username: None,
            ..
        } => {
            let editor = Editor::new(
                files
                    .into_iter()
                    .map(buffer::Source::from)
                    .chain(test.then_some(buffer::Source::Test)),
            )?;
            Ok(match line {
                None => editor,
                Some(line) => editor.at_line(line),
            })
        }
        Opt {
            line,
            files,
            test,
            host: Some(host),
            username: Some(username),
            no_password,
            private_key,
            public_key,
            port,
        } => {
            let editor = Editor::new_remote(
                files
                    .into_iter()
                    .map(buffer::Source::from)
                    .chain(test.then_some(buffer::Source::Test)),
                match private_key {
                    Some(private_key) => {
                        let password = if no_password {
                            None
                        } else {
                            Some(rpassword::prompt_password("Private Key Password : ")?)
                        };
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
                        let password = rpassword::prompt_password("Password : ")?;
                        let tcp = TcpStream::connect(&format!("{host}:{port}"))?;
                        let mut sess = Session::new()?;
                        sess.set_tcp_stream(tcp);
                        sess.handshake()?;
                        sess.userauth_password(&username, &password)?;
                        sess
                    }
                },
            )?;

            Ok(match line {
                None => editor,
                Some(line) => editor.at_line(line),
            })
        }
        _ => {
            #[derive(Debug)]
            struct ArgumentsMismatch;

            impl std::error::Error for ArgumentsMismatch {}

            impl std::fmt::Display for ArgumentsMismatch {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    "host requires username".fmt(f)
                }
            }

            // this shouldn't happen since host requires username
            Err(Box::new(ArgumentsMismatch))
        }
    }
}

/// Sets up terminal, executes editor, and automatically cleans up afterward
fn execute_terminal<T>(
    f: impl FnOnce(&mut ratatui::DefaultTerminal) -> std::io::Result<T>,
) -> std::io::Result<T> {
    use crossterm::{
        event::{
            DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
            EnableFocusChange, EnableMouseCapture,
        },
        execute,
    };

    let mut term = ratatui::init();
    execute!(
        std::io::stdout(),
        EnableBracketedPaste,
        EnableMouseCapture,
        EnableFocusChange
    )?;
    let result = f(&mut term);
    execute!(
        std::io::stdout(),
        DisableBracketedPaste,
        DisableMouseCapture,
        DisableFocusChange,
    )?;
    ratatui::restore();
    result
}
