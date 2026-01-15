// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum TutorialToken {
    #[token("__     __                _     _ _   _   _        _____    _ _ _")]
    #[token("\\ \\   / /__ _ __ _   _  | |   (_) |_| |_| | ___  | ____|__| (_) |_ ___  _ __")]
    #[token("\\ \\ / / _ \\ '__| | | | | |   | | __| __| |/ _ \\ |  _| / _` | | __/ _ \\| '__|")]
    #[token("\\ V /  __/ |  | |_| | | |___| | |_| |_| |  __/ | |__| (_| | | || (_) | |")]
    #[token("\\_/ \\___|_|   \\__, | |_____|_|\\__|\\__|_|\\___| |_____\\__,_|_|\\__\\___/|_|")]
    #[token("|___/")]
    Title,
    #[token("F1")]
    #[token("F2")]
    #[token("\u{2190}")]
    #[token("\u{2191}")]
    #[token("\u{2192}")]
    #[token("\u{2193}")]
    #[token("Ctrl-\u{2190}")]
    #[token("Ctrl-\u{2191}")]
    #[token("Ctrl-\u{2192}")]
    #[token("Ctrl-\u{2193}")]
    #[token("PgUp")]
    #[token("PgDn")]
    #[token("Home")]
    #[token("End")]
    #[token("Del")]
    #[token("Backspace")]
    #[token("Shift")]
    #[token("Enter")]
    #[token("Ctrl-S")]
    #[token("Ctrl-X")]
    #[token("Ctrl-C")]
    #[token("Ctrl-V")]
    #[token("Ctrl-Z")]
    #[token("Ctrl-Y")]
    #[token("Ctrl-E")]
    #[token("Ctrl-T")]
    #[token("Ctrl-P")]
    #[token("Ctrl-F")]
    #[token("Ctrl-R")]
    #[token("Ctrl-O")]
    #[token("Ctrl-Q")]
    #[token("Ctrl-N")]
    #[token("Ctrl-PgUp")]
    #[token("Ctrl-PgDn")]
    #[token("Ctrl-Home")]
    #[token("Ctrl-End")]
    #[token("Esc")]
    #[token("Tab")]
    #[token("Shift-Tab")]
    Keybinding,
    #[token("VLE_SPACES_PER_TAB")]
    #[token("VLE_ALWAYS_TAB")]
    Variable,
    #[regex("# [[:upper:]].+", allow_greedy = true)]
    Header,
    #[regex("## [[:upper:]].+", allow_greedy = true)]
    Subheader,
    #[token(">>> sphinx of black quartz judge my vow")]
    #[token(">>> A sentence that's just about perfect")]
    #[token(">>> This sentence is just right.")]
    #[token(">>> Duplicate sentence. Duplicate sentence.")]
    #[token(">>> Sentence 1. Sentence 2.")]
    #[token(">>> \"the correct string\"")]
    #[token(">>> (surround me)")]
    #[token(">>> un-surround this text")]
    #[token(">>> fixed")]
    #[token("    println!(\"a is {a}\");")]
    #[token("    println!(\"b is {b}\");")]
    #[token("    println!(\"c is {c}\");")]
    Correct,
    #[regex(">>> .+", allow_greedy = true)]
    #[token("println!(\"a is {a}\");")]
    #[token("println!(\"b is {b}\");")]
    #[token("println!(\"c is {c}\");")]
    Incorrect,
}

impl TryFrom<TutorialToken> for Color {
    type Error = ();

    fn try_from(t: TutorialToken) -> Result<Color, ()> {
        match t {
            TutorialToken::Title => Ok(Color::Cyan),
            TutorialToken::Keybinding => Ok(Color::Magenta),
            TutorialToken::Header => Ok(Color::Blue),
            TutorialToken::Subheader => Ok(Color::Blue),
            TutorialToken::Correct => Ok(Color::Green),
            TutorialToken::Incorrect => Ok(Color::Red),
            TutorialToken::Variable => Ok(Color::Cyan),
        }
    }
}

#[derive(Debug)]
pub struct Tutorial;

impl std::fmt::Display for Tutorial {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Tutorial".fmt(f)
    }
}

highlighter!(Tutorial, TutorialToken);
