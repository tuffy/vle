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
mod css;
mod html;
mod js;
mod json;
mod makefile;
mod markdown;
mod python;
mod rust;
mod sql;
mod xml;

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

impl Highlighter for Box<dyn Highlighter> {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::as_ref(self).highlight(s)
    }
}

#[derive(Debug)]
pub struct DefaultHighlighter;

impl Highlighter for DefaultHighlighter {
    fn highlight<'s>(
        &self,
        _s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(std::iter::empty())
    }
}

impl std::fmt::Display for DefaultHighlighter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Plain".fmt(f)
    }
}

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
        Some("py") => Box::new(python::Python),
        Some("json") => Box::new(json::Json),
        Some("md") => Box::new(markdown::Markdown),
        Some("html" | "htm") => Box::new(html::Html),
        Some("xml") => Box::new(xml::Xml),
        Some("sql") => Box::new(sql::Sql),
        Some("css") => Box::new(css::Css),
        Some("js") => Box::new(js::JavaScript),
        _ => Box::new(DefaultHighlighter),
    }
}

// TODO - add cmake syntax
// TODO - add go syntax
// TODO - add java syntax
// TODO - add lua syntax
// TODO - add patch syntax
// TODO - add perl syntax
// TODO - add php syntax
// TODO - add sh syntax
// TODO - add tex syntax
// TODO - add yaml syntax
