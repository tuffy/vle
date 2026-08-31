// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::buffer::{CaseInsensitiveNormalizations, SearchTerm, Source};
use crate::editor::{DirTarget, LastSearch, PasteContents, RemoteError, SearchType};
use crate::prompt::TextField;
use ratatui::widgets::StatefulWidget;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Width of text box, in characters
const TEXT_WIDTH: u16 = 30;

/// Size of each page, in rows
const PAGE_SIZE: usize = 10;

pub trait ChooserSource: Clone + std::fmt::Display {
    type Error: std::fmt::Display;

    fn current_dir(&self) -> Result<PathBuf, Self::Error>;

    fn read_dir(
        &self,
        scratch_buffers: &[PathBuf],
        dir: &Path,
        show_hidden: bool,
    ) -> Result<Vec<Entry>, Self::Error>;

    fn open(&mut self, path: PathBuf) -> Source;

    fn target(&self) -> DirTarget;

    /// Returns new target, if any
    fn toggle_source(&mut self) -> DirTarget {
        // nothing else to switch to by default
        self.target()
    }
}

#[derive(Clone, Default)]
pub struct LocalSource;

impl std::fmt::Display for LocalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Local Files".fmt(f)
    }
}

impl ChooserSource for LocalSource {
    type Error = std::io::Error;

    fn current_dir(&self) -> std::io::Result<PathBuf> {
        std::env::current_dir()
    }

    fn read_dir(
        &self,
        _scratch_buffers: &[PathBuf],
        dir: &Path,
        show_hidden: bool,
    ) -> Result<Vec<Entry>, Self::Error> {
        dir.read_dir()
            .and_then(|entries| {
                entries
                    .map(|e| e.and_then(Entry::try_from))
                    .filter_map(|e| {
                        if show_hidden {
                            Some(e)
                        } else {
                            match e {
                                Ok(e) if e.is_hidden() => None,
                                Ok(e) => Some(Ok(e)),
                                Err(e) => Some(Err(e)),
                            }
                        }
                    })
                    .collect()
            })
            .map(|mut entries: Vec<Entry>| {
                entries.sort_unstable_by(|x, y| {
                    x.is_dir.cmp(&y.is_dir).reverse().then(x.path.cmp(&y.path))
                });
                entries
            })
    }

    fn open(&mut self, path: PathBuf) -> Source {
        Source::Local(path)
    }

    fn target(&self) -> DirTarget {
        DirTarget::Local
    }
}

#[derive(Clone, Default)]
pub struct ScratchSource;

impl std::fmt::Display for ScratchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Scratch Files".fmt(f)
    }
}

impl ChooserSource for ScratchSource {
    type Error = std::convert::Infallible;

    fn current_dir(&self) -> Result<PathBuf, Self::Error> {
        Ok(PathBuf::from("<SCRATCH>"))
    }

    fn read_dir(
        &self,
        scratch_buffers: &[PathBuf],
        _dir: &Path,
        _show_hidden: bool,
    ) -> Result<Vec<Entry>, Self::Error> {
        Ok(scratch_buffers
            .iter()
            .map(|pb| Entry {
                name: pb
                    .file_name()
                    .map(|n| n.display().to_string())
                    .unwrap_or_default(),
                path: pb.clone(),
                is_dir: false,
            })
            .collect())
    }

    fn open(&mut self, path: PathBuf) -> Source {
        Source::Scratch {
            path,
            data: "\n".into(),
        }
    }

    fn target(&self) -> DirTarget {
        DirTarget::Scratch
    }
}

#[cfg(feature = "ssh")]
#[derive(Clone)]
pub struct SshSource {
    label: String,
    remote: std::rc::Rc<ssh2::Sftp>,
}

#[cfg(feature = "ssh")]
impl SshSource {
    pub fn open(label: String, remote: std::rc::Rc<ssh2::Sftp>) -> Self {
        Self { label, remote }
    }
}

#[cfg(feature = "ssh")]
impl std::fmt::Display for SshSource {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.label.fmt(f)
    }
}

#[cfg(feature = "ssh")]
impl ChooserSource for SshSource {
    type Error = ssh2::Error;

    fn current_dir(&self) -> Result<PathBuf, Self::Error> {
        self.remote.realpath(Path::new("."))
    }

    fn read_dir(
        &self,
        _scratch_buffers: &[PathBuf],
        dir: &Path,
        show_hidden: bool,
    ) -> Result<Vec<Entry>, Self::Error> {
        self.remote
            .readdir(dir)
            .map(|entries| {
                entries
                    .into_iter()
                    .map(|(pb, _)| {
                        Entry::from((
                            self.remote.stat(&pb).map(|s| s.is_dir()).unwrap_or(false),
                            pb,
                        ))
                    })
                    .filter_map(|e| (show_hidden || !e.is_hidden()).then_some(e))
                    .collect()
            })
            .map(|mut entries: Vec<Entry>| {
                entries.sort_unstable_by(|x, y| {
                    x.is_dir.cmp(&y.is_dir).reverse().then(x.path.cmp(&y.path))
                });
                entries
            })
    }

    fn open(&mut self, path: PathBuf) -> Source {
        Source::Ssh {
            sftp: std::rc::Rc::clone(&self.remote),
            path,
        }
    }

    fn target(&self) -> DirTarget {
        DirTarget::Ssh
    }
}

#[derive(Copy, Clone)]
pub enum LocalTarget {
    Local,
    Scratch,
}

impl LocalTarget {
    /// Toggles state and returns new state
    fn toggle(&mut self) -> Self {
        match self {
            Self::Local => {
                *self = Self::Scratch;
                Self::Scratch
            }
            Self::Scratch => {
                *self = Self::Local;
                Self::Local
            }
        }
    }
}

impl From<LocalTarget> for DirTarget {
    fn from(target: LocalTarget) -> Self {
        match target {
            LocalTarget::Local => DirTarget::Local,
            LocalTarget::Scratch => DirTarget::Scratch,
        }
    }
}

#[derive(Clone)]
pub enum MultiSource {
    /// No SSH connection specified, only local or scratch files possible
    Local {
        local: LocalSource,
        scratch: ScratchSource,
        active: LocalTarget,
    },
    /// Remote, local or scratch files are all possible
    #[cfg(feature = "ssh")]
    Ssh {
        local: LocalSource,
        scratch: ScratchSource,
        ssh: SshSource,
        active: DirTarget,
    },
}

impl MultiSource {
    pub fn local() -> Self {
        Self::Local {
            local: LocalSource,
            scratch: ScratchSource,
            active: LocalTarget::Local,
        }
    }

    #[cfg(feature = "ssh")]
    pub fn ssh(ssh: SshSource, active: DirTarget) -> Self {
        Self::Ssh {
            local: LocalSource,
            scratch: ScratchSource,
            ssh,
            active,
        }
    }
}

impl std::fmt::Display for MultiSource {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Local {
                local,
                active: LocalTarget::Local,
                ..
            } => local.fmt(f),
            Self::Local {
                scratch,
                active: LocalTarget::Scratch,
                ..
            } => scratch.fmt(f),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                local,
                active: DirTarget::Local,
                ..
            } => local.fmt(f),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                scratch,
                active: DirTarget::Scratch,
                ..
            } => scratch.fmt(f),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                ssh,
                active: DirTarget::Ssh,
                ..
            } => ssh.fmt(f),
        }
    }
}

impl ChooserSource for MultiSource {
    type Error = RemoteError;

    fn current_dir(&self) -> Result<PathBuf, Self::Error> {
        match self {
            Self::Local {
                local,
                active: LocalTarget::Local,
                ..
            } => local.current_dir().map_err(RemoteError::Io),
            Self::Local {
                scratch,
                active: LocalTarget::Scratch,
                ..
            } => Ok(scratch.current_dir().unwrap()),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                local,
                active: DirTarget::Local,
                ..
            } => local.current_dir().map_err(RemoteError::Io),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                scratch,
                active: DirTarget::Scratch,
                ..
            } => Ok(scratch.current_dir().unwrap()),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                ssh,
                active: DirTarget::Ssh,
                ..
            } => ssh.current_dir().map_err(RemoteError::Ssh),
        }
    }

    fn read_dir(
        &self,
        scratch_buffers: &[PathBuf],
        dir: &Path,
        show_hidden: bool,
    ) -> Result<Vec<Entry>, Self::Error> {
        match self {
            Self::Local {
                local,
                active: LocalTarget::Local,
                ..
            } => local
                .read_dir(scratch_buffers, dir, show_hidden)
                .map_err(RemoteError::Io),
            Self::Local {
                scratch,
                active: LocalTarget::Scratch,
                ..
            } => Ok(scratch.read_dir(scratch_buffers, dir, show_hidden).unwrap()),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                local,
                active: DirTarget::Local,
                ..
            } => local
                .read_dir(scratch_buffers, dir, show_hidden)
                .map_err(RemoteError::Io),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                scratch,
                active: DirTarget::Scratch,
                ..
            } => Ok(scratch.read_dir(scratch_buffers, dir, show_hidden).unwrap()),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                ssh,
                active: DirTarget::Ssh,
                ..
            } => ssh
                .read_dir(scratch_buffers, dir, show_hidden)
                .map_err(RemoteError::Ssh),
        }
    }

    fn open(&mut self, path: PathBuf) -> Source {
        match self {
            Self::Local {
                local,
                active: LocalTarget::Local,
                ..
            } => local.open(path),
            Self::Local {
                scratch,
                active: LocalTarget::Scratch,
                ..
            } => scratch.open(path),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                local,
                active: DirTarget::Local,
                ..
            } => local.open(path),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                scratch,
                active: DirTarget::Scratch,
                ..
            } => scratch.open(path),
            #[cfg(feature = "ssh")]
            Self::Ssh {
                ssh,
                active: DirTarget::Ssh,
                ..
            } => ssh.open(path),
        }
    }

    fn target(&self) -> DirTarget {
        match self {
            Self::Local { active, .. } => (*active).into(),
            #[cfg(feature = "ssh")]
            Self::Ssh { active, .. } => *active,
        }
    }

    fn toggle_source(&mut self) -> DirTarget {
        match self {
            Self::Local { active, .. } => active.toggle().into(),
            #[cfg(feature = "ssh")]
            Self::Ssh { active, .. } => active.toggle(),
        }
    }
}

pub struct FileChooser<S: ChooserSource> {
    phantom: std::marker::PhantomData<S>,
}

impl<S: ChooserSource> Default for FileChooser<S> {
    fn default() -> Self {
        Self {
            phantom: std::marker::PhantomData,
        }
    }
}

impl<S: ChooserSource> StatefulWidget for FileChooser<S> {
    type State = FileChooserState<S>;

    fn render(
        self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        state: &mut FileChooserState<S>,
    ) {
        use crate::buffer::{BufferMessage, render_message};
        use crate::help::{CREATE_FILE, FIND_IN_FILES, OPEN_FILE_TOGGLEABLE, render_help};
        use crate::scrollbar::{Scrollbar, ScrollbarState};
        use ratatui::{
            layout::{
                Constraint::{Length, Min},
                Layout,
            },
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Block, BorderType, List, ListState, Paragraph, Widget},
        };
        use std::borrow::Cow;

        let block = Block::bordered()
            .border_type(BorderType::Thick)
            .title_top(Line::from(vec![
                Span::raw("\u{252b}"),
                Span::styled(state.dir.display().to_string(), Style::default().bold()),
                Span::raw("\u{2523}"),
            ]))
            .title_bottom(
                Line::from(vec![
                    Span::raw("\u{252b}"),
                    Span::styled(state.source.to_string(), Style::default().bold()),
                    Span::raw("\u{2523}"),
                ])
                .centered(),
            );

        ratatui::widgets::Clear.render(area, buf);

        let [top_area, list_area] = Layout::vertical([Length(3), Min(0)]).areas(block.inner(area));

        let [list_area, scrollbar_area] = Layout::horizontal([Min(0), Length(1)]).areas(list_area);

        block.render(area, buf);

        let [text_area, _] = Layout::horizontal([Length(TEXT_WIDTH + 2), Min(0)]).areas(top_area);

        match &state.mode {
            Mode::Default => Paragraph::new("")
                .block(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .title("Filename"),
                )
                .render(text_area, buf),
            Mode::New(filename) => Paragraph::new(crate::truncate::line_start(
                filename.value().unwrap_or_default().into(),
                filename.cursor_column().saturating_sub(TEXT_WIDTH.into()),
            ))
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title("Filename"),
            )
            .render(text_area, buf),
            Mode::Selected(items) => Paragraph::new(match items.len() {
                1 => Cow::Borrowed("1 File Selected"),
                n => Cow::Owned(format!("{n} Files Selected")),
            })
            .block(Block::bordered().border_type(BorderType::Rounded))
            .render(text_area, buf),
            Mode::Search { .. } => Paragraph::new("")
                .block(Block::bordered().border_type(BorderType::Rounded))
                .render(text_area, buf),
        }

        StatefulWidget::render(
            (match &state.mode {
                Mode::Default | Mode::New(_) => List::new(state.dir_entries()),
                Mode::Selected(selected) | Mode::Search { selected, .. } => {
                    List::new(state.contents.iter().map(|e| {
                        if selected.contains_key(&e.path) {
                            format!("* {}", e.name)
                        } else {
                            format!("  {}", e.name)
                        }
                    }))
                }
            })
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
            list_area,
            buf,
            &mut ListState::default()
                .with_selected(state.selected_entry())
                .with_offset(
                    state
                        .selected_entry()
                        .map(|s| s.saturating_sub(usize::from(list_area.height) / 2))
                        .unwrap_or_default(),
                ),
        );

        Scrollbar.render(
            scrollbar_area,
            buf,
            &mut ScrollbarState::new(state.contents.len())
                .viewport_content_length(list_area.height.into())
                .position(
                    state
                        .selected_entry()
                        .map(|s| s.saturating_sub(usize::from(list_area.height) / 2))
                        .unwrap_or_default(),
                ),
        );

        render_help(
            list_area,
            buf,
            match &state.mode {
                Mode::Default | Mode::Selected(_) => OPEN_FILE_TOGGLEABLE,
                Mode::New(_) => CREATE_FILE,
                Mode::Search { .. } => FIND_IN_FILES,
            },
            |b| {
                if state.show_hidden {
                    b.title_top("Showing Hidden")
                } else {
                    b
                }
            },
        );

        if let Mode::Search { search, type_, .. } = &state.mode {
            use crate::buffer::widen_tabs;
            use ratatui::widgets::Clear;

            let [_, dialog_area, _] =
                Layout::vertical([Min(0), Length(3), Min(0)]).areas(list_area);

            Clear.render(dialog_area, buf);
            Paragraph::new(crate::truncate::line_start(
                widen_tabs(search.chars().collect::<String>().into()),
                search
                    .cursor_column()
                    .saturating_sub(dialog_area.width.saturating_sub(2).into()),
            ))
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title_top(Line::from("Select Files Containing").left_aligned())
                    .title_top(Line::from(type_.to_string()).right_aligned()),
            )
            .render(dialog_area, buf);
        }

        if let Some(error) = state.error.take() {
            render_message(list_area, buf, BufferMessage::Error(error.into()));
        }
    }
}

pub struct FileChooserState<S: ChooserSource> {
    cwd: PathBuf,          // editor's current working directory
    dir: PathBuf,          // directory we've navigated to
    contents: Vec<Entry>,  // directory entry
    dir_count: usize,      // number of directories in contents
    index: Option<usize>,  // index in directory entries
    mode: Mode,            // either new file or chosen entries
    error: Option<String>, // error message
    source: S,             // file source
    show_hidden: bool,     // whether to display hidden files
    scratch_buffers: Vec<PathBuf>,
}

impl<S: ChooserSource> FileChooserState<S> {
    /// May return an error if unable to get the current
    /// working directory or are unable to read it
    pub fn new(
        source: S,
        scratch_buffers: Vec<PathBuf>,
        dir: Option<PathBuf>,
    ) -> Result<Self, S::Error> {
        let cwd = source.current_dir()?;
        let dir = dir.unwrap_or_else(|| cwd.clone());

        let contents = source.read_dir(&scratch_buffers, &dir, false)?;

        Ok(Self {
            dir,
            dir_count: contents.iter().take_while(|e| e.is_dir).count(),
            contents,
            cwd,
            index: None,
            mode: Mode::default(),
            error: None,
            source,
            show_hidden: false,
            scratch_buffers,
        })
    }

    pub fn update_dir(&mut self, new_dir: PathBuf) {
        match self
            .source
            .read_dir(&self.scratch_buffers, &new_dir, self.show_hidden)
        {
            Ok(contents) => {
                self.dir_count = contents.iter().take_while(|e| e.is_dir).count();
                self.contents = contents;
                self.index = None;
                self.dir = new_dir;
            }
            Err(err) => {
                self.error = Some(err.to_string());
            }
        }
    }

    pub fn toggle_show_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        let dir = std::mem::take(&mut self.dir);
        self.update_dir(dir);
    }

    pub fn dir_entries(&self) -> impl Iterator<Item = &str> {
        self.contents.iter().map(|e| e.name.as_str())
    }

    pub fn selected_entry(&self) -> Option<usize> {
        self.index
    }

    pub fn selected_dir(&self) -> &Path {
        self.dir.as_path()
    }

    pub fn arrow_up(&mut self) {
        if matches!(self.mode, Mode::Default | Mode::Selected(_)) {
            self.index = match self.index {
                None => max_index(&self.mode, &self.contents, self.dir_count).checked_sub(1),
                Some(i) => i.checked_sub(1).or_else(|| {
                    max_index(&self.mode, &self.contents, self.dir_count).checked_sub(1)
                }),
            }
        }
    }

    pub fn arrow_down(&mut self) {
        if matches!(self.mode, Mode::Default | Mode::Selected(_)) {
            self.index = (match self.index {
                None => Some(0),
                Some(i) => Some(i + 1),
            })
            .and_then(|i| i.checked_rem(max_index(&self.mode, &self.contents, self.dir_count)));
        }
    }

    pub fn page_up(&mut self) {
        if matches!(self.mode, Mode::Default | Mode::Selected(_)) {
            self.index = (match self.index {
                None => Some(0),
                Some(idx) => Some(idx.saturating_sub(PAGE_SIZE)),
            })
            .filter(|i| *i < max_index(&self.mode, &self.contents, self.dir_count))
        }
    }

    pub fn page_down(&mut self) {
        if matches!(self.mode, Mode::Default | Mode::Selected(_)) {
            self.index = match max_index(&self.mode, &self.contents, self.dir_count) {
                0 => None,
                max => match self.index {
                    None => Some(PAGE_SIZE.min(max - 1)),
                    Some(idx) => Some((idx + PAGE_SIZE).min(max - 1)),
                },
            }
        }
    }

    pub fn home(&mut self) {
        match &mut self.mode {
            Mode::New(field) | Mode::Search { search: field, .. } => field.cursor_home(),
            Mode::Default | Mode::Selected(_) => {
                self.index = match max_index(&self.mode, &self.contents, self.dir_count) {
                    0 => None,
                    _ => Some(0),
                }
            }
        }
    }

    pub fn end(&mut self) {
        match &mut self.mode {
            Mode::New(field) | Mode::Search { search: field, .. } => field.cursor_end(),
            Mode::Default | Mode::Selected(_) => {
                self.index = max_index(&self.mode, &self.contents, self.dir_count).checked_sub(1);
            }
        }
    }

    pub fn arrow_right(&mut self) {
        match &mut self.mode {
            Mode::New(field) | Mode::Search { search: field, .. } => field.cursor_forward(),
            Mode::Default | Mode::Selected(_) => {
                if let Some(idx) = self.index
                    && let Some(Entry {
                        path, is_dir: true, ..
                    }) = self.contents.get(idx)
                {
                    self.update_dir(path.clone());
                }
            }
        }
    }

    pub fn arrow_left(&mut self) {
        match &mut self.mode {
            Mode::New(field) | Mode::Search { search: field, .. } => field.cursor_back(),
            Mode::Default | Mode::Selected(_) => {
                if let Some(parent) = self.dir.parent()
                    && parent != Path::new("")
                {
                    self.update_dir(parent.to_path_buf());
                }
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        match &mut self.mode {
            Mode::Default => {
                self.mode = Mode::New({
                    let mut filename = TextField::default();
                    filename.insert_char(c);
                    filename
                });
                self.index = None;
            }
            Mode::New(prompt) => {
                prompt.insert_char(c);
                self.index = None;
            }
            Mode::Search { search, .. } => search.insert_char(c),
            Mode::Selected(_) => { /* do nothing */ }
        }
    }

    pub fn backspace(&mut self) {
        match &mut self.mode {
            Mode::New(prompt) => {
                prompt.backspace();
                if prompt.is_empty() {
                    self.mode = Mode::Default;
                }
            }
            Mode::Search { search, .. } => search.backspace(),
            Mode::Default | Mode::Selected(_) => { /* do nothing */ }
        }
    }

    pub fn delete(&mut self) {
        match &mut self.mode {
            Mode::New(prompt) => {
                prompt.delete();
                if prompt.is_empty() {
                    self.mode = Mode::Default;
                }
            }
            Mode::Search { search, .. } => search.delete(),
            Mode::Default | Mode::Selected(_) => { /* do nothing */ }
        }
    }

    pub fn toggle_selected(&mut self) {
        match &mut self.mode {
            Mode::New(_) => { /* do nothing*/ }
            Mode::Default => {
                if let Some(idx) = self.index
                    && let Some(Entry {
                        path,
                        is_dir: false,
                        ..
                    }) = self.contents.get(idx)
                {
                    self.mode = Mode::Selected(BTreeMap::from([(path.clone(), ())]));
                }
            }
            Mode::Selected(selected) => {
                if let Some(idx) = self.index
                    && let Some(Entry {
                        path,
                        is_dir: false,
                        ..
                    }) = self.contents.get(idx)
                {
                    use std::collections::btree_map::Entry;

                    match selected.entry(path.clone()) {
                        Entry::Vacant(v) => {
                            v.insert(());
                        }
                        Entry::Occupied(o) => {
                            o.remove();
                            if selected.is_empty() {
                                self.mode = Mode::Default;
                            }
                        }
                    }
                }
            }
            Mode::Search { type_, .. } => {
                *type_ = type_.toggle_search();
            }
        }
    }

    pub fn toggle_all_selected(&mut self) {
        match &mut self.mode {
            Mode::Default => {
                let chosen = self
                    .contents
                    .iter()
                    .filter(|e| !e.is_dir)
                    .map(|e| (e.path.clone(), ()))
                    .collect::<BTreeMap<_, _>>();
                if !chosen.is_empty() {
                    self.mode = Mode::Selected(chosen);
                }
            }
            Mode::Selected(selected) => {
                use std::collections::btree_map::Entry;

                for e in self.contents.iter().filter(|e| !e.is_dir) {
                    match selected.entry(e.path.clone()) {
                        Entry::Vacant(v) => {
                            v.insert(());
                        }
                        Entry::Occupied(o) => {
                            o.remove();
                        }
                    }
                }
                if selected.is_empty() {
                    self.mode = Mode::Default;
                }
            }
            Mode::New(_) | Mode::Search { .. } => { /* do nothing */ }
        }
    }

    pub fn toggle_search(&mut self, last_search: &LastSearch) {
        match &mut self.mode {
            Mode::Default => {
                self.mode = Mode::Search {
                    search: TextField::default(),
                    type_: SearchType::default(),
                    selected: BTreeMap::default(),
                };
            }
            Mode::Selected(selected) => {
                self.mode = Mode::Search {
                    search: TextField::default(),
                    type_: SearchType::default(),
                    selected: std::mem::take(selected),
                };
            }
            Mode::Search { search, type_, .. } => {
                if search.is_empty()
                    && let Some(last) = &last_search[*type_]
                {
                    *search = last.clone();
                } else {
                    search.reset();
                }
            }
            Mode::New(_) => { /* creating new file, so do nothing */ }
        }
    }

    pub fn select(&mut self, last_search: &mut LastSearch) -> Option<Vec<Source>> {
        use crate::buffer::Normalizations;

        fn strip_cwd(cwd: &Path, path: &Path) -> PathBuf {
            match path.strip_prefix(cwd) {
                Ok(stripped) => stripped.to_path_buf(),
                Err(_) => path.to_owned(),
            }
        }

        fn append_matches<S: ChooserSource, T: SearchTerm>(
            source: &mut S,
            contents: &[Entry],
            term: T,
            matches: &mut BTreeMap<PathBuf, ()>,
        ) {
            matches.extend(contents.iter().filter_map(|e| {
                source
                    .open(e.path.clone())
                    .contains(&term)
                    .then_some((e.path.clone(), ()))
            }));
        }

        match std::mem::take(&mut self.mode) {
            Mode::Default => match self.contents.get(self.index?)? {
                Entry {
                    is_dir: true, path, ..
                } => {
                    self.update_dir(path.clone());
                    None
                }
                Entry {
                    is_dir: false,
                    path,
                    ..
                } => Some(vec![self.source.open(strip_cwd(&self.cwd, path))]),
            },
            Mode::New(filename) => Some(vec![self.source.open(strip_cwd(
                &self.cwd,
                &self.dir.join(filename.value().expect("empty filename")),
            ))]),
            Mode::Selected(selected) => Some(
                selected
                    .into_keys()
                    .map(|path| self.source.open(strip_cwd(&self.cwd, &path)))
                    .collect(),
            ),
            Mode::Search {
                search,
                type_,
                mut selected,
            } => {
                match type_ {
                    SearchType::CaseSensitive => match Normalizations::try_from(search.value()?) {
                        Err(term) => {
                            append_matches(&mut self.source, &self.contents, term, &mut selected)
                        }
                        Ok(normalizations) => append_matches(
                            &mut self.source,
                            &self.contents,
                            normalizations,
                            &mut selected,
                        ),
                    },
                    SearchType::CaseInsensitive => {
                        match Normalizations::try_from(search.value()?) {
                            Err(term) => {
                                match fancy_regex::RegexBuilder::new(&fancy_regex::escape(&term))
                                    .case_insensitive(true)
                                    .build()
                                {
                                    Ok(regex) => append_matches(
                                        &mut self.source,
                                        &self.contents,
                                        regex,
                                        &mut selected,
                                    ),
                                    Err(err) => {
                                        self.error = Some(err.to_string());
                                    }
                                }
                            }
                            Ok(normalizations) => append_matches(
                                &mut self.source,
                                &self.contents,
                                CaseInsensitiveNormalizations::from(normalizations),
                                &mut selected,
                            ),
                        }
                    }
                    SearchType::Regex => match search.value()?.parse::<fancy_regex::Regex>() {
                        Ok(regex) => {
                            append_matches(&mut self.source, &self.contents, regex, &mut selected)
                        }
                        Err(err) => {
                            self.error = Some(err.to_string());
                        }
                    },
                }

                self.mode = if selected.is_empty() {
                    Mode::Default
                } else {
                    last_search[type_] = Some(search);
                    Mode::Selected(selected)
                };

                None
            }
        }
    }

    pub fn cursor_position(&self, area: ratatui::layout::Rect) -> (u16, u16) {
        match &self.mode {
            Mode::Default => (area.x + 1, area.y + 1),
            Mode::New(filename) => (
                (area.x + filename.cursor_column() as u16).min(TEXT_WIDTH) + 1,
                area.y + 1,
            ),
            Mode::Selected(_) => (area.x + 1, area.y + 1),
            Mode::Search { search, .. } => {
                use ratatui::{
                    layout::{
                        Constraint::{Length, Min},
                        Layout,
                    },
                    widgets::Block,
                };

                let [_, list_area] = Layout::vertical([Length(3), Min(0)]).areas(area);
                let [list_area, _] = Layout::horizontal([Min(0), Length(1)]).areas(list_area);
                let [_, dialog_area, _] =
                    Layout::vertical([Min(0), Length(3), Min(0)]).areas(list_area);
                let dialog_area = Block::bordered().inner(dialog_area);
                let col = dialog_area.x + (search.cursor_column() as u16).min(TEXT_WIDTH);
                let row = dialog_area.y;

                (col, row)
            }
        }
    }

    pub fn target(&self) -> DirTarget {
        self.source.target()
    }

    pub fn toggle_source(&mut self, open_dir: &crate::editor::OpenDir) -> Result<(), S::Error> {
        let target = self.source.toggle_source();
        *self = Self::new(
            self.source.clone(),
            std::mem::take(&mut self.scratch_buffers),
            open_dir[target].clone(),
        )?;
        Ok(())
    }

    pub fn paste(&mut self, f: impl FnOnce() -> Option<PasteContents>) {
        fn append_matches<S: ChooserSource, T: SearchTerm>(
            source: &mut S,
            contents: &[Entry],
            term: T,
            matches: &mut BTreeMap<PathBuf, ()>,
        ) {
            matches.extend(contents.iter().filter_map(|e| {
                source
                    .open(e.path.clone())
                    .contains_multiline(&term)
                    .then_some((e.path.clone(), ()))
            }));
        }

        match &mut self.mode {
            Mode::Default | Mode::Selected(_) => { /* do nothing */ }
            Mode::New(filename) => {
                // don't attempt to stick multiline pastes
                // into the filename prompt
                if let Some(PasteContents::SingleLine(text)) = f() {
                    filename.paste(&text);
                }
            }
            Mode::Search {
                search, selected, ..
            } => match f() {
                Some(PasteContents::SingleLine(text)) => {
                    search.paste(&text);
                }
                Some(PasteContents::MultiLine(text)) => {
                    append_matches(&mut self.source, &self.contents, text, selected);

                    self.mode = if selected.is_empty() {
                        Mode::Default
                    } else {
                        Mode::Selected(std::mem::take(selected))
                    };
                }
                Some(PasteContents::MultiLineNormalized(normalizations)) => {
                    append_matches(&mut self.source, &self.contents, normalizations, selected);

                    self.mode = if selected.is_empty() {
                        Mode::Default
                    } else {
                        Mode::Selected(std::mem::take(selected))
                    };
                }
                None => { /* nothing to paste */ }
            },
        }
    }
}

fn max_index(mode: &Mode, contents: &[Entry], dir_count: usize) -> usize {
    // this should only be called in Default/Selected modes
    match mode {
        Mode::Default | Mode::Selected(_) => contents.len(),
        Mode::New(_) | Mode::Search { .. } => dir_count,
    }
}

pub struct Entry {
    name: String,  // user-visible name
    path: PathBuf, // actual path on disk
    is_dir: bool,  // whether item is directory
}

impl Entry {
    fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }
}

impl TryFrom<std::fs::DirEntry> for Entry {
    type Error = std::io::Error;

    fn try_from(entry: std::fs::DirEntry) -> std::io::Result<Self> {
        let path = entry.path();
        let is_dir = std::fs::metadata(&path)
            .map(|m| m.is_dir())
            .unwrap_or(false);
        Ok(Self {
            name: match is_dir {
                false => entry.file_name().display().to_string(),
                true => format!(
                    "{}{}",
                    entry.file_name().display(),
                    std::path::MAIN_SEPARATOR,
                ),
            },
            is_dir,
            path,
        })
    }
}

#[cfg(feature = "ssh")]
impl From<(bool, PathBuf)> for Entry {
    fn from((is_dir, path): (bool, PathBuf)) -> Self {
        Self {
            name: match is_dir {
                false => path
                    .file_name()
                    .map(|n| n.display().to_string())
                    .unwrap_or_default(),
                true => format!(
                    "{}{}",
                    path.file_name()
                        .map(|n| n.display().to_string())
                        .unwrap_or_default(),
                    std::path::MAIN_SEPARATOR,
                ),
            },
            path,
            is_dir,
        }
    }
}

#[derive(Default)]
enum Mode {
    #[default]
    Default, // nothing selected
    New(TextField),                  // new file
    Selected(BTreeMap<PathBuf, ()>), // selected existing file(s)
    Search {
        search: TextField,
        type_: SearchType,
        selected: BTreeMap<PathBuf, ()>,
    },
}
