// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::buffer::Source;
use ratatui::widgets::StatefulWidget;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Width of text box, in characters
const TEXT_WIDTH: u16 = 30;

/// Size of each page, in rows
const PAGE_SIZE: usize = 10;

pub trait ChooserSource {
    type Error: std::fmt::Display;

    fn current_dir(&self) -> Result<PathBuf, Self::Error>;

    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>, Self::Error>;

    fn open(&self, path: PathBuf) -> Source;
}

#[derive(Default)]
pub struct LocalSource;

impl ChooserSource for LocalSource {
    type Error = std::io::Error;

    fn current_dir(&self) -> std::io::Result<PathBuf> {
        std::env::current_dir()
    }

    fn read_dir(&self, dir: &Path) -> std::io::Result<Vec<Entry>> {
        dir.read_dir()
            .and_then(|entries| entries.map(|e| e.and_then(Entry::try_from)).collect())
            .map(|mut entries: Vec<Entry>| {
                entries.sort_unstable_by(|x, y| {
                    x.is_dir.cmp(&y.is_dir).reverse().then(x.path.cmp(&y.path))
                });
                entries
            })
    }

    fn open(&self, path: PathBuf) -> Source {
        Source::Local(path)
    }
}

#[cfg(feature = "ssh")]
pub struct SshSource {
    remote: std::rc::Rc<ssh2::Sftp>,
}

#[cfg(feature = "ssh")]
impl SshSource {
    pub fn open(session: &ssh2::Session) -> Result<Self, ssh2::Error> {
        session.sftp().map(|remote| Self {
            remote: std::rc::Rc::new(remote),
        })
    }
}

#[cfg(feature = "ssh")]
impl ChooserSource for SshSource {
    type Error = ssh2::Error;

    fn current_dir(&self) -> Result<PathBuf, Self::Error> {
        self.remote.realpath(Path::new("."))
    }

    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>, Self::Error> {
        self.remote
            .readdir(dir)
            .map(|entries| entries.into_iter().map(Entry::from).collect())
            .map(|mut entries: Vec<Entry>| {
                entries.sort_unstable_by(|x, y| {
                    x.is_dir.cmp(&y.is_dir).reverse().then(x.path.cmp(&y.path))
                });
                entries
            })
    }

    fn open(&self, path: PathBuf) -> Source {
        Source::Ssh {
            sftp: std::rc::Rc::clone(&self.remote),
            path,
        }
    }
}

#[cfg(feature = "ssh")]
pub enum EitherSource {
    Local(LocalSource),
    Ssh(SshSource),
}

#[cfg(feature = "ssh")]
impl ChooserSource for EitherSource {
    type Error = EitherError;

    fn current_dir(&self) -> Result<PathBuf, Self::Error> {
        match self {
            Self::Local(l) => l.current_dir().map_err(EitherError::Io),
            Self::Ssh(s) => s.current_dir().map_err(EitherError::Ssh),
        }
    }

    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>, Self::Error> {
        match self {
            Self::Local(l) => l.read_dir(dir).map_err(EitherError::Io),
            Self::Ssh(s) => s.read_dir(dir).map_err(EitherError::Ssh),
        }
    }

    fn open(&self, path: PathBuf) -> Source {
        match self {
            Self::Local(l) => l.open(path),
            Self::Ssh(s) => s.open(path),
        }
    }
}

#[cfg(feature = "ssh")]
#[derive(Debug)]
pub enum EitherError {
    Io(std::io::Error),
    Ssh(ssh2::Error),
}

#[cfg(feature = "ssh")]
impl std::fmt::Display for EitherError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Io(i) => i.fmt(f),
            Self::Ssh(s) => s.fmt(f),
        }
    }
}

#[cfg(feature = "ssh")]
impl std::error::Error for EitherError {}

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
        use crate::help::{OPEN_FILE, render_help};
        use ratatui::{
            layout::{
                Constraint::{Length, Min},
                Layout,
            },
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{
                Block, BorderType, Borders, List, ListState, Paragraph, Scrollbar,
                ScrollbarOrientation, ScrollbarState, Widget,
            },
        };
        use std::borrow::Cow;

        let block = Block::bordered()
            .border_type(BorderType::Thick)
            .borders(Borders::TOP)
            .title_top(Line::from(vec![
                Span::raw("\u{252b}"),
                Span::styled(state.dir.display().to_string(), Style::default().bold()),
                Span::raw("\u{2523}"),
            ]));

        ratatui::widgets::Clear.render(area, buf);

        let [top_area, list_area] = Layout::vertical([Length(3), Min(0)]).areas(block.inner(area));

        let [list_area, scrollbar_area] = Layout::horizontal([Min(0), Length(1)]).areas(list_area);

        block.render(area, buf);

        let [text_area, _] = Layout::horizontal([Length(TEXT_WIDTH + 2), Min(0)]).areas(top_area);

        match &state.chosen {
            Chosen::Default => Paragraph::new("")
                .block(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .title("Filename"),
                )
                .render(text_area, buf),
            Chosen::New(filename) => {
                use unicode_width::UnicodeWidthStr;

                let filename = filename.iter().copied().collect::<String>();
                let filename_width = filename.width();
                Paragraph::new(filename)
                    .scroll((
                        0,
                        filename_width
                            .saturating_sub(TEXT_WIDTH.into())
                            .try_into()
                            .unwrap(),
                    ))
                    .block(
                        Block::bordered()
                            .border_type(BorderType::Rounded)
                            .title("Filename"),
                    )
                    .render(text_area, buf)
            }
            Chosen::Selected(items) => Paragraph::new(match items.len() {
                1 => Cow::Borrowed("1 File Selected"),
                n => Cow::Owned(format!("{n} Files Selected")),
            })
            .block(Block::bordered().border_type(BorderType::Rounded))
            .render(text_area, buf),
        }

        StatefulWidget::render(
            (match &state.chosen {
                Chosen::Default | Chosen::New(_) => List::new(state.dir_entries()),
                Chosen::Selected(selected) => List::new(state.contents.iter().map(|e| {
                    if selected.contains(&e.path) {
                        format!("* {}", e.name)
                    } else {
                        format!("  {}", e.name)
                    }
                })),
            })
            .scroll_padding(10)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
            list_area,
            buf,
            &mut ListState::default().with_selected(state.selected_entry()),
        );

        Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
            scrollbar_area,
            buf,
            &mut ScrollbarState::new(state.contents.len())
                .viewport_content_length(list_area.height.into())
                .position(state.selected_entry().unwrap_or_default()),
        );

        render_help(list_area, buf, OPEN_FILE, |b| b);

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
    chosen: Chosen,        // either new file or chosen entries
    error: Option<String>, // error message
    source: S,             // file source
}

impl<S: ChooserSource> FileChooserState<S> {
    /// May return an error if unable to get the current
    /// working directory or are unable to read it
    pub fn new(source: S) -> Result<Self, S::Error> {
        let cwd = source.current_dir()?;

        let contents = source.read_dir(&cwd)?;

        Ok(Self {
            dir: cwd.clone(),
            dir_count: contents.iter().take_while(|e| e.is_dir).count(),
            contents,
            cwd,
            index: None,
            chosen: Chosen::default(),
            error: None,
            source,
        })
    }

    pub fn update_dir(&mut self, new_dir: PathBuf) {
        match self.source.read_dir(&new_dir) {
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

    pub fn dir_entries(&self) -> impl Iterator<Item = &str> {
        self.contents.iter().map(|e| e.name.as_str())
    }

    pub fn selected_entry(&self) -> Option<usize> {
        self.index
    }

    pub fn arrow_up(&mut self) {
        self.index = match self.index {
            None => max_index(&self.chosen, &self.contents, self.dir_count).checked_sub(1),
            Some(i) => i
                .checked_sub(1)
                .or_else(|| max_index(&self.chosen, &self.contents, self.dir_count).checked_sub(1)),
        }
    }

    pub fn arrow_down(&mut self) {
        self.index = (match self.index {
            None => Some(0),
            Some(i) => Some(i + 1),
        })
        .and_then(|i| i.checked_rem(max_index(&self.chosen, &self.contents, self.dir_count)));
    }

    pub fn page_up(&mut self) {
        self.index = (match self.index {
            None => Some(0),
            Some(idx) => Some(idx.saturating_sub(PAGE_SIZE)),
        })
        .filter(|i| *i < max_index(&self.chosen, &self.contents, self.dir_count))
    }

    pub fn page_down(&mut self) {
        self.index = match max_index(&self.chosen, &self.contents, self.dir_count) {
            0 => None,
            max => match self.index {
                None => Some(PAGE_SIZE.min(max - 1)),
                Some(idx) => Some((idx + PAGE_SIZE).min(max - 1)),
            },
        }
    }

    pub fn home(&mut self) {
        self.index = match max_index(&self.chosen, &self.contents, self.dir_count) {
            0 => None,
            _ => Some(0),
        }
    }

    pub fn end(&mut self) {
        self.index = max_index(&self.chosen, &self.contents, self.dir_count).checked_sub(1);
    }

    pub fn arrow_right(&mut self) {
        if let Some(idx) = self.index
            && let Some(Entry {
                path, is_dir: true, ..
            }) = self.contents.get(idx)
        {
            self.update_dir(path.clone());
        }
    }

    pub fn arrow_left(&mut self) {
        if let Some(parent) = self.dir.parent()
            && parent != Path::new("")
        {
            self.update_dir(parent.to_path_buf());
        }
    }

    pub fn push(&mut self, c: char) {
        match &mut self.chosen {
            Chosen::Default => {
                self.chosen = Chosen::New(vec![c]);
                self.index = None;
            }
            Chosen::New(prompt) => {
                prompt.push(c);
                self.index = None;
            }
            Chosen::Selected(_) => { /* do nothing */ }
        }
    }

    pub fn pop(&mut self) {
        if let Chosen::New(prompt) = &mut self.chosen {
            prompt.pop();
            if prompt.is_empty() {
                self.chosen = Chosen::Default;
            }
        }
    }

    pub fn toggle_selected(&mut self) {
        if let Some(idx) = self.index
            && let Some(Entry {
                path,
                is_dir: false,
                ..
            }) = self.contents.get(idx)
        {
            match &mut self.chosen {
                Chosen::Default => {
                    self.chosen = Chosen::Selected(BTreeSet::from([path.clone()]));
                }
                // use Entry API in the future, whenever that stabilizes
                Chosen::Selected(selected) => {
                    if !selected.insert(path.clone()) {
                        selected.remove(path);
                        if selected.is_empty() {
                            self.chosen = Chosen::Default;
                        }
                    }
                }
                Chosen::New(_) => { /* this shouldn't be possible */ }
            }
        }
    }

    pub fn select(&mut self) -> Option<Vec<Source>> {
        fn strip_cwd(cwd: &Path, path: &Path) -> PathBuf {
            match path.strip_prefix(cwd) {
                Ok(stripped) => stripped.to_path_buf(),
                Err(_) => path.to_owned(),
            }
        }

        match std::mem::take(&mut self.chosen) {
            Chosen::Default => match self.contents.get(self.index?)? {
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
            Chosen::New(filename) => Some(vec![self.source.open(strip_cwd(
                &self.cwd,
                &self.dir.join(filename.into_iter().collect::<String>()),
            ))]),
            Chosen::Selected(selected) => Some(
                selected
                    .into_iter()
                    .map(|path| self.source.open(strip_cwd(&self.cwd, &path)))
                    .collect(),
            ),
        }
    }

    pub fn cursor_position(&self) -> (u16, u16) {
        use unicode_width::UnicodeWidthStr;

        match &self.chosen {
            Chosen::Default => (1, 1),
            Chosen::New(filename) => (
                1u16 + (filename.iter().collect::<String>().width() as u16).min(TEXT_WIDTH),
                1,
            ),
            Chosen::Selected(_) => (0, self.index.map(|idx| 3u16 + idx as u16).unwrap_or(1)),
        }
    }
}

fn max_index(chosen: &Chosen, contents: &[Entry], dir_count: usize) -> usize {
    match chosen {
        Chosen::Default | Chosen::Selected(_) => contents.len(),
        Chosen::New(_) => dir_count,
    }
}

pub struct Entry {
    name: String,  // user-visible name
    path: PathBuf, // actual path on disk
    is_dir: bool,  // whether item is directory
}

impl TryFrom<std::fs::DirEntry> for Entry {
    type Error = std::io::Error;

    fn try_from(entry: std::fs::DirEntry) -> std::io::Result<Self> {
        let is_dir = entry.metadata()?.is_dir();
        Ok(Self {
            name: match is_dir {
                false => entry.file_name().display().to_string(),
                true => format!(
                    "{}{}",
                    entry.file_name().display(),
                    std::path::MAIN_SEPARATOR,
                ),
            },
            path: entry.path(),
            is_dir,
        })
    }
}

#[cfg(feature = "ssh")]
impl From<(PathBuf, ssh2::FileStat)> for Entry {
    fn from((path, stat): (PathBuf, ssh2::FileStat)) -> Self {
        let is_dir = stat.is_dir();
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
enum Chosen {
    #[default]
    Default, // nothing selected
    New(Vec<char>),              // new file
    Selected(BTreeSet<PathBuf>), // selected existing file(s)
}
