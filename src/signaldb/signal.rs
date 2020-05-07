// SPDX-License-Identifier: MIT
use super::time::Timestamp;
use super::value::{BitValue, SignalValue};
use std::fmt;
use std::io;

#[derive(Debug)]
struct Event {
    timestamp: Timestamp,
    new_value: SignalValue,
}

/// Representation of a single signal
pub struct Signal {
    /// Identifier of the signal
    pub id: String,
    /// Full name of the signal
    pub name: String,
    /// Width of the signal in bits
    pub width: usize,
    events: Vec<Event>,
    default: SignalValue,
}

impl Signal {
    /// Create a new `Signal`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::Signal;
    /// let signal = Signal::new("0", "foo", 32);
    /// assert_eq!(signal.id, "0");
    /// assert_eq!(signal.name, "foo");
    /// assert_eq!(signal.width, 32);
    /// ```
    pub fn new(id: &str, name: &str, width: usize) -> Signal {
        Signal {
            id: id.to_string(),
            name: name.to_string(),
            width,
            events: Vec::new(),
            default: SignalValue::new_default(width, BitValue::Undefined),
        }
    }

    fn prev_value_at_index(&self, index: usize) -> &SignalValue {
        if index == 0 {
            &self.default
        } else if index >= self.events.len() {
            match self.events.last() {
                Some(event) => &event.new_value,
                None => &self.default,
            }
        } else {
            &self.events[index - 1].new_value
        }
    }

    /// Add an event in the `Signal`
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalValue, Timestamp};
    /// let mut signal = Signal::new("0", "foo", 32);
    /// signal.add_event(Timestamp::new(42), SignalValue::new(1337));
    /// ```
    pub fn add_event(&mut self, timestamp: Timestamp, mut new_value: SignalValue) {
        new_value.expand(self.width);
        let seek = match self.events.last() {
            Some(e) => {
                if e.timestamp < timestamp {
                    Err(self.events.len())
                } else {
                    self.events
                        .binary_search_by_key(&timestamp, |e| e.timestamp)
                }
            }
            None => Err(0),
        };

        match seek {
            Ok(index) => {
                if *self.prev_value_at_index(index) == new_value {
                    let _ = self.events.remove(index);
                } else {
                    self.events[index].new_value = new_value
                }
            }
            Err(index) => {
                if *self.prev_value_at_index(index) != new_value {
                    self.events.insert(
                        index,
                        Event {
                            timestamp,
                            new_value,
                        },
                    )
                }
            }
        }
    }

    /// Get value of the `Signal` at a given time.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalValue, Timestamp};
    /// let mut signal = Signal::new("0", "foo", 32);
    /// signal.add_event(Timestamp::new(42), SignalValue::new(1337));
    /// assert_eq!(signal.value_at(Timestamp::new(43)), SignalValue::new(1337));
    /// ```
    pub fn value_at(&self, timestamp: Timestamp) -> SignalValue {
        let seek = self
            .events
            .binary_search_by_key(&timestamp, |e| e.timestamp);

        match seek {
            Ok(index) => self.events[index].new_value.clone(),
            Err(index) => self.prev_value_at_index(index).clone(),
        }
    }

    /// Get event of the `Signal` reported at a given time.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalValue, Timestamp};
    /// let mut signal = Signal::new("0", "foo", 32);
    /// signal.add_event(Timestamp::new(42), SignalValue::new(1337));
    /// assert_eq!(signal.event_at(Timestamp::new(42)).unwrap(), SignalValue::new(1337));
    /// assert_eq!(signal.event_at(Timestamp::new(43)).is_none(), true);
    /// ```
    pub fn event_at(&self, timestamp: Timestamp) -> Option<SignalValue> {
        let seek = self
            .events
            .binary_search_by_key(&timestamp, |e| e.timestamp);

        match seek {
            Ok(index) => Some(self.events[index].new_value.clone()),
            Err(_) => None,
        }
    }

    fn index_of(&self, timestamp: Timestamp) -> usize {
        let seek = self
            .events
            .binary_search_by_key(&timestamp, |e| e.timestamp);

        match seek {
            Ok(index) => index,
            Err(index) => index,
        }
    }

    /// Get summary of the events for a time period.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalValue, Timestamp};
    /// let mut signal = Signal::new("0", "foo", 32);
    /// signal.add_event(Timestamp::new(0), SignalValue::new(0));
    /// signal.add_event(Timestamp::new(42), SignalValue::new(1337));
    /// signal.add_event(Timestamp::new(43), SignalValue::new(1338));
    /// assert_eq!(
    ///     signal.events_between(Timestamp::new(40), Timestamp::new(45)),
    ///     (SignalValue::new(0), 2, SignalValue::new(1338))
    /// )
    /// ```
    pub fn events_between(
        &self,
        begin: Timestamp,
        end: Timestamp,
    ) -> (SignalValue, usize, SignalValue) {
        let begin_index = self.index_of(begin);
        let end_index = self.index_of(end);
        (
            self.prev_value_at_index(begin_index).clone(),
            end_index - begin_index,
            self.prev_value_at_index(end_index).clone(),
        )
    }

    /// Get the timestamp of the next rising edge.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalValue, Timestamp};
    /// let mut signal = Signal::new("0", "foo", 1);
    /// signal.add_event(Timestamp::new(0), SignalValue::new(0));
    /// signal.add_event(Timestamp::new(42), SignalValue::new(1));
    /// signal.add_event(Timestamp::new(43), SignalValue::new(0));
    ///
    /// assert_eq!(signal.get_next_rising_edge(Timestamp::new(40)).unwrap(), Timestamp::new(42));
    /// assert_eq!(signal.get_next_rising_edge(Timestamp::new(43)).is_none(), true);
    ///
    /// assert_eq!(signal.get_previous_rising_edge(
    ///     Timestamp::new(44)).unwrap(), Timestamp::new(42)
    /// );
    /// assert_eq!(signal.get_previous_rising_edge(Timestamp::new(40)).is_none(), true);
    ///
    /// assert_eq!(signal.get_next_falling_edge(Timestamp::new(40)).unwrap(), Timestamp::new(43));
    /// assert_eq!(signal.get_next_falling_edge(Timestamp::new(44)).is_none(), true);
    ///
    /// assert_eq!(signal.get_first_event().unwrap(), Timestamp::new(0));
    /// assert_eq!(signal.get_last_event().unwrap(), Timestamp::new(43));
    /// ```
    pub fn get_next_rising_edge(&self, timestamp: Timestamp) -> Option<Timestamp> {
        let start = self.index_of(Timestamp::new(timestamp.get_value() + 1));
        let zero = SignalValue::new(0);
        for evt in &self.events[start..] {
            if evt.new_value != zero {
                return Some(evt.timestamp);
            }
        }
        None
    }

    /// Get the timestamp of the next falling edge.
    ///
    /// # Example
    ///
    /// See [`get_next_rising_edge`].
    ///
    /// [`get_next_rising_edge`]: #method.get_next_rising_edge
    pub fn get_next_falling_edge(&self, timestamp: Timestamp) -> Option<Timestamp> {
        let start = self.index_of(Timestamp::new(timestamp.get_value() + 1));
        let zero = SignalValue::new(0);
        for evt in &self.events[start..] {
            if evt.new_value == zero {
                return Some(evt.timestamp);
            }
        }
        None
    }

    /// Get the timestamp of the previous rising edge.
    ///
    /// # Example
    ///
    /// See [`get_next_rising_edge`].
    ///
    /// [`get_next_rising_edge`]: #method.get_next_rising_edge
    pub fn get_previous_rising_edge(&self, timestamp: Timestamp) -> Option<Timestamp> {
        let end = self.index_of(Timestamp::new(timestamp.get_value()));
        let zero = SignalValue::new(0);
        for evt in self.events[0..end].iter().rev() {
            if evt.new_value != zero {
                return Some(evt.timestamp);
            }
        }
        None
    }

    /// Get the timestamp of the first event.
    ///
    /// # Example
    ///
    /// See [`get_next_rising_edge`].
    ///
    /// [`get_next_rising_edge`]: #method.get_next_rising_edge
    pub fn get_first_event(&self) -> Option<Timestamp> {
        self.events.first().map(|evt| evt.timestamp)
    }

    /// Get the timestamp of the last event.
    ///
    /// # Example
    ///
    /// See [`get_next_rising_edge`].
    ///
    /// [`get_next_rising_edge`]: #method.get_next_rising_edge
    pub fn get_last_event(&self) -> Option<Timestamp> {
        self.events.last().map(|evt| evt.timestamp)
    }

    /// Format some stats of the signal.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalValue, Timestamp};
    /// let mut signal = Signal::new("0", "foo", 32);
    /// signal.add_event(Timestamp::new(42), SignalValue::new(1337));
    /// signal.add_event(Timestamp::new(43), SignalValue::new(1338));
    ///
    /// let mut buf = Vec::new();
    /// signal.format_stats(&mut buf);
    /// assert_eq!(
    ///     String::from_utf8(buf).unwrap(),
    ///     "0 (foo) - width: 32, edges: 2, from: 42, to: 43\n"
    /// )
    /// ```
    pub fn format_stats(&self, output: &mut dyn io::Write) {
        let _ = write!(
            output,
            "{} - width: {}, edges: {}",
            self,
            self.width,
            self.events.len()
        );
        if let (Some(first), Some(last)) = (self.events.first(), self.events.last()) {
            let _ = writeln!(
                output,
                ", from: {}, to: {}",
                first.timestamp, last.timestamp
            );
        } else {
            let _ = writeln!(output);
        }
    }

    /// Format the value of a signal at a given time.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Signal, SignalValue, Timestamp};
    /// let mut signal = Signal::new("0", "foo", 32);
    /// signal.add_event(Timestamp::new(42), SignalValue::new(0x1337));
    ///
    /// let mut buf = Vec::new();
    /// signal.format_value_at(&mut buf, Timestamp::new(43));
    /// assert_eq!(
    ///     String::from_utf8(buf).unwrap(),
    ///     "0 (foo) = h00001337\n"
    /// )
    /// ```
    pub fn format_value_at(&self, output: &mut dyn io::Write, timestamp: Timestamp) {
        let (assign_symbol, value) = match self.event_at(timestamp) {
            Some(v) => ("->", v),
            None => ("=", self.value_at(timestamp)),
        };
        let _ = writeln!(output, "{} {} {}", self, assign_symbol, value);
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.id, self.name)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn add_events() {
        let mut s = Signal::new("t", "test", 32);
        s.add_event(Timestamp::new(42), SignalValue::new(0));
        s.add_event(Timestamp::new(43), SignalValue::new(1));
        assert_eq!(s.events[0].timestamp, Timestamp::new(42));
        assert_eq!(s.events[1].timestamp, Timestamp::new(43));

        s.add_event(Timestamp::new(44), SignalValue::new(1));
        assert_eq!(s.events.len(), 2);

        s.add_event(Timestamp::new(43), SignalValue::new(0));
        assert_eq!(s.events.len(), 1);
    }

    #[test]
    fn values() {
        let mut s = Signal::new("t", "test", 32);
        s.add_event(Timestamp::new(42), SignalValue::new(0));
        s.add_event(Timestamp::new(43), SignalValue::new(1));
        s.add_event(Timestamp::new(45), SignalValue::new(0));

        assert_eq!(
            s.value_at(Timestamp::new(41)),
            SignalValue::new_default(32, BitValue::Undefined)
        );
        assert_eq!(s.value_at(Timestamp::new(42)), SignalValue::new(0));
        assert_eq!(s.value_at(Timestamp::new(43)), SignalValue::new(1));
        assert_eq!(s.value_at(Timestamp::new(44)), SignalValue::new(1));
        assert_eq!(s.value_at(Timestamp::new(45)), SignalValue::new(0));
        assert_eq!(s.value_at(Timestamp::new(100)), SignalValue::new(0));
    }

    #[test]
    fn slices() {
        let mut s = Signal::new("t", "test", 32);
        s.add_event(Timestamp::new(42), SignalValue::new(0));
        s.add_event(Timestamp::new(43), SignalValue::new(1));
        s.add_event(Timestamp::new(45), SignalValue::new(0));

        let (_, big_slice, _) = s.events_between(Timestamp::new(0), Timestamp::new(100));
        assert_eq!(big_slice, 3);
        let (_, medium_slice, _) = s.events_between(Timestamp::new(0), Timestamp::new(44));
        assert_eq!(medium_slice, 2);
        let (_, small_slice, _) = s.events_between(Timestamp::new(0), Timestamp::new(43));
        assert_eq!(small_slice, 1);
        let (_, empty_slice, _) = s.events_between(Timestamp::new(0), Timestamp::new(10));
        assert_eq!(empty_slice, 0);
    }
}
