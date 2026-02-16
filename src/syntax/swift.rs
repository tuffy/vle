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
enum SwiftToken {
    #[regex("[[:upper:][:lower:]_][[:upper:][:lower:][:digit:]_]*")]
    Identifier,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r"'([^\\\']|\\.){0,1}'")]
    String,
    #[token("associatedtype")]
    #[token("borrowing")]
    #[token("class")]
    #[token("consuming")]
    #[token("deinit")]
    #[token("enum")]
    #[token("extension")]
    #[token("fileprivate")]
    #[token("func")]
    #[token("import")]
    #[token("init")]
    #[token("inout")]
    #[token("internal")]
    #[token("let")]
    #[token("nonisolated")]
    #[token("open")]
    #[token("operator")]
    #[token("precedencegroup")]
    #[token("private")]
    #[token("protocol")]
    #[token("public")]
    #[token("rethrows")]
    #[token("static")]
    #[token("struct")]
    #[token("subscript")]
    #[token("typealias")]
    #[token("var")]
    #[token("break")]
    #[token("case")]
    #[token("catch")]
    #[token("continue")]
    #[token("default")]
    #[token("defer")]
    #[token("do")]
    #[token("else")]
    #[token("fallthrough")]
    #[token("for")]
    #[token("guard")]
    #[token("if")]
    #[token("in")]
    #[token("repeat")]
    #[token("return")]
    #[token("switch")]
    #[token("throw")]
    #[token("where")]
    #[token("while")]
    #[token("Any")]
    #[token("as")]
    #[token("await")]
    #[token("false")]
    #[token("is")]
    #[token("nil")]
    #[token("self")]
    #[token("Self")]
    #[token("super")]
    #[token("throws")]
    #[token("true")]
    #[token("try")]
    #[token("#available")]
    #[token("#colorLiteral")]
    #[token("#else")]
    #[token("#elseif")]
    #[token("#endif")]
    #[token("#fileLiteral")]
    #[token("#if")]
    #[token("#imageLiteral")]
    #[token("#keyPath")]
    #[token("#selector")]
    #[token("#sourceLocation")]
    #[token("#unavailable")]
    #[token("associativity")]
    #[token("async")]
    #[token("convenience")]
    #[token("didSet")]
    #[token("dynamic")]
    #[token("final")]
    #[token("get")]
    #[token("indirect")]
    #[token("infix")]
    #[token("lazy")]
    #[token("left")]
    #[token("mutating")]
    #[token("none")]
    #[token("nonmutating")]
    #[token("optional")]
    #[token("override")]
    #[token("package")]
    #[token("postfix")]
    #[token("precendence")]
    #[token("prefix")]
    #[token("Protocol")]
    #[token("required")]
    #[token("right")]
    #[token("set")]
    #[token("some")]
    #[token("Type")]
    #[token("unowned")]
    #[token("weak")]
    #[token("willSet")]
    Keyword,
    #[token("Int")]
    #[token("Int32")]
    #[token("Int64")]
    #[token("UInt")]
    #[token("UInt32")]
    #[token("UInt64")]
    #[token("Float16")]
    #[token("Float80")]
    #[token("Float32")]
    #[token("Float64")]
    #[token("String")]
    #[token("Array")]
    #[token("Set")]
    #[token("Dictionary")]
    Type,
    #[regex("//.*", allow_greedy = true)]
    Comment,
    #[token("/*")]
    StartComment,
    #[token("*/")]
    EndComment,
}

impl TryFrom<SwiftToken> for Color {
    type Error = ();

    fn try_from(t: SwiftToken) -> Result<Color, ()> {
        match t {
            SwiftToken::String => Ok(color::STRING),
            SwiftToken::Keyword => Ok(color::KEYWORD),
            SwiftToken::Type => Ok(color::TYPE),
            SwiftToken::Identifier => Err(()),
            SwiftToken::Comment | SwiftToken::StartComment | SwiftToken::EndComment => {
                Ok(color::COMMENT)
            }
        }
    }
}

#[derive(Debug)]
pub struct Swift;

impl std::fmt::Display for Swift {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Swift".fmt(f)
    }
}

highlighter!(
    Swift,
    SwiftToken,
    StartComment,
    EndComment,
    "/*",
    "*/",
    color::COMMENT
);
