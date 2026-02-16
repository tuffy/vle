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
enum YamlToken {
    #[regex("#.*", allow_greedy = true)]
    Comment,
    #[token("[")]
    #[token("]")]
    #[token("{")]
    #[token("}")]
    #[token("-")]
    Symbol,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r"'([^\\']|\\.)*'")]
    String,
    #[regex(r"[[:alpha:]][[:alpha:][:digit:]]*:")]
    Name,
    #[regex(r"[+-]?[0-9][0-9_]*")]
    #[regex(r"[+-]?[0-9][0-9_]*\.[0-9_]+(e[+-][0-9]+)?")]
    #[regex(r"[+-]?0[bB][0-1][0-1_]*")]
    #[regex(r"[+-]?0[xX][0-9a-fA-F][0-9a-fA-F_]*")]
    #[regex(r"[+-]?0[oO]?[0-7][0-7_]*")]
    #[regex(r"[+-]?\.[iI][nN][fF]")]
    #[regex(r"\.[nN][aA][nN]")]
    Number,
    #[token("true")]
    #[token("false")]
    #[token("TRUE")]
    #[token("FALSE")]
    #[token("True")]
    #[token("False")]
    Boolean,
}

impl TryFrom<YamlToken> for Highlight {
    type Error = ();

    fn try_from(t: YamlToken) -> Result<Highlight, ()> {
        match t {
            YamlToken::Comment => Ok(color::COMMENT),
            YamlToken::Symbol => Ok(Color::Yellow.into()),
            YamlToken::String => Ok(color::STRING),
            YamlToken::Name => Ok(Color::Magenta.into()),
            YamlToken::Number => Ok(color::NUMBER),
            YamlToken::Boolean => Ok(color::KEYWORD),
        }
    }
}

#[derive(Debug)]
pub struct Yaml;

impl std::fmt::Display for Yaml {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "YAML".fmt(f)
    }
}

highlighter!(Yaml, YamlToken);
