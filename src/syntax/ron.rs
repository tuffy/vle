// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::{Commenting, Highlight, Plain, color};
use logos::Logos;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum RonToken {
    #[regex("[[:lower:]][[:lower:][:digit:]_]*")]
    Variable,

    #[token("true")]
    #[token("false")]
    Boolean,

    #[regex(r"-?[0-9][0-9_]*")]
    #[regex(r"-?[0-9][0-9_]*\.[0-9_]+(e[+-][0-9]+)?")]
    #[regex(r"-?0b[0-1][0-1_]*")]
    #[regex(r"-?0x[0-9a-fA-F][0-9a-fA-F_]*")]
    #[regex(r"-?0o[0-7][0-7_]*")]
    Number,

    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r"'([^\\\']|\\.){0,1}'")]
    String,

    #[regex("[[:upper:]][[:alnum:]]+")]
    Type,

    #[regex("//.*", allow_greedy = true)]
    Comment,

    #[token("/*")]
    StartComment,

    #[token("*/")]
    EndComment,
}

impl TryFrom<RonToken> for Highlight {
    type Error = ();

    fn try_from(t: RonToken) -> Result<Highlight, ()> {
        match t {
            RonToken::Type => Ok(color::TYPE),
            RonToken::Comment | RonToken::StartComment | RonToken::EndComment => Ok(color::COMMENT),
            RonToken::String => Ok(color::STRING),
            RonToken::Number => Ok(color::NUMBER),
            RonToken::Boolean => Ok(color::CONSTANT),
            RonToken::Variable => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Ron;

impl std::fmt::Display for Ron {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Ron".fmt(f)
    }
}

highlighter!(
    Ron,
    RonToken,
    StartComment,
    EndComment,
    "/*",
    "*/",
    color::COMMENT
);
