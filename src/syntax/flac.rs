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
enum AnalysisToken {
    #[regex("[a-z_]+")]
    Name,
    #[regex(r"-?[0-9]+")]
    Number,
    #[token("INDEPENDENT")]
    #[token("LEFT_SIDE")]
    #[token("SIDE_RIGHT")]
    #[token("MID_SIDE")]
    #[token("CONSTANT")]
    #[token("VERBATIM")]
    #[token("FIXED")]
    #[token("LPC")]
    #[token("RICE")]
    #[token("RICE2")]
    Literal,
    #[regex(r"\[[0-9]+\]")]
    Index,
}

impl TryFrom<AnalysisToken> for Highlight {
    type Error = ();

    fn try_from(t: AnalysisToken) -> Result<Highlight, ()> {
        match t {
            AnalysisToken::Name | AnalysisToken::Number => Err(()),
            AnalysisToken::Literal => Ok(Color::Magenta.into()),
            AnalysisToken::Index => Ok(Color::Blue.into()),
        }
    }
}

#[derive(Debug)]
pub struct Analysis;

impl std::fmt::Display for Analysis {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "FLAC Analysis".fmt(f)
    }
}

highlighter!(Analysis, AnalysisToken);
