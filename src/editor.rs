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
        AltCursor, BufferContext, BufferId, BufferList, EditorCutBuffer, MultiCursor,
        SelectionRange, Source,
    },
    files::{ChooserSource, FileChooserState, LocalSource},
    key::{Binding, CtrlBinding},
    prompt::{LinePrompt, TextField},
};
use crossterm::event::Event;
use ratatui::{
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::LazyLock;

static PAGE_SIZE: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("VLE_PAGE_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .map(|s| s.clamp(1, 100))
        .unwrap_or(25)
});

type DirMap = fn(Direction) -> Option<&'static [&'static str]>;

// External terminal multiplexer integration
static MULTIPLEXER: LazyLock<DirMap> = LazyLock::new(|| {
    if std::env::var("ZELLIJ").is_ok() {
        |direction| {
            Some(match direction {
                Direction::Up => &["zellij", "action", "move-focus", "up"],
                Direction::Down => &["zellij", "action", "move-focus", "down"],
                Direction::Left => &["zellij", "action", "move-focus", "left"],
                Direction::Right => &["zellij", "action", "move-focus", "right"],
            })
        }
    } else if std::env::var("TMUX").is_ok() {
        |direction| {
            Some(match direction {
                Direction::Up => &["tmux", "select-pane", "-U"],
                Direction::Down => &["tmux", "select-pane", "-D"],
                Direction::Left => &["tmux", "select-pane", "-L"],
                Direction::Right => &["tmux", "select-pane", "-R"],
            })
        }
    } else {
        |_| None
    }
});

#[derive(Default)]
pub enum EditorMode {
    /// Regular editing mode
    #[default]
    Editing,
    /// Making a selection
    MarkSet,
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
    SearchAll {
        prompt: TextField,
        type_: SearchType,
    },
    /// Multi-cursor operation
    MultiCursor {
        matches: Vec<MultiCursor>,
        match_idx: usize,
        range: Option<SelectionRange>,
        highlight: bool,
    },
    /// Multi-cursor operation with mark set
    MultiCursorMarkSet {
        matches: Vec<MultiCursor>,
        match_idx: usize,
        range: Option<SelectionRange>,
        highlight: bool,
    },
    /// Multi-cursor operation across multiple buffers
    MultiCursorAll {
        // a buffer index -> cursor matches mapping
        matches: BTreeMap<usize, Vec<MultiCursor>>,
        // which match is active in the currently active buffer
        match_idx: usize,
        highlight: bool,
    },
    /// Multi-cursor operation with mark set
    MultiCursorMarkSetAll {
        matches: BTreeMap<usize, Vec<MultiCursor>>,
        match_idx: usize,
        highlight: bool,
    },
    /// Querying for what regex group to paste
    PasteGroup {
        matches: Vec<MultiCursor>,
        match_idx: usize,
        total: usize,
        range: Option<SelectionRange>,
        highlight: bool,
    },
    /// Querying for what regex group to paste in all buffers
    PasteGroupAll {
        matches: BTreeMap<usize, Vec<MultiCursor>>,
        match_idx: usize,
        total: usize,
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
    /// Performing autocomplete during a search
    AutocompleteSearch {
        prompt: TextField,
        type_: SearchType,
        range: Option<SelectionRange>,
        offset: usize,            // our character offset in prompt
        completions: Vec<String>, // autocompletion candidates
        index: usize,             // the current candidate
    },
    /// Performing autocomplete during a search all
    AutocompleteSearchAll {
        prompt: TextField,
        type_: SearchType,
        offset: usize,            // our character offset in prompt
        completions: Vec<String>, // autocompletion candidates
        index: usize,             // the current candidate
    },
    /// Performing autocomplete in a multi-cursor context
    AutocompleteMulti {
        matches: Vec<MultiCursor>,
        match_idx: usize,
        range: Option<SelectionRange>,
        offsets: Vec<usize>,      // autocompletion offsets
        completions: Vec<String>, // autocompletion candidates
        index: usize,             // current autocompletion candidate
    },
    /// Performing autocomplete in multi-cursor mode across multiple buffers
    AutocompleteMultiAll {
        matches: BTreeMap<usize, Vec<MultiCursor>>,
        match_idx: usize,
        offsets: BTreeMap<usize, Vec<usize>>, // autocompletion offsets
        completions: Vec<String>,             // autocompletion candidates
        index: usize,                         // current candidate
    },
    /// Determining what buffer to select from menu
    SelectBuffer {
        buffer_list: Vec<BufferId>, // buffers
        index: usize,               // buffer index to select
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

macro_rules! ctrl_keybind {
    ($bind:ident) => {
        Event::Key(KeyEvent {
            code: key::$bind::KEY,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            ..
        })
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
                    .cursor_position(area, self.focused.then_some(&self.mode))
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
                EditorMode::MarkSet => {
                    if let Some(event) = self.process_mark_set_event(event) {
                        // end mark set
                        self.mode = EditorMode::default();
                        self.process_normal_event(area, event);
                    }
                }
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
                EditorMode::AutocompleteSearchAll {
                    prompt,
                    type_,
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
                        self.mode = EditorMode::SearchAll {
                            prompt: std::mem::take(prompt),
                            type_: std::mem::take(type_),
                        };
                        self.process_event(area, event);
                    }
                },
                EditorMode::AutocompleteMulti {
                    matches,
                    match_idx,
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
                            range: std::mem::take(range),
                            highlight: false,
                        };
                        self.process_event(area, event);
                    }
                },
                EditorMode::AutocompleteMultiAll {
                    matches,
                    match_idx,
                    offsets,
                    completions,
                    index,
                } => match event {
                    key!(Tab) => {
                        // switch to next candidate
                        let (current, next) = complete_forward(index, completions);
                        on_all_offset_at(
                            &mut self.layout,
                            matches,
                            offsets,
                            |b, a, matches, offsets| {
                                b.multi_autocomplete(a, matches, offsets, current, next);
                            },
                        );
                    }
                    key!(SHIFT, BackTab) => {
                        // switch to previous candidate
                        let (current, previous) = complete_backward(index, completions);
                        on_all_offset_at(
                            &mut self.layout,
                            matches,
                            offsets,
                            |b, a, matches, offsets| {
                                b.multi_autocomplete(a, matches, offsets, current, previous);
                            },
                        );
                    }
                    event => {
                        // end autocomplete
                        self.mode = EditorMode::MultiCursorAll {
                            matches: std::mem::take(matches),
                            match_idx: std::mem::take(match_idx),
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
                            self.cut_buffer.as_mut(),
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
                                // I think these are unnecessary
                                // buf.set_cursor(matches[match_idx].0.end);
                                // buf.clear_selection();

                                // TODO - try to avoid this re-mapping
                                let matches = matches.into_iter().map(|r| r.into()).collect();

                                EditorMode::MultiCursor {
                                    matches,
                                    match_idx,
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
                EditorMode::SearchAll { prompt, type_ } => {
                    if let Some(new_mode) = process_search_all(
                        self.layout.selected_buffer_list_mut(),
                        self.cut_buffer.as_mut(),
                        match type_ {
                            SearchType::Plain => &mut self.last_plain_search,
                            SearchType::Regex => &mut self.last_regex_search,
                        },
                        prompt,
                        type_,
                        event,
                    ) {
                        self.mode = match new_mode {
                            NextModeIncrementalAll::Browse { match_idx, matches } => {
                                EditorMode::MultiCursorAll {
                                    match_idx,
                                    matches,
                                    highlight: true,
                                }
                            }
                            NextModeIncrementalAll::SelectLine => EditorMode::SelectLine {
                                prompt: LinePrompt::default(),
                            },
                            NextModeIncrementalAll::Autocomplete {
                                offset,
                                completions,
                                index,
                            } => EditorMode::AutocompleteSearchAll {
                                prompt: std::mem::take(prompt),
                                type_: std::mem::take(type_),
                                offset,
                                completions,
                                index,
                            },
                        };
                    }
                }
                EditorMode::MultiCursor {
                    matches,
                    match_idx,
                    range,
                    highlight,
                } => {
                    if let Some(Some(new_mode)) = self.layout.on_current_at(|b, a| {
                        process_multi_cursor(
                            b,
                            &mut self.cut_buffer,
                            matches,
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
                EditorMode::MultiCursorMarkSet {
                    matches,
                    match_idx,
                    range,
                    highlight,
                } => {
                    match self
                        .layout
                        .on_current(|b| process_multi_cursor_mark_set(b, matches, highlight, event))
                    {
                        Some(Ok(Some(event))) => {
                            // end mark set
                            self.mode = EditorMode::MultiCursor {
                                matches: std::mem::take(matches),
                                match_idx: std::mem::take(match_idx),
                                range: std::mem::take(range),
                                highlight: std::mem::take(highlight),
                            };
                            self.process_event(area, event);
                        }
                        Some(Err(())) => {
                            // end mark set
                            self.mode = EditorMode::MultiCursor {
                                matches: std::mem::take(matches),
                                match_idx: std::mem::take(match_idx),
                                range: std::mem::take(range),
                                highlight: std::mem::take(highlight),
                            };
                        }
                        _ => { /* do nothing */ }
                    }
                }
                EditorMode::MultiCursorMarkSetAll {
                    matches,
                    match_idx,
                    highlight,
                } => {
                    match process_multi_cursor_mark_set_all(
                        &mut self.layout,
                        matches,
                        highlight,
                        event,
                    ) {
                        Ok(Some(event)) => {
                            // end mark set
                            self.mode = EditorMode::MultiCursorAll {
                                matches: std::mem::take(matches),
                                match_idx: std::mem::take(match_idx),
                                highlight: std::mem::take(highlight),
                            };
                            self.process_event(area, event);
                        }
                        Err(()) => {
                            // end mark set
                            self.mode = EditorMode::MultiCursorAll {
                                matches: std::mem::take(matches),
                                match_idx: std::mem::take(match_idx),
                                highlight: std::mem::take(highlight),
                            };
                        }
                        _ => { /* do nothing */ }
                    }
                }
                EditorMode::MultiCursorAll {
                    matches,
                    match_idx,
                    highlight,
                } => {
                    if let Some(new_mode) = process_multi_cursor_all(
                        &mut self.layout,
                        &mut self.cut_buffer,
                        matches,
                        match_idx,
                        highlight,
                        event,
                    ) {
                        self.mode = new_mode;
                    }
                }
                EditorMode::PasteGroup {
                    matches,
                    match_idx,
                    range,
                    highlight,
                    ..
                } => {
                    self.layout.update_current_at(|b, a| {
                        process_paste_group(b, matches, self.cut_buffer.as_mut(), event, a);
                    });

                    self.mode = EditorMode::MultiCursor {
                        matches: std::mem::take(matches),
                        match_idx: std::mem::take(match_idx),
                        range: range.take(),
                        highlight: std::mem::take(highlight),
                    };
                }
                EditorMode::PasteGroupAll {
                    matches,
                    match_idx,
                    highlight,
                    ..
                } => {
                    process_paste_group_all(
                        &mut self.layout,
                        matches,
                        self.cut_buffer.as_mut(),
                        event,
                    );

                    self.mode = EditorMode::MultiCursorAll {
                        matches: std::mem::take(matches),
                        match_idx: std::mem::take(match_idx),
                        highlight: std::mem::take(highlight),
                    };
                }
                EditorMode::SplitPane => self.process_split_pane(event),
                EditorMode::SelectBuffer { buffer_list, index } => {
                    match process_select_buffer(
                        self.layout.selected_buffer_list_mut(),
                        index,
                        event,
                    ) {
                        Some(SelectBuffer::Finish) => {
                            self.mode = EditorMode::default();
                        }
                        Some(SelectBuffer::SwapPanes(idx_a, idx_b)) => {
                            buffer_list.swap(idx_a, idx_b);
                            self.layout.swap_buffers(idx_a, idx_b);
                        }
                        Some(SelectBuffer::SaveAll) => {
                            if let Err(crate::buffer::Modified) =
                                self.layout.selected_buffer_list_mut().save_all()
                            {
                                self.mode = EditorMode::VerifySave;
                            }
                            self.layout.on_current(|b| set_title(b));
                            self.mode = EditorMode::default();
                        }
                        Some(SelectBuffer::ReloadAll) => {
                            let (list, mut alts) = self.layout.current_buffer_list_mut();
                            if let Err(crate::buffer::Modified) = list.reload_all(&mut alts) {
                                self.mode = EditorMode::VerifyReload
                            }
                            self.layout.on_current(|b| set_title(b));
                            self.mode = EditorMode::default();
                        }
                        Some(SelectBuffer::QuitAll) => {
                            while let Some(buf) = self.layout.selected_buffer_list().current() {
                                if buf.modified() {
                                    set_title(buf);
                                    self.mode = EditorMode::ConfirmClose { buffer: buf.id() };
                                    break;
                                } else {
                                    self.layout.remove(buf.id());
                                }
                            }
                            self.mode = EditorMode::default();
                        }
                        Some(SelectBuffer::FindAll) => {
                            self.mode = EditorMode::SearchAll {
                                prompt: TextField::default(),
                                type_: SearchType::default(),
                            };
                        }
                        None => { /* do nothing */ }
                    }
                }
            },
        }
    }

    fn process_normal_event(&mut self, area: Rect, event: Event) {
        use crate::buffer::SelectionType;
        use crossterm::event::{
            Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
            MouseEventKind,
        };
        use std::process::Command;

        match event {
            keybind!(Quit) => {
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    if buf.modified() {
                        self.mode = EditorMode::ConfirmClose { buffer: buf.id() };
                    } else {
                        self.layout.remove(buf.id());
                        if let Some(buf) = self.layout.selected_buffer_list().current() {
                            set_title(buf);
                        }
                    }
                }
            }
            key!(CONTROL, PageUp) => {
                if let Some(buf) = self.layout.previous_buffer() {
                    set_title(buf);
                }
            }
            key!(CONTROL, PageDown) => {
                if let Some(buf) = self.layout.next_buffer() {
                    set_title(buf);
                }
            }
            keybind!(SplitPane) => {
                self.mode = EditorMode::SplitPane;
            }
            Event::Key(KeyEvent {
                code:
                    code @ KeyCode::Left
                    | code @ KeyCode::Right
                    | code @ KeyCode::Up
                    | code @ KeyCode::Down,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                match self.layout.change_pane(match code {
                    KeyCode::Left => Direction::Left,
                    KeyCode::Right => Direction::Right,
                    KeyCode::Up => Direction::Up,
                    KeyCode::Down => Direction::Down,
                    _ => unreachable!(),
                }) {
                    Ok(Some(buf)) => {
                        set_title(buf);
                    }
                    Ok(None) => { /* do nothing */ }
                    Err(dir) => {
                        if let Some([cmd, args @ ..]) = MULTIPLEXER(dir)
                            && let Err(err) = Command::new(cmd).args(args).output()
                        {
                            self.layout
                                .update_current_at(|buf, _| buf.set_error(err.to_string()));
                        }
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
            ctrl_keybind!(Cut) => self.perform_cut(),
            ctrl_keybind!(Copy) => self.perform_copy(),
            ctrl_keybind!(Paste) => {
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
            ctrl_keybind!(Undo) => {
                let _ = self
                    .layout
                    .on_all(|b| b.perform_undo_active(), |b| b.perform_undo_inactive());
            }
            ctrl_keybind!(Redo) => {
                let _ = self
                    .layout
                    .on_all(|b| b.perform_redo_active(), |b| b.perform_redo_inactive());
            }
            keybind!(Save) => {
                // if save fails, we'll already be in normal mode
                // to display the save failure message
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
                if let Some(Err(())) = self.on_buffer(|b| b.try_select_inside()) {
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
                        use crate::buffer::Normalizations;

                        (match Normalizations::try_from(selection) {
                            Ok(normalizations) => {
                                b.all_matches(None, normalizations).map_err(|_| ())
                            }
                            Err(selection) => b.all_matches(None, selection).map_err(|_| ()),
                        })
                        .map(|(match_idx, matches)| {
                            b.set_cursor(matches[match_idx].0.end);
                            b.clear_selection();

                            let matches = matches.into_iter().map(|r| r.into()).collect();

                            EditorMode::MultiCursor {
                                matches,
                                match_idx,
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
                if let Some(Err(crate::buffer::Modified)) =
                    self.on_buffer_at(|b, a| b.verified_reload(a))
                {
                    self.mode = EditorMode::VerifyReload;
                }
            }
            keybind!(UpdateLines) | key!(CONTROL, 'r') => {
                if let Some(matches) = self.on_buffer(|b| b.selection_cursors())
                    && let Some(match_idx) = matches.len().checked_sub(1)
                {
                    self.mode = EditorMode::MultiCursor {
                        matches,
                        match_idx,
                        range: None,
                        highlight: false,
                    };
                }
            }
            ctrl_keybind!(Mark) => {
                self.mode = EditorMode::MarkSet;
            }
            key!(CONTROL, '5') => {
                let buffer_list = self.layout.selected_buffer_list();
                let index = buffer_list.current_index();
                let buffer_list = buffer_list.buffers().map(|b| b.id()).collect();
                self.mode = EditorMode::SelectBuffer { buffer_list, index };
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
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    set_title(buf);
                }
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Right),
                column,
                row,
                ..
            }) => {
                self.layout
                    .set_cursor_focus(area, Position { y: row, x: column });
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    set_title(buf);
                }
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
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    set_title(buf);
                }
                self.layout.update_current_at(|b, a| {
                    b.paste(a, &mut self.cut_buffer);
                });
            }
            _ => { /* ignore other events */ }
        }
    }

    pub fn process_mark_set_event(&mut self, event: Event) -> Option<Event> {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_up(1, true));
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_down(1, true));
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| {
                    b.cursor_up(*PAGE_SIZE, true);
                });
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| {
                    b.cursor_down(*PAGE_SIZE, true);
                });
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_back(true));
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_forward(true));
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_home(true));
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.update_buffer(|b| b.cursor_end(true));
                None
            }
            ctrl_keybind!(Mark) => {
                self.mode = EditorMode::default();
                None
            }
            event => Some(event),
        }
    }

    fn process_confirm_close(&mut self, event: Event, buffer_id: BufferId) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

        match event {
            key!('y') => {
                // close buffer anyway
                self.layout.remove(buffer_id);
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    set_title(buf);
                }
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
                self.update_buffer(|b| {
                    // buffer already updated with error message
                    // in case save doesn't succeed
                    let _ = b.save();
                });
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
                self.update_buffer_at(|b, a| {
                    // on failure, buffer already populated with error
                    let _ = b.reload(a);
                });
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
        use std::process::Command;

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
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    set_title(buf);
                }
                self.mode = EditorMode::default();
            }
            key!(CONTROL, Delete) => {
                self.layout.delete_other_panes();
                if let Some(buf) = self.layout.selected_buffer_list().current() {
                    set_title(buf);
                }
                self.mode = EditorMode::default();
            }
            key!(SHIFT, Left) => {
                let _ = self.layout.swap_pane(Direction::Left);
            }
            key!(SHIFT, Right) => {
                let _ = self.layout.swap_pane(Direction::Right);
            }
            key!(SHIFT, Up) => {
                let _ = self.layout.swap_pane(Direction::Up);
            }
            key!(SHIFT, Down) => {
                let _ = self.layout.swap_pane(Direction::Down);
            }
            Event::Key(KeyEvent {
                code:
                    code @ KeyCode::Left
                    | code @ KeyCode::Right
                    | code @ KeyCode::Up
                    | code @ KeyCode::Down,
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                match self.layout.change_pane(match code {
                    KeyCode::Left => Direction::Left,
                    KeyCode::Right => Direction::Right,
                    KeyCode::Up => Direction::Up,
                    KeyCode::Down => Direction::Down,
                    _ => unreachable!(),
                }) {
                    Ok(Some(buf)) => {
                        set_title(buf);
                    }
                    Ok(None) => { /* do nothing */ }
                    Err(dir) => {
                        if let Some([cmd, args @ ..]) = MULTIPLEXER(dir)
                            && let Err(err) = Command::new(cmd).args(args).output()
                        {
                            self.layout
                                .update_current_at(|buf, _| buf.set_error(err.to_string()));
                        }
                    }
                }
            }
            key!('+') => {
                self.layout.update_ratio(|ours, theirs, buf| {
                    *ours = (*ours + 1).clamp(1, 10);
                    buf.set_message(format!("Ratio {ours}:{theirs}"));
                });
            }
            key!('-') => {
                self.layout.update_ratio(|ours, theirs, buf| {
                    *ours = ours.saturating_sub(1).clamp(1, 10);
                    buf.set_message(format!("Ratio {ours}:{theirs}"));
                });
            }
            key!(Enter) => {
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

    pub fn auto_save(&mut self) -> bool {
        if matches!(self.mode, EditorMode::Editing) {
            self.layout
                .selected_buffer_list_mut()
                .buffers_mut()
                .fold(false, |saved, buf| {
                    if buf.modified() {
                        matches!(buf.verified_save(), Ok(Ok(()))) | saved
                    } else {
                        saved
                    }
                })
        } else {
            false
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
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
    };

    match event {
        key!(Up)
        | Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            ..
        }) => {
            chooser.arrow_up();
            None
        }
        key!(Down)
        | Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            ..
        }) => {
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
            if let Some(buf) = layout.selected_buffer_list().current() {
                set_title(buf);
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
        matches: Vec<(Range<usize>, Vec<String>)>,
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
    cut_buffer: Option<&mut EditorCutBuffer>,
    last_search: &mut Option<TextField>,
    prompt: &mut TextField,
    type_: &mut SearchType,
    range: Option<&SelectionRange>,
    event: Event,
) -> Option<NextModeIncremental> {
    use crate::buffer::Normalizations;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    static NOT_FOUND: &str = "Not Found";

    match event {
        ctrl_keybind!(Paste) => {
            let c = cut_buffer?;
            let b = c.paste_and_rotate();
            if b.multi_line() {
                match Normalizations::try_from(b.as_str().to_string()) {
                    Err(term) => match buffer.all_multiline_matches(range, term) {
                        Ok((match_idx, matches)) => {
                            *last_search = Some(std::mem::take(prompt));
                            Some(NextModeIncremental::Browse { match_idx, matches })
                        }
                        Err(_) => {
                            buffer.set_error(NOT_FOUND);
                            None
                        }
                    },
                    Ok(normalizations) => match buffer.all_multiline_matches(range, normalizations)
                    {
                        Ok((match_idx, matches)) => {
                            *last_search = Some(std::mem::take(prompt));
                            Some(NextModeIncremental::Browse { match_idx, matches })
                        }
                        Err(_) => {
                            buffer.set_error(NOT_FOUND);
                            None
                        }
                    },
                }
            } else {
                prompt.paste(c.paste_and_rotate().as_str());
                None
            }
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
            SearchType::Plain => match Normalizations::try_from(prompt.value()?) {
                Err(term) => match buffer.all_matches(range, term) {
                    Ok((match_idx, matches)) => {
                        *last_search = Some(std::mem::take(prompt));
                        Some(NextModeIncremental::Browse { match_idx, matches })
                    }
                    Err(_) => {
                        buffer.set_error(NOT_FOUND);
                        None
                    }
                },
                Ok(normalizations) => match buffer.all_matches(range, normalizations) {
                    Ok((match_idx, matches)) => {
                        *last_search = Some(std::mem::take(prompt));
                        Some(NextModeIncremental::Browse { match_idx, matches })
                    }
                    Err(_) => {
                        buffer.set_error(NOT_FOUND);
                        None
                    }
                },
            },
            SearchType::Regex => match prompt.value()?.parse::<fancy_regex::Regex>() {
                Ok(regex) => match buffer.all_matches(range, regex) {
                    Ok((match_idx, matches)) => {
                        *last_search = Some(std::mem::take(prompt));
                        Some(NextModeIncremental::Browse { match_idx, matches })
                    }
                    Err(_) => {
                        buffer.set_error(NOT_FOUND);
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

enum NextModeIncrementalAll {
    Browse {
        match_idx: usize,
        matches: BTreeMap<usize, Vec<MultiCursor>>,
    },
    SelectLine,
    Autocomplete {
        offset: usize,
        completions: Vec<String>,
        index: usize,
    },
}

fn process_search_all(
    buffer_list: &mut BufferList,
    cut_buffer: Option<&mut EditorCutBuffer>,
    last_search: &mut Option<TextField>,
    prompt: &mut TextField,
    type_: &mut SearchType,
    event: Event,
) -> Option<NextModeIncrementalAll> {
    use crate::buffer::Normalizations;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    static NOT_FOUND: &str = "Not Found";

    match event {
        ctrl_keybind!(Paste) => {
            let c = cut_buffer?;
            let b = c.paste_and_rotate();
            if b.multi_line() {
                match Normalizations::try_from(b.as_str().to_string()) {
                    Err(term) => match buffer_list.all_multiline_matches(term) {
                        Ok((match_idx, matches)) => {
                            *last_search = Some(std::mem::take(prompt));
                            Some(NextModeIncrementalAll::Browse { match_idx, matches })
                        }
                        Err(_) => {
                            buffer_list.current_mut()?.set_error(NOT_FOUND);
                            None
                        }
                    },
                    Ok(normalizations) => match buffer_list.all_multiline_matches(normalizations) {
                        Ok((match_idx, matches)) => {
                            *last_search = Some(std::mem::take(prompt));
                            Some(NextModeIncrementalAll::Browse { match_idx, matches })
                        }
                        Err(_) => {
                            buffer_list.current_mut()?.set_error(NOT_FOUND);
                            None
                        }
                    },
                }
            } else {
                prompt.paste(c.paste_and_rotate().as_str());
                None
            }
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
                let completions = buffer_list.search_autocomplete_matches(search);
                match init_complete_forward(&completions) {
                    Some((index, original, replacement)) => {
                        prompt.autocomplete(offset, original, replacement);
                        Some(NextModeIncrementalAll::Autocomplete {
                            offset,
                            completions,
                            index,
                        })
                    }
                    None => {
                        buffer_list.current_mut()?.set_error("No Completions Found");
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
                let completions = buffer_list.search_autocomplete_matches(search);
                match init_complete_backward(&completions) {
                    Some((index, original, replacement)) => {
                        prompt.autocomplete(offset, original, replacement);
                        Some(NextModeIncrementalAll::Autocomplete {
                            offset,
                            completions,
                            index,
                        })
                    }
                    None => {
                        buffer_list.current_mut()?.set_error("No Completions Found");
                        None
                    }
                }
            }
        }
        key!(Enter) => match type_ {
            SearchType::Plain => match Normalizations::try_from(prompt.value()?) {
                Err(term) => match buffer_list.all_matches(term) {
                    Ok((match_idx, matches)) => {
                        *last_search = Some(std::mem::take(prompt));
                        Some(NextModeIncrementalAll::Browse { match_idx, matches })
                    }
                    Err(_) => {
                        buffer_list.current_mut()?.set_error(NOT_FOUND);
                        None
                    }
                },
                Ok(normalizations) => match buffer_list.all_matches(normalizations) {
                    Ok((match_idx, matches)) => {
                        *last_search = Some(std::mem::take(prompt));
                        Some(NextModeIncrementalAll::Browse { match_idx, matches })
                    }
                    Err(_) => {
                        buffer_list.current_mut()?.set_error(NOT_FOUND);
                        None
                    }
                },
            },
            SearchType::Regex => match prompt.value()?.parse::<fancy_regex::Regex>() {
                Ok(regex) => match buffer_list.all_matches(regex) {
                    Ok((match_idx, matches)) => {
                        *last_search = Some(std::mem::take(prompt));
                        Some(NextModeIncrementalAll::Browse { match_idx, matches })
                    }
                    Err(_) => {
                        buffer_list.current_mut()?.set_error(NOT_FOUND);
                        None
                    }
                },
                Err(err) => {
                    buffer_list.current_mut()?.set_error(err.to_string());
                    None
                }
            },
        },
        keybind!(GotoLine) => Some(NextModeIncrementalAll::SelectLine),
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
        keybind!(SelectInside) => {
            *highlight = false;
            buffer.multi_select_inside(matches, *match_idx);
            None
        }
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
        ctrl_keybind!(Paste) => match matches.iter().map(|m| m.paste_group_count()).max() {
            Some(Some(total)) => Some(EditorMode::PasteGroup {
                total: total.get(),
                matches: std::mem::take(matches),
                match_idx: std::mem::take(match_idx),
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
        ctrl_keybind!(Copy) => {
            let cut = buffer.multi_cursor_copy(matches);
            match cut.len() {
                0 => { /* do nothing */ }
                1 => {
                    buffer.set_message("Copied 1 Item");
                    *highlight = false;
                    *cut_buffer = Some(EditorCutBuffer::Multiple(cut));
                }
                n => {
                    buffer.set_message(format!("Copied {n} Items"));
                    *highlight = false;
                    *cut_buffer = Some(EditorCutBuffer::Multiple(cut));
                }
            }
            None
        }
        ctrl_keybind!(Cut) => {
            let cut = buffer.multi_cursor_cut(alt, matches);
            match cut.len() {
                0 => { /* do nothing */ }
                1 => {
                    buffer.set_message("Cut 1 Item");
                    *highlight = false;
                    *cut_buffer = Some(EditorCutBuffer::Multiple(cut));
                }
                n => {
                    buffer.set_message(format!("Cut {n} Items"));
                    *highlight = false;
                    *cut_buffer = Some(EditorCutBuffer::Multiple(cut));
                }
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
        ctrl_keybind!(Mark) => Some(EditorMode::MultiCursorMarkSet {
            matches: std::mem::take(matches),
            match_idx: std::mem::take(match_idx),
            range: std::mem::take(range),
            highlight: std::mem::take(highlight),
        }),
        _ => None,
    }
}

fn process_multi_cursor_mark_set(
    buffer: &mut BufferContext,
    matches: &mut [MultiCursor],
    highlight: &mut bool,
    event: Event,
) -> Result<Option<Event>, ()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_cursor_back(matches, true);
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_cursor_forward(matches, true);
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_cursor_home(matches, true);
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            buffer.multi_cursor_end(matches, true);
            Ok(None)
        }
        ctrl_keybind!(Mark) => Err(()),
        event => Ok(Some(event)),
    }
}

fn process_multi_cursor_all(
    layout: &mut Layout,
    cut_buffer: &mut Option<EditorCutBuffer>,
    matches: &mut BTreeMap<usize, Vec<MultiCursor>>,
    match_idx: &mut usize,
    highlight: &mut bool,
    event: Event,
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
            on_all_at(layout, matches, |buffer, alt, matches| {
                buffer.multi_insert_char(alt, matches, c);
            });
            None
        }
        Event::Paste(pasted) => {
            *highlight = false;
            on_all_at(layout, matches, |buffer, alt, matches| {
                buffer.multi_insert_string(alt, matches, &pasted);
            });
            None
        }
        key!(Backspace) => {
            *highlight = false;
            on_all_at(layout, matches, |buffer, alt, matches| {
                buffer.multi_backspace(alt, matches);
            });
            None
        }
        key!(Delete) => {
            *highlight = false;
            on_all_at(layout, matches, |buffer, alt, matches| {
                buffer.multi_delete(alt, matches);
            });
            None
        }
        key!(CONTROL, Delete) => {
            use std::collections::btree_map::Entry;

            *highlight = true;
            let buffer_list = layout.selected_buffer_list_mut();
            let buffer_index = buffer_list.current_index();
            let Entry::Occupied(mut buffer_matches) = matches.entry(buffer_index) else {
                return None;
            };
            buffer_matches.get_mut().remove(*match_idx);
            match buffer_matches.get().len().checked_sub(1) {
                Some(max) => {
                    *match_idx = (*match_idx).min(max);
                    buffer_list
                        .get_mut(buffer_index)?
                        .set_cursor(buffer_matches.get().get(*match_idx)?.cursor());
                    None
                }
                None => {
                    use core::ops::Bound;

                    buffer_matches.remove();
                    if let Some((next_idx, next_cursors)) = matches
                        .range((Bound::Excluded(buffer_index), Bound::Unbounded))
                        .next()
                        .or_else(|| matches.first_key_value())
                        && let Some(r) = next_cursors.first()
                        && let Ok(buffer) = buffer_list.select_buffer(*next_idx)
                    {
                        *match_idx = 0;
                        buffer.set_cursor(r.cursor());
                        None
                    } else {
                        // no next buffer to switch to
                        Some(EditorMode::default())
                    }
                }
            }
        }
        keybind!(Find) => Some(EditorMode::SearchAll {
            prompt: TextField::default(),
            type_: SearchType::default(),
        }),
        keybind!(SelectInside) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_select_inside(matches, *match_idx);
            });
            None
        }
        key!(Enter) => Some(EditorMode::default()),
        Event::Key(KeyEvent {
            code: KeyCode::Left,
            modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_back(matches, modifiers.contains(KeyModifiers::SHIFT));
            });
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_forward(matches, modifiers.contains(KeyModifiers::SHIFT));
            });
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::Home,
            modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_home(matches, modifiers.contains(KeyModifiers::SHIFT));
            });
            None
        }
        Event::Key(KeyEvent {
            code: KeyCode::End,
            modifiers: modifiers @ KeyModifiers::NONE | modifiers @ KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_end(matches, modifiers.contains(KeyModifiers::SHIFT));
            });
            None
        }
        ctrl_keybind!(Paste) => match matches
            .values()
            .flat_map(|m| m.iter().map(|m| m.paste_group_count()))
            .max()
        {
            Some(Some(total)) => Some(EditorMode::PasteGroupAll {
                total: total.get(),
                matches: std::mem::take(matches),
                match_idx: std::mem::take(match_idx),
                highlight: std::mem::take(highlight),
            }),
            _ => {
                if let Some(cut) = cut_buffer {
                    on_all_at(layout, matches, |buffer, alt, matches| {
                        buffer.multi_paste(alt, matches, cut);
                    });
                }
                None
            }
        },
        ctrl_keybind!(Copy) => {
            let mut cut = vec![];
            on_all(layout, matches, |buffer, matches| {
                cut.extend(buffer.multi_cursor_copy(matches));
            });
            match cut.len() {
                0 => { /* do nothing */ }
                1 => {
                    *highlight = false;
                    *cut_buffer = Some(EditorCutBuffer::Multiple(cut));
                    layout
                        .selected_buffer_list_mut()
                        .current_mut()?
                        .set_message("Copied 1 Item");
                }
                n => {
                    *highlight = false;
                    *cut_buffer = Some(EditorCutBuffer::Multiple(cut));
                    layout
                        .selected_buffer_list_mut()
                        .current_mut()?
                        .set_message(format!("Copied {n} Items"));
                }
            }
            None
        }
        ctrl_keybind!(Cut) => {
            let mut cut = vec![];
            on_all_at(layout, matches, |buffer, alt, matches| {
                cut.extend(buffer.multi_cursor_cut(alt, matches));
            });
            match cut.len() {
                0 => { /* do nothing */ }
                1 => {
                    *highlight = false;
                    *cut_buffer = Some(EditorCutBuffer::Multiple(cut));
                    layout
                        .selected_buffer_list_mut()
                        .current_mut()?
                        .set_message("Cut 1 Item");
                }
                n => {
                    *highlight = false;
                    *cut_buffer = Some(EditorCutBuffer::Multiple(cut));
                    layout
                        .selected_buffer_list_mut()
                        .current_mut()?
                        .set_message(format!("Cut {n} Items"));
                }
            }
            None
        }
        keybind!(WidenSelection) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_widen(matches);
            });
            None
        }
        keybind!(Bookmark) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.toggle_bookmarks(matches.iter().map(|m| m.cursor()));
            });
            None
        }
        key!(Tab) => {
            let buffer_list = layout.selected_buffer_list_mut();
            let (offsets, completions) = buffer_list.multi_autocomplete_matches(matches)?;
            match init_complete_forward(&completions) {
                Some((index, original, replacement)) => {
                    on_all_offset_at(
                        layout,
                        matches,
                        &offsets,
                        |buffer, alt, matches, offsets| {
                            buffer.multi_autocomplete(alt, matches, offsets, original, replacement);
                        },
                    );
                    Some(EditorMode::AutocompleteMultiAll {
                        matches: std::mem::take(matches),
                        match_idx: std::mem::take(match_idx),
                        offsets,
                        completions,
                        index,
                    })
                }
                None => {
                    buffer_list.current_mut()?.set_error("No Completions Found");
                    None
                }
            }
        }
        key!(SHIFT, BackTab) => {
            let buffer_list = layout.selected_buffer_list_mut();
            let (offsets, completions) = buffer_list.multi_autocomplete_matches(matches)?;
            match init_complete_backward(&completions) {
                Some((index, original, replacement)) => {
                    on_all_offset_at(
                        layout,
                        matches,
                        &offsets,
                        |buffer, alt, matches, offsets| {
                            buffer.multi_autocomplete(alt, matches, offsets, original, replacement);
                        },
                    );
                    Some(EditorMode::AutocompleteMultiAll {
                        matches: std::mem::take(matches),
                        match_idx: std::mem::take(match_idx),
                        offsets,
                        completions,
                        index,
                    })
                }
                None => {
                    buffer_list.current_mut()?.set_error("No Completions Found");
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
            let buffer_list = layout.selected_buffer_list_mut();
            let buffer_index = buffer_list.current_index();
            if let Some(buffer_cursors) = matches.get(&buffer_index) {
                *highlight = true;
                match match_idx.checked_sub(1) {
                    Some(new_idx) => {
                        // move on to previous index in the set of matches
                        *match_idx = new_idx;
                        if let Some(r) = buffer_cursors.get(new_idx)
                            && let Some(buffer) = buffer_list.get_mut(buffer_index)
                        {
                            buffer.set_cursor(r.cursor());
                        }
                    }
                    None => {
                        use core::ops::Bound;

                        // move on to the last index of the previous buffer's matches
                        if let Some((prev_idx, prev_cursors)) = matches
                            .range((Bound::Unbounded, Bound::Excluded(buffer_index)))
                            .next_back()
                            .or_else(|| matches.last_key_value())
                            && let Some(r) = prev_cursors.last()
                            && let Ok(buffer) = buffer_list.select_buffer(*prev_idx)
                        {
                            *match_idx = prev_cursors.len() - 1;
                            buffer.set_cursor(r.cursor());
                        }
                    }
                }
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
            let buffer_list = layout.selected_buffer_list_mut();
            let buffer_index = buffer_list.current_index();
            if let Some(buffer_cursors) = matches.get(&buffer_index) {
                *highlight = true;
                if (*match_idx + 1) < buffer_cursors.len() {
                    // move on to the next index in the set of matches
                    *match_idx += 1;
                    if let Some(r) = buffer_cursors.get(*match_idx)
                        && let Some(buffer) = buffer_list.get_mut(buffer_index)
                    {
                        buffer.set_cursor(r.cursor());
                    }
                } else {
                    use core::ops::Bound;

                    // move to the first index of the next buffer's matches
                    if let Some((next_idx, next_cursors)) = matches
                        .range((Bound::Excluded(buffer_index), Bound::Unbounded))
                        .next()
                        .or_else(|| matches.first_key_value())
                        && let Some(r) = next_cursors.first()
                        && let Ok(buffer) = buffer_list.select_buffer(*next_idx)
                    {
                        *match_idx = 0;
                        buffer.set_cursor(r.cursor());
                    }
                }
            }
            None
        }
        ctrl_keybind!(Mark) => Some(EditorMode::MultiCursorMarkSetAll {
            matches: std::mem::take(matches),
            match_idx: std::mem::take(match_idx),
            highlight: std::mem::take(highlight),
        }),
        _ => None,
    }
}

fn process_multi_cursor_mark_set_all(
    layout: &mut Layout,
    matches: &mut BTreeMap<usize, Vec<MultiCursor>>,
    highlight: &mut bool,
    event: Event,
) -> Result<Option<Event>, ()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_back(matches, true);
            });
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_forward(matches, true);
            });
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_home(matches, true);
            });
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => {
            *highlight = false;
            on_all(layout, matches, |buffer, matches| {
                buffer.multi_cursor_end(matches, true);
            });
            Ok(None)
        }
        ctrl_keybind!(Mark) => Err(()),
        event => Ok(Some(event)),
    }
}

fn process_paste_group(
    buf: &mut BufferContext,
    matches: &mut [MultiCursor],
    cut_buffer: Option<&mut EditorCutBuffer>,
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

            buf.multi_insert_group(alt, matches, group);
        }
        ctrl_keybind!(Paste) => {
            if let Some(cut) = cut_buffer {
                buf.multi_paste(alt, matches, cut);
            }
        }
        _ => { /* ignore other events */ }
    }
}

fn process_paste_group_all(
    layout: &mut Layout,
    matches: &mut BTreeMap<usize, Vec<MultiCursor>>,
    cut_buffer: Option<&mut EditorCutBuffer>,
    event: Event,
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

            on_all_at(layout, matches, |buf, alt, matches| {
                buf.multi_insert_group(alt, matches, group);
            });
        }
        ctrl_keybind!(Paste) => {
            if let Some(cut) = cut_buffer {
                on_all_at(layout, matches, |buf, alt, matches| {
                    buf.multi_paste(alt, matches, cut);
                });
            }
        }
        _ => { /* ignore other events */ }
    }
}

enum SelectBuffer {
    Finish,
    SwapPanes(usize, usize),
    SaveAll,
    FindAll,
    ReloadAll,
    QuitAll,
}

// None                  - index moved from one buffer to another or no-op
// Some(Ok(mode))        - buffer in buffer list set to index, selection complete
// Some(Err((idx, idx))) - swap buffers with the given indexes, continue selection
fn process_select_buffer(
    buffer_list: &mut BufferList,
    index: &mut usize,
    event: Event,
) -> Option<SelectBuffer> {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
    };

    const PAGE_SIZE: usize = 5;

    fn char_to_index(c: char) -> Option<usize> {
        match c {
            c @ '1'..='9' => Some((u32::from(c) - u32::from('1')) as usize),
            '0' => Some(9),
            c @ 'a'..='z' => Some((u32::from(c) - u32::from('a')) as usize + 10),
            c @ 'A'..='Z' => Some((u32::from(c) - u32::from('A')) as usize + 10),
            _ => None,
        }
    }

    match event {
        key!(Up)
        | Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            ..
        }) => match index.checked_sub(1) {
            Some(new_index) => {
                *index = new_index;
                None
            }
            None => {
                if let Some(new_index) = buffer_list.len().checked_sub(1) {
                    *index = new_index;
                }
                None
            }
        },
        key!(Down)
        | Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            ..
        }) => {
            *index = (*index + 1) % buffer_list.len();
            None
        }
        key!(CONTROL, Up) => match index.checked_sub(1) {
            Some(new_index) => Some(SelectBuffer::SwapPanes(
                std::mem::replace(index, new_index),
                new_index,
            )),
            None => {
                let new_index = buffer_list.len().checked_sub(1)?;
                Some(SelectBuffer::SwapPanes(
                    std::mem::replace(index, new_index),
                    new_index,
                ))
            }
        },
        key!(CONTROL, Down) => {
            let new_index = (*index + 1) % buffer_list.len();
            Some(SelectBuffer::SwapPanes(
                std::mem::replace(index, new_index),
                new_index,
            ))
        }
        key!(PageDown) => {
            *index = (*index + PAGE_SIZE).min(buffer_list.len().saturating_sub(1));
            None
        }
        key!(PageUp) => {
            *index = index.saturating_sub(PAGE_SIZE);
            None
        }
        key!(Home) => {
            *index = 0;
            None
        }
        key!(End) => {
            if let Some(max) = buffer_list.len().checked_sub(1) {
                *index = max;
            }
            None
        }
        key!(Enter) => buffer_list
            .select_buffer(*index)
            .ok()
            .map(|_| SelectBuffer::Finish),
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            ..
        }) => buffer_list
            .select_buffer(char_to_index(c)?)
            .ok()
            .map(|_| SelectBuffer::Finish),
        keybind!(Save) => Some(SelectBuffer::SaveAll),
        keybind!(Find) => Some(SelectBuffer::FindAll),
        keybind!(Reload) => Some(SelectBuffer::ReloadAll),
        keybind!(Quit) => Some(SelectBuffer::QuitAll),
        _ => None, // ignore other events
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
        top_fill: u16,
        bottom: Box<Layout>,
        bottom_fill: u16,
        which: HorizontalPos,
    },
    Vertical {
        left: Box<Layout>,
        left_fill: u16,
        right: Box<Layout>,
        right_fill: u16,
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
                    ..
                }
                | Layout::Horizontal {
                    which: HorizontalPos::Bottom,
                    bottom: current,
                    top: inactive,
                    ..
                }
                | Layout::Vertical {
                    which: VerticalPos::Left,
                    left: current,
                    right: inactive,
                    ..
                }
                | Layout::Vertical {
                    which: VerticalPos::Right,
                    right: current,
                    left: inactive,
                    ..
                } => {
                    add(current, ctx.clone(), active);
                    add(inactive, ctx, false);
                }
            }
        }

        fn add_err(layout: &mut Layout, error: String) {
            if let Some(ctx) = layout.selected_buffer_list_mut().current_mut() {
                ctx.set_error(error);
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

    /// Swaps buffers at the given indexes across all BufferLists
    fn swap_buffers(&mut self, a: usize, b: usize) {
        match self {
            Self::Single(buffer) => {
                buffer.swap_buffers(a, b);
            }
            Self::Horizontal {
                top: buf_a,
                bottom: buf_b,
                ..
            }
            | Self::Vertical {
                left: buf_a,
                right: buf_b,
                ..
            } => {
                buf_a.swap_buffers(a, b);
                buf_b.swap_buffers(a, b);
            }
        }
    }

    fn selected_buffer_list(&self) -> &BufferList {
        let mut current = self;
        loop {
            match current {
                Self::Single(buffer) => break buffer,
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
                } => {
                    current = buffer;
                }
            }
        }
    }

    fn selected_buffer_list_mut(&mut self) -> &mut BufferList {
        let mut current = self;

        loop {
            match current {
                Self::Single(buffer) => break buffer,
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
                } => {
                    current = buffer;
                }
            }
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
                ..
            }
            | Self::Horizontal {
                which: HorizontalPos::Bottom,
                bottom: active,
                top: inactive,
                ..
            }
            | Self::Vertical {
                which: VerticalPos::Left,
                left: active,
                right: inactive,
                ..
            }
            | Self::Vertical {
                which: VerticalPos::Right,
                right: active,
                left: inactive,
                ..
            } => {
                let (buffer_idx, buf, alts) = active.current_buffer_mut()?;
                Some((
                    buffer_idx,
                    buf,
                    concat_vec(alts, inactive.alt_cursors(buffer_idx)),
                ))
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
            } => concat_vec(
                first.alt_cursors(buffer_idx),
                second.alt_cursors(buffer_idx),
            ),
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

    fn on_all<F, G>(&mut self, on_active: F, on_rest: G) -> Result<usize, ()>
    where
        F: FnOnce(&mut BufferContext) -> Result<(), ()>,
        G: Fn(&mut BufferContext) + Copy,
    {
        match self {
            Self::Single(b) => {
                let index = b.current_index();
                on_active(b.get_mut(index).ok_or(())?).map(|()| index)
            }
            Self::Horizontal {
                which: HorizontalPos::Top,
                top: active,
                bottom: inactive,
                ..
            }
            | Self::Horizontal {
                which: HorizontalPos::Bottom,
                bottom: active,
                top: inactive,
                ..
            }
            | Self::Vertical {
                which: VerticalPos::Left,
                left: active,
                right: inactive,
                ..
            }
            | Self::Vertical {
                which: VerticalPos::Right,
                right: active,
                left: inactive,
                ..
            } => {
                let index = active.on_all(on_active, on_rest)?;
                inactive.on_rest(index, on_rest);
                Ok(index)
            }
        }
    }

    fn on_rest<F>(&mut self, index: usize, on_rest: F)
    where
        F: Fn(&mut BufferContext) + Copy,
    {
        match self {
            Self::Single(b) => {
                if let Some(buf) = b.get_mut(index) {
                    on_rest(buf);
                }
            }
            Self::Horizontal {
                top: a, bottom: b, ..
            }
            | Self::Vertical {
                left: a, right: b, ..
            } => {
                a.on_rest(index, on_rest);
                b.on_rest(index, on_rest);
            }
        }
    }

    /// Returns currently active buffer_list and all alt buffer_lists
    fn current_buffer_list_mut(
        &mut self,
    ) -> (
        &mut crate::buffer::BufferList,
        Vec<&mut crate::buffer::BufferList>,
    ) {
        match self {
            Self::Single(buffer_list) => (buffer_list, vec![]),
            Self::Horizontal {
                which: HorizontalPos::Top,
                top: active,
                bottom: inactive,
                ..
            }
            | Self::Horizontal {
                which: HorizontalPos::Bottom,
                bottom: active,
                top: inactive,
                ..
            }
            | Self::Vertical {
                which: VerticalPos::Left,
                left: active,
                right: inactive,
                ..
            }
            | Self::Vertical {
                which: VerticalPos::Right,
                right: active,
                left: inactive,
                ..
            } => {
                let (buffer_list, mut alts) = active.current_buffer_list_mut();
                alts.extend(inactive.alt_buffer_lists());
                (buffer_list, alts)
            }
        }
    }

    fn alt_buffer_lists(&mut self) -> Vec<&mut crate::buffer::BufferList> {
        match self {
            Self::Single(buffer_list) => vec![buffer_list],
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
                let mut buffer_lists = first.alt_buffer_lists();
                buffer_lists.extend(second.alt_buffer_lists());
                buffer_lists
            }
        }
    }

    /// Returns newly selected buffer
    fn previous_buffer(&mut self) -> Option<&BufferContext> {
        let buf_list = self.selected_buffer_list_mut();
        buf_list.previous_buffer();
        buf_list.current()
    }

    /// Returns newly selected buffer
    fn next_buffer(&mut self) -> Option<&BufferContext> {
        let buf_list = self.selected_buffer_list_mut();
        buf_list.next_buffer();
        buf_list.current()
    }

    /// Ok(BufferContext) => move performed successfully in ourself or a child
    /// Err(direction) => unable to perform a move
    fn change_pane(&mut self, direction: Direction) -> Result<Option<&BufferContext>, Direction> {
        match (self, direction) {
            (Self::Single(_), direction) => Err(direction),
            (
                Self::Horizontal {
                    which: which @ HorizontalPos::Bottom,
                    bottom,
                    top,
                    ..
                },
                direction @ Direction::Up,
            ) => bottom.change_pane(direction).or_else(|_| {
                *which = HorizontalPos::Top;
                Ok(top.selected_buffer_list().current())
            }),
            (
                Self::Horizontal {
                    which: which @ HorizontalPos::Top,
                    top,
                    bottom,
                    ..
                },
                direction @ Direction::Down,
            ) => top.change_pane(direction).or_else(|_| {
                *which = HorizontalPos::Bottom;
                Ok(bottom.selected_buffer_list().current())
            }),
            (
                Self::Vertical {
                    which: which @ VerticalPos::Left,
                    left,
                    right,
                    ..
                },
                direction @ Direction::Right,
            ) => left.change_pane(direction).or_else(|_| {
                *which = VerticalPos::Right;
                Ok(right.selected_buffer_list().current())
            }),
            (
                Self::Vertical {
                    which: which @ VerticalPos::Right,
                    right,
                    left,
                    ..
                },
                direction @ Direction::Left,
            ) => right.change_pane(direction).or_else(|_| {
                *which = VerticalPos::Left;
                Ok(left.selected_buffer_list().current())
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

    fn swap_pane(&mut self, direction: Direction) -> Result<(), &mut BufferList> {
        match (self, direction) {
            (Self::Single(buflist), _) => Err(buflist),
            (
                Self::Horizontal {
                    which: which @ HorizontalPos::Bottom,
                    bottom,
                    top,
                    ..
                },
                direction @ Direction::Up,
            ) => bottom.swap_pane(direction).or_else(|buflist| {
                *which = HorizontalPos::Top;
                std::mem::swap(top.selected_buffer_list_mut(), buflist);
                Ok(())
            }),
            (
                Self::Horizontal {
                    which: which @ HorizontalPos::Top,
                    top,
                    bottom,
                    ..
                },
                direction @ Direction::Down,
            ) => top.swap_pane(direction).or_else(|buflist| {
                *which = HorizontalPos::Bottom;
                std::mem::swap(bottom.selected_buffer_list_mut(), buflist);
                Ok(())
            }),
            (
                Self::Vertical {
                    which: which @ VerticalPos::Left,
                    left,
                    right,
                    ..
                },
                direction @ Direction::Right,
            ) => left.swap_pane(direction).or_else(|buflist| {
                *which = VerticalPos::Right;
                std::mem::swap(right.selected_buffer_list_mut(), buflist);
                Ok(())
            }),
            (
                Self::Vertical {
                    which: which @ VerticalPos::Right,
                    right,
                    left,
                    ..
                },
                direction @ Direction::Left,
            ) => right.swap_pane(direction).or_else(|buflist| {
                *which = VerticalPos::Left;
                std::mem::swap(left.selected_buffer_list_mut(), buflist);
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
            ) => active.swap_pane(direction),
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
                            top_fill: 1,
                            bottom_fill: 1,
                        };
                        break;
                    }
                    Direction::Down => {
                        *current = Self::Horizontal {
                            top: Box::new(Self::Single(buffer.clone())),
                            bottom: Box::new(Self::Single(std::mem::take(buffer))),
                            which: HorizontalPos::Bottom,
                            top_fill: 1,
                            bottom_fill: 1,
                        };
                        break;
                    }
                    Direction::Left => {
                        *current = Self::Vertical {
                            left: Box::new(Self::Single(buffer.clone())),
                            right: Box::new(Self::Single(std::mem::take(buffer))),
                            which: VerticalPos::Left,
                            left_fill: 1,
                            right_fill: 1,
                        };
                        break;
                    }
                    Direction::Right => {
                        *current = Self::Vertical {
                            left: Box::new(Self::Single(buffer.clone())),
                            right: Box::new(Self::Single(std::mem::take(buffer))),
                            which: VerticalPos::Right,
                            left_fill: 1,
                            right_fill: 1,
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
                ..
            }
            | Self::Horizontal {
                which: HorizontalPos::Bottom,
                bottom: active,
                top: remaining,
                ..
            }
            | Self::Vertical {
                which: VerticalPos::Left,
                left: active,
                right: remaining,
                ..
            }
            | Self::Vertical {
                which: VerticalPos::Right,
                right: active,
                left: remaining,
                ..
            } => {
                if matches!(&**active, Layout::Single(_)) {
                    *self = std::mem::take(remaining);
                } else {
                    active.delete_current_pane();
                }
            }
        }
    }

    fn delete_other_panes(&mut self) {
        let current = std::mem::take(self).into_selected_buffer_list();
        *self = Layout::Single(current);
    }

    fn into_selected_buffer_list(self) -> BufferList {
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
            } => buffer.into_selected_buffer_list(),
        }
    }

    fn cursor_position(&self, area: Rect, mode: Option<&EditorMode>) -> Option<Position> {
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

    fn cursor_position_inner(&self, area: Rect, mode: Option<&EditorMode>) -> Option<Position> {
        use crate::buffer::BufferWidget;
        use ratatui::layout::Constraint::{Length, Min};
        use ratatui::layout::{Constraint, Layout};

        // generate a duplicate of our existing block layout
        // and then apply cursor's position to it
        fn apply_position(
            area: Rect,
            (row, col): (usize, usize),
            mode: Option<&EditorMode>,
        ) -> Option<Position> {
            use ratatui::widgets::Block;

            let [text_area, _] =
                Layout::horizontal([Min(0), Length(1)]).areas(Block::bordered().inner(area));

            match mode {
                // SelectLine pushes the cursor up into the title bar,
                // which is why its Y coordinate subtracts one
                Some(EditorMode::SelectLine { .. }) => Some(Position {
                    x: text_area.x + text_area.width,
                    y: text_area.y.saturating_sub(1),
                }),
                Some(
                    EditorMode::Search { prompt, .. }
                    | EditorMode::AutocompleteSearch { prompt, .. }
                    | EditorMode::SearchAll { prompt, .. }
                    | EditorMode::AutocompleteSearchAll { prompt, .. },
                ) => {
                    let [_, dialog_area, _] =
                        Layout::vertical([Min(0), Length(3), Min(0)]).areas(text_area);
                    let dialog_area = Block::bordered().inner(dialog_area);
                    Some(Position {
                        x: dialog_area.x + (prompt.cursor_column() as u16).min(dialog_area.width),
                        y: dialog_area.y,
                    })
                }
                Some(EditorMode::Open { chooser }) => {
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
                top_fill,
                bottom_fill,
                ..
            } => {
                let [top_area, _] =
                    Layout::vertical(Constraint::from_fills([*top_fill, *bottom_fill])).areas(area);

                top.cursor_position_inner(top_area, mode)
            }
            Self::Horizontal {
                bottom,
                which: HorizontalPos::Bottom,
                top_fill,
                bottom_fill,
                ..
            } => {
                let [_, bottom_area] =
                    Layout::vertical(Constraint::from_fills([*top_fill, *bottom_fill])).areas(area);

                bottom.cursor_position_inner(bottom_area, mode)
            }
            Self::Vertical {
                left,
                which: VerticalPos::Left,
                left_fill,
                right_fill,
                ..
            } => {
                let [left_area, _] =
                    Layout::horizontal(Constraint::from_fills([*left_fill, *right_fill]))
                        .areas(area);

                left.cursor_position_inner(left_area, mode)
            }
            Self::Vertical {
                right,
                which: VerticalPos::Right,
                left_fill,
                right_fill,
                ..
            } => {
                let [_, right_area] =
                    Layout::horizontal(Constraint::from_fills([*left_fill, *right_fill]))
                        .areas(area);

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
            Self::Horizontal {
                top,
                bottom,
                which,
                top_fill,
                bottom_fill,
            } => {
                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([*top_fill, *bottom_fill])).areas(area);

                if top_area.contains(position) {
                    *which = HorizontalPos::Top;
                    top.set_cursor_focus_inner(top_area, position);
                } else if bottom_area.contains(position) {
                    *which = HorizontalPos::Bottom;
                    bottom.set_cursor_focus_inner(bottom_area, position);
                }
            }
            Self::Vertical {
                left,
                right,
                which,
                left_fill,
                right_fill,
            } => {
                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([*left_fill, *right_fill]))
                        .areas(area);

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

    fn update_ratio<F>(&mut self, f: F)
    where
        F: FnOnce(&mut u16, u16, &mut BufferContext),
    {
        let mut current = self;

        loop {
            match current {
                Self::Single(_) => break,
                Self::Horizontal {
                    which: HorizontalPos::Top,
                    top: selected,
                    top_fill: selected_fill,
                    bottom_fill: other_fill,
                    ..
                }
                | Self::Horizontal {
                    which: HorizontalPos::Bottom,
                    bottom: selected,
                    bottom_fill: selected_fill,
                    top_fill: other_fill,
                    ..
                }
                | Self::Vertical {
                    which: VerticalPos::Left,
                    left: selected,
                    left_fill: selected_fill,
                    right_fill: other_fill,
                    ..
                }
                | Self::Vertical {
                    which: VerticalPos::Right,
                    right: selected,
                    right_fill: selected_fill,
                    left_fill: other_fill,
                    ..
                } => {
                    if let Self::Single(buflist) = &mut **selected {
                        if let Some(ctx) = buflist.current_mut() {
                            f(selected_fill, *other_fill, ctx);
                        }
                        break;
                    } else {
                        current = selected;
                    }
                }
            }
        }
    }
}

/// Directions for moving or splitting panes
#[derive(Eq, PartialEq, Hash)]
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
                let buffer_idx = single.current_index();

                if let Some(buffer) = single.current_mut() {
                    BufferWidget {
                        focused,
                        mode: Some(mode).filter(|_| focused),
                        show_help: show_help
                            .then(|| buffer.help_options(multiple_buffers, multiple_panes)),
                        show_sub_help,
                        buffer_idx,
                    }
                    .render(area, buf, buffer);
                }
            }
            Layout::Horizontal {
                which,
                top,
                bottom,
                top_fill,
                bottom_fill,
            } => {
                use ratatui::layout::{Constraint, Layout};

                let [top_area, bottom_area] =
                    Layout::vertical(Constraint::from_fills([*top_fill, *bottom_fill])).areas(area);

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
            Layout::Vertical {
                which,
                left,
                right,
                left_fill,
                right_fill,
            } => {
                use ratatui::layout::{Constraint, Layout};

                let [left_area, right_area] =
                    Layout::horizontal(Constraint::from_fills([*left_fill, *right_fill]))
                        .areas(area);

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
fn complete_forward<'c, T, R>(index: &mut usize, completions: &'c [T]) -> (&'c R, &'c R)
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
fn complete_backward<'c, T, R>(index: &mut usize, completions: &'c [T]) -> (&'c R, &'c R)
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

fn on_all(
    layout: &mut Layout,
    matches: &mut BTreeMap<usize, Vec<MultiCursor>>,
    mut f: impl FnMut(&mut BufferContext, &mut [MultiCursor]),
) {
    let buffer_list = layout.selected_buffer_list_mut();

    matches.iter_mut().for_each(|(idx, matches)| {
        if let Some(buf) = buffer_list.get_mut(*idx) {
            f(buf, matches);
        }
    });
}

fn on_all_at(
    layout: &mut Layout,
    matches: &mut BTreeMap<usize, Vec<MultiCursor>>,
    mut f: impl FnMut(&mut BufferContext, Vec<AltCursor<'_>>, &mut [MultiCursor]),
) {
    let (buffer_list, mut alts) = layout.current_buffer_list_mut();

    matches.iter_mut().for_each(|(idx, matches)| {
        if let Some(buf) = buffer_list.get_mut(*idx) {
            f(
                buf,
                alts.iter_mut()
                    .filter_map(|a| a.get_mut(*idx).map(|b| b.alt_cursor()))
                    .collect(),
                matches,
            );
        }
    });
}

fn on_all_offset_at(
    layout: &mut Layout,
    matches: &mut BTreeMap<usize, Vec<MultiCursor>>,
    offsets: &BTreeMap<usize, Vec<usize>>,
    mut f: impl FnMut(&mut BufferContext, Vec<AltCursor<'_>>, &mut [MultiCursor], &[usize]),
) {
    let (buffer_list, mut alts) = layout.current_buffer_list_mut();

    matches.iter_mut().for_each(|(idx, matches)| {
        if let Some(buf) = buffer_list.get_mut(*idx)
            && let Some(offsets) = offsets.get(idx)
        {
            f(
                buf,
                alts.iter_mut()
                    .filter_map(|a| a.get_mut(*idx).map(|b| b.alt_cursor()))
                    .collect(),
                matches,
                offsets,
            );
        }
    })
}

fn set_title<D: std::fmt::Display>(d: D) {
    use crossterm::{execute, terminal::SetTitle};

    let _ = execute!(std::io::stdout(), SetTitle(format!("vle {d}")),);
}

fn concat_vec<T>(mut a: Vec<T>, b: Vec<T>) -> Vec<T> {
    if a.is_empty() {
        b
    } else {
        a.extend(b);
        a
    }
}
