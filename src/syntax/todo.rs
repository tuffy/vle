// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::syntax::{Highlight, Highlighter, Syntax};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum TodoToken {
    Completed,
    #[regex(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")]
    Date,
    #[regex(r"\([A-Z]\) ")]
    Priority,
    #[regex(r"\+\S+")]
    Project,
    #[regex(r"@\S+")]
    Context,
    #[regex(r"[^:\s]+:[^\s]+")]
    KeyValue,
}

impl TryFrom<TodoToken> for Highlight {
    type Error = ();

    fn try_from(t: TodoToken) -> Result<Highlight, ()> {
        use crate::syntax::Modifier;

        match t {
            TodoToken::Completed => Ok(Highlight {
                color: None,
                modifier: Modifier::Strikethrough,
            }),
            TodoToken::Date => Ok(Color::Blue.into()),
            TodoToken::Priority => Ok(Color::Red.into()),
            TodoToken::Project => Ok(Color::Magenta.into()),
            TodoToken::Context => Ok(Color::Green.into()),
            TodoToken::KeyValue => Ok(Color::Cyan.into()),
        }
    }
}

#[derive(Debug)]
pub struct Todo;

impl std::fmt::Display for Todo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "TODO".fmt(f)
    }
}

impl Syntax for Todo {
    fn initialize(
        &self,
        _rope: &ropey::Rope,
        _viewport_line: usize,
        _viewport_height: u16,
    ) -> Box<dyn Highlighter> {
        Box::new(TodoHighlighter)
    }

    fn initialize_find(&self) -> Box<dyn Highlighter> {
        Box::new(TodoHighlighter)
    }
}

struct TodoHighlighter;

impl Highlighter for TodoHighlighter {
    fn highlight<'s>(
        &'s mut self,
        line: &'s str,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
        if line.starts_with("x ") {
            Box::new(
                Highlight::try_from(TodoToken::Completed)
                    .ok()
                    .map(|c| (c, 0..line.len()))
                    .into_iter(),
            )
        } else {
            Box::new(TodoToken::lexer(line).spanned().filter_map(|(t, r)| {
                t.ok()
                    .and_then(|t| Highlight::try_from(t).ok())
                    .map(|c| (c, r))
            }))
        }
    }
}
