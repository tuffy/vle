// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(feature = "ssh")]
use crate::files::{EitherSource, SshSource};
use crate::key;
use crate::{
    buffer::{
        AltCursor, BufferContext, BufferId, BufferList, EditorCutBuffer, MatchCapture, MultiCursor,
        SelectionRange, Source,
    },
    files::{ChooserSource, FileChooserState, LocalSource},
    key::Binding,
    prompt::{LinePrompt, TextField},
};
use crossterm::event::Event;
use ratatui::{
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
use std::ops::Range;
use std::sync::LazyLock;

static PAGE_SIZE: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("VLE_PAGE_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .map(|s| s.clamp(1, 100))
        .unwrap_or(25)
});

#[derive(Default)]
pub enum EditorMode {
    /// Regular editing mode
    #[default]
    Editing,
    /// Verifying a file overwrite
    VerifySave,
    /// Verifying a reload over a dirty buffer
    VerifyReload,
    /// Querying which direction to split the pane
    SplitPane,
    /// Verifying whether to close dirty buffer
    ConfirmClose { buffer: BufferId },
    /// Querying for what to select inside of
    SelectInside,
    /// Querying for which line to select
    SelectLine { prompt: LinePrompt },
    /// Querying for what text to search for
    Search {
        prompt: TextField,
        type_: SearchType,
        range: Option<SelectionRange>,
    },
    /// Multi-cursor operation
    MultiCursor {
        matches: Vec<MultiCursor>,
        match_idx: usize,
        groups: CaptureGroups,
        range: Option<SelectionRange>,
        highlight: bool,
    },
    /// Querying for what regex group to paste
    PasteGroup {
        matches: Vec<MultiCursor>,
        match_idx: usize,
        total: usize,
        groups: Vec<Vec<Option<MatchCapture>>>,
        range: Option<SelectionRange>,
        highlight: bool,
    },
    /// Opening a new file
    Open {
        #[cfg(not(feature = "ssh"))]
        chooser: Box<FileChooserState<LocalSource>>,
        #[cfg(feature = "ssh")]
        chooser: Box<FileChooserState<EitherSource>>,
    },
    /// Performing autocomplete on a partial word
    Autocomplete {
        offset: usize,            // our character offset in rope
        completions: Vec<String>, // autocompletion candidates
        index: usize,             // the current candidate
    },
    AutocompleteSearch {
        prompt: TextField,
        type_: SearchType,
        range: Option<SelectionRange>,
        offset: usize,            // our character offset in rope
        completions: Vec<String>, // autocompletion candidates
        index: usize,             // the current candidate
    },
    AutocompleteMulti {
        matches: Vec<MultiCursor>,
        match_idx: usize,
        groups: CaptureGroups,
        range: Option<SelectionRange>,
        offsets: Vec<usize>,      // autocompletion offsets
        completions: Vec<String>, // autocompletion candidates
        index: usize,             // current autocompletion candidate
    },
}

#[derive(Copy, Clone, Default)]
pub enum SearchType {
    #[default]
    Plain,
    Regex,
}

impl std::fmt::Display for SearchType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Plain => "Find".fmt(f),
            Self::Regex => "Find Regex".fmt(f),
        }
    }
}

/// The regex groups captured during a find/replace
#[derive(Default)]
pub enum CaptureGroups {
    #[default]
    None,
    Some {
        // total number of capture groups
        total: usize,
        // groups[match][group]
        groups: Vec<Vec<Option<MatchCapture>>>,
    },
}

impl CaptureGroups {
    fn remove(&mut self, match_idx: usize) {
        if let Self::Some { groups, .. } = self {
            groups.remove(match_idx);
        }
    }
}

impl From<Vec<Vec<Option<MatchCapture>>>> for CaptureGroups {
    fn from(groups: Vec<Vec<Option<MatchCapture>>>) -> Self {
        match groups.first().map(|g| g.len()) {
            None | Some(0) => CaptureGroups::None,
            Some(total) => CaptureGroups::Some { total, groups },
        }
    }
}

macro_rules! keybind {
    ($bind:ident) => {
        Event::Key(
            KeyEvent {
                code: key::$bind::PRIMARY_KEY,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } | KeyEvent {
                code: key::$bind::SECONDARY_KEY,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            },
        )
    };
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
    (F($code:literal)) => {
        Event::Key(KeyEvent {
            code: KeyCode::F($code),
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
    layout: Layout,                       // the editor's pane layout
    focused: bool,                        // whether the editor has focus
    mode: EditorMode,                     // what mode the editing is in
    cut_buffer: Option<EditorCutBuffer>,  // contents of cut buffer
    last_plain_search: Option<TextField>, // previous plaintext search
    last_regex_search: Option<TextField>, // previous regex search
    show_help: bool,                      // whether to show keybindinings
    show_sub_help: bool,                  // whether to show sub-mode help
    #[cfg(feature = "ssh")]
    remote: Option<ssh2::Session>, // remote SSH session
}

impl Editor {
    pub fn new(buffers: impl IntoIterator<Item = Source>) -> std::io::Result<Self> {
        Ok(Self {
            layout: Layout::Single(BufferList::new(buffers)?),
            focused: true,
            mode: EditorMode::default(),
            cut_buffer: None,
            last_plain_search: None,
            last_regex_search: None,
            show_help: false,
            show_sub_help: true,
            #[cfg(feature = "ssh")]
            remote: None,
        })
    }

    #[cfg(feature = "ssh")]
    pub fn new_remote(
        buffers: impl IntoIterator<Item = Source>,
        remote: ssh2::Session,
    ) -> std::io::Result<Self> {
        Ok(Self {
            mode: SshSource::open(&remote)
                .ok()
                .and_then(|source| FileChooserState::new(EitherSource::Ssh(source)).ok())
                .map(|chooser| EditorMode::Open {
                    chooser: Box::new(chooser),
                })
                .unwrap_or_default(),
            remote: Some(remote),
            ..Self::new(buffers)?
        })
    }

    pub fn at_line(mut self, LineNumber { line, column }: LineNumber) -> Self {
        let buffers = self.layout.selected_buffer_list_mut();
        if let Some(first) = buffers.current_mut() {
            match column {
                None => first.select_line(line),
                Some(column) => first.select_line_and_column(line, column),
            }
        }
        self
    }

    pub fn has_open_buffers(&self) -> bool {
        self.layout.has_open_buffers()
    }

    /// Returns size of frame, if successful
    pub fn display(&mut self, term: &mut ratatui::DefaultTerminal) -> std::io::Result<Rect> {
        term.draw(|frame| {
            let area = frame.area();
            frame.render_stateful_widget(
                EditorWidget {
                    focused: self.focused,
                    show_help: self.show_help
                        && matches!(
                            &self.mode,
                            EditorMode::Editing | EditorMode::Autocomplete { .. }
                        ),
                    show_sub_help: self.show_sub_help,
                    mode: &mut self.mode,
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
        .map(|completed_frame| completed_frame.area)
    }

    fn update_buffer(&mut self, f: impl FnOnce(&mut crate::buffer::BufferContext)) {
        self.layout.selected_buffer_list_mut().update_buf(f)
    }

    fn update_buffer_at(
        &mut self,
        f: impl FnOnce(&mut crate::buffer::BufferContext, Vec<AltCursor<'_>>),
    ) {
        self.layout.update_current_at(f);
    }

    fn on_buffer<T>(
        &mut self,
        f: impl FnOnce(&mut crate::buffer::BufferContext) -> T,
    ) -> Option<T> {
        self.layout.on_current(f)
    }

    fn on_buffer_at<T>(
        &mut self,
        f: impl FnOnce(&mut crate::buffer::BufferContext, Vec<AltCursor<'_>>) -> T,
    ) -> Option<T> {
        self.layout.on_current_at(f)
    }

    fn perform_cut(&mut self) {
        if let Some(Some(selection)) = self.layout.on_current_at(|b, a| b.take_selection(a)) {
            self.cut_buffer = Some(EditorCutBuffer::Single(selection));
        }
    }

    fn perform_copy(&mut self) {
        if let Some(Some(selection)) = self.layout.on_current(|b| b.get_selection()) {
            self.cut_buffer = Some(EditorCutBuffer::Single(selection));
        }
    }

    pub fn process_event(&mut self, area: Rect, event: Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            // Esc is an escape hatch that always returns to normal mode
            // regardless of what mode we were in before
            key!(Esc) => {
                self.mode = EditorMode::default();
            }
            Event::Key(KeyEvent {
                code: KeyCode::F(1),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => match self.mode {
                EditorMode::Editing => {
                    self.show_help = !self.show_help;
                }
                _ => {
                    self.show_sub_help = !self.show_sub_help;
                }
            },
            Event::FocusGained => {
                self.focused = true;
            }
            Event::FocusLost => {
                self.focused = false;
            }
            event => match &mut self.mode {
                EditorMode::Editing => self.process_normal_event(area, event),
                EditorMode::Autocomplete {
                    offset,
                    completions,
                    index,
                } => match event {
                    key!(Tab) => {
                        // switch to next candidate
                        self.layout.update_current_at(|b, a| {
                            let (current, next) = complete_forward(index, completions);
                            b.autocomplete(a, *offset, current, next);
                        });
                    }
                    key!(SHIFT, BackTab) => {
                        // switch to previous candidate
                        self.layout.update_current_at(|b, a| {
                            let (current, previous) = complete_backward(index, completions);
                            b.autocomplete(a, *offset, current, previous);
                        })
                    }
                    event => {
                        // end autocomplete
                        self.mode = EditorMode::default();
                        self.process_normal_event(area, event);
                    }
                },
                EditorMode::AutocompleteSearch {
                    prompt,
                    type_,
                    range,
                    offset,
                    index,
                    completions,
                } => match event {
                    key!(Tab) => {
                        // switch to next candidate
                        let (current, next) = complete_forward(index, completions);
                        prompt.autocomplete(*offset, current, next);
                    }
                    key!(SHIFT, BackTab) => {
                        // switch to previous candidate
                        let (current, previous) = complete_backward(index, completions);
                        prompt.autocomplete(*offset, current, previous);
                    }
                    event => {
                        // end autocomplete
                        self.mode = EditorMode::Search {
                            prompt: std::mem::take(prompt),
                            type_: std::mem::take(type_),
                            range: std::mem::take(range),
                        };
                        self.process_event(area, event);
                    }
                },
                EditorMode::AutocompleteMulti {
                    matches,
                    match_idx,
                    groups,
                    range,
                    offsets,
                    completions,
                    index,
                } => match event {
                    key!(Tab) => {
                        // switch to next candidate
                        let (current, next) = complete_forward(index, completions);
                        self.layout.update_current_at(|b, a| {
                            b.multi_autocomplete(a, matches, offsets, current, next);
                        });
                    }
                    key!(SHIFT, BackTab) => {
                        // switch to previous candidate
                        let (current, previous) = complete_backward(index, completions);
                        self.layout.update_current_at(|b, a| {
                            b.multi_autocomplete(a, matches, offsets, current, previous);
                        });
                    }
                    event => {
                        // end autocomplete
                        self.mode = EditorMode::MultiCursor {
                            matches: std::mem::take(matches),
                            match_idx: std::mem::take(match_idx),
                            groups: std::mem::take(groups),
                            range: std::mem::take(range),
                            highlight: false,
                        };
                        self.process_event(area, event);
                    }
                },
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
                EditorMode::Open { chooser } => {
                    if let Some(new_mode) = process_open_file(&mut self.layout, chooser, event) {
                        self.mode = new_mode;
                    }
                }
                EditorMode::Search {
                    prompt,
                    type_,
                    range,
                } => {
                    if let Some(buf) = self.layout.selected_buffer_list_mut().current_mut()
                        && let Some(new_mode) = process_search(
                            buf,
                            self.cut_buffer.as_ref(),
                            match type_ {
                                SearchType::Plain => &mut self.last_plain_search,
                                SearchType::Regex => &mut self.last_regex_search,
                            },
                            prompt,
                            type_,
                            range.as_ref(),
                            event,
                        )
                    {
                        self.mode = match new_mode {
                            NextModeIncremental::Browse { match_idx, matches } => {
                                buf.set_cursor(matches[match_idx].0.end);
                                buf.clear_selection();

                                let (matches, groups): (_, Vec<Vec<_>>) =
                                    matches.into_iter().map(|(r, c)| (r.into(), c)).unzip();

                                EditorMode::MultiCursor {
                                    matches,
                                    match_idx,
                                    groups: groups.into(),
                                    range: range.take(),
                                    highlight: true,
                                }
                            }
                            NextModeIncremental::Autocomplete {
                                offset,
                                completions,
                                index,
                            } => EditorMode::AutocompleteSearch {
                                prompt: std::mem::take(prompt),
                                type_: std::mem::take(type_),
                                range: std::mem::take(range),
                                offset,
                                completions,
                                index,
                            },
                            NextModeIncremental::SelectLine => EditorMode::SelectLine {
                                prompt: LinePrompt::default(),
                            },
                        };
                    }
                }
                EditorMode::MultiCursor {
                    matches,
                    match_idx,
                    groups,
                    range,
                    highlight,
                } => {
                    if let Some(Some(new_mode)) = self.layout.on_current_at(|b, a| {
                        process_multi_cursor(
                            b,
                            &mut self.cut_buffer,
                            matches,
                            groups,
                            range,
                            match_idx,
                            highlight,
                            event,
                            a,
                        )
                    }) {
                        self.mode = new_mode;
                    }
                }
                EditorMode::PasteGroup {
                    matches,
                    match_idx,
                    total,
                    groups,
                    range,
                    highlight,
                } => {
                    self.layout.update_current_at(|b, a| {
                        process_paste_group(b, matches, self.cut_buffer.as_mut(), groups, event, a);
                    });

                    self.mode = EditorMode::MultiCursor {
                        matches: std::mem::take(matches),
                        match_idx: std::mem::take(match_idx),
                        groups: CaptureGroups::Some {
                            total: std::mem::take(total),
                            groups: std::mem::take(groups),
                        },
                        range: range.take(),
                        highlight: std::mem::take(highlight),
                    };
                }
                EditorMode::SplitPane => self.process_split_pane(event),
            },
        }
    }

    fn process_normal_event(&mut self, area: Rect, event: Event) {
        use crate::buffer::SelectionType;
        use crossterm::event::{
            Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
            MouseEventKind,
        };

        match event {
            keybind!(Quit) => {
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
            keybind!(SplitPane) => {
                self.mode = EditorMode::SplitPane;
            }
            key!(CONTROL, Left) => {
                let _ = self.layout.change_pane(Direction::Left);
            }
            key!(CONTROL, Right) => {
                let _ = self.layout.change_pane(Direction::Right);
            }
            key!(CONTROL, Up) => {
                let _ = self.layout.change_pane(Direction::Up);
            }
            key!(CONTROL, Down) => {
                let _ = self.layout.change_pane(Direction::Down);
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
            }) => self.update_buffer(|b| {
                b.cursor_up(*PAGE_SIZE, modifiers.contains(KeyModifiers::SHIFT))
            }),
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => self.update_buffer(|b| {
                b.cursor_down(*PAGE_SIZE, modifiers.contains(KeyModifiers::SHIFT))
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
            }) => self.update_buffer_at(|b, a| b.insert_char(a, c)),
            key!(Backspace) => self.update_buffer_at(|b, a| b.backspace(a)),
            key!(Delete) => self.update_buffer_at(|b, a| b.delete(a)),
            key!(Enter) => self.update_buffer_at(|b, a| b.newline(a)),
            keybind!(WidenSelection) => self.update_buffer(|b| b.select_word_or_lines()),
            key!(CONTROL, 'x') => self.perform_cut(),
            key!(CONTROL, 'c') => self.perform_copy(),
            key!(CONTROL, 'v') => {
                self.layout.update_current_at(|b, a| {
                    b.paste(a, &mut self.cut_buffer);
                });
            }
            Event::Paste(pasted) => {
                self.cut_buffer = Some(EditorCutBuffer::Single(pasted.into()));
                self.layout.update_current_at(|b, a| {
                    b.paste(a, &mut self.cut_buffer);
                });
            }
            key!(CONTROL, 'z') => self.update_buffer(|b| b.perform_undo()),
            key!(CONTROL, 'y') => self.update_buffer(|b| b.perform_redo()),
            keybind!(Save) => {
                if let Some(Err(crate::buffer::Modified)) = self.on_buffer(|b| b.verified_save()) {
                    self.mode = EditorMode::VerifySave;
                }
            }
            key!(Tab) => {
                if let Some(Some((offset, completions))) =
                    self.on_buffer_at(|b, a| b.complete_or_indent(a))
                {
                    match init_complete_forward(&completions) {
                        Some((index, original, replacement)) => {
                            self.update_buffer_at(|b, a| {
                                b.autocomplete(a, offset, original, replacement)
                            });
                            self.mode = EditorMode::Autocomplete {
                                offset,
                                completions,
                                index,
                            };
                        }
                        None => {
                            self.update_buffer(|b| b.set_error("No Completion Found"));
                        }
                    }
                };
            }
            key!(SHIFT, BackTab) => {
                if let Some(Some((offset, completions))) =
                    self.on_buffer_at(|b, a| b.complete_or_unindent(a))
                {
                    match init_complete_backward(&completions) {
                        Some((index, original, replacement)) => {
                            self.update_buffer_at(|b, a| {
                                b.autocomplete(a, offset, original, replacement)
                            });
                            self.mode = EditorMode::Autocomplete {
                                offset,
                                completions,
                                index,
                            };
                        }
                        None => {
                            self.update_buffer(|b| b.set_error("No Completion Found"));
                        }
                    }
                };
            }
            keybind!(GotoPair) => self.update_buffer(|b| b.select_matching_paren()),
            keybind!(Bookmark) => self.update_buffer(|b| b.toggle_bookmark()),
            keybind!(SelectInside) => {
                if let Some(Err(())) = self.on_buffer(|b| b.try_auto_pair()) {
                    self.mode = EditorMode::SelectInside;
                }
            }
            keybind!(GotoLine) => {
                self.mode = EditorMode::SelectLine {
                    prompt: LinePrompt::default(),
                };
            }
            keybind!(Find) => {
                if let Some(Ok(find)) = self.on_buffer(|b| match b.selection_range() {
                    Some(SelectionType::Term(selection)) => {
                        b.all_matches(None, selection).map(|(match_idx, matches)| {
                            b.set_cursor(matches[match_idx].0.end);
                            b.clear_selection();

                            let (matches, groups): (_, Vec<Vec<_>>) =
                                matches.into_iter().map(|(r, c)| (r.into(), c)).unzip();

                            EditorMode::MultiCursor {
                                matches,
                                match_idx,
                                groups: groups.into(),
                                range: None,
                                highlight: true,
                            }
                        })
                    }
                    Some(SelectionType::Range(range)) => Ok(EditorMode::Search {
                        prompt: TextField::default(),
                        type_: SearchType::default(),
                        range: Some(range),
                    }),
                    None => Ok(EditorMode::Search {
                        prompt: TextField::default(),
                        type_: SearchType::default(),
                        range: None,
                    }),
                }) {
                    self.mode = find;
                }
            }
            #[cfg(not(feature = "ssh"))]
            keybind!(Open) => match FileChooserState::new(LocalSource) {
                Ok(chooser) => {
                    self.mode = EditorMode::Open {
                        chooser: Box::new(chooser),
                    }
                }
                Err(err) => {
                    self.update_buffer(|b| b.set_error(err.to_string()));
                }
            },
            #[cfg(feature = "ssh")]
            keybind!(Open) => match self.remote.as_ref() {
                None => match FileChooserState::new(EitherSource::Local(LocalSource)) {
                    Ok(chooser) => {
                        self.mode = EditorMode::Open {
                            chooser: Box::new(chooser),
                        }
                    }
                    Err(err) => {
                        self.update_buffer(|b| b.set_error(err.to_string()));
                    }
                },
                Some(remote) => match SshSource::open(remote) {
                    Ok(source) => match FileChooserState::new(EitherSource::Ssh(source)) {
                        Ok(chooser) => {
                            self.mode = EditorMode::Open {
                                chooser: Box::new(chooser),
                            }
                        }
                        Err(err) => {
                            self.update_buffer(|b| b.set_error(err.to_string()));
                        }
                    },
                    Err(err) => {
                        self.update_buffer(|b| b.set_error(err.to_string()));
                    }
                },
            },
            keybind!(Reload) => {
                if let Some(new_mode) = self.on_buffer_at(|b, a| {
                    if b.modified() {
                        EditorMode::VerifyReload
                    } else {
                        b.reload(a);
                        EditorMode::default()
                    }
                }) {
                    self.mode = new_mode;
                }
            }
            keybind!(UpdateLines) | key!(CONTROL, 'r') => {
                if let Some(matches) = self.on_buffer(|b| b.selection_cursors())
                    && let Some(match_idx) = matches.len().checked_sub(1)
                {
                    self.mode = EditorMode::MultiCursor {
                        matches,
                        match_idx,
                        groups: CaptureGroups::default(),
                        range: None,
                        highlight: false,
                    };
                }
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_down(1, modifiers.contains(KeyModifiers::SHIFT)));
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollUp,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_up(1, modifiers.contains(KeyModifiers::SHIFT)));
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollLeft,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_back(modifiers.contains(KeyModifiers::SHIFT)));
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollRight,
                modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_forward(modifiers.contains(KeyModifiers::SHIFT)));
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                ..
            }) => {
                self.layout
                    .set_cursor_focus(area, Position { y: row, x: column });
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Right),
                column,
                row,
                ..
            }) => {
                self.layout
                    .set_cursor_focus(area, Position { y: row, x: column });
                self.update_buffer(|b| b.select_word_or_lines());
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Middle),
                column,
                row,
                ..
            }) => {
                self.layout
                    .set_cursor_focus(area, Position { y: row, x: column });
                self.layout.update_current_at(|b, a| {
                    b.paste(a, &mut self.cut_buffer);
                });
            }
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
                self.update_buffer_at(|b, a| b.reload(a));
                self.mode = EditorMode::default();
            }
            key!('n') => {
                // cancel reload
                self.mode = EditorMode::default();
            }
            _ => { /* ignore other events */ }
        }
    }

    fn process_split_pane(&mut self, event: Event) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            key!(Up) => {
                self.layout.split_pane(Direction::Up);
                self.mode = EditorMode::default();
            }
            key!(Down) => {
                self.layout.split_pane(Direction::Down);
                self.mode = EditorMode::default();
            }
            key!(Left) => {
                self.layout.split_pane(Direction::Left);
                self.mode = EditorMode::default();
            }
            key!(Right) => {
                self.layout.split_pane(Direction::Right);
                self.mode = EditorMode::default();
            }
            key!(Delete) => {
                self.layout.delete_current_pane();
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
        Event::Paste(pasted) => match pasted.split_once(':') {
            None => match pasted.parse::<usize>() {
                Ok(line) => {
                    buffer.select_line(line.saturating_sub(1));
                    Some(EditorMode::default())
                }
                Err(_) => {
                    buffer.set_error("Invalid Line Number");
                    None
                }
            },
            Some((line, column)) => match line.parse::<usize>() {
                Ok(line) => match column.parse::<usize>() {
                    Ok(col) => {
                        buffer
                            .select_line_and_column(line.saturating_sub(1), col.saturating_sub(1));
                        Some(EditorMode::default())
                    }
                    Err(_) => {
                        buffer.set_error("Invalid Column Number");
                        None
                    }
                },
                Err(_) => {
                    buffer.set_error("Invalid Line Number");
                    None
                }
            },
        },
        key!(Backspace) => {
            prompt.pop();
            None
        }
        key!(Enter) => {
            if prompt.is_empty() {
                Some(EditorMode::default())
            } else {
                match prompt.line_and_column() {
                    (line, None) => buffer.select_line(line.saturating_sub(1)),
                    (line, Some(col)) => {
                        buffer.select_line_and_column(line.saturating_sub(1), col.saturating_sub(1))
                    }
                }
                Some(EditorMode::default())
            }
        }
        key!(Home) => {
            buffer.select_line(0);
            Some(EditorMode::default())
        }
        key!(End) => {
            buffer.select_line(buffer.last_line());
            Some(EditorMode::default())
        }
        keybind!(Find) => Some(EditorMode::Search {
            prompt: TextField::default(),
            type_: SearchType::default(),
            range: None,
        }),
        key!(Up) => {
            buffer.previous_bookmark();
            None
        }
        key!(Down) => {
            buffer.next_bookmark();
            None
        }
        key!(Delete) => {
            buffer.delete_bookmark();
            None
        }
        _ => {
            None // ignore other events
        }
    }
}

fn process_open_file<S: ChooserSource>(
    layout: &mut Layout,
    chooser: &mut FileChooserState<S>,
    event: Event,
) -> Option<EditorMode> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        key!(Up) => {
            chooser.arrow_up();
            None
        }
        key!(Down) => {
            chooser.arrow_down();
            None
        }
        key!(Home) => {
            chooser.home();
            None
        }
        key!(End) => {
            chooser.end();
            None
        }
        key!(PageUp) => {
            chooser.page_up();
            None
        }
        key!(PageDown) => {
            chooser.page_down();
            None
        }
        key!(Left) => {
            chooser.arrow_left();
            None
        }
        key!(Right) => {
            chooser.arrow_right();
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            chooser.insert_char(c);
            None
        }
        key!(Backspace) => {
            chooser.backspace();
            None
        }
        key!(Tab) => {
            chooser.toggle_selected();
            None
        }
        key!(CONTROL, 'h') => {
            chooser.toggle_show_hidden();
            None
        }
        key!(Enter) => {
            for selected in chooser.select()? {
                if let Err(()) = layout.add(selected) {
                    return Some(EditorMode::default());
                }
            }
            Some(EditorMode::default())
        }
        _ => None, // ignore other events
    }
}

// which mode to switch to next
enum NextModeIncremental {
    Browse {
        match_idx: usize,
        matches: Vec<(Range<usize>, Vec<Option<MatchCapture>>)>,
    },
    SelectLine,
    Autocomplete {
        offset: usize,
        completions: Vec<String>,
        index: usize,
    },
}

fn process_search(
    buffer: &mut BufferContext,
    cut_buffer: Option<&EditorCutBuffer>,
    last_search: &mut Option<TextField>,
    prompt: &mut TextField,
    type_: &mut SearchType,
    range: Option<&SelectionRange>,
    event: Event,
) -> Option<NextModeIncremental> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    fn not_found<Q: std::fmt::Display>(query: Q) -> String {
        format!("Not Found : {query}")
    }

    match event {
        key!(CONTROL, 'v') => {
            if let Some(s) = cut_buffer.and_then(|b| b.cut_str()) {
                prompt.paste(s);
            }
            None
        }
        key!(Tab) => {
            if prompt.is_empty() {
                prompt.reset();
                *type_ = match *type_ {
                    SearchType::Plain => SearchType::Regex,
                    SearchType::Regex => SearchType::Plain,
                };
                None
            } else {
                let (offset, search) = prompt.autocomplete_word()?;
                let completions = buffer.search_autocomplete_matches(search);
                match init_complete_forward(&completions) {
                    Some((index, original, replacement)) => {
                        prompt.autocomplete(offset, original, replacement);
                        Some(NextModeIncremental::Autocomplete {
                            offset,
                            completions,
                            index,
                        })
                    }
                    None => {
                        buffer.set_error("No Completions Found");
                        None
                    }
                }
            }
        }
        key!(SHIFT, BackTab) => {
            if prompt.is_empty() {
                prompt.reset();
                *type_ = match *type_ {
                    SearchType::Plain => SearchType::Regex,
                    SearchType::Regex => SearchType::Plain,
                };
                None
            } else {
                let (offset, search) = prompt.autocomplete_word()?;
                let completions = buffer.search_autocomplete_matches(search);
                match init_complete_backward(&completions) {
                    Some((index, original, replacement)) => {
                        prompt.autocomplete(offset, original, replacement);
                        Some(NextModeIncremental::Autocomplete {
                            offset,
                            completions,
                            index,
                        })
                    }
                    None => {
                        buffer.set_error("No Completions Found");
                        None
                    }
                }
            }
        }
        key!(Enter) => match type_ {
            SearchType::Plain => match buffer.all_matches(range, prompt.value()?) {
                Ok((match_idx, matches)) => {
                    *last_search = Some(std::mem::take(prompt));
                    Some(NextModeIncremental::Browse { match_idx, matches })
                }
                Err(err) => {
                    buffer.set_error(not_found(err));
                    None
                }
            },
            SearchType::Regex => match prompt.value()?.parse::<fancy_regex::Regex>() {
                Ok(regex) => match buffer.all_matches(range, regex) {
                    Ok((match_idx, matches)) => {
                        *last_search = Some(std::mem::take(prompt));
                        Some(NextModeIncremental::Browse { match_idx, matches })
                    }
                    Err(err) => {
                        buffer.set_error(not_found(err));
                        None
                    }
                },
                Err(err) => {
                    buffer.set_error(err.to_string());
                    None
                }
            },
        },
        keybind!(GotoLine) => Some(NextModeIncremental::SelectLine),
        keybind!(Find) => {
            if prompt.is_empty()
                && let Some(last) = last_search
            {
                *prompt = last.clone();
            } else {
                prompt.reset();
            }
            None
        }
        event => {
            prompt.process_event(event);
            None
        }
    }
}

// Yes, I know this has a lot of arguments
#[allow(clippy::too_many_arguments)]
fn process_multi_cursor(
    buffer: &mut BufferContext,
    cut_buffer: &mut Option<EditorCutBuffer>,
    matches: &mut Vec<MultiCursor>,
    groups: &mut CaptureGroups,
    range: &mut Option<SelectionRange>,
    match_idx: &mut usize,
    highlight: &mut bool,
    event: Event,
    alt: Vec<AltCursor<'_>>,
) -> Option<EditorMode> {
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
    };

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_insert_char(alt, matches, c);
            None
        }
        Event::Paste(pasted) => {
            *highlight = false;
            buffer.multi_insert_string(alt, matches, &pasted);
            None
        }
        key!(Backspace) => {
            *highlight = false;
            buffer.multi_backspace(alt, matches);
            None
        }
        key!(Delete) => {
            *highlight = false;
            buffer.multi_delete(alt, matches);
            None
        }
        key!(CONTROL, Delete) => {
            *highlight = true;
            matches.remove(*match_idx);
            groups.remove(*match_idx);
            match matches.len().checked_sub(1) {
                Some(max) => {
                    *match_idx = (*match_idx).min(max);
                    buffer.set_cursor(matches.get(*match_idx)?.cursor());
                    None
                }
                None => Some(EditorMode::default()),
            }
        }
        keybind!(Find) => Some(EditorMode::Search {
            prompt: TextField::default(),
            type_: SearchType::default(),
            range: range.take(),
        }),
        key!(Enter) => Some(EditorMode::default()),
        Event::Key(KeyEvent {
            code: KeyCode::Left,
            modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_cursor_back(matches, modifiers.contains(KeyModifiers::SHIFT));
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_cursor_forward(matches, modifiers.contains(KeyModifiers::SHIFT));
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Home,
            modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_cursor_home(matches, modifiers.contains(KeyModifiers::SHIFT));
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::End,
            modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_cursor_end(matches, modifiers.contains(KeyModifiers::SHIFT));
            None
        }
        key!(CONTROL, 'v') => match groups {
            CaptureGroups::Some { total, groups } => Some(EditorMode::PasteGroup {
                matches: std::mem::take(matches),
                match_idx: std::mem::take(match_idx),
                total: std::mem::take(total),
                groups: std::mem::take(groups),
                range: range.take(),
                highlight: std::mem::take(highlight),
            }),
            _ => {
                if let Some(cut) = cut_buffer {
                    buffer.multi_paste(alt, matches, cut);
                }
                None
            }
        },
        key!(CONTROL, 'c') => {
            if let cut @ Some(_) = buffer.multi_cursor_copy(matches) {
                *highlight = false;
                *cut_buffer = cut;
            }
            None
        }
        key!(CONTROL, 'x') => {
            if let cut @ Some(_) = buffer.multi_cursor_cut(alt, matches) {
                *highlight = false;
                *cut_buffer = cut;
            }
            None
        }
        keybind!(WidenSelection) => {
            *highlight = false;
            buffer.multi_cursor_widen(matches);
            None
        }
        keybind!(Bookmark) => {
            *highlight = false;
            buffer.toggle_bookmarks(matches.iter().map(|m| m.cursor()));
            None
        }
        key!(Tab) => {
            let (offsets, completions) = buffer.multi_autocomplete_matches(matches)?;
            match init_complete_forward(&completions) {
                Some((index, original, replacement)) => {
                    buffer.multi_autocomplete(alt, matches, &offsets, original, replacement);
                    Some(EditorMode::AutocompleteMulti {
                        matches: std::mem::take(matches),
                        match_idx: std::mem::take(match_idx),
                        groups: std::mem::take(groups),
                        range: std::mem::take(range),
                        offsets,
                        completions,
                        index,
                    })
                }
                None => {
                    buffer.set_error("No Completions Found");
                    None
                }
            }
        }
        key!(SHIFT, BackTab) => {
            let (offsets, completions) = buffer.multi_autocomplete_matches(matches)?;
            match init_complete_backward(&completions) {
                Some((index, original, replacement)) => {
                    buffer.multi_autocomplete(alt, matches, &offsets, original, replacement);
                    Some(EditorMode::AutocompleteMulti {
                        matches: std::mem::take(matches),
                        match_idx: std::mem::take(match_idx),
                        groups: std::mem::take(groups),
                        range: std::mem::take(range),
                        offsets,
                        completions,
                        index,
                    })
                }
                None => {
                    buffer.set_error("No Completions Found");
                    None
                }
            }
        }
        Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        })
        | Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            ..
        }) => {
            *highlight = true;
            *match_idx = match_idx.checked_sub(1).unwrap_or(matches.len() - 1);
            if let Some(r) = matches.get(*match_idx) {
                buffer.set_cursor(r.cursor());
            }
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        })
        | Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            ..
        }) => {
            *highlight = true;
            *match_idx = (*match_idx + 1) % matches.len();
            if let Some(r) = matches.get(*match_idx) {
                buffer.set_cursor(r.cursor());
            }
            None
        }
        _ => None,
    }
}

fn process_paste_group(
    buf: &mut BufferContext,
    matches: &mut [MultiCursor],
    cut_buffer: Option<&mut EditorCutBuffer>,
    groups: &mut [Vec<Option<MatchCapture>>],
    event: Event,
    alt: Vec<AltCursor<'_>>,
) {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c @ '0'..='9'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => {
            let group = match c {
                '0' => 0,
                '1' => 1,
                '2' => 2,
                '3' => 3,
                '4' => 4,
                '5' => 5,
                '6' => 6,
                '7' => 7,
                '8' => 8,
                '9' => 9,
                _ => unreachable!(),
            };

            buf.multi_insert_strings(
                alt,
                matches,
                groups.iter().map(|g| match g.get(group) {
                    Some(Some(MatchCapture { string: s, .. })) => (s.chars().count(), s.as_str()),
                    Some(None) | None => (0, ""),
                }),
            );
        }
        key!(CONTROL, 'v') => {
            if let Some(cut) = cut_buffer {
                buf.multi_paste(alt, matches, cut);
            }
        }
        _ => { /* ignore other events */ }
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
        top: Box<Layout>,
        bottom: Box<Layout>,
        which: HorizontalPos,
    },
    Vertical {
        left: Box<Layout>,
        right: Box<Layout>,
        which: VerticalPos,
    },
}

impl Default for Layout {
    fn default() -> Self {
        Self::Single(BufferList::default())
    }
}

impl Layout {
    fn has_open_buffers(&self) -> bool {
        match self {
            Self::Single(b) => !b.is_empty(),
            Self::Horizontal { top: b, .. } | Self::Vertical { left: b, .. } => {
                b.has_open_buffers()
            }
        }
    }

    fn add(&mut self, path: Source) -> Result<(), ()> {
        fn add(layout: &mut Layout, ctx: BufferContext, active: bool) {
            match layout {
                Layout::Single(b) => {
                    b.push(ctx, active);
                }
                Layout::Horizontal {
                    which: HorizontalPos::Top,
                    top: current,
                    bottom: inactive,
                }
                | Layout::Horizontal {
                    which: HorizontalPos::Bottom,
                    bottom: current,
                    top: inactive,
                }
                | Layout::Vertical {
                    which: VerticalPos::Left,
                    left: current,
                    right: inactive,
                }
                | Layout::Vertical {
                    which: VerticalPos::Right,
                    right: current,
                    left: inactive,
                } => {
                    add(current, ctx.clone(), active);
                    add(inactive, ctx, false);
                }
            }
        }

        fn add_err(mut layout: &mut Layout, error: String) {
            loop {
                match layout {
                    Layout::Single(b) => {
                        if let Some(ctx) = b.current_mut() {
                            ctx.set_error(error);
                        }
                        break;
                    }
                    Layout::Horizontal {
                        which: HorizontalPos::Top,
                        top: current,
                        ..
                    }
                    | Layout::Horizontal {
                        which: HorizontalPos::Bottom,
                        bottom: current,
                        ..
                    }
                    | Layout::Vertical {
                        which: VerticalPos::Left,
                        left: current,
                        ..
                    }
                    | Layout::Vertical {
                        which: VerticalPos::Right,
                        right: current,
                        ..
                    } => {
                        layout = current;
                    }
                }
            }
        }

        self.selected_buffer_list_mut()
            .select_by_source(&path)
            .or_else(|()| match BufferContext::open(path) {
                Ok(ctx) => {
                    add(self, ctx, true);
                    Ok(())
                }
                Err(err) => {
                    add_err(self, err.to_string());
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
                x.remove(buffer.clone());
                y.remove(buffer);
            }
        }
    }

    fn selected_buffer_list(&self) -> &BufferList {
        match self {
            Self::Single(buffer) => buffer,
            Self::Horizontal {
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
            } => buffer.selected_buffer_list(),
        }
    }

    fn selected_buffer_list_mut(&mut self) -> &mut BufferList {
        match self {
            Self::Single(buffer) => buffer,
            Self::Horizontal {
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
            } => buffer.selected_buffer_list_mut(),
        }
    }

    /// Returns current buffer index
    /// mutable reference to that BufferContext
    /// and AltCursors for all other BufferContexts sharing the same Buffer
    fn current_buffer_mut(
        &mut self,
    ) -> Option<(usize, &mut crate::buffer::BufferContext, Vec<AltCursor<'_>>)> {
        match self {
            Self::Single(buffer) => Some((buffer.current_index(), buffer.current_mut()?, vec![])),
            Self::Horizontal {
                which: HorizontalPos::Top,
                top: active,
                bottom: inactive,
            }
            | Self::Horizontal {
                which: HorizontalPos::Bottom,
                bottom: active,
                top: inactive,
            }
            | Self::Vertical {
                which: VerticalPos::Left,
                left: active,
                right: inactive,
            }
            | Self::Vertical {
                which: VerticalPos::Right,
                right: active,
                left: inactive,
            } => {
                let (buffer_idx, buf, mut alts) = active.current_buffer_mut()?;
                alts.extend(inactive.alt_cursors(buffer_idx));
                Some((buffer_idx, buf, alts))
            }
        }
    }

    fn alt_cursors(&mut self, buffer_idx: usize) -> Vec<AltCursor<'_>> {
        match self {
            Self::Single(buffer) => match buffer.get_mut(buffer_idx) {
                Some(buf) => vec![buf.alt_cursor()],
                None => vec![],
            },
            Self::Horizontal {
                top: first,
                bottom: second,
                ..
            }
            | Self::Vertical {
                left: first,
                right: second,
                ..
            } => {
                let mut cursors = first.alt_cursors(buffer_idx);
                cursors.extend(second.alt_cursors(buffer_idx));
                cursors
            }
        }
    }

    fn update_current_at<F>(&mut self, f: F)
    where
        F: FnOnce(&mut crate::buffer::BufferContext, Vec<AltCursor<'_>>),
    {
        if let Some((_, buf, alts)) = self.current_buffer_mut() {
            f(buf, alts);
        }
    }

    fn on_current<T, F>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce(&mut crate::buffer::BufferContext) -> T,
    {
        self.selected_buffer_list_mut().on_buf(f)
    }

    fn on_current_at<T, F>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce(&mut crate::buffer::BufferContext, Vec<AltCursor<'_>>) -> T,
    {
        self.current_buffer_mut().map(|(_, buf, alts)| f(buf, alts))
    }

    fn previous_buffer(&mut self) {
        self.selected_buffer_list_mut().previous_buffer()
    }

    fn next_buffer(&mut self) {
        self.selected_buffer_list_mut().next_buffer()
    }

    /// Ok(()) => move performed successfully in ourself or a child
    /// Err(()) => unable to perform a move
    fn change_pane(&mut self, direction: Direction) -> Result<(), ()> {
        match (self, direction) {
            (Self::Single(_), _) => Err(()),
            (
                Self::Horizontal {
                    which: which @ HorizontalPos::Bottom,
                    bottom,
                    ..
                },
                direction @ Direction::Up,
            ) => bottom.change_pane(direction).or_else(|()| {
                *which = HorizontalPos::Top;
                Ok(())
            }),
            (
                Self::Horizontal {
                    which: which @ HorizontalPos::Top,
                    top,
                    ..
                },
                direction @ Direction::Down,
            ) => top.change_pane(direction).or_else(|()| {
                *which = HorizontalPos::Bottom;
                Ok(())
            }),
            (
                Self::Vertical {
                    which: which @ VerticalPos::Left,
                    left,
                    ..
                },
                direction @ Direction::Right,
            ) => left.change_pane(direction).or_else(|()| {
                *which = VerticalPos::Right;
                Ok(())
            }),
            (
                Self::Vertical {
                    which: which @ VerticalPos::Right,
                    right,
                    ..
                },
                direction @ Direction::Left,
            ) => right.change_pane(direction).or_else(|()| {
                *which = VerticalPos::Left;
                Ok(())
            }),
            (
                Self::Horizontal {
                    which: HorizontalPos::Bottom,
                    bottom: active,
                    ..
                }
                | Self::Horizontal {
                    which: HorizontalPos::Top,
                    top: active,
                    ..
                }
                | Self::Vertical {
                    which: VerticalPos::Left,
                    left: active,
                    ..
                }
                | Self::Vertical {
                    which: VerticalPos::Right,
                    right: active,
                    ..
                },
                direction,
            ) => active.change_pane(direction),
        }
    }

    fn split_pane(&mut self, direction: Direction) {
        let mut current = self;

        loop {
            match current {
                Self::Single(buffer) => match direction {
                    Direction::Up => {
                        *current = Self::Horizontal {
                            top: Box::new(Self::Single(buffer.clone())),
                            bottom: Box::new(Self::Single(std::mem::take(buffer))),
                            which: HorizontalPos::Top,
                        };
                        break;
                    }
                    Direction::Down => {
                        *current = Self::Horizontal {
                            top: Box::new(Self::Single(buffer.clone())),
                            bottom: Box::new(Self::Single(std::mem::take(buffer))),
                            which: HorizontalPos::Bottom,
                        };
                        break;
                    }
                    Direction::Left => {
                        *current = Self::Vertical {
                            left: Box::new(Self::Single(buffer.clone())),
                            right: Box::new(Self::Single(std::mem::take(buffer))),
                            which: VerticalPos::Left,
                        };
                        break;
                    }
                    Direction::Right => {
                        *current = Self::Vertical {
                            left: Box::new(Self::Single(buffer.clone())),
                            right: Box::new(Self::Single(std::mem::take(buffer))),
                            which: VerticalPos::Right,
                        };
                        break;
                    }
                },
                Self::Horizontal {
                    which: HorizontalPos::Top,
                    top: active,
                    ..
                }
                | Self::Horizontal {
                    which: HorizontalPos::Bottom,
                    bottom: active,
                    ..
                }
                | Self::Vertical {
                    which: VerticalPos::Left,
                    left: active,
                    ..
                }
                | Self::Vertical {
                    which: VerticalPos::Right,
                    right: active,
                    ..
                } => {
                    current = active;
                }
            }
        }
    }

    fn delete_current_pane(&mut self) {
        match self {
            Self::Single(_) => { /* don't delete last pane */ }
            Self::Horizontal {
                which: HorizontalPos::Top,
                top: active,
                bottom: remaining,
            }
            | Self::Horizontal {
                which: HorizontalPos::Bottom,
                bottom: active,
                top: remaining,
            }
            | Self::Vertical {
                which: VerticalPos::Left,
                left: active,
                right: remaining,
            }
            | Self::Vertical {
                which: VerticalPos::Right,
                right: active,
                left: remaining,
            } => {
                if matches!(&**active, Layout::Single(_)) {
                    *self = std::mem::take(remaining);
                } else {
                    active.delete_current_pane();
                }
            }
        }
    }

    fn cursor_position(&self, area: Rect, mode: &EditorMode) -> Option<Position> {
        use ratatui::layout::Constraint::{Length, Min};
        use ratatui::layout::Layout;

        // apply tabs exactly once
        let area = match self.selected_buffer_list().has_tabs() {
            true => {
                let [_, widget_area] = Layout::vertical([Length(1), Min(0)]).areas(area);
                widget_area
            }
            false => area,
        };

        self.cursor_position_inner(area, mode)
    }

    fn cursor_position_inner(&self, area: Rect, mode: &EditorMode) -> Option<Position> {
        use crate::buffer::BufferWidget;
        use ratatui::layout::Constraint::{Length, Min};
        use ratatui::layout::{Constraint, Layout};

        // generate a duplicate of our existing block layout
        // and then apply cursor's position to it
        fn apply_position(
            area: Rect,
            (row, col): (usize, usize),
            mode: &EditorMode,
        ) -> Option<Position> {
            use ratatui::widgets::Block;

            let [text_area, _] =
                Layout::horizontal([Min(0), Length(1)]).areas(Block::bordered().inner(area));

            match mode {
                // SelectLine pushes the cursor up into the title bar,
                // which is why its Y coordinate subtracts one
                EditorMode::SelectLine { .. } => Some(Position {
                    x: text_area.x + text_area.width,
                    y: text_area.y.saturating_sub(1),
                }),
                EditorMode::Search { prompt, .. }
                | EditorMode::AutocompleteSearch { prompt, .. } => {
                    let [_, dialog_area, _] =
                        Layout::vertical([Min(0), Length(3), Min(0)]).areas(text_area);
                    let dialog_area = Block::bordered().inner(dialog_area);
                    Some(Position {
                        x: dialog_area.x + (prompt.cursor_column() as u16).min(dialog_area.width),
                        y: dialog_area.y,
                    })
                }
                EditorMode::Open { chooser } => {
                    let (x, y) = chooser.cursor_position();
                    Some(Position {
                        x: text_area.x + x,
                        y: text_area.y + y,
                    })
                }
                _ => {
                    let x = (col + usize::from(text_area.x)).min(
                        (text_area.x
                            + text_area
                                .width
                                .saturating_sub(crate::buffer::BufferWidget::RIGHT_MARGIN))
                        .into(),
                    );
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
                .cursor_viewport_position(BufferWidget::viewport_height(area))
                .and_then(|pos| apply_position(area, pos, mode)),
            Self::Horizontal {
                top,
                which: HorizontalPos::Top,
                ..
            } => {
                let [top_area, _] = Layout::vertical(Constraint::from_fills([1, 1])).areas(area);

                top.cursor_position_inner(top_area, mode)
            }
            Self::Horizontal {
                bottom,
                which: HorizontalPos::Bottom,
                ..
            } => {
                let [_, bottom_area] = Layout::vertical(Constraint::from_fills([1, 1])).areas(area);

                bottom.cursor_position_inner(bottom_area, mode)
            }
            Self::Vertical {
                left,
                which: VerticalPos::Left,
                ..
            } => {
                let [left_area, _] = Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);

                left.cursor_position_inner(left_area, mode)
            }
            Self::Vertical {
                right,
                which: VerticalPos::Right,
                ..
            } => {
                let [_, right_area] =
                    Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);

                right.cursor_position_inner(right_area, mode)
            }
        }
    }

    /// The inverse of cursor_position
    ///
    /// Given an onscreen row and column, sets focus somewhere
    /// in the editor if possible.
    fn set_cursor_focus(&mut self, mut area: Rect, position: Position) {
        use ratatui::layout::{
            Constraint::{Length, Min},
            Layout,
        };

        if let Some((_, tabs)) = self.selected_buffer_list().tabs() {
            let [tabs_area, layout_area] = Layout::vertical([Length(1), Min(0)]).areas(area);
            if tabs_area.contains(position) {
                let mut col = position.x;
                for (index, tab) in tabs.into_iter().enumerate() {
                    use unicode_width::UnicodeWidthStr;

                    let tab_width = tab.width() as u16 + 2; // +2 for padding
                    if col <= tab_width {
                        self.selected_buffer_list_mut().set_index(index);
                        return;
                    } else {
                        // +1 for separator
                        col = match col.checked_sub(tab_width + 1) {
                            Some(col) => col,
                            None => return,
                        };
                    }
                }
                return;
            }
            area = layout_area;
        }

        self.set_cursor_focus_inner(area, position);
    }

    /// set_cursor_focus, but with tabs already accounted for
    fn set_cursor_focus_inner(&mut self, area: Rect, position: Position) {
        use ratatui::layout::{Constraint, Layout};

        match self {
            Self::Single(buffer) => {
                buffer.set_cursor_focus(area, position);
            }
            Self::Horizontal { top, bottom, which } => {
                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);

                if top_area.contains(position) {
                    *which = HorizontalPos::Top;
                    top.set_cursor_focus_inner(top_area, position);
                } else if bottom_area.contains(position) {
                    *which = HorizontalPos::Bottom;
                    bottom.set_cursor_focus_inner(bottom_area, position);
                }
            }
            Self::Vertical { left, right, which } => {
                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);

                if left_area.contains(position) {
                    *which = VerticalPos::Left;
                    left.set_cursor_focus_inner(left_area, position);
                } else if right_area.contains(position) {
                    *which = VerticalPos::Right;
                    right.set_cursor_focus_inner(right_area, position);
                }
            }
        }
    }
}

/// Directions for moving or splitting panes
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

struct EditorWidget<'e> {
    focused: bool,
    mode: &'e mut EditorMode,
    show_help: bool,
    show_sub_help: bool,
}

impl StatefulWidget for EditorWidget<'_> {
    type State = Layout;

    fn render(
        self,
        mut area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        layout: &mut Layout,
    ) {
        let Self {
            mode,
            show_help,
            show_sub_help,
            focused,
        } = self;

        if let Some((index, tabs)) = layout.selected_buffer_list().tabs() {
            use ratatui::{
                layout::{
                    Constraint::{Length, Min},
                    Layout,
                },
                style::Style,
                symbols,
                widgets::{Tabs, Widget},
            };

            let [tabs_area, layout_area] = Layout::vertical([Length(1), Min(0)]).areas(area);
            Tabs::new(tabs)
                .highlight_style(if self.focused {
                    Style::default().bold().underlined()
                } else {
                    Style::default()
                })
                .divider(symbols::DOT)
                .select(index)
                .render(tabs_area, buf);
            area = layout_area;
        }

        LayoutWidget {
            mode,
            show_help,
            show_sub_help,
            focused,
            multiple_panes: !matches!(layout, Layout::Single(_)),
        }
        .render(area, buf, layout)
    }
}

struct LayoutWidget<'e> {
    focused: bool,
    mode: &'e mut EditorMode,
    show_help: bool,
    show_sub_help: bool,
    multiple_panes: bool,
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

        let Self {
            mode,
            show_help,
            show_sub_help,
            focused,
            multiple_panes,
        } = self;

        match layout {
            Layout::Single(single) => {
                let multiple_buffers = single.multiple_buffers();

                if let Some(buffer) = single.current_mut() {
                    BufferWidget {
                        focused,
                        mode: Some(mode).filter(|_| focused),
                        show_help: show_help
                            .then(|| buffer.help_options(multiple_buffers, multiple_panes)),
                        show_sub_help,
                    }
                    .render(area, buf, buffer);
                }
            }
            Layout::Horizontal { which, top, bottom } => {
                use ratatui::layout::{Constraint, Layout};

                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([1, 1])).areas(area);

                (match which {
                    HorizontalPos::Top => LayoutWidget {
                        focused,
                        mode,
                        show_help,
                        show_sub_help,
                        multiple_panes,
                    },
                    HorizontalPos::Bottom => LayoutWidget {
                        focused: false,
                        mode,
                        show_help: false,
                        show_sub_help: false,
                        multiple_panes: false,
                    },
                })
                .render(top_area, buf, top);

                (match which {
                    HorizontalPos::Top => LayoutWidget {
                        focused: false,
                        mode,
                        show_help: false,
                        show_sub_help: false,
                        multiple_panes: false,
                    },
                    HorizontalPos::Bottom => LayoutWidget {
                        focused,
                        mode,
                        show_help,
                        show_sub_help,
                        multiple_panes,
                    },
                })
                .render(bottom_area, buf, bottom);
            }
            Layout::Vertical { which, left, right } => {
                use ratatui::layout::{Constraint, Layout};

                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([1, 1])).areas(area);

                (match which {
                    VerticalPos::Left => LayoutWidget {
                        focused,
                        mode,
                        show_help,
                        show_sub_help,
                        multiple_panes,
                    },
                    VerticalPos::Right => LayoutWidget {
                        focused: false,
                        mode,
                        show_help: false,
                        show_sub_help: false,
                        multiple_panes: false,
                    },
                })
                .render(left_area, buf, left);

                (match which {
                    VerticalPos::Left => LayoutWidget {
                        focused: false,
                        mode,
                        show_help: false,
                        show_sub_help: false,
                        multiple_panes: false,
                    },
                    VerticalPos::Right => LayoutWidget {
                        focused,
                        mode,
                        show_help,
                        show_sub_help,
                        multiple_panes,
                    },
                })
                .render(right_area, buf, right);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct LineNumber {
    line: usize,
    column: Option<usize>,
}

impl std::str::FromStr for LineNumber {
    type Err = InvalidLine;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((line, column)) => Ok(Self {
                line: line
                    .parse::<usize>()
                    .map_err(|_| InvalidLine)?
                    .saturating_sub(1),
                column: Some(
                    column
                        .parse::<usize>()
                        .map_err(|_| InvalidLine)?
                        .saturating_sub(1),
                ),
            }),
            None => Ok(Self {
                line: s
                    .parse::<usize>()
                    .map_err(|_| InvalidLine)?
                    .saturating_sub(1),
                column: None,
            }),
        }
    }
}

#[derive(Debug)]
pub struct InvalidLine;

impl std::error::Error for InvalidLine {}

impl std::fmt::Display for InvalidLine {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "invalid line".fmt(f)
    }
}

/// Returns (index, original, replacement)
fn init_complete_forward<T, R>(completions: &[T]) -> Option<(usize, &R, &R)>
where
    T: AsRef<R>,
    R: ?Sized,
{
    if let Some(original) = completions.first()
        && let Some(replacement) = completions.get(1)
    {
        Some((1, original.as_ref(), replacement.as_ref()))
    } else {
        None
    }
}

/// Returns current and next autocompletion, and increments index
/// completions.len() must be > 0
fn complete_forward<'c, T, R>(index: &mut usize, completions: &'c mut [T]) -> (&'c R, &'c R)
where
    T: AsRef<R>,
    R: ?Sized,
{
    let next_index = (*index + 1) % completions.len();
    (
        completions[std::mem::replace(index, next_index)].as_ref(),
        completions[next_index].as_ref(),
    )
}

/// Returns (index, original, replacement)
fn init_complete_backward<T, R>(completions: &[T]) -> Option<(usize, &R, &R)>
where
    T: AsRef<R>,
    R: ?Sized,
{
    if let Some(original) = completions.first()
        && let Some(index) = completions.len().checked_sub(1)
        && index != 0
        && let Some(replacement) = completions.get(index)
    {
        Some((index, original.as_ref(), replacement.as_ref()))
    } else {
        None
    }
}

/// Returns current and next autocompletion, and increments index
/// completions.len() must be > 0
fn complete_backward<'c, T, R>(index: &mut usize, completions: &'c mut [T]) -> (&'c R, &'c R)
where
    T: AsRef<R>,
    R: ?Sized,
{
    let previous_index = index.checked_sub(1).unwrap_or(completions.len() - 1);
    (
        completions[std::mem::replace(index, previous_index)].as_ref(),
        completions[previous_index].as_ref(),
    )
}
