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
enum CToken {
    #[regex("[[:upper:]_][[:upper:][:digit:]_]+")]
    Constant,

    #[regex("[[:lower:][:upper:]][[:lower:][:upper:][:digit:]_]*")]
    Variable,

    #[token("auto")]
    #[token("const")]
    #[token("char")]
    #[token("double")]
    #[token("enum")]
    #[token("extern")]
    #[token("float")]
    #[token("inline")]
    #[token("int")]
    #[token("long")]
    #[token("restrict")]
    #[token("register")]
    #[token("short")]
    #[token("signed")]
    #[token("sizeof")]
    #[token("static")]
    #[token("struct")]
    #[token("typedef")]
    #[token("union")]
    #[token("unsigned")]
    #[token("void")]
    #[token("volatile")]
    Keyword,

    #[token("if")]
    #[token("else")]
    #[token("for")]
    #[token("while")]
    #[token("do")]
    #[token("switch")]
    #[token("case")]
    #[token("default")]
    Flowcontrol1,

    #[token("break")]
    #[token("continue")]
    #[token("goto")]
    #[token("return")]
    Flowcontrol2,

    #[regex("//.*", allow_greedy = true)]
    Comment,

    #[token("/*")]
    StartComment,

    #[token("*/")]
    EndComment,

    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r"'([^\\\']|\\.){0,1}'")]
    String,

    #[regex(r"0x[0-9a-fA-F]+[uU]?(|ll|LL)?")]
    #[regex(r"[0-9]+[uU]?(ll|LL)?")]
    Integer,

    #[token("#define")]
    #[token("#include")]
    #[token("#if")]
    #[token("#ifdef")]
    #[token("#ifndef")]
    #[token("#elif")]
    #[token("#error")]
    #[token("#warning")]
    #[token("#pragma")]
    #[token("#else")]
    #[token("#endif")]
    #[token("#undef")]
    Preprocessor,
}

impl TryFrom<CToken> for Color {
    type Error = ();

    fn try_from(t: CToken) -> Result<Color, ()> {
        match t {
            CToken::Constant => Ok(Color::Red),
            CToken::Integer => Ok(Color::Blue),
            CToken::Keyword => Ok(Color::Green),
            CToken::Flowcontrol1 => Ok(Color::LightYellow),
            CToken::Flowcontrol2 => Ok(Color::Magenta),
            CToken::Comment => Ok(Color::LightBlue),
            CToken::String => Ok(Color::LightYellow),
            CToken::Variable => Err(()),
            CToken::Preprocessor => Ok(Color::LightCyan),
            CToken::StartComment | CToken::EndComment => Ok(Color::Blue),
        }
    }
}

#[derive(Debug)]
pub struct C;

impl std::fmt::Display for C {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "C".fmt(f)
    }
}

highlighter!(C, CToken, StartComment, EndComment, Blue);
