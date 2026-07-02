// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::buffer::Source;
use logos::{Lexer, Logos};
use ratatui::style::Color;

mod c;
mod cpp;
mod css;
mod csv;
mod cue;
mod fish;
mod flac;
mod git;
mod go;
mod html;
mod ini;
mod java;
mod js;
mod json;
mod makefile;
mod markdown;
mod patch;
mod perl;
mod php;
mod python;
mod regex;
mod ron;
mod rust;
mod sh;
mod sql;
mod swift;
mod test;
mod tex;
mod todo;
mod toml;
mod ts;
mod tutorial;
mod xml;
mod yaml;
mod zig;

// This editor is intended to be used on terminals with both
// light text on dark backgrounds as well as dark text on light backgrounds
// without having to modify any colors or probe for the terminal's color scheme.
// As such, predefined colors (red, green, yellow, blue, magenta, etc.)
// should be preferred instead of RGB colors (since users can redefine them)
// and black/white should be avoided altogether.
// Boldface is also difficult to detect in a dark color scheme
// and shouldn't be relied upon.

/// A subset of all of Ratatui's possible modifiers
#[derive(Copy, Clone, Default)]
pub enum Modifier {
    #[default]
    Plain,
    Bold,
    Italic,
    Underlined,
    Strikethrough,
}

#[derive(Copy, Clone)]
pub struct Highlight {
    pub color: Option<Color>,
    pub modifier: Modifier,
}

impl From<Color> for Highlight {
    fn from(color: Color) -> Self {
        Self {
            color: Some(color),
            modifier: Modifier::default(),
        }
    }
}

impl From<Highlight> for ratatui::style::Style {
    fn from(highlight: Highlight) -> Self {
        let style = match highlight.modifier {
            Modifier::Plain => Self::default(),
            Modifier::Italic => Self::default().italic(),
            Modifier::Underlined => Self::default().underlined(),
            Modifier::Bold => Self::default().bold(),
            Modifier::Strikethrough => Self::default().crossed_out(),
        };
        match highlight.color {
            Some(color) => style.fg(color),
            None => style,
        }
    }
}

pub trait Highlighter {
    fn highlight<'s>(
        &'s mut self,
        line: &'s str,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's>;

    /// Yields portions of the string to underline
    fn underline(&self) -> Option<Underliner> {
        None
    }
}

impl Highlighter for Box<dyn Highlighter> {
    fn highlight<'s>(
        &'s mut self,
        line: &'s str,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
        Box::as_mut(self).highlight(line)
    }

    fn underline(&self) -> Option<Underliner> {
        Box::as_ref(self).underline()
    }
}

type Underliner = for<'s> fn(&'s str) -> Box<dyn Iterator<Item = std::ops::Range<usize>> + 's>;

/// Implemented for different syntax highlighters
pub trait Syntax: std::fmt::Debug + std::fmt::Display {
    fn initialize(
        &self,
        rope: &ropey::Rope,
        viewport_line: usize,
        viewport_height: u16,
    ) -> Box<dyn Highlighter>;

    fn initialize_find(&self) -> Box<dyn Highlighter>;

    /// Returns true if the format requires actual tabs instead of spaces
    /// (pretty sure this only applies to Makefiles)
    fn tabs_required(&self) -> bool {
        false
    }
}

impl Syntax for Box<dyn Syntax> {
    fn initialize(
        &self,
        rope: &ropey::Rope,
        viewport_line: usize,
        viewport_height: u16,
    ) -> Box<dyn Highlighter> {
        Box::as_ref(self).initialize(rope, viewport_line, viewport_height)
    }

    fn initialize_find(&self) -> Box<dyn Highlighter> {
        Box::as_ref(self).initialize_find()
    }

    fn tabs_required(&self) -> bool {
        Box::as_ref(self).tabs_required()
    }
}

#[derive(Debug)]
pub struct DefaultSyntax;

impl Syntax for DefaultSyntax {
    fn initialize(
        &self,
        _rope: &ropey::Rope,
        _viewport_line: usize,
        _viewport_height: u16,
    ) -> Box<dyn Highlighter> {
        Box::new(DefaultSyntaxHighlighter)
    }

    fn initialize_find(&self) -> Box<dyn Highlighter> {
        Box::new(DefaultSyntaxHighlighter)
    }
}

struct DefaultSyntaxHighlighter;

impl Highlighter for DefaultSyntaxHighlighter {
    fn highlight<'s>(
        &'s mut self,
        _line: &'s str,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
        Box::new(std::iter::empty())
    }
}

impl std::fmt::Display for DefaultSyntax {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Plain".fmt(f)
    }
}

pub use regex::Regex;
pub use test::Test;
pub use tutorial::Tutorial;

pub trait Plain {
    fn is_comment_start(&self) -> bool;
}

pub trait Commenting {
    fn is_comment_end(&self) -> bool;
}

pub enum EitherLexer<'s, P: Logos<'s>, C: Logos<'s>> {
    Plain(Lexer<'s, P>),
    Commenting(Lexer<'s, C>),
}

impl<'s, P, C> EitherLexer<'s, P, C>
where
    P: Logos<'s, Extras: Default>,
    C: Logos<'s, Source = P::Source, Extras = P::Extras>,
{
    pub fn normal(source: &'s <P as Logos<'s>>::Source) -> Self {
        Self::Plain(Lexer::new(source))
    }

    pub fn commenting(source: &'s <P as Logos<'s>>::Source) -> Self {
        Self::Commenting(Lexer::new(source))
    }
}

impl<'s, P, C> Iterator for EitherLexer<'s, P, C>
where
    P: Logos<'s, Source = str, Extras: Default> + Plain,
    C: Logos<'s, Source = P::Source, Extras = P::Extras, Error = P::Error> + Commenting + Into<P>,
{
    type Item = (Result<P, P::Error>, std::ops::Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Plain(lexer) => {
                let token = lexer.next()?;
                let pair = (token, lexer.span());
                if let (Ok(token), _) = &pair
                    && token.is_comment_start()
                {
                    *self =
                        EitherLexer::Commenting(std::mem::replace(lexer, Lexer::new("")).morph());
                }
                Some(pair)
            }
            Self::Commenting(lexer) => {
                let token = lexer.next()?;
                let span = lexer.span();
                match token {
                    Ok(token) => {
                        if token.is_comment_end() {
                            *self = EitherLexer::Plain(
                                std::mem::replace(lexer, Lexer::new("")).morph(),
                            );
                        }
                        Some((Ok(token.into()), span))
                    }
                    Err(err) => Some((Err(err), span)),
                }
            }
        }
    }
}

pub fn syntax(source: &Source) -> Box<dyn Syntax> {
    use std::collections::HashMap;
    use std::sync::LazyLock;

    static EXT_MAP: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
        std::env::var("VLE_EXT_MAP")
            .ok()
            .map(|whole| {
                whole
                    .split(',')
                    .filter_map(|s| {
                        s.split_once('=')
                            .map(|(from, to)| (from.trim().to_string(), to.trim().to_string()))
                            .filter(|(from, to)| !from.is_empty() && !to.is_empty())
                    })
                    .collect()
            })
            .unwrap_or_default()
    });

    if matches!(source, Source::Test) {
        return Box::new(Test);
    }

    match source
        .extension()
        .map(|ext| EXT_MAP.get(ext).map(|s| s.as_str()).unwrap_or(ext))
    {
        None => match source.file_name() {
            Some(file_name) => match file_name.as_ref() {
                "Makefile" | "makefile" => Box::new(makefile::Makefile),
                "COMMIT_EDITMSG" => Box::new(git::Git),
                _ => Box::new(DefaultSyntax),
            },
            None => Box::new(DefaultSyntax),
        },
        Some("rs") => Box::new(rust::Rust),
        Some("c" | "h" | "C" | "H") => Box::new(c::C),
        Some("cpp" | "cc" | "cxx" | "c++" | "hh" | "hpp" | "hxx" | "h++") => Box::new(cpp::Cpp),
        Some("py") => Box::new(python::Python),
        Some("json") => Box::new(json::Json),
        Some("ron") => Box::new(ron::Ron),
        Some("md" | "markdown") => Box::new(markdown::Markdown),
        Some("html" | "htm") => Box::new(html::Html),
        Some("xml" | "svg") => Box::new(xml::Xml),
        Some("sql") => Box::new(sql::Sql),
        Some("css") => Box::new(css::Css),
        Some("js") => Box::new(js::JavaScript),
        Some("ts") => Box::new(ts::TypeScript),
        Some("php") => Box::new(php::Php),
        Some("yaml") => Box::new(yaml::Yaml),
        Some("java") => Box::new(java::Java),
        Some("go") => Box::new(go::Go),
        Some("patch" | "diff") => Box::new(patch::Patch),
        Some("csv") => Box::new(csv::Csv),
        Some("toml") => Box::new(toml::Toml),
        Some("ini") => Box::new(ini::Ini),
        Some("fish") => Box::new(fish::Fish),
        Some("sh") => Box::new(sh::Shell),
        Some("zig") => Box::new(zig::Zig),
        Some("swift") => Box::new(swift::Swift),
        Some("pl" | "pm") => Box::new(perl::Perl),
        Some("tex") => Box::new(tex::Tex),
        Some("ana") => Box::new(flac::Analysis),
        Some("cue" | "CUE") => Box::new(cue::Cuesheet),
        Some("txt") if source.file_name().as_deref() == Some("todo.txt") => Box::new(todo::Todo),
        _ => Box::new(DefaultSyntax),
    }
}

#[macro_export]
macro_rules! define_syntax {
    ($syntax:ty, $token:ty) => {
        define_syntax!($syntax, $token, None);
    };
    ($syntax:ty, $token:ty, $underliner:expr) => {
        impl $crate::syntax::Syntax for $syntax {
            fn initialize(
                &self,
                _rope: &ropey::Rope,
                _viewport_line: usize,
                _viewport_height: u16,
            ) -> Box<dyn $crate::syntax::Highlighter> {
                Box::new(SyntaxHighlighter)
            }

            fn initialize_find(&self) -> Box<dyn $crate::syntax::Highlighter> {
                Box::new(SyntaxHighlighter)
            }
        }

        struct SyntaxHighlighter;

        impl $crate::syntax::Highlighter for SyntaxHighlighter {
            fn highlight<'s>(
                &'s mut self,
                line: &'s str,
            ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
                Box::new(<$token>::lexer(line).spanned().filter_map(|(t, r)| {
                    t.ok()
                        .and_then(|t| Highlight::try_from(t).ok())
                        .map(|c| (c, r))
                }))
            }

            fn underline(&self) -> Option<$crate::syntax::Underliner> {
                $underliner
            }
        }
    };
    ($syntax:ty, $token:ty, $comment_start:ident, $comment_end:ident, $start:literal, $end:literal, $comment_color:expr) => {
        define_syntax!(
            $syntax,
            $token,
            $comment_start,
            $comment_end,
            $start,
            $end,
            $comment_color,
            None
        );
    };
    ($syntax:ty, $token:ty, $comment_start:ident, $comment_end:ident, $start:literal, $end:literal, $comment_color:expr, $underliner:expr) => {
        impl Plain for $token {
            fn is_comment_start(&self) -> bool {
                matches!(self, Self::$comment_start)
            }
        }

        impl Commenting for $token {
            fn is_comment_end(&self) -> bool {
                matches!(self, Self::$comment_end)
            }
        }

        #[derive(Logos, Debug)]
        #[logos(skip r"[ \t\n]+")]
        enum CommentEnd {
            #[token($end)]
            EndComment,
        }

        impl From<CommentEnd> for $token {
            fn from(c: CommentEnd) -> Self {
                match c {
                    CommentEnd::EndComment => Self::$comment_end,
                }
            }
        }

        impl Commenting for CommentEnd {
            fn is_comment_end(&self) -> bool {
                true
            }
        }

        impl $crate::syntax::Syntax for $syntax {
            fn initialize(
                &self,
                rope: &ropey::Rope,
                viewport_line: usize,
                viewport_height: u16,
            ) -> Box<dyn $crate::syntax::Highlighter> {
                use std::borrow::Cow;

                #[derive(Logos, Debug)]
                #[logos(skip r"[ \t\n]+")]
                enum Comment {
                    #[token($start)]
                    Start,
                    #[token($end)]
                    End,
                }

                impl From<Comment> for SyntaxHighlighter {
                    fn from(comment: Comment) -> Self {
                        match comment {
                            Comment::Start => SyntaxHighlighter::Normal,
                            Comment::End => SyntaxHighlighter::Commenting,
                        }
                    }
                }

                Box::new(
                    rope.lines_at(viewport_line)
                        .take(viewport_height.into())
                        .find_map(|line| {
                            Comment::lexer(&Cow::from(line))
                                .find_map(|token| token.ok().map(|t| t.into()))
                        })
                        .unwrap_or(SyntaxHighlighter::Normal),
                )
            }

            fn initialize_find(&self) -> Box<dyn $crate::syntax::Highlighter> {
                Box::new(SyntaxHighlighter::Normal)
            }
        }

        enum SyntaxHighlighter {
            Normal,
            Commenting,
        }

        impl $crate::syntax::Highlighter for SyntaxHighlighter {
            fn highlight<'s>(
                &'s mut self,
                line: &'s str,
            ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
                use $crate::syntax::EitherLexer;

                let lexer: EitherLexer<$token, CommentEnd> = match self {
                    Self::Normal => EitherLexer::normal(line),
                    Self::Commenting => EitherLexer::commenting(line),
                };

                Box::new(lexer.filter_map(move |(t, r)| {
                    match self {
                        Self::Normal => t
                            .ok()
                            .inspect(|t| {
                                if t.is_comment_start() {
                                    *self = Self::Commenting;
                                }
                            })
                            .and_then(|t| Highlight::try_from(t).ok())
                            .map(|c| (c, r)),
                        Self::Commenting => Some(match t {
                            Ok(end) if end.is_comment_end() => {
                                *self = Self::Normal;
                                (Highlight::try_from(end).ok()?, r)
                            }
                            _ => ($comment_color, r),
                        }),
                    }
                }))
            }

            /// Yields portions of the string to underline
            fn underline(&self) -> Option<$crate::syntax::Underliner> {
                $underliner
            }
        }
    };
}

#[macro_export]
macro_rules! underliner {
    ($s:ident, $class:ty) => {
        Some(|$s| {
            Box::new(
                <$class>::lexer($s)
                    .spanned()
                    .filter_map(|(t, r)| t.ok().map(|_| r)),
            )
        })
    };
}

pub mod color {
    use crate::syntax::{Highlight, Modifier};
    use ratatui::style::Color;

    // A unified color scheme across common syntax items

    pub const KEYWORD: Highlight = Highlight {
        color: Some(Color::Blue),
        modifier: Modifier::Plain,
    };
    pub const FLOW: Highlight = Highlight {
        color: Some(Color::Blue),
        modifier: Modifier::Plain,
    };
    pub const CONSTANT: Highlight = Highlight {
        color: Some(Color::Red),
        modifier: Modifier::Plain,
    };
    pub const TYPE: Highlight = Highlight {
        color: Some(Color::Magenta),
        modifier: Modifier::Plain,
    };
    pub const COMMENT: Highlight = Highlight {
        color: Some(Color::DarkGray),
        modifier: Modifier::Italic,
    };
    pub const STRING: Highlight = Highlight {
        color: Some(Color::Green),
        modifier: Modifier::Plain,
    };
    pub const NUMBER: Highlight = Highlight {
        color: Some(Color::Cyan),
        modifier: Modifier::Plain,
    };
}
