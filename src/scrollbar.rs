use ratatui::{
    layout::{Position, Rect},
    widgets::StatefulWidget,
};

pub struct Scrollbar;

impl StatefulWidget for Scrollbar {
    type State = ratatui::widgets::ScrollbarState;

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
