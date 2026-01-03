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

    let [mod_width, keys_width, _, _] = field_widths(keybindings);

    Paragraph::new(
        keybindings
            .iter()
            .map(|k| {
                let mut line = vec![];
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
                let mut key_width = 0;
                for k in k.keys {
                    line.push(key(k));
                    line.push(Span::from(" "));
                    key_width += k.width() + 1;
                }
                line.extend(spaces(keys_width - key_width));
                line.push(Span::from(": "));
                line.push(Span::from(k.action));
                Line::from(line)
            })
            .collect::<Vec<_>>(),
    )
}

pub fn field_widths(keybindings: &[Keybinding]) -> [usize; 4] {
    use unicode_width::UnicodeWidthStr;

    keybindings.iter().fold(
        [0, 0, 2, 0],
        |[mod_len, keys_len, _, action_len]: [usize; 4],
         Keybinding {
             modifier,
             keys,
             action,
         }: &Keybinding| {
            [
                mod_len.max(match modifier {
                    Some(m) => m.width() + 1,
                    None => 0,
                }),
                keys_len.max(keys.iter().map(|k| k.width() + 1).sum()),
                2,
                action_len.max(action.width()),
            ]
        },
    )
}

static UP: &str = "\u{2191}";
static DOWN: &str = "\u{2193}";
static LEFT: &str = "\u{2190}";
static RIGHT: &str = "\u{2192}";

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
        action: "Select First Line in File",
    },
    Keybinding {
        modifier: None,
        keys: &["End"],
        action: "Select Last Line in File",
    },
];

pub static OPEN_FILE: &[Keybinding] = &[Keybinding {
    modifier: None,
    keys: &["Enter"],
    action: "Select File",
}];

pub static FIND: &[Keybinding] = &[Keybinding {
    modifier: None,
    keys: &["Enter"],
    action: "Select All Matches",
}];

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
        keys: &["Backspace"],
        action: "Remove Match",
    },
    Keybinding {
        modifier: Some("Ctrl"),
        keys: &["R"],
        action: "Replace Matches",
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
