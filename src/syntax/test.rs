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
enum TestToken {
    #[regex("Black #+")]
    Black,
    #[regex("Red #+")]
    Red,
    #[regex("Green #+")]
    Green,
    #[regex("Yellow #+")]
    Yellow,
    #[regex("Blue #+")]
    Blue,
    #[regex("Magenta #+")]
    Magenta,
    #[regex("Cyan #+")]
    Cyan,
    #[regex("White #+")]
    White,
    #[regex("Bright Black #+")]
    BrightBlack,
    #[regex("Bright Red #+")]
    BrightRed,
    #[regex("Bright Green #+")]
    BrightGreen,
    #[regex("Bright Yellow #+")]
    BrightYellow,
    #[regex("Bright Blue #+")]
    BrightBlue,
    #[regex("Bright Magenta #+")]
    BrightMagenta,
    #[regex("Bright Cyan #+")]
    BrightCyan,
    #[regex("Bright White #+")]
    BrightWhite,
    #[regex("Bold #+")]
    Bold,
    #[regex("Italic #+")]
    Italic,
    #[regex("Underlined #+")]
    Underlined,
}

impl TryFrom<TestToken> for Highlight {
    type Error = ();

    fn try_from(t: TestToken) -> Result<Highlight, ()> {
        match t {
            TestToken::Black => Ok(Color::Black.into()),
            TestToken::Red => Ok(Color::Red.into()),
            TestToken::Green => Ok(Color::Green.into()),
            TestToken::Yellow => Ok(Color::Yellow.into()),
            TestToken::Blue => Ok(Color::Blue.into()),
            TestToken::Magenta => Ok(Color::Magenta.into()),
            TestToken::Cyan => Ok(Color::Cyan.into()),
            TestToken::White => Ok(Color::Gray.into()),
            TestToken::BrightBlack => Ok(Color::DarkGray.into()),
            TestToken::BrightRed => Ok(Color::LightRed.into()),
            TestToken::BrightGreen => Ok(Color::LightGreen.into()),
            TestToken::BrightYellow => Ok(Color::LightYellow.into()),
            TestToken::BrightBlue => Ok(Color::LightBlue.into()),
            TestToken::BrightMagenta => Ok(Color::LightMagenta.into()),
            TestToken::BrightCyan => Ok(Color::LightCyan.into()),
            TestToken::BrightWhite => Ok(Color::White.into()),
            TestToken::Bold => Ok(Highlight {
                color: None,
                modifier: Modifier::Bold,
            }),
            TestToken::Italic => Ok(Highlight {
                color: None,
                modifier: Modifier::Italic,
            }),
            TestToken::Underlined => Ok(Highlight {
                color: None,
                modifier: Modifier::Underlined,
            }),
        }
    }
}

#[derive(Debug)]
pub struct Test;

impl std::fmt::Display for Test {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Test".fmt(f)
    }
}

highlighter!(Test, TestToken);
