// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::buffer::Source;
use ratatui::style::Color;

mod c;
mod cpp;
mod css;
mod csv;
mod fish;
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
mod rust;
mod sh;
mod sql;
mod swift;
mod tex;
mod toml;
mod tutorial;
mod xml;
mod yaml;
mod zig;

#[derive(Default)]
pub enum HighlightState {
    #[default]
    Normal,
    Commenting,
}

/// A multi-line comment start or end
pub enum MultiComment {
    Start,
    End,
}

/// Implemented for different syntax highlighters
pub trait Highlighter: std::fmt::Debug + std::fmt::Display {
    /// Yields portions of the string to highlight in a particular color
    fn highlight<'s>(
        &self,
        s: &'s str,
        state: &'s mut HighlightState,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's>;

    /// Returns true if the format requires actual tabs instead of spaces
    /// (pretty sure this only applies to Makefiles)
    fn tabs_required(&self) -> bool {
        false
    }

    /// If format supports multi-line comments,
    /// returns function which returns the first one that
    /// exists in a line, if any
    fn multicomment(&self) -> Option<fn(&str) -> Option<MultiComment>> {
        None
    }
}

impl Highlighter for Box<dyn Highlighter> {
    fn highlight<'s>(
        &self,
        s: &'s str,
        state: &'s mut HighlightState,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::as_ref(self).highlight(s, state)
    }

    fn tabs_required(&self) -> bool {
        Box::as_ref(self).tabs_required()
    }

    fn multicomment(&self) -> Option<fn(&str) -> Option<MultiComment>> {
        Box::as_ref(self).multicomment()
    }
}

#[derive(Debug)]
pub struct DefaultHighlighter;

impl Highlighter for DefaultHighlighter {
    fn highlight<'s>(
        &self,
        _s: &'s str,
        _state: &'s mut HighlightState,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(std::iter::empty())
    }
}

impl std::fmt::Display for DefaultHighlighter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Plain".fmt(f)
    }
}

pub use regex::Regex;
pub use tutorial::Tutorial;

pub fn syntax(source: &Source) -> Box<dyn Highlighter> {
    match source.extension() {
        None => match source.file_name() {
            Some(file_name) => match file_name.as_ref() {
                "Makefile" | "makefile" => Box::new(makefile::Makefile),
                _ => Box::new(DefaultHighlighter),
            },
            None => Box::new(DefaultHighlighter),
        },
        Some("rs") => Box::new(rust::Rust),
        Some("c" | "h" | "C" | "H") => Box::new(c::C),
        Some("cpp" | "cc" | "cxx" | "c++" | "hh" | "hpp" | "hxx" | "h++") => Box::new(cpp::Cpp),
        Some("py") => Box::new(python::Python),
        Some("json") => Box::new(json::Json),
        Some("md") => Box::new(markdown::Markdown),
        Some("html" | "htm") => Box::new(html::Html),
        Some("xml" | "svg") => Box::new(xml::Xml),
        Some("sql") => Box::new(sql::Sql),
        Some("css") => Box::new(css::Css),
        Some("js") => Box::new(js::JavaScript),
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
        _ => Box::new(DefaultHighlighter),
    }
}

#[macro_export]
macro_rules! highlighter {
    ($syntax:ty, $token:ty) => {
        impl $crate::syntax::Highlighter for $syntax {
            fn highlight<'s>(
                &self,
                s: &'s str,
                _state: &'s mut $crate::syntax::HighlightState,
            ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
                Box::new(<$token>::lexer(s).spanned().filter_map(|(t, r)| {
                    t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))
                }))
            }
        }
    };
    ($syntax:ty, $token:ty, $comment_start:ident, $comment_end:ident, $comment_color:ident) => {
        impl $crate::syntax::Highlighter for $syntax {
            fn highlight<'s>(
                &self,
                s: &'s str,
                state: &'s mut $crate::syntax::HighlightState,
            ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
                use $crate::syntax::HighlightState;

                Box::new(<$token>::lexer(s).spanned().filter_map(move |(t, r)| {
                    match state {
                        HighlightState::Normal => t
                            .ok()
                            .inspect(|t| {
                                if matches!(t, <$token>::$comment_start) {
                                    *state = HighlightState::Commenting;
                                }
                            })
                            .and_then(|t| Color::try_from(t).ok())
                            .map(|c| (c, r)),
                        HighlightState::Commenting => Some(match t {
                            Ok(end @ <$token>::$comment_end) => {
                                *state = HighlightState::default();
                                (Color::try_from(end).ok()?, r)
                            }
                            _ => (Color::$comment_color, r),
                        }),
                    }
                }))
            }

            fn multicomment(&self) -> Option<fn(&str) -> Option<$crate::syntax::MultiComment>> {
                use $crate::syntax::MultiComment;

                Some(|s: &str| {
                    <$token>::lexer(s).find_map(|token| match token {
                        Ok(<$token>::$comment_start) => Some(MultiComment::Start),
                        Ok(<$token>::$comment_end) => Some(MultiComment::End),
                        _ => None,
                    })
                })
            }
        }
    };
}
