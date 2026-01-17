// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ratatui::widgets::Block;

pub struct Keybinding {
    modifier: Option<&'static str>,
    keys: &'static [&'static str],
    action: &'static str,
}

const fn ctrl(keys: &'static [&'static str], action: &'static str) -> Keybinding {
    Keybinding {
        modifier: Some("Ctrl"),
        keys,
        action,
    }
}

const fn none(keys: &'static [&'static str], action: &'static str) -> Keybinding {
    Keybinding {
        modifier: None,
        keys,
        action,
    }
}

pub fn help_message(keybindings: &[Keybinding]) -> ratatui::widgets::Paragraph<'_> {
    use ratatui::{
        style::{Modifier, Style},
        text::{Line, Span},
        widgets::Paragraph,
    };
    use unicode_width::UnicodeWidthStr;

    fn key(label: &str) -> Span<'_> {
        Span::styled(label, Style::new().add_modifier(Modifier::REVERSED))
    }

    fn spaces(s: usize) -> Option<Span<'static>> {
        (s > 0).then(|| Span::raw(std::iter::repeat_n(' ', s).collect::<String>()))
    }

    let [action_width, _, mod_width, _] = field_widths(keybindings);

    Paragraph::new(
        keybindings
            .iter()
            .map(|k| {
                let mut line = vec![];
                line.extend(spaces(action_width - k.action.width()));
                line.push(Span::from(k.action));
                line.push(Span::from(" : "));
                match k.modifier {
                    Some(modifier) => {
                        line.extend(spaces(mod_width - (modifier.width() + 1)));
                        line.push(key(modifier));
                        line.push(Span::from("-"));
                    }
                    None => {
                        line.extend(spaces(mod_width));
                    }
                }
                for k in k.keys {
                    line.push(key(k));
                    line.push(Span::from(" "));
                }

                Line::from(line)
            })
            .collect::<Vec<_>>(),
    )
}

pub fn field_widths(keybindings: &[Keybinding]) -> [usize; 4] {
    use unicode_width::UnicodeWidthStr;

    keybindings.iter().fold(
        [0, 2, 0, 0],
        |[action_len, _, mod_len, keys_len]: [usize; 4],
         Keybinding {
             modifier,
             keys,
             action,
         }: &Keybinding| {
            [
                action_len.max(action.width()),
                2,
                mod_len.max(match modifier {
                    Some(m) => m.width() + 1,
                    None => 0,
                }),
                keys_len.max(keys.iter().map(|k| k.width() + 1).sum()),
            ]
        },
    )
}

pub fn render_help(
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
    keybindings: &[Keybinding],
    block: impl FnOnce(Block) -> Block,
) {
    use ratatui::{
        layout::{
            Constraint::{Length, Min},
            Layout,
        },
        widgets::{BorderType, Widget},
    };

    let [_, help] = Layout::horizontal([
        Min(0),
        Length((field_widths(keybindings).into_iter().sum::<usize>() + 2) as u16),
    ])
    .areas(area);

    let [_, help] = Layout::vertical([Min(0), Length(keybindings.len() as u16 + 2)]).areas(help);

    let block = block(Block::bordered().border_type(BorderType::Rounded));

    let help_table = block.inner(help);
    ratatui::widgets::Clear.render(help, buf);
    block.render(help, buf);
    help_message(keybindings).render(help_table, buf);
}

static UP: &str = "\u{2191}";
static DOWN: &str = "\u{2193}";
static LEFT: &str = "\u{2190}";
static RIGHT: &str = "\u{2192}";

pub static EDITING: &[Keybinding] = &[
    ctrl(&["O"], "Open File"),
    ctrl(&["L"], "Reload File"),
    ctrl(&["S"], "Save File"),
    ctrl(&["PgUp", "PgDn"], "Switch Buffer"),
    ctrl(&["Q"], "Quit Buffer"),
    ctrl(&[LEFT, DOWN, UP, RIGHT], "Open / Switch Pane"),
    ctrl(&["N"], "Swap Panes"),
    Keybinding {
        modifier: Some("Shift"),
        keys: &[LEFT, DOWN, UP, RIGHT],
        action: "Highlight Text",
    },
    ctrl(&["W"], "Widen Selection to Lines"),
    ctrl(&["Home", "End"], "Start / End of Selection"),
    ctrl(&["E"], "Handle Enveloped Items"),
    ctrl(&["X", "C", "V"], "Cut / Copy / Paste"),
    ctrl(&["Z", "Y"], "Undo / Redo"),
    ctrl(&["P"], "Goto Matching Pair"),
    ctrl(&["T"], "Goto Line"),
    ctrl(&["F"], "Find Text"),
];

pub static VERIFY_SAVE: &[Keybinding] = &[
    none(&["Y"], "Yes, Overwrite Contents"),
    none(&["N"], "No, Do Not Save"),
];

pub static VERIFY_RELOAD: &[Keybinding] = &[
    none(&["Y"], "Yes, Overwrite Buffer From Disk"),
    none(&["N"], "No, Do Not Overwrite"),
];

pub static CONFIRM_CLOSE: &[Keybinding] = &[
    none(&["Y"], "Yes, Close Without Saving"),
    none(&["N"], "No, Do Not Close"),
];

pub static SURROUND_WITH: &[Keybinding] = &[
    none(&["(", ")"], "Surround With ( \u{2026} )"),
    none(&["[", "]"], "Surround With [ \u{2026} ]"),
    none(&["{", "}"], "Surround With { \u{2026} }"),
    none(&["<", ">"], "Surround With < \u{2026} >"),
    none(&["\""], "Surround With \" \u{2026} \""),
    none(&["'"], "Surround With ' \u{2026} '"),
    none(&["Del"], "Delete Surround"),
    none(&["Esc"], "Cancel"),
];

pub static SELECT_INSIDE: &[Keybinding] = &[
    none(&["(", ")"], "Select Inside ( \u{2026} )"),
    none(&["[", "]"], "Select Inside [ \u{2026} ]"),
    none(&["{", "}"], "Select Inside { \u{2026} }"),
    none(&["<", ">"], "Select Inside < \u{2026} >"),
    none(&["\""], "Select Inside \" \u{2026} \""),
    none(&["'"], "Select Inside ' \u{2026} '"),
    none(&["Esc"], "Cancel"),
];

pub static SELECT_LINE: &[Keybinding] = &[
    none(&["Enter"], "Select Line"),
    none(&["Home"], "Goto First Line"),
    none(&["End"], "Goto Last Line"),
    none(&["Esc"], "Cancel"),
];

pub static OPEN_FILE: &[Keybinding] = &[
    none(&[DOWN, UP], "Navigate Entries"),
    none(&[LEFT], "Up Directory"),
    none(&[RIGHT], "Down Directory"),
    none(&["Tab"], "Toggle File to Open"),
    none(&["Enter"], "Select File(s)"),
    none(&["Esc"], "Cancel"),
];

pub static FIND: &[Keybinding] = &[
    none(&[UP, LEFT], "Select Previous Match"),
    none(&[DOWN, RIGHT], "Select Next Match"),
    ctrl(&["V"], "Copy from Cut Buffer"),
    none(&["Del"], "Remove Selected Match"),
    ctrl(&["R"], "Replace Selected Matches"),
    ctrl(&["F"], "Begin New Find"),
    none(&["Enter"], "Finish"),
];

pub static REPLACE_MATCHES: &[Keybinding] = &[
    none(&[UP, LEFT], "Select Previous Match"),
    none(&[DOWN, RIGHT], "Select Next Match"),
    ctrl(&["V"], "Copy from Cut Buffer"),
    none(&["Enter", "Esc"], "Finish Replacement"),
];
