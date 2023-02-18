// SPDX-License-Identifier: MIT
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Modifier, Style};
use tui::widgets::Widget;

pub struct StatusBar {
    message: String,
    input_buffer: String,
}

impl StatusBar {
    pub fn new(message: String, input_buffer: String) -> StatusBar {
        StatusBar {
            message,
            input_buffer,
        }
    }
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD);

        for i in 0..area.width {
            buf.get_mut(area.left() + i, area.top()).set_style(style);
        }

        buf.set_stringn(
            area.left(),
            area.top(),
            &self.message,
            area.width as usize,
            style,
        );

        buf.set_stringn(
            (area.right() as usize - self.input_buffer.len() - 1) as u16,
            area.top(),
            &self.input_buffer,
            self.input_buffer.len(),
            style,
        );
    }
}
