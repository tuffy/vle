// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::syntax::{Commenting, HighlightState, Highlighter, MultiCommentType, Plain, color};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum PythonToken {
    #[regex("def [[:alpha:]_][[:alnum:]_.]*")]
    Function,
    #[token("and")]
    #[token("as")]
    #[token("assert")]
    #[token("async")]
    #[token("await")]
    #[token("break")]
    #[token("class")]
    #[token("continue")]
    #[token("def")]
    #[token("del")]
    #[token("elif")]
    #[token("else")]
    #[token("except")]
    #[token("finally")]
    #[token("for")]
    #[token("from")]
    #[token("global")]
    #[token("if")]
    #[token("import")]
    #[token("in")]
    #[token("is")]
    #[token("lambda")]
    #[token("nonlocal")]
    #[token("not")]
    #[token("or")]
    #[token("pass")]
    #[token("raise")]
    #[token("return")]
    #[token("try")]
    #[token("while")]
    #[token("with")]
    #[token("yield")]
    Keyword,
    #[token("True")]
    #[token("False")]
    #[token("None")]
    Literal,
    #[regex("@[[:alpha:]_][[:alnum:]_.]*")]
    Decorator,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r"'([^\\']|\\.)*'")]
    String,
    #[token("\"\"\"")]
    #[token("'''")]
    MultiLineString,
    #[regex("#.*", allow_greedy = true)]
    Comment,
    #[regex("[[:alpha:]_][[:alnum:]_.]*")]
    Variable,
}

impl TryFrom<PythonToken> for Color {
    type Error = ();

    fn try_from(t: PythonToken) -> Result<Color, ()> {
        match t {
            PythonToken::Function => Ok(color::FUNCTION),
            PythonToken::Keyword => Ok(color::KEYWORD),
            PythonToken::Literal => Ok(Color::LightMagenta),
            PythonToken::Decorator => Ok(Color::Cyan),
            PythonToken::String | PythonToken::MultiLineString => Ok(color::STRING),
            PythonToken::Comment => Ok(color::COMMENT),
            PythonToken::Variable => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Python;

impl std::fmt::Display for Python {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Python".fmt(f)
    }
}

impl Plain for PythonToken {
    fn is_comment_start(&self) -> bool {
        matches!(self, Self::MultiLineString)
    }
}

impl Commenting for PythonToken {
    fn is_comment_end(&self) -> bool {
        matches!(self, Self::MultiLineString)
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum MultiLineString {
    #[token("\"\"\"")]
    #[token("'''")]
    StartEnd,
}

impl From<MultiLineString> for PythonToken {
    fn from(s: MultiLineString) -> Self {
        match s {
            MultiLineString::StartEnd => Self::MultiLineString,
        }
    }
}

impl Commenting for MultiLineString {
    fn is_comment_end(&self) -> bool {
        true
    }
}

impl Highlighter for Python {
    fn highlight<'s>(
        &self,
        s: &'s str,
        state: &'s mut crate::syntax::HighlightState,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        use crate::syntax::EitherLexer;

        let lexer: EitherLexer<PythonToken, MultiLineString> = EitherLexer::new(state, s);

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
                    _ => (color::STRING, r),
                }),
            }
        }))
    }

    fn multicomment(&self) -> Option<MultiCommentType> {
        Some(MultiCommentType::Unidirectional(|acc, s| {
            MultiLineString::lexer(s).fold(acc, |acc, s| match s {
                Ok(MultiLineString::StartEnd) => match acc {
                    HighlightState::Normal => HighlightState::Commenting,
                    HighlightState::Commenting => HighlightState::Normal,
                },
                Err(()) => acc,
            })
        }))
    }
}
