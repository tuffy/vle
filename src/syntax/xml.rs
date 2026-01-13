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
enum XmlToken {
    #[regex("<[[:alpha:]][[:alpha:][:digit:]_]*")]
    TagStart,
    #[regex("</[[:alpha:]][[:alpha:][:digit:]_]*>")]
    #[token("/>")]
    TagEnd,
    #[regex("[[:alpha:]][[:alpha:][:digit:]_]*=")]
    FieldName,
    #[regex(r#"\"[^\"]*\""#)]
    String,
    #[token("<!--")]
    StartComment,
    #[token("-->")]
    EndComment,
}

impl TryFrom<XmlToken> for Color {
    type Error = ();

    fn try_from(t: XmlToken) -> Result<Color, ()> {
        match t {
            XmlToken::TagStart | XmlToken::TagEnd => Ok(Color::Cyan),
            XmlToken::FieldName => Ok(Color::Green),
            XmlToken::String => Ok(Color::Magenta),
            XmlToken::StartComment | XmlToken::EndComment => Ok(Color::Yellow),
        }
    }
}

#[derive(Debug)]
pub struct Xml;

impl std::fmt::Display for Xml {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "XML".fmt(f)
    }
}

highlighter!(Xml, XmlToken, StartComment, EndComment, Yellow);
