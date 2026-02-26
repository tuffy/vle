// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use crate::syntax::{Highlight, Modifier};
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum TutorialToken {
    #[token("__     __                _     _ _   _   _        _____    _ _ _")]
    #[token("\\ \\   / /__ _ __ _   _  | |   (_) |_| |_| | ___  | ____|__| (_) |_ ___  _ __")]
    #[token("\\ \\ / / _ \\ '__| | | | | |   | | __| __| |/ _ \\ |  _| / _` | | __/ _ \\| '__|")]
    #[token("\\ V /  __/ |  | |_| | | |___| | |_| |_| |  __/ | |__| (_) | | || (_) | |")]
    #[token("\\_/ \\___|_|   \\__, | |_____|_|\\__|\\__|_|\\___| |_____\\__,_|_|\\__\\___/|_|")]
    #[token("|___/")]
    #[token("- An Exercise in Minimalist Text Editing -")]
    Title,
    #[regex("F[0-9]{1,2}")]
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
    #[token("Insert")]
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
    #[token("Ctrl-B")]
    #[token("Ctrl-PgUp")]
    #[token("Ctrl-PgDn")]
    #[token("Ctrl-Home")]
    #[token("Ctrl-End")]
    #[token("Esc")]
    #[token("Tab")]
    #[token("Shift-Tab")]
    #[token("Ctrl-Tab")]
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
    #[token(">>> {new text}")]
    #[token(">>> {fixed text}")]
    #[token(">>> 1111,2222")]
    #[token(">>> 333333,44")]
    #[token(">>> 555,66666")]
    #[token(">>> 7777777,8")]
    #[token(">>> pneumonoultramicroscopicsilicovolcanoconiosis")]
    #[token(">>> # Lorem ipsum dolor sit amet, consectetur adipiscing elit.")]
    #[token(">>> # Nullam viverra est nec sem feugiat blandit.")]
    #[token(">>> # Vestibulum ante ipsum primis in faucibus orci luctus et")]
    #[token(">>> # ultrices posuere cubilia curae;")]
    #[token(">>> # Phasellus consequat massa lorem, vel cursus enim tristique vel.")]
    #[token(">>> # Nunc dictum imperdiet porttitor.")]
    Correct,
    #[regex(">>> .+", allow_greedy = true)]
    #[token("println!(\"a is {a}\");")]
    #[token("println!(\"b is {b}\");")]
    #[token("println!(\"c is {c}\");")]
    Incorrect,
    #[regex("    # [A-Za-z ,.;]+")]
    Comment,
    #[regex("[A-Za-z][a-z]*")]
    Word,
}

impl TryFrom<TutorialToken> for Highlight {
    type Error = ();

    fn try_from(t: TutorialToken) -> Result<Highlight, ()> {
        match t {
            TutorialToken::Title => Ok(Color::Cyan.into()),
            TutorialToken::Keybinding => Ok(Highlight {
                color: Some(Color::Magenta),
                modifier: Modifier::Bold,
            }),
            TutorialToken::Header | TutorialToken::Subheader => Ok(Highlight {
                color: Some(Color::Blue),
                modifier: Modifier::Underlined,
            }),
            TutorialToken::Correct => Ok(Color::Green.into()),
            TutorialToken::Incorrect => Ok(Color::Red.into()),
            TutorialToken::Variable => Ok(Color::Cyan.into()),
            TutorialToken::Comment => Ok(Highlight {
                color: Some(Color::DarkGray),
                modifier: Modifier::Italic,
            }),
            TutorialToken::Word => Err(()),
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
