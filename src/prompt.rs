// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Default)]
pub struct TextField {
    chars: Vec<char>,
    cursor: usize,
}

impl TextField {
    pub fn insert_char(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn paste(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(cursor) = self.cursor.checked_sub(1) {
            self.chars.remove(cursor);
            self.cursor = cursor;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub fn cursor_back(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn cursor_forward(&mut self) {
        if self.cursor < self.chars.len() {
            self.cursor += 1;
        }
    }

    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cursor = self.chars.len();
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn chars(&self) -> impl Iterator<Item = char> {
        self.chars.iter().copied()
    }

    pub fn cursor_column(&self) -> usize {
        use unicode_width::UnicodeWidthChar;

        self.chars()
            .take(self.cursor)
            .map(|c| match c {
                '\t' => *crate::buffer::SPACES_PER_TAB,
                c => c.width().unwrap_or(0),
            })
            .sum()
    }

    pub fn value(&self) -> Option<String> {
        (!self.is_empty()).then(|| self.chars().collect())
    }

    pub fn reset(&mut self) {
        self.chars.clear();
        self.cursor = 0;
    }

    pub fn process_event(&mut self, event: crossterm::event::Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.insert_char(c),
            Event::Paste(pasted) => self.paste(&pasted),
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.backspace(),
            Event::Key(KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.delete(),
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.cursor_back(),
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.cursor_forward(),
            Event::Key(KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.cursor_home(),
            Event::Key(KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.cursor_end(),
            _ => { /* ignore other events */ }
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Digit {
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Separator,
    Column,
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
            ',' | '_' | '.' => Ok(Digit::Separator),
            ':' => Ok(Digit::Column),
            c => Err(c),
        }
    }
}

pub struct Column;

impl TryFrom<Digit> for usize {
    type Error = Option<Column>;

    fn try_from(d: Digit) -> Result<Self, Self::Error> {
        match d {
            Digit::Digit0 => Ok(0),
            Digit::Digit1 => Ok(1),
            Digit::Digit2 => Ok(2),
            Digit::Digit3 => Ok(3),
            Digit::Digit4 => Ok(4),
            Digit::Digit5 => Ok(5),
            Digit::Digit6 => Ok(6),
            Digit::Digit7 => Ok(7),
            Digit::Digit8 => Ok(8),
            Digit::Digit9 => Ok(9),
            Digit::Separator => Err(None),
            Digit::Column => Err(Some(Column)),
        }
    }
}

impl std::fmt::Display for Digit {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Digit::Digit0 => 0.fmt(f),
            Digit::Digit1 => 1.fmt(f),
            Digit::Digit2 => 2.fmt(f),
            Digit::Digit3 => 3.fmt(f),
            Digit::Digit4 => 4.fmt(f),
            Digit::Digit5 => 5.fmt(f),
            Digit::Digit6 => 6.fmt(f),
            Digit::Digit7 => 7.fmt(f),
            Digit::Digit8 => 8.fmt(f),
            Digit::Digit9 => 9.fmt(f),
            Digit::Separator => '_'.fmt(f),
            Digit::Column => ':'.fmt(f),
        }
    }
}

#[derive(Default)]
pub struct LinePrompt {
    value: Vec<Digit>,
}

impl LinePrompt {
    pub const MAX: usize = 9;

    pub fn push(&mut self, digit: Digit) {
        if self.value.len() < Self::MAX {
            match digit {
                d @ Digit::Digit0 | d @ Digit::Separator => {
                    if !self.value.is_empty() {
                        self.value.push(d);
                    }
                }
                d @ Digit::Column => {
                    if !self.value.is_empty() && !self.value.contains(&d) {
                        self.value.push(d);
                    }
                }
                d => self.value.push(d),
            }
        }
    }

    pub fn pop(&mut self) -> Option<Digit> {
        self.value.pop()
    }

    pub fn line_and_column(&self) -> (usize, Option<usize>) {
        let mut digits = self.value.iter();
        match digits
            .by_ref()
            .try_fold(0, |acc, d| match usize::try_from(*d) {
                Ok(digit) => Ok(acc * 10 + digit),
                Err(None) => Ok(acc),          // skip digits separators
                Err(Some(Column)) => Err(acc), // hit columns separator
            }) {
            Ok(lines) => (lines, None),
            Err(lines) => (
                lines,
                Some(
                    digits
                        .filter_map(|d| usize::try_from(*d).ok())
                        .fold(0, |acc, digit| acc * 10 + digit),
                ),
            ),
        }
    }
}

impl std::fmt::Display for LinePrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.value.iter().copied().try_for_each(|d| d.fmt(f))
    }
}
