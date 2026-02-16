// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::syntax::color;
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum MakefileToken {
    #[regex(r"\$+[{(][[:alnum:]_-]+[})]")]
    Variable,
    #[regex(r" (:?:|\+|\?)?= ")]
    Assignment,
    #[regex("#.*", allow_greedy = true)]
    Comment,
}

impl TryFrom<MakefileToken> for Color {
    type Error = ();

    fn try_from(t: MakefileToken) -> Result<Color, ()> {
        match t {
            MakefileToken::Variable => Ok(Color::Cyan),
            MakefileToken::Assignment => Ok(Color::Red),
            MakefileToken::Comment => Ok(color::COMMENT),
        }
    }
}

#[derive(Debug)]
pub struct Makefile;

impl std::fmt::Display for Makefile {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Makefile".fmt(f)
    }
}

impl crate::syntax::Highlighter for Makefile {
    fn highlight<'s>(
        &self,
        s: &'s str,
        _state: &'s mut crate::syntax::HighlightState,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(
            MakefileToken::lexer(s)
                .spanned()
                .filter_map(|(t, r)| t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))),
        )
    }

    fn tabs_required(&self) -> bool {
        true
    }
}
