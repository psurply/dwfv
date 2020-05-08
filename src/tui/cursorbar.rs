// SPDX-License-Identifier: MIT
use super::symbols::arrow;
use crate::signaldb::Timestamp;
use tuirs::buffer::Buffer;
use tuirs::layout::Rect;
use tuirs::style::{Color, Style};
use tuirs::widgets::Widget;

pub enum CursorType {
    Top,
    Bottom,
}

pub struct CursorBar {
    cursor_type: CursorType,
    timestamp: Timestamp,
    scale: Timestamp,
    scrollable: bool,
    cursor: usize,
}

impl CursorBar {
    pub fn new(
        cursor_type: CursorType,
        timestamp: Timestamp,
        scale: Timestamp,
        cursor: usize,
        scrollable: bool,
    ) -> CursorBar {
        CursorBar {
            cursor_type,
            timestamp,
            cursor,
            scale,
            scrollable,
        }
    }
}

impl Widget for CursorBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = Style::default().fg(Color::Gray).bg(Color::Black);

        for i in 0..area.width {
            buf.get_mut(area.left() + i, area.top()).set_style(style);
        }

        let symbol = match self.cursor_type {
            CursorType::Top => arrow::DOWN,
            CursorType::Bottom => arrow::UP,
        };

        let symbol_scroll = match self.cursor_type {
            CursorType::Top => arrow::DOUBLE_UP,
            CursorType::Bottom => arrow::DOUBLE_DOWN,
        };

        buf.get_mut(area.left() + self.cursor as u16, area.top())
            .set_symbol(symbol)
            .set_style(style);

        if self.scrollable {
            buf.get_mut(area.left(), area.top())
                .set_symbol(symbol_scroll)
                .set_style(style);
        }

        let status = match self.cursor_type {
            CursorType::Top => format!("↔ {}", self.scale.get_value()),
            CursorType::Bottom => format!("I {}", self.timestamp.get_value()),
        };

        buf.set_stringn(
            (area.right() as usize - status.len() - 1) as u16,
            area.top(),
            &status,
            status.len(),
            style,
        );
    }
}
