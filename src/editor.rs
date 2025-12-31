use crate::{
    buffer::{BufferContext, BufferId, BufferList, CutBuffer},
    prompt::Prompt,
};
use crossterm::event::Event;
use ratatui::{
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
use std::ffi::OsString;

const PAGE_SIZE: usize = 25;

#[derive(Default)]
pub enum EditorMode {
    #[default]
    Editing,
    ConfirmClose {
        buffer: BufferId,
    },
    SelectInside,
    SelectLine {
        prompt: Prompt,
    },
    Find {
        prompt: Prompt,
        cache: String,
    },
    Replace {
        replace: Prompt,
    },
    ReplaceWith {
        with: Prompt,
        matches: Vec<(usize, usize)>,
        match_idx: usize,
    },
    Open {
        prompt: Prompt,
    },
}

pub struct Editor {
    layout: Layout,
    mode: EditorMode,
    cut_buffer: Option<CutBuffer>, // cut buffer shared globally across editor
}

impl Editor {
    pub fn new(buffers: impl IntoIterator<Item = OsString>) -> std::io::Result<Self> {
        Ok(Self {
            layout: Layout::Single(BufferList::new(buffers)?),
            mode: EditorMode::default(),
            cut_buffer: None,
        })
    }

    pub fn has_open_buffers(&self) -> bool {
        self.layout.has_open_buffers()
    }

    pub fn display(&mut self, term: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        // TODO - display per-mode help if toggled on

        term.draw(|frame| {
            let area = frame.area();
            frame.render_stateful_widget(LayoutWidget { mode: &self.mode }, area, &mut self.layout);
            frame.set_cursor_position(self.layout.cursor_position(area).unwrap_or_default());
        })
        .map(|_| ())
    }

    fn update_buffer(&mut self, f: impl FnOnce(&mut crate::buffer::BufferContext)) {
        self.layout.selected_buffer_list_mut().update_buf(f)
    }

    fn perform_cut(&mut self) {
        if let Some(buffer) = self.layout.selected_buffer_list_mut().current_mut()
            && let Some(selection) = buffer.take_selection()
        {
            self.cut_buffer = Some(selection);
        }
    }

    fn perform_copy(&mut self) {
        if let Some(buffer) = self.layout.selected_buffer_list_mut().current_mut()
            && let Some(selection) = buffer.get_selection()
        {
            self.cut_buffer = Some(selection);
        }
    }

    fn perform_paste(&mut self) {
        if let Some(cut_buffer) = &self.cut_buffer
            && let Some(buffer) = self.layout.selected_buffer_list_mut().current_mut()
        {
            buffer.paste(cut_buffer);
        }
    }

    pub fn process_event(&mut self, event: Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.mode = EditorMode::default();
            }
            // TODO - F1 - toggle help display
            event => match &mut self.mode {
                EditorMode::Editing => self.process_normal_event(event),
                EditorMode::ConfirmClose { buffer } => {
                    let buffer = buffer.clone();
                    self.process_confirm_close(event, buffer)
                }
                EditorMode::SelectInside => self.process_select_inside(event),
                EditorMode::SelectLine { prompt } => {
                    if let Some(buf) = self.layout.selected_buffer_list_mut().current_mut()
                        && let Some(new_mode) = process_select_line(buf, prompt, event)
                    {
                        self.mode = new_mode;
                    }
                }
                EditorMode::Open { prompt } => {
                    if let Some(new_mode) = process_open_file(&mut self.layout, prompt, event) {
                        self.mode = new_mode;
                    }
                }
                EditorMode::Find { prompt, cache } => {
                    if let Some(buf) = self.layout.selected_buffer_list_mut().current_mut() {
                        process_find(buf, prompt, event, cache)
                    }
                }
                EditorMode::Replace { replace } => {
                    if let Some(buf) = self.layout.selected_buffer_list_mut().current_mut()
                        && let Some(new_mode) = process_replace(buf, replace, event)
                    {
                        self.mode = new_mode;
                    }
                }
                EditorMode::ReplaceWith {
                    with,
                    matches,
                    match_idx,
                } => {
                    if let Some(buf) = self.layout.selected_buffer_list_mut().current_mut()
                        && let Some(new_mode) =
                            process_replace_with(buf, with, matches, match_idx, event)
                    {
                        self.mode = new_mode;
                    }
                }
            },
        }
    }

    fn process_normal_event(&mut self, event: Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    if buf.modified() {
                        self.mode = EditorMode::ConfirmClose { buffer: buf.id() };
                    } else {
                        self.layout.remove(buf.id());
                    }
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.previous_buffer(),
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => self.layout.next_buffer(),
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => match &mut self.layout {
                Layout::Vertical { which, .. } => {
                    *which = VerticalPos::Left;
                }
                Layout::Single(buffer) => {
                    self.layout = Layout::Vertical {
                        left: buffer.clone(),
                        right: std::mem::take(buffer),
                        which: VerticalPos::Left,
                    };
                }
                Layout::Horizontal { top, bottom, .. } => {
                    self.layout = Layout::Vertical {
                        left: std::mem::take(top),
                        right: std::mem::take(bottom),
                        which: VerticalPos::Left,
                    };
                }
            },
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => match &mut self.layout {
                Layout::Vertical { which, .. } => {
                    *which = VerticalPos::Right;
                }
                Layout::Single(buffer) => {
                    self.layout = Layout::Vertical {
                        left: buffer.clone(),
                        right: std::mem::take(buffer),
                        which: VerticalPos::Right,
                    };
                }
                Layout::Horizontal { top, bottom, .. } => {
                    self.layout = Layout::Vertical {
                        left: std::mem::take(top),
                        right: std::mem::take(bottom),
                        which: VerticalPos::Right,
                    };
                }
            },
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => match &mut self.layout {
                Layout::Horizontal { which, .. } => {
                    *which = HorizontalPos::Top;
                }
                Layout::Single(buffer) => {
                    self.layout = Layout::Horizontal {
                        top: buffer.clone(),
                        bottom: std::mem::take(buffer),
                        which: HorizontalPos::Top,
                    }
                }
                Layout::Vertical { left, right, .. } => {
                    self.layout = Layout::Horizontal {
                        top: std::mem::take(left),
                        bottom: std::mem::take(right),
                        which: HorizontalPos::Top,
                    }
                }
            },
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => match &mut self.layout {
                Layout::Horizontal { which, .. } => {
                    *which = HorizontalPos::Bottom;
                }
                Layout::Single(buffer) => {
                    self.layout = Layout::Horizontal {
                        top: buffer.clone(),
                        bottom: std::mem::take(buffer),
                        which: HorizontalPos::Bottom,
                    }
                }
                Layout::Vertical { left, right, .. } => {
                    self.layout = Layout::Horizontal {
                        top: std::mem::take(left),
                        bottom: std::mem::take(right),
                        which: HorizontalPos::Bottom,
                    }
                }
            },
            Event::Key(KeyEvent {
                code: KeyCode::F(10),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                match &mut self.layout {
                    Layout::Single(_) => { /* do nothing */ }
                    Layout::Horizontal {
                        top,
                        which: HorizontalPos::Top,
                        ..
                    } => {
                        self.layout = Layout::Single(std::mem::take(top));
                    }
                    Layout::Horizontal {
                        bottom,
                        which: HorizontalPos::Bottom,
                        ..
                    } => {
                        self.layout = Layout::Single(std::mem::take(bottom));
                    }
                    Layout::Vertical {
                        left,
                        which: VerticalPos::Left,
                        ..
                    } => {
                        self.layout = Layout::Single(std::mem::take(left));
                    }
                    Layout::Vertical {
                        right,
                        which: VerticalPos::Right,
                        ..
                    } => {
                        self.layout = Layout::Single(std::mem::take(right));
                    }
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.cursor_up(1, modifiers.contains(KeyModifiers::SHIFT))),
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.cursor_down(1, modifiers.contains(KeyModifiers::SHIFT))),
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self
                .update_buffer(|b| b.cursor_up(PAGE_SIZE, modifiers.contains(KeyModifiers::SHIFT))),
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| {
                b.cursor_down(PAGE_SIZE, modifiers.contains(KeyModifiers::SHIFT))
            }),
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.cursor_back(modifiers.contains(KeyModifiers::SHIFT))),
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.cursor_forward(modifiers.contains(KeyModifiers::SHIFT))),
            Event::Key(KeyEvent {
                code: KeyCode::Home,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.cursor_home(modifiers.contains(KeyModifiers::SHIFT))),
            Event::Key(KeyEvent {
                code: KeyCode::End,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.cursor_end(modifiers.contains(KeyModifiers::SHIFT))),
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.insert_char(c)),
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.backspace()),
            Event::Key(KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.delete()),
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.newline()),
            Event::Key(KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => self.perform_cut(),
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => self.perform_copy(),
            Event::Key(KeyEvent {
                code: KeyCode::Char('v'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => self.perform_paste(),
            Event::Key(KeyEvent {
                code: KeyCode::Char('z'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.perform_undo()),
            Event::Key(KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.perform_redo()),
            Event::Key(KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.save()),
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.indent()),
            Event::Key(KeyEvent {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| b.un_indent()),
            Event::Key(KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.select_matching_paren());
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.mode = EditorMode::SelectInside;
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.mode = EditorMode::SelectLine {
                    prompt: Prompt::default(),
                };
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.mode = EditorMode::Find {
                    prompt: Prompt::default(),
                    cache: String::default(),
                };
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.mode = EditorMode::Replace {
                    replace: Prompt::default(),
                };
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('o'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.mode = EditorMode::Open {
                    prompt: Prompt::default(),
                };
            }
            _ => { /* ignore other events */ } // TODO - Ctrl-W - write buffer to disk with name
                                               // TODO - Ctrl-? - reload file from disk
        }
    }

    fn process_confirm_close(&mut self, event: Event, buffer_id: BufferId) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('y' | 'Y'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                // close buffer anyway
                self.layout.remove(buffer_id);
                self.mode = EditorMode::default();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('n' | 'N'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                // cancel close buffer
                self.mode = EditorMode::default();
            }
            _ => { /* ignore other events */ }
        }
    }

    fn process_select_inside(&mut self, event: Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('(' | ')'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.select_inside(('(', ')'), Some((')', '('))));
                self.mode = EditorMode::default();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('[' | ']'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.select_inside(('[', ']'), Some((']', '['))));
                self.mode = EditorMode::default();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('{' | '}'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.select_inside(('{', '}'), Some(('}', '{'))));
                self.mode = EditorMode::default();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('"'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.select_inside(('"', '"'), None));
                self.mode = EditorMode::default();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('\''),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.select_inside(('\'', '\''), None));
                self.mode = EditorMode::default();
            }
            _ => { /* do nothing */ }
        }
    }
}

fn process_select_line(
    buffer: &mut BufferContext,
    prompt: &mut Prompt,
    event: Event,
) -> Option<EditorMode> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c @ '0'..='9'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.push(c);
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.pop();
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            if let Ok(line) = prompt.to_string().parse::<usize>()
                && let Some(line) = line.checked_sub(1)
            {
                buffer.select_line(line);
                Some(EditorMode::default())
            } else {
                None
            }
        }
        Event::Key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            buffer.select_line(0);
            Some(EditorMode::default())
        }
        Event::Key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            buffer.select_line(buffer.last_line());
            Some(EditorMode::default())
        }
        _ => {
            None // ignore other events
        }
    }
}

// TODO - need set of buffers to assign opened file to, if necessary
fn process_open_file(layout: &mut Layout, prompt: &mut Prompt, event: Event) -> Option<EditorMode> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    // TODO - support tab completion
    // TODO - support interactive with a tree view
    // TODO - enter key attempts to open file
    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.push(c);
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.pop();
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => layout
            .add(prompt.to_string().into())
            .map(|()| EditorMode::default())
            .ok(),
        _ => None, // ignore other events
    }
}

fn process_find(buffer: &mut BufferContext, prompt: &mut Prompt, event: Event, cache: &mut String) {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.push(c);
        }
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.pop();
        }
        Event::Key(KeyEvent {
            code: code @ KeyCode::Up | code @ KeyCode::Down | code @ KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            buffer.search(
                matches!(code, KeyCode::Down | KeyCode::Enter),
                &prompt.chars().iter().collect::<String>(),
                cache,
            );
        }
        Event::Key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            buffer.select_line(0);
        }
        Event::Key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            buffer.select_line(buffer.last_line());
        }
        Event::Key(KeyEvent {
            code: KeyCode::Char('f'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.set(&[]);
        }
        _ => { /* ignore other events */ }
    }
}

// range is in characters, *not* bytes
fn process_replace(
    buffer: &mut BufferContext,
    prompt: &mut Prompt,
    event: Event,
) -> Option<EditorMode> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.push(c);
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.pop();
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            let matches = buffer.matches(&prompt.to_string());
            Some(match matches.first() {
                None => {
                    buffer.set_error("Not Found");
                    EditorMode::default()
                }
                Some((s, e)) => {
                    buffer.set_selection(*s, *e);
                    EditorMode::ReplaceWith {
                        with: Prompt::default(),
                        matches,
                        match_idx: 0,
                    }
                }
            })
        }
        _ => {
            None // ignore other events
        }
    }
}

// ranges in matches should be in increasing order
fn process_replace_with(
    buffer: &mut BufferContext,
    prompt: &mut Prompt,
    matches: &mut Vec<(usize, usize)>,
    match_idx: &mut usize,
    event: Event,
) -> Option<EditorMode> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.push(c);
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            prompt.pop();
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *match_idx = match_idx.checked_sub(1).unwrap_or(matches.len() - 1);
            if let Some((s, e)) = matches.get(*match_idx) {
                buffer.set_selection(*s, *e);
            }
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *match_idx = (*match_idx + 1) % matches.len();
            if let Some((s, e)) = matches.get(*match_idx) {
                buffer.set_selection(*s, *e);
            }
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Delete,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            if *match_idx < matches.len() {
                matches.remove(*match_idx);
            }
            match matches.len().checked_sub(1) {
                None => Some(EditorMode::default()),
                Some(new_max) => {
                    *match_idx = (*match_idx).min(new_max);
                    if let Some((s, e)) = matches.get(*match_idx) {
                        buffer.set_selection(*s, *e);
                    }
                    None
                }
            }
        }
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            buffer.replace(matches, &prompt.to_string());
            Some(EditorMode::default())
        }
        _ => {
            None // ignore other events
        }
    }
}

#[derive(Default)]
enum HorizontalPos {
    #[default]
    Top,
    Bottom,
}

#[derive(Default)]
enum VerticalPos {
    #[default]
    Left,
    Right,
}

enum Layout {
    Single(BufferList),
    Horizontal {
        top: BufferList,
        bottom: BufferList,
        which: HorizontalPos,
    },
    Vertical {
        left: BufferList,
        right: BufferList,
        which: VerticalPos,
    },
}

impl Layout {
    fn has_open_buffers(&self) -> bool {
        match self {
            Self::Single(b) | Self::Horizontal { top: b, .. } | Self::Vertical { left: b, .. } => {
                !b.is_empty()
            }
        }
    }

    fn add(&mut self, path: OsString) -> Result<(), ()> {
        match BufferContext::open(path) {
            Ok(buffer) => match self {
                Self::Single(b) => {
                    b.push(buffer, true);
                    Ok(())
                }
                Self::Horizontal { top, bottom, which } => {
                    top.push(buffer.clone(), matches!(which, HorizontalPos::Top));
                    bottom.push(buffer, matches!(which, HorizontalPos::Bottom));
                    Ok(())
                }
                Self::Vertical { left, right, which } => {
                    left.push(buffer.clone(), matches!(which, VerticalPos::Left));
                    right.push(buffer, matches!(which, VerticalPos::Right));
                    Ok(())
                }
            },
            Err(_) => todo!(),
        }
    }

    fn remove(&mut self, buffer: BufferId) {
        match self {
            Self::Single(buf) => buf.remove(&buffer),
            Self::Horizontal {
                top: x, bottom: y, ..
            }
            | Self::Vertical {
                left: x, right: y, ..
            } => {
                x.remove(&buffer);
                y.remove(&buffer);
            }
        }
    }

    fn selected_buffer_list(&self) -> &BufferList {
        match self {
            Self::Single(buffer)
            | Self::Horizontal {
                top: buffer,
                which: HorizontalPos::Top,
                ..
            }
            | Self::Horizontal {
                bottom: buffer,
                which: HorizontalPos::Bottom,
                ..
            }
            | Self::Vertical {
                left: buffer,
                which: VerticalPos::Left,
                ..
            }
            | Self::Vertical {
                right: buffer,
                which: VerticalPos::Right,
                ..
            } => buffer,
        }
    }

    fn selected_buffer_list_mut(&mut self) -> &mut BufferList {
        match self {
            Self::Single(buffer)
            | Self::Horizontal {
                top: buffer,
                which: HorizontalPos::Top,
                ..
            }
            | Self::Horizontal {
                bottom: buffer,
                which: HorizontalPos::Bottom,
                ..
            }
            | Self::Vertical {
                left: buffer,
                which: VerticalPos::Left,
                ..
            }
            | Self::Vertical {
                right: buffer,
                which: VerticalPos::Right,
                ..
            } => buffer,
        }
    }

    fn previous_buffer(&mut self) {
        self.selected_buffer_list_mut().previous_buffer()
    }

    fn next_buffer(&mut self) {
        self.selected_buffer_list_mut().next_buffer()
    }

    fn cursor_position(&self, area: Rect) -> Option<Position> {
        use ratatui::layout::{Constraint, Layout};

        fn apply_position(area: Rect, (row, col): (usize, usize)) -> Option<Position> {
            // TODO - filter out position if outside of area

            let x = (col + usize::from(area.x)).min((area.x + area.width).saturating_sub(1).into());
            let y =
                (row + usize::from(area.y)).min((area.y + area.height).saturating_sub(1).into());

            Some(Position {
                x: u16::try_from(x).ok()?,
                y: u16::try_from(y).ok()?,
            })
        }

        match self {
            Self::Single(buf) => buf
                .cursor_viewport_position()
                .and_then(|pos| apply_position(area, pos)),
            Self::Horizontal { top, bottom, which } => {
                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);

                match which {
                    HorizontalPos::Top => top
                        .cursor_viewport_position()
                        .and_then(|pos| apply_position(top_area, pos)),
                    HorizontalPos::Bottom => bottom
                        .cursor_viewport_position()
                        .and_then(|pos| apply_position(bottom_area, pos)),
                }
            }
            Self::Vertical { left, right, which } => {
                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);

                match which {
                    VerticalPos::Left => left
                        .cursor_viewport_position()
                        .and_then(|pos| apply_position(left_area, pos)),
                    VerticalPos::Right => right
                        .cursor_viewport_position()
                        .and_then(|pos| apply_position(right_area, pos)),
                }
            }
        }
    }
}

struct LayoutWidget<'e> {
    mode: &'e EditorMode,
}

impl StatefulWidget for LayoutWidget<'_> {
    type State = Layout;

    fn render(
        self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        layout: &mut Layout,
    ) {
        use crate::buffer::BufferWidget;

        let Self { mode } = self;

        match layout {
            Layout::Single(single) => {
                if let Some(buffer) = single.current_mut() {
                    BufferWidget { mode: Some(mode) }.render(area, buf, buffer);
                }
            }
            Layout::Horizontal { top, bottom, which } => {
                use ratatui::layout::{Constraint, Layout};

                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);
                if let Some(buffer) = top.current_mut() {
                    BufferWidget {
                        mode: match which {
                            HorizontalPos::Top => Some(mode),
                            HorizontalPos::Bottom => None,
                        },
                    }
                    .render(top_area, buf, buffer);
                }
                if let Some(buffer) = bottom.current_mut() {
                    BufferWidget {
                        mode: match which {
                            HorizontalPos::Top => None,
                            HorizontalPos::Bottom => Some(mode),
                        },
                    }
                    .render(bottom_area, buf, buffer);
                }
            }
            Layout::Vertical { left, right, which } => {
                use ratatui::layout::{Constraint, Layout};

                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);
                if let Some(buffer) = left.current_mut() {
                    BufferWidget {
                        mode: match which {
                            VerticalPos::Left => Some(mode),
                            VerticalPos::Right => None,
                        },
                    }
                    .render(left_area, buf, buffer);
                }
                if let Some(buffer) = right.current_mut() {
                    BufferWidget {
                        mode: match which {
                            VerticalPos::Left => None,
                            VerticalPos::Right => Some(mode),
                        },
                    }
                    .render(right_area, buf, buffer);
                }
            }
        }
    }
}
