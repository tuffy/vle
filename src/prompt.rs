// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Default)]
pub struct SearchPrompt {
    value: Vec<char>,
}

impl SearchPrompt {
    pub const MAX_WIDTH: u16 = 30;

    pub fn push(&mut self, c: char) {
        self.value.push(c);
    }

    pub fn pop(&mut self) -> Option<char> {
        self.value.pop()
    }

    pub fn get_value(&self) -> Option<String> {
        (!self.value.is_empty()).then(|| self.value.iter().copied().collect())
    }

    pub fn width(&self) -> u16 {
        use unicode_width::UnicodeWidthStr;

        self.to_string().width().try_into().unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

impl std::fmt::Display for SearchPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.get_value().unwrap_or_default().fmt(f)
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

    pub fn len(&self) -> usize {
        self.line.len()
    }

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
