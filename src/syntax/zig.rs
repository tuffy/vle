// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::color;
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum ZigToken {
    #[token("addrspace")]
    #[token("align")]
    #[token("allowzero")]
    #[token("and")]
    #[token("anyframe")]
    #[token("anytime")]
    #[token("asm")]
    #[token("callconv")]
    #[token("comptime")]
    #[token("const")]
    #[token("defer")]
    #[token("enum")]
    #[token("errdefer")]
    #[token("error")]
    #[token("export")]
    #[token("extern")]
    #[token("fn")]
    #[token("inline")]
    #[token("linksection")]
    #[token("noalias")]
    #[token("noinline")]
    #[token("nosuspend")]
    #[token("opaque")]
    #[token("or")]
    #[token("packed")]
    #[token("pub")]
    #[token("resume")]
    #[token("struct")]
    #[token("suspend")]
    #[token("test")]
    #[token("threadlocal")]
    #[token("union")]
    #[token("unreachable")]
    #[token("var")]
    #[token("volatile")]
    Keyword,
    #[token("break")]
    #[token("catch")]
    #[token("continue")]
    #[token("else")]
    #[token("for")]
    #[token("if")]
    #[token("orelse")]
    #[token("return")]
    #[token("switch")]
    #[token("try")]
    #[token("while")]
    Flow,
    #[regex("@[[:upper:][:lower:]]+")]
    BuiltinFunction,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r"'([^\\\']|\\.){0,1}'")]
    String,
    #[token("i8")]
    #[token("u8")]
    #[token("i16")]
    #[token("u16")]
    #[token("i32")]
    #[token("u32")]
    #[token("i64")]
    #[token("u64")]
    #[token("i128")]
    #[token("u128")]
    #[token("isize")]
    #[token("usize")]
    #[token("c_char")]
    #[token("c_short")]
    #[token("c_ushort")]
    #[token("c_int")]
    #[token("c_uint")]
    #[token("c_long")]
    #[token("c_ulong")]
    #[token("c_longlong")]
    #[token("c_ulonglong")]
    #[token("c_longdouble")]
    #[token("f16")]
    #[token("f32")]
    #[token("f64")]
    #[token("f80")]
    #[token("f128")]
    #[token("bool")]
    #[token("anyopaque")]
    #[token("void")]
    #[token("noreturn")]
    #[token("type")]
    #[token("anyerror")]
    #[token("comptime_int")]
    #[token("comptime_float")]
    Type,
    #[regex("[[:upper:][:lower:]_][[:upper:][:lower:][:digit:]_]*")]
    Identifier,
    #[regex("//.*", allow_greedy = true)]
    Comment,
}

impl TryFrom<ZigToken> for Color {
    type Error = ();

    fn try_from(t: ZigToken) -> Result<Color, ()> {
        match t {
            ZigToken::Keyword => Ok(color::KEYWORD),
            ZigToken::Flow => Ok(color::FLOW),
            ZigToken::String => Ok(color::STRING),
            ZigToken::BuiltinFunction => Ok(Color::Cyan),
            ZigToken::Comment => Ok(color::COMMENT),
            ZigToken::Type => Ok(color::TYPE),
            ZigToken::Identifier => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Zig;

impl std::fmt::Display for Zig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Zig".fmt(f)
    }
}

highlighter!(Zig, ZigToken);
