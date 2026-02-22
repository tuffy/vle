// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crossterm::event::KeyCode;

pub trait Binding {
    const PRIMARY_KEY: KeyCode;
    const SECONDARY_KEY: KeyCode;
    const PRIMARY_LABEL: &'static str;
    const SECONDARY_LABEL: &'static str;
}

macro_rules! binding {
    ($name:ident, $primary:ident, $secondary:ident) => {
        pub struct $name;

        impl Binding for $name {
            const PRIMARY_KEY: KeyCode = Key::$primary.to_char();
            const SECONDARY_KEY: KeyCode = Key::$secondary.to_char();
            const PRIMARY_LABEL: &'static str = Key::$primary.to_str();
            const SECONDARY_LABEL: &'static str = Key::$secondary.to_str();
        }
    };
}

binding!(Open, O, F2);
binding!(Save, S, F3);
binding!(GotoLine, T, F4);
binding!(Find, F, F5);
binding!(Replace, R, F6);
binding!(GotoPair, P, F7);
binding!(SelectInside, E, F8);
binding!(WidenSelection, W, F9);
binding!(SplitPane, N, F10);
binding!(Reload, L, F11);
binding!(Quit, Q, F12);
binding!(Bookmark, B, Insert);
binding!(UpdateMatches, U, Space);

#[derive(Copy, Clone)]
#[allow(unused)]
enum Key {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Insert,
    Space,
}

impl Key {
    const fn to_char(self) -> KeyCode {
        match self {
            Self::A => KeyCode::Char('a'),
            Self::B => KeyCode::Char('b'),
            Self::C => KeyCode::Char('c'),
            Self::D => KeyCode::Char('d'),
            Self::E => KeyCode::Char('e'),
            Self::F => KeyCode::Char('f'),
            Self::G => KeyCode::Char('g'),
            Self::H => KeyCode::Char('h'),
            Self::I => KeyCode::Char('i'),
            Self::J => KeyCode::Char('j'),
            Self::K => KeyCode::Char('k'),
            Self::L => KeyCode::Char('l'),
            Self::M => KeyCode::Char('m'),
            Self::N => KeyCode::Char('n'),
            Self::O => KeyCode::Char('o'),
            Self::P => KeyCode::Char('p'),
            Self::Q => KeyCode::Char('q'),
            Self::R => KeyCode::Char('r'),
            Self::S => KeyCode::Char('s'),
            Self::T => KeyCode::Char('t'),
            Self::U => KeyCode::Char('u'),
            Self::V => KeyCode::Char('v'),
            Self::W => KeyCode::Char('w'),
            Self::X => KeyCode::Char('x'),
            Self::Y => KeyCode::Char('y'),
            Self::Z => KeyCode::Char('z'),
            Self::F1 => KeyCode::F(1),
            Self::F2 => KeyCode::F(2),
            Self::F3 => KeyCode::F(3),
            Self::F4 => KeyCode::F(4),
            Self::F5 => KeyCode::F(5),
            Self::F6 => KeyCode::F(6),
            Self::F7 => KeyCode::F(7),
            Self::F8 => KeyCode::F(8),
            Self::F9 => KeyCode::F(9),
            Self::F10 => KeyCode::F(10),
            Self::F11 => KeyCode::F(11),
            Self::F12 => KeyCode::F(12),
            Self::Insert => KeyCode::Insert,
            Self::Space => KeyCode::Char(' '),
        }
    }
    const fn to_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::Insert => "Ins",
            Self::Space => "Spc",
        }
    }
}
