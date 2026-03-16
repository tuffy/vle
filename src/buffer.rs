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

pub static TAB_SUBSTITUTION: LazyLock<String> =
    LazyLock::new(|| std::iter::repeat_n(' ', *SPACES_PER_TAB).collect());

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
    Test,
}

#[cfg(not(feature = "ssh"))]
impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local(x), Self::Local(y)) => x == y,
            (Self::Tutorial, Self::Tutorial) => true,
            (Self::Test, Self::Test) => true,
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
            (Self::Test, Self::Test) => true,
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
            Self::Test => "Terminal Test".into(),
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
            Self::Test => "Terminal Test".into(),
        }
    }

    /// Used to determine syntax highlighting
    pub fn file_name(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::Local(path) => path.file_name().map(|s| s.to_string_lossy()),
            #[cfg(feature = "ssh")]
            Self::Ssh { path, .. } => path.file_name().map(|s| s.to_string_lossy()),
            Self::Tutorial => None,
            Self::Test => None,
        }
    }

    /// Also used to determine syntax highlighting
    pub fn extension(&self) -> Option<&str> {
        match self {
            Self::Local(path) => path.extension().and_then(|s| s.to_str()),
            #[cfg(feature = "ssh")]
            Self::Ssh { path, .. } => path.extension().and_then(|s| s.to_str()),
            Self::Tutorial => None,
            Self::Test => None,
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
            Self::Test => Ok((None, include_str!("test.txt").to_string())),
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
            Self::Tutorial | Self::Test => self
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
            Self::Tutorial | Self::Test => Ok(None),
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
            Self::Tutorial | Self::Test => None,
        }
    }
}

mod private {
    use crate::buffer::{AltCursor, Buffer, Toggle};
    use std::cell::{Ref, RefCell, RefMut};
    use std::collections::BTreeMap;
    use std::ops::{Deref, DerefMut};
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
    impl Deref for Rope {
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

    impl Deref for RopeHandle<'_> {
        type Target = ropey::Rope;

        fn deref(&self) -> &ropey::Rope {
            self.rope
        }
    }

    impl DerefMut for RopeHandle<'_> {
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
            let mut buf = self.0.borrow_mut();
            buf.perform_update(cursor, cursor_column);
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

    impl Deref for MoveHandle<'_> {
        type Target = Buffer;

        fn deref(&self) -> &Buffer {
            &self.0
        }
    }

    impl DerefMut for MoveHandle<'_> {
        fn deref_mut(&mut self) -> &mut Buffer {
            &mut self.0
        }
    }

    impl Drop for MoveHandle<'_> {
        fn drop(&mut self) {
            self.0.undo_finished = true;
        }
    }

    #[derive(Clone, Default)]
    pub struct Bookmarks(BTreeMap<usize, ()>);

    impl Bookmarks {
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }

        pub fn len(&self) -> usize {
            self.0.len()
        }

        pub fn iter(&self) -> impl Iterator<Item = usize> {
            self.0.keys().copied()
        }

        pub fn range<R>(&self, range: R) -> impl DoubleEndedIterator<Item = usize>
        where
            R: std::ops::RangeBounds<usize>,
        {
            self.0.range(range).map(|(b, ())| *b)
        }

        pub fn toggle(&mut self, cursor: usize) -> Toggle {
            use std::collections::btree_map::Entry;

            match self.0.entry(cursor) {
                Entry::Vacant(v) => {
                    v.insert(());
                    Toggle::Inserted
                }
                Entry::Occupied(o) => {
                    let () = o.remove();
                    Toggle::Removed
                }
            }
        }

        /// Returns Ok if bookmark removed
        pub fn remove(&mut self, cursor: usize) -> Result<(), ()> {
            self.0.remove(&cursor).ok_or(())
        }

        /// Returns next bookmark after (but not including) the cursor
        /// wrapping around to the beginning if necessary
        pub fn next_after(&self, cursor: usize) -> Option<usize> {
            use std::ops::Bound;

            self.0
                .range((Bound::Excluded(cursor), Bound::Unbounded))
                .next()
                .or_else(|| self.0.first_key_value())
                .map(|(k, ())| k)
                .copied()
        }

        /// Returns next bookmark before (but not including) the cursor
        /// wrapping around to the end if necessary
        pub fn next_before(&self, cursor: usize) -> Option<usize> {
            use std::ops::Bound;

            self.0
                .range((Bound::Unbounded, Bound::Excluded(cursor)))
                .next_back()
                .or_else(|| self.0.last_key_value())
                .map(|(k, ())| k)
                .copied()
        }

        pub fn get_mut(&mut self) -> BookmarksHandle<'_> {
            BookmarksHandle(&mut self.0)
        }
    }

    pub struct BookmarksHandle<'m>(&'m mut BTreeMap<usize, ()>);

    impl BookmarksHandle<'_> {
        /// Update all bookmark entries whose positions are >= cursor
        pub fn update_ge(&mut self, cursor: usize, mut update: impl FnMut(&mut usize)) {
            let mut updated = self
                .0
                .extract_if(cursor.., |_, _| true)
                .map(|(mut pos, ())| {
                    update(&mut pos);
                    (pos, ())
                })
                .collect();

            self.0.append(&mut updated);
        }

        /// Remove all bookmarks in range
        pub fn remove<R>(&mut self, range: R)
        where
            R: std::ops::RangeBounds<usize>,
        {
            self.0.extract_if(range, |_, _| true).for_each(drop);
        }

        pub fn extract<R>(&mut self, range: R) -> impl Iterator<Item = usize>
        where
            R: std::ops::RangeBounds<usize>,
        {
            self.0.extract_if(range, |_, _| true).map(|(b, ())| b)
        }

        pub fn add(&mut self, bookmarks: impl Iterator<Item = usize>) {
            self.0.extend(bookmarks.into_iter().map(|b| (b, ())));
        }
    }

    /// A secondary cursor which implements various math operations
    pub struct Secondary<'a, 'b> {
        // secondary cursor's position and selection, if any
        alt_cursor_selections: Vec<(&'a mut usize, Option<&'a mut usize>)>,
        // a handle to the current buffer's bookmarks
        bookmarks: BookmarksHandle<'b>,
        // minimum primary cursor position we're concerned with
        cursor: usize,
    }

    impl<'a, 'b> Secondary<'a, 'b> {
        pub fn new(alt: Vec<AltCursor<'a>>, bookmarks: BookmarksHandle<'b>) -> Self {
            Self {
                alt_cursor_selections: alt
                    .into_iter()
                    .map(|a| (a.cursor, a.selection.as_mut()))
                    .collect(),
                bookmarks,
                cursor: 0,
            }
        }

        /// Constrained to values greater than or equal to the cursor
        pub fn ge(alt: Vec<AltCursor<'a>>, bookmarks: BookmarksHandle<'b>, cursor: usize) -> Self {
            Self::filtered(alt, bookmarks, cursor, |a| a >= cursor)
        }

        /// Constrained to values greater than or equal to the cursor
        pub fn gt(alt: Vec<AltCursor<'a>>, bookmarks: BookmarksHandle<'b>, cursor: usize) -> Self {
            Self::filtered(alt, bookmarks, cursor, |a| a > cursor)
        }

        fn filtered(
            alt: Vec<AltCursor<'a>>,
            bookmarks: BookmarksHandle<'b>,
            cursor: usize,
            mut f: impl FnMut(usize) -> bool,
        ) -> Self {
            Self {
                alt_cursor_selections: alt
                    .into_iter()
                    .filter_map(|a| {
                        Some((
                            f(*a.cursor).then_some(a.cursor)?,
                            a.selection.as_mut().filter(|s| f(**s)),
                        ))
                    })
                    .collect(),
                bookmarks,
                cursor,
            }
        }

        /// Updates secondary cursor in-place, if available
        pub fn update(&mut self, mut f: impl FnMut(&mut usize)) {
            self.alt_cursor_selections
                .iter_mut()
                .for_each(|(cursor, selection)| {
                    f(cursor);

                    if let Some(selection) = selection {
                        f(selection);
                    }
                });

            self.bookmarks.update_ge(self.cursor, f);
        }

        /// Inserts fresh bookmarks into bookmarks set
        pub fn add_bookmarks(&mut self, bookmarks: impl Iterator<Item = usize>) {
            self.bookmarks.add(bookmarks);
        }

        /// Removes and returns bookmarks in the given range
        pub fn extract_bookmarks<R>(&mut self, range: R) -> impl Iterator<Item = usize>
        where
            R: std::ops::RangeBounds<usize>,
        {
            self.bookmarks.extract(range)
        }

        /// Removes bookmarks in range and returns range unchanged
        pub fn remove<R>(&mut self, range: R) -> R
        where
            R: std::ops::RangeBounds<usize> + Clone,
        {
            self.bookmarks.remove(range.clone());
            range
        }

        /// Increments our position by chars
        /// returns chars unchanged
        pub fn inc(&mut self, chars: usize) -> usize {
            self.update(|a| *a += chars);
            chars
        }
    }

    impl std::ops::AddAssign<usize> for Secondary<'_, '_> {
        fn add_assign(&mut self, rhs: usize) {
            self.update(|c| {
                *c += rhs;
            })
        }
    }

    impl std::ops::SubAssign<usize> for Secondary<'_, '_> {
        fn sub_assign(&mut self, rhs: usize) {
            self.update(|c| {
                *c -= rhs;
            })
        }
    }
}

use private::Secondary;

/// A buffer corresponding to a file on disk (either local or remote)
///
/// May be shared between panes
pub struct Buffer {
    source: Source,                // the source file
    endings: LineEndings,          // the source file's line endings
    saved: Option<SystemTime>,     // when the file was last saved
    rope: private::Rope,           // the data rope
    undo: Vec<BufferState>,        // the undo stack
    undo_finished: bool,           // whether cursor moved since last undo
    redo: Vec<BufferState>,        // the redo stack
    syntax: Box<dyn Highlighter>,  // the syntax highlighting to use
    tabs_required: bool,           // whether the format demands actual tabs
    tab_substitution: String,      // spaces to substitute for tabs
    bookmarks: private::Bookmarks, // saved bookmark positions (sorted)
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
            tab_substitution: std::iter::repeat_n(
                ' ',
                probe_indent(&rope, if syntax.tabs_required() { '\t' } else { ' ' })
                    .unwrap_or(*SPACES_PER_TAB),
            )
            .collect(),
            rope: rope.into(),
            endings,
            saved,
            tabs_required: *ALWAYS_TAB || syntax.tabs_required(),
            syntax,
            source,
            undo: vec![],
            undo_finished: true,
            redo: vec![],
            bookmarks: private::Bookmarks::default(),
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
            undo_finished: true,
            redo: vec![],
            bookmarks: private::Bookmarks::default(),
        }
    }

    /// Attempts to reload buffer from disk
    fn reload(
        &mut self,
        cursor: &mut usize,
        selection: &mut Option<usize>,
        alt: Vec<AltCursor<'_>>,
    ) -> std::io::Result<()> {
        let (saved, reloaded) = self.source.read_string(self.endings)?;
        patch_rope(
            &mut self.rope.get_mut(),
            reloaded,
            cursor,
            selection,
            Secondary::new(alt, self.bookmarks.get_mut()),
        );
        self.rope.save();
        self.saved = saved;
        self.undo_finished = true;
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
        self.undo_finished = true;
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

    /// A simple rope / bookmarks split borrow
    pub fn rope_bookmarks_mut(
        &mut self,
    ) -> (private::RopeHandle<'_>, private::BookmarksHandle<'_>) {
        (self.rope.get_mut(), self.bookmarks.get_mut())
    }

    /// Whether this buffer has any bookmarks
    pub fn has_bookmarks(&self) -> bool {
        !self.bookmarks.is_empty()
    }

    /// Adjust internal state for a move
    pub fn perform_update(&mut self, cursor: usize, cursor_column: usize) {
        if std::mem::take(&mut self.undo_finished) {
            self.undo.push(BufferState {
                rope: self.rope.clone(),
                cursor,
                cursor_column,
                bookmarks: self.bookmarks.clone(),
            });
            self.redo.clear();
        }
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

pub enum FindMode {
    WholeFile,
    Selected,
    InSelection,
}

impl From<FindMode> for crate::help::Keybinding {
    fn from(mode: FindMode) -> Self {
        crate::help::keybind::<crate::key::Find>(match mode {
            FindMode::WholeFile => "Find in File",
            FindMode::Selected => "Find Selected Text",
            FindMode::InSelection => "Find in Selected Lines",
        })
    }
}

/// Where the cursor is located, for help message purposes
#[derive(Copy, Clone, Default)]
pub enum CursorPos {
    #[default]
    Other, // cursor is somewhere else
    InWord,    // cursor is inside a word
    AfterWord, // cursor is after a word
    AtParen,   // cursor is at a parenthesis
}

pub struct Help {
    find: FindMode,
    cursor_pos: CursorPos,
    has_bookmarks: bool,
    has_selection: bool,
    multiple_buffers: bool,
    multiple_panes: bool,
}

type MatchAndCaptures = (Range<usize>, Vec<String>);

/// A buffer with additional context on a per-view basis
#[derive(Clone)]
pub struct BufferContext {
    buffer: private::BufferCell,    // the buffer we're wrapping
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

    pub fn reload(&mut self, alt: Vec<AltCursor<'_>>) {
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
        self.cursor_column = cursor_column(&self.buffer.borrow_move().rope, self.cursor);
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
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
            widgets::Block,
        };

        // rebuild layout from BufferWidget
        let [text_area, _] =
            Layout::horizontal([Min(0), Length(1)]).areas(Block::bordered().inner(area));

        if !text_area.contains(position) {
            return;
        }

        let buffer = self.buffer.borrow();
        let rope = &buffer.rope;
        let row = position.y.saturating_sub(text_area.y);
        let col = position.x.saturating_sub(text_area.x);

        let current_line = rope.try_char_to_line(self.cursor).ok();
        let viewport_height: usize = text_area.height.into();

        let viewport_line: usize = current_line
            .map(|line| line.saturating_sub(viewport_height / 2))
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
        use unicode_width::UnicodeWidthChar;

        let buf = self.buffer.borrow_move();
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = self.cursor.saturating_sub(
            buf.rope
                .chars_at(self.cursor)
                .reversed()
                .take_while(|c| c.width() == Some(0))
                .count()
                + 1,
        );
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
    }

    pub fn cursor_forward(&mut self, selecting: bool) {
        use unicode_width::UnicodeWidthChar;

        let buf = self.buffer.borrow_move();
        update_selection(&mut self.selection, self.cursor, selecting);
        if self.cursor < buf.rope.len_chars() {
            self.cursor = self.cursor
                + buf
                    .rope
                    .chars_at(self.cursor + 1)
                    .take_while(|c| c.width() == Some(0))
                    .count()
                + 1;
        }
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
            use unicode_width::UnicodeWidthChar;

            let start = (line_start + column).min(next_line_start.saturating_sub(1));
            self.cursor = start
                + buf
                    .rope
                    .chars_at(start)
                    .take_while(|c| c.width() == Some(0))
                    .count();
            self.cursor_column = cursor_column(&buf.rope, self.cursor);
            self.selection = None;
        } else {
            self.message = Some(BufferMessage::Error("invalid line".into()));
        }
    }

    pub fn insert_char(&mut self, alt: Vec<AltCursor<'_>>, c: char) {
        use unicode_width::UnicodeWidthChar;

        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();

        match &mut self.selection {
            Some(selection) => match c {
                '(' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    bookmarks,
                    ['(', ')'],
                ),
                '[' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    bookmarks,
                    ['[', ']'],
                ),
                '{' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    bookmarks,
                    ['{', '}'],
                ),
                '<' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    bookmarks,
                    ['<', '>'],
                ),
                '\"' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    bookmarks,
                    ['\"', '\"'],
                ),
                '\'' => perform_surround(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    alt,
                    bookmarks,
                    ['\'', '\''],
                ),
                _ => {
                    let mut alt = Secondary::ge(alt, bookmarks, self.cursor.min(*selection));
                    zap_selection(
                        &mut rope,
                        &mut self.cursor,
                        &mut self.cursor_column,
                        *selection,
                        &mut alt,
                    );
                    self.selection = None;
                    rope.insert_char(self.cursor, c);
                    self.cursor += alt.inc(1);
                    self.cursor_column += c.width().unwrap_or(1);
                }
            },
            None => {
                insert_char_or_pair(
                    &mut rope,
                    self.cursor,
                    &mut Secondary::new(alt, bookmarks),
                    c,
                );
                self.cursor += 1;
                self.cursor_column += c.width().unwrap_or(1);
            }
        }
    }

    pub fn paste(&mut self, alt: Vec<AltCursor<'_>>, cut_buffer: &mut Option<EditorCutBuffer>) {
        match self.selection.as_mut() {
            None => {
                // No active selection, so paste as-is

                let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
                let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
                let mut alt = Secondary::ge(alt, bookmarks, self.cursor);

                match cut_buffer {
                    Some(EditorCutBuffer::Single(pasted)) => {
                        if rope.try_insert(self.cursor, &pasted.data).is_ok() {
                            let old_cursor = self.cursor;
                            self.cursor += alt.inc(pasted.chars_len);
                            alt.add_bookmarks(pasted.bookmarks.iter().map(|b| old_cursor + b));
                            self.cursor_column = cursor_column(&rope, self.cursor);
                        }
                    }
                    Some(EditorCutBuffer::Multiple(pasted_items)) => {
                        // insert first pasted item and rotate
                        if let Some(pasted) = pasted_items.first() {
                            if rope.try_insert(self.cursor, &pasted.data).is_ok() {
                                let old_cursor = self.cursor;
                                self.cursor += alt.inc(pasted.chars_len);
                                alt.add_bookmarks(pasted.bookmarks.iter().map(|b| old_cursor + b));
                                self.cursor_column = cursor_column(&rope, self.cursor);
                            }
                            pasted_items.rotate_left(1);
                        }
                    }
                    None => { /* nothing in cut buffer, so nothing to do */ }
                }
            }
            Some(selection) => {
                if let Some(cut_pasted) = cut_buffer.as_mut().and_then(|c| c.primary_mut()) {
                    let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
                    let (selection_start, selection_end) = reorder(self.cursor, *selection);
                    let cut_range = selection_start..selection_end;
                    let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
                    let mut alt = Secondary::ge(alt, bookmarks, selection_start);

                    if let Some(cut) = rope.get_slice(cut_range.clone()).map(|slice| {
                        CutBuffer::new(
                            slice,
                            alt.extract_bookmarks(cut_range.clone())
                                .map(|b| b - cut_range.start),
                        )
                    }) {
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
                        let pasted = std::mem::replace(cut_pasted, cut);
                        *cut_buffer = Some(EditorCutBuffer::Single(std::mem::take(cut_pasted)));
                        if rope.try_insert(self.cursor, &pasted.data).is_ok() {
                            let old_cursor = self.cursor;
                            alt += pasted.chars_len;
                            alt.add_bookmarks(pasted.bookmarks.iter().map(|b| old_cursor + b));
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

    pub fn newline(&mut self, alt: Vec<AltCursor<'_>>) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let indent_char = if buf.tabs_required { '\t' } else { ' ' };
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();

        let mut alt = match self.selection.take() {
            Some(selection) => {
                let mut secondary = Secondary::ge(alt, bookmarks, self.cursor.min(selection));

                zap_selection(
                    &mut rope,
                    &mut self.cursor,
                    &mut self.cursor_column,
                    selection,
                    &mut secondary,
                );

                secondary
            }
            None => Secondary::ge(alt, bookmarks, self.cursor),
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
            self.cursor += alt.inc(1);
        } else {
            rope.insert_char(self.cursor, '\n');
            self.cursor += alt.inc(1);
            self.cursor_column = 0;
            for _ in 0..indent {
                rope.insert_char(self.cursor, indent_char);
                self.cursor += alt.inc(1);
                self.cursor_column += 1;
            }
        }
    }

    pub fn backspace(&mut self, alt: Vec<AltCursor<'_>>) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();

        match self.selection.take() {
            None => {
                if let Ok((removed, _)) = backspace_or_un_pair(
                    &mut rope,
                    self.cursor,
                    &mut Secondary::new(alt, bookmarks),
                ) {
                    self.cursor -= removed;
                    self.cursor_column = cursor_column(&rope, self.cursor);
                }
            }
            Some(current_selection) => {
                let mut alt = Secondary::ge(alt, bookmarks, self.cursor.min(current_selection));
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

    pub fn delete(&mut self, alt: Vec<AltCursor<'_>>) {
        let buf = &mut self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();

        match &mut self.selection {
            None => {
                use unicode_width::UnicodeWidthChar;

                let mut alt = Secondary::gt(alt, bookmarks, self.cursor);

                let to_delete = rope
                    .chars_at(self.cursor)
                    .skip(1)
                    .take_while(|c| c.width() == Some(0))
                    .count()
                    + 1;

                if rope
                    .try_remove(alt.remove(self.cursor..(self.cursor + to_delete)))
                    .is_ok()
                {
                    alt -= to_delete;
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
                    bookmarks,
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
        let buffer = self.buffer.borrow();
        Some(CutBuffer::new(
            buffer.rope.get_slice(selection_start..selection_end)?,
            buffer
                .bookmarks
                .range(selection_start..selection_end)
                .map(|b| b - selection_start),
        ))
    }

    pub fn take_selection(&mut self, alt: Vec<AltCursor<'_>>) -> Option<CutBuffer> {
        let selection = self.selection.take()?;
        let (selection_start, selection_end) = reorder(self.cursor, selection);
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::ge(alt, bookmarks, selection_start);

        rope.get_slice(selection_start..selection_end)
            .map(|r| {
                CutBuffer::new(
                    r,
                    alt.extract_bookmarks(selection_start..selection_end)
                        .map(|b| b - selection_start),
                )
            })
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
    pub fn all_matches<S: SearchTerm>(
        &mut self,
        range: Option<&SelectionRange>,
        term: S,
    ) -> Result<(usize, Vec<MatchAndCaptures>), S> {
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
                        c,
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
        self.cursor = next_match.end;
        self.selection = None;
        Ok((idx, matches))
    }

    pub fn perform_undo(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        match buf.undo.pop() {
            Some(mut state) => {
                use std::ops::DerefMut;
                std::mem::swap(buf.rope.get_mut().deref_mut(), &mut state.rope);
                std::mem::swap(&mut buf.bookmarks, &mut state.bookmarks);
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
                std::mem::swap(&mut buf.bookmarks, &mut state.bookmarks);
                std::mem::swap(&mut self.cursor, &mut state.cursor);
                std::mem::swap(&mut self.cursor_column, &mut state.cursor_column);
                buf.undo.push(state);
                buf.undo_finished = true;
                self.selection = None;
            }
            None => {
                self.message = Some(BufferMessage::Notice("nothing left to redo".into()));
            }
        }
    }

    pub fn complete_or_indent(&mut self, alt: Vec<AltCursor<'_>>) -> Option<(usize, Vec<String>)> {
        match self.selection {
            None => {
                if let matches @ Some(_) = self.autocomplete_matches() {
                    return matches;
                }
                let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
                let indent = match buf.tabs_required {
                    false => buf.tab_substitution.clone(),
                    true => "\t".to_string(),
                };
                let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
                if let Ok(line_start) = rope
                    .try_char_to_line(self.cursor)
                    .and_then(|line| rope.try_line_to_char(line))
                {
                    let mut alt = Secondary::ge(alt, bookmarks, line_start);
                    rope.insert(line_start, &indent);
                    self.cursor += indent.len();
                    alt += indent.len();
                }
                None
            }
            selection_opt @ Some(_) => {
                use std::convert::Infallible;

                let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
                let indent = match buf.tabs_required {
                    false => buf.tab_substitution.clone(),
                    true => "\t".to_string(),
                };
                let indent_chars = indent.chars().count();
                let mut indent_lines = selected_lines(&buf.rope, self.cursor, selection_opt)
                    .filter_map(|line| (!line.is_empty()).then(|| line.into()))
                    .collect::<Vec<_>>();
                let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
                let mut alt = Secondary::new(alt, bookmarks);
                multicursor_update(
                    &mut indent_lines,
                    |m| {
                        m.indent(&mut rope, &mut alt, &indent, indent_chars);
                        Ok::<_, Infallible>(())
                    },
                    |m, ()| {
                        *m += indent_chars;
                    },
                );
                if let Some(start) = indent_lines.first()
                    && let Some(end) = indent_lines.last()
                {
                    self.selection = Some(start.range.start);
                    self.cursor = end.range.end;
                }
                None
            }
        }
    }

    pub fn complete_or_unindent(
        &mut self,
        alt: Vec<AltCursor<'_>>,
    ) -> Option<(usize, Vec<String>)> {
        match self.selection {
            None => {
                if let matches @ Some(_) = self.autocomplete_matches() {
                    return matches;
                }
                let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
                let indent = match buf.tabs_required {
                    false => buf.tab_substitution.clone(),
                    true => "\t".to_string(),
                };
                let (mut rope, bookmarks) = buf.rope_bookmarks_mut();

                if let Some(line_start) = rope
                    .try_char_to_line(self.cursor)
                    .ok()
                    .and_then(|line| rope.try_line_to_char(line).ok())
                    && rope
                        .chars_at(line_start)
                        .take(indent.len())
                        .eq(indent.chars())
                {
                    let mut alt = Secondary::ge(alt, bookmarks, line_start);
                    let to_remove = line_start..line_start + indent.len();
                    rope.remove(alt.remove(to_remove.clone()));
                    if to_remove.contains(&self.cursor) {
                        self.cursor = line_start;
                        self.cursor_column = 0;
                    } else {
                        self.cursor -= to_remove.end - to_remove.start;
                        self.cursor_column = cursor_column(&rope, self.cursor);
                    }

                    alt.update(|pos| {
                        if (line_start..line_start + indent.len()).contains(pos) {
                            *pos = line_start;
                        } else {
                            *pos -= indent.len();
                        }
                    });
                }
                None
            }
            selection_opt @ Some(_) => {
                let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
                let indent = match buf.tabs_required {
                    false => buf.tab_substitution.clone(),
                    true => "\t".to_string(),
                };
                let indent_chars = indent.chars().count();
                let mut unindent_lines = selected_lines(&buf.rope, self.cursor, selection_opt)
                    .filter_map(|line| (!line.is_empty()).then(|| line.into()))
                    .collect::<Vec<_>>();
                let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
                if unindent_lines
                    .iter()
                    .all(|l: &MultiCursor| l.can_unindent(&rope, &indent, indent_chars))
                {
                    let mut alt = Secondary::new(alt, bookmarks);
                    multicursor_update(
                        &mut unindent_lines,
                        |m| m.un_indent(&mut rope, &mut alt, indent_chars),
                        |m, ()| {
                            *m -= indent_chars;
                        },
                    );
                    if let Some(start) = unindent_lines.first()
                        && let Some(end) = unindent_lines.last()
                    {
                        self.selection = Some(start.range.start);
                        self.cursor = end.range.end;
                    }
                }
                None
            }
        }
    }

    /// Takes selection's lines, clears selection, and moves cursor to the last line
    pub fn selection_cursors(&mut self) -> Vec<MultiCursor> {
        let buf = self.buffer.borrow_move();

        let lines = selected_lines(&buf.rope, self.cursor, self.selection.take())
            .filter_map(|SelectedLine { start, end }| {
                (start != end).then_some(MultiCursor {
                    range: start..end,
                    cursor: start,
                    selection: None,
                    groups: vec![],
                })
            })
            .collect::<Vec<_>>();

        self.cursor = match lines.last() {
            Some(MultiCursor { cursor, .. }) => *cursor,
            None => return vec![],
        };

        lines
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
                        use unicode_width::UnicodeWidthChar;

                        // widen selection to current word
                        let word_start = rope
                            .chars_at(self.cursor)
                            .reversed()
                            .position(|c| !is_word(c) && c.width() != Some(0))
                            .and_then(|pos| self.cursor.checked_sub(pos))
                            .unwrap_or(0);

                        let word_end = rope
                            .chars_at(self.cursor)
                            .position(|c| !is_word(c) && c.width() != Some(0))
                            .map(|pos| self.cursor + pos)
                            .unwrap_or(rope.len_chars());

                        if word_start != word_end {
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

    /// Returns set of autocompletion matches and their offset in the rope,
    /// in characters.
    /// The partial word being autocompleted will be first in the Vec.
    fn autocomplete_matches(&self) -> Option<(usize, Vec<String>)> {
        use unicode_width::UnicodeWidthChar;

        let buf = &mut self.buffer.borrow();
        let rope = &buf.rope;

        if let Some(c) = rope.get_char(self.cursor)
            && is_word(c)
        {
            return None;
        }

        let prefix_chars = rope
            .chars_at(self.cursor)
            .reversed()
            .take_while(|c| is_word(*c) || c.width() == Some(0))
            .collect::<Vec<_>>();

        if prefix_chars.is_empty() {
            return None;
        }

        let prefix_start = self.cursor.checked_sub(prefix_chars.len())?;

        Some((
            prefix_start,
            autocomplete_matches(rope, prefix_chars.into_iter().rev().collect()),
        ))
    }

    /// Returns autocomplete matches for a Find prompt
    pub fn search_autocomplete_matches(&self, prefix: String) -> Vec<String> {
        autocomplete_matches(&self.buffer.borrow().rope, prefix)
    }

    /// Given a set of cursors, attempts to find a common search prefix.
    /// If found, returns set of completions.
    pub fn multi_autocomplete_matches(
        &self,
        cursors: &[MultiCursor],
    ) -> Option<(Vec<usize>, Vec<String>)> {
        let buf = &mut self.buffer.borrow();
        let rope = &buf.rope;

        let mut offsets = Vec::with_capacity(cursors.len());
        let mut prefix = None;

        for cursor in cursors {
            let (offset, p) = cursor.autocomplete_prefix(rope)?;
            offsets.push(offset);
            match &mut prefix {
                None => {
                    prefix = Some(p);
                }
                Some(prefix) => {
                    if prefix != &p {
                        return None;
                    }
                }
            }
        }

        Some((offsets, autocomplete_matches(rope, prefix?)))
    }

    pub fn autocomplete(
        &mut self,
        alt: Vec<AltCursor<'_>>,
        offset: usize,
        original: &str,
        replacement: &str,
    ) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::ge(alt, bookmarks, offset);

        // cut out original string at offset
        let original_chars = original.chars().count();
        let cut_range = offset..offset + original_chars;
        rope.remove(alt.remove(cut_range.clone()));
        alt -= original_chars;

        // insert replacement string at the same offset
        let replacement_chars = replacement.chars().count();
        rope.insert(offset, replacement);
        alt += replacement_chars;
        self.cursor = offset + replacement_chars;
        self.cursor_column = cursor_column(&rope, self.cursor);
        self.selection = None;
    }

    pub fn multi_autocomplete(
        &mut self,
        alt: Vec<AltCursor<'_>>,
        matches: &mut [MultiCursor],
        mut offsets: &[usize],
        original: &str,
        replacement: &str,
    ) {
        let original_chars = original.chars().count();
        let replacement_chars = replacement.chars().count();

        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::new(alt, bookmarks);

        multicursor_update(
            matches,
            |m| {
                let offset = offsets.split_off_first().ok_or(())?;
                m.autocomplete(
                    &mut rope,
                    &mut self.cursor,
                    &mut alt,
                    *offset,
                    original_chars,
                    replacement,
                    replacement_chars,
                );
                Ok::<_, ()>(())
            },
            |r, ()| {
                *r -= original_chars;
                *r += replacement_chars;
            },
        );
    }

    pub fn multi_insert_char(
        &mut self,
        alt: Vec<AltCursor<'_>>,
        matches: &mut [MultiCursor],
        c: char,
    ) {
        use std::convert::Infallible;

        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::new(alt, bookmarks);

        multicursor_update(
            matches,
            |m| Ok::<_, Infallible>(m.insert_char(&mut rope, &mut self.cursor, &mut alt, c)),
            |r, (zapped, inserted)| {
                *r -= zapped;
                *r += inserted;
            },
        );
    }

    pub fn multi_insert_group(
        &mut self,
        alt: Vec<AltCursor<'_>>,
        matches: &mut [MultiCursor],
        group_num: usize,
    ) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::new(alt, bookmarks);

        multicursor_update(
            matches,
            |m| {
                m.insert_group(&mut rope, &mut self.cursor, &mut alt, group_num)
                    .ok_or(())
            },
            |r, (zapped, inserted)| {
                *r -= zapped;
                *r += inserted;
            },
        );
    }

    pub fn multi_insert_string(
        &mut self,
        alt: Vec<AltCursor<'_>>,
        matches: &mut [MultiCursor],
        s: &str,
    ) {
        self.multi_insert_strings(alt, matches, std::iter::repeat((s.chars().count(), s)))
    }

    pub fn multi_paste(
        &mut self,
        alt: Vec<AltCursor<'_>>,
        matches: &mut [MultiCursor],
        cut: &mut EditorCutBuffer,
    ) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::new(alt, bookmarks);

        match cut {
            EditorCutBuffer::Single(cut) => {
                multicursor_update(
                    matches,
                    |m| {
                        let zapped = m.paste_single(&mut rope, &mut self.cursor, &mut alt, cut);
                        Ok::<_, std::convert::Infallible>(zapped)
                    },
                    |r, zapped| {
                        *r -= zapped;
                        *r += cut.chars_len;
                    },
                );
            }
            EditorCutBuffer::Multiple(cuts) => {
                let mut cuts_iter = cuts.iter_mut();
                let mut pasted = 0;
                multicursor_update(
                    matches,
                    |m| match cuts_iter.next() {
                        Some(cut) => {
                            pasted += 1;
                            Ok(m.paste_multiple(&mut rope, &mut self.cursor, &mut alt, cut))
                        }
                        None => {
                            m.selection = None;
                            Err(())
                        }
                    },
                    |r, (zapped, s_len)| {
                        *r -= zapped;
                        *r += s_len;
                    },
                );
                let cuts_len = cuts.len();
                cuts.rotate_left(pasted % cuts_len);
            }
        }
    }

    pub fn multi_insert_strings<'s>(
        &mut self,
        alt: Vec<AltCursor<'_>>,
        matches: &mut [MultiCursor],
        mut strings: impl std::iter::FusedIterator<Item = (usize, &'s str)>,
    ) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::new(alt, bookmarks);

        multicursor_update(
            matches,
            |m| {
                let (s_len, s) = strings.next().ok_or(())?;
                let zapped = m.insert_str(&mut rope, &mut self.cursor, &mut alt, s, s_len);
                Ok::<_, ()>((zapped, s_len))
            },
            |r, (zapped, s_len)| {
                *r -= zapped;
                *r += s_len;
            },
        );
    }

    pub fn multi_backspace(&mut self, alt: Vec<AltCursor<'_>>, matches: &mut [MultiCursor]) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::new(alt, bookmarks);

        multicursor_update(
            matches,
            |m| m.backspace(&mut rope, &mut self.cursor, &mut alt),
            |r, removed| {
                *r -= removed;
            },
        );
    }

    pub fn multi_delete(&mut self, alt: Vec<AltCursor<'_>>, matches: &mut [MultiCursor]) {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::new(alt, bookmarks);

        multicursor_update(
            matches,
            |m| m.delete(&mut rope, &mut self.cursor, &mut alt),
            |r, removed| {
                *r -= removed;
            },
        );
    }

    pub fn multi_cursor_back(&mut self, matches: &mut [MultiCursor], selecting: bool) {
        matches.iter_mut().for_each(|m| {
            m.cursor_back(
                &mut self.cursor,
                &mut self.cursor_column,
                &self.buffer.borrow_move().rope,
                selecting,
            )
        });
    }

    pub fn multi_cursor_forward(&mut self, matches: &mut [MultiCursor], selecting: bool) {
        matches.iter_mut().for_each(|m| {
            m.cursor_forward(
                &mut self.cursor,
                &mut self.cursor_column,
                &self.buffer.borrow_move().rope,
                selecting,
            )
        });
    }

    pub fn multi_cursor_home(&mut self, matches: &mut [MultiCursor], selecting: bool) {
        matches.iter_mut().for_each(|m| {
            m.cursor_home(
                &mut self.cursor,
                &mut self.cursor_column,
                &self.buffer.borrow_move().rope,
                selecting,
            )
        });
    }

    pub fn multi_cursor_end(&mut self, matches: &mut [MultiCursor], selecting: bool) {
        matches.iter_mut().for_each(|m| {
            m.cursor_end(
                &mut self.cursor,
                &mut self.cursor_column,
                &self.buffer.borrow_move().rope,
                selecting,
            )
        });
    }

    pub fn multi_cursor_copy(&mut self, matches: &mut [MultiCursor]) -> Option<EditorCutBuffer> {
        let buffer = &self.buffer.borrow();

        let copies = matches
            .iter_mut()
            .filter_map(|m| m.get_selection(&buffer.rope, &buffer.bookmarks))
            .collect::<Vec<_>>();

        match copies.len() {
            0 => None,
            1 => {
                self.message = Some(BufferMessage::Notice("Copied 1 Item".into()));
                Some(EditorCutBuffer::Multiple(copies))
            }
            n => {
                self.message = Some(BufferMessage::Notice(format!("Copied {n} Items").into()));
                Some(EditorCutBuffer::Multiple(copies))
            }
        }
    }

    pub fn multi_cursor_cut(
        &mut self,
        alt: Vec<AltCursor<'_>>,
        matches: &mut [MultiCursor],
    ) -> Option<EditorCutBuffer> {
        let mut buf = self.buffer.borrow_update(self.cursor, self.cursor_column);
        let (mut rope, bookmarks) = buf.rope_bookmarks_mut();
        let mut alt = Secondary::new(alt, bookmarks);
        let mut cut_buffers = vec![];

        multicursor_update(
            matches,
            |m| match m.take_selection(&mut rope, &mut self.cursor, &mut alt) {
                Some(cut) => {
                    let removed = cut.chars_len;
                    cut_buffers.push(cut);
                    Ok::<_, std::convert::Infallible>(removed)
                }
                None => Ok(0),
            },
            |r, removed| {
                *r -= removed;
            },
        );

        match cut_buffers.len() {
            0 => None,
            1 => {
                self.message = Some(BufferMessage::Notice("Cut 1 Item".into()));
                Some(EditorCutBuffer::Multiple(cut_buffers))
            }
            n => {
                self.message = Some(BufferMessage::Notice(format!("Cut {n} Items").into()));
                Some(EditorCutBuffer::Multiple(cut_buffers))
            }
        }
    }

    pub fn multi_cursor_widen(&mut self, matches: &mut [MultiCursor]) {
        matches
            .iter_mut()
            .for_each(|m| m.widen_selection(&mut self.cursor));
    }

    pub fn set_error<S: Into<Cow<'static, str>>>(&mut self, err: S) {
        self.message = Some(BufferMessage::Error(err.into()))
    }

    pub fn set_message<S: Into<Cow<'static, str>>>(&mut self, msg: S) {
        self.message = Some(BufferMessage::Notice(msg.into()))
    }

    pub fn alt_cursor(&mut self) -> AltCursor<'_> {
        AltCursor {
            cursor: &mut self.cursor,
            selection: &mut self.selection,
        }
    }

    pub fn help_options(&self, multiple_buffers: bool, multiple_panes: bool) -> Help {
        let buffer = &self.buffer.borrow();
        let has_bookmarks = buffer.has_bookmarks();
        let rope = &buffer.rope;

        match self.selection {
            Some(selection) => Help {
                find: if rope.try_char_to_line(self.cursor).ok()
                    == rope.try_char_to_line(selection).ok()
                {
                    FindMode::Selected
                } else {
                    FindMode::InSelection
                },
                has_bookmarks,
                cursor_pos: CursorPos::default(),
                has_selection: true,
                multiple_buffers,
                multiple_panes,
            },
            None => {
                let current_char = rope.get_char(self.cursor);
                Help {
                    find: FindMode::WholeFile,
                    has_bookmarks,
                    cursor_pos: match current_char {
                        Some('(' | ')' | '{' | '}' | '[' | ']' | '<' | '>') => CursorPos::AtParen,
                        Some(c) if is_word(c) => CursorPos::InWord,
                        _ => match self
                            .cursor
                            .checked_sub(1)
                            .and_then(|prev| rope.get_char(prev))
                        {
                            Some(c) => {
                                if is_word(c) {
                                    CursorPos::AfterWord
                                } else {
                                    CursorPos::default()
                                }
                            }
                            None => CursorPos::default(),
                        },
                    },
                    has_selection: false,
                    multiple_buffers,
                    multiple_panes,
                }
            }
        }
    }

    /// If no bookmark at cursor, add one
    /// If bookmark at cursor, remove one
    pub fn toggle_bookmark(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        self.message = Some(BufferMessage::Notice(
            (match buf.bookmarks.toggle(self.cursor) {
                Toggle::Inserted => "Bookmark Added",
                Toggle::Removed => "Bookmark Removed",
            })
            .into(),
        ))
    }

    pub fn toggle_bookmarks(&mut self, positions: impl Iterator<Item = usize>) {
        let mut added = 0;
        let mut removed = 0;
        let mut buf = self.buffer.borrow_mut();

        for pos in positions {
            match buf.bookmarks.toggle(pos) {
                Toggle::Inserted => {
                    added += 1;
                }
                Toggle::Removed => {
                    removed += 1;
                }
            }
        }

        match (added, removed) {
            (0, 0) => {}
            (1, 0) => {
                self.message = Some(BufferMessage::Notice("Bookmark Added".into()));
            }
            (n, 0) => {
                self.message = Some(BufferMessage::Notice(format!("{n} Bookmarks Added").into()));
            }
            (0, 1) => {
                self.message = Some(BufferMessage::Notice("Bookmark Removed".into()));
            }
            (0, n) => {
                self.message = Some(BufferMessage::Notice(
                    format!("{n} Bookmarks Removed").into(),
                ));
            }
            _ => {
                // an unusual case
                self.message = Some(BufferMessage::Notice("Bookmarks Toggled".into()));
            }
        }
    }

    /// If cursor at a bookmark, delete it
    pub fn delete_bookmark(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        if let Ok(()) = buf.bookmarks.remove(self.cursor) {
            self.message = Some(BufferMessage::Notice("Bookmark Removed".into()));
        }
    }

    fn goto_bookmark(&mut self, forward: bool) {
        let buf = self.buffer.borrow_move();
        let Some(cursor) = (if forward {
            buf.bookmarks.next_after(self.cursor)
        } else {
            buf.bookmarks.next_before(self.cursor)
        }) else {
            return;
        };
        self.cursor = cursor;
        self.cursor_column = cursor_column(&buf.rope, self.cursor);
        self.selection = None;
    }

    /// Moves to next bookmark, if any
    pub fn next_bookmark(&mut self) {
        self.goto_bookmark(true);
    }

    /// Moves to previous bookmark, if any
    pub fn previous_bookmark(&mut self) {
        self.goto_bookmark(false);
    }
}

pub struct AltCursor<'a> {
    cursor: &'a mut usize,
    selection: &'a mut Option<usize>,
}

pub struct MultiCursor {
    /// cursor's range within rope, in characters
    range: Range<usize>,
    /// cursor's position in rope, in characters
    cursor: usize,
    /// cursor's selection anchor in rope, in characters, if any
    selection: Option<usize>,
    /// regular expression capture groups
    groups: Vec<String>,
}

impl MultiCursor {
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Widens select to whole multi-cursor area
    pub fn widen_selection(&mut self, cursor: &mut usize) {
        self.selection = Some(self.range.start);
        if *cursor == self.cursor {
            *cursor = self.range.end;
        }
        self.cursor = self.range.end;
    }

    /// If a selection is active, removes all characters
    /// between start and end of selection, erases selection,
    /// and moves cursor to start of selection.
    /// Returns number of characters removed from selection, if any.
    fn zap_selection(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
    ) -> Option<usize> {
        let (start, end) = reorder(self.cursor, self.selection.take()?);
        let removed = end - start;
        rope.try_remove(secondary.remove(start..end)).ok()?;
        if end <= *cursor {
            *cursor -= removed;
        } else if start <= *cursor {
            *cursor = start;
        }
        secondary.update(|a| {
            if start <= *a {
                *a -= removed;
            }
        });
        self.cursor = start;
        self.range.end -= removed;
        Some(removed)
    }

    /// Returns number of characters zapped (0 or more)
    /// and number of characters inserted (1 or 2)
    #[must_use]
    fn insert_char(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
        c: char,
    ) -> (usize, usize) {
        use std::cmp::Ordering;

        let zapped = self
            .zap_selection(rope, cursor, secondary)
            .unwrap_or_default();
        let inserted = insert_char_or_pair(rope, self.cursor, secondary, c);
        *cursor += match self.cursor.cmp(cursor) {
            Ordering::Less => inserted,
            Ordering::Equal => 1,
            Ordering::Greater => 0,
        };
        self.cursor += 1;
        self.range.end += inserted;
        (zapped, inserted)
    }

    /// Returns number of zapped characters, if any
    #[must_use]
    fn insert_str(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
        s: &str,
        s_chars: usize,
    ) -> usize {
        let zapped = self
            .zap_selection(rope, cursor, secondary)
            .unwrap_or_default();
        if self.cursor <= *cursor {
            *cursor += s_chars;
        }
        secondary.update(|a| {
            if self.cursor <= *a {
                *a += s_chars;
            }
        });
        rope.insert(self.cursor, s);
        self.cursor += s_chars;
        self.range.end += s_chars;
        zapped
    }

    /// Returns number of zapped characters, if any
    #[must_use]
    fn paste_single(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
        cut: &CutBuffer,
    ) -> usize {
        let zapped = self
            .zap_selection(rope, cursor, secondary)
            .unwrap_or_default();

        if self.cursor <= *cursor {
            *cursor += cut.chars_len;
        }
        secondary.update(|a| {
            if self.cursor <= *a {
                *a += cut.chars_len;
            }
        });
        rope.insert(self.cursor, cut.as_str());
        secondary.add_bookmarks(cut.bookmarks.iter().map(|b| self.cursor + b));
        self.cursor += cut.chars_len;
        self.range.end += cut.chars_len;
        zapped
    }

    /// Returns number of zapped characters and added characters
    #[must_use]
    fn paste_multiple(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
        cut: &mut CutBuffer,
    ) -> (usize, usize) {
        match self.take_selection(rope, cursor, secondary) {
            None => (
                self.paste_single(rope, cursor, secondary, cut),
                cut.chars_len,
            ),
            Some(mut old_selection) => {
                let zapped = old_selection.chars_len;
                let added = cut.chars_len;
                // taking the selection clears the selection
                // so there's nothing left for paste_single to zap
                assert_eq!(self.paste_single(rope, cursor, secondary, cut), 0);
                std::mem::swap(&mut old_selection, cut);
                (zapped, added)
            }
        }
    }

    fn indent(
        &mut self,
        rope: &mut ropey::Rope,
        secondary: &mut Secondary,
        indent: &str,
        indent_chars: usize,
    ) {
        // there shouldn't be a selection anchor to clear
        secondary.update(|a| {
            if self.range.start <= *a {
                *a += indent_chars;
            }
        });
        rope.insert(self.range.start, indent);
        self.cursor += indent_chars;
        self.range.end += indent_chars;
    }

    fn can_unindent(&self, rope: &ropey::Rope, indent: &str, indent_len: usize) -> bool {
        rope.chars_at(self.range.start)
            .take(indent_len)
            .eq(indent.chars())
    }

    fn un_indent(
        &mut self,
        rope: &mut ropey::Rope,
        secondary: &mut Secondary,
        indent_chars: usize,
    ) -> Result<(), ()> {
        // there shouldn't be a selection anchor to clear
        rope.try_remove(secondary.remove(self.range.start..self.range.start + indent_chars))
            .map_err(|_| ())?;
        self.cursor = self.cursor.saturating_sub(indent_chars);
        self.range.end -= indent_chars;

        secondary.update(|a| {
            if self.range.start <= *a {
                *a = a.saturating_sub(indent_chars);
            }
        });
        Ok(())
    }

    /// Returns Ok and number of characters removed
    /// if backspace performed successfully
    fn backspace(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
    ) -> Result<usize, ()> {
        use std::cmp::Ordering;

        if self.cursor <= self.range.start {
            // can't backup before start of range
            return Err(());
        }
        match self.zap_selection(rope, cursor, secondary) {
            None => {
                let (before, after) = backspace_or_un_pair(rope, self.cursor, secondary)?;
                let total = before + after;
                *cursor -= match self.cursor.cmp(cursor) {
                    Ordering::Less => total,
                    Ordering::Equal => before,
                    Ordering::Greater => 0,
                };
                self.cursor -= before;
                self.range.end -= total;
                Ok(total)
            }
            Some(removed) => Ok(removed),
        }
    }

    /// Returns Ok if delete performed successfully
    fn delete(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
    ) -> Result<usize, ()> {
        if self.cursor < self.range.end {
            match self.zap_selection(rope, cursor, secondary) {
                None => {
                    use unicode_width::UnicodeWidthChar;

                    let to_delete = rope
                        .chars_at(self.cursor)
                        .skip(1)
                        .take_while(|c| c.width() == Some(0))
                        .count()
                        + 1;

                    if self.cursor < *cursor {
                        *cursor = cursor.saturating_sub(to_delete);
                    }
                    secondary.update(|a| {
                        if self.cursor < *a {
                            *a = a.saturating_sub(to_delete);
                        }
                    });
                    let _ = rope.try_remove(secondary.remove(self.cursor..self.cursor + to_delete));
                    self.range.end -= to_delete;
                    Ok(to_delete)
                }
                Some(zapped) => Ok(zapped),
            }
        } else {
            Err(())
        }
    }

    fn cursor_back(
        &mut self,
        cursor: &mut usize,
        cursor_col: &mut usize,
        rope: &ropey::Rope,
        selecting: bool,
    ) {
        use unicode_width::UnicodeWidthChar;

        let to_retreat = rope
            .chars_at(self.cursor)
            .reversed()
            .take_while(|c| c.width() == Some(0))
            .count()
            + 1;

        if self.cursor > self.range.start {
            if self.cursor == *cursor {
                *cursor = cursor.saturating_sub(to_retreat);
                *cursor_col = cursor_column(rope, *cursor);
            }
            update_selection(&mut self.selection, self.cursor, selecting);
            self.cursor -= to_retreat;
        } else if !selecting {
            self.selection = None;
        }
    }

    fn cursor_forward(
        &mut self,
        cursor: &mut usize,
        cursor_col: &mut usize,
        rope: &ropey::Rope,
        selecting: bool,
    ) {
        use unicode_width::UnicodeWidthChar;

        if self.cursor < self.range.end {
            let to_advance = rope
                .chars_at(self.cursor + 1)
                .take_while(|c| c.width() == Some(0))
                .count()
                + 1;

            if self.cursor == *cursor {
                *cursor += to_advance;
                *cursor_col = cursor_column(rope, *cursor);
            }
            update_selection(&mut self.selection, self.cursor, selecting);
            self.cursor += to_advance;
        } else if !selecting {
            self.selection = None;
        }
    }

    fn cursor_home(
        &mut self,
        cursor: &mut usize,
        cursor_col: &mut usize,
        rope: &ropey::Rope,
        selecting: bool,
    ) {
        if self.cursor == *cursor {
            *cursor = self.range.start;
            *cursor_col = cursor_column(rope, *cursor);
        }
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = self.range.start;
    }

    fn cursor_end(
        &mut self,
        cursor: &mut usize,
        cursor_col: &mut usize,
        rope: &ropey::Rope,
        selecting: bool,
    ) {
        if self.cursor == *cursor {
            *cursor = self.range.end;
            *cursor_col = cursor_column(rope, *cursor);
        }
        update_selection(&mut self.selection, self.cursor, selecting);
        self.cursor = self.range.end;
    }

    /// If cursor is just past a word, return prefix and prefix's offset
    /// where offset is relative to start of this cursor's range
    /// (*not* start of entire rope)
    fn autocomplete_prefix(&self, rope: &ropey::Rope) -> Option<(usize, String)> {
        use unicode_width::UnicodeWidthChar;

        if let Some(c) = rope.get_char(self.cursor)
            && is_word(c)
        {
            return None;
        }

        let prefix_chars = rope
            .chars_at(self.cursor)
            .reversed()
            // don't walk past start of cursor's range
            .take(self.cursor - self.range.start)
            .take_while(|c| is_word(*c) || c.width() == Some(0))
            .collect::<Vec<_>>();

        if prefix_chars.is_empty() {
            return None;
        }

        let offset = self
            .cursor
            .checked_sub(prefix_chars.len())?
            .checked_sub(self.range.start)?;

        Some((offset, prefix_chars.into_iter().rev().collect()))
    }

    /// Clears out everything from offset..offset + original_chars
    /// Replaces it with replacement string which has the given chars length
    // Yes, I know it has a lot of arguments
    #[allow(clippy::too_many_arguments)]
    fn autocomplete(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary<'_, '_>,
        offset: usize,
        original_chars: usize,
        replacement: &str,
        replacement_chars: usize,
    ) {
        self.selection = None;

        // make relative offset an absolute offset
        let abs_start = self.range.start + offset;

        // remove old autocomplete match
        rope.remove(secondary.remove(abs_start..abs_start + original_chars));
        if self.range.start <= *cursor {
            *cursor -= original_chars;
        }
        secondary.update(|a| {
            if self.range.start <= *a {
                *a -= original_chars;
            }
        });
        self.range.end -= original_chars;

        // replace with new autocomplete match
        rope.insert(abs_start, replacement);
        if self.range.start <= *cursor {
            *cursor += replacement_chars;
        }
        secondary.update(|a| {
            if self.range.start <= *a {
                *a += replacement_chars;
            }
        });

        // place cursor at end of replacement
        // (which isn't necessarily the end of its total range)
        self.cursor = abs_start + replacement_chars;
        self.range.end += replacement_chars;
    }

    fn selection_range(&self) -> Option<Range<usize>> {
        self.selection.map(|sel| {
            let (start, end) = reorder(self.cursor, sel);
            start..end
        })
    }

    /// Retrieves current selection (if any)
    /// and also clears selection
    fn get_selection(
        &mut self,
        rope: &ropey::Rope,
        bookmarks: &private::Bookmarks,
    ) -> Option<CutBuffer> {
        let (start, end) = reorder(self.cursor, self.selection.take()?);
        Some(CutBuffer::new(
            rope.get_slice(start..end)?,
            bookmarks.range(start..end).map(|b| b - start),
        ))
    }

    fn take_selection(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
    ) -> Option<CutBuffer> {
        // a lot like zap_selection, except we're saving the selection
        let (start, end) = reorder(self.cursor, self.selection.take()?);

        let cut = CutBuffer::new(
            rope.get_slice(start..end)?,
            secondary.extract_bookmarks(start..end).map(|b| b - start),
        );

        let removed = end - start;
        rope.try_remove(secondary.remove(start..end)).ok()?;
        if end <= *cursor {
            *cursor -= removed;
        } else if start <= *cursor {
            *cursor = start;
        }
        secondary.update(|a| {
            if start <= *a {
                *a -= removed;
            }
        });
        self.cursor = start;
        self.range.end -= removed;

        Some(cut)
    }

    /// If this contains at least 1 capture group, returns the total
    pub fn paste_group_count(&self) -> Option<NonZero<usize>> {
        NonZero::new(self.groups.len())
    }

    /// Inserts the given capture group at the current position
    /// Returns number of zapped characters
    /// and number of inserted characters, if any.
    pub fn insert_group(
        &mut self,
        rope: &mut ropey::Rope,
        cursor: &mut usize,
        secondary: &mut Secondary,
        group_num: usize,
    ) -> Option<(usize, usize)> {
        let s = self.groups.get(group_num).cloned()?;
        let s_chars = s.chars().count();
        let zapped = self.insert_str(rope, cursor, secondary, &s, s_chars);
        Some((zapped, s_chars))
    }
}

impl From<usize> for MultiCursor {
    fn from(cursor: usize) -> Self {
        Self {
            range: cursor..cursor,
            cursor,
            selection: None,
            groups: vec![],
        }
    }
}

impl From<Range<usize>> for MultiCursor {
    fn from(range: Range<usize>) -> Self {
        Self {
            cursor: range.end,
            selection: Some(range.start),
            range,
            groups: vec![],
        }
    }
}

impl From<(Range<usize>, Vec<String>)> for MultiCursor {
    fn from((range, groups): (Range<usize>, Vec<String>)) -> Self {
        Self {
            cursor: range.end,
            selection: Some(range.start),
            range,
            groups,
        }
    }
}

impl From<SelectedLine> for MultiCursor {
    fn from(line: SelectedLine) -> Self {
        Self {
            cursor: line.start,
            range: line.start..line.end,
            selection: None,
            groups: vec![],
        }
    }
}

impl std::ops::AddAssign<usize> for MultiCursor {
    fn add_assign(&mut self, chars: usize) {
        self.range.start += chars;
        self.range.end += chars;
        self.cursor += chars;
        if let Some(selection) = &mut self.selection {
            *selection += chars;
        }
    }
}

impl std::ops::SubAssign<usize> for MultiCursor {
    fn sub_assign(&mut self, chars: usize) {
        self.range.start -= chars;
        self.range.end -= chars;
        self.cursor -= chars;
        if let Some(selection) = &mut self.selection {
            *selection -= chars;
        }
    }
}

fn multicursor_update<T: Copy, E>(
    mut cursors: &mut [MultiCursor],
    mut on_cursor: impl FnMut(&mut MultiCursor) -> Result<T, E>,
    mut on_next: impl FnMut(&mut MultiCursor, T),
) {
    loop {
        match cursors {
            [] => break,
            [m] => {
                let _ = on_cursor(m);
                break;
            }
            [m, rest @ ..] => {
                if let Ok(t) = on_cursor(m) {
                    for r in rest.iter_mut() {
                        on_next(r, t);
                    }
                }
                cursors = rest;
            }
        }
    }
}

/// Returns number of characters inserted (1 or 2)
fn insert_char_or_pair(
    rope: &mut ropey::Rope,
    cursor: usize,
    alt: &mut Secondary,
    c: char,
) -> usize {
    match match c {
        '(' => Err("()"),
        '[' => Err("[]"),
        '{' => Err("{}"),
        c => Ok(c),
    } {
        Ok(c) => {
            rope.insert_char(cursor, c);
            alt.update(|a| {
                if *a >= cursor {
                    *a += 1;
                }
            });
            1
        }
        Err(s) => {
            rope.insert(cursor, s);
            alt.update(|a| {
                *a += match (*a).cmp(&cursor) {
                    std::cmp::Ordering::Greater => 2,
                    std::cmp::Ordering::Equal => 1,
                    std::cmp::Ordering::Less => 0,
                };
            });
            2
        }
    }
}

/// On success, returns number of characters removed before and after cursor
fn backspace_or_un_pair(
    rope: &mut ropey::Rope,
    cursor: usize,
    alt: &mut Secondary,
) -> Result<(usize, usize), ()> {
    use unicode_width::UnicodeWidthChar;

    fn remove_pair(
        rope: &mut ropey::Rope,
        alt: &mut Secondary,
        prev: usize,
        cursor: usize,
    ) -> Result<(usize, usize), ()> {
        rope.try_remove(alt.remove(prev..cursor + 1))
            .map_err(|_| ())?;
        alt.update(|a| {
            *a -= match (*a).cmp(&cursor) {
                std::cmp::Ordering::Greater => 2,
                std::cmp::Ordering::Equal => 1,
                std::cmp::Ordering::Less => 0,
            };
        });
        Ok((1, 1))
    }

    let prev = cursor.checked_sub(1).ok_or(())?;
    match rope.get_char(prev).ok_or(())? {
        '(' if rope.get_char(cursor) == Some(')') => remove_pair(rope, alt, prev, cursor),
        '[' if rope.get_char(cursor) == Some(']') => remove_pair(rope, alt, prev, cursor),
        '{' if rope.get_char(cursor) == Some('}') => remove_pair(rope, alt, prev, cursor),
        c if c.width() == Some(0) => {
            let removed = rope
                .chars_at(cursor)
                .reversed()
                .take_while(|c| c.width() == Some(0))
                .count()
                + 1;

            rope.try_remove(alt.remove(cursor.saturating_sub(removed)..cursor))
                .map_err(|_| ())?;
            alt.update(|a| {
                if *a >= cursor {
                    *a -= removed;
                }
            });
            Ok((removed, 0))
        }
        _ => {
            // the common case
            rope.try_remove(alt.remove(prev..cursor)).map_err(|_| ())?;
            alt.update(|a| {
                if *a >= cursor {
                    *a -= 1;
                }
            });
            Ok((1, 0))
        }
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

pub trait SearchTerm: std::fmt::Display {
    /// Returns iterator of match ranges in bytes and any captured groups
    fn match_ranges(&self, s: &str) -> impl Iterator<Item = SearchMatch>;
}

pub struct SearchMatch {
    start: usize,
    end: usize,
    groups: Vec<String>,
}

impl std::ops::Add<usize> for SearchMatch {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        Self {
            start: self.start + rhs,
            end: self.end + rhs,
            groups: self.groups,
        }
    }
}

impl SearchTerm for fancy_regex::Regex {
    fn match_ranges(&self, s: &str) -> impl Iterator<Item = SearchMatch> {
        self.captures_iter(s).filter_map(|c| c.ok()).map(|c| {
            // guaranteed to have at least one capture
            let first = c.get(0).unwrap();
            SearchMatch {
                start: first.start(),
                end: first.end(),
                groups: c
                    .iter()
                    .map(|m| m.map(|m| m.as_str().to_string()).unwrap_or_default())
                    .collect(),
            }
        })
    }
}

impl SearchTerm for String {
    fn match_ranges(&self, s: &str) -> impl Iterator<Item = SearchMatch> {
        s.match_indices(self.as_str()).map(|(idx, s)| SearchMatch {
            start: idx,
            end: idx + s.len(),
            groups: vec![],
        })
    }
}

pub struct Normalizations(std::collections::HashSet<String>);

impl TryFrom<String> for Normalizations {
    type Error = String;

    fn try_from(base: String) -> Result<Self, Self::Error> {
        use unicode_normalization::UnicodeNormalization;

        [
            base.nfc().collect(),
            base.nfkd().collect(),
            base.nfd().collect(),
            base.nfkc().collect(),
        ]
        .into_iter()
        .fold(Err(base), |acc, s| match acc {
            Ok(mut map) => {
                map.0.insert(s);
                Ok(map)
            }
            Err(mut current) => {
                if current == s {
                    Err(current)
                } else {
                    Ok(Self([std::mem::take(&mut current), s].into()))
                }
            }
        })
    }
}

impl std::fmt::Display for Normalizations {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.0.iter().min_by_key(|s| s.len()) {
            Some(s) => s.fmt(f),
            None => "".fmt(f), // this shouldn't happen
        }
    }
}

impl SearchTerm for Normalizations {
    fn match_ranges(&self, s: &str) -> impl Iterator<Item = SearchMatch> {
        let mut ranges = self
            .0
            .iter()
            .flat_map(|string| string.match_ranges(s))
            .collect::<Vec<_>>();
        ranges.sort_unstable_by_key(|r| r.start);
        ranges.into_iter()
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

impl SelectedLine {
    fn is_empty(&self) -> bool {
        self.start >= self.end
    }
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

    while let Some(c) = chars.next()
        && c.width() == Some(0)
    {
        line_start += 1;
    }

    line_start
}

// Returns characters from the cursor's line start
// up to (not not including) the cursor itself
fn line_start_to_cursor(rope: &ropey::Rope, cursor: usize) -> Option<impl Iterator<Item = char>> {
    let line = rope.try_char_to_line(cursor).ok()?;
    let start = rope.try_line_to_char(line).ok()?;
    rope.get_chars_at(start)
        .map(|iter| iter.take(cursor.saturating_sub(start)))
}

/// Returns all characters in cursor's line
fn line_chars(rope: &ropey::Rope, cursor: usize) -> Option<impl Iterator<Item = char>> {
    line_char_range(rope, rope.try_char_to_line(cursor).ok()?).and_then(|(start, end)| {
        rope.get_chars_at(start)
            .map(|iter| iter.take(end.saturating_sub(start)))
    })
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
    if rope
        .try_remove(secondary.remove(selection_start..selection_end))
        .is_ok()
    {
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
    alt: Vec<AltCursor<'_>>,
    bookmarks: private::BookmarksHandle<'_>,
    [start, end]: [char; 2],
) {
    {
        let (start_pos, end_pos) = reorder(&mut *cursor, selection);
        let mut alt = Secondary::ge(alt, bookmarks, *start_pos);
        let _ = rope.try_insert_char(*end_pos, end);
        let _ = rope.try_insert_char(*start_pos, start);
        alt.update(|pos| *pos += if *pos > *end_pos { 2 } else { 1 });
        *start_pos += 1;
        *end_pos += 1;
    }
    *cursor_col = cursor_column(rope, *cursor);
}

/// Returns Ok is surround performed, or Err(Secondary) if not
fn delete_surround<'s, 'm>(
    rope: &mut ropey::Rope,
    cursor: &mut usize,
    cursor_col: &mut usize,
    selection: &mut usize,
    alt: Vec<AltCursor<'s>>,
    bookmarks: private::BookmarksHandle<'m>,
) -> Result<(), Secondary<'s, 'm>> {
    let (start, end) = reorder(&mut *cursor, selection);
    let mut alt = Secondary::ge(alt, bookmarks, *start);

    if let Some(prev_pos) = start.checked_sub(1)
        && let Some(prev_char) = rope.get_char(prev_pos)
        && let Some(next_char) = rope.get_char(*end)
        && matches!(
            (prev_char, next_char),
            ('(', ')') | ('[', ']') | ('{', '}') | ('<', '>') | ('"', '"') | ('\'', '\'')
        )
    {
        let _ = rope.try_remove(alt.remove(*end..*end + 1));
        let _ = rope.try_remove(alt.remove(prev_pos..*start));
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

    pub fn multiple_buffers(&self) -> bool {
        self.buffers.len() > 1
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
    pub fn cursor_viewport_position(&self, viewport_height: usize) -> Option<(usize, usize)> {
        let buf = self.current()?;
        buf.cursor_position()
            .map(|(row, col)| ((viewport_height / 2).min(row), col))
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
}

pub struct BufferWidget<'e> {
    pub focused: bool,
    pub mode: Option<&'e mut EditorMode>,
    pub show_help: Option<Help>,
    pub show_sub_help: bool,
}

impl BufferWidget<'_> {
    pub const RIGHT_MARGIN: u16 = 5;
}

impl BufferWidget<'_> {
    pub fn viewport_height(area: Rect) -> usize {
        use ratatui::{
            layout::{
                Constraint::{Length, Min},
                Layout,
            },
            widgets::Block,
        };

        let block = Block::bordered();

        let [text_area, _] = Layout::horizontal([Min(0), Length(1)]).areas(block.inner(area));

        text_area.height.into()
    }
}

impl StatefulWidget for BufferWidget<'_> {
    type State = BufferContext;

    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer, state: &mut BufferContext) {
        use crate::editor::SearchType;
        use crate::help::{
            CONFIRM_CLOSE, PASTE_GROUP, REPLACE_MATCHES, SELECT_INSIDE, SELECT_LINE,
            SELECT_LINE_BOOKMARKED, SPLIT_PANE, VERIFY_RELOAD, VERIFY_SAVE, render_help,
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
                Block, BorderType, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
                ScrollbarState, Widget,
            },
        };
        use std::borrow::Cow;
        use std::collections::{BTreeMap, VecDeque};
        use std::ops::RangeInclusive;

        const EDITING: Style = Style::new().add_modifier(Modifier::REVERSED);
        const MATCHING: Color = Color::Green;
        const MISMATCH: Color = Color::Red;
        const BOOKMARK: Color = Color::Cyan;

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

        fn widen_tabs<'l>(mut input: Line<'l>) -> Line<'l> {
            fn tabs_to_spaces(s: &mut Cow<'_, str>) {
                if s.as_ref().contains('\t') {
                    *s = Cow::Owned(s.as_ref().replace('\t', &TAB_SUBSTITUTION));
                }
            }

            input
                .spans
                .iter_mut()
                .for_each(|s| tabs_to_spaces(&mut s.content));
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
            fn colorize<'r, R: FromRange<'r>, S: Highlighter>(
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
                        Style::from(highlight),
                    ));
                    idx = range.end;
                }
                let last = text.extract_range_from(idx..);
                if !last.as_ref().is_empty() {
                    elements.push(Span::raw(last));
                }
                match syntax.underline() {
                    None => elements,
                    Some(underline) => add_underlines(underline(text.as_ref()), elements),
                }
            }

            fn add_underlines<'r>(
                underlines: impl Iterator<Item = std::ops::Range<usize>>,
                elements: Vec<Span<'r>>,
            ) -> Vec<Span<'r>> {
                let mut underlines = underlines.peekable();
                if underlines.peek().is_none() {
                    // nothing to underline (the common case)
                    return elements;
                }

                let mut output = Vec::with_capacity(elements.len());
                let mut input = elements.into();
                let mut idx = 0;
                for underline in underlines {
                    extract_bytes(&mut input, underline.start - idx, &mut output, |span| span);
                    extract_bytes(
                        &mut input,
                        underline.end - underline.start,
                        &mut output,
                        |span| {
                            span.patch_style(
                                Style::new().underlined().underline_color(Color::DarkGray),
                            )
                        },
                    );
                    idx = underline.end;
                }
                output.extend(input);
                output
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
                    Cow::Borrowed(s) => colorize(syntax, state, s.trim_end_matches('\n')),
                    Cow::Owned(s) => colorize(syntax, state, trim_string_matches(s, '\n')),
                }
            } else {
                highlight_trailing_whitespace(match text {
                    Cow::Borrowed(s) => colorize(syntax, state, s.trim_end_matches('\n')),
                    Cow::Owned(s) => colorize(syntax, state, trim_string_matches(s, '\n')),
                })
            }
        }

        /// Widens line of spans by 1, for appending something at the end
        fn widen<'s>(mut line: Vec<Span<'s>>) -> Vec<Span<'s>> {
            line.push(Span::raw(" "));
            line
        }

        /// Widens range by 1, for appending something at the end
        fn widen_range(range: std::ops::RangeInclusive<usize>) -> std::ops::RangeInclusive<usize> {
            let (s, e) = range.into_inner();
            s..=e + 1
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

        fn extract_bytes<'s>(
            input: &mut VecDeque<Span<'s>>,
            mut bytes: usize,
            output: &mut Vec<Span<'s>>,
            map: impl Fn(Span<'s>) -> Span<'s>,
        ) {
            fn split_cow(s: Cow<'_, str>, bytes: usize) -> (Cow<'_, str>, Cow<'_, str>) {
                let split_point = if bytes < s.len() {
                    bytes
                } else {
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

            while bytes > 0 {
                let Some(span) = input.pop_front() else {
                    return;
                };
                let span_width = span.content.len();
                if span_width <= bytes {
                    bytes -= span_width;
                    output.push(map(span));
                } else {
                    let (prefix, suffix) = split_cow(span.content, bytes);
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
        // and returns text in those ranges highlighted in some style
        fn highlight_matches<'s>(
            colorized: Vec<Span<'s>>,
            line_range: RangeInclusive<usize>,
            matches: &mut VecDeque<Range<usize>>,
            apply: impl Fn(Span<'s>) -> Span<'s> + Copy,
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
                let Some(match_range) = matches.pop_front() else {
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
                    apply,
                );

                // push any remaining partial match back into VecDeque
                if !match_range.is_empty() {
                    matches.push_front(match_range.into());
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
            highlight: impl Fn(Span<'s>) -> Span<'s>,
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
                    highlight,
                );

                // output any remaining characters verbatim
                highlighted.extend(colorized);

                highlighted
            }
        }

        fn highlight_parens<'s>(
            colorized: Vec<Span<'s>>,
            line_range: RangeInclusive<usize>,
            parens: &mut VecDeque<(usize, Color)>,
        ) -> Vec<Span<'s>> {
            let (line_start, line_end) = line_range.into_inner();
            let mut colorized: VecDeque<_> = colorized.into();
            let mut highlighted = Vec::with_capacity(colorized.len());
            let mut offset = line_start;
            while parens
                .pop_front_if(|(position, _)| *position < offset)
                .is_some()
            {
                // drain unwanted preceding elements
            }
            while let Some((position, color)) =
                parens.pop_front_if(|(position, _)| *position >= offset && *position <= line_end)
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

        fn render_find_prompt<'t, 's, S: Highlighter>(
            syntax: FindSyntax<'t, S>,
            text_area: Rect,
            buf: &mut ratatui::buffer::Buffer,
            prompt: &TextField,
            highlight: impl FnOnce(Vec<Span<'s>>) -> Vec<Span<'s>>,
            block: impl FnOnce(Block) -> Block,
        ) {
            let [_, dialog_area, _] =
                Layout::vertical([Min(0), Length(3), Min(0)]).areas(text_area);

            Clear.render(dialog_area, buf);
            Paragraph::new(Line::from(match syntax {
                FindSyntax::Plain(syntax) => highlight(colorize(
                    syntax,
                    &mut HighlightState::default(),
                    prompt.chars().collect::<String>().into(),
                    true,
                )),
                FindSyntax::Regex => highlight(colorize(
                    &crate::syntax::Regex,
                    &mut HighlightState::default(),
                    prompt.chars().collect::<String>().into(),
                    true,
                )),
            }))
            .scroll((
                0,
                (prompt.cursor_column() as u16).saturating_sub(dialog_area.width.saturating_sub(2)),
            ))
            .block(block(Block::bordered().border_type(BorderType::Rounded)))
            .render(dialog_area, buf);
        }

        fn find_mode_help(prompt: &TextField, type_: SearchType) -> Vec<crate::help::Keybinding> {
            use crate::help::{ctrl, keybind, none};

            let mut help = if prompt.is_empty() {
                // Tab when buffer is empty switches mode
                vec![none(
                    &["Tab"],
                    match type_ {
                        SearchType::Plain => "Regex Find",
                        SearchType::Regex => "Plain Text Find",
                    },
                )]
            } else if prompt.can_autocomplete() {
                vec![none(&["Tab"], "Autocomplete Word")]
            } else {
                vec![]
            };

            help.extend([
                ctrl(&["V"], "Paste From Cut Buffer"),
                keybind::<crate::key::GotoLine>("Goto Line"),
                keybind::<crate::key::Find>(match prompt.is_empty() {
                    true => "Redo Last Find",
                    false => "Begin New Find",
                }),
                none(&["Enter"], "Browse All Matches"),
                none(&["Esc"], "Cancel"),
            ]);

            help
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
        let focused = self.focused && self.mode.is_some();
        let show_sub_help: fn(
            ratatui::layout::Rect,
            &mut ratatui::buffer::Buffer,
            &[crate::help::Keybinding],
        ) = if self.show_sub_help {
            |text_area, buf, keybindings| {
                render_help(text_area, buf, keybindings, |b| {
                    b.title_bottom(
                        Line::from(vec![
                            Span::styled("F1", Style::default().add_modifier(Modifier::REVERSED)),
                            Span::raw(" to toggle"),
                        ])
                        .centered(),
                    )
                })
            }
        } else {
            |_, _, _| { /* do nothing */ }
        };

        let block = Block::bordered()
            .border_type(if focused {
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
                focused,
            ));

        let block = if focused {
            block
        } else {
            block.style(Style::default().dim())
        };

        let block = match buffer.endings.name() {
            Some(name) => block
                .title_bottom(border_title(syntax.to_string(), focused).right_aligned())
                .title_bottom(border_title(name.to_string(), focused)),
            None => block.title_bottom(border_title(syntax.to_string(), focused).right_aligned()),
        };

        let block = match buffer.bookmarks.len() {
            0 => block,
            bookmarks => block.title_top(if focused {
                Line::from(vec![
                    Span::raw("\u{252b}"),
                    Span::styled(
                        bookmarks.to_string(),
                        Style::default().bold().bg(Color::Cyan),
                    ),
                    Span::raw("\u{2523}"),
                ])
                .right_aligned()
            } else {
                Line::from(vec![
                    Span::raw("\u{2524}"),
                    Span::styled(bookmarks.to_string(), Style::default().bg(Color::Cyan)),
                    Span::raw("\u{251c}"),
                ])
                .right_aligned()
            }),
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
                focused,
            )
            .right_aligned(),
        );

        let block = match self.mode {
            Some(
                EditorMode::MultiCursor {
                    match_idx, matches, ..
                }
                | EditorMode::AutocompleteMulti {
                    match_idx, matches, ..
                },
            ) => block.title_bottom(
                border_title(
                    format!("Match {} / {}", *match_idx + 1, matches.len()),
                    focused,
                )
                .centered(),
            ),
            Some(EditorMode::SelectLine { .. }) => {
                let mut bookmarks = buffer.bookmarks.range(..=state.cursor).rev().peekable();
                if let Some(b) = bookmarks.peek()
                    && *b == state.cursor
                {
                    block.title_bottom(
                        border_title(
                            format!(
                                "Bookmark {} / {}",
                                bookmarks.count(),
                                buffer.bookmarks.len()
                            ),
                            focused,
                        )
                        .centered(),
                    )
                } else {
                    block
                }
            }
            Some(EditorMode::SplitPane) => block.border_style(Style::default().blue()),
            Some(EditorMode::Open { .. }) => block,
            _ => match state.selection {
                Some(selection) => {
                    let (start, end) = reorder(state.cursor, selection);
                    if let Ok(start_line) = rope.try_char_to_line(start)
                        && let Ok(end_line) = rope.try_char_to_line(end)
                        && let Some(lines) = end_line.checked_sub(start_line)
                    {
                        block.title_bottom(
                            border_title(
                                match lines {
                                    0 => "1 Line".to_string(),
                                    n => format!("{} Lines", n + 1),
                                },
                                focused,
                            )
                            .centered(),
                        )
                    } else {
                        block
                    }
                }
                None => block,
            },
        };

        let [text_area, scrollbar_area] =
            Layout::horizontal([Min(0), Length(1)]).areas(block.inner(area));

        block.render(area, buf);

        let current_line = rope.try_char_to_line(state.cursor).ok();
        let viewport_height: usize = text_area.height.into();

        let viewport_line: usize = current_line
            .map(|line| line.saturating_sub(viewport_height / 2))
            .unwrap_or(0);

        let viewport_start = rope.try_line_to_char(viewport_line).unwrap_or(0);

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
            .try_line_to_char(current_line.unwrap_or(0) + viewport_height)
            .unwrap_or(rope.len_chars())
            .saturating_sub(viewport_start);

        let mut marks: BTreeMap<usize, Color> =
            match prev_opening_char(rope, state.cursor, viewport_size) {
                Some((opener, start)) => match next_closing_char(rope, state.cursor, viewport_size)
                {
                    Some((closer, end)) => {
                        if opener == closer {
                            [(start.saturating_sub(1), MATCHING), (end, MATCHING)].into()
                        } else {
                            [(start.saturating_sub(1), MISMATCH), (end, MISMATCH)].into()
                        }
                    }
                    None => [(start.saturating_sub(1), MATCHING)].into(),
                },
                None => BTreeMap::default(),
            };

        for bookmark in buffer.bookmarks.iter().filter(|p| *p >= viewport_start) {
            use std::collections::btree_map::Entry;

            match marks.entry(bookmark) {
                Entry::Occupied(o) => {
                    *o.into_mut() = BOOKMARK;
                }
                Entry::Vacant(v) => {
                    v.insert(BOOKMARK);
                }
            }
        }

        match self.mode {
            Some(EditorMode::SelectLine { .. }) => {
                if let Some(mark) = marks.get_mut(&state.cursor) {
                    *mark = Color::Yellow;
                }
            }
            _ => {
                marks.remove(&state.cursor);
            }
        }

        let mut marks = marks.into_iter().collect();

        Clear.render(text_area, buf);
        Paragraph::new(match self.mode {
            Some(
                EditorMode::MultiCursor {
                    matches,
                    match_idx,
                    highlight: true,
                    ..
                }
                | EditorMode::PasteGroup {
                    matches,
                    match_idx,
                    highlight: true,
                    ..
                },
            ) => {
                const HIGHLIGHTED: Style = Style::new()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::REVERSED);

                fn sub_match_ranges(matches: &[MultiCursor]) -> VecDeque<Range<usize>> {
                    matches.iter().map(|m| m.range.start..m.range.end).collect()
                }

                let selection_start = matches[*match_idx].range.start;
                let selection_end = matches[*match_idx].range.end;
                let mut matches = sub_match_ranges(matches);

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
                                    |span| span.style(HIGHLIGHTED),
                                ),
                                range.clone(),
                                (selection_start, selection_end),
                                |span| {
                                    span.style(
                                        Style::new()
                                            .fg(Color::Red)
                                            .add_modifier(Modifier::REVERSED),
                                    )
                                },
                            )
                            .into()
                        },
                    )
                    .map(|line| widen_tabs(line))
                    .take(area.height.into())
                    .collect::<Vec<_>>()
            }
            Some(
                EditorMode::MultiCursor {
                    matches,
                    highlight: false,
                    ..
                }
                | EditorMode::PasteGroup {
                    matches,
                    highlight: false,
                    ..
                },
            ) => {
                let (mut cursors, (mut ranges, selections)): (VecDeque<_>, (_, VecFiltered<_>)) =
                    matches
                        .iter()
                        .map(|m| {
                            (
                                m.cursor..m.cursor + 1,
                                (m.range.clone(), m.selection_range()),
                            )
                        })
                        .unzip();

                cursors.retain(|r| r.start != state.cursor);
                let mut selections = selections.into();

                EditorLine::iter(rope, viewport_line)
                    .map(
                        |EditorLine {
                             line,
                             range,
                             number,
                         }| {
                            let whole_range = widen_range(range);

                            highlight_matches(
                                highlight_matches(
                                    highlight_matches(
                                        widen(colorize(
                                            syntax,
                                            &mut hlstate,
                                            line,
                                            current_line == Some(number),
                                        )),
                                        whole_range.clone(),
                                        &mut ranges,
                                        |span| {
                                            span.patch_style(
                                                Style::new()
                                                    .underlined()
                                                    .underline_color(Color::Blue),
                                            )
                                        },
                                    ),
                                    whole_range.clone(),
                                    &mut selections,
                                    |span| span.style(EDITING),
                                ),
                                whole_range,
                                &mut cursors,
                                |span| {
                                    span.style(
                                        Style::new()
                                            .fg(Color::Blue)
                                            .add_modifier(Modifier::REVERSED),
                                    )
                                },
                            )
                            .into()
                        },
                    )
                    .map(|line| widen_tabs(line))
                    .take(area.height.into())
                    .collect::<Vec<_>>()
            }
            Some(EditorMode::AutocompleteMulti {
                matches,
                offsets,
                completions,
                index,
                ..
            }) => {
                // We're underlining the multicursors' effective range (in blue),
                // the autocompletion replacements (in red)
                // *and* the cursors themselves (as a blue block).
                // Yes, I know it's a lot.

                let (mut cursors, mut ranges): (VecDeque<_>, _) = matches
                    .iter()
                    .map(|m| (m.cursor..m.cursor + 1, m.range.clone()))
                    .unzip();

                let completion_chars = completions[*index].chars().count();
                let mut replacements = matches
                    .iter()
                    .zip(offsets)
                    .map(|(m, o)| m.range.start + *o..m.range.start + *o + completion_chars)
                    .collect();

                cursors.retain(|r| r.start != state.cursor);

                EditorLine::iter(rope, viewport_line)
                    .map(
                        |EditorLine {
                             line,
                             range,
                             number,
                         }| {
                            let whole_range = widen_range(range);

                            highlight_matches(
                                highlight_matches(
                                    highlight_matches(
                                        widen(colorize(
                                            syntax,
                                            &mut hlstate,
                                            line,
                                            current_line == Some(number),
                                        )),
                                        whole_range.clone(),
                                        &mut ranges,
                                        |span| {
                                            span.patch_style(
                                                Style::new()
                                                    .underlined()
                                                    .underline_color(Color::Blue),
                                            )
                                        },
                                    ),
                                    whole_range.clone(),
                                    &mut replacements,
                                    |span| {
                                        span.patch_style(
                                            Style::new().underlined().underline_color(Color::Red),
                                        )
                                    },
                                ),
                                whole_range,
                                &mut cursors,
                                |span| {
                                    span.style(
                                        Style::new()
                                            .fg(Color::Blue)
                                            .add_modifier(Modifier::REVERSED),
                                    )
                                },
                            )
                            .into()
                        },
                    )
                    .map(|line| widen_tabs(line))
                    .take(area.height.into())
                    .collect::<Vec<_>>()
            }
            Some(EditorMode::Autocomplete {
                offset,
                completions,
                index,
            }) => {
                let completion_start = *offset;
                let completion_end = *offset + completions[*index].chars().count();

                EditorLine::iter(rope, viewport_line)
                    .map(
                        |EditorLine {
                             line,
                             range,
                             number,
                         }| {
                            highlight_parens(
                                widen(highlight_selection(
                                    colorize(
                                        syntax,
                                        &mut hlstate,
                                        line,
                                        current_line == Some(number),
                                    ),
                                    range.clone(),
                                    (completion_start, completion_end),
                                    |span| {
                                        span.patch_style(
                                            Style::new().underlined().underline_color(Color::Red),
                                        )
                                    },
                                )),
                                range,
                                &mut marks,
                            )
                            .into()
                        },
                    )
                    .map(|line| widen_tabs(line))
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
                                    widen(colorize(
                                        syntax,
                                        &mut hlstate,
                                        line,
                                        current_line == Some(number),
                                    )),
                                    range,
                                    &mut marks,
                                )
                                .into()
                            },
                        )
                        .map(|line| widen_tabs(line))
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
                                        widen(highlight_selection(
                                            colorize(
                                                syntax,
                                                &mut hlstate,
                                                line,
                                                current_line == Some(number),
                                            ),
                                            range.clone(),
                                            (selection_start, selection_end),
                                            |span| span.style(EDITING),
                                        )),
                                        range,
                                        &mut marks,
                                    )
                                    .into()
                                },
                            )
                            .map(|line| widen_tabs(line))
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
            None | Some(EditorMode::Editing) | Some(EditorMode::Autocomplete { .. }) => {
                if let Some(Help {
                    find,
                    has_bookmarks,
                    cursor_pos,
                    has_selection,
                    multiple_buffers,
                    multiple_panes,
                }) = self.show_help
                {
                    use crate::help::{
                        EDITING_0, EDITING_2, EDITING_3, F10, SWITCH_PANE, ctrl, keybind, none,
                    };
                    use crate::key::{
                        GotoLine, GotoPair, SelectInside, UpdateLines, WidenSelection,
                    };

                    let mut help = Vec::with_capacity(16);
                    help.extend(EDITING_0);
                    help.push(keybind::<GotoLine>(if has_bookmarks {
                        "Goto Line / Bookmark"
                    } else {
                        "Goto Line"
                    }));
                    help.push(find.into());
                    help.extend(
                        has_selection.then_some(keybind::<UpdateLines>("Update Selected Lines")),
                    );
                    help.extend(
                        matches!(cursor_pos, CursorPos::AtParen)
                            .then_some(keybind::<GotoPair>("Goto Matching Pair")),
                    );
                    help.push(keybind::<SelectInside>("Select Inside Pair"));
                    help.push(keybind::<WidenSelection>(
                        if matches!(cursor_pos, CursorPos::InWord) {
                            "Select Word"
                        } else {
                            "Widen Selection"
                        },
                    ));
                    help.push(F10);
                    help.extend(EDITING_2);
                    help.extend(
                        has_selection.then_some(ctrl(&["Home", "End"], "Start / End of Selection")),
                    );
                    help.push(none(
                        &["Tab"],
                        if matches!(cursor_pos, CursorPos::AfterWord) {
                            "Autocomplete Word"
                        } else {
                            "Indent Text"
                        },
                    ));
                    help.extend(EDITING_3);
                    help.extend(multiple_panes.then_some(SWITCH_PANE));
                    help.extend(multiple_buffers.then_some(ctrl(&["PgUp", "PgDn"], "Switch File")));

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
                show_sub_help(text_area, buf, CONFIRM_CLOSE);
                render_message(
                    text_area,
                    buf,
                    BufferMessage::Error("Unsaved changes. Really quit?".into()),
                );
            }
            Some(EditorMode::VerifySave) => {
                show_sub_help(text_area, buf, VERIFY_SAVE);
                render_message(
                    text_area,
                    buf,
                    BufferMessage::Error("Buffer changed on disk. Really save?".into()),
                );
            }
            Some(EditorMode::SplitPane) => {
                show_sub_help(text_area, buf, SPLIT_PANE);
            }
            Some(EditorMode::VerifyReload) => {
                show_sub_help(text_area, buf, VERIFY_RELOAD);
                render_message(
                    text_area,
                    buf,
                    BufferMessage::Error("Buffer not yet saved. Really reload?".into()),
                );
            }
            Some(EditorMode::SelectInside) => {
                show_sub_help(text_area, buf, SELECT_INSIDE);
            }
            Some(EditorMode::SelectLine { .. }) => {
                show_sub_help(
                    text_area,
                    buf,
                    if buffer.has_bookmarks() {
                        SELECT_LINE_BOOKMARKED
                    } else {
                        SELECT_LINE
                    },
                );
            }
            Some(EditorMode::Search { prompt, type_, .. }) => {
                show_sub_help(text_area, buf, &find_mode_help(prompt, *type_));
                render_find_prompt(
                    match type_ {
                        SearchType::Plain => FindSyntax::Plain(syntax),
                        SearchType::Regex => FindSyntax::Regex,
                    },
                    text_area,
                    buf,
                    prompt,
                    |s| s,
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
            Some(EditorMode::AutocompleteSearch {
                prompt,
                type_,
                offset,
                completions,
                index,
                ..
            }) => {
                show_sub_help(text_area, buf, &find_mode_help(prompt, *type_));
                render_find_prompt(
                    match type_ {
                        SearchType::Plain => FindSyntax::Plain(syntax),
                        SearchType::Regex => FindSyntax::Regex,
                    },
                    text_area,
                    buf,
                    prompt,
                    |s| {
                        let mut colorized = VecDeque::from(s);
                        let mut highlighted = Vec::with_capacity(colorized.len());

                        // output everything to offset verbatim
                        extract(&mut colorized, *offset, &mut highlighted, |span| span);

                        // output autcompleted text underlined
                        extract(
                            &mut colorized,
                            completions[*index].chars().count(),
                            &mut highlighted,
                            |span| {
                                span.patch_style(
                                    Style::new().underlined().underline_color(Color::Red),
                                )
                            },
                        );

                        // output the rest verbatim
                        highlighted.extend(colorized);

                        highlighted
                    },
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
            Some(EditorMode::MultiCursor { .. } | EditorMode::AutocompleteMulti { .. }) => {
                show_sub_help(text_area, buf, REPLACE_MATCHES);
            }
            Some(EditorMode::PasteGroup { total, .. }) => {
                show_sub_help(
                    text_area,
                    buf,
                    PASTE_GROUP
                        .iter()
                        .copied()
                        .take(*total + 1)
                        .collect::<Vec<_>>()
                        .as_slice(),
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

pub enum EditorCutBuffer {
    Single(CutBuffer),
    Multiple(Vec<CutBuffer>), // should contain at least 2 cut buffers
}

impl EditorCutBuffer {
    pub fn primary(&self) -> Option<&CutBuffer> {
        match self {
            Self::Single(b) => Some(b),
            Self::Multiple(v) => v.first(),
        }
    }

    pub fn primary_mut(&mut self) -> Option<&mut CutBuffer> {
        match self {
            Self::Single(b) => Some(b),
            Self::Multiple(v) => v.first_mut(),
        }
    }

    pub fn cut_str(&self) -> Option<&str> {
        self.primary().map(|c| c.as_str())
    }
}

#[derive(Default)]
pub struct CutBuffer {
    data: String,
    chars_len: usize,
    bookmarks: Vec<usize>, // positions relative to start of cut buffer
}

impl CutBuffer {
    pub fn new<B>(rope: ropey::RopeSlice<'_>, bookmarks: B) -> Self
    where
        B: IntoIterator<Item = usize>,
    {
        Self {
            chars_len: rope.len_chars(),
            data: rope.chunks().collect(),
            bookmarks: bookmarks.into_iter().collect(),
        }
    }

    pub fn as_str(&self) -> &str {
        self.data.as_str()
    }
}

impl From<String> for CutBuffer {
    fn from(data: String) -> Self {
        Self {
            chars_len: data.chars().count(),
            data,
            bookmarks: vec![],
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
    bookmarks: private::Bookmarks,
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
    mut alt: Secondary<'_, '_>,
) {
    use imara_diff::{Algorithm::Histogram, Diff, Hunk, InternedInput};
    use ropey::Rope;
    use std::ops::Range;

    #[must_use]
    fn remove_lines(
        rope: &mut Rope,
        alt: &mut Secondary<'_, '_>,
        lines: Range<u32>,
    ) -> Range<usize> {
        let removed =
            rope.line_to_char(lines.start as usize)..rope.line_to_char(lines.end as usize);
        rope.remove(alt.remove(removed.clone()));
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

    for Hunk { before, after } in hunks.into_iter().rev() {
        let removed = remove_lines(source, &mut alt, before.clone());

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

fn autocomplete_matches(rope: &ropey::Rope, prefix: String) -> Vec<String> {
    use radix_trie::{Trie, TrieCommon};
    use unicode_width::UnicodeWidthChar;

    let mut counts: Trie<String, u64> = Trie::default();

    for line in rope.lines() {
        let line = Cow::from(line);
        for word in line
            .split(|c| !is_word(c) && c.width() != Some(0))
            .filter(|s| !s.is_empty())
        {
            if word.starts_with(&prefix) && word != prefix {
                counts.map_with_default(word.to_string(), |c| *c += 1, 1);
            }
        }
    }

    let mut counts = counts
        .iter()
        .map(|(s, c)| (s.to_string(), c))
        .collect::<Vec<_>>();

    counts.sort_unstable_by(|(s1, c1), (s2, c2)| c1.cmp(c2).then(s1.cmp(s2).reverse()));

    std::iter::once(prefix)
        .chain(counts.into_iter().map(|(s, _)| s).rev())
        .collect()
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

enum Toggle {
    Inserted, // new bookmark added
    Removed,  // existing bookmark removed
}

#[derive(Default)]
struct VecFiltered<T>(Vec<T>);

impl<T> Extend<Option<T>> for VecFiltered<T> {
    fn extend<I: IntoIterator<Item = Option<T>>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().flatten());
    }
}

impl<T> From<VecFiltered<T>> for std::collections::VecDeque<T> {
    fn from(v: VecFiltered<T>) -> Self {
        v.0.into()
    }
}

fn probe_indent(rope: &ropey::Rope, indent_char: char) -> Option<usize> {
    rope.lines()
        .map(|l| l.chars().take_while(|c| *c == indent_char).count())
        .filter(|c| *c != 0)
        .reduce(std::cmp::min)
}

#[inline]
pub fn is_word(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

fn reorder<T: Ord>(x: T, y: T) -> (T, T) {
    if x <= y { (x, y) } else { (y, x) }
}
