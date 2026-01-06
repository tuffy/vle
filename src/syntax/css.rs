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
enum CssToken {
    #[regex(r"[[:alpha:]-]*: ")]
    Property,
    #[regex(r"/\*.*?\*/")]
    Comment,
    #[token("{")]
    #[token("}")]
    #[token(";")]
    Syntax,
    #[regex(r"\.[[:alpha:]-]+")]
    Class,
    #[regex(r"#[[:alpha:]-]+")]
    Id,
    #[regex(r"\[[[:alpha:]-]+\]")]
    #[regex(r#"\[[[:alpha:]-]+=\".*?\"\]"#)]
    #[regex(r#"\[[[:alpha:]-]+~=\".*?\"\]"#)]
    #[regex(r#"\[[[:alpha:]-]+\|=\".*?\"\]"#)]
    #[regex(r#"\[[[:alpha:]-]+\^=\".*?\"\]"#)]
    #[regex(r#"\[[[:alpha:]-]+\$=\".*?\"\]"#)]
    #[regex(r#"\[[[:alpha:]-]+\*=\".*?\"\]"#)]
    Attribute,
}

impl TryFrom<CssToken> for Color {
    type Error = ();

    fn try_from(t: CssToken) -> Result<Color, ()> {
        match t {
            CssToken::Property => Ok(Color::Yellow),
            CssToken::Comment => Ok(Color::Blue),
            CssToken::Syntax => Ok(Color::Green),
            CssToken::Class => Ok(Color::Red),
            CssToken::Id => Ok(Color::Magenta),
            CssToken::Attribute => Ok(Color::Red),
        }
    }
}

#[derive(Debug)]
pub struct Css;

impl std::fmt::Display for Css {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "CSS".fmt(f)
    }
}

impl crate::syntax::Highlighter for Css {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(
            CssToken::lexer(s)
                .spanned()
                .filter_map(|(t, r)| t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))),
        )
    }
}
