// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::{Highlight, color};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum TexToken {
    #[regex("\\\\.|\\\\[[:alpha:]]*")]
    Command,
    #[token("{")]
    #[token("}")]
    Punctuation,
    #[token("$")]
    Math,
    #[regex("%.*", allow_greedy = true)]
    Comment,
}

impl TryFrom<TexToken> for Highlight {
    type Error = ();

    fn try_from(t: TexToken) -> Result<Highlight, ()> {
        match t {
            TexToken::Command => Ok(Color::Green.into()),
            TexToken::Punctuation => Ok(Color::Magenta.into()),
            TexToken::Math => Ok(Color::Red.into()),
            TexToken::Comment => Ok(color::COMMENT),
        }
    }
}

#[derive(Debug)]
pub struct Tex;

impl std::fmt::Display for Tex {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "TeX".fmt(f)
    }
}

highlighter!(Tex, TexToken);
