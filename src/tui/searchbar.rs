// SPDX-License-Identifier: MIT
use super::symbols::block;
use crate::search::search::FindingsSummary;
use tuirs::buffer::Buffer;
use tuirs::layout::Rect;
use tuirs::style::{Color, Style};
use tuirs::symbols::line;
use tuirs::widgets::Widget;

pub struct SearchBar<'a> {
    data: &'a [FindingsSummary],
    name: String,
    selected: bool,
    cursor: usize,
    visual_cursor: Option<usize>,
}

impl<'a> SearchBar<'a> {
    pub fn new(
        name: String,
        data: &'a [FindingsSummary],
        selected: bool,
        cursor: usize,
        visual_cursor: Option<usize>,
    ) -> SearchBar<'a> {
        SearchBar {
            data,
            name,
            selected,
            cursor,
            visual_cursor,
        }
    }

    fn findings_to_symbol(findings: &FindingsSummary) -> &'static str {
        match findings {
            FindingsSummary::Nothing => "·",
            FindingsSummary::Timestamp => line::VERTICAL,
            FindingsSummary::RangeBegin => line::VERTICAL_RIGHT,
            FindingsSummary::Range => line::HORIZONTAL,
            FindingsSummary::RangeEnd => line::VERTICAL_LEFT,
            FindingsSummary::Complex(i) => {
                if *i < 3 {
                    block::LIGHT
                } else if *i < 10 {
                    block::MEDIUM
                } else {
                    block::FULL
                }
            }
        }
    }
}

impl<'a> Widget for SearchBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (i, elmt) in self.data.iter().enumerate() {
            let fg = if i == self.cursor {
                if self.selected {
                    Color::White
                } else {
                    Color::Black
                }
            } else if self.selected {
                Color::LightYellow
            } else {
                Color::Yellow
            };
            let bg = if i == self.cursor {
                Color::Gray
            } else if let Some(visual_cursor) = self.visual_cursor {
                if (visual_cursor <= i && i <= self.cursor)
                    || (self.cursor <= i && i <= visual_cursor)
                {
                    Color::Blue
                } else {
                    Color::Black
                }
            } else {
                Color::Black
            };
            let style = Style::default().fg(fg).bg(bg);

            buf.get_mut(area.left() + i as u16, area.top())
                .set_symbol(SearchBar::findings_to_symbol(elmt))
                .set_style(style);
        }
        buf.set_stringn(
            area.left(),
            area.top(),
            &self.name,
            area.width as usize,
            Style::default()
                .bg(if self.selected {
                    Color::White
                } else {
                    Color::Gray
                })
                .fg(Color::Black),
        );
    }
}
