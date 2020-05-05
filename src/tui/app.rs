// SPDX-License-Identifier: MIT
use super::cursorbar::{CursorBar, CursorType};
use super::errorbar::ErrorBar;
use super::event::{Event, Events, SearchTarget};
use super::instr::TuiInstr;
use super::searchbar::SearchBar;
use super::statusbar::StatusBar;
use super::waveform::{Waveform, WaveformElement};
use crate::signaldb::{AsyncSignalDB, SignalValue, Timestamp};
use std::cmp;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;
use tuirs::backend::Backend;
use tuirs::layout::Rect;
use tuirs::terminal::Frame;
use tuirs::widgets::Widget;

const MAX_ZOOM:i64 = 1 << 48;
const HELP_MSG:&str = "q:Quit  h,j,k,l:Move  +,-,=:Zoom  v:Select  /,f:Search  o:Edit  \
    yy:Peek  p,P:Pop  dd:Stash  u,r:Undo/Redo";

#[derive(Clone)]
struct Position {
    x: Timestamp,
    y: usize,
}

struct Memento {
    past: Vec<Vec<TuiInstr>>,
    future: Vec<Vec<TuiInstr>>
}

pub struct App {
    signaldb: AsyncSignalDB,
    scale: Timestamp,
    height: u16,
    visual_cursor: Position,
    cursor: Position,
    window: Position,
    events: Events,
    area: Rect,
    layout: Vec<TuiInstr>,
    memento: Memento,
    clipboard: Vec<TuiInstr>,
    search_pattern: String
}

impl App {
    pub fn new(signaldb: AsyncSignalDB) -> App {
        let layout = signaldb
            .sync_db
            .get_signal_ids()
            .iter()
            .map(|i| TuiInstr::Signal(i.to_string()))
            .collect();
        let mut app = App {
            signaldb,
            scale: Timestamp::new(1),
            height: 0,
            visual_cursor: Position {
                x: Timestamp::new(0),
                y: 0,
            },
            cursor: Position {
                x: Timestamp::new(0),
                y: 0,
            },
            window: Position {
                x: Timestamp::new(0),
                y: 0,
            },
            events: Events::new(),
            area: Rect::new(0, 0, 0, 0),
            layout,
            memento: Memento {
                past: Vec::new(),
                future: Vec::new()
            },
            clipboard: Vec::new(),
            search_pattern: String::new()
        };

        app.goto_first_event();
        app
    }

    fn alloc_rect_instr(&mut self, area: Rect, height: u16) -> Result<Rect, Rect> {
        if self.height + height <= area.height {
            let rect = Rect::new(area.x, area.y + self.height, area.width, height);
            self.height += height;
            Ok(rect)
        } else {
            Err(Rect::new(
                area.x,
                area.y + self.height,
                area.width,
                area.height - self.height,
            ))
        }
    }

    fn alloc_top_level_layout(area: Rect) -> (Rect, Rect, Rect) {
        let header = Rect::new(area.x, area.y, area.width, 1);
        let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        let body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height - header.height - footer.height - 1,
        );
        (header, body, footer)
    }

    fn get_relative_cursor_x(&self) -> usize {
        ((self.cursor.x - self.window.x).get_value() / self.scale.get_value()) as usize
    }

    fn get_relative_visual_cursor_x(&self) -> Option<usize> {
        if self.events.in_visual_mode() {
            let cursor = if self.visual_cursor.x < self.window.x {
                0
            } else {
                ((self.visual_cursor.x - self.window.x).get_value() / self.scale.get_value())
                    as usize
            };
            Some(cursor)
        } else {
            None
        }
    }

    fn get_time_range(&self, offset: u16) -> (Timestamp, Timestamp) {
        let begin = self.window.x + Timestamp::new(self.scale.get_value() * offset as i64);
        let end = begin + self.scale;
        (begin, end)
    }

    fn render_waveform<B: Backend>(
        &mut self,
        f: &mut Frame<B>,
        rect: Rect,
        signal_id: &str,
        selected: bool,
    ) -> Result<(), Box<dyn Error>> {
        let mut data = Vec::new();
        for i in 0..rect.width {
            let (begin, end) = self.get_time_range(i);
            let (before, nb_events, after) = self.signaldb.sync_db.events_between(
                signal_id, begin, end
            )?;
            if after.is_invalid() {
                data.push(WaveformElement::Invalid)
            } else if nb_events == 0 || (nb_events == 1 && before.is_invalid()) {
                if before.width() == 1 {
                    if after == SignalValue::from_str("0").unwrap() {
                        data.push(WaveformElement::Low)
                    } else {
                        data.push(WaveformElement::High)
                    }
                } else {
                    data.push(WaveformElement::Value(format!("{}", before)))
                }
            } else if nb_events == 1 {
                if before.width() == 1 {
                    if before == SignalValue::from_str("0").unwrap() {
                        data.push(WaveformElement::RisingEdge)
                    } else {
                        data.push(WaveformElement::FallingEdge)
                    }
                } else {
                    data.push(WaveformElement::Transition)
                }
            } else if nb_events <= 3 {
                data.push(WaveformElement::LowDensity)
            } else if nb_events <= 10 {
                data.push(WaveformElement::MediumDensity)
            } else {
                data.push(WaveformElement::HighDensity)
            }
        }
        let value = self.signaldb.sync_db.value_at(signal_id, self.cursor.x)?;
        let fullname = self.signaldb.sync_db.get_signal_fullname(signal_id)?;
        Waveform::new(
            format!("{}{}: {} = {}",
                    if selected { "-> " } else { "" },
                    signal_id, fullname, value),
            &data[..],
            selected,
            self.get_relative_cursor_x(),
            self.get_relative_visual_cursor_x()
        )
        .render(f, rect);
        Ok(())
    }

    fn render_search<B: Backend>(
        &mut self,
        f: &mut Frame<B>,
        rect: Rect,
        expr: &str,
        selected: bool,
    ) -> Result<(), Box<dyn Error>> {
        let mut data = Vec::new();
        for i in 0..rect.width {
            let (begin, end) = self.get_time_range(i);
            data.push(self.signaldb.sync_db.findings_between(expr, begin, end)?)
        }
        SearchBar::new(
            format!("{}{}", if selected { "-> " } else { "" }, expr),
            &data[..],
            selected,
            self.get_relative_cursor_x(),
            self.get_relative_visual_cursor_x()
        )
        .render(f, rect);
        Ok(())
    }

    fn render_instr<B: Backend>(
        &mut self,
        f: &mut Frame<B>,
        rect: Rect,
        instr: &TuiInstr,
        selected: bool,
    ) -> Result<(), Box<dyn Error>> {
        match instr {
            TuiInstr::Signal(id) => self.render_waveform(f, rect, id, selected)?,
            TuiInstr::Search(expr) => self.render_search(f, rect, expr, selected)?,
            TuiInstr::Error(line, err) => {
                ErrorBar::new(format!("{}: {}", line, err), selected).render(f, rect)
            }
        }
        Ok(())
    }

    fn render_instrs<B: Backend>(&mut self, f: &mut Frame<B>) {
        let cursor = self.cursor.y - self.window.y;
        let area = self.area;
        let mut scrollable = false;
        let layout = self.layout.clone();
        for (i, instr) in layout[self.window.y..].iter().enumerate() {
            let selected = i == cursor;
            match self.alloc_rect_instr(area, TuiInstr::height(instr) as u16) {
                Ok(instr_rect) => match self.render_instr(f, instr_rect, &instr, selected) {
                    Ok(_) => (),
                    Err(err) => ErrorBar::new(format!("{}", err), selected).render(f, instr_rect),
                },
                Err(_) => scrollable = true,
            }
        }
        let last_instr = Rect::new(self.area.x, self.height + 1, self.area.width, 1);
        CursorBar::new(
            CursorType::Bottom,
            self.cursor.x,
            self.scale,
            self.get_relative_cursor_x(),
            scrollable,
        )
        .render(f, last_instr)
    }

    fn adjust_window(&mut self) {
        if self.cursor.x < self.window.x {
            self.window.x = self.cursor.x
        }

        let period = Timestamp::new(self.scale.get_value() * (self.area.width - 1) as i64);
        if self.window.x + period < self.cursor.x {
            self.window.x = self.cursor.x - period
        }

        if self.cursor.y < self.window.y {
            self.window.y = self.cursor.y
        }

        if self.cursor.y > self.layout.len() {
            self.cursor.y = self.layout.len() - 1;
            self.set_status("Reached last signal")
        }
        while TuiInstr::total_height(&self.layout[self.window.y..=self.cursor.y])
            > self.area.height as usize
        {
            self.window.y += 1
        }
    }

    fn center_window(&mut self) {
        let period = Timestamp::new(self.scale.get_value() * (self.area.width / 2) as i64);
        self.window.x = self.cursor.x - period
    }

    fn get_current_instr_height(&self) -> usize {
        let mut height = 0;
        while self.window.y + height < self.layout.len() &&
            TuiInstr::total_height(&self.layout[self.window.y..=self.window.y + height]) <
            self.area.height as usize
        {
            height += 1
        }
        height
    }

    fn center_window_vertical(&mut self) {
        let height = self.get_current_instr_height();
        let middle = self.window.y + height / 2;
        if middle < self.cursor.y {
            self.window.y += self.cursor.y - middle
        } else if middle > self.cursor.y {
            let shift = middle - self.cursor.y;
            if self.window.y > shift {
                self.window.y -= shift
            } else {
                self.window.y = 0
            }
        }

        self.adjust_window()
    }

    fn set_cursor_horizontal(&mut self, x: u16) {
        let offset = Timestamp::new((x - 1) as i64 * self.scale.get_value());
        self.cursor.x = self.window.x + offset;

    }

    fn set_cursor_vertical(&mut self, y: u16) {
        let mut height: usize = 0;
        if y == 0 {
            self.up()
        } else {
            for (i, instr) in self.layout[self.window.y..].iter().enumerate() {
                if (y as usize) >= height && (y as usize) <= height + instr.height() + 1 {
                    self.cursor.y = i + self.window.y;
                    break
                }
                height += instr.height()
            }
        }
    }

    pub fn render<B: Backend>(&mut self, f: &mut Frame<B>) {
        if self.cursor.y >= self.layout.len() {
            self.cursor.y = self.layout.len() - 1;
            self.set_status("Reached last signal")
        }
        let (header, body, footer) = App::alloc_top_level_layout(f.size());
        self.area = body;
        self.height = 0;
        self.adjust_window();

        CursorBar::new(
            CursorType::Top,
            self.cursor.x,
            self.scale,
            self.get_relative_cursor_x(),
            self.window.y > 0,
        )
        .render(f, header);
        self.render_instrs(f);
        let status = self.signaldb.sync_db.get_status();
        if !status.is_empty() {
            self.set_status("")
        };
        StatusBar::new(
            if self.events.in_visual_mode() {
                format!(
                    "-- VISUAL -- ({})  Enter:Zoom Fit  hjkl:Move  Esc:Abort",
                    self.cursor.x - self.visual_cursor.x
                )
            } else if self.events.in_search_mode() {
                format!(
                    "Search {:?}: {}█",
                    self.events.get_search_target(),
                    self.events.get_buffer()).to_string()
            } else if !status.is_empty() {
                status
            } else {
                HELP_MSG.to_string()
            },
            if !self.events.in_search_mode() {
                self.events.get_buffer().to_string()
            } else {
                "".to_string()
            }
        )
        .render(f, footer)
    }

    pub fn edit(&mut self) {
        let mut dir = env::temp_dir();
        dir.push("dwfv_layout");

        {
            let mut f = File::create(&dir).expect("Cannot create file");
            TuiInstr::format_instrs(&self.layout[..], &mut f);
            let _ = f.write_all(b"\n# Signals:\n#\n");
            self.signaldb.sync_db.format_stats(&mut f);
        }

        match env::var("EDITOR") {
            Ok(editor) => {
                let mut child = Command::new(editor)
                    .arg(dir.as_os_str())
                    .spawn()
                    .expect("Failed to start editor");
                child
                    .wait()
                    .expect("Failed while waiting for child process");
            }
            Err(e) => self
                .signaldb
                .sync_db
                .set_status(format!("Error while reading $EDITOR: {}", e).as_str()),
        }

        self.update_layout(&dir).expect("Cannot reload layout file")
    }

    pub fn update_layout<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<dyn Error>> {
        let f = File::open(&path)?;
        let file = BufReader::new(f);
        self.snapshot_layout();
        self.update_layout_list(TuiInstr::parse(file));
        Ok(())
    }

    fn update_layout_list(&mut self, layout: Vec<TuiInstr>) {
        let mut reviewed_layout = Vec::new();
        for instr in layout.iter() {
            match instr {
                TuiInstr::Search(expr) => {
                    self.signaldb.search(&expr);
                    reviewed_layout.push(instr.clone())
                }
                TuiInstr::Signal(signal) => {
                    if self.signaldb.sync_db.signal_exists(signal) {
                        reviewed_layout.push(TuiInstr::Signal(signal.clone()))
                    } else {
                        let mut ids = self.signaldb.sync_db.find_signals(|s| s.name == *signal);
                        if let Some(id) = ids.pop() {
                            reviewed_layout.push(TuiInstr::Signal(id))
                        } else {
                            reviewed_layout.push(
                                TuiInstr::Error(signal.clone(), "Unknown signal".to_string())
                            )
                        }
                    }
                }
                TuiInstr::Error(_, _) => {
                    reviewed_layout.push(instr.clone())
                }
            }
        }
        self.layout = reviewed_layout
    }

    fn goto_next_rising_edge(&mut self) {
        let res = match &self.layout[self.cursor.y] {
            TuiInstr::Signal(id) => {
                self.signaldb.sync_db.get_next_rising_edge(&id, self.cursor.x).unwrap()
            },
            TuiInstr::Search(expr) => {
                self.signaldb.sync_db.get_next_finding(&expr, self.cursor.x).unwrap()
            },
            _ => None
        };
        if let Some(t) = res {
            self.cursor.x = t;
            self.center_window()
        } else {
            self.set_status("No further rising edge")
        }
    }

    fn goto_next_falling_edge(&mut self) {
        let res = match &self.layout[self.cursor.y] {
            TuiInstr::Signal(id) => {
                self.signaldb.sync_db.get_next_falling_edge(&id, self.cursor.x).unwrap()
            },
            TuiInstr::Search(expr) => {
                self.signaldb.sync_db.get_end_of_next_finding(&expr, self.cursor.x).unwrap()
            },
            _ => None
        };
        if let Some(t) = res {
            self.cursor.x = t;
            self.center_window()
        } else {
            self.set_status("No further falling edge")
        }
    }

    fn goto_previous_rising_edge(&mut self) {
        let res = match &self.layout[self.cursor.y] {
            TuiInstr::Signal(id) => {
                self.signaldb.sync_db.get_previous_rising_edge(&id, self.cursor.x).unwrap_or(None)
            },
            TuiInstr::Search(expr) => {
                self.signaldb.sync_db.get_previous_finding(&expr, self.cursor.x).unwrap_or(None)
            },
            _ => None
        };
        if let Some(t) = res {
            self.cursor.x = t;
            self.center_window()
        } else {
            self.set_status("No previous rising edge")
        }
    }

    fn goto_first_event(&mut self) {
        let res = match &self.layout[self.cursor.y] {
            TuiInstr::Signal(id) => {
                self.signaldb.sync_db.get_first_event(&id).unwrap_or(None)
            },
            TuiInstr::Search(expr) => {
                self.signaldb.sync_db.get_first_finding(&expr).unwrap_or(None)
            },
            _ => None
        };
        if let Some(t) = res {
            self.cursor.x = t;
            self.center_window()
        } else {
            self.set_status("No first event")
        }
    }

    fn goto_last_event(&mut self) {
        let res = match &self.layout[self.cursor.y] {
            TuiInstr::Signal(id) => {
                self.signaldb.sync_db.get_last_event(&id).unwrap_or(None)
            },
            TuiInstr::Search(expr) => {
                self.signaldb.sync_db.get_last_finding(&expr).unwrap_or(None)
            },
            _ => None
        };
        if let Some(t) = res {
            self.cursor.x = t;
            self.center_window()
        } else {
            self.set_status("No last event")
        }
    }

    fn fit_to_selection(&mut self) {
        let visual_cursor = self.visual_cursor.x.get_value();
        let cursor = self.cursor.x.get_value();
        let begin = cmp::min(visual_cursor, cursor);
        let end = cmp::max(visual_cursor, cursor);

        let scale = (end - begin) / self.area.width as i64;
        if scale >= 1 {
            self.scale = Timestamp::new(scale);
            self.window.x = Timestamp::new(begin);
            self.cursor.x = Timestamp::new(begin + ((end - begin) / 2))
        }
    }

    fn zoom_fit(&mut self) {
        let period = match &self.layout[self.cursor.y] {
            TuiInstr::Signal(id) => Some((
                self.signaldb.sync_db.get_first_event(&id).unwrap_or(None),
                self.signaldb.sync_db.get_last_event(&id).unwrap_or(None)
            )),
            TuiInstr::Search(expr) => Some((
                self.signaldb.sync_db.get_first_finding(&expr).unwrap_or(None),
                self.signaldb.sync_db.get_last_finding(&expr).unwrap_or(None)
            )),
            _ => None
        };

        match period {
            Some((Some(first_event), Some(last_event))) => {
                let begin = first_event.get_value();
                let end = last_event.get_value();

                let scale = (end - begin) / self.area.width as i64;
                if scale >= 1 {
                    self.scale = Timestamp::new(scale);
                    self.window.x = Timestamp::new(begin);
                    if self.cursor.x < first_event {
                        self.cursor.x = first_event;
                    } else if last_event < self.cursor.x {
                        self.cursor.x = last_event;
                    }
                } else {
                    self.set_status("Cannot zoom more")
                }
            },
            _ => {
                self.set_status(&format!("Cannot zoom fit {}", &self.layout[self.cursor.y]))
            }
        }
    }

    fn matches_search_pattern(&self, instr: &TuiInstr) -> bool {
        let id = match instr {
            TuiInstr::Signal(id) => self.signaldb.sync_db.get_signal_fullname(id).unwrap(),
            TuiInstr::Search(expr) => expr.to_string(),
            _ => return false
        };
        id.contains(&self.search_pattern)
    }

    fn search_next(&mut self) {
        if self.cursor.y + 1 >= self.layout.len() {
            return
        }
        for (i, instr) in self.layout[self.cursor.y + 1..].iter().enumerate() {
            if self.matches_search_pattern(&instr) {
                self.cursor.y += i + 1;
                self.adjust_window();
                self.center_window_vertical();
                return
            }
        }
        self.set_status(&format!("Cannot find '{}' downward", self.search_pattern))
    }

    fn search_prev(&mut self) {
        if self.cursor.y == 0 {
            return
        }
        for (i, instr) in self.layout[..self.cursor.y].iter().rev().enumerate() {
            if self.matches_search_pattern(instr) {
                self.cursor.y -= i + 1;
                self.adjust_window();
                self.center_window_vertical();
                return
            }
        }
        self.set_status(&format!("Cannot find '{}' upward", self.search_pattern))
    }

    fn set_status(&mut self, msg: &str) {
        self.signaldb.sync_db.set_status(msg);
    }

    fn undo(&mut self) {
        if let Some(prev_layout) = self.memento.past.pop() {
            self.memento.future.push(self.layout.clone());
            self.update_layout_list(prev_layout)
        } else {
            self.set_status("No previous changes")
        }
    }

    fn redo(&mut self) {
        if let Some(next_layout) = self.memento.future.pop() {
            self.memento.past.push(self.layout.clone());
            self.update_layout_list(next_layout)
        } else {
            self.set_status("Already at newest change")
        }
    }

    fn snapshot_layout(&mut self) {
        self.memento.past.push(self.layout.clone());
        self.memento.future.clear();
    }

    fn up(&mut self) {
        if self.cursor.y > 0 {
            self.cursor.y -= 1
        } else {
            self.set_status("Reached first signal")
        }
    }

    fn down(&mut self) {
        self.cursor.y += 1
    }

    fn show_clipboard(&mut self) {
        let mut s = String::new();
        for element in self.clipboard.iter().rev() {
            s.push_str(&format!("{}, ", element))
        }
        s.push_str("EOS");
        self.set_status(&s);
    }

    fn search(&mut self, target: SearchTarget, pattern: &str) {
        match target {
            SearchTarget::Signal => {
                self.search_pattern = String::from(pattern);
                self.search_next()
            }
            SearchTarget::Event => {
                if let TuiInstr::Signal(signal_id) = &self.layout[self.cursor.y] {
                    let expr = format!("${} = {}", signal_id, pattern);
                    self.signaldb.search(&expr);
                    let instr = TuiInstr::Search(expr);
                    self.snapshot_layout();
                    self.layout.insert(self.cursor.y + 1, instr);
                } else {
                    self.set_status("Cannot search events in this line")
                }
            }
            _ => {}
        }
    }

    pub fn update(&mut self) -> bool {
        self.events.update();
        loop {
            let evt = self.events.get_event();
            match evt {
                Event::None => return false,
                Event::Quit => return true,
                Event::Left => {
                    self.cursor.x -= self.scale;
                }
                Event::Right => {
                    self.cursor.x += self.scale;
                }
                Event::Up => self.up(),
                Event::Down => self.down(),
                Event::PageUp => {
                    let height = self.get_current_instr_height();
                    if self.cursor.y > height {
                        self.cursor.y -= height
                    } else {
                        self.cursor.y = 0;
                        self.set_status("Reached first signal")
                    }
                }
                Event::PageDown => {
                    self.cursor.y += self.get_current_instr_height()
                }
                Event::ZoomOut => {
                    let mut scale = self.scale.get_value() * 2;
                    if scale > MAX_ZOOM {
                        scale = MAX_ZOOM;
                        self.set_status("Cannot zoom out more")
                    }
                    self.scale = Timestamp::new(scale);
                    self.center_window()
                }
                Event::ZoomIn => {
                    let mut scale = self.scale.get_value() / 2;
                    if scale < 1 {
                        scale = 1;
                        self.set_status("Cannot zoom in more")
                    };
                    self.scale = Timestamp::new(scale);
                    self.center_window()
                },
                Event::ZoomFit => self.zoom_fit(),
                Event::CenterWindow => {
                    self.center_window();
                    self.center_window_vertical()
                },
                Event::GotoTop => self.cursor.y = 0,
                Event::GotoLast => self.cursor.y = std::usize::MAX,
                Event::GotoNextRisingEdge => self.goto_next_rising_edge(),
                Event::GotoNextFallingEdge => self.goto_next_falling_edge(),
                Event::GotoPreviousRisingEdge => self.goto_previous_rising_edge(),
                Event::GotoFirstEvent => self.goto_first_event(),
                Event::GotoLastEvent => self.goto_last_event(),
                Event::GotoZero => self.cursor.x = Timestamp::new(0),
                Event::StartVisualMode => self.visual_cursor = self.cursor.clone(),
                Event::FitToSelection => self.fit_to_selection(),
                Event::Edit => self.edit(),
                Event::Delete => {
                    self.snapshot_layout();
                    self.set_status(&format!(
                        "Stashed {}", self.layout[self.cursor.y]
                    ));
                    if self.layout.len() > 1 {
                        self.clipboard.push(self.layout.remove(self.cursor.y))
                    };
                },
                Event::Yank => {
                    self.set_status(&format!("Peeked {}", self.layout[self.cursor.y]));
                    self.clipboard.push(self.layout[self.cursor.y].clone())
                },
                Event::PasteBefore => {
                    self.snapshot_layout();
                    if let Some(clipboard) = self.clipboard.pop() {
                        self.layout.insert(self.cursor.y, clipboard.clone());
                        self.signaldb.sync_db.set_status(&format!("Popped {}", clipboard))
                    } else {
                        self.set_status("Clipboard is empty");
                    }
                },
                Event::PasteAfter => {
                    self.snapshot_layout();
                    if let Some(clipboard) = self.clipboard.pop() {
                        self.cursor.y += 1;
                        self.layout.insert(self.cursor.y, clipboard.clone());
                        self.signaldb.sync_db.set_status(&format!("Popped {}", clipboard))
                    } else {
                        self.set_status("Clipboard is empty");
                    }
                },
                Event::Search(target, pattern) => {
                    self.search(target, &pattern)
                },
                Event::SearchNext => self.search_next(),
                Event::SearchPrev => self.search_prev(),
                Event::SetCursorVertical(x) => self.set_cursor_vertical(x),
                Event::SetCursorHorizontal(y) => self.set_cursor_horizontal(y),
                Event::Undo => self.undo(),
                Event::Redo => self.redo(),
                _ => (),
            }
        }
    }
}
