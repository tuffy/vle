// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::{Commenting, Plain};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum CppToken {
    #[regex("[[:upper:]_][[:upper:][:digit:]_]+")]
    Constant,

    #[regex("[[:lower:][:upper:]_][[:lower:][:upper:][:digit:]_]*")]
    Variable,

    #[token("alignas")]
    #[token("alignof")]
    #[token("asm")]
    #[token("auto")]
    #[token("bool")]
    #[token("const")]
    #[token("consteval")]
    #[token("constexpr")]
    #[token("constinit")]
    #[token("const_cast")]
    #[token("contract_assert")]
    #[token("char")]
    #[token("char8_t")]
    #[token("char16_t")]
    #[token("char32_t")]
    #[token("class")]
    #[token("concept")]
    #[token("decltype")]
    #[token("double")]
    #[token("dynamic_cast")]
    #[token("enum")]
    #[token("extern")]
    #[token("explicit")]
    #[token("export")]
    #[token("float")]
    #[token("friend")]
    #[token("import")]
    #[token("inline")]
    #[token("int")]
    #[token("long")]
    #[token("module")]
    #[token("mutable")]
    #[token("namespace")]
    #[token("new")]
    #[token("operator")]
    #[token("private")]
    #[token("protected")]
    #[token("public")]
    #[token("reinterpret_cast")]
    #[token("requires")]
    #[token("restrict")]
    #[token("register")]
    #[token("short")]
    #[token("signed")]
    #[token("sizeof")]
    #[token("static")]
    #[token("static_assert")]
    #[token("static_cast")]
    #[token("struct")]
    #[token("template")]
    #[token("typedef")]
    #[token("typeid")]
    #[token("typename")]
    #[token("union")]
    #[token("using")]
    #[token("unsigned")]
    #[token("virtual")]
    #[token("void")]
    #[token("volatile")]
    #[token("wchar_t")]
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

    #[token("try")]
    #[token("throw")]
    #[token("catch")]
    #[token("co_await")]
    #[token("co_return")]
    #[token("co_yield")]
    #[token("noexcept")]
    Flowcontrol3,

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

    #[token("true")]
    #[token("false")]
    #[token("nullptr")]
    Literal,

    #[token("and")]
    #[token("compl")]
    #[token("or_eq")]
    #[token("and_eq")]
    #[token("not")]
    #[token("xor")]
    #[token("bitand")]
    #[token("not_eq")]
    #[token("xor_eq")]
    #[token("bitor")]
    #[token("or")]
    Operator,

    #[token("#define")]
    #[token("#include")]
    #[token("#if")]
    #[token("#ifdef")]
    #[token("#ifndef")]
    #[token("#elif")]
    #[token("#elifdef")]
    #[token("#elifndef")]
    #[token("#error")]
    #[token("#embed")]
    #[token("#line")]
    #[token("#warning")]
    #[token("#pragma")]
    #[token("#else")]
    #[token("#endif")]
    #[token("#undef")]
    Preprocessor,
}

impl TryFrom<CppToken> for Color {
    type Error = ();

    fn try_from(t: CppToken) -> Result<Color, ()> {
        match t {
            CppToken::Constant | CppToken::Operator => Ok(Color::Red),
            CppToken::Integer | CppToken::Literal => Ok(Color::Blue),
            CppToken::Keyword => Ok(Color::Green),
            CppToken::Flowcontrol1 => Ok(Color::LightYellow),
            CppToken::Flowcontrol2 => Ok(Color::Magenta),
            CppToken::Flowcontrol3 => Ok(Color::LightMagenta),
            CppToken::Comment => Ok(Color::LightBlue),
            CppToken::String => Ok(Color::LightYellow),
            CppToken::Variable => Err(()),
            CppToken::Preprocessor => Ok(Color::LightCyan),
            CppToken::StartComment | CppToken::EndComment => Ok(Color::Blue),
        }
    }
}

#[derive(Debug)]
pub struct Cpp;

impl std::fmt::Display for Cpp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "C++".fmt(f)
    }
}

highlighter!(Cpp, CppToken, StartComment, EndComment, "/*", "*/", Blue);
