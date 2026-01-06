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
enum HtmlToken {
    #[regex("<[[:alpha:]][[:alpha:][:digit:]_]*")]
    TagStart,
    #[regex("</[[:alpha:]][[:alpha:][:digit:]_]*>")]
    #[token("/>")]
    TagEnd,
    #[regex("[[:alpha:]][[:alpha:][:digit:]_]*\\s*=\\s*")]
    FieldName,
    #[regex(r#"\"[^\"]*\""#)]
    String,
    #[regex("&[^;[:space:]]*;")]
    CharRef,
    #[regex("<!-- .*? -->")]
    Comment,
}

impl TryFrom<HtmlToken> for Color {
    type Error = ();

    fn try_from(t: HtmlToken) -> Result<Color, ()> {
        match t {
            HtmlToken::TagStart | HtmlToken::TagEnd => Ok(Color::Cyan),
            HtmlToken::CharRef => Ok(Color::Red),
            HtmlToken::Comment => Ok(Color::Yellow),
            HtmlToken::FieldName => Ok(Color::Green),
            HtmlToken::String => Ok(Color::Magenta),
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

impl crate::syntax::Highlighter for Html {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(
            HtmlToken::lexer(s)
                .spanned()
                .filter_map(|(t, r)| t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))),
        )
    }
}
