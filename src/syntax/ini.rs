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
enum IniToken {
    #[regex("[[:alpha:]][[:alpha:][:digit:]_-]*?\\s*?=\\s*?")]
    Key,
    #[regex(";.*", allow_greedy = true)]
    #[regex("#.*", allow_greedy = true)]
    Comment,
    #[regex("\\[.+\\]", allow_greedy = true)]
    Section,
}

impl TryFrom<IniToken> for Color {
    type Error = ();

    fn try_from(t: IniToken) -> Result<Color, ()> {
        match t {
            IniToken::Key => Ok(Color::Blue),
            IniToken::Comment => Ok(Color::LightRed),
            IniToken::Section => Ok(Color::Green),
        }
    }
}

#[derive(Debug)]
pub struct Ini;

impl std::fmt::Display for Ini {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "TOML".fmt(f)
    }
}

highlighter!(Ini, IniToken);
