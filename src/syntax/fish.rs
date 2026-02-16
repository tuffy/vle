// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::color;
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum FishToken {
    #[regex("#.*", allow_greedy = true)]
    Comment,
    #[token("alias")]
    #[token("bind")]
    #[token("builtin")]
    #[token("cd")]
    #[token("command")]
    #[token("echo")]
    #[token("eval")]
    #[token("exec")]
    #[token("exit")]
    #[token("false")]
    #[token("fg")]
    #[token("function")]
    #[token("help")]
    #[token("history")]
    #[token("jobs")]
    #[token("kill")]
    #[token("set")]
    #[token("true")]
    #[token("umask")]
    #[token("wait")]
    Keyword,
    #[token("and")]
    #[token("or")]
    #[token("not")]
    #[token("begin")]
    #[token("end")]
    #[token("if")]
    #[token("else")]
    #[token("while")]
    #[token("for")]
    #[token("switch")]
    #[token("case")]
    #[token("break")]
    #[token("continue")]
    #[token("return")]
    #[token("in")]
    Loop,
    #[regex("\\$[[:alnum:]_]+")]
    Variable,
    #[regex("\\S+", priority = 3)]
    Misc,
}

impl TryFrom<FishToken> for Color {
    type Error = ();

    fn try_from(t: FishToken) -> Result<Color, ()> {
        match t {
            FishToken::Comment => Ok(color::COMMENT),
            FishToken::Keyword => Ok(color::KEYWORD),
            FishToken::Loop => Ok(color::FLOW),
            FishToken::Variable => Ok(Color::Cyan),
            FishToken::Misc => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Fish;

impl std::fmt::Display for Fish {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Fish".fmt(f)
    }
}

highlighter!(Fish, FishToken);
