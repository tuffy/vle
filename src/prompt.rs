// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Default)]
pub struct Prompt {
    value: Vec<char>,
}

impl Prompt {
    pub fn push(&mut self, c: char) {
        self.value.push(c)
    }

    pub fn pop(&mut self) -> Option<char> {
        self.value.pop()
    }

    pub fn width(&self) -> u16 {
        use unicode_width::UnicodeWidthStr;

        self.to_string().width().try_into().unwrap()
    }
}

impl std::fmt::Display for Prompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.value.iter().try_for_each(|c| c.fmt(f))
    }
}

pub struct SearchPrompt {
    value: Vec<char>,
    history: SearchHistory,
    index: Option<usize>,
}

impl SearchPrompt {
    pub fn new(history: &SearchHistory) -> Self {
        Self {
            value: vec![],
            history: history.clone(),
            index: None,
        }
    }

    // Operates on our current value (if no index)
    // or moves the value from history into our current value
    // before operating on it
    fn update<T>(&mut self, f: impl FnOnce(&mut Vec<char>) -> T) -> T {
        match self.index {
            None => f(&mut self.value),
            Some(index) => match self.history.try_remove(index) {
                Some(v) => {
                    self.value = v;
                    f(&mut self.value)
                }
                None => {
                    // index being outside of history shouldn't happen
                    f(&mut self.value)
                }
            },
        }
    }

    pub fn push(&mut self, c: char) {
        self.update(|v| v.push(c));
    }

    pub fn pop(&mut self) -> Option<char> {
        self.update(|v| v.pop())
    }

    /// Retrieves value, if any, and updates history
    pub fn get_value(&mut self) -> Option<String> {
        let s = self.update(|v| v.iter().copied().collect::<String>());
        (!s.is_empty()).then(|| {
            self.history.push(std::mem::take(&mut self.value));
            self.index = None;
            s
        })
    }

    pub fn history(&self) -> &SearchHistory {
        &self.history
    }

    pub fn previous_entry(&mut self) {
        // if no index is set, goto top entry in history (if any)
        self.index = match self.index {
            None => self.history.len().checked_sub(1),
            Some(index) => index.checked_sub(1),
        }
    }

    pub fn next_entry(&mut self) {
        fn checked_value(index: usize, max: usize) -> Option<usize> {
            (index < max).then_some(index)
        }

        // index increments unless it exceeds maximum
        self.index = match self.index {
            None => checked_value(0, self.history.len()),
            Some(index) => checked_value(index + 1, self.history.len()),
        }
    }

    pub fn width(&self) -> u16 {
        use unicode_width::UnicodeWidthStr;

        self.to_string().width().try_into().unwrap()
    }
}

impl std::fmt::Display for SearchPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.index
            .and_then(|idx| self.history.get(idx))
            .unwrap_or_else(|| self.value.clone())
            .into_iter()
            .try_for_each(|c| c.fmt(f))
    }
}

#[derive(Clone, Default)]
pub struct SearchHistory(std::rc::Rc<std::cell::RefCell<private::SearchHistory>>);

impl SearchHistory {
    pub fn get(&self, index: usize) -> Option<Vec<char>> {
        self.0.borrow().get(index)
    }

    pub fn push(&mut self, value: Vec<char>) {
        self.0.borrow_mut().push(value)
    }

    pub fn try_remove(&mut self, index: usize) -> Option<Vec<char>> {
        self.0.borrow_mut().try_remove(index)
    }

    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }
}

mod private {
    #[derive(Clone, Default)]
    pub struct SearchHistory {
        history: Vec<Vec<char>>,
    }

    impl SearchHistory {
        pub fn get(&self, index: usize) -> Option<Vec<char>> {
            self.history.get(index).cloned()
        }

        pub fn push(&mut self, value: Vec<char>) {
            self.history.push(value);
        }

        pub fn try_remove(&mut self, index: usize) -> Option<Vec<char>> {
            (index < self.history.len()).then(|| self.history.remove(index))
        }

        pub fn len(&self) -> usize {
            self.history.len()
        }
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
