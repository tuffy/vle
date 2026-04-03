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
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum RustToken {
    #[token("abstract")]
    #[token("as")]
    #[token("async")]
    #[token("await")]
    #[token("become")]
    #[token("box")]
    #[token("const")]
    #[token("crate")]
    #[token("do")]
    #[token("dyn")]
    #[token("enum")]
    #[token("extern")]
    #[token("false")]
    #[token("final")]
    #[token("fn")]
    #[token("impl")]
    #[token("in")]
    #[token("let")]
    #[token("macro")]
    #[token("mod")]
    #[token("move")]
    #[token("mut")]
    #[token("override")]
    #[token("priv")]
    #[token("pub")]
    #[token("ref")]
    #[token("self")]
    #[token("static")]
    #[token("struct")]
    #[token("super")]
    #[token("trait")]
    #[token("true")]
    #[token("try")]
    #[token("type")]
    #[token("typeof")]
    #[token("unsafe")]
    #[token("unsized")]
    #[token("use")]
    #[token("where")]
    #[token("virtual")]
    Keyword,

    #[token("break")]
    #[token("continue")]
    #[token("else")]
    #[token("for")]
    #[token("if")]
    #[token("loop")]
    #[token("match")]
    #[token("return")]
    #[token("while")]
    #[token("yield")]
    Flow,

    #[regex("[[:upper:]][[:upper:][:digit:]_]+", priority = 5)]
    Constant,

    #[regex("[[:lower:]][[:lower:][:digit:]_]*")]
    Variable,

    #[regex(r"-?[0-9][0-9_]*(u8|u16|u32|u64|u128|i8|i16|i32|i64|i128)?")]
    #[regex(r"-?[0-9][0-9_]*\.[0-9_]+(e[+-][0-9]+)?")]
    #[regex(r"-?0b[0-1][0-1_]*(u8|u16|u32|u64|u128|i8|i16|i32|i64|i128)?")]
    #[regex(r"-?0x[0-9a-fA-F][0-9a-fA-F_]*(u8|u16|u32|u64|u128|i8|i16|i32|i64|i128)?")]
    #[regex(r"-?0o[0-7][0-7_]*(u8|u16|u32|u64|u128|i8|i16|i32|i64|i128)?")]
    Number,

    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r"'([^\\\']|\\.){0,1}'")]
    #[regex(r"'\\u\{[0-9A-Za-z]+\}'")]
    String,

    #[regex("[[:lower:]_]+!")]
    Macro,

    #[regex("[[:upper:]][[:alnum:]]+")]
    Type,

    #[regex("//.*", allow_greedy = true)]
    Comment,

    #[token("/*")]
    StartComment,

    #[token("*/")]
    EndComment,
}

impl TryFrom<RustToken> for Highlight {
    type Error = ();

    fn try_from(t: RustToken) -> Result<Highlight, ()> {
        match t {
            RustToken::Keyword => Ok(color::KEYWORD),
            RustToken::Constant => Ok(color::CONSTANT),
            RustToken::Macro => Ok(Color::Red.into()),
            RustToken::Flow => Ok(color::FLOW),
            RustToken::Type => Ok(color::TYPE),
            RustToken::Comment | RustToken::StartComment | RustToken::EndComment => {
                Ok(color::COMMENT)
            }
            RustToken::String => Ok(color::STRING),
            RustToken::Number => Ok(color::NUMBER),
            RustToken::Variable => Err(()),
        }
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum RustDef {
    #[regex("fn [[:lower:]][[:lower:][:digit:]_]*")]
    #[regex("struct [[:upper:]][[:alnum:]]+")]
    #[regex("enum [[:upper:]][[:alnum:]]+")]
    #[regex("trait [[:upper:]][[:alnum:]]+")]
    Definition,
    #[regex("type [[:upper:]][[:alnum:]]+")]
    Type,
}

#[derive(Debug)]
pub struct Rust;

impl std::fmt::Display for Rust {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Rust".fmt(f)
    }
}

highlighter!(
    Rust,
    RustToken,
    StartComment,
    EndComment,
    "/*",
    "*/",
    color::COMMENT,
    Some(|s| {
        Box::new(RustDef::lexer(s).spanned().filter_map(|(t, r)| match t {
            Ok(RustDef::Definition) => Some(r),
            Ok(RustDef::Type) => (r.start == 0).then_some(r),
            Err(_) => None,
        }))
    })
);
