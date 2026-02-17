// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::syntax::{Commenting, Highlight, Plain, color};
use crate::{highlighter, underliner};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum GoToken {
    #[token("bool")]
    #[token("uint8")]
    #[token("uint16")]
    #[token("uint32")]
    #[token("uint64")]
    #[token("int8")]
    #[token("int16")]
    #[token("int32")]
    #[token("int64")]
    #[token("float32")]
    #[token("float64")]
    #[token("complex64")]
    #[token("complex128")]
    #[token("byte")]
    #[token("rune")]
    #[token("uintptr")]
    #[token("string")]
    #[token("error")]
    #[token("chan")]
    #[token("const")]
    #[token("interface")]
    #[token("map")]
    #[token("struct")]
    #[token("type")]
    #[token("var")]
    Type,
    #[token("append")]
    #[token("cap")]
    #[token("close")]
    #[token("complex")]
    #[token("copy")]
    #[token("delete")]
    #[token("func")]
    #[token("imag")]
    #[token("len")]
    #[token("make")]
    #[token("new")]
    #[token("panic")]
    #[token("print")]
    #[token("println")]
    #[token("real")]
    #[token("recover")]
    Keyword,
    #[token("case")]
    #[token("default")]
    #[token("defer")]
    #[token("else")]
    #[token("for")]
    #[token("go")]
    #[token("if")]
    #[token("range")]
    #[token("select")]
    #[token("switch")]
    #[token("break")]
    #[token("continue")]
    #[token("fallthrough")]
    #[token("goto")]
    #[token("return")]
    Flow,
    #[token("package")]
    #[token("import")]
    Declaration,
    #[token("false")]
    #[token("true")]
    #[token("nil")]
    #[token("iota")]
    Literal,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,
    #[regex("//.*", allow_greedy = true)]
    Comment,
    #[token("/*")]
    StartComment,
    #[token("*/")]
    EndComment,
    #[regex("[[:lower:][:upper:]_][[:lower:][:upper:][:digit:]_]*")]
    Identifier,
}

impl TryFrom<GoToken> for Highlight {
    type Error = ();

    fn try_from(t: GoToken) -> Result<Highlight, ()> {
        match t {
            GoToken::Type => Ok(color::TYPE),
            GoToken::Keyword => Ok(color::KEYWORD),
            GoToken::Flow => Ok(color::FLOW),
            GoToken::Declaration => Ok(Color::LightCyan.into()),
            GoToken::Literal => Ok(Color::Red.into()),
            GoToken::String => Ok(color::STRING),
            GoToken::Comment | GoToken::StartComment | GoToken::EndComment => Ok(color::COMMENT),
            GoToken::Identifier => Err(()),
        }
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum GoDef {
    #[regex("func [[:lower:][:upper:]_][[:lower:][:upper:][:digit:]_]*")]
    #[regex(r"func \([^\)]+?\) [[:lower:][:upper:]_][[:lower:][:upper:][:digit:]_]*")]
    #[token("func")]
    #[regex("type [[:lower:][:upper:]_][[:lower:][:upper:][:digit:]_]*")]
    Definition,
}

#[derive(Debug)]
pub struct Go;

impl std::fmt::Display for Go {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Go".fmt(f)
    }
}

highlighter!(
    Go,
    GoToken,
    StartComment,
    EndComment,
    "/*",
    "*/",
    color::COMMENT,
    underliner!(s, GoDef)
);
