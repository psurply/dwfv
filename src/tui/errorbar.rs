// SPDX-License-Identifier: MIT
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::widgets::Widget;

pub struct ErrorBar {
    message: String,
    selected: bool,
}

impl ErrorBar {
    pub fn new(message: String, selected: bool) -> ErrorBar {
        ErrorBar { message, selected }
    }
}

impl Widget for ErrorBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = Style::default().fg(Color::White).bg(if self.selected {
            Color::LightRed
        } else {
            Color::Red
        });

        for x in 0..area.width {
            for y in 0..area.height {
                buf.get_mut(area.left() + x, area.top() + y)
                    .set_style(style);
            }
        }

        buf.set_stringn(
            area.left(),
            area.top(),
            &self.message,
            area.width as usize,
            style,
        );
    }
}
