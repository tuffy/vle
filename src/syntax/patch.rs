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
// #[logos(skip r"[ \t\n]+")]
enum PatchToken {
    #[regex("(Index:|diff|index)[[:blank:]].*", allow_greedy = true)]
    Header,
    #[regex("\\+.*", allow_greedy = true)]
    Added,
    #[regex(" .*", allow_greedy = true)]
    Context,
    #[regex("\\-.*", allow_greedy = true)]
    Deleted,
    #[regex("@@.*", allow_greedy = true)]
    Linenumber,
}

impl TryFrom<PatchToken> for Color {
    type Error = ();

    fn try_from(t: PatchToken) -> Result<Color, ()> {
        match t {
            PatchToken::Header => Ok(Color::Magenta),
            PatchToken::Added => Ok(Color::LightGreen),
            PatchToken::Context => Err(()),
            PatchToken::Deleted => Ok(Color::LightRed),
            PatchToken::Linenumber => Ok(Color::LightYellow),
        }
    }
}

#[derive(Debug)]
pub struct Patch;

impl std::fmt::Display for Patch {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Patch".fmt(f)
    }
}

highlighter!(Patch, PatchToken);
