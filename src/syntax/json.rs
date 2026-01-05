// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

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

impl TryFrom<JsonToken> for Color {
    type Error = ();

    fn try_from(t: JsonToken) -> Result<Color, ()> {
        match t {
            JsonToken::Name => Ok(Color::Blue),
            JsonToken::String => Ok(Color::LightMagenta),
            JsonToken::Number => Ok(Color::Green),
            JsonToken::Literal => Ok(Color::Green),
            JsonToken::Punctuation1 => Ok(Color::LightBlue),
            JsonToken::Punctuation2 => Ok(Color::LightRed),
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

impl crate::syntax::Highlighter for Json {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(
            JsonToken::lexer(s)
                .spanned()
                .filter_map(|(t, r)| t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))),
        )
    }
}
