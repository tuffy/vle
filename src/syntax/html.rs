// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::{Commenting, Highlight, Plain, color};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum HtmlToken {
    #[regex("<[[:alpha:]][[:alpha:][:digit:]_]*>?")]
    TagStart,
    #[regex("</[[:alpha:]][[:alpha:][:digit:]_]*>")]
    #[token(">")]
    #[token("/>")]
    TagEnd,
    #[regex("[[:alpha:]][[:alpha:][:digit:]_]*\\s*=\\s*")]
    FieldName,
    #[regex(r#"\"[^\"]*\""#)]
    String,
    #[regex("&[^;[:space:]]*;")]
    CharRef,
    #[token("<!--")]
    StartComment,
    #[token("-->")]
    EndComment,
}

impl TryFrom<HtmlToken> for Highlight {
    type Error = ();

    fn try_from(t: HtmlToken) -> Result<Highlight, ()> {
        match t {
            HtmlToken::TagStart | HtmlToken::TagEnd => Ok(Color::Cyan.into()),
            HtmlToken::CharRef => Ok(Color::Red.into()),
            HtmlToken::StartComment | HtmlToken::EndComment => Ok(color::COMMENT),
            HtmlToken::FieldName => Ok(Color::Blue.into()),
            HtmlToken::String => Ok(color::STRING),
        }
    }
}

#[derive(Debug)]
pub struct Html;

impl std::fmt::Display for Html {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "HTML".fmt(f)
    }
}

highlighter!(
    Html,
    HtmlToken,
    StartComment,
    EndComment,
    "<!--",
    "-->",
    color::COMMENT
);
