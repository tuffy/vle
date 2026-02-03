// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum RegexToken {
    #[token("$")]
    #[token("^")]
    Position,
    #[token("(")]
    #[token(")")]
    #[token("|")]
    SubExpression,
    #[token("*")]
    #[token("+")]
    #[token("?")]
    #[regex("\\{[0-9]+\\}")]
    #[regex("\\{[0-9]+,[0-9]+\\}")]
    Repeat,
    #[regex("\\[[^]]+?\\]")]
    #[regex("\\[\\][^]]+?\\]")]
    #[regex("\\[\\^\\][^]]+?\\]")]
    Bracketed,
    #[token("\\[")]
    #[token("\\]")]
    #[token("\\(")]
    #[token("\\)")]
    Bracket,
}

impl TryFrom<RegexToken> for Color {
    type Error = ();

    fn try_from(t: RegexToken) -> Result<Color, ()> {
        match t {
            RegexToken::Position => Ok(Color::Green),
            RegexToken::SubExpression => Ok(Color::Blue),
            RegexToken::Repeat => Ok(Color::Red),
            RegexToken::Bracketed => Ok(Color::Magenta),
            RegexToken::Bracket => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Regex;

impl std::fmt::Display for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Regex".fmt(f)
    }
}

highlighter!(Regex, RegexToken);
