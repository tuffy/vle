// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

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
    #[token("bool")]
    #[token("char")]
    #[token("const")]
    #[token("double")]
    #[token("enum")]
    #[token("extern")]
    #[token("float")]
    #[token("inline")]
    #[token("int")]
    #[token("long")]
    #[token("restrict")]
    #[token("short")]
    #[token("signed")]
    #[token("sizeof")]
    #[token("static")]
    #[token("struct")]
    #[token("typedef")]
    #[token("union")]
    #[token("unsigned")]
    #[token("void")]
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
    #[regex(r"/\*.*?\*/")]
    Comment,

    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,

    #[regex(r"#[[:blank:]]*(define|include|if(n?def)?|elif|error|warning|pragma|else|endif)")]
    Preprocessor,
}

impl TryFrom<CToken> for Color {
    type Error = ();

    fn try_from(t: CToken) -> Result<Color, ()> {
        match t {
            CToken::Constant => Ok(Color::Red),
            CToken::Keyword => Ok(Color::Green),
            CToken::Flowcontrol1 => Ok(Color::LightYellow),
            CToken::Flowcontrol2 => Ok(Color::Magenta),
            CToken::Comment => Ok(Color::LightBlue),
            CToken::String => Ok(Color::LightYellow),
            CToken::Variable => Err(()),
            CToken::Preprocessor => Ok(Color::LightCyan),
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

impl crate::syntax::Highlighter for C {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(
            CToken::lexer(s)
                .spanned()
                .filter_map(|(t, r)| t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))),
        )
    }
}
