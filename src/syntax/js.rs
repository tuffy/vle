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
enum JavaScriptToken {
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
    #[regex("[[:lower:]][[:lower:][:digit:]_]*")]
    Identifier,
    #[regex("//.*", allow_greedy = true)]
    Comment,
    #[token("/*")]
    StartComment,
    #[token("*/")]
    EndComment,
}

impl TryFrom<JavaScriptToken> for Color {
    type Error = ();

    fn try_from(t: JavaScriptToken) -> Result<Color, ()> {
        match t {
            JavaScriptToken::Comment | JavaScriptToken::StartComment | JavaScriptToken::EndComment => Ok(Color::LightBlue),
            JavaScriptToken::Keyword => Ok(Color::Green),
            JavaScriptToken::Flow => Ok(Color::LightYellow),
            JavaScriptToken::Break => Ok(Color::Magenta),
            JavaScriptToken::String => Ok(Color::LightMagenta),
            JavaScriptToken::Number => Ok(Color::Cyan),
            JavaScriptToken::Identifier => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct JavaScript;

impl std::fmt::Display for JavaScript {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "JavaScript".fmt(f)
    }
}

highlighter!(JavaScript, JavaScriptToken, StartComment, EndComment, LightBlue);
