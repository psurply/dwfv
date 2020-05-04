// SPDX-License-Identifier: MIT
use super::db::SignalDB;
use std::default::Default;
use std::io;
use std::sync::Arc;
use std::thread;

/// Asynchronous Signal DB
pub struct AsyncSignalDB {
    /// Synchronous Signal Database
    pub sync_db: Arc<SignalDB>,
    workers: Vec<thread::JoinHandle<()>>
}

impl Default for AsyncSignalDB {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncSignalDB {
    /// Create a new `AsyncSignalDB`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::AsyncSignalDB;
    /// let db = AsyncSignalDB::new();
    /// ```
    pub fn new() -> Self {
        AsyncSignalDB {
            sync_db: Arc::new(SignalDB::new()),
            workers: Vec::new()
        }
    }

    /// Populate the `SignalDB` using a Value Change Dump (VCD) file in a separate thread.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::AsyncSignalDB;
    /// let vcd = std::io::Cursor::new("
    /// $scope module logic $end
    /// $var wire 1 # foo $end
    /// $upscope $end
    /// $enddefinitions $end
    /// $dumpvars
    /// b1 #
    /// $end
    /// ");
    ///
    /// let mut db = AsyncSignalDB::new();
    /// db.parse_vcd(vcd);
    /// db.sync_db.wait_until_initialized();
    /// ```
    pub fn parse_vcd<I: io::BufRead>(&mut self, input: I)
        where I: std::marker::Send,
              I: 'static {
        let db_parse = Arc::clone(&self.sync_db);
        self.workers.push(thread::spawn(move || {
            let _ = db_parse.parse_vcd(input);
        }))
    }

    /// Search in the `SignalDB` in a separate thread.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{AsyncSignalDB, Timestamp};
    /// let vcd = std::io::Cursor::new("
    /// $scope module logic $end
    /// $var wire 1 # foo $end
    /// $upscope $end
    /// $enddefinitions $end
    /// $dumpvars
    /// b1 #
    /// $end
    /// #42
    /// b0 #
    /// ");
    ///
    /// let mut db = AsyncSignalDB::new();
    /// // Synchronous parsing of the VCD
    /// db.sync_db.parse_vcd(vcd);
    ///
    /// // Search expression
    /// let expr = "$# <- 0";
    ///
    /// // Asynchronous search
    /// db.search(expr);
    ///
    /// // Wait for the search process to find something
    /// loop {
    ///     if let Some(timestamp) = db.sync_db.get_first_finding(expr).unwrap_or(None) {
    ///         assert_eq!(timestamp, Timestamp::new(42));
    ///         break
    ///     }
    /// }
    /// ```
    pub fn search(&mut self, expr: &str) {
        let db_search = Arc::clone(&self.sync_db);
        let expr = expr.to_string();
        self.workers.push(thread::spawn(move || {
            if let Err(e) = db_search.search_init(&expr) {
                db_search.set_status(
                    format!("Cannot initialize search: {}: {}", expr, e).as_str()
                )
            };
            for timestamp in db_search.get_timestamps() {
                if let Err(e) = db_search.search_at(&expr, timestamp) {
                    db_search.set_status(
                        format!("Invalid search expression: {}: {}", expr, e).as_str()
                    );
                    return
                }
            }
            let _ = db_search.finish_search(&expr);
        }))
    }
}
