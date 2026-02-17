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
    #[token("echo")]
    #[token("final")]
    #[token("print")]
    #[token("and")]
    #[token("or")]
    #[token("xor")]
    Keyword,

    #[token("break")]
    #[token("continue")]
    #[token("goto")]
    #[token("return")]
    #[token("yield")]
    #[token("case")]
    #[token("catch")]
    #[token("default")]
    #[token("do")]
    #[token("else")]
    #[token("elseif")]
    #[token("end")]
    #[token("for")]
    #[token("foreach")]
    #[token("if")]
    #[token("switch")]
    #[token("throw")]
    #[token("while")]
    #[token("try")]
    Flow,

    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,

    #[token("true")]
    #[token("false")]
    #[token("TRUE")]
    #[token("FALSE")]
    Constant,

    #[regex("//.*", allow_greedy = true)]
    Comment,

    #[token("/*")]
    StartComment,

    #[token("*/")]
    EndComment,

    #[regex("[[:upper:][:lower:]_][[:upper:][:lower:][:digit:]_]*")]
    Identifier,
}

impl TryFrom<PhpToken> for Highlight {
    type Error = ();

    fn try_from(t: PhpToken) -> Result<Highlight, ()> {
        match t {
            PhpToken::Variable => Ok(Color::Cyan.into()),
            PhpToken::Type => Ok(color::TYPE),
            PhpToken::Keyword => Ok(color::KEYWORD),
            PhpToken::Flow => Ok(color::FLOW),
            PhpToken::String => Ok(color::STRING),
            PhpToken::Comment | PhpToken::StartComment | PhpToken::EndComment => Ok(color::COMMENT),
            PhpToken::Constant => Ok(color::CONSTANT),
            PhpToken::Identifier => Err(()),
        }
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum PhpDef {
    #[regex("function [[:upper:][:lower:]_][[:upper:][:lower:][:digit:]_]*")]
    #[regex("class [[:upper:][:lower:]_][[:upper:][:lower:][:digit:]_]*")]
    Definition,
}

#[derive(Debug)]
pub struct Php;

impl std::fmt::Display for Php {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Php".fmt(f)
    }
}

highlighter!(
    Php,
    PhpToken,
    StartComment,
    EndComment,
    "/*",
    "*/",
    color::COMMENT,
    underliner!(s, PhpDef)
);
