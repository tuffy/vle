use ratatui::widgets::Widget;

#[derive(Default)]
pub struct Prompt {
    value: Vec<char>,
}

impl Prompt {
    pub fn push(&mut self, c: char) {
        self.value.push(c)
    }

    pub fn pop(&mut self) -> Option<char> {
        self.value.pop()
    }
}

impl std::fmt::Display for Prompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.value.iter().try_for_each(|c| c.fmt(f))
    }
}

#[derive(Copy, Clone)]
pub struct PromptWidget<'p> {
    pub prompt: &'p Prompt,
}

impl Widget for PromptWidget<'_> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::style::{Modifier, Style};

        ratatui::widgets::Paragraph::new(self.prompt.to_string())
            .style(Style::new().add_modifier(Modifier::REVERSED))
            .render(area, buf)
    }
}
