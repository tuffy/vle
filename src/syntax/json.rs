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
enum JsonToken {
    #[regex(r#""([^"\\\x00-\x1F]|\\(["\\bnfrt/]|u[a-fA-F0-9]{4}))*""#)]
    String,
    #[regex(r#""([^"\\\x00-\x1F]|\\(["\\bnfrt/]|u[a-fA-F0-9]{4}))*":"#)]
    Name,
    #[regex(r"-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?")]
    Number,
    #[token("true")]
    #[token("false")]
    #[token("null")]
    Literal,
    #[token("[")]
    #[token("]")]
    Punctuation1,
    #[token("{")]
    #[token("}")]
    #[token(",")]
    #[token(":")]
    Punctuation2,
}

impl TryFrom<JsonToken> for Highlight {
    type Error = ();

    fn try_from(t: JsonToken) -> Result<Highlight, ()> {
        match t {
            JsonToken::Name => Ok(color::TYPE),
            JsonToken::String => Ok(color::STRING),
            JsonToken::Number => Ok(color::NUMBER),
            JsonToken::Literal => Ok(Color::Red.into()),
            JsonToken::Punctuation1 => Ok(Color::LightBlue.into()),
            JsonToken::Punctuation2 => Ok(Color::LightRed.into()),
        }
    }
}

#[derive(Debug)]
pub struct Json;

impl std::fmt::Display for Json {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Json".fmt(f)
    }
}

highlighter!(Json, JsonToken);
