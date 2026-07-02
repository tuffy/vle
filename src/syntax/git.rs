// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::syntax::{Highlight, Highlighter, Syntax};

#[derive(Debug)]
pub struct Git;

impl std::fmt::Display for Git {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Git".fmt(f)
    }
}

impl Syntax for Git {
    fn initialize(
        &self,
        _rope: &ropey::Rope,
        _viewport_line: usize,
        _viewport_height: u16,
    ) -> Box<dyn Highlighter> {
        Box::new(GitHighlighter)
    }

    fn initialize_find(&self) -> Box<dyn Highlighter> {
        Box::new(GitHighlighter)
    }
}

struct GitHighlighter;

impl Highlighter for GitHighlighter {
    fn highlight<'s>(
        &'s mut self,
        line: &'s str,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
        use crate::syntax::color::COMMENT;

        if line.starts_with('#') {
            Box::new(std::iter::once((COMMENT, 0..line.len())))
        } else {
            Box::new(std::iter::empty())
        }
    }
}
