use crate::buffer::Source;
use ratatui::style::Color;

mod rust;

/// Implemented for different syntax highlighters
pub trait Highlighter: std::fmt::Debug + std::fmt::Display {
    /// Yields portions of the string to highlight in a particular color
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's>;
}

/// Which syntax highlighting method is in use
#[derive(Default, Debug)]
pub enum Syntax {
    #[default]
    Plain,
    Rust(rust::Rust),
}

impl Syntax {
    pub fn new(source: &Source) -> Self {
        match source.extension() {
            Some("rs") => Self::Rust(rust::Rust),
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
        }
    }
}

impl std::fmt::Display for Syntax {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Plain => "Plain".fmt(f),
            Self::Rust(r) => r.fmt(f),
        }
    }
}

// TODO - add cmake syntax
// TODO - add c syntax
// TODO - add css syntax
// TODO - add go syntax
// TODO - add html syntax
// TODO - add java syntax
// TODO - add javascript syntax
// TODO - add json syntax
// TODO - add lua syntax
// TODO - add makefile syntax
// TODO - add markdown syntax
// TODO - add patch syntax
// TODO - add perl syntax
// TODO - add php syntax
// TODO - add python syntax
// TODO - add sh syntax
// TODO - add sql syntax
// TODO - add tex syntax
// TODO - add xml syntax
// TODO - add yaml syntax
