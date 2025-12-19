use ratatui::widgets::StatefulWidget;
use std::path::Path;

pub struct Buffer {
    // TODO - support buffer's source as Source enum (file on disk, ssh target, etc.)
    rope: ropey::Rope,
    // TODO - support undo stack
    // TODO - support redo stack
}

impl Buffer {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        use std::fs::File;
        use std::io::BufReader;

        Ok(Self {
            rope: ropey::Rope::from_reader(BufReader::new(File::open(path)?))?,
        })
    }

    pub fn total_lines(&self) -> usize {
        self.rope.len_lines()
    }
}

pub struct BufferWidget {
    pub line: usize,
}

impl StatefulWidget for BufferWidget {
    type State = Buffer;

    fn render(
        self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        state: &mut Buffer,
    ) {
        use ratatui::{
            text::Line,
            widgets::{Paragraph, Widget},
        };
        use std::borrow::Cow;

        fn tabs_to_spaces<'s, S: Into<Cow<'s, str>> + AsRef<str>>(s: S) -> Cow<'s, str> {
            if s.as_ref().contains('\t') {
                s.as_ref().replace('\t', "    ").into()
            } else {
                s.into()
            }
        }

        Paragraph::new(
            state
                .rope
                .lines_at(self.line)
                .map(|line| Line::from(tabs_to_spaces(Cow::from(line)).into_owned()))
                .take(area.height.into())
                .collect::<Vec<_>>(),
        )
        .render(area, buf)

        // TODO - support horizontal scrolling
        // TODO - draw vertical scrollbar at right
        // TODO - draw status bar at bottom
    }
}

type BufIndex = usize;
type LineNum = usize;

#[derive(Copy, Clone, Default)]
pub struct BufferPosition {
    pub index: BufIndex,
    pub line: LineNum,
    // TODO - support cursor's column
    // TODO - support optional text selection
}

impl BufferPosition {
    pub fn get_buffer<'b>(&self, buffers: &'b [Buffer]) -> Option<&'b Buffer> {
        buffers.get(self.index)
    }

    pub fn viewport_up(&mut self, lines: usize) {
        self.line = self.line.saturating_sub(lines)
    }

    pub fn viewport_down(&mut self, lines: usize, max_lines: usize) {
        self.line = (self.line + lines).min(max_lines)
    }
}
