// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::syntax::{Highlight, Highlighter};

#[derive(Debug)]
pub struct Git;

impl std::fmt::Display for Git {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Git".fmt(f)
    }
}

impl Highlighter for Git {
    fn highlight<'s>(
        &self,
        s: &'s str,
        _state: &'s mut crate::syntax::HighlightState,
    ) -> Box<dyn Iterator<Item = (Highlight, std::ops::Range<usize>)> + 's> {
        use crate::syntax::color::COMMENT;

        if s.starts_with('#') {
            Box::new(std::iter::once((COMMENT, 0..s.len())))
        } else {
            Box::new(std::iter::empty())
        }
    }
}
