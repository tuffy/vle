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
mod json;
mod makefile;
mod markdown;
mod python;
mod rust;

/// Implemented for different syntax highlighters
pub trait Highlighter: std::fmt::Debug + std::fmt::Display {
    /// Yields portions of the string to highlight in a particular color
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's>;

    /// Returns true if the format requires actual tabs instead of spaces
    /// (pretty sure this only applies to Makefiles)
    fn tabs_required(&self) -> bool {
        false
    }
}

/// Which syntax highlighting method is in use
#[derive(Default, Debug)]
pub enum Syntax {
    #[default]
    Plain,
    Rust(rust::Rust),
    C(c::C),
    Python(python::Python),
    Json(json::Json),
    Makefile(makefile::Makefile),
    Markdown(markdown::Markdown),
}

impl Syntax {
    pub fn new(source: &Source) -> Self {
        match source.extension() {
            None => match source.file_name() {
                Some(file_name) => match file_name.as_ref() {
                    "Makefile" | "makefile" => Self::Makefile(makefile::Makefile),
                    _ => Self::default(),
                },
                None => Self::default(),
            },
            Some("rs") => Self::Rust(rust::Rust),
            Some("c" | "h" | "C" | "H") => Self::C(c::C),
            Some("py") => Self::Python(python::Python),
            Some("json") => Self::Json(json::Json),
            Some("md") => Self::Markdown(markdown::Markdown),
            _ => Self::default(),
        }
    }
}

impl Highlighter for Syntax {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        match self {
            Self::Plain => Box::new(std::iter::empty()),
            Self::Rust(r) => r.highlight(s),
            Self::C(c) => c.highlight(s),
            Self::Python(p) => p.highlight(s),
            Self::Json(j) => j.highlight(s),
            Self::Markdown(m) => m.highlight(s),
            Self::Makefile(m) => m.highlight(s),
        }
    }

    fn tabs_required(&self) -> bool {
        match self {
            Self::Plain => false,
            Self::Rust(r) => r.tabs_required(),
            Self::C(c) => c.tabs_required(),
            Self::Python(p) => p.tabs_required(),
            Self::Json(j) => j.tabs_required(),
            Self::Markdown(m) => m.tabs_required(),
            Self::Makefile(m) => m.tabs_required(),
        }
    }
}

impl std::fmt::Display for Syntax {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Plain => "Plain".fmt(f),
            Self::Rust(r) => r.fmt(f),
            Self::C(c) => c.fmt(f),
            Self::Python(p) => p.fmt(f),
            Self::Json(j) => j.fmt(f),
            Self::Markdown(m) => m.fmt(f),
            Self::Makefile(m) => m.fmt(f),
        }
    }
}

// TODO - add cmake syntax
// TODO - add css syntax
// TODO - add go syntax
// TODO - add html syntax
// TODO - add java syntax
// TODO - add javascript syntax
// TODO - add lua syntax
// TODO - add patch syntax
// TODO - add perl syntax
// TODO - add php syntax
// TODO - add sh syntax
// TODO - add sql syntax
// TODO - add tex syntax
// TODO - add xml syntax
// TODO - add yaml syntax
