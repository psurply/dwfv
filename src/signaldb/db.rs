// SPDX-License-Identifier: MIT
use super::scope::{Scope, ScopeChild};
use super::signal::Signal;
use super::time::Timestamp;
use super::value::SignalValue;
use crate::search::{FindingsSummary, Search};
use crate::vcd::parser::Parser;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;
use std::io;
use std::sync::{Condvar, Mutex};

/// Signal Database
///
/// Example:
///
/// ```
/// use dwfv::signaldb::{Signal, SignalDB, SignalValue, Timestamp};
/// let mut db = SignalDB::new();
/// ```
pub struct SignalDB {
    scope: Mutex<Scope>,
    signals: Mutex<BTreeMap<String, Signal>>,
    timestamps: Mutex<Vec<Timestamp>>,
    now: Mutex<Timestamp>,
    searches: Mutex<HashMap<String, Search>>,
    status: Mutex<String>,
    initialized: (Mutex<bool>, Condvar),
    valid: Mutex<bool>,
}

#[derive(Debug)]
pub struct SignalNotFound {
    signal_id: String,
}

impl Error for SignalNotFound {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for SignalNotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signal not found in the database: {}", self.signal_id)
    }
}

impl SignalNotFound {
    fn new(signal_id: &str) -> Self {
        SignalNotFound {
            signal_id: signal_id.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct SearchNotFound {
    expr: String,
}

impl SearchNotFound {
    fn new(expr: &str) -> SearchNotFound {
        SearchNotFound {
            expr: expr.to_string(),
        }
    }
}

impl Error for SearchNotFound {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for SearchNotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Search not found in the database: {}", self.expr)
    }
}

#[derive(Debug)]
pub struct InitializationError {
    msg: String,
}

impl InitializationError {
    fn new(msg: &str) -> InitializationError {
        InitializationError {
            msg: msg.to_string(),
        }
    }
}

impl Error for InitializationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for InitializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to initialize the database: {}", self.msg)
    }
}

pub struct EventIterator<'a> {
    signaldb: &'a SignalDB,
    index: usize,
}

impl<'a> EventIterator<'a> {
    pub fn new(signaldb: &'a SignalDB) -> EventIterator<'a> {
        EventIterator { signaldb, index: 0 }
    }
}

impl<'a> Iterator for EventIterator<'a> {
    type Item = Timestamp;

    fn next(&mut self) -> Option<Self::Item> {
        let timestamps = self.signaldb.timestamps.lock().unwrap();
        let timestamp = timestamps.get(self.index).copied();
        self.index += 1;
        timestamp
    }
}

impl Default for SignalDB {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalDB {
    /// Create a new and empty `SignalDB`
    ///
    /// # Examples
    ///
    /// ```
    /// use dwfv::signaldb::SignalDB;
    ///
    /// let mut db = dwfv::signaldb::SignalDB::new();
    /// ```
    pub fn new() -> SignalDB {
        SignalDB {
            scope: Mutex::new(Scope::new(String::from("top"))),
            signals: Mutex::new(BTreeMap::new()),
            timestamps: Mutex::new(vec![Timestamp::new(0)]),
            now: Mutex::new(Timestamp::new(0)),
            searches: Mutex::new(HashMap::new()),
            status: Mutex::new(String::from("Test")),
            initialized: (Mutex::new(false), Condvar::new()),
            valid: Mutex::new(true),
        }
    }

    /// Create a new `SignalDB` from a Value Change Dump (VCD) file.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{SignalDB, SignalValue, Timestamp};
    /// let buf = std::io::Cursor::new("$scope module top $end
    /// $var wire 32 0 foo $end
    /// $var string 1 1 state $end
    /// $var wire 1 2 bar $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// sINIT 1
    /// $end
    /// #1337
    /// b101010 0
    /// sTEST 1
    /// ");
    ///
    /// let db = SignalDB::from_vcd(buf).unwrap();
    ///
    /// let timestamp = Timestamp::new(1336);
    /// assert_eq!(db.value_at("0", timestamp).unwrap(), SignalValue::new(0));
    /// assert_eq!(db.value_at("1", timestamp).unwrap(), SignalValue::from_symbol_str("INIT"));
    ///
    /// let timestamp = Timestamp::new(1338);
    /// assert_eq!(db.value_at("0", timestamp).unwrap(), SignalValue::new(42));
    /// assert_eq!(db.value_at("1", timestamp).unwrap(), SignalValue::from_symbol_str("TEST"));
    /// ```
    pub fn from_vcd<I: io::BufRead>(input: I) -> Result<SignalDB, Box<dyn Error>> {
        SignalDB::from_vcd_with_limit(input, None)
    }

    /// Create a new `SignalDB` from a Value Change Dump (VCD) file and stop parsing the VCD file
    /// after reaching a given timestamp.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{SignalDB, SignalValue, Timestamp};
    /// let buf = std::io::Cursor::new("$scope module top $end
    /// $var wire 32 0 foo $end
    /// $var string 1 1 state $end
    /// $var wire 1 2 bar $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// sINIT 1
    /// $end
    /// #1337
    /// b101010 0
    /// sTEST 1
    /// ");
    ///
    /// let db = SignalDB::from_vcd_with_limit(buf, Some(Timestamp::new(1300))).unwrap();
    ///
    /// let timestamp = Timestamp::new(1000);
    /// assert_eq!(db.value_at("0", timestamp).unwrap(), SignalValue::new(0));
    /// assert_eq!(db.value_at("1", timestamp).unwrap(), SignalValue::from_symbol_str("INIT"));
    ///
    /// let timestamp = Timestamp::new(1338);
    /// assert_eq!(db.value_at("0", timestamp).unwrap(), SignalValue::new(0));
    /// assert_eq!(db.value_at("1", timestamp).unwrap(), SignalValue::from_symbol_str("INIT"));
    /// ```
    pub fn from_vcd_with_limit<I: io::BufRead>(
        input: I,
        timestamp: Option<Timestamp>,
    ) -> Result<SignalDB, Box<dyn Error>> {
        let db = SignalDB::new();
        db.parse_vcd_with_limit(input, timestamp)?;
        Ok(db)
    }

    /// Extend the current `SignalDB` with the signals defined in a Value Capture Dump (VCD) file.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{SignalDB, SignalValue, Timestamp};
    /// let buf = std::io::Cursor::new("$scope module top $end
    /// $var wire 32 0 foo $end
    /// $var string 1 1 state $end
    /// $var wire 1 2 bar $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// sINIT 1
    /// $end
    /// #1337
    /// b101010 0
    /// sTEST 1
    /// ");
    ///
    /// let mut db = SignalDB::new();
    /// db.parse_vcd(buf).unwrap();
    ///
    /// let timestamp = Timestamp::new(1336);
    /// assert_eq!(db.value_at("0", timestamp).unwrap(), SignalValue::new(0));
    /// assert_eq!(db.value_at("1", timestamp).unwrap(), SignalValue::from_symbol_str("INIT"));
    ///
    /// let timestamp = Timestamp::new(1338);
    /// assert_eq!(db.value_at("0", timestamp).unwrap(), SignalValue::new(42));
    /// assert_eq!(db.value_at("1", timestamp).unwrap(), SignalValue::from_symbol_str("TEST"));
    /// ```
    pub fn parse_vcd<I: io::BufRead>(&self, input: I) -> Result<(), Box<dyn Error>> {
        self.parse_vcd_with_limit(input, None)
    }

    /// Extend the current `SignalDB` with the signals defined in a Value Change Dump (VCD) file
    /// and stop parsing the VCD file after reaching a given timestamp.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{SignalDB, SignalValue, Timestamp};
    /// let buf = std::io::Cursor::new("$scope module top $end
    /// $var wire 32 0 foo $end
    /// $var string 1 1 state $end
    /// $var wire 1 2 bar $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// sINIT 1
    /// $end
    /// #1337
    /// b101010 0
    /// sTEST 1
    /// ");
    ///
    /// let mut db = SignalDB::new();
    /// db.parse_vcd_with_limit(buf, Some(Timestamp::new(1300))).unwrap();
    ///
    /// let timestamp = Timestamp::new(1000);
    /// assert_eq!(db.value_at("0", timestamp).unwrap(), SignalValue::new(0));
    /// assert_eq!(db.value_at("1", timestamp).unwrap(), SignalValue::from_symbol_str("INIT"));
    ///
    /// let timestamp = Timestamp::new(1338);
    /// assert_eq!(db.value_at("0", timestamp).unwrap(), SignalValue::new(0));
    /// assert_eq!(db.value_at("1", timestamp).unwrap(), SignalValue::from_symbol_str("INIT"));
    /// ```
    pub fn parse_vcd_with_limit<I: io::BufRead>(
        &self,
        input: I,
        timestamp: Option<Timestamp>,
    ) -> Result<(), Box<dyn Error>> {
        let mut parser = Parser::new(input, self);
        if let Some(t) = timestamp {
            parser.set_limit(t)
        }
        self.set_status("Parsing VCD file...");
        parser.parse().map_err(|err| {
            self.set_status(format!("{}", err).as_str());
            self.mark_as_invalid();
            self.mark_as_initialized();
            err
        })?;
        let timestamps = self.timestamps.lock().unwrap();
        self.set_status(format!("Ready: {} events", timestamps.len()).as_str());
        Ok(())
    }

    /// Indicate that the `SignalDB` is initialized, meaning that no additional signals are
    /// expected to be added after that point.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalDB;
    ///
    /// let mut db = SignalDB::new();
    ///
    /// db.mark_as_initialized()
    /// ```
    pub fn mark_as_initialized(&self) {
        let &(ref lock, ref cvar) = &self.initialized;
        let mut initialized = lock.lock().unwrap();
        *initialized = true;
        cvar.notify_all()
    }

    /// Indicate that the content of the `SignalDB` may be invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalDB;
    /// let mut db = SignalDB::new();
    ///
    /// assert_eq!(db.is_valid(), true);
    /// db.mark_as_invalid();
    /// assert_eq!(db.is_valid(), false);
    /// ```
    pub fn mark_as_invalid(&self) {
        let mut valid = self.valid.lock().unwrap();
        *valid = false
    }

    /// Check that the content of the `SignalDB` has not be been marked as invalid.
    pub fn is_valid(&self) -> bool {
        let valid = self.valid.lock().unwrap();
        *valid
    }

    /// Wait until the `SignalDB` is marked as initialized and check that the content has not been
    /// invalidated.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalDB;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let db = Arc::new(SignalDB::new());
    /// let db2 = db.clone();
    ///
    /// thread::spawn(move|| {
    ///     db2.mark_as_initialized()
    /// });
    ///
    /// db.wait_until_initialized();
    /// ```
    pub fn wait_until_initialized(&self) -> Result<(), InitializationError> {
        let &(ref lock, ref cvar) = &self.initialized;
        let mut initialized = lock.lock().unwrap();
        while !*initialized {
            initialized = cvar.wait(initialized).unwrap()
        }
        let valid = self.valid.lock().unwrap();
        if *valid {
            Ok(())
        } else {
            Err(InitializationError::new(&self.get_status()))
        }
    }

    /// Set status message of the `SignalDB`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalDB;
    /// let mut db = SignalDB::new();
    ///
    /// db.set_status("Hello, World!");
    /// assert_eq!(db.get_status(), "Hello, World!");
    /// ```
    pub fn set_status(&self, new_status: &str) {
        let mut status = self.status.lock().unwrap();
        status.clear();
        status.push_str(new_status);
    }

    /// Get status message of the `SignalDB`
    pub fn get_status(&self) -> String {
        let status = self.status.lock().unwrap();
        status.clone()
    }

    /// Create a scope in the `SignalDB`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalDB;
    /// let mut db = SignalDB::new();
    ///
    /// db.create_scope(&vec!["foo", "bar"])
    /// ```
    pub fn create_scope(&self, path: &[&str]) {
        let mut scope = self.scope.lock().unwrap();
        scope.add_scope(path);
    }

    /// Add a new signal in the ``SignalDB``.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal)
    /// ```
    pub fn declare_signal(&self, scope: &[&str], signal: Signal) {
        let signal_id = signal.id.clone();
        {
            let mut signals = self.signals.lock().unwrap();
            signals.insert(signal.id.clone(), signal);
        }
        {
            let mut scopes = self.scope.lock().unwrap();
            match scopes.get_scope_by_path(scope) {
                Some(scope) => scope.add_signal(signal_id),
                None => panic!("Scope {:?} is not defined", scope),
            }
        }
    }

    /// Insert an event in the specified signal.
    ///
    /// # Examples
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB, SignalValue, Timestamp};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    ///
    /// db.insert_event("0", Timestamp::new(42), SignalValue::new(0));
    /// ```
    pub fn insert_event(
        &self,
        signal_id: &str,
        timestamp: Timestamp,
        new_value: SignalValue,
    ) -> Result<(), SignalNotFound> {
        self.set_time(timestamp);
        let mut signals = self.signals.lock().unwrap();
        signals
            .get_mut(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .add_event(timestamp, new_value);
        Ok(())
    }

    /// Set the current time of the `SignalDB`
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB, SignalValue, Timestamp};
    /// let mut db = SignalDB::new();
    /// db.set_time(Timestamp::new(42));
    /// ```
    pub fn set_time(&self, timestamp: Timestamp) {
        let mut now = self.now.lock().unwrap();
        if timestamp > *now {
            let mut timestamps = self.timestamps.lock().unwrap();
            timestamps.push(timestamp);
            *now = timestamp;
        };
    }

    /// Set the value of a signal at the current time.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB, SignalValue, Timestamp};
    /// let mut db = SignalDB::new();
    /// db.set_time(Timestamp::new(42));
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    ///
    /// db.set_time(Timestamp::new(42));
    /// db.set_current_value("0", SignalValue::new(0));
    /// ```
    pub fn set_current_value(
        &self,
        signal_id: &str,
        new_value: SignalValue,
    ) -> Result<(), SignalNotFound> {
        let t = {
            let now = self.now.lock().unwrap();
            *now
        };
        self.insert_event(signal_id, t, new_value)
    }

    /// Return all the timestamp where an event has been reported.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB, SignalValue, Timestamp};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    /// db.insert_event("0", Timestamp::new(42), SignalValue::new(0x1337));
    /// db.insert_event("0", Timestamp::new(43), SignalValue::new(0x1338));
    /// db.insert_event("0", Timestamp::new(44), SignalValue::new(0x1339));
    ///
    /// let mut events = db.get_timestamps();
    /// assert_eq!(events.next().unwrap(), Timestamp::new(0));
    /// assert_eq!(events.next().unwrap(), Timestamp::new(42));
    /// assert_eq!(events.next().unwrap(), Timestamp::new(43));
    /// assert_eq!(events.next().unwrap(), Timestamp::new(44));
    /// ```
    pub fn get_timestamps(&self) -> EventIterator {
        EventIterator::new(self)
    }

    /// Return value of a signal at a given time.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB, SignalValue, BitValue, Timestamp};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    /// db.insert_event("0", Timestamp::new(42), SignalValue::new(0x1337));
    ///
    /// assert_eq!(
    ///     db.value_at("0", Timestamp::new(41)).unwrap(),
    ///     SignalValue::new_default(32, BitValue::Undefined)
    /// );
    /// assert_eq!(db.value_at("0", Timestamp::new(42)).unwrap(), SignalValue::new(0x1337));
    /// assert_eq!(db.value_at("0", Timestamp::new(43)).unwrap(), SignalValue::new(0x1337));
    /// ```
    pub fn value_at(
        &self,
        signal_id: &str,
        timestamp: Timestamp,
    ) -> Result<SignalValue, SignalNotFound> {
        let signals = self.signals.lock().unwrap();
        Ok(signals
            .get(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .value_at(timestamp))
    }

    /// Return event of a signal reported at a given time.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB, SignalValue, BitValue, Timestamp};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    /// db.insert_event("0", Timestamp::new(42), SignalValue::new(0x1337));
    ///
    /// assert_eq!(db.event_at("0", Timestamp::new(41)).unwrap().is_none(), true);
    /// assert_eq!(
    ///     db.event_at("0", Timestamp::new(42)).unwrap().unwrap(),
    ///     SignalValue::new(0x1337)
    /// );
    /// assert_eq!(db.event_at("0", Timestamp::new(43)).unwrap().is_none(), true);
    /// ```
    pub fn event_at(
        &self,
        signal_id: &str,
        timestamp: Timestamp,
    ) -> Result<Option<SignalValue>, SignalNotFound> {
        let signals = self.signals.lock().unwrap();
        Ok(signals
            .get(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .event_at(timestamp))
    }

    /// Get fullname of a signal.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB, SignalValue, BitValue, Timestamp};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    ///
    /// assert_eq!(db.get_signal_fullname("0").unwrap(), "baz");
    /// ```
    pub fn get_signal_fullname(&self, signal_id: &str) -> Result<String, SignalNotFound> {
        let signals = self.signals.lock().unwrap();
        Ok(signals
            .get(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .name
            .to_string())
    }

    /// Check that a signal exists in the `SignalDB`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    ///
    /// assert_eq!(db.signal_exists("0"), true);
    /// assert_eq!(db.signal_exists("1"), false);
    /// ```
    pub fn signal_exists(&self, signal_id: &str) -> bool {
        let signals = self.signals.lock().unwrap();
        signals.get(signal_id).is_some()
    }

    /// Search signals in the `SignalDB` that fulfill a given predicate.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    ///
    /// assert_eq!(db.find_signals(|s| s.name == "baz"), vec!["0"]);
    /// assert_eq!(db.find_signals(|s| s.name == "").len(), 0);
    /// ```
    pub fn find_signals<F>(&self, f: F) -> Vec<String>
    where
        F: Fn(&Signal) -> bool,
    {
        let mut matches = Vec::new();
        let signals = self.signals.lock().unwrap();
        for (key, signal) in signals.iter() {
            if f(signal) {
                matches.push(key.clone())
            }
        }
        matches
    }

    /// Return a vector of all the signal IDs registered in the `SignalDB`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    ///
    /// assert_eq!(db.get_signal_ids(), vec!["0"]);
    /// ```
    pub fn get_signal_ids(&self) -> Vec<String> {
        self.find_signals(|_| true)
    }

    /// Get the timestamp of the next rising edge of a given signal.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{SignalDB, Timestamp};
    /// let buf = std::io::Cursor::new("$scope module top $end
    /// $var wire 1 0 foo $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// $end
    /// #1337
    /// 10
    /// #1338
    /// 00
    /// ");
    ///
    /// let db = SignalDB::from_vcd(buf).unwrap();
    /// assert_eq!(
    ///     db.get_next_rising_edge("0", Timestamp::new(0)).unwrap().unwrap(),
    ///     Timestamp::new(1337)
    /// );
    /// assert_eq!(
    ///     db.get_next_rising_edge("0", Timestamp::new(1338)).unwrap().is_none(),
    ///     true
    /// );
    /// assert_eq!(
    ///     db.get_previous_rising_edge("0", Timestamp::new(1338)).unwrap().unwrap(),
    ///     Timestamp::new(1337)
    /// );
    /// assert_eq!(
    ///     db.get_previous_rising_edge("0", Timestamp::new(2)).unwrap().is_none(),
    ///     true
    /// );
    /// ```
    pub fn get_next_rising_edge(
        &self,
        signal_id: &str,
        timestamp: Timestamp,
    ) -> Result<Option<Timestamp>, SignalNotFound> {
        let signals = self.signals.lock().unwrap();
        Ok(signals
            .get(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .get_next_rising_edge(timestamp))
    }

    /// Get the timestamp of the previous rising edge of a given signal.
    ///
    /// # Example
    ///
    /// See [`get_next_rising_edge`].
    ///
    /// [`get_next_rising_edge`]: #method.get_next_rising_edge
    pub fn get_previous_rising_edge(
        &self,
        signal_id: &str,
        timestamp: Timestamp,
    ) -> Result<Option<Timestamp>, SignalNotFound> {
        let signals = self.signals.lock().unwrap();
        Ok(signals
            .get(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .get_previous_rising_edge(timestamp))
    }

    /// Get the timestamp of the next falling edge of a given signal.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{SignalDB, Timestamp};
    /// let buf = std::io::Cursor::new("$scope module top $end
    /// $var wire 1 0 foo $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// $end
    /// #1337
    /// 10
    /// #1338
    /// 00
    /// ");
    ///
    /// let db = SignalDB::from_vcd(buf).unwrap();
    /// assert_eq!(
    ///     db.get_next_falling_edge("0", Timestamp::new(0)).unwrap().unwrap(),
    ///     Timestamp::new(1338)
    /// );
    /// ```
    pub fn get_next_falling_edge(
        &self,
        signal_id: &str,
        timestamp: Timestamp,
    ) -> Result<Option<Timestamp>, SignalNotFound> {
        let signals = self.signals.lock().unwrap();
        Ok(signals
            .get(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .get_next_falling_edge(timestamp))
    }

    /// Get the timestamp of the first event of the signal.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{SignalDB, Timestamp};
    /// let buf = std::io::Cursor::new("$scope module top $end
    /// $var wire 1 0 foo $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// $end
    /// #1337
    /// 10
    /// #1338
    /// 00
    /// ");
    ///
    /// let db = SignalDB::from_vcd(buf).unwrap();
    /// assert_eq!(db.get_first_event("0").unwrap().unwrap(), Timestamp::new(0));
    /// assert_eq!(db.get_last_event("0").unwrap().unwrap(), Timestamp::new(1338));
    /// ```
    pub fn get_first_event(&self, signal_id: &str) -> Result<Option<Timestamp>, SignalNotFound> {
        let signals = self.signals.lock().unwrap();
        Ok(signals
            .get(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .get_first_event())
    }

    /// Get the timestamp of the last event of the signal.
    ///
    /// # Example
    ///
    /// See [`get_first_event`].
    ///
    /// [`get_first_event`]: #method.get_first_event
    pub fn get_last_event(&self, signal_id: &str) -> Result<Option<Timestamp>, SignalNotFound> {
        let signals = self.signals.lock().unwrap();
        Ok(signals
            .get(signal_id)
            .ok_or_else(|| SignalNotFound::new(signal_id))?
            .get_last_event())
    }

    /// Get an overview of the events of a signal during a given time period.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{SignalDB, SignalValue, Timestamp};
    /// let buf = std::io::Cursor::new("$scope module top $end
    /// $var wire 1 0 foo $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// $end
    /// #1337
    /// 10
    /// #1339
    /// 00
    /// #1340
    /// b101010 0
    /// ");
    ///
    /// let db = SignalDB::from_vcd(buf).unwrap();
    /// assert_eq!(
    ///     db.events_between("0", Timestamp::new(1), Timestamp::new(1340)).unwrap(),
    ///     (SignalValue::new(0), 2, SignalValue::new(0))
    /// );
    /// assert_eq!(
    ///     db.events_between("0", Timestamp::new(1), Timestamp::new(1339)).unwrap(),
    ///     (SignalValue::new(0), 1, SignalValue::new(1))
    /// );
    /// ```
    pub fn events_between(
        &self,
        signal_id: &str,
        begin: Timestamp,
        end: Timestamp,
    ) -> Result<(SignalValue, usize, SignalValue), SignalNotFound> {
        let is_unknown = {
            let now = self.now.lock().unwrap();
            end > *now
        };

        if is_unknown {
            Ok((SignalValue::invalid(), 0, SignalValue::invalid()))
        } else {
            let signals = self.signals.lock().unwrap();
            Ok(signals
                .get(signal_id)
                .ok_or_else(|| SignalNotFound::new(signal_id))?
                .events_between(begin, end))
        }
    }

    /// Search in the database and format the result in `output`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalDB;
    /// let vcd = std::io::Cursor::new("$scope module top $end
    /// $var wire 1 0 foo $end
    /// $upscope $end
    /// $enddefinitions $end
    /// #0
    /// $dumpvars
    /// b0 0
    /// $end
    /// #1337
    /// 10
    /// #1338
    /// 00
    /// ");
    ///
    /// let mut db = SignalDB::from_vcd(vcd).unwrap();
    /// let mut buf = Vec::new();
    /// db.search_all(&mut buf, "$0 = 1").expect("Invalid search expression");
    /// assert_eq!(
    ///     String::from_utf8(buf).unwrap(),
    ///     "1337-1338\n"
    /// );
    ///
    /// let mut buf = Vec::new();
    /// db.search_all(&mut buf, "$0 <- 1").expect("Invalid search expression");
    /// assert_eq!(
    ///     String::from_utf8(buf).unwrap(),
    ///     "1337\n"
    /// );
    /// ```
    pub fn search_all<'a>(
        &mut self,
        output: &mut dyn io::Write,
        expr: &'a str,
    ) -> Result<(), Box<dyn Error>> {
        let mut search = Search::new(expr)?;
        search.search_all(self)?;
        search.format_findings(output);
        Ok(())
    }

    /// Search in the `SignalDB`. The result of the search have to be retrieved with the functions
    /// defined below.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{FindingsSummary, SignalDB, Timestamp};
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
    /// #43
    /// b1 #
    /// #1337
    /// b0 #
    /// #1338
    /// b1 #
    /// ");
    ///
    /// let mut db = SignalDB::from_vcd(vcd).unwrap();
    ///
    /// // Search expression
    /// let expr = "$# = 0";
    ///
    /// // Synchronous search
    /// db.search(expr);
    ///
    /// assert_eq!(db.get_first_finding(expr).unwrap().unwrap(), Timestamp::new(42));
    /// assert_eq!(db.get_last_finding(expr).unwrap().unwrap(), Timestamp::new(1338));
    /// assert_eq!(
    ///     db.get_next_finding(expr, Timestamp::new(43)).unwrap().unwrap(),
    ///     Timestamp::new(1337)
    /// );
    /// assert_eq!(
    ///     db.get_end_of_next_finding(expr, Timestamp::new(43)).unwrap().unwrap(),
    ///     Timestamp::new(1338)
    /// );
    /// assert_eq!(
    ///     db.get_previous_finding(expr, Timestamp::new(43)).unwrap().unwrap(),
    ///     Timestamp::new(42)
    /// );
    /// assert_eq!(
    ///     db.findings_between(expr, Timestamp::new(0), Timestamp::new(1339)).unwrap(),
    ///     FindingsSummary::Complex(2)
    /// );
    /// ```
    pub fn search<'a>(&self, expr: &'a str) -> Result<(), Box<dyn Error>> {
        let mut search = Search::new(expr)?;
        search.search_all(self)?;
        {
            let mut searches = self.searches.lock().unwrap();
            searches.insert(expr.to_string(), search);
        }
        Ok(())
    }

    /// Allocate a new search object in the `SignalDB` but don't perform actual search.
    /// This is meant to be used for asynchronous searches (see `AsyncSignalDB`)
    pub fn search_init<'a>(&self, expr: &'a str) -> Result<(), Box<dyn Error>> {
        let mut searches = self.searches.lock().unwrap();
        let search = Search::new(expr)?;
        searches.insert(expr.to_string(), search);
        Ok(())
    }

    /// Check if an expression is valid at a given point of time.
    /// The expression must first be reported to the `SignalDB` using the `search_init` function.
    /// This is meant to be used for asynchronous searches (see `AsyncSignalDB`)
    pub fn search_at(&self, expr: &str, timestamp: Timestamp) -> Result<(), Box<dyn Error>> {
        let mut searches = self.searches.lock().unwrap();
        Ok(searches
            .get_mut(expr)
            .ok_or_else(|| SearchNotFound::new(expr))?
            .search_at(self, timestamp)?)
    }

    /// Indicate that a search object isn't active anymore.
    /// This is meant to be used for asynchronous searches (see `AsyncSignalDB`)
    pub fn finish_search(&self, expr: &str) -> Result<(), Box<dyn Error>> {
        let mut searches = self.searches.lock().unwrap();
        searches
            .get_mut(expr)
            .ok_or_else(|| SearchNotFound::new(expr))?
            .finish();
        Ok(())
    }

    /// Get summary of findings within a time period.
    ///
    /// # Example
    ///
    /// See [`search`].
    ///
    /// [`search`]: #method.search
    pub fn findings_between(
        &self,
        expr: &str,
        begin: Timestamp,
        end: Timestamp,
    ) -> Result<FindingsSummary, SearchNotFound> {
        let searches = self.searches.lock().unwrap();
        Ok(searches
            .get(expr)
            .ok_or_else(|| SearchNotFound::new(expr))?
            .findings_between(begin, end))
    }

    /// Get next finding of a given search expression.
    ///
    /// # Example
    ///
    /// See [`search`].
    ///
    /// [`search`]: #method.search
    pub fn get_next_finding(
        &self,
        search_expr: &str,
        timestamp: Timestamp,
    ) -> Result<Option<Timestamp>, SearchNotFound> {
        let searches = self.searches.lock().unwrap();
        Ok(searches
            .get(search_expr)
            .ok_or_else(|| SearchNotFound::new(search_expr))?
            .get_next_finding(timestamp))
    }

    /// Get the timestamp where the search expression is not true anymore.
    ///
    /// # Example
    ///
    /// See [`search`].
    ///
    /// [`search`]: #method.search
    pub fn get_end_of_next_finding(
        &self,
        search_expr: &str,
        timestamp: Timestamp,
    ) -> Result<Option<Timestamp>, SearchNotFound> {
        let searches = self.searches.lock().unwrap();
        Ok(searches
            .get(search_expr)
            .ok_or_else(|| SearchNotFound::new(search_expr))?
            .get_end_of_next_finding(timestamp))
    }

    /// Get the previous finding of the given search expression.
    ///
    /// # Example
    ///
    /// See [`search`].
    ///
    /// [`search`]: #method.search
    pub fn get_previous_finding(
        &self,
        search_expr: &str,
        timestamp: Timestamp,
    ) -> Result<Option<Timestamp>, SearchNotFound> {
        let searches = self.searches.lock().unwrap();
        Ok(searches
            .get(search_expr)
            .ok_or_else(|| SearchNotFound::new(search_expr))?
            .get_previous_finding(timestamp))
    }

    /// Get the first finding of the given search expression.
    ///
    /// # Example
    ///
    /// See [`search`].
    ///
    /// [`search`]: #method.search
    pub fn get_first_finding(&self, expr: &str) -> Result<Option<Timestamp>, SearchNotFound> {
        let searches = self.searches.lock().unwrap();
        Ok(searches
            .get(expr)
            .ok_or_else(|| SearchNotFound::new(expr))?
            .get_first_finding())
    }

    /// Get the last finding of the given search expression.
    ///
    /// # Example
    ///
    /// See [`search`].
    ///
    /// [`search`]: #method.search
    pub fn get_last_finding(&self, expr: &str) -> Result<Option<Timestamp>, SearchNotFound> {
        let searches = self.searches.lock().unwrap();
        Ok(searches
            .get(expr)
            .ok_or_else(|| SearchNotFound::new(expr))?
            .get_last_finding())
    }

    /// Format some stats of the `SignalDB`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    ///
    /// let mut buf = Vec::new();
    /// db.format_stats(&mut buf);
    /// assert_eq!(
    ///     String::from_utf8(buf).unwrap(),
    ///     "# foo\n#   bar\n#     0 (baz) - width: 32, edges: 0\n"
    /// )
    /// ```
    pub fn format_stats(&self, output: &mut dyn io::Write) {
        let scope = self.scope.lock().unwrap();
        let signals = self.signals.lock().unwrap();
        scope.traverse(&mut |name, node: &ScopeChild, depth| {
            let _ = write!(output, "# ");
            for _ in 0..depth {
                let _ = write!(output, "  ");
            }
            match node {
                ScopeChild::Signal => signals.get(name).unwrap().format_stats(output),
                ScopeChild::Scope(scope) => {
                    let _ = writeln!(output, "{}", scope.name);
                }
            }
        })
    }

    /// Format the value of the signals at a given time.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalDB, SignalValue, Timestamp};
    /// let mut db = SignalDB::new();
    ///
    /// let scope = &vec!["foo", "bar"];
    /// db.create_scope(&scope);
    ///
    /// let signal = Signal::new("0", "baz", 32);
    /// db.declare_signal(&scope, signal);
    /// db.insert_event("0", Timestamp::new(42), SignalValue::new(0x1337));
    ///
    /// let mut buf = Vec::new();
    /// db.format_values_at(&mut buf, Timestamp::new(43));
    /// assert_eq!(
    ///     String::from_utf8(buf).unwrap(),
    ///     "foo\n  bar\n    0 (baz) = h00001337\n"
    /// )
    /// ```
    pub fn format_values_at(&self, output: &mut dyn io::Write, timestamp: Timestamp) {
        let scope = self.scope.lock().unwrap();
        let signals = self.signals.lock().unwrap();
        scope.traverse(&mut |name, node: &ScopeChild, depth| {
            for _ in 0..depth {
                let _ = write!(output, "  ");
            }
            match node {
                ScopeChild::Signal => signals
                    .get(name)
                    .unwrap()
                    .format_value_at(output, timestamp),
                ScopeChild::Scope(scope) => {
                    let _ = writeln!(output, "{}", scope.name);
                }
            }
        })
    }
}
