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

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum TypeScriptToken {
    #[token("async")]
    #[token("class")]
    #[token("const")]
    #[token("extends")]
    #[token("function")]
    #[token("let")]
    #[token("this")]
    #[token("typeof")]
    #[token("var")]
    #[token("void")]
    #[token("interface")]
    #[token("declare")]
    #[token("as")]
    Keyword,
    #[token("do")]
    #[token("while")]
    #[token("if")]
    #[token("else")]
    #[token("switch")]
    #[token("case")]
    #[token("default")]
    #[token("for")]
    #[token("each")]
    #[token("in")]
    #[token("of")]
    #[token("with")]
    #[token("await")]
    #[token("export")]
    #[token("import")]
    #[token("throw")]
    #[token("try")]
    #[token("catch")]
    #[token("finally")]
    #[token("new")]
    #[token("delete")]
    Flow,
    #[token("break")]
    #[token("continue")]
    #[token("return")]
    #[token("yield")]
    Break,
    #[regex("([0-9]+|0x[[:xdigit:]]+)")]
    Number,
    #[regex("'([^']|\\')*'")]
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,
    #[token("string")]
    #[token("number")]
    #[token("boolean")]
    #[token("any")]
    #[token("Array")]
    #[token("Promise")]
    Type,
    #[token("null")]
    #[token("undefined")]
    Constant,
    #[regex("[[:lower:][:upper:]][[:lower:][:upper:][:digit:]_]*")]
    Identifier,
    #[regex("//.*", allow_greedy = true)]
    Comment,
    #[token("/*")]
    StartComment,
    #[token("*/")]
    EndComment,
}

impl TryFrom<TypeScriptToken> for Highlight {
    type Error = ();

    fn try_from(t: TypeScriptToken) -> Result<Highlight, ()> {
        match t {
            TypeScriptToken::Comment
            | TypeScriptToken::StartComment
            | TypeScriptToken::EndComment => Ok(color::COMMENT),
            TypeScriptToken::Keyword => Ok(color::KEYWORD),
            TypeScriptToken::Flow | TypeScriptToken::Break => Ok(color::FLOW),
            TypeScriptToken::String => Ok(color::STRING),
            TypeScriptToken::Number => Ok(color::NUMBER),
            TypeScriptToken::Type => Ok(color::TYPE),
            TypeScriptToken::Constant => Ok(color::CONSTANT),
            TypeScriptToken::Identifier => Err(()),
        }
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum TypeScriptDef {
    #[regex("function [[:lower:][:upper:]][[:lower:][:upper:][:digit:]_]*")]
    #[token("function")]
    #[regex("class [[:lower:][:upper:]][[:lower:][:upper:][:digit:]_]*")]
    #[regex("interface [[:lower:][:upper:]][[:lower:][:upper:][:digit:]_]*")]
    Definition,
}

#[derive(Debug)]
pub struct TypeScript;

impl std::fmt::Display for TypeScript {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "TypeScript".fmt(f)
    }
}

highlighter!(
    TypeScript,
    TypeScriptToken,
    StartComment,
    EndComment,
    "/*",
    "*/",
    color::COMMENT,
    underliner!(s, TypeScriptDef)
);
