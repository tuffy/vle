use ratatui::{
    layout::{Position, Rect},
    widgets::StatefulWidget,
};

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
        // TODO - determine start of thumb in track
        // TODO - determine middle of thumb in track
        // TODO - determine end of thumb in track
        // TODO - render start and middle of thumb as block characters
        // TODO - render end of thumb as inverted block characters
    }
}
