// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// use crate::highlighter;
use crate::syntax::{Commenting, MultiComment, Plain};
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
    #[token("break")]
    #[token("const")]
    #[token("continue")]
    #[token("crate")]
    #[token("do")]
    #[token("dyn")]
    #[token("else")]
    #[token("enum")]
    #[token("extern")]
    #[token("false")]
    #[token("final")]
    #[token("fn")]
    #[token("for")]
    #[token("if")]
    #[token("impl")]
    #[token("in")]
    #[token("let")]
    #[token("loop")]
    #[token("macro")]
    #[token("match")]
    #[token("mod")]
    #[token("move")]
    #[token("mut")]
    #[token("override")]
    #[token("priv")]
    #[token("pub")]
    #[token("ref")]
    #[token("return")]
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
    #[token("virtual")]
    #[token("where")]
    #[token("while")]
    #[token("yield")]
    Keyword,

    #[regex("[[:upper:]][[:upper:][:digit:]_]+", priority = 5)]
    Constant,

    #[regex("[[:lower:]][[:lower:][:digit:]_]*")]
    Variable,

    #[regex("[[:digit:]]+")]
    Integer,

    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r"'([^\\\']|\\.){0,1}'")]
    String,

    #[regex("[[:lower:]_]+!")]
    Macro,

    #[regex("[[:upper:]][[:alnum:]]+")]
    Type,

    #[regex("//.*", allow_greedy = true)]
    Comment,

    #[token("/*")]
    StartComment,

    EndComment,

    #[regex("fn [[:lower:][:digit:]_]+")]
    Function,
}

impl TryFrom<RustToken> for Color {
    type Error = ();

    fn try_from(t: RustToken) -> Result<Color, ()> {
        match t {
            RustToken::Keyword => Ok(Color::Yellow),
            RustToken::Constant => Ok(Color::Magenta),
            RustToken::Macro => Ok(Color::Red),
            RustToken::Type => Ok(Color::Magenta),
            RustToken::Comment | RustToken::StartComment | RustToken::EndComment => Ok(Color::Blue),
            RustToken::Function => Ok(Color::Magenta),
            RustToken::String => Ok(Color::Green),
            RustToken::Variable | RustToken::Integer => Err(()),
        }
    }
}

impl Plain for RustToken {
    fn is_comment_start(&self) -> bool {
        matches!(self, Self::StartComment)
    }
}

impl Commenting for RustToken {
    fn is_comment_end(&self) -> bool {
        matches!(self, Self::EndComment)
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum RustComment {
    #[token("/*")]
    Start,
    #[token("*/")]
    End,
}

impl From<RustComment> for MultiComment {
    fn from(c: RustComment) -> MultiComment {
        match c {
            RustComment::Start => MultiComment::Start,
            RustComment::End => MultiComment::End,
        }
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum RustCommentEnd {
    #[token("*/")]
    EndComment,
}

impl From<RustCommentEnd> for RustToken {
    fn from(c: RustCommentEnd) -> Self {
        match c {
            RustCommentEnd::EndComment => Self::EndComment,
        }
    }
}

impl Commenting for RustCommentEnd {
    fn is_comment_end(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct Rust;

impl std::fmt::Display for Rust {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Rust".fmt(f)
    }
}

impl crate::syntax::Highlighter for Rust {
    fn highlight<'s>(
        &self,
        s: &'s str,
        state: &'s mut crate::syntax::HighlightState,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        use crate::syntax::{EitherLexer, HighlightState};

        let lexer: EitherLexer<RustToken, RustCommentEnd> = EitherLexer::new(&state, s);

        Box::new(lexer.filter_map(move |(t, r)| {
            match state {
                HighlightState::Normal => t
                    .ok()
                    .inspect(|t| {
                        if t.is_comment_start() {
                            *state = HighlightState::Commenting;
                        }
                    })
                    .and_then(|t| Color::try_from(t).ok())
                    .map(|c| (c, r)),
                HighlightState::Commenting => Some(match t {
                    Ok(end) if end.is_comment_end() => {
                        *state = HighlightState::default();
                        (Color::try_from(end).ok()?, r)
                    }
                    _ => (Color::Blue, r),
                }),
            }
        }))
    }

    fn multicomment(&self) -> Option<fn(&str) -> Option<crate::syntax::MultiComment>> {
        Some(|s: &str| RustComment::lexer(s).find_map(|token| token.ok().map(|t| t.into())))
    }
}
// highlighter!(Rust, RustToken, StartComment, EndComment, Blue);
