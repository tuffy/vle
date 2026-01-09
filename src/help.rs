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
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["O"],
        action: "Open New File",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["R"],
        action: "Reload File",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["PgUp", "PgDn"],
        action: "Switch Buffer",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &[LEFT, DOWN, UP, RIGHT],
        action: "Open / Switch Pane",
    },
    Keybinding {
        modifier: None,
        keys: &["F2"],
        action: "Swap Panes",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["S"],
        action: "Save",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["Q"],
        action: "Quit Buffer",
    },
    Keybinding {
        modifier: Some("Shift"),
        keys: &[LEFT, DOWN, UP, RIGHT],
        action: "Highlight Text",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["W"],
        action: "Widen Selection to Lines",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["Home", "End"],
        action: "Start / End of Selection",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["E"],
        action: "Select Enveloped Items",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["X", "C", "V"],
        action: "Cut / Copy / Paste",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["Z", "Y"],
        action: "Undo / Redo",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["P"],
        action: "Goto Matching Pair",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["T"],
        action: "Goto Line",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["F"],
        action: "Find Text",
    },
];

pub static VERIFY_SAVE: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &["Y"],
        action: "Yes, Overwrite Contents",
    },
    Keybinding {
        modifier: None,
        keys: &["N"],
        action: "No, Do Not Save",
    },
];

pub static VERIFY_RELOAD: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &["Y"],
        action: "Yes, Overwrite Buffer From Disk",
    },
    Keybinding {
        modifier: None,
        keys: &["N"],
        action: "No, Do Not Overwrite",
    },
];

pub static CONFIRM_CLOSE: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &["Y"],
        action: "Yes, Close Without Saving",
    },
    Keybinding {
        modifier: None,
        keys: &["N"],
        action: "No, Do Not Close",
    },
];

pub static SURROUND_WITH: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &["(", ")"],
        action: "Surround With ( \u{2026} )",
    },
    Keybinding {
        modifier: None,
        keys: &["[", "]"],
        action: "Surround With [ \u{2026} ]",
    },
    Keybinding {
        modifier: None,
        keys: &["{", "}"],
        action: "Surround With { \u{2026} }",
    },
    Keybinding {
        modifier: None,
        keys: &["<", ">"],
        action: "Surround With < \u{2026} >",
    },
    Keybinding {
        modifier: None,
        keys: &["\""],
        action: "Surround With \" \u{2026} \"",
    },
    Keybinding {
        modifier: None,
        keys: &["'"],
        action: "Surround With ' \u{2026} '",
    },
    Keybinding {
        modifier: None,
        keys: &["Del"],
        action: "Delete Surround",
    }
];

pub static SELECT_INSIDE: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &["(", ")"],
        action: "Select Inside ( \u{2026} )",
    },
    Keybinding {
        modifier: None,
        keys: &["[", "]"],
        action: "Select Inside [ \u{2026} ]",
    },
    Keybinding {
        modifier: None,
        keys: &["{", "}"],
        action: "Select Inside { \u{2026} }",
    },
    Keybinding {
        modifier: None,
        keys: &["<", ">"],
        action: "Select Inside < \u{2026} >",
    },
    Keybinding {
        modifier: None,
        keys: &["\""],
        action: "Select Inside \" \u{2026} \"",
    },
    Keybinding {
        modifier: None,
        keys: &["'"],
        action: "Select Inside ' \u{2026} '",
    },
];

pub static SELECT_LINE: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &["Enter"],
        action: "Select Line",
    },
    Keybinding {
        modifier: None,
        keys: &["Home"],
        action: "Goto First Line",
    },
    Keybinding {
        modifier: None,
        keys: &["End"],
        action: "Goto Last Line",
    },
];

pub static OPEN_FILE: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &[DOWN, UP],
        action: "Navigate Entries",
    },
    Keybinding {
        modifier: None,
        keys: &[LEFT],
        action: "Up Directory",
    },
    Keybinding {
        modifier: None,
        keys: &[RIGHT],
        action: "Down Directory",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["T"],
        action: "Toggle File to Open",
    },
    Keybinding {
        modifier: None,
        keys: &["Enter"],
        action: "Select File(s)",
    },
];

pub static FIND: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &[UP],
        action: "Earlier Item in History",
    },
    Keybinding {
        modifier: None,
        keys: &[DOWN],
        action: "Later Item in History",
    },
    Keybinding {
        modifier: None,
        keys: &["Enter"],
        action: "Select All Matches",
    },
];

pub static SELECT_MATCHES: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &[UP, LEFT],
        action: "Select Previous Match",
    },
    Keybinding {
        modifier: None,
        keys: &[DOWN, RIGHT],
        action: "Select Next Match",
    },
    Keybinding {
        modifier: None,
        keys: &["Del"],
        action: "Remove Selected Match",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["R"],
        action: "Replace Selected Matches",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["F"],
        action: "Perform New Find",
    },
    Keybinding {
        modifier: None,
        keys: &["Enter"],
        action: "Finish Find",
    },
];

pub static REPLACE_MATCHES: &[Keybinding] = &[
    Keybinding {
        modifier: None,
        keys: &[UP, LEFT],
        action: "Select Previous Match",
    },
    Keybinding {
        modifier: None,
        keys: &[DOWN, RIGHT],
        action: "Select Next Match",
    },
    Keybinding {
        modifier: None,
        keys: &["Enter"],
        action: "Finish Replacement",
    },
];
