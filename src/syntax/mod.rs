use ratatui::style::Color;

/// Implemented for different syntax highlighters
pub trait Highlighter: std::fmt::Debug + std::fmt::Display {
    /// Yields portions of the string to highlight in a particular color
    fn highlight(&self, s: &str) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + '_>;
}

/// Which syntax highlighting method is in use
#[derive(Default, Debug)]
pub enum Syntax {
    #[default]
    Plain,
    Rust(Rust),
}

impl Syntax {
    pub fn new(extension: &str) -> Self {
        match extension {
            "rs" => Self::Rust(Rust),
            _ => Self::default(),
        }
    }
}

impl Highlighter for Syntax {
    fn highlight(&self, s: &str) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + '_> {
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

#[derive(Debug)]
pub struct Rust;

impl std::fmt::Display for Rust {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Rust".fmt(f)
    }
}

impl Highlighter for Rust {
    fn highlight(&self, s: &str) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + '_> {
        Box::new(
            s.find("match")
                .map(|start| (Color::Blue, start..start + 5))
                .into_iter(),
        )
    }
}
