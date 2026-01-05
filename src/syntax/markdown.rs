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
enum MarkdownToken {
    #[regex("`[^`]+`")]
    Code,
    #[regex(r"\*[^*]+\*")]
    Emphasis,
    #[regex("#.*", allow_greedy = true)]
    Heading,
    #[regex(r"\[[^]]+\]\([^)]+\)")]
    Url,
    #[regex(r"\[[^]]+\]")]
    Link,
}

impl TryFrom<MarkdownToken> for Color {
    type Error = ();

    fn try_from(t: MarkdownToken) -> Result<Color, ()> {
        match t {
            MarkdownToken::Code => Ok(Color::LightCyan),
            MarkdownToken::Emphasis => Ok(Color::Green),
            MarkdownToken::Heading => Ok(Color::LightYellow),
            MarkdownToken::Url => Ok(Color::LightBlue),
            MarkdownToken::Link => Ok(Color::LightMagenta),
        }
    }
}

#[derive(Debug)]
pub struct Markdown;

impl std::fmt::Display for Markdown {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Markdown".fmt(f)
    }
}

impl crate::syntax::Highlighter for Markdown {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(
            MarkdownToken::lexer(s)
                .spanned()
                .filter_map(|(t, r)| t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))),
        )
    }
}
