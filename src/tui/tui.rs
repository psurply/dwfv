// SPDX-License-Identifier: MIT
use crate::signaldb::AsyncSignalDB;
use std::error::Error;
use std::io;
use std::path::Path;
use super::app::App;
use termion::raw::IntoRawMode;
use tuirs::backend::TermionBackend;
use tuirs::Terminal;

/// Digital Waveform Viewer Text User Interface
pub struct Tui {
    term: Terminal<TermionBackend<termion::raw::RawTerminal<io::Stdout>>>,
    app: App,
}

impl Tui {
    /// Create a new `Tui`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::AsyncSignalDB;
    /// use dwfv::tui::Tui;
    /// let tui = Tui::new(AsyncSignalDB::new());
    /// ```
    pub fn new(signaldb: AsyncSignalDB) -> Result<Tui, Box<dyn Error>> {
        let stdout = io::stdout().into_raw_mode()?;
        let backend = TermionBackend::new(stdout);
        let term = Terminal::new(backend)?;
        Ok(Tui {
            term,
            app: App::new(signaldb),
        })
    }

    fn render(&mut self) -> Result<(), Box<dyn Error>> {
        let term = &mut self.term;
        let app = &mut self.app;
        term.draw(|mut f| app.render(&mut f))?;
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.term.clear()?;
        loop {
            self.term.hide_cursor()?;
            self.render()?;
            if self.app.update() {
                break;
            }
        }
        self.term.clear()?;
        Ok(())
    }

    pub fn update_layout<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<dyn Error>> {
        self.app.update_layout(path)
    }
}
