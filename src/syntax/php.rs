// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::syntax::{Highlight, Highlighter, Syntax, Underliner, color, html::HtmlToken};
use logos::{Lexer, Logos};
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum PhpToken {
    #[regex(r#"\$[[:alpha:]_][[:alnum:]_]*"#)]
    Variable,

    #[token("array")]
    #[token("bool")]
    #[token("callable")]
    #[token("const")]
    #[token("float")]
    #[token("global")]
    #[token("int")]
    #[token("object")]
    #[token("string")]
    #[token("var")]
    Type,

    #[token("abstract")]
    #[token("as")]
    #[token("class")]
    #[token("clone")]
    #[token("enddeclare")]
    #[token("declare")]
    #[token("extends")]
    #[token("function")]
    #[token("implements")]
    #[token("include")]
    #[token("include_once")]
    #[token("inst")]
    #[token("instance")]
    #[token("interface")]
    #[token("namespace")]
    #[token("new")]
    #[token("private")]
    #[token("protected")]
    #[token("public")]
    #[token("require")]
    #[token("require_once")]
    #[token("static")]
    #[token("trait")]
    #[token("use")]
    #[token("echo")]
    #[token("final")]
    #[token("print")]
    #[token("and")]
    #[token("or")]
    #[token("xor")]
    Keyword,

    #[token("break")]
    #[token("continue")]
    #[token("goto")]
    #[token("return")]
    #[token("yield")]
    #[token("case")]
    #[token("catch")]
    #[token("default")]
    #[token("do")]
    #[token("else")]
    #[token("elseif")]
    #[token("end")]
    #[token("for")]
    #[token("foreach")]
    #[token("if")]
    #[token("switch")]
    #[token("throw")]
    #[token("while")]
    #[token("try")]
    Flow,

    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    #[regex(r#"\'([^\\\']|\\.)*\'"#)]
    String,

    #[token("true")]
    #[token("false")]
    #[token("TRUE")]
    #[token("FALSE")]
    Constant,

    #[regex("//.*", allow_greedy = true)]
    Comment,

    #[token("/*")]
    StartComment,

    #[token("*/")]
    EndComment,

    #[regex("[[:upper:][:lower:]_][[:upper:][:lower:][:digit:]_]*")]
    Identifier,

    #[token("?>")]
    PhpEnd,

    Html(HtmlToken),
}

impl TryFrom<PhpToken> for Highlight {
    type Error = ();

    fn try_from(t: PhpToken) -> Result<Highlight, ()> {
        match t {
            PhpToken::Variable => Ok(Color::Cyan.into()),
            PhpToken::Type => Ok(color::TYPE),
            PhpToken::Keyword => Ok(color::KEYWORD),
            PhpToken::Flow => Ok(color::FLOW),
            PhpToken::String => Ok(color::STRING),
            PhpToken::Comment | PhpToken::StartComment | PhpToken::EndComment => Ok(color::COMMENT),
            PhpToken::Constant => Ok(color::CONSTANT),
            PhpToken::Identifier | PhpToken::PhpEnd => Err(()),
            PhpToken::Html(t) => t.try_into(),
        }
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum PhpDef {
    #[regex("function [[:upper:][:lower:]_][[:upper:][:lower:][:digit:]_]*")]
    #[regex("class [[:upper:][:lower:]_][[:upper:][:lower:][:digit:]_]*")]
    Definition,
}

#[derive(Debug)]
pub struct Php;

impl std::fmt::Display for Php {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "PHP".fmt(f)
    }
}

impl Syntax for Php {
    fn initialize(
        &self,
        rope: &ropey::Rope,
        viewport_line: usize,
        viewport_height: u16,
    ) -> Box<dyn Highlighter> {
        use std::borrow::Cow;

        #[derive(Logos, Debug)]
        #[logos(skip r"[ \t\n]+")]
        enum PhpInit {
            #[token("<?php")]
            PhpStart,
            #[token("?>")]
            PhpEnd,
            #[token("/*")]
            PhpCommentStart,
            #[token("*/")]
            PhpCommentEnd,
            #[token("<!--")]
            HtmlCommentStart,
            #[token("-->")]
            HtmlCommentEnd,
            #[regex(r#"\"([^\\\"]|\\.)*\""#)]
            #[regex(r#"\'([^\\\']|\\.)*\'"#)]
            String,
        }

        Box::new(
            rope.lines_at(viewport_line)
                .take(viewport_height.into())
                .find_map(|line| {
                    PhpInit::lexer(&Cow::from(line)).find_map(|token| match token {
                        Ok(PhpInit::PhpStart) => Some(PhpHighlighter::Html),
                        Ok(PhpInit::PhpEnd) => Some(PhpHighlighter::Php),
                        Ok(PhpInit::PhpCommentStart) => Some(PhpHighlighter::Php),
                        Ok(PhpInit::PhpCommentEnd) => Some(PhpHighlighter::PhpComment),
                        Ok(PhpInit::HtmlCommentStart) => Some(PhpHighlighter::Html),
                        Ok(PhpInit::HtmlCommentEnd) => Some(PhpHighlighter::HtmlComment),
                        Ok(PhpInit::String) | Err(()) => None,
                    })
                })
                .unwrap_or(PhpHighlighter::Php),
        )
    }

    fn initialize_find(&self) -> Box<dyn Highlighter> {
        Box::new(PhpHighlighter::Php)
    }
}

enum PhpHighlighter {
    Php,
    PhpComment,
    Html,
    HtmlComment,
}

impl Highlighter for PhpHighlighter {
    fn highlight<'s>(
        &'s mut self,
        line: &'s str,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
        let lexer = match self {
            Self::Php => PhpLexer::Php(Lexer::new(line)),
            Self::PhpComment => PhpLexer::PhpCommentEnd(Lexer::new(line)),
            Self::Html => PhpLexer::Html(Lexer::new(line)),
            Self::HtmlComment => PhpLexer::HtmlCommentEnd(Lexer::new(line)),
        };

        Box::new(lexer.filter_map(move |(t, r)| {
            match self {
                Self::Php => t
                    .ok()
                    .inspect(|t| match t {
                        PhpToken::StartComment => {
                            *self = Self::PhpComment;
                        }
                        PhpToken::PhpEnd => {
                            *self = Self::Html;
                        }
                        _ => { /* do nothing */ }
                    })
                    .and_then(|t| Highlight::try_from(t).ok())
                    .map(|c| (c, r)),
                Self::PhpComment => Some(match t {
                    Ok(end @ PhpToken::EndComment) => {
                        *self = Self::Php;
                        (Highlight::try_from(end).ok()?, r)
                    }
                    _ => (color::COMMENT, r),
                }),
                Self::Html => t
                    .ok()
                    .inspect(|t| match t {
                        PhpToken::Html(HtmlToken::StartComment) => {
                            *self = Self::HtmlComment;
                        }
                        PhpToken::Html(HtmlToken::PhpStart) => {
                            *self = Self::Php;
                        }
                        _ => { /* do nothing */ }
                    })
                    .and_then(|t| Highlight::try_from(t).ok())
                    .map(|c| (c, r)),
                Self::HtmlComment => Some(match t {
                    Ok(end @ PhpToken::Html(HtmlToken::EndComment)) => {
                        *self = Self::Html;
                        (Highlight::try_from(end).ok()?, r)
                    }
                    Ok(end @ PhpToken::Html(HtmlToken::PhpStart)) => {
                        *self = Self::Php;
                        (Highlight::try_from(end).ok()?, r)
                    }
                    _ => (color::COMMENT, r),
                }),
            }
        }))
    }

    fn underline(&self) -> Option<Underliner> {
        match self {
            Self::Php => Some(|line| {
                Box::new(
                    PhpDef::lexer(line)
                        .spanned()
                        .filter_map(|(t, r)| t.ok().map(|_| r)),
                )
            }),
            _ => None,
        }
    }
}

enum PhpLexer<'s> {
    Php(Lexer<'s, PhpToken>),
    PhpCommentEnd(Lexer<'s, PhpCommentEnd>),
    Html(Lexer<'s, HtmlToken>),
    HtmlCommentEnd(Lexer<'s, HtmlCommentEnd>),
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum PhpCommentEnd {
    #[token("*/")]
    EndComment,
}

impl From<PhpCommentEnd> for PhpToken {
    fn from(c: PhpCommentEnd) -> Self {
        match c {
            PhpCommentEnd::EndComment => Self::EndComment,
        }
    }
}

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum HtmlCommentEnd {
    #[token("-->")]
    EndComment,
    #[token("<?php")]
    EndHtml,
}

impl From<HtmlCommentEnd> for PhpToken {
    fn from(c: HtmlCommentEnd) -> Self {
        match c {
            HtmlCommentEnd::EndComment => Self::Html(HtmlToken::EndComment),
            HtmlCommentEnd::EndHtml => Self::Html(HtmlToken::PhpStart),
        }
    }
}

impl<'s> Iterator for PhpLexer<'s> {
    type Item = (Result<PhpToken, ()>, std::ops::Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Php(lexer) => {
                let token = lexer.next()?;
                let span = lexer.span();
                match &token {
                    Ok(PhpToken::StartComment) => {
                        *self =
                            Self::PhpCommentEnd(std::mem::replace(lexer, Lexer::new("")).morph());
                    }
                    Ok(PhpToken::PhpEnd) => {
                        *self = Self::Html(std::mem::replace(lexer, Lexer::new("")).morph());
                    }
                    _ => { /* do nothing */ }
                }
                Some((token, span))
            }
            Self::PhpCommentEnd(lexer) => {
                let token = lexer.next()?;
                let span = lexer.span();
                match token {
                    Ok(token @ PhpCommentEnd::EndComment) => {
                        *self = Self::Php(std::mem::replace(lexer, Lexer::new("")).morph());
                        Some((Ok(token.into()), span))
                    }
                    Err(err) => Some((Err(err), span)),
                }
            }
            Self::Html(lexer) => {
                let token = lexer.next()?;
                let span = lexer.span();
                match &token {
                    Ok(HtmlToken::StartComment) => {
                        *self =
                            Self::HtmlCommentEnd(std::mem::replace(lexer, Lexer::new("")).morph());
                    }
                    Ok(HtmlToken::PhpStart) => {
                        *self = Self::Php(std::mem::replace(lexer, Lexer::new("")).morph());
                    }
                    _ => { /* do nothing */ }
                }
                Some((token.map(PhpToken::Html), span))
            }
            Self::HtmlCommentEnd(lexer) => {
                let token = lexer.next()?;
                let span = lexer.span();
                match token {
                    Ok(token @ HtmlCommentEnd::EndComment) => {
                        *self = Self::Html(std::mem::replace(lexer, Lexer::new("")).morph());
                        Some((Ok(token.into()), span))
                    }
                    Ok(token @ HtmlCommentEnd::EndHtml) => {
                        *self = Self::Php(std::mem::replace(lexer, Lexer::new("")).morph());
                        Some((Ok(token.into()), span))
                    }
                    Err(err) => Some((Err(err), span)),
                }
            }
        }
    }
}
