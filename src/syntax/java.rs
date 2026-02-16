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
enum JavaToken {
    #[token("boolean")]
    #[token("byte")]
    #[token("char")]
    #[token("double")]
    #[token("float")]
    #[token("int")]
    #[token("long")]
    #[token("new")]
    #[token("short")]
    #[token("this")]
    #[token("transient")]
    #[token("void")]
    Type,
    #[token("break")]
    #[token("case")]
    #[token("catch")]
    #[token("continue")]
    #[token("default")]
    #[token("do")]
    #[token("else")]
    #[token("finally")]
    #[token("for")]
    #[token("if")]
    #[token("return")]
    #[token("switch")]
    #[token("throw")]
    #[token("try")]
    #[token("while")]
    Flow,
    #[token("abstract")]
    #[token("class")]
    #[token("extends")]
    #[token("final")]
    #[token("implements")]
    #[token("import")]
    #[token("instanceof")]
    #[token("interface")]
    #[token("native")]
    #[token("package")]
    #[token("private")]
    #[token("protected")]
    #[token("public")]
    #[token("static")]
    #[token("strictfp")]
    #[token("super")]
    #[token("synchronized")]
    #[token("throws")]
    #[token("volatile")]
    Keyword,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,
    #[regex("//.*", allow_greedy = true)]
    Comment,
    #[token("/*")]
    StartComment,
    #[token("*/")]
    EndComment,
    #[regex("@[[:alpha:]][[:alpha:].]*?")]
    Annotation,
}

impl TryFrom<JavaToken> for Highlight {
    type Error = ();

    fn try_from(t: JavaToken) -> Result<Highlight, ()> {
        match t {
            JavaToken::Type => Ok(color::TYPE),
            JavaToken::Flow => Ok(color::FLOW),
            JavaToken::Keyword => Ok(color::KEYWORD),
            JavaToken::String => Ok(color::STRING),
            JavaToken::Annotation => Ok(Color::Magenta.into()),
            JavaToken::Comment | JavaToken::StartComment | JavaToken::EndComment => {
                Ok(color::COMMENT)
            }
        }
    }
}

#[derive(Debug)]
pub struct Java;

impl std::fmt::Display for Java {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Java".fmt(f)
    }
}

highlighter!(
    Java,
    JavaToken,
    StartComment,
    EndComment,
    "/*",
    "*/",
    color::COMMENT
);
