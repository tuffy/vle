// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::{Commenting, Plain, color};
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
    #[token("func")]
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
    #[token("imag")]
    #[token("len")]
    #[token("make")]
    #[token("new")]
    #[token("panic")]
    #[token("print")]
    #[token("println")]
    #[token("real")]
    #[token("recover")]
    Function,
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
    Control,
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
}

impl TryFrom<GoToken> for Color {
    type Error = ();

    fn try_from(t: GoToken) -> Result<Color, ()> {
        match t {
            GoToken::Type => Ok(color::TYPE),
            GoToken::Function => Ok(color::FUNCTION),
            GoToken::Control => Ok(Color::LightYellow),
            GoToken::Flow => Ok(color::FLOW),
            GoToken::Declaration => Ok(Color::LightCyan),
            GoToken::Literal => Ok(Color::Red),
            GoToken::String => Ok(color::STRING),
            GoToken::Comment | GoToken::StartComment | GoToken::EndComment => Ok(color::COMMENT),
        }
    }
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
    color::COMMENT
);
