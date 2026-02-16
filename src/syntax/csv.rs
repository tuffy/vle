// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::syntax::Highlight;
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[\n]+")]
enum CsvToken {
    #[token(",")]
    #[token(";")]
    #[token("|")]
    Separator,
    #[regex(r#"\"[^\"]*\""#)]
    Quoted,
}

#[derive(Debug)]
pub struct Csv;

impl std::fmt::Display for Csv {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "CSV".fmt(f)
    }
}

impl crate::syntax::Highlighter for Csv {
    fn highlight<'s>(
        &self,
        s: &'s str,
        _state: &'s mut crate::syntax::HighlightState,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
        let colors = &[
            Color::Blue,
            Color::Green,
            Color::Magenta,
            Color::Cyan,
            Color::Red,
            Color::LightBlue,
            Color::LightGreen,
            Color::LightMagenta,
            Color::LightCyan,
            Color::LightRed,
        ];

        let mut next_color = colors.iter().cycle();
        let mut color = next_color.next().unwrap();

        Box::new(
            CsvToken::lexer(s)
                .spanned()
                .filter_map(move |(t, r)| match t {
                    Ok(CsvToken::Separator) => {
                        color = next_color.next().unwrap();
                        None
                    }
                    Ok(CsvToken::Quoted) | Err(_) => Some(((*color).into(), r)),
                }),
        )
    }
}
