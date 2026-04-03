// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// use crate::highlighter;
use crate::syntax::{Highlight, Highlighter};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum MarkdownToken {
    #[regex("`[^`]+`")]
    Code,
    #[regex(r"\*[^*]+\*")]
    Emphasis,
    Heading,
    #[regex(r"\[[^]]+\]\([^)]+\)")]
    Url,
    #[regex(r"\[[^]]+\]")]
    Link,
}

impl TryFrom<MarkdownToken> for Highlight {
    type Error = ();

    fn try_from(t: MarkdownToken) -> Result<Highlight, ()> {
        use crate::syntax::Modifier;

        match t {
            MarkdownToken::Code => Ok(Highlight {
                color: None,
                modifier: Modifier::Italic,
            }),
            MarkdownToken::Emphasis => Ok(Highlight {
                color: None,
                modifier: Modifier::Bold,
            }),
            MarkdownToken::Heading => Ok(Highlight {
                color: Some(Color::Blue),
                modifier: Modifier::Underlined,
            }),
            MarkdownToken::Url => Ok(Color::Blue.into()),
            MarkdownToken::Link => Ok(Color::Magenta.into()),
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

impl Highlighter for Markdown {
    fn highlight<'s>(
        &self,
        s: &'s str,
        _state: &'s mut crate::syntax::HighlightState,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
        if s.starts_with('#') {
            Box::new(
                Highlight::try_from(MarkdownToken::Heading)
                    .ok()
                    .map(|h| (h, 0..s.len()))
                    .into_iter(),
            )
        } else if s.starts_with("    ") || s.starts_with('\t') {
            Box::new(
                Highlight::try_from(MarkdownToken::Code)
                    .ok()
                    .map(|h| (h, 0..s.len()))
                    .into_iter(),
            )
        } else {
            Box::new(MarkdownToken::lexer(s).spanned().filter_map(|(t, r)| {
                t.ok()
                    .and_then(|t| Highlight::try_from(t).ok())
                    .map(|c| (c, r))
            }))
        }
    }
}
