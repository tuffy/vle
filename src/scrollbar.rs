use ratatui::{
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
use std::ops::Range;

pub struct ScrollbarState {
    content_length: usize,
    position: usize,
    viewport_content_length: usize,
}

impl ScrollbarState {
    pub fn new(content_length: usize) -> Self {
        Self {
            content_length,
            position: 0,
            viewport_content_length: 0,
        }
    }

    pub fn viewport_content_length(self, viewport_content_length: usize) -> Self {
        Self {
            viewport_content_length,
            ..self
        }
    }

    pub fn position(self, position: usize) -> Self {
        Self { position, ..self }
    }

    /// Returns range of scrollbar's thumb area
    fn thumb(&self) -> Range<usize> {
        // ensure start point of thumb doesn't push the thumb itself
        // outside of the scrollbar area
        let start = self.position.min(
            self.content_length
                .saturating_sub(self.viewport_content_length),
        );
        let end = (start + self.viewport_content_length).min(self.content_length);
        Range { start, end }
    }
}

pub struct Scrollbar;

impl StatefulWidget for Scrollbar {
    type State = ScrollbarState;

    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer, state: &mut Self::State) {
        use ratatui::layout::{
            Constraint::{Length, Min},
            Layout,
        };

        let [top_arrow, track, bottom_arrow] =
            Layout::vertical([Length(1), Min(0), Length(1)]).areas(area);

        buf[(top_arrow.x, top_arrow.y)].set_char('\u{25B2}');
        buf[(bottom_arrow.x, bottom_arrow.y)].set_char('\u{25BC}');
        for i in range_end_map(state.thumb(), |u| {
            (((u as f64) / (state.content_length as f64)) * (track.height as f64)) as u16
        }) {
            // TODO - update this
            buf[(track.x, track.y + i)].set_char('#');
        }
        // TODO - render top in block characters
        // TODO - render bottom in inverted block characters
    }
}

fn range_end_map<T, U>(r: Range<T>, mut f: impl FnMut(T) -> U) -> Range<U> {
    Range {
        start: f(r.start),
        end: f(r.end),
    }
}
