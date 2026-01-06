// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{
    buffer::{BufferContext, BufferId, BufferList, CutBuffer},
    prompt::{LinePrompt, Prompt, SearchHistory, SearchPrompt},
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
    VerifySave,
    VerifyReload,
    ConfirmClose {
        buffer: BufferId,
    },
    SelectInside,
    SelectLine {
        prompt: LinePrompt,
    },
    Find {
        prompt: SearchPrompt,
    },
    SelectMatches {
        matches: Vec<(usize, usize)>,
        match_idx: Option<usize>,
        search_history: SearchHistory,
    },
    ReplaceMatches {
        matches: Vec<(usize, usize)>,
        match_idx: Option<usize>,
    },
    Open {
        prompt: Prompt,
    },
}

macro_rules! key {
    ($code:ident) => {
        Event::Key(KeyEvent {
            code: KeyCode::$code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        })
    };
    ($modifier:ident, $code:ident) => {
        Event::Key(KeyEvent {
            code: KeyCode::$code,
            modifiers: KeyModifiers::$modifier,
            kind: KeyEventKind::Press,
            ..
        })
    };
    ($code:literal) => {
        Event::Key(KeyEvent {
            code: KeyCode::Char($code),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        })
    };
    ($modifier:ident, $code:literal) => {
        Event::Key(KeyEvent {
            code: KeyCode::Char($code),
            modifiers: KeyModifiers::$modifier,
            kind: KeyEventKind::Press,
            ..
        })
    };
}

pub struct Editor {
    layout: Layout,
    mode: EditorMode,
    cut_buffer: Option<CutBuffer>, // cut buffer shared globally across editor
    show_help: bool,
    search_history: SearchHistory,
}

impl Editor {
    pub fn new(buffers: impl IntoIterator<Item = OsString>) -> std::io::Result<Self> {
        Ok(Self {
            layout: Layout::Single(BufferList::new(buffers)?),
            mode: EditorMode::default(),
            cut_buffer: None,
            show_help: false,
            search_history: SearchHistory::default(),
        })
    }

    pub fn has_open_buffers(&self) -> bool {
        self.layout.has_open_buffers()
    }

    pub fn display(&mut self, term: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        term.draw(|frame| {
            let area = frame.area();
            frame.render_stateful_widget(
                LayoutWidget {
                    mode: &mut self.mode,
                    show_help: self.show_help,
                },
                area,
                &mut self.layout,
            );
            frame.set_cursor_position(
                self.layout
                    .cursor_position(area, &self.mode)
                    .unwrap_or_default(),
            );
        })
        .map(|_| ())
    }

    fn update_buffer(&mut self, f: impl FnOnce(&mut crate::buffer::BufferContext)) {
        self.layout.selected_buffer_list_mut().update_buf(f)
    }

    fn on_buffer<T>(
        &mut self,
        f: impl FnOnce(&mut crate::buffer::BufferContext) -> T,
    ) -> Option<T> {
        self.layout.selected_buffer_list_mut().on_buf(f)
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
            key!(Esc) => {
                self.mode = EditorMode::default();
            }
            Event::Key(KeyEvent {
                code: KeyCode::F(1),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.show_help = !self.show_help;
            }
            event => match &mut self.mode {
                EditorMode::Editing => self.process_normal_event(event),
                EditorMode::ConfirmClose { buffer } => {
                    let buffer = buffer.clone();
                    self.process_confirm_close(event, buffer)
                }
                EditorMode::VerifySave => self.process_verify_save(event),
                EditorMode::VerifyReload => self.process_verify_reload(event),
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
                EditorMode::Find { prompt } => {
                    if let Some(buf) = self.layout.selected_buffer_list_mut().current_mut()
                        && let Some(new_mode) = process_find(buf, prompt, event)
                    {
                        self.mode = new_mode;
                    }
                }
                EditorMode::SelectMatches {
                    matches,
                    match_idx,
                    search_history,
                } => {
                    if let Some(buf) = self.layout.selected_buffer_list_mut().current_mut()
                        && let Some(new_mode) =
                            process_select_matches(buf, matches, match_idx, search_history, event)
                    {
                        self.mode = new_mode;
                    }
                }
                EditorMode::ReplaceMatches { matches, match_idx } => {
                    if let Some(buf) = self.layout.selected_buffer_list_mut().current_mut()
                        && let Some(new_mode) =
                            process_replace_matches(buf, matches, match_idx, event)
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
            key!(CONTROL, 'q') => {
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    if buf.modified() {
                        self.mode = EditorMode::ConfirmClose { buffer: buf.id() };
                    } else {
                        self.layout.remove(buf.id());
                    }
                }
            }
            key!(CONTROL, PageUp) => self.layout.previous_buffer(),
            key!(CONTROL, PageDown) => self.layout.next_buffer(),
            key!(CONTROL, Left) => match &mut self.layout {
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
            },
            key!(CONTROL, Right) => match &mut self.layout {
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
            },
            key!(CONTROL, Up) => match &mut self.layout {
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
            },
            key!(CONTROL, Down) => match &mut self.layout {
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
            },
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
            key!(CONTROL, Home) => self.update_buffer(|b| b.cursor_to_selection_start()),
            key!(CONTROL, End) => self.update_buffer(|b| {
                b.cursor_to_selection_end();
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
            key!(Backspace) => self.update_buffer(|b| b.backspace()),
            key!(Delete) => self.update_buffer(|b| b.delete()),
            key!(Enter) => self.update_buffer(|b| b.newline()),
            key!(CONTROL, 'w') => self.update_buffer(|b| b.select_whole_lines()),
            key!(CONTROL, 'x') => self.perform_cut(),
            key!(CONTROL, 'c') => self.perform_copy(),
            key!(CONTROL, 'v') => self.perform_paste(),
            key!(CONTROL, 'z') => self.update_buffer(|b| b.perform_undo()),
            key!(CONTROL, 'y') => self.update_buffer(|b| b.perform_redo()),
            key!(CONTROL, 's') => {
                if let Some(Err(crate::buffer::Modified)) = self.on_buffer(|b| b.verified_save()) {
                    self.mode = EditorMode::VerifySave;
                }
            }
            // key!(ALT, 's') => TODO - prompt for new name for buffer and save
            key!(Tab) => self.update_buffer(|b| b.indent()),
            key!(SHIFT, BackTab) => self.update_buffer(|b| b.un_indent()),
            key!(CONTROL, 'p') => self.update_buffer(|b| b.select_matching_paren()),
            key!(CONTROL, 'e') => {
                self.mode = EditorMode::SelectInside;
            }
            key!(CONTROL, 't') => {
                self.mode = EditorMode::SelectLine {
                    prompt: LinePrompt::default(),
                };
            }
            key!(CONTROL, 'f') => {
                self.mode = EditorMode::Find {
                    prompt: SearchPrompt::new(&self.search_history),
                };
            }
            key!(CONTROL, 'o') => {
                self.mode = EditorMode::Open {
                    prompt: Prompt::default(),
                };
            }
            key!(ALT, 'o') => {
                if let Some(new_mode) = self.on_buffer(|b| {
                    if b.modified() {
                        EditorMode::VerifyReload
                    } else {
                        b.reload();
                        EditorMode::default()
                    }
                }) {
                    self.mode = new_mode;
                }
            }
            // self.update_buffer(|b| b.reload()),
            key!(CONTROL, 'j') => self.update_buffer(|b| b.join_selected_lines()),
            _ => { /* ignore other events */ }
        }
    }

    fn process_confirm_close(&mut self, event: Event, buffer_id: BufferId) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            key!('y') => {
                // close buffer anyway
                self.layout.remove(buffer_id);
                self.mode = EditorMode::default();
            }
            key!('n') => {
                // cancel close buffer
                self.mode = EditorMode::default();
            }
            _ => { /* ignore other events */ }
        }
    }

    fn process_verify_save(&mut self, event: Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            key!('y') => {
                // save buffer anyway
                self.update_buffer(|b| b.save());
                self.mode = EditorMode::default();
            }
            key!('n') => {
                // cancel save
                self.mode = EditorMode::default();
            }
            _ => { /* ignore other events */ }
        }
    }

    fn process_verify_reload(&mut self, event: Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            key!('y') => {
                // reload buffer anyway
                self.update_buffer(|b| b.reload());
                self.mode = EditorMode::default();
            }
            key!('n') => {
                // cancel reload
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
                code: KeyCode::Char('<' | '>'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.select_inside(('<', '>'), Some(('>', '<'))));
                self.mode = EditorMode::default();
            }
            key!('"') => {
                self.update_buffer(|b| b.select_inside(('"', '"'), None));
                self.mode = EditorMode::default();
            }
            key!('\'') => {
                self.update_buffer(|b| b.select_inside(('\'', '\''), None));
                self.mode = EditorMode::default();
            }
            _ => { /* do nothing */ }
        }
    }
}

fn process_select_line(
    buffer: &mut BufferContext,
    prompt: &mut LinePrompt,
    event: Event,
) -> Option<EditorMode> {
    use crate::prompt::Digit;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            if let Ok(d) = Digit::try_from(c) {
                prompt.push(d);
            }
            None
        }
        key!(Backspace) => {
            prompt.pop();
            None
        }
        key!(Enter) => {
            buffer.select_line(prompt.line().saturating_sub(1));
            Some(EditorMode::default())
        }
        key!(Home) => {
            buffer.select_line(0);
            Some(EditorMode::default())
        }
        key!(End) => {
            buffer.select_line(buffer.last_line());
            Some(EditorMode::default())
        }
        _ => {
            None // ignore other events
        }
    }
}

fn process_open_file(layout: &mut Layout, prompt: &mut Prompt, event: Event) -> Option<EditorMode> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    // TODO - support tab completion
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
        key!(Backspace) => {
            prompt.pop();
            None
        }
        key!(Enter) => layout
            .add(prompt.to_string().into())
            .map(|()| EditorMode::default())
            .ok(),
        _ => None, // ignore other events
    }
}

fn process_find(
    buffer: &mut BufferContext,
    prompt: &mut SearchPrompt,
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
        key!(Backspace) => {
            prompt.pop();
            None
        }
        key!(Up) => {
            prompt.previous_entry();
            None
        }
        key!(Down) => {
            prompt.next_entry();
            None
        }
        key!(Enter) => {
            match prompt.get_value() {
                Some(search) => {
                    let matches = buffer.matches(&search);
                    Some(if matches.is_empty() {
                        buffer.set_error("Not Found");
                        EditorMode::default()
                    } else {
                        buffer.clear_selection();
                        EditorMode::SelectMatches {
                            matches,
                            match_idx: None,
                            search_history: prompt.history().clone(),
                        }
                    })
                }
                None => Some(EditorMode::default()), // no search term
            }
        }
        _ => None, // ignore other events
    }
}

fn process_select_matches(
    buffer: &mut BufferContext,
    matches: &mut Vec<(usize, usize)>,
    match_idx: &mut Option<usize>,
    search_history: &SearchHistory,
    event: Event,
) -> Option<EditorMode> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Left | KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => match match_idx {
            Some(match_idx) => {
                *match_idx = match_idx.checked_sub(1).unwrap_or(matches.len() - 1);
                if let Some((s, e)) = matches.get(*match_idx) {
                    buffer.set_selection(*s, *e);
                }
                None
            }
            None => {
                let cursor = buffer.get_cursor();
                let (idx, (s, e)) = matches
                    .iter()
                    .enumerate()
                    .rfind(|(_, (s, _))| *s < cursor)?;
                *match_idx = Some(idx);
                buffer.set_selection(*s, *e);
                None
            }
        },
        Event::Key(KeyEvent {
            code: KeyCode::Down | KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => match match_idx {
            Some(match_idx) => {
                *match_idx = (*match_idx + 1) % matches.len();
                if let Some((s, e)) = matches.get(*match_idx) {
                    buffer.set_selection(*s, *e);
                }
                None
            }
            None => {
                let cursor = buffer.get_cursor();
                let (idx, (s, e)) = matches.iter().enumerate().find(|(_, (s, _))| *s > cursor)?;
                *match_idx = Some(idx);
                buffer.set_selection(*s, *e);
                None
            }
        },
        Event::Key(KeyEvent {
            code: KeyCode::Backspace | KeyCode::Delete,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            let match_idx = match_idx.as_mut()?;

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
        key!(CONTROL, 'f') => {
            buffer.clear_selection();
            Some(EditorMode::Find {
                prompt: SearchPrompt::new(search_history),
            })
        }
        key!(CONTROL, 'r') => {
            buffer.clear_matches(matches);
            if let Some((cursor, _)) = match_idx.and_then(|idx| matches.get(idx)) {
                buffer.set_cursor(*cursor);
            }
            Some(EditorMode::ReplaceMatches {
                matches: std::mem::take(matches)
                    .into_iter()
                    .map(|(s, _)| (s, s))
                    .collect(),
                match_idx: std::mem::take(match_idx),
            })
        }
        key!(Enter) => Some(EditorMode::default()),
        _ => None, // ignore other events
    }
}

fn process_replace_matches(
    buffer: &mut BufferContext,
    matches: &mut [(usize, usize)],
    match_idx: &mut Option<usize>,
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
            buffer.multi_insert_char(matches, c);
            if let Some((_, cursor)) = match_idx.and_then(|idx| matches.get(idx)) {
                buffer.set_cursor(*cursor);
            }
            None
        }
        key!(Backspace) => {
            buffer.multi_backspace(matches);
            if let Some((_, cursor)) = match_idx.and_then(|idx| matches.get(idx)) {
                buffer.set_cursor(*cursor);
            }
            None
        }
        key!(Enter) => Some(EditorMode::default()),
        Event::Key(KeyEvent {
            code: KeyCode::Left | KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => match match_idx {
            Some(match_idx) => {
                *match_idx = match_idx.checked_sub(1).unwrap_or(matches.len() - 1);
                if let Some((_, e)) = matches.get(*match_idx) {
                    buffer.set_cursor(*e);
                }
                None
            }
            None => {
                let cursor = buffer.get_cursor();
                let (idx, (_, e)) = matches
                    .iter()
                    .enumerate()
                    .rfind(|(_, (s, _))| *s < cursor)?;
                *match_idx = Some(idx);
                buffer.set_cursor(*e);
                None
            }
        },
        Event::Key(KeyEvent {
            code: KeyCode::Down | KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => match match_idx {
            Some(match_idx) => {
                *match_idx = (*match_idx + 1) % matches.len();
                if let Some((_, e)) = matches.get(*match_idx) {
                    buffer.set_cursor(*e);
                }
                None
            }
            None => {
                let cursor = buffer.get_cursor();
                let (idx, (_, e)) = matches.iter().enumerate().find(|(_, (s, _))| *s > cursor)?;
                *match_idx = Some(idx);
                buffer.set_cursor(*e);
                None
            }
        },
        _ => None,
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
        self.selected_buffer_list_mut()
            .select_by_name(&path)
            .or_else(|()| match BufferContext::open(path) {
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
                Err(err) => {
                    if let Some(buf) = self.selected_buffer_list_mut().current_mut() {
                        buf.set_error(err.to_string());
                    }
                    Err(())
                }
            })
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

    fn cursor_position(&self, area: Rect, mode: &EditorMode) -> Option<Position> {
        use ratatui::layout::{Constraint, Layout};

        // generate a duplicate of our existing block layout
        // and then apply cursor's position to it
        fn apply_position(
            area: Rect,
            (row, col): (usize, usize),
            mode: &EditorMode,
        ) -> Option<Position> {
            use crate::prompt::Prompt;
            use ratatui::{
                layout::Constraint::{Length, Min},
                widgets::Block,
            };

            let [text_area, _] =
                Layout::horizontal([Min(0), Length(1)]).areas(Block::bordered().inner(area));

            match mode {
                EditorMode::SelectLine { prompt } => Some(Position {
                    x: text_area.x + prompt.len() as u16 + 1,
                    y: text_area.y + text_area.height.saturating_sub(2),
                }),
                EditorMode::Find { prompt } => Some(Position {
                    x: text_area.x
                        + prompt
                            .width()
                            .min(Prompt::MAX_WIDTH)
                            .min(text_area.width.saturating_sub(2))
                        + 1,
                    y: text_area.y + text_area.height.saturating_sub(2),
                }),
                EditorMode::Open { prompt } => Some(Position {
                    x: text_area.x
                        + prompt
                            .width()
                            .min(Prompt::MAX_WIDTH)
                            .min(text_area.width.saturating_sub(2))
                        + 1,
                    y: text_area.y + text_area.height.saturating_sub(2),
                }),
                _ => {
                    let x = (col + usize::from(text_area.x))
                        .min((text_area.x + text_area.width).into());
                    let y = (row + usize::from(text_area.y))
                        .min((text_area.y + text_area.height).into());

                    Some(Position {
                        x: u16::try_from(x).ok()?,
                        y: u16::try_from(y).ok()?,
                    })
                }
            }
        }

        match self {
            Self::Single(buf) => buf
                .cursor_viewport_position()
                .and_then(|pos| apply_position(area, pos, mode)),
            Self::Horizontal { top, bottom, which } => {
                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);

                match which {
                    HorizontalPos::Top => top
                        .cursor_viewport_position()
                        .and_then(|pos| apply_position(top_area, pos, mode)),
                    HorizontalPos::Bottom => bottom
                        .cursor_viewport_position()
                        .and_then(|pos| apply_position(bottom_area, pos, mode)),
                }
            }
            Self::Vertical { left, right, which } => {
                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);

                match which {
                    VerticalPos::Left => left
                        .cursor_viewport_position()
                        .and_then(|pos| apply_position(left_area, pos, mode)),
                    VerticalPos::Right => right
                        .cursor_viewport_position()
                        .and_then(|pos| apply_position(right_area, pos, mode)),
                }
            }
        }
    }
}

struct LayoutWidget<'e> {
    mode: &'e mut EditorMode,
    show_help: bool,
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

        let Self { mode, show_help } = self;

        match layout {
            Layout::Single(single) => {
                let buffer_index = single.current_index();
                let total_buffers = single.total_buffers();
                if let Some(buffer) = single.current_mut() {
                    BufferWidget {
                        mode: Some(mode),
                        show_help,
                        buffer_index,
                        total_buffers,
                    }
                    .render(area, buf, buffer);
                }
            }
            Layout::Horizontal { top, bottom, which } => {
                use ratatui::layout::{Constraint, Layout};

                let top_index = top.current_index();
                let top_total = top.total_buffers();
                let bottom_index = bottom.current_index();
                let bottom_total = bottom.total_buffers();

                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);

                if let Some(buffer) = top.current_mut() {
                    BufferWidget {
                        mode: match which {
                            HorizontalPos::Top => Some(mode),
                            HorizontalPos::Bottom => None,
                        },
                        show_help: show_help && !matches!(which, HorizontalPos::Top),
                        buffer_index: top_index,
                        total_buffers: top_total,
                    }
                    .render(top_area, buf, buffer);
                }
                if let Some(buffer) = bottom.current_mut() {
                    BufferWidget {
                        mode: match which {
                            HorizontalPos::Top => None,
                            HorizontalPos::Bottom => Some(mode),
                        },
                        show_help: show_help && !matches!(which, HorizontalPos::Bottom),
                        buffer_index: bottom_index,
                        total_buffers: bottom_total,
                    }
                    .render(bottom_area, buf, buffer);
                }
            }
            Layout::Vertical { left, right, which } => {
                use ratatui::layout::{Constraint, Layout};

                let left_index = left.current_index();
                let left_total = left.total_buffers();
                let right_index = right.current_index();
                let right_total = right.total_buffers();

                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);

                if let Some(buffer) = left.current_mut() {
                    BufferWidget {
                        mode: match which {
                            VerticalPos::Left => Some(mode),
                            VerticalPos::Right => None,
                        },
                        show_help: show_help && !matches!(which, VerticalPos::Left),
                        buffer_index: left_index,
                        total_buffers: left_total,
                    }
                    .render(left_area, buf, buffer);
                }
                if let Some(buffer) = right.current_mut() {
                    BufferWidget {
                        mode: match which {
                            VerticalPos::Left => None,
                            VerticalPos::Right => Some(mode),
                        },
                        show_help: show_help && !matches!(which, VerticalPos::Right),
                        buffer_index: right_index,
                        total_buffers: right_total,
                    }
                    .render(right_area, buf, buffer);
                }
            }
        }
    }
}
