// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub trait TextPrompt: Default + std::fmt::Display {
    fn push(&mut self, c: char);

    fn extend(&mut self, s: &str);

    fn pop(&mut self) -> Option<char>;

    fn is_empty(&self) -> bool;

    fn chars(&self) -> impl Iterator<Item = char>;

    fn cursor_column(&self) -> usize {
        use unicode_width::UnicodeWidthChar;

        self.chars()
            .map(|c| match c {
                '\t' => *crate::buffer::SPACES_PER_TAB,
                c => c.width().unwrap_or(0),
            })
            .sum()
    }
}

pub enum SearchPrompt {
    Plain(PlaintextPrompt),
    Regex(RegexPrompt),
}

impl SearchPrompt {
    pub fn reset(&mut self) {
        match self {
            Self::Plain(p) => {
                *p = PlaintextPrompt::default();
            }
            Self::Regex(r) => {
                *r = RegexPrompt::default();
            }
        }
    }

    pub fn swap(&mut self) {
        *self = match self {
            Self::Plain(_) => Self::Regex(RegexPrompt::default()),
            Self::Regex(_) => Self::Plain(PlaintextPrompt::default()),
        }
    }
}

impl Default for SearchPrompt {
    fn default() -> Self {
        Self::Plain(PlaintextPrompt::default())
    }
}

impl std::fmt::Display for SearchPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Plain(p) => p.fmt(f),
            Self::Regex(r) => r.fmt(f),
        }
    }
}

impl TextPrompt for SearchPrompt {
    fn push(&mut self, c: char) {
        match self {
            Self::Plain(p) => p.push(c),
            Self::Regex(r) => r.push(c),
        }
    }

    fn extend(&mut self, s: &str) {
        match self {
            Self::Plain(p) => p.extend(s),
            Self::Regex(r) => r.extend(s),
        }
    }

    fn pop(&mut self) -> Option<char> {
        match self {
            Self::Plain(p) => p.pop(),
            Self::Regex(r) => r.pop(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Plain(p) => p.is_empty(),
            Self::Regex(r) => r.is_empty(),
        }
    }

    fn chars(&self) -> impl Iterator<Item = char> {
        match self {
            Self::Plain(p) => Box::new(p.chars()) as Box<dyn Iterator<Item = char>>,
            Self::Regex(r) => Box::new(r.chars()),
        }
    }
}

#[derive(Default)]
pub struct PlaintextPrompt {
    chars: Vec<char>,
    value: String,
}

impl PlaintextPrompt {
    fn recompile(&mut self) {
        self.value = self.chars.iter().copied().collect();
    }

    pub fn value(&self) -> Option<&str> {
        (!self.is_empty()).then_some(self.value.as_str())
    }
}

impl std::fmt::Display for PlaintextPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Find Plain".fmt(f)
    }
}

impl TextPrompt for PlaintextPrompt {
    fn push(&mut self, c: char) {
        self.chars.push(c);
        self.recompile();
    }

    fn extend(&mut self, s: &str) {
        self.chars.extend(s.chars());
        self.recompile();
    }

    fn pop(&mut self) -> Option<char> {
        let c = self.chars.pop();
        self.recompile();
        c
    }

    fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    fn chars(&self) -> impl Iterator<Item = char> {
        self.chars.iter().copied()
    }
}

#[derive(Default)]
pub struct RegexPrompt {
    chars: Vec<char>,
    value: Option<Result<regex_lite::Regex, regex_lite::Error>>,
}

impl RegexPrompt {
    fn recompile(&mut self) {
        self.value = (!self.chars.is_empty())
            .then(|| regex_lite::Regex::new(self.chars.iter().collect::<String>().as_str()));
    }

    pub fn value(&self) -> Option<Result<&regex_lite::Regex, &regex_lite::Error>> {
        self.value.as_ref().map(|r| r.as_ref())
    }
}

impl std::fmt::Display for RegexPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Find Regex".fmt(f)
    }
}

impl TextPrompt for RegexPrompt {
    fn push(&mut self, c: char) {
        self.chars.push(c);
        self.recompile();
    }

    fn extend(&mut self, s: &str) {
        self.chars.extend(s.chars());
        self.recompile();
    }

    fn pop(&mut self) -> Option<char> {
        let c = self.chars.pop();
        self.recompile();
        c
    }

    fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    fn chars(&self) -> impl Iterator<Item = char> {
        self.chars.iter().copied()
    }
}

#[derive(Copy, Clone)]
pub enum Digit {
    Digit0 = 0,
    Digit1 = 1,
    Digit2 = 2,
    Digit3 = 3,
    Digit4 = 4,
    Digit5 = 5,
    Digit6 = 6,
    Digit7 = 7,
    Digit8 = 8,
    Digit9 = 9,
}

impl TryFrom<char> for Digit {
    type Error = char;

    fn try_from(c: char) -> Result<Self, Self::Error> {
        match c {
            '0' => Ok(Digit::Digit0),
            '1' => Ok(Digit::Digit1),
            '2' => Ok(Digit::Digit2),
            '3' => Ok(Digit::Digit3),
            '4' => Ok(Digit::Digit4),
            '5' => Ok(Digit::Digit5),
            '6' => Ok(Digit::Digit6),
            '7' => Ok(Digit::Digit7),
            '8' => Ok(Digit::Digit8),
            '9' => Ok(Digit::Digit9),
            c => Err(c),
        }
    }
}

#[derive(Default)]
pub struct LinePrompt {
    line: Vec<Digit>,
}

impl LinePrompt {
    pub const MAX: usize = 9;

    pub fn push(&mut self, d: Digit) {
        if !(self.line.is_empty() && matches!(d, Digit::Digit0)) && self.line.len() < Self::MAX {
            self.line.push(d);
        }
    }

    pub fn pop(&mut self) -> Option<Digit> {
        self.line.pop()
    }

    pub fn line(&self) -> usize {
        let mut line = 0;
        for digit in self.line.iter().copied() {
            line *= 10;
            line += digit as usize;
        }
        line
    }
}

impl std::fmt::Display for LinePrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.line
            .iter()
            .copied()
            .try_for_each(|d| (d as usize).fmt(f))
    }
}
