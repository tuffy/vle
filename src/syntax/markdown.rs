// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::Highlight;
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
                modifier: Modifier::Bold,
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

highlighter!(Markdown, MarkdownToken);
