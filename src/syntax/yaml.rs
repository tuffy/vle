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
enum YamlToken {
    #[regex("#.*", allow_greedy = true)]
    Comment,
    #[token("[")]
    #[token("]")]
    #[token("{")]
    #[token("}")]
    Symbol,
    #[regex(r#""([^"\\\x00-\x1F]|\\(["\\bnfrt/]|u[a-fA-F0-9]{4}))*""#)]
    String,
    #[regex(r"[[:alpha:]][[:alpha:][:digit:]]*:")]
    Name,
    #[regex(r"-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?")]
    Number,
}

impl TryFrom<YamlToken> for Color {
    type Error = ();

    fn try_from(t: YamlToken) -> Result<Color, ()> {
        match t {
            YamlToken::Comment => Ok(Color::Cyan),
            YamlToken::Symbol => Ok(Color::Yellow),
            YamlToken::String => Ok(Color::LightMagenta),
            YamlToken::Name => Ok(Color::LightGreen),
            YamlToken::Number => Ok(Color::Red),
        }
    }
}

#[derive(Debug)]
pub struct Yaml;

impl std::fmt::Display for Yaml {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "YAML".fmt(f)
    }
}

impl crate::syntax::Highlighter for Yaml {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(
            YamlToken::lexer(s)
                .spanned()
                .filter_map(|(t, r)| t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))),
        )
    }
}
