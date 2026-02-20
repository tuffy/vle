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
enum CuesheetToken {
    #[token("PERFORMER")]
    #[token("TITLE")]
    #[token("FILE")]
    #[token("TRACK")]
    #[token("INDEX")]
    #[token("CATALOG")]
    #[token("ISRC")]
    #[token("FLAGS")]
    #[token("PRE")]
    Name,
    #[token("AUDIO")]
    #[token("NON_AUDIO")]
    Type,
    #[regex(r#"\"[^\"]*\""#)]
    String,
    #[regex("REM .*", allow_greedy = true)]
    Comment,
}

impl TryFrom<CuesheetToken> for Highlight {
    type Error = ();

    fn try_from(t: CuesheetToken) -> Result<Highlight, ()> {
        match t {
            CuesheetToken::Name => Ok(color::KEYWORD),
            CuesheetToken::Type => Ok(Color::Magenta.into()),
            CuesheetToken::String => Ok(color::STRING),
            CuesheetToken::Comment => Ok(color::COMMENT),
        }
    }
}

#[derive(Debug)]
pub struct Cuesheet;

impl std::fmt::Display for Cuesheet {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "CUE Sheet".fmt(f)
    }
}

highlighter!(Cuesheet, CuesheetToken);
