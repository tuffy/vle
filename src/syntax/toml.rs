// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::{Highlight, color};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum TomlToken {
    #[token("{")]
    #[token("}")]
    #[token("[")]
    #[token("]")]
    Encloser,
    #[regex("[[:alpha:]][[:alpha:][:digit:]_-]*?\\s*?=\\s*?")]
    Key,
    #[token("true")]
    #[token("false")]
    #[regex(r#"\"[^\"]*\""#)]
    Value,
    #[regex("#.*", allow_greedy = true)]
    Comment,
}

impl TryFrom<TomlToken> for Highlight {
    type Error = ();

    fn try_from(t: TomlToken) -> Result<Highlight, ()> {
        match t {
            TomlToken::Encloser => Ok(Color::Red.into()),
            TomlToken::Key => Ok(color::KEYWORD),
            TomlToken::Value => Ok(color::STRING),
            TomlToken::Comment => Ok(color::COMMENT),
        }
    }
}

#[derive(Debug)]
pub struct Toml;

impl std::fmt::Display for Toml {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "TOML".fmt(f)
    }
}

highlighter!(Toml, TomlToken);
