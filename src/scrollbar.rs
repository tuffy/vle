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
        use ratatui::style::Style;

        let [top_arrow, track, bottom_arrow] =
            Layout::vertical([Length(1), Min(0), Length(1)]).areas(area);

        buf[(top_arrow.x, top_arrow.y)].set_char('\u{25B2}');
        buf[(bottom_arrow.x, bottom_arrow.y)].set_char('\u{25BC}');

        // convert thumb from ScrollBar units to a proportion of the track's height
        // but in multiples of 8 subpixels
        let mut thumb = range_end_map(state.thumb(), |u| {
            (((u as f64) / (state.content_length as f64)) * ((track.height * 8) as f64)).round() as u32
        });

        // ensure the thumb is at least 8 subpixels high and doesn't slide out of the track
        thumb.start = thumb.start.min((track.height.saturating_sub(1) * 8).into());
        thumb.end = thumb.end.max(thumb.start + 8);

        // convert the thumb back to rows/subpixels
        let (start, start_subpixels) = ((thumb.start / 8) as u16, thumb.start % 8);
        let (end, end_subpixels) = ((thumb.end / 8) as u16, thumb.end % 8);

        // paint start of the thumb in subpixels
        buf[(track.x, track.y + start)].set_char(subpixels_char_top(start_subpixels));

        // paint whole blocks between start and end of thumb
        for i in (start + 1)..end {
            buf[(track.x, track.y + i)].set_char('\u{2588}');
        }

        // paint end of thumb in inverted subpixels
        if end_subpixels > 0 {
            buf[(track.x, track.y + end)].set_char(subpixels_char(end_subpixels));
            buf[(track.x, track.y + end)].set_style(Style::default().reversed());
        }
    }
}

fn range_end_map<T, U>(r: Range<T>, mut f: impl FnMut(T) -> U) -> Range<U> {
    Range {
        start: f(r.start),
        end: f(r.end),
    }
}

fn subpixels_char_top(subpixels: u32) -> char {
    match subpixels {
        0 => '\u{2588}',
        1 => '\u{2587}',
        2 => '\u{2586}',
        3 => '\u{2585}',
        4 => '\u{2584}',
        5 => '\u{2583}',
        6 => '\u{2582}',
        7 => '\u{2581}',
        _ => '?',
    }
}

fn subpixels_char(subpixels: u32) -> char {
    match subpixels {
        0 => '\u{2588}',
        1 => '\u{2587}',
        2 => '\u{2586}',
        3 => '\u{2585}',
        4 => '\u{2584}',
        5 => '\u{2583}',
        6 => '\u{2582}',
        7 => '\u{2581}',
        _ => '?',
    }
}
