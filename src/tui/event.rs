// SPDX-License-Identifier: MIT
use regex::Regex;
use std::collections::VecDeque;
use std::io;
use termion::event::{Key, MouseEvent, MouseButton};
use termion::input::TermRead;
use termion::event::Event as RawEvent;

const BUFFER_MAX_SIZE:usize = 8;

#[derive(Clone)]
pub enum Event {
    None,
    Quit,
    Left,
    Right,
    Up,
    Down,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    CenterWindow,
    GotoTop,
    GotoLast,
    GotoNextRisingEdge,
    GotoPreviousRisingEdge,
    GotoNextFallingEdge,
    GotoFirstEvent,
    GotoLastEvent,
    GotoZero,
    StartVisualMode,
    FitToSelection,
    StopVisualMode,
    Edit,
    PageDown,
    PageUp,
    PasteAfter,
    PasteBefore,
    Yank,
    Delete,
    Search(String),
    SearchNext,
    SearchPrev,
    SetCursorVertical(u16),
    SetCursorHorizontal(u16),
    Undo,
    Redo
}

pub enum InputMode {
    Command,
    Visual,
    Search
}

pub struct Events {
    buffer: String,
    previous_buffer: String,
    events: VecDeque<Event>,
    mode: InputMode
}

impl Events {
    pub fn new() -> Events {
        Events {
            buffer: String::new(),
            previous_buffer: String::new(),
            events: VecDeque::new(),
            mode: InputMode::Command,
        }
    }

    pub fn in_visual_mode(&self) -> bool {
        if let InputMode::Visual = self.mode {
            true
        } else {
            false
        }
    }

    pub fn in_search_mode(&self) -> bool {
        if let InputMode::Search = self.mode {
            true
        } else {
            false
        }
    }

    fn clear_buffer(&mut self) {
        self.previous_buffer.clear();
        self.previous_buffer.push_str(&self.buffer);
        self.buffer.clear()
    }

    const CMDS: [(&'static str, &'static dyn Fn(&mut Events) -> Event); 33] = [
        ("j", &|_| Event::Down),
        ("k", &|_| Event::Up),
        ("l", &|_| Event::Right),
        ("h", &|_| Event::Left),
        ("q", &|_| Event::Quit),
        ("-", &|_| Event::ZoomOut),
        ("+", &|_| Event::ZoomIn),
        ("=", &|_| Event::ZoomFit),
        ("zo", &|_| Event::ZoomOut),
        ("zi", &|_| Event::ZoomIn),
        ("zc", &|_| Event::ZoomFit),
        ("w", &|_| Event::GotoNextRisingEdge),
        ("b", &|_| Event::GotoPreviousRisingEdge),
        ("e", &|_| Event::GotoNextFallingEdge),
        ("zz", &|_| Event::CenterWindow),
        ("gg", &|_| Event::GotoTop),
        ("G", &|_| Event::GotoLast),
        ("0", &|_| Event::GotoZero),
        ("^", &|_| Event::GotoFirstEvent),
        ("$", &|_| Event::GotoLastEvent),
        ("o", &|_| Event::Edit),
        ("dd", &|_| Event::Delete),
        ("yy", &|_| Event::Yank),
        ("p", &|_| Event::PasteAfter),
        ("P", &|_| Event::PasteBefore),
        ("N", &|_| Event::SearchPrev),
        ("n", &|_| Event::SearchNext),
        ("u", &|_| Event::Undo),
        ("r", &|_| Event::Redo),
        ("v", &|evt| {
            if let InputMode::Visual = evt.mode {
                evt.mode = InputMode::Command;
                Event::StopVisualMode
            } else {
                evt.mode = InputMode::Visual;
                Event::StartVisualMode
            }
        }),
        (" ", &|evt| {
            if let InputMode::Visual = evt.mode {
                evt.mode = InputMode::Command;
                Event::StopVisualMode
            } else {
                evt.mode = InputMode::Visual;
                Event::StartVisualMode
            }
        }),
        ("/", &|evt| {
            evt.mode = InputMode::Search;
            evt.buffer.clear();
            Event::None
        }),
        (".", &|evt| {
            evt.buffer.clear();
            evt.buffer.push_str(&evt.previous_buffer);
            let _ = evt.parse_buffer();
            Event::None
        })
    ];

    fn parse_buffer(&mut self) -> Result<(), ()> {
        lazy_static! {
            static ref RE: Regex = Regex::new("(?P<i>[1-9][0-9]*)?(?P<cmd_buff>.*)?").unwrap();
        }

        let buf = self.buffer.clone();
        if let Some(cap) = RE.captures(&buf) {
            let mut cmd = Event::None;
            if let Some(cmd_buff) = cap.name("cmd_buff") {
                let cmd_buff = cmd_buff.as_str().to_string();
                for (name, action) in Events::CMDS.iter() {
                    if cmd_buff.contains(name) {
                         cmd = action(self)
                    }
                }
            } else {
                return Err(())
            }

            let repeat = if let Some(i) = cap.name("i") {
                i.as_str().parse().unwrap()
            } else {
                1
            };

            if let Event::None = cmd {
                Err(())
            } else {
                for _ in 0..repeat {
                    self.events.push_back(cmd.clone())
                }
                Ok(())
            }
        } else {
            Err(())
        }
    }

    pub fn update(&mut self) {
        let evt = io::stdin().events().next();
        if let Some(Ok(evt)) = evt {
            match evt {
                RawEvent::Key(key) => match key {
                    Key::Up => {
                        self.events.push_back(Event::Up);
                        self.clear_buffer()
                    }
                    Key::Down => {
                        self.events.push_back(Event::Down);
                        self.clear_buffer()
                    }
                    Key::Left => {
                        self.events.push_back(Event::Left);
                        self.clear_buffer()
                    }
                    Key::Right => {
                        self.events.push_back(Event::Right);
                        self.clear_buffer()
                    }
                    Key::PageUp => {
                        self.events.push_back(Event::PageUp);
                        self.clear_buffer()
                    }
                    Key::PageDown => {
                        self.events.push_back(Event::PageDown);
                        self.clear_buffer()
                    }
                    Key::Delete => {
                        if let InputMode::Command = self.mode {
                            self.events.push_back(Event::Delete);
                            self.clear_buffer()
                        }
                    }
                    Key::Home => {
                        if let InputMode::Command = self.mode {
                            self.events.push_back(Event::GotoFirstEvent);
                            self.clear_buffer()
                        }
                    }
                    Key::End => {
                        if let InputMode::Command = self.mode {
                            self.events.push_back(Event::GotoLastEvent);
                            self.clear_buffer()
                        }
                    }
                    Key::Esc => {
                        self.mode = InputMode::Command;
                        self.clear_buffer()
                    }
                    Key::Backspace => {
                        if let InputMode::Search = self.mode {
                            self.buffer.pop();
                        }
                    }
                    Key::Char(c) => {
                        if c == '\n' {
                            match self.mode {
                                InputMode::Visual => {
                                    self.mode = InputMode::Command;
                                    self.events.push_back(Event::FitToSelection)
                                },
                                InputMode::Command => {
                                    self.mode = InputMode::Visual;
                                    self.events.push_back(Event::StartVisualMode)
                                },
                                InputMode::Search => {
                                    self.mode = InputMode::Command;
                                    self.events.push_back(Event::Search(self.buffer.clone()))
                                }
                            }
                            self.buffer.clear();
                        } else {
                            self.buffer.push(c);
                            match self.mode {
                                InputMode::Command | InputMode::Visual =>
                                    if self.buffer.len() >= BUFFER_MAX_SIZE {
                                        self.buffer.clear()
                                    }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                },
                RawEvent::Mouse(m) => {
                    match m {
                        MouseEvent::Press(button, x, y) => {
                            match button {
                                MouseButton::WheelUp => {
                                    self.events.push_back(Event::ZoomIn);
                                    self.clear_buffer()
                                },
                                MouseButton::WheelDown => {
                                    self.events.push_back(Event::ZoomOut);
                                    self.clear_buffer()
                                },
                                MouseButton::Left => {
                                    self.events.push_back(Event::SetCursorHorizontal(x));
                                    self.events.push_back(Event::SetCursorVertical(y));
                                    self.clear_buffer()
                                },
                                MouseButton::Middle => {
                                    self.events.push_back(Event::SetCursorHorizontal(x));
                                    self.events.push_back(Event::SetCursorVertical(y));
                                    self.events.push_back(Event::PasteBefore);
                                    self.clear_buffer()
                                },
                                MouseButton::Right => {
                                    self.events.push_back(Event::SetCursorHorizontal(x));
                                    self.events.push_back(Event::SetCursorVertical(y));
                                    self.events.push_back(Event::Yank);
                                    self.clear_buffer()
                                },
                            }
                        },
                        MouseEvent::Release(x, _) => {
                            if let InputMode::Visual = self.mode {
                                self.mode = InputMode::Command;
                                self.events.push_back(Event::SetCursorHorizontal(x));
                                self.events.push_back(Event::FitToSelection);
                                self.clear_buffer()
                            }
                        },
                        MouseEvent::Hold(x, _) => {
                            if let InputMode::Visual = self.mode {
                            } else {
                                self.mode = InputMode::Visual;
                                self.events.push_back(Event::StartVisualMode);
                            }
                            self.events.push_back(Event::SetCursorHorizontal(x));
                            self.clear_buffer()
                        }
                    }
                },
                _ => {}
            }
        }
        if let InputMode::Search = self.mode {
        } else if let Ok(()) = self.parse_buffer() {
            self.clear_buffer()
        }
    }

    pub fn get_event(&mut self) -> Event {
        if let Some(evt) = self.events.pop_front() {
            evt
        } else {
            Event::None
        }
    }

    pub fn get_buffer(&self) -> &str {
        &self.buffer
    }
}
