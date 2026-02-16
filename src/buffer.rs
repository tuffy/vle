// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::editor::EditorMode;
use crate::endings::LineEndings;
use crate::syntax::Highlighter;
use ratatui::{
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
use std::borrow::Cow;
use std::cell::RefCell;
use std::num::NonZero;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::SystemTime;

pub static SPACES_PER_TAB: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("VLE_SPACES_PER_TAB")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .map(|s| s.clamp(1, 16))
        .unwrap_or(4)
});

static ALWAYS_TAB: LazyLock<bool> = LazyLock::new(|| std::env::var("VLE_ALWAYS_TAB").is_ok());

/// A buffer's source file
pub enum Source {
    Local(PathBuf),
    #[cfg(feature = "ssh")]
    Ssh {
        sftp: Rc<ssh2::Sftp>,
        path: PathBuf,
    },
    Tutorial,
}

#[cfg(not(feature = "ssh"))]
impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local(x), Self::Local(y)) => x == y,
            (Self::Tutorial, Self::Tutorial) => true,
            _ => false,
        }
    }
}

#[cfg(feature = "ssh")]
impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local(x), Self::Local(y)) => x == y,
            (Self::Ssh { sftp: s1, path: x }, Self::Ssh { sftp: s2, path: y }) => {
                Rc::ptr_eq(s1, s2) && x == y
            }
            (Self::Tutorial, Self::Tutorial) => true,
            _ => false,
        }
    }
}

impl Eq for Source {}

impl From<PathBuf> for Source {
    fn from(s: PathBuf) -> Self {
        Self::Local(s)
    }
}

impl Source {
    /// Used to display in the title
    fn name(&self) -> Cow<'_, str> {
        match self {
            Self::Local(path) => path.to_string_lossy(),
            #[cfg(feature = "ssh")]
            Self::Ssh { path, .. } => path.to_string_lossy(),
            Self::Tutorial => "Welcome!".into(),
        }
    }

    /// Used to display in the tab bar
    fn short_name(&self) -> Cow<'_, str> {
        match self {
            Self::Local(path) => path
                .file_prefix()
                .map(|s| s.to_string_lossy())
                .unwrap_or_else(|| "???".into()),
            #[cfg(feature = "ssh")]
            Self::Ssh { path, .. } => path
                .file_prefix()
                .map(|s| s.to_string_lossy())
                .unwrap_or_else(|| "???".into()),
            Self::Tutorial => "Welcome!".into(),
        }
    }

    /// Used to determine syntax highlighting
    pub fn file_name(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::Local(path) => path.file_name().map(|s| s.to_string_lossy()),
            #[cfg(feature = "ssh")]
            Self::Ssh { path, .. } => path.file_name().map(|s| s.to_string_lossy()),
            Self::Tutorial => None,
        }
    }

    /// Also used to determine syntax highlighting
    pub fn extension(&self) -> Option<&str> {
        match self {
            Self::Local(path) => path.extension().and_then(|s| s.to_str()),
            #[cfg(feature = "ssh")]
            Self::Ssh { path, .. } => path.extension().and_then(|s| s.to_str()),
            Self::Tutorial => None,
        }
    }

    /// Used for file reloading
    fn read_string(&self, endings: LineEndings) -> std::io::Result<(Option<SystemTime>, String)> {
        match self {
            Self::Local(path) => {
                let s = std::fs::File::open(path).and_then(|f| endings.reader_to_string(f))?;
                Ok((path.metadata().and_then(|m| m.modified()).ok(), s))
            }
            #[cfg(feature = "ssh")]
            Self::Ssh { sftp, path } => match sftp.open(path) {
                Ok(mut f) => {
                    let s = endings.reader_to_string(&mut f)?;
                    Ok((
                        f.stat().ok().and_then(|stat| stat.mtime).and_then(|secs| {
                            SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs))
                        }),
                        s,
                    ))
                }
                Err(e) => Err(e.into()),
            },
            Self::Tutorial => Ok((
                None,
                include_str!("tutorial.txt").replacen("VERSION", env!("CARGO_PKG_VERSION"), 1),
            )),
        }
    }

    /// Used for file loading (can be based on read_string)
    fn read_data(&self) -> std::io::Result<(Option<SystemTime>, ropey::Rope, LineEndings)> {
        use std::fs::File;

        match self {
            Self::Local(path) => match File::open(path) {
                Ok(mut f) => {
                    let (endings, rope) = LineEndings::reader_to_rope(&mut f)?;
                    Ok((f.metadata().and_then(|m| m.modified()).ok(), rope, endings))
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    Ok((None, ropey::Rope::default(), LineEndings::default()))
                }
                Err(e) => Err(e),
            },
            #[cfg(feature = "ssh")]
            Self::Ssh { sftp, path } => match sftp.open(path) {
                Ok(mut f) => {
                    let (endings, rope) = LineEndings::reader_to_rope(&mut f)?;
                    Ok((
                        f.stat().ok().and_then(|stat| stat.mtime).and_then(|secs| {
                            SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs))
                        }),
                        rope,
                        endings,
                    ))
                }
                Err(e) if e.code() == ssh2::ErrorCode::SFTP(2) => {
                    Ok((None, ropey::Rope::default(), LineEndings::default()))
                }
                Err(e) => Err(e.into()),
            },
            Self::Tutorial => self
                .read_string(LineEndings::default())
                .map(|(t, s)| (t, ropey::Rope::from(s), LineEndings::default())),
        }
    }

    /// Used for file saving
    fn save_data(
        &self,
        data: &ropey::Rope,
        endings: LineEndings,
    ) -> std::io::Result<Option<SystemTime>> {
        use std::fs::File;
        use std::io::{BufWriter, Write};

        match self {
            Self::Local(path) => File::create(path).map(BufWriter::new).and_then(|mut f| {
                endings.rope_to_writer(data, &mut f)?;
                f.flush()?;
                Ok(f.get_mut().metadata().and_then(|m| m.modified()).ok())
            }),
            #[cfg(feature = "ssh")]
            Self::Ssh { sftp, path } => match sftp.create(path) {
                Ok(mut f) => {
                    endings.rope_to_writer(data, &mut f)?;
                    f.flush()?;
                    Ok(f.stat().ok().and_then(|stat| stat.mtime).and_then(|secs| {
                        SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs))
                    }))
                }
                Err(e) => Err(e.into()),
            },
            Self::Tutorial => Ok(None),
        }
    }

    /// Used for the "buffer changed on disk" warning
    fn last_modified(&self) -> Option<SystemTime> {
        match self {
            Self::Local(path) => path.metadata().and_then(|m| m.modified()).ok(),
            #[cfg(feature = "ssh")]
            Self::Ssh { sftp, path } => {
                sftp.stat(path)
                    .ok()
                    .and_then(|stat| stat.mtime)
                    .and_then(|secs| {
                        SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs))
                    })
            }
            Self::Tutorial => None,
        }
    }
}

mod private {
    use crate::buffer::Buffer;
    use std::cell::{Ref, RefCell, RefMut};
    use std::rc::Rc;

    pub struct Rope {
        rope: ropey::Rope,  // the primary data rope
        saved: ropey::Rope, // the rope's contents on disk
        modified: bool,     // whether the rope has been modified
    }

    impl From<ropey::Rope> for Rope {
        fn from(rope: ropey::Rope) -> Self {
            Self {
                saved: rope.clone(),
                rope,
                modified: false,
            }
        }
    }

    impl Rope {
        /// Whether the rope has been modified
        pub fn modified(&self) -> bool {
            self.modified
        }

        /// Tag rope as having been saved successfully
        pub fn save(&mut self) {
            self.saved = self.rope.clone();
            self.modified = false;
        }

        /// Mutable handle to data rope
        pub fn get_mut(&mut self) -> RopeHandle<'_> {
            RopeHandle {
                rope: &mut self.rope,
                saved: &mut self.saved,
                modified: &mut self.modified,
            }
        }
    }

    /// If we're not modifying the rope, its modified state can't be changed
    impl std::ops::Deref for Rope {
        type Target = ropey::Rope;

        fn deref(&self) -> &ropey::Rope {
            &self.rope
        }
    }

    /// A handle to guarantee the "modified buffer" flag is calculated correctly
    pub struct RopeHandle<'r> {
        rope: &'r mut ropey::Rope,
        saved: &'r mut ropey::Rope,
        modified: &'r mut bool,
    }

    impl std::ops::Deref for RopeHandle<'_> {
        type Target = ropey::Rope;

        fn deref(&self) -> &ropey::Rope {
            self.rope
        }
    }

    impl std::ops::DerefMut for RopeHandle<'_> {
        fn deref_mut(&mut self) -> &mut ropey::Rope {
            self.rope
        }
    }

    impl std::ops::Drop for RopeHandle<'_> {
        fn drop(&mut self) {
            // log whether the rope value has been changed
            // from the version that exists on disk
            *self.modified = self.rope != self.saved;
        }
    }

    /// For managing the undo/redo stack properly
    #[derive(Clone)]
    pub struct BufferCell(Rc<RefCell<Buffer>>);

    impl BufferCell {
        pub fn id(&self) -> crate::buffer::BufferId {
            crate::buffer::BufferId(Rc::clone(&self.0))
        }

        pub fn borrow_mut(&self) -> RefMut<'_, Buffer> {
            self.0.borrow_mut()
        }

        pub fn borrow(&self) -> Ref<'_, Buffer> {
            self.0.borrow()
        }

        /// If we're updating the buffer, log its old state on the undo stack
        pub fn borrow_update(&self, cursor: usize, cursor_column: usize) -> RefMut<'_, Buffer> {
            use crate::buffer::{BufferState, Undo};

            let mut buf = self.0.borrow_mut();
            if let None | Some(Undo { finished: true, .. }) = buf.undo.last() {
                let rope = buf.rope.clone();
                buf.undo.push(Undo {
                    state: BufferState {
                        rope,
                        cursor,
                        cursor_column,
                    },
                    finished: false,
                });
                buf.redo.clear();
            }
            buf
        }

        /// If we're performing a move, lock down an undo point once finished
        pub fn borrow_move(&self) -> MoveHandle<'_> {
            MoveHandle(self.0.borrow_mut())
        }
    }

    impl From<Buffer> for BufferCell {
        fn from(buffer: Buffer) -> Self {
            BufferCell(Rc::new(RefCell::new(buffer)))
        }
    }

    pub struct MoveHandle<'b>(RefMut<'b, Buffer>);

    impl std::ops::Deref for MoveHandle<'_> {
        type Target = Buffer;

        fn deref(&self) -> &Buffer {
            &self.0
        }
    }

    impl std::ops::DerefMut for MoveHandle<'_> {
        fn deref_mut(&mut self) -> &mut Buffer {
            &mut self.0
        }
    }

    impl Drop for MoveHandle<'_> {
        fn drop(&mut self) {
            if let Some(last) = self.0.undo.last_mut() {
                last.finished = true;
            }
        }
    }
}

/// A buffer corresponding to a file on disk (either local or remote)
///
/// May be shared between panes
pub struct Buffer {
    source: Source,               // the source file
    endings: LineEndings,         // the source file's line endings
    saved: Option<SystemTime>,    // when the file was last saved
    rope: private::Rope,          // the data rope
    undo: Vec<Undo>,              // the undo stack
    redo: Vec<BufferState>,       // the redo stack
    syntax: Box<dyn Highlighter>, // the syntax highlighting to use
    tabs_required: bool,          // whether the format demands actual tabs
    tab_substitution: String,     // spaces to substitute for tabs
}

impl Buffer {
    /// Used to find if Source has already been opened
    fn source(&self) -> &Source {
        &self.source
    }

    /// Opens file from source, either local or remote
    fn open(source: Source) -> std::io::Result<Self> {
        let (saved, rope, endings) = source.read_data()?;
        let syntax = crate::syntax::syntax(&source);

        Ok(Self {
            rope: rope.into(),
            endings,
            saved,
            tabs_required: *ALWAYS_TAB || syntax.tabs_required(),
            tab_substitution: std::iter::repeat_n(' ', *SPACES_PER_TAB).collect(),
            syntax,
            source,
            undo: vec![],
            redo: vec![],
        })
    }

    /// Builds fresh tutorial buffer
    fn tutorial() -> Self {
        Self {
            rope: ropey::Rope::from(include_str!("tutorial.txt").replacen(
                "VERSION",
                env!("CARGO_PKG_VERSION"),
                1,
            ))
            .into(),
            endings: LineEndings::default(),
            saved: None,
            syntax: Box::new(crate::syntax::Tutorial),
            tab_substitution: std::iter::repeat_n(' ', *SPACES_PER_TAB).collect(),
            tabs_required: *ALWAYS_TAB || crate::syntax::Tutorial.tabs_required(),
            source: Source::Tutorial,
            undo: vec![],
            redo: vec![],
        }
    }

    /// Attempts to reload buffer from disk
    fn reload(
        &mut self,
        cursor: &mut usize,
        selection: &mut Option<usize>,
        alt: Option<AltCursor<'_>>,
    ) -> std::io::Result<()> {
        let (saved, reloaded) = self.source.read_string(self.endings)?;
        patch_rope(&mut self.rope.get_mut(), reloaded, cursor, selection, alt);
        self.rope.save();
        self.saved = saved;
        if let Some(last) = self.undo.last_mut() {
            last.finished = true;
        }
        Ok(())
    }

    /// Attempts to save buffer to disk
    fn save(&mut self) -> std::io::Result<()> {
        self.saved = {
            // if the file is non-empty and doesn't end
            // with a newline, append one
            // (needs to be in its own block because we
            //  have to drop RopeHandle before saving)
            let mut rope = self.rope.get_mut();
            let len_chars = rope.len_chars();
            if let Some(last_char) = len_chars.checked_sub(1)
                && rope.get_char(last_char) != Some('\n')
            {
                rope.insert_char(len_chars, '\n');
            }
            self.source.save_data(&rope, self.endings)?
        };
        self.rope.save();
        if let Some(last) = self.undo.last_mut() {
            last.finished = true;
        }
        Ok(())
    }

    /// Total lines in buffer
    fn total_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Whether the buffer has been modified
    pub fn modified(&self) -> bool {
        self.rope.modified()
    }

    /// When the buffer was last modified, according to the filesystem
    pub fn last_modified(&self) -> Option<SystemTime> {
        self.source.last_modified()
    }

    /// When we last saved the buffer, if it can be known
    pub fn last_saved(&self) -> Option<SystemTime> {
        self.saved
    }
}

#[derive(Clone)]
pub struct BufferId(Rc<RefCell<Buffer>>);

impl Eq for BufferId {}

impl PartialEq for BufferId {
    fn eq(&self, rhs: &BufferId) -> bool {
        Rc::ptr_eq(&self.0, &rhs.0)
    }
}

pub enum WholeSelect {
    Word,
    Lines,
}

impl From<WholeSelect> for crate::help::Keybinding {
    fn from(mode: WholeSelect) -> Self {
        crate::help::ctrl_f(
            &["W"],
            "F9",
            match mode {
                WholeSelect::Word => "Select Word",
                WholeSelect::Lines => "Widen Selection to Lines",
            },
        )
    }
}

pub enum FindMode {
    WholeFile,
    Selected,
    InSelection,
}

impl From<FindMode> for crate::help::Keybinding {
    fn from(mode: FindMode) -> Self {
        crate::help::ctrl_f(
            &["T"],
            "F5",
            match mode {
                FindMode::WholeFile => "Find in File",
                FindMode::Selected => "Find Selected Text",
                FindMode::InSelection => "Find in Selected Lines",
            },
        )
    }
}

pub struct Help {
    select: WholeSelect,
    find: FindMode,
}

/// A buffer with additional context on a per-view basis
#[derive(Clone)]
pub struct BufferContext {
    buffer: private::BufferCell,    // the buffer we're wrapping
    viewport_height: usize,         // viewport's current height in lines
    cursor: usize,                  // cursor's absolute position in rope, in characters
    cursor_column: usize,           // cursor's desired column, as a display column
    selection: Option<usize>,       // cursor's text selection anchor
    message: Option<BufferMessage>, // some user-facing message
}

// moving the cursor vertically should preserve the cursor column
// even if the intervening lines are shorter
// (moving down then back up should always round-trip back to the same
// column, even if the next line is shorter)
// while horizontal movement or adding text updates the column
// to the current position

impl BufferContext {
    pub fn id(&self) -> BufferId {
        self.buffer.id()
    }

    pub fn modified(&self) -> bool {
        self.buffer.borrow().modified()
    }

    pub fn open(source: Source) -> std::io::Result<Self> {
        Buffer::open(source).map(|b| b.into())
    }

    pub fn reload(&mut self, alt: Option<AltCursor<'_>>) {
        let mut buf = self.buffer.borrow_mut();
        match buf.reload(&mut self.cursor, &mut self.selection, alt) {
            Ok(()) => {
                self.message = Some(BufferMessage::Notice("Reloaded".into()));
            }
            Err(err) => {
                self.message = Some(BufferMessage::Error(err.to_string().into()));
            }
        }
    }

    pub fn save(&mut self) {
        if let Err(err) = self.buffer.borrow_mut().save() {
            self.message = Some(BufferMessage::Error(err.to_string().into()));
        }
    }

    pub fn verified_save(&mut self) -> Result<(), Modified> {
        let mut buf = self.buffer.borrow_mut();
        if let Some(saved) = buf.last_saved()
            && let Some(modified) = buf.last_modified()
            && modified > saved
        {
            Err(Modified)
        } else {
            if let Err(err) = buf.save() {
                self.message = Some(BufferMessage::Error(err.to_string().into()));
            }
            Ok(())
        }
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor;
    }

    pub fn set_selection(&mut self, start: usize, end: usize) {
        assert!(end >= start);
        let buf = self.buffer.borrow();
        self.cursor = start;
        self.selection = Some(end);
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
    }

    /// Returns cursor position in rope as (row, col), if possible
    ///
    /// Both indexes start from 0
    ///
    /// This position is independent of the viewport position
    fn cursor_position(&self) -> Option<(usize, usize)> {
        use unicode_width::UnicodeWidthChar;

        let rope = &self.buffer.borrow().rope;
        let line = rope.try_char_to_line(self.cursor).ok()?;
        let line_start = rope.try_line_to_char(line).ok()?;

        Some((
            line,
            rope.chars_at(line_start)
                .take(self.cursor.checked_sub(line_start)?)
                .map(|c| match c {
                    '\t' => *SPACES_PER_TAB,
                    c => c.width().unwrap_or(0),
                })
                .sum(),
        ))
    }

    /// This is the inverse of cursor_position
    ///
    /// Given some mouse-selected position, attempt to place focus
    /// in the document where the cursor should be.
    fn set_cursor_focus(&mut self, area: Rect, position: Position) {
        use ratatui::{
            layout::{
                Constraint::{Length, Min},
                Layout,
            },
            widgets::{Block, Borders},
        };

        // rebuild layout from BufferWidget
        let [text_area, _] = Layout::horizontal([Min(0), Length(1)])
            .areas(Block::bordered().borders(Borders::TOP).inner(area));

        if !text_area.contains(position) {
            return;
        }

        let buffer = self.buffer.borrow();
        let rope = &buffer.rope;
        let row = position.y.saturating_sub(text_area.y);
        let col = position.x.saturating_sub(text_area.x);

        let current_line = rope.try_char_to_line(self.cursor).ok();

        let viewport_line: usize = current_line
            .map(|line| line.saturating_sub(self.viewport_height / 2))
            .unwrap_or(0);

        let line = viewport_line + usize::from(row);

        let starting_col = self
            .cursor_position()
            .map(|(_, col)| {
                col.saturating_sub(
                    text_area
                        .width
                        .saturating_sub(BufferWidget::RIGHT_MARGIN)
                        .into(),
                ) as u16
            })
            .unwrap_or(0);

        // the column we're aiming for, in onscreen characters
        let mut desired_col: usize = (starting_col + col).into();
        self.cursor_column = desired_col;

        let col_chars = rope
            .try_line_to_char(line)
            .map(|line_start| {
                rope.chars_at(line_start)
                    .take_while(|c| {
                        use unicode_width::UnicodeWidthChar;

                        desired_col = match desired_col.checked_sub(match c {
                            '\t' => *SPACES_PER_TAB,
                            c => c.width().unwrap_or(0),
                        }) {
                            Some(col) => col,
                            None => return false,
                        };
                        true
                    })
                    .count()
            })
            .unwrap_or(0);

        // ensure cursor doesn't walk past desired line
        self.cursor = (rope.try_line_to_char(line).unwrap_or(rope.len_chars()) + col_chars).min(
            rope.try_line_to_char(line + 1)
                .unwrap_or(rope.len_chars())
                .saturating_sub(1),
        );

        self.selection = None;
    }

    pub fn cursor_up(&mut self, lines: usize, selecting: bool) {
        let buf = self.buffer.borrow_move();
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
            let previous_line = current_line.saturating_sub(lines);
            if let Some((prev_start, prev_end)) = line_char_range(&buf.rope, previous_line) {
                update_selection(&mut self.selection, self.cursor, selecting);
                self.cursor =
                    apply_cursor_column(&buf.rope, self.cursor_column, prev_start, prev_end);
            }
        }
    }

    pub fn cursor_down(&mut self, lines: usize, selecting: bool) {
        let buf = self.buffer.borrow_move();
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor) {
            let next_line = (current_line + lines).min(buf.rope.len_lines().saturating_sub(1));
            if let Some((next_start, next_end)) = line_char_range(&buf.rope, next_line) {
                update_selection(&mut self.selection, self.cursor, selecting);
                self.cursor =
                    apply_cursor_column(&buf.rope, self.cursor_column, next_start, next_end);
            }
        }
    }

    pub fn cursor_back(&mut self, selecting: bool) {
        let buf = self.buffer.borrow_move();
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = self.cursor.saturating_sub(1);
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
    }

    pub fn cursor_forward(&mut self, selecting: bool) {
        let buf = self.buffer.borrow_move();
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = (self.cursor + 1).min(buf.rope.len_chars());
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
    }

    pub fn cursor_home(&mut self, selecting: bool) {
        let buf = self.buffer.borrow_move();
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor)
            && let Some((home, _)) = line_char_range(&buf.rope, current_line)
        {
            use unicode_width::UnicodeWidthChar;

            // Copies Nano's "smart home" behavior by
            // moving cursor to start of text or start of line,
            // depending on where we find it.

            let indent_char = if buf.tabs_required { '\t' } else { ' ' };

            update_selection(&mut self.selection, self.cursor, selecting);

            match line_chars(&buf.rope, self.cursor) {
                Some(iter) => {
                    let mut iter = iter.peekable();
                    let mut indent = home;
                    let mut cursor_column = 0;
                    while let Some(c) = iter.next_if(|c| *c == indent_char) {
                        indent += 1;
                        cursor_column += match c {
                            '\t' => *SPACES_PER_TAB,
                            c => c.width().unwrap_or(1),
                        };
                    }

                    if self.cursor == indent {
                        self.cursor = home;
                        self.cursor_column = 0;
                    } else {
                        self.cursor = indent;
                        self.cursor_column = cursor_column;
                    }
                }
                None => {
                    self.cursor = home;
                    self.cursor_column = 0;
                }
            }
        }
    }

    pub fn cursor_end(&mut self, selecting: bool) {
        let buf = self.buffer.borrow_move();
        if let Ok(current_line) = buf.rope.try_char_to_line(self.cursor)
            && let Some((_, end)) = line_char_range(&buf.rope, current_line)
        {
            update_selection(&mut self.selection, self.cursor, selecting);
            self.cursor = end;
            self.cursor_column = cursor_column(&buf.rope, self.cursor);
        }
    }

    pub fn last_line(&self) -> usize {
        self.buffer.borrow_mut().rope.len_lines().saturating_sub(1)
    }

    pub fn select_line(&mut self, line: usize) {
        let buf = self.buffer.borrow_move();
        match buf.rope.try_line_to_char(line) {
            Ok(cursor) => {
                self.cursor_column = 0;
                self.cursor = cursor;
                self.selection = None;
            }
            Err(_) => {
                self.message = Some(BufferMessage::Error("invalid line".into()));
            }
        }
    }

    pub fn select_line_and_column(&mut self, line: usize, column: usize) {
        let buf = self.buffer.borrow_move();
        if let Ok(line_start) = buf.rope.try_line_to_char(line)
            && let Ok(next_line_start) = buf.rope.try_line_to_char(line + 1)
        {
            self.cursor = (line_start + column).min(next_line_start.saturating_sub(1));
            self.cursor_column = cursor_column(&buf.rope, self.cursor);
            self.selection = None;
        } else {
            self.message = Some(BufferMessage::Error("invalid line".into()));
        }
    }

    pub fn insert_char(&mut self, alt: Option<AltCursor<'_>>, c: char) {
        use unicode_width::UnicodeWidthChar;

        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();

        match &mut self.selection {
            Some(selection) => match c {
                '(' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    ['(', ')'],
                ),
                '[' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    ['[', ']'],
                ),
                '{' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    ['{', '}'],
                ),
                '<' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    ['<', '>'],
                ),
                '\"' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    ['\"', '\"'],
                ),
                '\'' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    ['\'', '\''],
                ),
                _ => {
                    let mut alt = Secondary::new(alt, |a| a >= self.cursor.min(*selection));
                    zap_selection(
                        &mut rope,
                        &mut self.cursor,
                        &mut self.cursor_column,
                        *selection,
                        &mut alt,
                    );
                    self.selection = None;
                    rope.insert_char(self.cursor, c);
                    self.cursor += 1;
                    self.cursor_column += c.width().unwrap_or(1);
                    alt += 1;
                }
            },
            None => {
                rope.insert_char(self.cursor, c);
                self.cursor += 1;
                self.cursor_column += c.width().unwrap_or(1);
                Secondary::new(alt, |a| a >= self.cursor).update(|pos| *pos += 1);
            }
        }
    }

    pub fn paste(&mut self, alt: Option<AltCursor<'_>>, cut_buffer: &mut Option<CutBuffer>) {
        match self.selection.as_mut() {
            None => {
                if let Some(pasted) = cut_buffer {
                    // No active selection, so paste as-is
                    let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
                    let mut rope = buf.rope.get_mut();
                    let mut alt = Secondary::new(alt, |a| a >= self.cursor);
                    if rope.try_insert(self.cursor, &pasted.data).is_ok() {
                        self.cursor += pasted.chars_len;
                        alt += pasted.chars_len;
                        self.cursor_column = cursor_column(&rope, self.cursor);
                    }
                }
            }
            Some(selection) => {
                if let Some(pasted) = cut_buffer {
                    let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
                    let (selection_start, selection_end) = reorder(self.cursor, *selection);
                    let cut_range = selection_start..selection_end;
                    let mut rope = buf.rope.get_mut();
                    let mut alt = Secondary::new(alt, |a| a >= selection_start);

                    if let Some(cut) = rope.get_slice(cut_range.clone()).map(|slice| slice.into()) {
                        // cut out part of rope we want
                        rope.remove(cut_range.clone());
                        alt.update(|pos| {
                            if (cut_range.clone()).contains(pos) {
                                *pos = selection_start;
                            } else {
                                *pos -= selection_end - selection_start;
                            }
                        });
                        self.cursor = selection_start;

                        // insert contents of cut buffer
                        // and transfer cut rope into cut buffer
                        let pasted = std::mem::replace(pasted, cut);
                        if rope.try_insert(self.cursor, &pasted.data).is_ok() {
                            alt += pasted.chars_len;
                            self.selection = Some(selection_start);
                            self.cursor = selection_start + pasted.chars_len;
                            self.cursor_column = cursor_column(&rope, self.cursor);
                        }

                        // display indicator
                        self.message = Some(BufferMessage::Notice(
                            "swapped cut buffer with selection".into(),
                        ));
                    }
                }
            }
        }
    }

    pub fn newline(&mut self, alt: Option<AltCursor<'_>>) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let indent_char = if buf.tabs_required { '\t' } else { ' ' };
        let mut rope = buf.rope.get_mut();

        let mut alt = match self.selection.take() {
            Some(selection) => {
                let mut secondary = Secondary::new(alt, |a| a >= self.cursor.min(selection));

                zap_selection(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    &mut secondary,
                );

                secondary
            }
            None => Secondary::new(alt, |a| a >= self.cursor),
        };

        let (indent, all_indent) = match line_start_to_cursor(&rope, self.cursor) {
            Some(iter) => {
                let mut iter = iter.peekable();
                let mut indent = 0;
                while iter.next_if(|c| *c == indent_char).is_some() {
                    indent += 1;
                }
                (indent, iter.next().is_none())
            }
            None => (0, false),
        };

        // if the whole line is indent, insert newline *before* indent
        // instead of adding a fresh indentation
        if all_indent {
            rope.insert_char(self.cursor - indent, '\n');
            self.cursor += 1;
            alt += 1;
        } else {
            rope.insert_char(self.cursor, '\n');
            self.cursor += 1;
            alt += 1;
            self.cursor_column = 0;
            for _ in 0..indent {
                rope.insert_char(self.cursor, indent_char);
                self.cursor += 1;
                alt += 1;
                self.cursor_column += 1;
            }
        }
    }

    pub fn backspace(&mut self, alt: Option<AltCursor<'_>>) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();

        match self.selection.take() {
            None => {
                let mut alt = Secondary::new(alt, |a| a >= self.cursor);
                if let Some(prev) = self.cursor.checked_sub(1)
                    && rope.try_remove(prev..self.cursor).is_ok()
                {
                    alt -= 1;
                    self.cursor -= 1;
                    self.cursor_column = cursor_column(&rope, self.cursor);
                }
            }
            Some(current_selection) => {
                let mut alt = Secondary::new(alt, |a| a >= self.cursor.min(current_selection));
                zap_selection(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    current_selection,
                    &mut alt,
                );
            }
        }
    }

    pub fn delete(&mut self, alt: Option<AltCursor<'_>>) {
        let buf = &mut self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();

        match &mut self.selection {
            None => {
                let mut alt = Secondary::new(alt, |a| a > self.cursor);
                if rope.try_remove(self.cursor..(self.cursor + 1)).is_ok() {
                    alt -= 1;
                }
                // leave our cursor position and current column unchanged
            }
            Some(selection) => {
                if let Err(mut alt) = delete_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                ) {
                    zap_selection(
                        &mut rope,
                        &mut self.cursor,
                        &mut self.cursor_column,
                        *selection,
                        &mut alt,
                    );
                    self.selection = None;
                }
            }
        }
    }

    /// Returns selection without clearing it, if any
    pub fn selection_range(&self) -> Option<SelectionType> {
        let (selection_start, selection_end) = reorder(self.cursor, self.selection?);

        if selection_start == selection_end {
            return None;
        }

        let buf = self.buffer.borrow();
        let rope = &buf.rope;

        let start_line = rope.try_char_to_line(selection_start).ok()?;
        let end_line = rope.try_char_to_line(selection_end).ok()?;
        if start_line == end_line {
            rope.get_slice(selection_start..selection_end)
                .map(|r| SelectionType::Term(r.into()))
        } else {
            Some(SelectionType::Range(SelectionRange {
                start: start_line,
                lines: NonZero::new((end_line - start_line) + 1)?,
            }))
        }
    }

    pub fn get_selection(&mut self) -> Option<CutBuffer> {
        let selection = self.selection.take()?;
        let (selection_start, selection_end) = reorder(self.cursor, selection);
        self.buffer
            .borrow()
            .rope
            .get_slice(selection_start..selection_end)
            .map(|r| r.into())
    }

    pub fn take_selection(&mut self, alt: Option<AltCursor<'_>>) -> Option<CutBuffer> {
        let selection = self.selection.take()?;
        let (selection_start, selection_end) = reorder(self.cursor, selection);
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        let mut alt = Secondary::new(alt, |a| a >= selection_start);

        rope.get_slice(selection_start..selection_end)
            .map(|r| r.into())
            .inspect(|_| {
                rope.remove(selection_start..selection_end);
                self.cursor = selection_start;
                self.cursor_column = cursor_column(&rope, self.cursor);
                alt.update(|pos| {
                    if (selection_start..selection_end).contains(pos) {
                        *pos = selection_start;
                    } else {
                        *pos -= selection_end - selection_start;
                    }
                });
            })
    }

    /// Returns Ok((current_idx, matches)) on success
    /// Returns Err(term) if no matches found
    pub fn all_matches<'s, S: SearchTerm<'s>>(
        &mut self,
        range: Option<&SelectionRange>,
        term: S,
    ) -> Result<(usize, Vec<(Range<usize>, Vec<Option<MatchCapture>>)>), S> {
        let buf = self.buffer.borrow_move();
        let rope = &buf.rope;

        let matches = search_area(rope, range)
            .flat_map(|(line, offset)| {
                term.match_ranges(&line)
                    .map(|m| m + offset)
                    .collect::<Vec<_>>()
            })
            .filter_map(
                |SearchMatch {
                     start: s,
                     end: e,
                     groups: c,
                 }| {
                    // convert ranges in bytes (from SearchTerm)
                    // to ranges in characters (for Ropey)
                    Some((
                        rope.try_byte_to_char(s).ok()?..rope.try_byte_to_char(e).ok()?,
                        c.into_iter()
                            // if None, keep it None,
                            // otherwise filter out any bad conversions
                            // (which shouldn't happen, really)
                            .filter_map(|m| match m {
                                Some(MatchCapture { start, end, string }) => {
                                    let start_chars = rope.try_byte_to_char(start).ok()?;
                                    let end_chars = rope.try_byte_to_char(end).ok()?;
                                    Some(Some(MatchCapture {
                                        start: start_chars,
                                        end: end_chars,
                                        string,
                                    }))
                                }
                                None => Some(None),
                            })
                            .collect(),
                    ))
                },
            )
            .collect::<Vec<_>>();

        let start = match self.selection {
            Some(selection) => selection.min(self.cursor),
            None => self.cursor,
        };

        let (idx, (next_match, _)) = matches
            .iter()
            .enumerate()
            .find(|(_, (m, _))| m.start >= start)
            .or_else(|| matches.first().map(|m| (0, m)))
            .ok_or(term)?;
        self.cursor = next_match.start;
        self.selection = Some(next_match.end);
        Ok((idx, matches))
    }

    pub fn perform_undo(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        match buf.undo.pop() {
            Some(Undo { mut state, .. }) => {
                use std::ops::DerefMut;
                std::mem::swap(buf.rope.get_mut().deref_mut(), &mut state.rope);
                std::mem::swap(&mut self.cursor, &mut state.cursor);
                std::mem::swap(&mut self.cursor_column, &mut state.cursor_column);
                buf.redo.push(state);
                self.selection = None;
            }
            None => {
                self.message = Some(BufferMessage::Notice("nothing left to undo".into()));
            }
        }
    }

    pub fn perform_redo(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        match buf.redo.pop() {
            Some(mut state) => {
                use std::ops::DerefMut;
                std::mem::swap(buf.rope.get_mut().deref_mut(), &mut state.rope);
                std::mem::swap(&mut self.cursor, &mut state.cursor);
                std::mem::swap(&mut self.cursor_column, &mut state.cursor_column);
                buf.undo.push(Undo {
                    state,
                    finished: true,
                });
                self.selection = None;
            }
            None => {
                self.message = Some(BufferMessage::Notice("nothing left to redo".into()));
            }
        }
    }

    pub fn indent(&mut self, alt: Option<AltCursor<'_>>) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let indent = match buf.tabs_required {
            false => buf.tab_substitution.clone(),
            true => "\t".to_string(),
        };

        match self.selection {
            None => {
                let mut alt = Secondary::new(alt, |a| a >= self.cursor);
                let mut rope = buf.rope.get_mut();
                if let Ok(line_start) = rope
                    .try_char_to_line(self.cursor)
                    .and_then(|line| rope.try_line_to_char(line))
                {
                    rope.insert(line_start, &indent);
                    self.cursor += indent.len();
                    alt += indent.len();
                }
            }
            selection_opt @ Some(selection) => {
                let (start, end) = reorder(self.cursor, selection);
                let mut alt = Secondary::new(alt, |a| a >= start);
                let mut rope = buf.rope.get_mut();
                let indent_lines = selected_lines(&rope, self.cursor, selection_opt)
                    .filter(|l| l.end > l.start)
                    .collect::<Vec<_>>();

                for SelectedLine { start, .. } in indent_lines.iter().rev() {
                    rope.insert(*start, &indent);
                }

                self.selection = indent_lines.first().map(|l| l.start);
                self.cursor = indent_lines
                    .last()
                    .map(|l| l.end + (indent.len() * indent_lines.len()))
                    .unwrap_or(0);

                alt.update(|pos| {
                    if (start..end).contains(pos) {
                        *pos = self.cursor;
                    } else {
                        *pos += indent.len() * indent_lines.len()
                    }
                });
            }
        }
    }

    pub fn un_indent(&mut self, alt: Option<AltCursor<'_>>) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let indent = match buf.tabs_required {
            false => buf.tab_substitution.clone(),
            true => "\t".to_string(),
        };

        match self.selection {
            None => {
                let mut alt = Secondary::new(alt, |a| a >= self.cursor);

                if let Some(line_start) = buf
                    .rope
                    .try_char_to_line(self.cursor)
                    .ok()
                    .and_then(|line| buf.rope.try_line_to_char(line).ok())
                    && buf
                        .rope
                        .chars_at(line_start)
                        .take(indent.len())
                        .eq(indent.chars())
                {
                    let mut rope = buf.rope.get_mut();
                    rope.remove(line_start..line_start + indent.len());
                    self.cursor = line_start;
                    self.cursor_column = 0;
                    alt.update(|pos| {
                        if (line_start..line_start + indent.len()).contains(pos) {
                            *pos = line_start;
                        } else {
                            *pos -= indent.len();
                        }
                    });
                }
            }
            selection_opt @ Some(selection) => {
                let (start, end) = reorder(self.cursor, selection);
                let mut alt = Secondary::new(alt, |a| a >= self.cursor);

                let unindent_lines = selected_lines(&buf.rope, self.cursor, selection_opt)
                    .filter(|l| l.end > l.start)
                    .collect::<Vec<_>>();

                // un-indent whole selection as a unit
                // so long as each non-empty line has the proper amount
                // of prefixed spaces
                if unindent_lines.iter().all(|SelectedLine { start, .. }| {
                    buf.rope
                        .chars_at(*start)
                        .take(indent.len())
                        .eq(indent.chars())
                }) {
                    let mut rope = buf.rope.get_mut();

                    for line in unindent_lines.iter().rev() {
                        rope.remove(line.start..line.start + indent.len());
                    }

                    self.selection = unindent_lines.first().map(|l| l.start);
                    self.cursor = unindent_lines
                        .last()
                        .map(|l| l.end - (unindent_lines.len() * indent.len()))
                        .unwrap_or(0);

                    alt.update(|pos| {
                        if (start..end).contains(pos) {
                            *pos = self.cursor;
                        } else {
                            *pos = pos.saturating_sub(indent.len() * unindent_lines.len());
                        }
                    });
                }
            }
        }
    }

    pub fn select_inside(&mut self, (start, end): (char, char), stack: Option<(char, char)>) {
        let buf = self.buffer.borrow();
        let (stack_back, stack_forward) = match stack {
            Some((back, forward)) => (Some(back), Some(forward)),
            None => (None, None),
        };

        match self.selection {
            Some(selection) => {
                let (sel_start, sel_end) = reorder(self.cursor, selection);
                if let (Some(start), Some(end)) = (
                    select_next_char::<false>(&buf.rope, sel_start, start, stack_back),
                    select_next_char::<true>(&buf.rope, sel_end, end, stack_forward),
                ) {
                    self.selection = Some(start);
                    self.cursor = end;
                }
            }
            None => {
                if let (Some(start), Some(end)) = (
                    select_next_char::<false>(&buf.rope, self.cursor, start, stack_back),
                    select_next_char::<true>(&buf.rope, self.cursor, end, stack_forward),
                ) {
                    self.selection = Some(start);
                    self.cursor = end;
                }
            }
        }
    }

    pub fn cursor_to_selection_start(&mut self) {
        let buf = self.buffer.borrow_move();
        if let Some(selection) = &mut self.selection
            && self.cursor > *selection
        {
            std::mem::swap(selection, &mut self.cursor);
            self.cursor_column = cursor_column(&buf.rope, self.cursor);
        }
    }

    pub fn cursor_to_selection_end(&mut self) {
        let buf = self.buffer.borrow_move();
        if let Some(selection) = &mut self.selection
            && self.cursor < *selection
        {
            std::mem::swap(selection, &mut self.cursor);
            self.cursor_column = cursor_column(&buf.rope, self.cursor);
        }
    }

    pub fn select_matching_paren(&mut self) {
        let buf = self.buffer.borrow_move();

        if let Some(new_pos) = buf.rope.get_char(self.cursor).and_then(|c| match c {
            '(' => select_next_char::<true>(&buf.rope, self.cursor + 1, ')', Some('(')),
            ')' => select_next_char::<false>(&buf.rope, self.cursor, '(', Some(')'))
                .map(|c| c.saturating_sub(1)),
            '{' => select_next_char::<true>(&buf.rope, self.cursor + 1, '}', Some('{')),
            '}' => select_next_char::<false>(&buf.rope, self.cursor, '{', Some('}'))
                .map(|c| c.saturating_sub(1)),
            '[' => select_next_char::<true>(&buf.rope, self.cursor + 1, ']', Some('[')),
            ']' => select_next_char::<false>(&buf.rope, self.cursor, '[', Some(']'))
                .map(|c| c.saturating_sub(1)),
            '<' => select_next_char::<true>(&buf.rope, self.cursor + 1, '>', Some('<')),
            '>' => select_next_char::<false>(&buf.rope, self.cursor, '<', Some('>'))
                .map(|c| c.saturating_sub(1)),
            _ => None,
        }) {
            self.cursor = new_pos;
            self.selection = None;
        }
    }

    /// Attempts to auto pair set, returning Ok if successful
    pub fn try_auto_pair(&mut self) -> Result<(), ()> {
        let buf = self.buffer.borrow();
        let rope = &buf.rope;
        let (start, end) = match self.selection {
            Some(selection) => reorder(self.cursor, selection),
            None => (self.cursor, self.cursor),
        };
        let start = start.checked_sub(1).ok_or(())?;

        match match (rope.get_char(start), rope.get_char(end)) {
            (Some('('), Some(')'))
            | (Some('['), Some(']'))
            | (Some('{'), Some('}'))
            | (Some('<'), Some('>'))
            | (Some('"'), Some('"'))
            | (Some('\''), Some('\'')) => Some((start, end + 1)),
            (_, Some(')')) => prev_pairing_char(rope, start)
                .and_then(|(c, start)| (c == '(').then_some((start, end))),
            (Some('('), _) => next_pairing_char(rope, end)
                .and_then(|(c, end)| (c == ')').then_some((start + 1, end))),
            (_, Some(']')) => prev_pairing_char(rope, start)
                .and_then(|(c, start)| (c == '[').then_some((start, end))),
            (Some('['), _) => next_pairing_char(rope, end)
                .and_then(|(c, end)| (c == ']').then_some((start + 1, end))),
            (_, Some('}')) => prev_pairing_char(rope, start)
                .and_then(|(c, start)| (c == '{').then_some((start, end))),
            (Some('{'), _) => next_pairing_char(rope, end)
                .and_then(|(c, end)| (c == '}').then_some((start + 1, end))),
            (_, Some('>')) => prev_pairing_char(rope, start)
                .and_then(|(c, start)| (c == '<').then_some((start, end))),
            (Some('<'), _) => next_pairing_char(rope, end)
                .and_then(|(c, end)| (c == '>').then_some((start + 1, end))),
            (_, Some('"')) => prev_pairing_char(rope, start)
                .and_then(|(c, start)| (c == '"').then_some((start, end))),
            (Some('"'), _) => next_pairing_char(rope, end)
                .and_then(|(c, end)| (c == '"').then_some((start + 1, end))),
            (_, Some('\'')) => prev_pairing_char(rope, start)
                .and_then(|(c, start)| (c == '\'').then_some((start, end))),
            (Some('\''), _) => next_pairing_char(rope, end)
                .and_then(|(c, end)| (c == '\'').then_some((start + 1, end))),
            _ => match (
                prev_pairing_char(rope, start),
                next_pairing_char(rope, end + 1),
            ) {
                (Some(('(', start)), Some((')', end)))
                | (Some(('[', start)), Some((']', end)))
                | (Some(('{', start)), Some(('}', end)))
                | (Some(('<', start)), Some(('>', end)))
                | (Some(('"', start)), Some(('"', end)))
                | (Some(('\'', start)), Some(('\'', end))) => Some((start, end)),
                _ => None,
            },
        } {
            Some((start, end)) => {
                self.cursor = end;
                self.selection = Some(start);
                self.cursor_column = cursor_column(rope, self.cursor);
                Ok(())
            }
            None => Err(()),
        }
    }

    pub fn select_word_or_lines(&mut self) {
        let buf = &mut self.buffer.borrow_move();
        let rope = &buf.rope;

        match self.selection {
            None => {
                // no selection
                match rope.get_char(self.cursor) {
                    Some(c) if is_word(c) => {
                        // widen selection to current word
                        if let Some(word_start) = rope
                            .chars_at(self.cursor)
                            .reversed()
                            .position(|c| !is_word(c))
                            .and_then(|pos| self.cursor.checked_sub(pos))
                            && let Some(word_end) = rope
                                .chars_at(self.cursor)
                                .position(|c| !is_word(c))
                                .map(|pos| self.cursor + pos)
                        {
                            self.selection = Some(word_start);
                            self.cursor = word_end;
                            self.cursor_column = cursor_column(rope, self.cursor);
                        }
                    }
                    _ => {
                        // select current line
                        if let Some((start, end)) = rope
                            .try_char_to_line(self.cursor)
                            .ok()
                            .and_then(|line| line_char_range(rope, line))
                        {
                            self.selection = Some(start);
                            self.cursor = end;
                            self.cursor_column = cursor_column(rope, self.cursor);
                        }
                    }
                }
            }
            Some(selection) => {
                // widen start and end of selection to line boundaries
                if selection < self.cursor {
                    // selection to start of line, cursor to end of line
                    if let Some(start) = rope
                        .try_char_to_line(selection)
                        .ok()
                        .and_then(|line| rope.try_line_to_char(line).ok())
                        && let Some(end) = rope
                            .try_char_to_line(self.cursor)
                            .ok()
                            .and_then(|line| rope.try_line_to_char(line + 1).ok())
                    {
                        self.selection = Some(start);
                        self.cursor = end - 1;
                    }
                } else {
                    // cursor to start of line, selection to end of line
                    if let Some(start) = rope
                        .try_char_to_line(self.cursor)
                        .ok()
                        .and_then(|line| rope.try_line_to_char(line).ok())
                        && let Some(end) = rope
                            .try_char_to_line(selection)
                            .ok()
                            .and_then(|line| rope.try_line_to_char(line + 1).ok())
                    {
                        self.cursor = start;
                        self.selection = Some(end - 1);
                    }
                }
            }
        }
    }

    pub fn clear_matches<P>(
        &mut self,
        alt: Option<AltCursor<'_>>,
        mut matches: &mut [(Range<usize>, P)],
    ) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        let mut alt = Secondary::new(alt, |_| true);

        loop {
            match matches {
                [] => break,
                [(r, _)] => {
                    let _ = rope.try_remove(r.clone());
                    if r.start <= self.cursor {
                        self.cursor = self
                            .cursor
                            .saturating_sub((r.end - r.start).min(self.cursor - r.start));
                    }
                    alt.update(|cursor| {
                        if r.start <= *cursor {
                            *cursor =
                                cursor.saturating_sub((r.end - r.start).min(*cursor - r.start));
                        }
                    });
                    break;
                }
                [(r, _), rest @ ..] => {
                    let len = r.end - r.start;
                    let _ = rope.try_remove(r.clone());
                    if r.start <= self.cursor {
                        self.cursor = self.cursor.saturating_sub(len.min(self.cursor - r.start));
                    }
                    alt.update(|cursor| {
                        if r.start <= *cursor {
                            *cursor = cursor.saturating_sub(len.min(*cursor - r.start));
                        }
                    });
                    for (r, _) in rest.iter_mut() {
                        r.start -= len;
                        r.end -= len;
                    }
                    matches = rest;
                }
            }
        }
        self.selection = None;
    }

    pub fn multi_insert_char(
        &mut self,
        alt: Option<AltCursor<'_>>,
        mut matches: &mut [MultiCursor],
        c: char,
    ) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        let mut alt = Secondary::new(alt, |_| true);

        loop {
            match matches {
                [] => break,
                [m] => {
                    m.insert_char(&mut rope, &mut self.cursor, &mut alt, c);
                    break;
                }
                [m, rest @ ..] => {
                    m.insert_char(&mut rope, &mut self.cursor, &mut alt, c);
                    for r in rest.iter_mut() {
                        *r += 1;
                    }
                    matches = rest;
                }
            }
        }
    }

    pub fn multi_insert_string(
        &mut self,
        alt: Option<AltCursor<'_>>,
        matches: &mut [MultiCursor],
        s: &str,
    ) {
        self.multi_insert_strings(alt, matches, std::iter::repeat((s.chars().count(), s)))
    }

    pub fn multi_insert_strings<'s>(
        &mut self,
        alt: Option<AltCursor<'_>>,
        mut matches: &mut [MultiCursor],
        mut strings: impl Iterator<Item = (usize, &'s str)>,
    ) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        let mut alt = Secondary::new(alt, |_| true);

        loop {
            match matches {
                [] => break,
                [m] => {
                    let Some((s_len, s)) = strings.next() else {
                        return;
                    };
                    m.insert_str(&mut rope, &mut self.cursor, &mut alt, s, s_len);
                    break;
                }
                [m, rest @ ..] => {
                    let Some((s_len, s)) = strings.next() else {
                        return;
                    };
                    m.insert_str(&mut rope, &mut self.cursor, &mut alt, s, s_len);
                    for r in rest.iter_mut() {
                        *r += s_len;
                    }
                    matches = rest;
                }
            }
        }
    }

    pub fn multi_backspace(&mut self, alt: Option<AltCursor<'_>>, mut matches: &mut [MultiCursor]) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        let mut alt = Secondary::new(alt, |_| true);

        loop {
            match matches {
                [] => break,
                [m] => {
                    // don't worry if backspace unsuccessful
                    let _ = m.backspace(&mut rope, &mut self.cursor, &mut alt);
                    break;
                }
                [m, rest @ ..] => {
                    if let Ok(()) = m.backspace(&mut rope, &mut self.cursor, &mut alt) {
                        for r in rest.iter_mut() {
                            *r -= 1;
                        }
                    }
                    matches = rest;
                }
            }
        }
    }

    pub fn multi_delete(&mut self, alt: Option<AltCursor<'_>>, mut matches: &mut [MultiCursor]) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let mut rope = buf.rope.get_mut();
        let mut alt = Secondary::new(alt, |_| true);

        loop {
            match matches {
                [] => break,
                [m] => {
                    // don't worry if delete unsuccessful
                    let _ = m.delete(&mut rope, &mut self.cursor, &mut alt);
                    break;
                }
                [m, rest @ ..] => {
                    if let Ok(()) = m.delete(&mut rope, &mut self.cursor, &mut alt) {
                        for r in rest.iter_mut() {
                            *r -= 1;
                        }
                    }
                    matches = rest;
                }
            }
        }
    }

    pub fn multi_cursor_back(&mut self, matches: &mut [MultiCursor]) {
        matches
            .iter_mut()
            .for_each(|m| m.cursor_back(&mut self.cursor));
    }

    pub fn multi_cursor_forward(&mut self, matches: &mut [MultiCursor]) {
        matches
            .iter_mut()
            .for_each(|m| m.cursor_forward(&mut self.cursor));
    }

    pub fn set_error<S: Into<Cow<'static, str>>>(&mut self, err: S) {
        self.message = Some(BufferMessage::Error(err.into()))
    }

    pub fn alt_cursor(&mut self) -> AltCursor<'_> {
        AltCursor {
            cursor: &mut self.cursor,
            selection: &mut self.selection,
        }
    }

    pub fn find_mode(&self) -> Help {
        let rope = &self.buffer.borrow().rope;

        match self.selection {
            Some(selection) => Help {
                select: WholeSelect::Lines,
                find: if rope.try_char_to_line(self.cursor).ok()
                    == rope.try_char_to_line(selection).ok()
                {
                    FindMode::Selected
                } else {
                    FindMode::InSelection
                },
            },
            None => Help {
                select: match rope.get_char(self.cursor) {
                    Some(c) if is_word(c) => WholeSelect::Word,
                    _ => WholeSelect::Lines,
                },
                find: FindMode::WholeFile,
            },
        }
    }
}

pub struct AltCursor<'b> {
    cursor: &'b mut usize,
    selection: &'b mut Option<usize>,
}

/// A secondary cursor which implements various math operations
// struct Secondary<'b>(Option<AltCursor<'b>>);
struct Secondary<'b> {
    cursor: Option<&'b mut usize>,
    selection: Option<&'b mut usize>,
}

impl<'b> Secondary<'b> {
    /// Takes some optional alternative cursor
    /// and a conditional which takes that cursor's position and
    /// returns true if the secondary cursor should be manipulated
    /// and returns ourself, which implements necessary math operations.
    fn new(alt: Option<AltCursor<'b>>, mut f: impl FnMut(usize) -> bool) -> Self {
        match alt {
            Some(alt) => Self {
                cursor: f(*alt.cursor).then_some(alt.cursor),
                selection: alt.selection.as_mut().filter(|s| f(**s)),
            },
            None => Self {
                cursor: None,
                selection: None,
            },
        }
    }

    /// Updates secondary cursor in-place, if available
    fn update(&mut self, mut f: impl FnMut(&mut usize)) {
        if let Some(cursor) = &mut self.cursor {
            f(cursor)
        }
        if let Some(selection) = &mut self.selection {
            f(selection)
        }
    }
}

impl std::ops::AddAssign<usize> for Secondary<'_> {
    fn add_assign(&mut self, rhs: usize) {
        self.update(|c| {
            *c += rhs;
        })
    }
}

impl std::ops::SubAssign<usize> for Secondary<'_> {
    fn sub_assign(&mut self, rhs: usize) {
        self.update(|c| {
            *c -= rhs;
        })
    }
}

pub struct MultiCursor {
    /// cursor's range within rope, in characters
    range: Range<usize>,
    /// cursor's position in rope, in characters
    cursor: usize,
}

impl MultiCursor {
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn insert_char(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
        c: char,
    ) {
        if self.cursor <= *cursor {
            *cursor += 1;
        }
        secondary.update(|a| {
            if self.cursor <= *a {
                *a += 1;
            }
        });
        rope.insert_char(self.cursor, c);
        self.cursor += 1;
        self.range.end += 1;
    }

    fn insert_str(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
        s: &str,
        s_len: usize,
    ) {
        if self.cursor <= *cursor {
            *cursor += s_len;
        }
        secondary.update(|a| {
            if self.cursor <= *a {
                *a += s_len;
            }
        });
        rope.insert(self.cursor, s);
        self.cursor += s_len;
        self.range.end += s_len;
    }

    /// Returns Ok if backspace performed successfully
    fn backspace(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
    ) -> Result<(), ()> {
        if self.cursor > self.range.start {
            if self.cursor <= *cursor {
                *cursor = cursor.saturating_sub(1);
            }
            secondary.update(|a| {
                if self.cursor <= *a {
                    *a = a.saturating_sub(1);
                }
            });
            let _ = rope.try_remove((self.cursor - 1)..self.cursor);
            self.cursor -= 1;
            self.range.end -= 1;
            Ok(())
        } else {
            Err(())
        }
    }

    /// Returns Ok if delete performed successfully
    fn delete(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
    ) -> Result<(), ()> {
        if self.cursor < self.range.end {
            if self.cursor < *cursor {
                *cursor = cursor.saturating_sub(1);
            }
            secondary.update(|a| {
                if self.cursor < *a {
                    *a = a.saturating_sub(1);
                }
            });
            let _ = rope.try_remove(self.cursor..self.cursor + 1);
            self.range.end -= 1;
            Ok(())
        } else {
            Err(())
        }
    }

    fn cursor_back(&mut self, cursor: &mut usize) {
        if self.cursor > self.range.start {
            if self.cursor == *cursor {
                *cursor = cursor.saturating_sub(1);
            }
            self.cursor -= 1;
        }
    }

    fn cursor_forward(&mut self, cursor: &mut usize) {
        if self.cursor < self.range.end {
            if self.cursor == *cursor {
                *cursor += 1;
            }
            self.cursor += 1;
        }
    }
}

impl From<usize> for MultiCursor {
    fn from(cursor: usize) -> Self {
        Self {
            range: cursor..cursor,
            cursor,
        }
    }
}

impl std::ops::AddAssign<usize> for MultiCursor {
    fn add_assign(&mut self, chars: usize) {
        self.range.start += chars;
        self.range.end += chars;
        self.cursor += chars;
    }
}

impl std::ops::SubAssign<usize> for MultiCursor {
    fn sub_assign(&mut self, chars: usize) {
        self.range.start -= chars;
        self.range.end -= chars;
        self.cursor -= chars;
    }
}

pub enum SelectionType {
    Term(String),
    Range(SelectionRange),
}

pub struct SelectionRange {
    start: usize,
    lines: NonZero<usize>,
}

pub trait SearchTerm<'s>: std::fmt::Display {
    /// Returns iterator of match ranges in bytes and any captured groups
    fn match_ranges(&self, s: &str) -> impl Iterator<Item = SearchMatch>;
}

pub struct MatchCapture {
    pub string: String,
    start: usize,
    end: usize,
}

impl std::ops::AddAssign<usize> for MatchCapture {
    fn add_assign(&mut self, rhs: usize) {
        self.start += rhs;
        self.end += rhs;
    }
}

pub struct SearchMatch {
    start: usize,
    end: usize,
    groups: Vec<Option<MatchCapture>>,
}

impl std::ops::Add<usize> for SearchMatch {
    type Output = Self;

    fn add(mut self, rhs: usize) -> Self {
        self.groups.iter_mut().for_each(|m| {
            if let Some(m) = m {
                *m += rhs;
            }
        });
        Self {
            start: self.start + rhs,
            end: self.end + rhs,
            groups: self.groups,
        }
    }
}

impl SearchTerm<'static> for fancy_regex::Regex {
    fn match_ranges(&self, s: &str) -> impl Iterator<Item = SearchMatch> {
        self.captures_iter(s).filter_map(|c| c.ok()).map(|c| {
            // guaranteed to have at least one capture
            let first = c.get(0).unwrap();
            SearchMatch {
                start: first.start(),
                end: first.end(),
                groups: c
                    .iter()
                    .map(|m| {
                        m.map(|m| MatchCapture {
                            string: m.as_str().to_string(),
                            start: m.start(),
                            end: m.end(),
                        })
                    })
                    .collect(),
            }
        })
    }
}

impl SearchTerm<'static> for String {
    fn match_ranges(&self, s: &str) -> impl Iterator<Item = SearchMatch> {
        s.match_indices(self.as_str()).map(|(idx, s)| SearchMatch {
            start: idx,
            end: idx + s.len(),
            groups: vec![],
        })
    }
}

/// Buffer has been modified since last save
pub struct Modified;

// Given line in rope, returns (start, end) of that line in characters from start of rope
fn line_char_range(rope: &ropey::Rope, line: usize) -> Option<(usize, usize)> {
    Some((
        rope.try_line_to_char(line).ok()?,
        rope.try_line_to_char(line + 1).ok()?.saturating_sub(1),
    ))
}

struct SelectedLine {
    start: usize,
    end: usize,
}

// Iterates over position ranges of all selected lines
//
// If no selection, yields current line's position ranges
fn selected_lines(
    rope: &ropey::Rope,
    cursor: usize,
    selection: Option<usize>,
) -> Box<dyn DoubleEndedIterator<Item = SelectedLine> + '_> {
    match selection {
        // select current line
        None => match rope.try_char_to_line(cursor) {
            Ok(line) => Box::new(
                line_char_range(rope, line)
                    .map(|(start, end)| SelectedLine { start, end })
                    .into_iter(),
            ),
            Err(_) => Box::new(std::iter::empty()),
        },
        Some(selection) => {
            let (start, end) = reorder(cursor, selection);
            if let Ok(start_line) = rope.try_char_to_line(start)
                && let Ok(end_line) = rope.try_char_to_line(end)
            {
                Box::new((start_line..=end_line).filter_map(move |line| {
                    line_char_range(rope, line).map(|(start, end)| SelectedLine { start, end })
                }))
            } else {
                Box::new(std::iter::empty())
            }
        }
    }
}

// Given cursor position from start of rope,
// return that cursor's column in line
fn cursor_column(rope: &ropey::Rope, cursor: usize) -> usize {
    use unicode_width::UnicodeWidthChar;

    rope.try_char_to_line(cursor)
        .ok()
        .and_then(|line| rope.try_line_to_char(line).ok())
        .map(|line_start| {
            rope.chars_at(line_start)
                .take(cursor.saturating_sub(line_start))
                .map(|c| match c {
                    '\t' => *SPACES_PER_TAB,
                    c => c.width().unwrap_or(1),
                })
                .sum()
        })
        .unwrap_or(0)
}

/// Given desired cursor column and line boundaries,
/// returns cursor's absolute position in rope
fn apply_cursor_column(
    rope: &ropey::Rope,
    mut cursor_column: usize,
    mut line_start: usize,
    line_end: usize,
) -> usize {
    use unicode_width::UnicodeWidthChar;

    let mut chars = rope.chars_at(line_start);
    while cursor_column > 0 && line_start < line_end {
        match chars.next() {
            Some('\t') => {
                cursor_column = cursor_column.saturating_sub(*SPACES_PER_TAB);
                line_start += 1;
            }
            Some(c) => {
                cursor_column = cursor_column.saturating_sub(c.width().unwrap_or(1));
                line_start += 1;
            }
            None => break,
        }
    }

    line_start
}

// Returns characters from the cursor's line start
// up to (not not including) the cursor itself
fn line_start_to_cursor(rope: &ropey::Rope, cursor: usize) -> Option<impl Iterator<Item = char>> {
    let line = rope.try_char_to_line(cursor).ok()?;
    let start = rope.try_line_to_char(line).ok()?;
    rope.get_chars_at(start)
        .map(|iter| iter.take(cursor - start))
}

/// Returns all characters in cursor's line
fn line_chars(rope: &ropey::Rope, cursor: usize) -> Option<impl Iterator<Item = char>> {
    line_char_range(rope, rope.try_char_to_line(cursor).ok()?)
        .and_then(|(start, end)| rope.get_chars_at(start).map(|iter| iter.take(end - start)))
}

// If we move the cursor without performing a selection, clear the selection
// If we move the cursor while performing a selection, set the selection if necessary
fn update_selection(selection: &mut Option<usize>, cursor: usize, selecting: bool) {
    if selecting && selection.is_none() {
        *selection = Some(cursor);
    } else if !selecting && selection.is_some() {
        *selection = None
    }
}

fn zap_selection(
    rope: &mut ropey::Rope,
    cursor: &mut usize,
    column: &mut usize,
    selection: usize,
    secondary: &mut Secondary,
) {
    let (selection_start, selection_end) = reorder(*cursor, selection);
    if rope.try_remove(selection_start..selection_end).is_ok() {
        *cursor = selection_start;
        *column = cursor_column(rope, *cursor);
        secondary.update(|pos| {
            if (selection_start..selection_end).contains(pos) {
                *pos = selection_start;
            } else {
                *pos -= selection_end - selection_start;
            }
        });
    }
}

fn select_next_char<const FORWARD: bool>(
    rope: &ropey::Rope,
    cursor: usize,
    target: char,
    stack: Option<char>,
) -> Option<usize> {
    let mut chars = rope.chars_at(cursor);
    if !FORWARD {
        chars.reverse();
    }
    match stack {
        None => chars
            .position(|c| c == target)
            .map(|pos| if FORWARD { cursor + pos } else { cursor - pos }),
        Some(stack) => {
            let mut stacked = 0;
            chars
                .zip(0..)
                .find(|(c, _)| {
                    if *c == target {
                        if stacked > 0 {
                            stacked -= 1;
                            false
                        } else {
                            true
                        }
                    } else if *c == stack {
                        stacked += 1;
                        false
                    } else {
                        false
                    }
                })
                .map(|(_, pos)| if FORWARD { cursor + pos } else { cursor - pos })
        }
    }
}

fn select_limited_next_char<const FORWARD: bool>(
    rope: &ropey::Rope,
    cursor: usize,
    target: char,
    stack: char,
    limit: usize,
) -> Option<usize> {
    let mut chars = rope.chars_at(cursor);
    if !FORWARD {
        chars.reverse();
    }
    let mut stacked = 0;
    chars
        .zip(0..limit)
        .find(|(c, _)| {
            if *c == target {
                if stacked > 0 {
                    stacked -= 1;
                    false
                } else {
                    true
                }
            } else if *c == stack {
                stacked += 1;
                false
            } else {
                false
            }
        })
        .map(|(_, pos)| if FORWARD { cursor + pos } else { cursor - pos })
}

/// Attempts to find next pairing character
/// (closing parens, quotes, etc.)
/// returning the character and its character position
pub fn next_pairing_char(rope: &ropey::Rope, offset: usize) -> Option<(char, usize)> {
    let mut stacked_paren = 0;
    let mut stacked_square_bracket = 0;
    let mut stacked_curly_bracket = 0;
    let mut stacked_angle_bracket = 0;

    fn checked_dec(i: &mut usize) -> bool {
        if *i > 0 {
            *i -= 1;
            false
        } else {
            true
        }
    }

    if offset > rope.len_chars() {
        return None;
    }

    rope.chars_at(offset)
        .zip(0..)
        .find(|(c, _)| match c {
            '(' => {
                stacked_paren += 1;
                false
            }
            '[' => {
                stacked_square_bracket += 1;
                false
            }
            '{' => {
                stacked_curly_bracket += 1;
                false
            }
            '<' => {
                stacked_angle_bracket += 1;
                false
            }
            ')' => checked_dec(&mut stacked_paren),
            ']' => checked_dec(&mut stacked_square_bracket),
            '}' => checked_dec(&mut stacked_curly_bracket),
            '>' => checked_dec(&mut stacked_angle_bracket),
            '"' | '\'' => true,
            _ => false,
        })
        .map(|(c, pos)| (c, offset + pos))
}

/// Attempts to find previous pairing character
/// (opening parens, quotes, etc.)
/// returning the character and its character position
pub fn prev_pairing_char(rope: &ropey::Rope, offset: usize) -> Option<(char, usize)> {
    let mut stacked_paren = 0;
    let mut stacked_square_bracket = 0;
    let mut stacked_curly_bracket = 0;
    let mut stacked_angle_bracket = 0;

    fn checked_dec(i: &mut usize) -> bool {
        if *i > 0 {
            *i -= 1;
            false
        } else {
            true
        }
    }

    if offset > rope.len_chars() {
        return None;
    }

    let mut chars = rope.chars_at(offset);
    chars.reverse();
    chars
        .zip(0..)
        .find(|(c, _)| match c {
            ')' => {
                stacked_paren += 1;
                false
            }
            ']' => {
                stacked_square_bracket += 1;
                false
            }
            '}' => {
                stacked_curly_bracket += 1;
                false
            }
            '>' => {
                stacked_angle_bracket += 1;
                false
            }
            '(' => checked_dec(&mut stacked_paren),
            '[' => checked_dec(&mut stacked_square_bracket),
            '{' => checked_dec(&mut stacked_curly_bracket),
            '<' => checked_dec(&mut stacked_angle_bracket),
            '"' | '\'' => true,
            _ => false,
        })
        .map(|(c, pos)| (c, offset - pos))
}

/// Attempts to find previous opening character
/// returning character and its position
pub fn prev_opening_char(rope: &ropey::Rope, offset: usize, limit: usize) -> Option<(char, usize)> {
    let mut stacked_paren = 0;
    let mut stacked_square_bracket = 0;
    let mut stacked_curly_bracket = 0;

    fn checked_dec(i: &mut usize) -> bool {
        if *i > 0 {
            *i -= 1;
            false
        } else {
            true
        }
    }

    if offset > rope.len_chars() {
        return None;
    }

    let mut chars = rope.chars_at(offset);
    chars.reverse();
    chars
        .zip(0..limit)
        .find(|(c, _)| match c {
            ')' => {
                stacked_paren += 1;
                false
            }
            ']' => {
                stacked_square_bracket += 1;
                false
            }
            '}' => {
                stacked_curly_bracket += 1;
                false
            }
            '(' => checked_dec(&mut stacked_paren),
            '[' => checked_dec(&mut stacked_square_bracket),
            '{' => checked_dec(&mut stacked_curly_bracket),
            _ => false,
        })
        .map(|(c, pos)| (c, offset - pos))
}

/// Attempts to find next closing character
/// returning opening character and its position
pub fn next_closing_char(rope: &ropey::Rope, offset: usize, limit: usize) -> Option<(char, usize)> {
    let mut stacked_paren = 0;
    let mut stacked_square_bracket = 0;
    let mut stacked_curly_bracket = 0;

    fn checked_dec(i: &mut usize) -> bool {
        if *i > 0 {
            *i -= 1;
            false
        } else {
            true
        }
    }

    if offset > rope.len_chars() {
        return None;
    }

    rope.chars_at(offset)
        .zip(0..limit)
        .find(|(c, _)| match c {
            '(' => {
                stacked_paren += 1;
                false
            }
            '[' => {
                stacked_square_bracket += 1;
                false
            }
            '{' => {
                stacked_curly_bracket += 1;
                false
            }
            ')' => checked_dec(&mut stacked_paren),
            ']' => checked_dec(&mut stacked_square_bracket),
            '}' => checked_dec(&mut stacked_curly_bracket),
            _ => false,
        })
        .map(|(c, pos)| {
            (
                match c {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => unreachable!(),
                },
                offset + pos,
            )
        })
}

fn perform_surround(
    rope: &mut ropey::Rope,
    cursor: &mut usize,
    cursor_col: &mut usize,
    selection: &mut usize,
    alt: Option<AltCursor<'_>>,
    [start, end]: [char; 2],
) {
    {
        let (start_pos, end_pos) = reorder(&mut *cursor, selection);
        let mut alt = Secondary::new(alt, |a| a >= *start_pos);
        let _ = rope.try_insert_char(*end_pos, end);
        let _ = rope.try_insert_char(*start_pos, start);
        alt.update(|pos| *pos += if *pos > *end_pos { 2 } else { 1 });
        *start_pos += 1;
        *end_pos += 1;
    }
    *cursor_col = cursor_column(rope, *cursor);
}

/// Returns Ok is surround performed, or Err(Secondary) if not
fn delete_surround<'s>(
    rope: &mut ropey::Rope,
    cursor: &mut usize,
    cursor_col: &mut usize,
    selection: &mut usize,
    alt: Option<AltCursor<'s>>,
) -> Result<(), Secondary<'s>> {
    let (start, end) = reorder(&mut *cursor, selection);
    let mut alt = Secondary::new(alt, |a| a >= *start);

    if let Some(prev_pos) = start.checked_sub(1)
        && let Some(prev_char) = rope.get_char(prev_pos)
        && let Some(next_char) = rope.get_char(*end)
        && matches!(
            (prev_char, next_char),
            ('(', ')') | ('[', ']') | ('{', '}') | ('<', '>') | ('"', '"') | ('\'', '\'')
        )
    {
        let _ = rope.try_remove(*end..*end + 1);
        let _ = rope.try_remove(prev_pos..*start);
        alt.update(|pos| *pos -= if *pos > *end { 2 } else { 1 });
        *end -= 1;
        *start -= 1;
        *cursor_col = cursor_column(rope, *cursor);
        Ok(())
    } else {
        Err(alt)
    }
}

impl From<Buffer> for BufferContext {
    fn from(buffer: Buffer) -> Self {
        Self {
            buffer: buffer.into(),
            viewport_height: 0,
            cursor: 0,
            cursor_column: 0,
            selection: None,
            message: None,
        }
    }
}

/// A set of buffer contexts on a per-view basis
#[derive(Clone, Default)]
pub struct BufferList {
    buffers: Vec<BufferContext>,
    // if we have any buffers at all,
    // must be a valid index pointing to one of our buffers
    current: usize,
}

impl BufferList {
    pub fn new(paths: impl IntoIterator<Item = Source>) -> std::io::Result<Self> {
        let buffers = paths
            .into_iter()
            .map(|p| Buffer::open(p).map(BufferContext::from))
            .collect::<Result<Vec<_>, _>>()?;

        if buffers.is_empty() {
            Ok(Self {
                buffers: vec![Buffer::tutorial().into()],
                current: 0,
            })
        } else {
            Ok(Self {
                buffers,
                current: 0,
            })
        }
    }

    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    pub fn push(&mut self, buffer: BufferContext, select: bool) {
        self.buffers.push(buffer);
        if select {
            // must always be at least one buffer present,
            // so this cannot fail
            self.current = self.buffers.len() - 1;
        }
    }

    pub fn remove(&mut self, buffer: &BufferId) {
        let current_id = self.buffers.get(self.current).map(|buf| buf.id());

        self.buffers.retain(|buf| buf.buffer.id() != *buffer);

        self.current = current_id
            .and_then(|id| self.buffers.iter().position(|buf| buf.buffer.id() == id))
            .unwrap_or(self.buffers.len().saturating_sub(1));
    }

    pub fn current(&self) -> Option<&BufferContext> {
        self.buffers.get(self.current)
    }

    pub fn current_mut(&mut self) -> Option<&mut BufferContext> {
        self.buffers.get_mut(self.current)
    }

    pub fn next_buffer(&mut self) {
        if !self.buffers.is_empty() {
            self.current = (self.current + 1) % self.buffers.len()
        }
    }

    pub fn previous_buffer(&mut self) {
        if !self.buffers.is_empty() {
            self.current = self
                .current
                .checked_sub(1)
                .unwrap_or(self.buffers.len() - 1);
        }
    }

    /// Returns cursor's position relative to the viewport as (row, col)
    ///
    /// The cursor should be centered in the viewport unless
    /// at the very beginning of the file.
    pub fn cursor_viewport_position(&self) -> Option<(usize, usize)> {
        let buf = self.current()?;
        buf.cursor_position()
            .map(|(row, col)| ((buf.viewport_height / 2).min(row), col))
    }

    pub fn set_cursor_focus(&mut self, area: Rect, position: Position) {
        if let Some(buf) = self.current_mut() {
            buf.set_cursor_focus(area, position);
        }
    }

    pub fn update_buf(&mut self, f: impl FnOnce(&mut BufferContext)) {
        if let Some(buf) = self.current_mut() {
            f(buf);
        }
    }

    pub fn on_buf<T>(&mut self, f: impl FnOnce(&mut BufferContext) -> T) -> Option<T> {
        self.current_mut().map(f)
    }

    /// Attempts to select existing buffer by its source
    /// Returns Ok on success, Err on failure
    pub fn select_by_source(&mut self, source: &Source) -> Result<(), ()> {
        match self
            .buffers
            .iter()
            .position(|buf| buf.buffer.borrow().source() == source)
        {
            Some(idx) => {
                self.current = idx;
                Ok(())
            }
            None => Err(()),
        }
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn set_index(&mut self, index: usize) {
        if index < self.buffers.len() {
            self.current = index;
        }
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut BufferContext> {
        self.buffers.get_mut(idx)
    }

    /// If more than one buffer open, returns selected index and Vec of tab names
    pub fn tabs(&self) -> Option<(usize, Vec<String>)> {
        (self.buffers.len() > 1).then(|| {
            (
                self.current,
                self.buffers
                    .iter()
                    .map(|b| b.buffer.borrow().source.short_name().into_owned())
                    .collect(),
            )
        })
    }

    pub fn has_tabs(&self) -> bool {
        self.buffers.len() > 1
    }

    pub fn find_mode(&self) -> Option<Help> {
        self.current().map(|b| b.find_mode())
    }
}

pub struct BufferWidget<'e> {
    pub mode: Option<&'e mut EditorMode>,
    pub layout: crate::editor::EditorLayout,
    pub show_help: Option<Help>,
}

impl BufferWidget<'_> {
    pub const RIGHT_MARGIN: u16 = 5;
}

impl StatefulWidget for BufferWidget<'_> {
    type State = BufferContext;

    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer, state: &mut BufferContext) {
        use crate::editor::SearchType;
        use crate::help::{
            BROWSE_MATCHES, CONFIRM_CLOSE, PASTE_GROUP, REPLACE_MATCHES, REPLACE_MATCHES_REGEX,
            SELECT_INSIDE, SELECT_LINE, SPLIT_PANE, VERIFY_RELOAD, VERIFY_SAVE, render_help,
        };
        use crate::prompt::TextField;
        use crate::syntax::{HighlightState, Highlighter, MultiComment, MultiCommentType};
        use ratatui::{
            layout::{
                Constraint::{Length, Min},
                Layout,
            },
            style::{Color, Modifier, Style},
            text::{Line, Span},
            widgets::{
                Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
                ScrollbarState, Widget,
            },
        };
        use std::borrow::Cow;
        use std::collections::VecDeque;
        use std::ops::RangeInclusive;

        const EDITING: Style = Style::new().add_modifier(Modifier::REVERSED);
        const HIGHLIGHTED: Style = Style::new()
            .fg(Color::Yellow)
            .add_modifier(Modifier::REVERSED);

        struct EditorLine<'s> {
            line: Cow<'s, str>,
            range: RangeInclusive<usize>, // range in rope in characters
            number: usize,                // line number, starting from 0
        }

        impl<'s> EditorLine<'s> {
            fn iter(rope: &'s ropey::Rope, start_line: usize) -> impl Iterator<Item = Self> {
                let mut lines = rope.lines_at(start_line);
                let mut line_numbers = start_line..;
                let mut line_start_numbers = start_line..;
                let mut line_starts = std::iter::from_fn(move || {
                    line_start_numbers
                        .next()
                        .and_then(|l| rope.try_line_to_char(l).ok())
                })
                .peekable();

                std::iter::from_fn(move || {
                    Some(EditorLine {
                        line: Cow::from(lines.next()?),
                        range: line_starts.next()?
                            ..=line_starts
                                .peek()
                                .map(|e| e.saturating_sub(1))
                                .unwrap_or_else(|| rope.len_chars() + 1),
                        number: line_numbers.next()?,
                    })
                })
            }
        }

        fn widen_tabs<'l>(mut input: Line<'l>, tab_substitution: &str) -> Line<'l> {
            fn tabs_to_spaces(s: &mut Cow<'_, str>, tab_substitution: &str) {
                if s.as_ref().contains('\t') {
                    *s = Cow::Owned(s.as_ref().replace('\t', tab_substitution));
                }
            }

            input
                .spans
                .iter_mut()
                .for_each(|s| tabs_to_spaces(&mut s.content, tab_substitution));
            input
        }

        // Colorize syntax of the given text
        fn colorize<'s, S: Highlighter>(
            syntax: &S,
            state: &mut HighlightState,
            text: Cow<'s, str>,
            current_line: bool,
        ) -> Vec<Span<'s>> {
            // Replace with String::remove_last(), if that ever stabilizes
            fn trim_string_matches(mut s: String, to_trim: char) -> String {
                loop {
                    match s.pop() {
                        Some(c) if c == to_trim => { /* drop char*/ }
                        Some(c) => {
                            s.push(c);
                            break s;
                        }
                        None => {
                            break s;
                        }
                    }
                }
            }

            trait FromRange<'s>: Sized + Into<Cow<'s, str>> + AsRef<str> {
                fn extract_range(&self, range: std::ops::Range<usize>) -> Self;

                fn extract_range_from(&self, range: std::ops::RangeFrom<usize>) -> Self;
            }

            impl<'s> FromRange<'s> for &'s str {
                fn extract_range(&self, range: std::ops::Range<usize>) -> Self {
                    &self[range]
                }
                fn extract_range_from(&self, range: std::ops::RangeFrom<usize>) -> Self {
                    &self[range]
                }
            }

            impl FromRange<'static> for String {
                fn extract_range(&self, range: std::ops::Range<usize>) -> Self {
                    self[range].to_string()
                }
                fn extract_range_from(&self, range: std::ops::RangeFrom<usize>) -> Self {
                    self[range].to_string()
                }
            }

            /// Colorizes &str or String to spans based on syntax
            fn colorize_s<'r, R: FromRange<'r>, S: Highlighter>(
                syntax: &S,
                state: &mut HighlightState,
                text: R,
            ) -> Vec<Span<'r>> {
                let mut elements = vec![];
                let mut idx = 0;
                for (highlight, range) in syntax.highlight(text.as_ref(), state) {
                    if idx < range.start {
                        elements.push(Span::raw(text.extract_range(idx..range.start)));
                    }
                    elements.push(Span::styled(
                        text.extract_range(range.clone()),
                        // Style::default().fg(highlight),
                        Style::from(highlight),
                    ));
                    idx = range.end;
                }
                let last = text.extract_range_from(idx..);
                if !last.as_ref().is_empty() {
                    elements.push(Span::raw(last));
                }
                elements
            }

            fn highlight_trailing_whitespace(mut colorized: Vec<Span<'_>>) -> Vec<Span<'_>> {
                fn trim_end(s: &str) -> Result<(&str, &str), &str> {
                    let trimmed = s.trim_ascii_end();
                    if trimmed.len() == s.len() {
                        Err(s)
                    } else {
                        Ok((trimmed, &s[trimmed.len()..]))
                    }
                }

                if let Some(last) = colorized.last()
                    && let Ok((non_ws, ws)) = trim_end(&last.content)
                    && !ws.is_empty()
                {
                    let non_ws = Span {
                        content: Cow::Owned(non_ws.to_string()),
                        style: last.style,
                    };

                    let ws = Span {
                        content: Cow::Owned(ws.to_string()),
                        style: Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::REVERSED),
                    };

                    colorized.pop();
                    if !non_ws.content.is_empty() {
                        colorized.push(non_ws);
                    }
                    colorized.push(ws);
                    colorized
                } else {
                    colorized
                }
            }

            if current_line {
                match text {
                    Cow::Borrowed(s) => colorize_s(syntax, state, s.trim_end_matches('\n')),
                    Cow::Owned(s) => colorize_s(syntax, state, trim_string_matches(s, '\n')),
                }
            } else {
                highlight_trailing_whitespace(match text {
                    Cow::Borrowed(s) => colorize_s(syntax, state, s.trim_end_matches('\n')),
                    Cow::Owned(s) => colorize_s(syntax, state, trim_string_matches(s, '\n')),
                })
            }
        }

        fn extract<'s>(
            input: &mut VecDeque<Span<'s>>,
            mut characters: usize,
            output: &mut Vec<Span<'s>>,
            map: impl Fn(Span<'s>) -> Span<'s>,
        ) {
            fn split_cow(s: Cow<'_, str>, chars: usize) -> (Cow<'_, str>, Cow<'_, str>) {
                let Some((split_point, _)) = s.char_indices().nth(chars) else {
                    return (s, "".into());
                };

                match s {
                    Cow::Borrowed(slice) => {
                        let (start, end) = slice.split_at(split_point);
                        (Cow::Borrowed(start), Cow::Borrowed(end))
                    }
                    Cow::Owned(mut string) => {
                        let suffix = string.split_off(split_point);
                        (Cow::Owned(string), Cow::Owned(suffix))
                    }
                }
            }

            while characters > 0 {
                let Some(span) = input.pop_front() else {
                    return;
                };
                let span_width = span.content.chars().count();
                if span_width <= characters {
                    characters -= span_width;
                    output.push(map(span));
                } else {
                    let (prefix, suffix) = split_cow(span.content, characters);
                    input.push_front(Span {
                        style: span.style,
                        content: suffix,
                    });
                    output.push(map(Span {
                        style: span.style,
                        content: prefix,
                    }));
                    return;
                }
            }
        }

        // Takes syntaxed-colorized line of text along with
        // highlighted match ranges (in ascending order)
        // and returns text in those ranges highlighted in blue
        fn highlight_matches<'s>(
            colorized: Vec<Span<'s>>,
            line_range: RangeInclusive<usize>,
            matches: &mut VecDeque<(Range<usize>, Style)>,
        ) -> Vec<Span<'s>> {
            // A trivial abstraction to make working
            // simultaneously with both line and match ranges
            // more intuitive.
            struct IntRange {
                start: usize,
                end: usize,
            }

            impl From<Range<usize>> for IntRange {
                #[inline]
                fn from(r: Range<usize>) -> Self {
                    Self {
                        start: r.start,
                        end: r.end,
                    }
                }
            }

            impl From<IntRange> for Range<usize> {
                #[inline]
                fn from(IntRange { start, end }: IntRange) -> Self {
                    start..end
                }
            }

            impl IntRange {
                #[inline]
                fn is_empty(&self) -> bool {
                    self.start == self.end
                }

                #[inline]
                fn remaining(&self) -> usize {
                    self.end.saturating_sub(self.start)
                }

                #[inline]
                fn take(&mut self, requested: usize) -> usize {
                    let to_extract = requested.min(self.remaining());
                    self.start += to_extract;
                    to_extract
                }

                #[inline]
                fn take_both(&mut self, other: &mut Self, requested: usize) -> usize {
                    let to_extract = requested.min(self.remaining().min(other.remaining()));
                    self.start += to_extract;
                    other.start += to_extract;
                    to_extract
                }
            }

            let (line_start, line_end) = line_range.into_inner();
            let mut colorized = VecDeque::from(colorized);
            let mut highlighted = Vec::with_capacity(colorized.len());
            let mut line_range = IntRange {
                start: line_start,
                end: line_end,
            };

            while !line_range.is_empty() {
                let Some((match_range, highlight)) = matches.pop_front() else {
                    // if there's no remaining matches,
                    // there's nothing left to highlight
                    highlighted.extend(colorized);
                    return highlighted;
                };
                let mut match_range = IntRange::from(match_range);

                // if match ending is before start of line, just drop it
                if match_range.end < line_range.start {
                    continue;
                }
                // if match starts before start of line,
                // bump match range start up accordingly
                if match_range.start < line_range.start {
                    match_range.start = line_range.start;
                }

                // output line_start to match_start verbatim
                extract(
                    &mut colorized,
                    line_range.take(match_range.start - line_range.start),
                    &mut highlighted,
                    |span| span,
                );

                // output as much of highlighted match as possible
                extract(
                    &mut colorized,
                    match_range.take_both(&mut line_range, match_range.remaining()),
                    &mut highlighted,
                    |span| span.style(highlight),
                );

                // push any remaining partial match back into VecDeque
                if !match_range.is_empty() {
                    matches.push_front((match_range.into(), highlight));
                }
            }

            highlighted.extend(colorized);
            highlighted
        }

        // Takes syntax-colorized line of text and returns
        // portion highlighted, if necessary
        fn highlight_selection<'s>(
            colorized: Vec<Span<'s>>,
            line_range: RangeInclusive<usize>,
            (selection_start, selection_end): (usize, usize),
        ) -> Vec<Span<'s>> {
            let (line_start, line_end) = line_range.into_inner();
            if selection_end <= line_start || selection_start >= line_end {
                colorized
            } else {
                let mut colorized = VecDeque::from(colorized);
                let mut highlighted = Vec::with_capacity(colorized.len());

                // output line_start to selection_start characters verbatim
                extract(
                    &mut colorized,
                    selection_start.saturating_sub(line_start),
                    &mut highlighted,
                    |span| span,
                );

                // output selection_start to selection_end characters highlighted
                extract(
                    &mut colorized,
                    selection_end - selection_start.max(line_start),
                    &mut highlighted,
                    |span| span.style(EDITING),
                );

                // output any remaining characters verbatim
                highlighted.extend(colorized);

                highlighted
            }
        }

        #[derive(Copy, Clone)]
        struct Paren {
            position: usize,
            color: Color,
        }

        impl Paren {
            fn pair(position: usize) -> Self {
                Self {
                    position,
                    color: Color::Green,
                }
            }

            fn opener(position: usize) -> Self {
                Self {
                    position,
                    color: Color::Cyan,
                }
            }

            fn matching(position: usize) -> Self {
                Self {
                    position,
                    color: Color::Green,
                }
            }

            fn mismatch(position: usize) -> Self {
                Self {
                    position,
                    color: Color::Red,
                }
            }
        }

        fn highlight_parens<'s>(
            colorized: Vec<Span<'s>>,
            line_range: RangeInclusive<usize>,
            parens: &mut VecDeque<Paren>,
        ) -> Vec<Span<'s>> {
            let (line_start, line_end) = line_range.into_inner();
            let mut colorized: VecDeque<_> = colorized.into();
            let mut highlighted = Vec::with_capacity(colorized.len());
            let mut offset = line_start;
            while let Some(Paren { position, color }) =
                parens.pop_front_if(|p| p.position >= offset && p.position <= line_end)
            {
                extract(&mut colorized, position - offset, &mut highlighted, |s| s);
                extract(&mut colorized, 1, &mut highlighted, |s| {
                    s.style(Style::new().bg(color))
                });
                offset = position + 1;
            }
            highlighted.extend(colorized);
            highlighted
        }

        fn border_title(title: String, active: bool) -> Line<'static> {
            if active {
                Line::from(vec![
                    Span::raw("\u{252b}"),
                    Span::styled(title, Style::default().bold()),
                    Span::raw("\u{2523}"),
                ])
            } else {
                Line::from(vec![
                    Span::raw("\u{2524}"),
                    Span::raw(title),
                    Span::raw("\u{251c}"),
                ])
            }
        }

        enum FindSyntax<'s, S> {
            Plain(&'s S),
            Regex,
        }

        fn render_find_prompt<S: Highlighter>(
            syntax: FindSyntax<'_, S>,
            text_area: Rect,
            buf: &mut ratatui::buffer::Buffer,
            prompt: &TextField,
            f: impl FnOnce(Block) -> Block,
        ) {
            let [_, dialog_area, _] =
                Layout::vertical([Min(0), Length(3), Min(0)]).areas(text_area);

            Clear.render(dialog_area, buf);
            Paragraph::new(Line::from(match syntax {
                FindSyntax::Plain(syntax) => colorize(
                    syntax,
                    &mut HighlightState::default(),
                    prompt.chars().collect::<String>().into(),
                    true,
                ),
                FindSyntax::Regex => colorize(
                    &crate::syntax::Regex,
                    &mut HighlightState::default(),
                    prompt.chars().collect::<String>().into(),
                    true,
                ),
            }))
            .scroll((
                0,
                (prompt.cursor_column() as u16).saturating_sub(dialog_area.width.saturating_sub(2)),
            ))
            .block(f(Block::bordered().border_type(BorderType::Rounded)))
            .render(dialog_area, buf);
        }

        fn sub_match_ranges(
            matches: &[(Range<usize>, Vec<Option<MatchCapture>>)],
        ) -> VecDeque<(Range<usize>, Style)> {
            let mut ranges = VecDeque::with_capacity(matches.len());

            for (whole_range, sub_captures) in matches {
                let mut whole_range = whole_range.clone();
                if sub_captures.is_empty() {
                    ranges.push_back((whole_range, HIGHLIGHTED));
                } else {
                    for (sub_capture, style) in sub_captures
                        .iter()
                        .skip(1)
                        .zip(
                            [
                                Style::new()
                                    .bg(Color::Blue)
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::REVERSED),
                                Style::new()
                                    .bg(Color::Green)
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::REVERSED),
                                Style::new()
                                    .bg(Color::Magenta)
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::REVERSED),
                                Style::new()
                                    .bg(Color::Cyan)
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::REVERSED),
                            ]
                            .into_iter()
                            .cycle(),
                        )
                        .filter_map(|(sub_cap, style)| Some((sub_cap.as_ref()?, style)))
                    {
                        if whole_range.start < sub_capture.start {
                            ranges.push_back((whole_range.start..sub_capture.start, HIGHLIGHTED));
                        }
                        ranges.push_back((sub_capture.start..sub_capture.end, style));
                        whole_range.start = sub_capture.end;
                    }
                    if whole_range.start < whole_range.end {
                        ranges.push_back((whole_range, HIGHLIGHTED));
                    }
                }
            }

            ranges
        }

        if let Some(EditorMode::Open { chooser }) = self.mode {
            // file selection mode overrides main editing mode
            use crate::files::FileChooser;

            FileChooser::default().render(area, buf, chooser);
            return;
        }

        let buffer = state.buffer.borrow();
        let rope = &buffer.rope;
        let syntax = &buffer.syntax;

        let block = Block::bordered()
            .borders(Borders::TOP)
            .border_type(if self.mode.is_some() {
                BorderType::Thick
            } else {
                BorderType::Plain
            })
            .title_top(border_title(
                if buffer.modified() {
                    format!("{} *", buffer.source.name())
                } else {
                    buffer.source.name().to_string()
                },
                self.mode.is_some(),
            ));

        let block = match buffer.endings.name() {
            Some(name) => {
                block.title_top(border_title(name.to_string(), self.mode.is_some()).right_aligned())
            }
            None => block,
        };

        let block = block.title_top(
            border_title(
                match self.mode {
                    Some(EditorMode::SelectLine { prompt }) => prompt.to_string(),
                    _ => match buffer.rope.try_char_to_line(state.cursor) {
                        Ok(line) => match buffer.rope.try_line_to_char(line) {
                            Ok(line_start) => {
                                format!(
                                    "{}:{}",
                                    Thousands(line + 1),
                                    (state.cursor - line_start) + 1
                                )
                            }
                            Err(_) => format!("{}", Thousands(line + 1)),
                        },
                        Err(_) => "???".to_string(),
                    },
                },
                self.mode.is_some(),
            )
            .right_aligned(),
        );

        let [text_area, scrollbar_area] =
            Layout::horizontal([Min(0), Length(1)]).areas(block.inner(area));

        block.render(area, buf);

        let current_line = rope.try_char_to_line(state.cursor).ok();

        let viewport_line: usize = current_line
            .map(|line| line.saturating_sub(state.viewport_height / 2))
            .unwrap_or(0);

        state.viewport_height = text_area.height.into();

        let mut hlstate: HighlightState = match syntax.multicomment() {
            Some(MultiCommentType::Bidirectional(f)) => rope
                .lines_at(viewport_line)
                .take(area.height.into())
                .find_map(|line| {
                    f(&Cow::from(line)).map(|multicomment| match multicomment {
                        MultiComment::Start => HighlightState::Normal,
                        MultiComment::End => HighlightState::Commenting,
                    })
                })
                .unwrap_or_default(),
            Some(MultiCommentType::Unidirectional(f)) => rope
                .lines()
                .take(viewport_line)
                .fold(HighlightState::default(), |acc, line| {
                    f(acc, &Cow::from(line))
                }),
            None => HighlightState::default(),
        };

        // we're technically only viewing half of the viewport most of the time
        // but it's okay for the viewport_size to be a bit larger than necessary
        let viewport_size = rope
            .try_line_to_char(current_line.unwrap_or(0) + state.viewport_height)
            .unwrap_or(rope.len_chars())
            .saturating_sub(rope.try_line_to_char(viewport_line).unwrap_or(0));

        let mut parentheses: VecDeque<Paren> = match rope.get_char(state.cursor) {
            Some('(') => {
                match select_limited_next_char::<true>(
                    rope,
                    state.cursor + 1,
                    ')',
                    '(',
                    viewport_size,
                ) {
                    Some(paren) => vec![Paren::pair(paren)].into(),
                    None => VecDeque::default(),
                }
            }
            Some('[') => {
                match select_limited_next_char::<true>(
                    rope,
                    state.cursor + 1,
                    ']',
                    '[',
                    viewport_size,
                ) {
                    Some(paren) => vec![Paren::pair(paren)].into(),
                    None => VecDeque::default(),
                }
            }
            Some('{') => {
                match select_limited_next_char::<true>(
                    rope,
                    state.cursor + 1,
                    '}',
                    '{',
                    viewport_size,
                ) {
                    Some(paren) => vec![Paren::pair(paren)].into(),
                    None => VecDeque::default(),
                }
            }
            Some('<') => {
                match select_limited_next_char::<true>(
                    rope,
                    state.cursor + 1,
                    '>',
                    '<',
                    viewport_size,
                ) {
                    Some(paren) => vec![Paren::pair(paren)].into(),
                    None => VecDeque::default(),
                }
            }
            Some(')') => {
                match select_limited_next_char::<false>(rope, state.cursor, '(', ')', viewport_size)
                {
                    Some(paren) => vec![Paren::pair(paren.saturating_sub(1))].into(),
                    None => VecDeque::default(),
                }
            }
            Some(']') => {
                match select_limited_next_char::<false>(rope, state.cursor, '[', ']', viewport_size)
                {
                    Some(paren) => vec![Paren::pair(paren.saturating_sub(1))].into(),
                    None => VecDeque::default(),
                }
            }
            Some('}') => {
                match select_limited_next_char::<false>(rope, state.cursor, '{', '}', viewport_size)
                {
                    Some(paren) => vec![Paren::pair(paren.saturating_sub(1))].into(),
                    None => VecDeque::default(),
                }
            }
            Some('>') => {
                match select_limited_next_char::<false>(rope, state.cursor, '<', '>', viewport_size)
                {
                    Some(paren) => vec![Paren::pair(paren.saturating_sub(1))].into(),
                    None => VecDeque::default(),
                }
            }
            _ => match prev_opening_char(rope, state.cursor, viewport_size) {
                Some((opener, start)) => match next_closing_char(rope, state.cursor, viewport_size)
                {
                    Some((closer, end)) => {
                        if opener == closer {
                            vec![
                                Paren::matching(start.saturating_sub(1)),
                                Paren::matching(end),
                            ]
                            .into()
                        } else {
                            vec![
                                Paren::mismatch(start.saturating_sub(1)),
                                Paren::mismatch(end),
                            ]
                            .into()
                        }
                    }
                    None => vec![Paren::opener(start.saturating_sub(1))].into(),
                },
                None => VecDeque::default(),
            },
        };

        Clear.render(text_area, buf);
        Paragraph::new(match self.mode {
            Some(EditorMode::BrowseMatches { matches, .. }) => {
                let mut matches = sub_match_ranges(matches);

                match state.selection {
                    // no selection, so highlight matches only
                    // (this shouldn't happen)
                    None => EditorLine::iter(rope, viewport_line)
                        .map(
                            |EditorLine {
                                 line,
                                 range,
                                 number,
                             }| {
                                highlight_matches(
                                    colorize(
                                        syntax,
                                        &mut hlstate,
                                        line,
                                        current_line == Some(number),
                                    ),
                                    range,
                                    &mut matches,
                                )
                                .into()
                            },
                        )
                        .map(|line| widen_tabs(line, &buffer.tab_substitution))
                        .take(area.height.into())
                        .collect::<Vec<_>>(),
                    // highlight both matches *and* selection
                    Some(selection) => {
                        let (selection_start, selection_end) = reorder(state.cursor, selection);

                        EditorLine::iter(rope, viewport_line)
                            .map(
                                |EditorLine {
                                     line,
                                     range,
                                     number,
                                 }| {
                                    highlight_selection(
                                        highlight_matches(
                                            colorize(
                                                syntax,
                                                &mut hlstate,
                                                line,
                                                current_line == Some(number),
                                            ),
                                            range.clone(),
                                            &mut matches,
                                        ),
                                        range.clone(),
                                        (selection_start, selection_end),
                                    )
                                    .into()
                                },
                            )
                            .map(|line| widen_tabs(line, &buffer.tab_substitution))
                            .take(area.height.into())
                            .collect::<Vec<_>>()
                    }
                }
            }
            Some(EditorMode::ReplaceMatches { matches, .. }) => {
                let mut cursors = matches
                    .iter()
                    .filter(|m| m.cursor() != state.cursor)
                    .map(|m| {
                        (
                            m.cursor()..m.cursor() + 1,
                            Style::new()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::REVERSED),
                        )
                    })
                    .collect();

                EditorLine::iter(rope, viewport_line)
                    .map(
                        |EditorLine {
                             line,
                             range,
                             number,
                         }| {
                            highlight_matches(
                                {
                                    let mut colorized = colorize(
                                        syntax,
                                        &mut hlstate,
                                        line,
                                        current_line == Some(number),
                                    );
                                    colorized.push(Span::raw(" "));
                                    colorized
                                },
                                {
                                    let (s, e) = range.into_inner();
                                    s..=e + 1
                                },
                                &mut cursors,
                            )
                            .into()
                        },
                    )
                    .map(|line| widen_tabs(line, &buffer.tab_substitution))
                    .take(area.height.into())
                    .collect::<Vec<_>>()
            }
            _ => {
                match state.selection {
                    // no selection, so nothing to highlight
                    None => EditorLine::iter(rope, viewport_line)
                        .map(
                            |EditorLine {
                                 line,
                                 range,
                                 number,
                             }| {
                                highlight_parens(
                                    colorize(
                                        syntax,
                                        &mut hlstate,
                                        line,
                                        current_line == Some(number),
                                    ),
                                    range,
                                    &mut parentheses,
                                )
                                .into()
                            },
                        )
                        .map(|line| widen_tabs(line, &buffer.tab_substitution))
                        .take(area.height.into())
                        .collect::<Vec<_>>(),
                    // highlight whole line, no line, or part of the line
                    Some(selection) => {
                        let (selection_start, selection_end) = reorder(state.cursor, selection);

                        EditorLine::iter(rope, viewport_line)
                            .map(
                                |EditorLine {
                                     line,
                                     range,
                                     number,
                                 }| {
                                    highlight_parens(
                                        highlight_selection(
                                            colorize(
                                                syntax,
                                                &mut hlstate,
                                                line,
                                                current_line == Some(number),
                                            ),
                                            range.clone(),
                                            (selection_start, selection_end),
                                        ),
                                        range,
                                        &mut parentheses,
                                    )
                                    .into()
                                },
                            )
                            .map(|line| widen_tabs(line, &buffer.tab_substitution))
                            .take(area.height.into())
                            .collect::<Vec<_>>()
                    }
                }
            }
        })
        .scroll((
            0,
            state
                .cursor_position()
                .map(|(_, col)| {
                    col.saturating_sub(text_area.width.saturating_sub(Self::RIGHT_MARGIN).into())
                        as u16
                })
                .unwrap_or(0),
        ))
        .render(text_area, buf);

        Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
            scrollbar_area,
            buf,
            &mut ScrollbarState::new(buffer.total_lines())
                .viewport_content_length(text_area.height.into())
                .position(rope.try_char_to_line(state.cursor).unwrap_or(viewport_line)),
        );

        match self.mode {
            None | Some(EditorMode::Editing) => {
                if let Some(Help { select, find }) = self.show_help {
                    use crate::editor::EditorLayout;
                    use crate::help::{
                        EDITING_0, EDITING_1, EDITING_2, F10_SPLIT, F10_UNSPLIT,
                        SWITCH_PANE_HORIZONTAL, SWITCH_PANE_VERTICAL,
                    };

                    let mut help = Vec::with_capacity(16);
                    help.extend(EDITING_0);
                    help.push(find.into());
                    help.extend(EDITING_1);
                    help.push(select.into());
                    help.push(match self.layout {
                        EditorLayout::Single => F10_UNSPLIT,
                        EditorLayout::Horizontal | EditorLayout::Vertical => F10_SPLIT,
                    });
                    help.extend(EDITING_2);
                    match self.layout {
                        EditorLayout::Horizontal => help.push(SWITCH_PANE_HORIZONTAL),
                        EditorLayout::Vertical => help.push(SWITCH_PANE_VERTICAL),
                        EditorLayout::Single => { /* do nothing */ }
                    }

                    crate::help::render_help(text_area, buf, &help, |b| {
                        b.title_top("Keybindings").title_bottom(
                            Line::from(vec![
                                Span::styled(
                                    "F1",
                                    Style::default().add_modifier(Modifier::REVERSED),
                                ),
                                Span::raw(" to toggle"),
                            ])
                            .centered(),
                        )
                    });
                }
            }
            Some(EditorMode::ConfirmClose { .. }) => {
                render_help(text_area, buf, CONFIRM_CLOSE, |b| b);
                render_message(
                    text_area,
                    buf,
                    BufferMessage::Error("Unsaved changes. Really quit?".into()),
                );
            }
            Some(EditorMode::VerifySave) => {
                render_help(text_area, buf, VERIFY_SAVE, |b| b);
                render_message(
                    text_area,
                    buf,
                    BufferMessage::Error("Buffer changed on disk. Really save?".into()),
                );
            }
            Some(EditorMode::SplitPane) => {
                render_help(text_area, buf, SPLIT_PANE, |b| b);
            }
            Some(EditorMode::VerifyReload) => {
                render_help(text_area, buf, VERIFY_RELOAD, |b| b);
                render_message(
                    text_area,
                    buf,
                    BufferMessage::Error("Buffer not yet saved. Really reload?".into()),
                );
            }
            Some(EditorMode::SelectInside) => {
                render_help(text_area, buf, SELECT_INSIDE, |b| b);
            }
            Some(EditorMode::SelectLine { .. }) => {
                render_help(text_area, buf, SELECT_LINE, |b| b);
            }
            Some(EditorMode::Search { prompt, type_, .. }) => {
                use crate::help::{ctrl, ctrl_f, none};

                render_help(
                    text_area,
                    buf,
                    &[
                        ctrl(&["V"], "Paste From Cut Buffer"),
                        none(
                            &["Tab"],
                            match type_ {
                                SearchType::Plain => "Regex Find",
                                SearchType::Regex => "Plain Text Find",
                            },
                        ),
                        ctrl_f(&["T"], "F4", "Goto Line"),
                        ctrl_f(
                            &["F"],
                            "F5",
                            match prompt.is_empty() {
                                true => "Redo Last Find",
                                false => "Begin New Find",
                            },
                        ),
                        none(&["Enter"], "Browse All Matches"),
                        none(&["Esc"], "Cancel"),
                    ],
                    |b| b,
                );
                render_find_prompt(
                    match type_ {
                        SearchType::Plain => FindSyntax::Plain(syntax),
                        SearchType::Regex => FindSyntax::Regex,
                    },
                    text_area,
                    buf,
                    prompt,
                    |b| match state
                        .message
                        .take_if(|m| matches!(m, BufferMessage::Error(_)))
                    {
                        Some(BufferMessage::Error(err)) => b
                            .title_top(type_.to_string())
                            .title_bottom(Line::from(err.to_string()).centered())
                            .border_style(Style::default().fg(Color::Red)),
                        _ => b.title_top(type_.to_string()),
                    },
                );
            }
            Some(EditorMode::BrowseMatches { matches, match_idx }) => {
                render_help(text_area, buf, BROWSE_MATCHES, |block| {
                    block.title(format!("Match {} / {}", *match_idx + 1, matches.len()))
                });
            }
            Some(EditorMode::ReplaceMatches {
                matches,
                match_idx,
                groups,
            }) => {
                render_help(
                    text_area,
                    buf,
                    if matches!(groups, crate::editor::CaptureGroups::None) {
                        REPLACE_MATCHES
                    } else {
                        REPLACE_MATCHES_REGEX
                    },
                    |block| {
                        block.title(format!(
                            "Replacement {} / {}",
                            *match_idx + 1,
                            matches.len()
                        ))
                    },
                );
            }
            Some(EditorMode::PasteGroup { total, .. }) => {
                render_help(
                    text_area,
                    buf,
                    PASTE_GROUP
                        .iter()
                        .copied()
                        .take(*total + 1)
                        .collect::<Vec<_>>()
                        .as_slice(),
                    |b| b,
                );
            }
            Some(EditorMode::Open { .. }) => { /* already handled, above */ }
        }

        if let Some(message) = state.message.take() {
            render_message(text_area, buf, message);
        }
    }
}

pub fn render_message(area: Rect, buf: &mut ratatui::buffer::Buffer, message: BufferMessage) {
    use ratatui::{
        layout::{
            Constraint::{Length, Min},
            Layout,
        },
        style::{Color, Style},
        widgets::{Block, BorderType, Clear, Paragraph, Widget},
    };
    use unicode_width::UnicodeWidthStr;

    let width = message.as_str().width().try_into().unwrap_or(u16::MAX);
    let [_, dialog_area, _] = Layout::horizontal([Min(0), Length(width + 2), Min(0)]).areas(area);
    let [_, dialog_area, _] = Layout::vertical([Min(0), Length(3), Min(0)]).areas(dialog_area);

    Clear.render(dialog_area, buf);
    Paragraph::new(message.as_str())
        .style(match message {
            BufferMessage::Notice(_) => Style::default(),
            BufferMessage::Error(_) => Style::default().fg(Color::Red),
        })
        .block(Block::bordered().border_type(BorderType::Rounded))
        .render(dialog_area, buf);
}

pub struct CutBuffer {
    data: String,
    chars_len: usize,
}

impl CutBuffer {
    pub fn as_str(&self) -> &str {
        self.data.as_str()
    }
}

impl From<ropey::RopeSlice<'_>> for CutBuffer {
    fn from(slice: ropey::RopeSlice<'_>) -> Self {
        Self {
            data: slice.chunks().collect(),
            chars_len: slice.len_chars(),
        }
    }
}

impl From<String> for CutBuffer {
    fn from(data: String) -> Self {
        Self {
            chars_len: data.chars().count(),
            data,
        }
    }
}

/// Given rope and starting area in chars,
/// yields lines and their start points in bytes
fn search_area<'r>(
    rope: &'r ropey::Rope,
    range: Option<&SelectionRange>,
) -> impl Iterator<Item = (Cow<'r, str>, usize)> {
    fn no_nl(s: Cow<'_, str>) -> Option<Cow<'_, str>> {
        (!s.is_empty()).then(|| match s {
            Cow::Borrowed(s) => Cow::Borrowed(s.trim_end_matches('\n')),
            Cow::Owned(mut s) => {
                while s.ends_with('\n') {
                    let _ = s.pop();
                }
                Cow::Owned(s)
            }
        })
    }

    match range {
        None => Box::new(rope.lines().enumerate().filter_map(|(line_num, line)| {
            Some((no_nl(line.into())?, rope.try_line_to_byte(line_num).ok()?))
        })) as Box<dyn Iterator<Item = (Cow<'_, str>, usize)>>,
        Some(SelectionRange { start, lines }) => Box::new(
            (*start..)
                .zip(rope.lines_at(*start))
                .take(lines.get())
                .filter_map(|(line_num, line)| {
                    Some((no_nl(line.into())?, rope.try_line_to_byte(line_num).ok()?))
                }),
        ),
    }
}

/// Buffer's undo/redo state
struct BufferState {
    rope: ropey::Rope,
    cursor: usize,
    cursor_column: usize,
}

struct Undo {
    state: BufferState,
    finished: bool, // whether we've done any movement since undo added
}

#[derive(Clone)]
pub enum BufferMessage {
    Notice(Cow<'static, str>),
    Error(Cow<'static, str>),
}

impl BufferMessage {
    fn as_str(&self) -> &str {
        match self {
            Self::Notice(s) | Self::Error(s) => s.as_ref(),
        }
    }
}

/// Patches source to match target using diffs
///
/// Adjusts cursor and alt cursor in the process
fn patch_rope(
    source: &mut ropey::Rope,
    target: String,
    cursor: &mut usize,
    selection: &mut Option<usize>,
    alt: Option<AltCursor<'_>>,
) {
    use imara_diff::{Algorithm::Histogram, Diff, Hunk, InternedInput};
    use ropey::Rope;
    use std::ops::Range;

    #[must_use]
    fn remove_lines(rope: &mut Rope, lines: Range<u32>) -> Range<usize> {
        let removed =
            rope.line_to_char(lines.start as usize)..rope.line_to_char(lines.end as usize);
        rope.remove(removed.clone());
        removed
    }

    // returns string and length in characters
    fn get_lines(rope: &Rope, lines: Range<u32>) -> (String, usize) {
        if lines.end > lines.start {
            rope.lines_at(lines.start as usize)
                .take((lines.end - lines.start) as usize)
                .fold((String::default(), 0), |(mut s, chars), line| {
                    s.extend(line.chunks());
                    (s, chars + line.len_chars())
                })
        } else {
            (String::default(), 0)
        }
    }

    fn decrement_pos(pos: &mut usize, removed: &Range<usize>) {
        if *pos > removed.end {
            *pos -= removed.end - removed.start;
        } else if *pos > removed.start {
            *pos = removed.start;
        }
    }

    fn increment_pos(pos: &mut usize, inserted_pos: usize, inserted_chars: usize) {
        if *pos >= inserted_pos {
            *pos += inserted_chars;
        }
    }

    let source_str =
        source
            .chunks()
            .fold(String::with_capacity(source.len_bytes()), |mut acc, s| {
                acc.push_str(s);
                acc
            });

    let hunks = Diff::compute(Histogram, &InternedInput::new(source_str.as_str(), &target))
        .hunks()
        .collect::<Vec<_>>();

    let target = Rope::from(target);

    let mut alt = Secondary::new(alt, |_| true);

    for Hunk { before, after } in hunks.into_iter().rev() {
        let removed = remove_lines(source, before.clone());

        decrement_pos(cursor, &removed);
        if let Some(selection) = selection.as_mut() {
            decrement_pos(selection, &removed);
        }
        alt.update(|a| decrement_pos(a, &removed));

        let (to_insert, inserted_chars) = get_lines(&target, after);

        if !to_insert.is_empty() {
            let inserted_pos = source.line_to_char(before.start as usize);

            increment_pos(cursor, inserted_pos, inserted_chars);
            if let Some(selection) = selection.as_mut() {
                increment_pos(selection, inserted_pos, inserted_chars);
            }
            alt.update(|a| increment_pos(a, inserted_pos, inserted_chars));

            source.insert(inserted_pos, &to_insert);
        }
    }
}

struct Thousands(usize);

impl std::fmt::Display for Thousands {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // this is recursive but also very limited
        // on a 64-bit platform, usize::MAX only recurses 7 levels deep
        // which is impossibly huge for a text file
        fn write_separated(u: usize, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match u {
                u @ 0..1000 => u.fmt(f),
                u => {
                    write_separated(u / 1000, f)?;
                    write!(f, "_{:03}", u % 1000)
                }
            }
        }

        match self.0 {
            u @ 0..10000 => u.fmt(f),
            u => write_separated(u, f),
        }
    }
}

#[inline]
fn is_word(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

fn reorder<T: Ord>(x: T, y: T) -> (T, T) {
    if x <= y { (x, y) } else { (y, x) }
}
