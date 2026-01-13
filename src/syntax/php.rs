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
enum PhpToken {
    #[regex(r#"\$[[:alpha:]_][[:alnum:]_]*"#)]
    Variable,

    #[token("array")]
    #[token("bool")]
    #[token("callable")]
    #[token("const")]
    #[token("float")]
    #[token("global")]
    #[token("int")]
    #[token("object")]
    #[token("string")]
    #[token("var")]
    Type,

    #[token("abstract")]
    #[token("as")]
    #[token("class")]
    #[token("clone")]
    #[token("enddeclare")]
    #[token("declare")]
    #[token("extends")]
    #[token("function")]
    #[token("implements")]
    #[token("include")]
    #[token("include_once")]
    #[token("inst")]
    #[token("instance")]
    #[token("interface")]
    #[token("namespace")]
    #[token("new")]
    #[token("private")]
    #[token("protected")]
    #[token("public")]
    #[token("require")]
    #[token("require_once")]
    #[token("static")]
    #[token("trait")]
    #[token("use")]
    #[token("yield")]
    #[token("case")]
    #[token("catch")]
    #[token("default")]
    #[token("do")]
    #[token("echo")]
    #[token("else")]
    #[token("elseif")]
    #[token("end")]
    #[token("for")]
    #[token("foreach")]
    #[token("if")]
    #[token("switch")]
    #[token("final")]
    #[token("print")]
    #[token("throw")]
    #[token("while")]
    #[token("and")]
    #[token("or")]
    #[token("xor")]
    #[token("try")]
    Keyword,

    #[token("break")]
    #[token("continue")]
    #[token("goto")]
    #[token("return")]
    Flow,

    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,

    #[token("true")]
    #[token("false")]
    #[token("TRUE")]
    #[token("FALSE")]
    Constant,

    #[regex("//.*", allow_greedy = true)]
    #[regex(r"/\*.*?\*/")]
    Comment,
}

impl TryFrom<PhpToken> for Color {
    type Error = ();

    fn try_from(t: PhpToken) -> Result<Color, ()> {
        match t {
            PhpToken::Variable => Ok(Color::Cyan),
            PhpToken::Type => Ok(Color::Green),
            PhpToken::Keyword => Ok(Color::LightCyan),
            PhpToken::Flow => Ok(Color::Magenta),
            PhpToken::String => Ok(Color::LightYellow),
            PhpToken::Comment => Ok(Color::LightBlue),
            PhpToken::Constant => Ok(Color::Red),
        }
    }
}

#[derive(Debug)]
pub struct Php;

impl std::fmt::Display for Php {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Php".fmt(f)
    }
}

highlighter!(Php, PhpToken);
