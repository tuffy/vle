// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::key;
use crate::key::CtrlBinding;
use ratatui::widgets::Block;

#[derive(Copy, Clone)]
pub struct Keybinding {
    action: &'static str,
    modifier: Option<&'static str>,
    keys: &'static [&'static str],
    f: &'static str,
}

pub const fn shift(keys: &'static [&'static str], action: &'static str) -> Keybinding {
    Keybinding {
        modifier: Some("Shift"),
        keys,
        action,
        f: "",
    }
}

pub const fn ctrl(keys: &'static [&'static str], action: &'static str) -> Keybinding {
    Keybinding {
        modifier: Some("Ctrl"),
        keys,
        action,
        f: "",
    }
}

pub const fn keybind<B: key::Binding>(action: &'static str) -> Keybinding {
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &[B::PRIMARY_LABEL],
        action,
        f: B::SECONDARY_LABEL,
    }
}

pub const fn ctrl_keybind<B: key::CtrlBinding>(action: &'static str) -> Keybinding {
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &[B::LABEL],
        action,
        f: "",
    }
}

pub const fn none(keys: &'static [&'static str], action: &'static str) -> Keybinding {
    Keybinding {
        modifier: None,
        keys,
        action,
        f: "",
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

    let [action_width, _, f_width, mod_width, _] = field_widths(keybindings);

    Paragraph::new(
        keybindings
            .iter()
            .map(|k| {
                let mut line = vec![];
                let Keybinding {
                    modifier,
                    keys,
                    action,
                    f,
                } = k;

                line.extend(spaces(action_width - action.width()));
                line.push(Span::from(*action));
                line.push(Span::from(" : "));
                if f_width > 0 {
                    if f.is_empty() {
                        line.extend(spaces(f_width));
                    } else {
                        line.push(key(f));
                        line.extend(spaces(f_width.saturating_sub(f.width())));
                    }
                }
                match modifier {
                    Some(modifier) => {
                        line.extend(spaces(mod_width.saturating_sub(modifier.width() + 1)));
                        line.push(key(modifier));
                        line.push(Span::from("-"));
                    }
                    None => {
                        line.extend(spaces(mod_width));
                    }
                }
                for k in keys.iter() {
                    line.push(key(k));
                    line.push(Span::from(" "));
                }

                Line::from(line)
            })
            .collect::<Vec<_>>(),
    )
}

pub fn field_widths(keybindings: &[Keybinding]) -> [usize; 5] {
    use unicode_width::UnicodeWidthStr;

    keybindings.iter().fold(
        [0, 2, 0, 0, 0],
        |[action_len, _, f_len, mod_len, keys_len]: [usize; 5], key| {
            let Keybinding {
                modifier,
                keys,
                action,
                f,
            } = key;
            [
                action_len.max(action.width()),
                2,
                f_len.max(if !f.is_empty() { f.width() + 1 } else { 0 }),
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

pub fn render_main_help(
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
    keybindings: &[Keybinding],
    mut block: impl FnMut(Block) -> Block,
) {
    use ratatui::{
        layout::{
            Constraint::{Length, Min},
            Layout,
        },
        widgets::{BorderType, Widget},
    };

    let (f_keys, non_f_keys): (Vec<_>, Vec<_>) = keybindings.iter().partition(|k| !k.f.is_empty());

    let [_, non_f_area, f_area] = Layout::horizontal([
        Min(0),
        Length((field_widths(&non_f_keys).into_iter().sum::<usize>() + 2) as u16),
        Length((field_widths(&f_keys).into_iter().sum::<usize>() + 2) as u16),
    ])
    .areas(area);

    let [_, non_f_area] =
        Layout::vertical([Min(0), Length(non_f_keys.len() as u16 + 2)]).areas(non_f_area);
    let [_, f_area] = Layout::vertical([Min(0), Length(f_keys.len() as u16 + 2)]).areas(f_area);

    for (bindings, area) in [(f_keys, f_area), (non_f_keys, non_f_area)] {
        let bindings_block = block(Block::bordered().border_type(BorderType::Rounded));
        let help_table = bindings_block.inner(area);
        ratatui::widgets::Clear.render(area, buf);
        bindings_block.render(area, buf);
        help_message(&bindings).render(help_table, buf);
    }
}

static UP: &str = "\u{2191}";
static DOWN: &str = "\u{2193}";
static LEFT: &str = "\u{2190}";
static RIGHT: &str = "\u{2192}";

pub static EDITING_0: &[Keybinding] = &[
    keybind::<key::Open>("Open File"),
    keybind::<key::Save>("Save File"),
];

pub static F10: Keybinding = keybind::<key::SplitPane>("Manage Panes");

pub static EDITING_2: &[Keybinding] = &[
    keybind::<key::Reload>("Reload File"),
    keybind::<key::Quit>("Quit File"),
    keybind::<key::Bookmark>("Toggle Bookmark"),
    shift(&[LEFT, DOWN, UP, RIGHT], "Highlight Text"),
    ctrl_keybind::<key::Mark>("Set Mark"),
];

pub static EDITING_3: &[Keybinding] = &[
    ctrl(
        &[key::Cut::LABEL, key::Copy::LABEL, key::Paste::LABEL],
        "Cut / Copy / Paste",
    ),
    ctrl(&[key::Undo::LABEL, key::Redo::LABEL], "Undo / Redo"),
];

pub static MARK_SET: &[Keybinding] = &[
    none(
        &[LEFT, DOWN, UP, RIGHT, "PgUp", "PgDn", "Home", "End"],
        "Highlight Text",
    ),
    ctrl_keybind::<key::Mark>("Finish"),
];

pub static SWITCH_PANE: Keybinding = ctrl(&[LEFT, DOWN, UP, RIGHT], "Switch Pane");

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

pub static SPLIT_PANE: &[Keybinding] = &[
    none(&[LEFT, RIGHT], "Split Vertically \u{25e7} / \u{25e8}"),
    none(&[UP, DOWN], "Split Horizontally \u{2b12} / \u{2b13}"),
    ctrl(&[LEFT, DOWN, UP, RIGHT], "Switch Pane in Direction"),
    shift(&[LEFT, DOWN, UP, RIGHT], "Swap Panes in Direction"),
    none(&["+", "-"], "Change Size Ratio"),
    none(&["Del"], "Delete Current Pane"),
    ctrl(&["Del"], "Delete All Other Panes"),
    none(&["Enter"], "Finish"),
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
    keybind::<key::Find>("Find Text"),
    none(&["Esc"], "Cancel"),
];

pub static SELECT_LINE_BOOKMARKED: &[Keybinding] = &[
    none(&["Enter"], "Select Line"),
    none(&["Home"], "Goto First Line"),
    none(&["End"], "Goto Last Line"),
    none(&[UP, DOWN], "Select Bookmark"),
    none(&["Del"], "Delete Bookmark"),
    keybind::<key::Find>("Find Text"),
    none(&["Esc"], "Cancel"),
];

pub static OPEN_FILE: &[Keybinding] = &[
    none(&[DOWN, UP], "Navigate Entries"),
    none(&[LEFT], "Up Directory"),
    none(&[RIGHT], "Down Directory"),
    none(&["Tab"], "Toggle File to Open"),
    ctrl(&["H"], "Toggle Show Hidden Files"),
    none(&["Enter"], "Select File(s)"),
    none(&["Esc"], "Cancel"),
];

pub static CREATE_FILE: &[Keybinding] = &[
    none(&["Enter"], "Create New File"),
    none(&["Esc"], "Cancel"),
];

pub static REPLACE_MATCHES: &[Keybinding] = &[
    none(&[UP, DOWN], "Select Match"),
    ctrl(&["Del"], "Remove Match"),
    keybind::<key::Find>("New Search"),
    keybind::<key::SelectInside>("Select Inside Pairs"),
    keybind::<key::WidenSelection>("Widen Selections"),
    keybind::<key::Bookmark>("Bookmark Positions"),
    none(&[LEFT, RIGHT], "Move Cursors"),
    shift(&[LEFT, RIGHT], "Highlight Text"),
    ctrl_keybind::<key::Mark>("Set Mark"),
    ctrl(
        &[key::Cut::LABEL, key::Copy::LABEL, key::Paste::LABEL],
        "Cut / Copy / Paste",
    ),
    none(&["Enter"], "Finish"),
];

pub static REPLACE_MATCHES_ALL: &[Keybinding] = &[
    none(&[UP, DOWN], "Select Match"),
    ctrl(&["Del"], "Remove Match"),
    keybind::<key::Find>("New Search"),
    keybind::<key::SelectInside>("Select Inside Pairs"),
    keybind::<key::WidenSelection>("Widen Selections"),
    keybind::<key::Bookmark>("Bookmark Positions"),
    none(&[LEFT, RIGHT], "Move Cursors"),
    shift(&[LEFT, RIGHT], "Highlight Text"),
    ctrl_keybind::<key::Mark>("Set Mark"),
    ctrl_keybind::<key::Paste>("Paste"),
    none(&["Enter"], "Finish"),
];

pub static MULTICURSOR_MARK_SET: &[Keybinding] = &[
    none(&[LEFT, RIGHT, "Home", "End"], "Highlight Text"),
    ctrl_keybind::<key::Mark>("Finish"),
];

pub static PASTE_GROUP: &[Keybinding] = &[
    ctrl(&[key::Paste::LABEL], "Paste From Cut Buffer"),
    none(&["0"], "Paste From Capture Group 0"),
    none(&["1"], "Paste From Capture Group 1"),
    none(&["2"], "Paste From Capture Group 2"),
    none(&["3"], "Paste From Capture Group 3"),
    none(&["4"], "Paste From Capture Group 4"),
    none(&["5"], "Paste From Capture Group 5"),
    none(&["6"], "Paste From Capture Group 6"),
    none(&["7"], "Paste From Capture Group 7"),
    none(&["8"], "Paste From Capture Group 8"),
    none(&["9"], "Paste From Capture Group 9"),
];

pub static SELECT_BUFFER: &[Keybinding] = &[
    none(&["0\u{2026}9", "A\u{2026}Z"], "Select Buffer by Letter"),
    none(&[UP, DOWN], "Choose Buffer"),
    ctrl(&[UP, DOWN], "Swap Buffer Locations"),
    none(&["Enter"], "Select Chosen Buffer"),
    keybind::<key::Save>("Save All Buffers"),
    keybind::<key::Reload>("Reload All Buffers"),
    keybind::<key::Quit>("Quit All Buffers"),
];
