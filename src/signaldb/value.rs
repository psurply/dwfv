// SPDX-License-Identifier: MIT
use std::collections::VecDeque;
use std::fmt;
use std::str::FromStr;

/// Value a single bit
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BitValue {
    Low,
    High,
    HighZ,
    Invalid,
    Overflow,
    Undefined,
    Filtered,
}

struct NibbleValue([BitValue; 4]);

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ValueFormat {
    Hex,
    Bin
}

/// Value of a signal
#[derive(Debug, Clone)]
pub enum SignalValue {
    /// Concrete value of the signal
    Literal(Vec<BitValue>, ValueFormat),
    /// Symbolic value of the signal
    Symbol(String)
}

impl BitValue {
    /// Create a `BitValue` from a `char`.
    ///
    /// Example:
    ///
    /// ```
    /// use dwfv::signaldb::BitValue;
    /// let b = BitValue::from_char('0');
    /// assert_eq!(b, BitValue::Low);
    /// ```
    pub fn from_char(c: char) -> BitValue {
        match c {
            '0' => BitValue::Low,
            '1' => BitValue::High,
            'z' => BitValue::HighZ,
            'u' => BitValue::Undefined,
            '-' => BitValue::Overflow,
            'w' => BitValue::Filtered,
            _ => BitValue::Invalid,
        }
    }

    /// Return the `char` representation of a `BitValue`.
    ///
    /// Example:
    ///
    /// ```
    /// use dwfv::signaldb::BitValue;
    /// assert_eq!(BitValue::Low.to_char(), '0');
    /// ```
    pub fn to_char(self) -> char {
        match self {
            BitValue::Low => '0',
            BitValue::High => '1',
            BitValue::HighZ => 'z',
            BitValue::Undefined => 'u',
            BitValue::Overflow => '-',
            BitValue::Filtered => 'w',
            BitValue::Invalid => 'x',
        }
    }
}

impl NibbleValue {
    fn from_vec(v: &[BitValue]) -> Vec<NibbleValue> {
        let mut nibbles = Vec::new();
        let mut queue = VecDeque::from(v.to_owned());
        while !queue.is_empty() {
            nibbles.push(NibbleValue::pop_from(&mut queue))
        }
        nibbles
    }

    fn pop_from(q: &mut VecDeque<BitValue>) -> NibbleValue {
        let mut nibble = [BitValue::Low; 4];

        for i in &mut nibble {
            *i = q.pop_front().unwrap_or(BitValue::Low)
        }

        NibbleValue(nibble)
    }

    pub fn to_char(&self) -> char {
        let mut acc = 0;
        for i in self.0.iter().rev() {
            acc = (acc << 1) | {
                match i {
                    BitValue::Low => 0,
                    BitValue::High => 1,
                    b => return b.to_char()
                }
            }
        }
        "0123456789ABCDEF".chars().nth(acc).unwrap()
    }
}

impl FromStr for SignalValue {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut v = Vec::new();
        for c in s.chars() {
            v.push(BitValue::from_char(c))
        }
        v.reverse();
        Ok(SignalValue::Literal(v, ValueFormat::Hex))
    }
}

impl SignalValue {
    /// Create a new `SignalValue` from an integer.
    ///
    /// Example:
    ///
    /// ```
    /// use dwfv::signaldb::SignalValue;
    /// let mut value = SignalValue::new(0x42);
    /// ```
    pub fn new(mut value: u64) -> SignalValue {
        let mut v = Vec::new();
        while value != 0 {
            v.push(match value & 1 {
                0 => BitValue::Low,
                _ => BitValue::High,
            });
            value >>= 1;
        }
        SignalValue::Literal(v, ValueFormat::Hex)
    }

    /// Create a new `SignalValue` with the same `BitValue` for every bit.
    ///
    /// Example:
    ///
    /// ```
    /// use dwfv::signaldb::{BitValue, SignalValue};
    /// let mut value = SignalValue::new_default(16, BitValue::High);
    /// assert_eq!(value, SignalValue::new(0xFFFF));
    /// ```
    pub fn new_default(width: usize, value: BitValue) -> SignalValue {
        let mut v = Vec::new();
        for _ in 0..width {
            v.push(value)
        }
        SignalValue::Literal(v, ValueFormat::Hex)
    }

    /// Create a `SignalValue` from a symbol.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalValue;
    /// let v = SignalValue::from_symbol_str("foo");
    /// assert_eq!(v, SignalValue::Symbol("foo".to_string()));
    /// ```
    pub fn from_symbol_str(s: &str) -> SignalValue {
        SignalValue::Symbol(s.to_string())
    }

    /// Create a `SignalValue` from an hex string.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalValue;
    /// let v = SignalValue::from_hex("2A");
    /// assert_eq!(v, SignalValue::new(42));
    /// ```
    pub fn from_hex(s: &str) -> SignalValue {
        let mut value = 0;
        let chars = "0123456789abcdef";
        for (nibble, c) in s.chars().rev().enumerate() {
            if let Some(i) = chars.find(&c.to_lowercase().to_string()) {
                value |= i << (nibble * 4)
            }
        }
        SignalValue::new(value as u64)
    }

    /// Create an invalid `SignalValue`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalValue;
    /// let v = SignalValue::invalid();
    /// assert_eq!(v.is_invalid(), true);
    /// ```
    pub fn invalid() -> SignalValue {
        SignalValue::Literal(vec![BitValue::Undefined], ValueFormat::Hex)
    }

    /// Expand the width of a `SignalValue`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{BitValue, SignalValue};
    /// let mut v = SignalValue::new_default(16, BitValue::High);
    /// assert_eq!(v.width(), 16);
    /// v.expand(32);
    /// assert_eq!(v.width(), 32);
    /// assert_eq!(v, SignalValue::new(0xFFFF));
    /// ```
    pub fn expand(&mut self, width: usize) {
        if let SignalValue::Literal(literal, _) = self {
            let mut expand_value = *literal.get(0).unwrap_or(&BitValue::High);
            if expand_value == BitValue::High {
                expand_value = BitValue::Low
            }
            for _i in literal.len()..width {
                literal.push(expand_value)
            }
        }
    }

    /// Get the width of a `SignalValue`.
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::SignalValue;
    /// let v = SignalValue::new(42);
    /// assert_eq!(v.width(), 6);
    /// ```
    pub fn width(&self) -> usize {
        match self {
            SignalValue::Literal(literal, _) => literal.len(),
            SignalValue::Symbol(_) => 2
        }
    }

    /// Check if the `SignalValue` is invalid.
    ///
    /// # Example
    ///
    /// See [`invalid`].
    ///
    /// [`invalid`]: #method.invalid
    pub fn is_invalid(&self) -> bool {
        match self {
            SignalValue::Literal(literal, _) => {
                for b in literal {
                    match b {
                        BitValue::HighZ
                            | BitValue::Invalid
                            | BitValue::Overflow
                            | BitValue::Undefined
                            | BitValue::Filtered => return true,
                        _ => {}
                    }
                }
                false
            },
            SignalValue::Symbol(_) => false
        }
    }
}

impl fmt::Display for SignalValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignalValue::Literal(literal, value_format) => {
                match value_format {
                    ValueFormat::Bin => {
                        write!(f, "b")?;
                        for b in literal.iter().rev() {
                            write!(f, "{}", b.to_char())?;
                        }
                    },
                    ValueFormat::Hex => {
                        write!(f, "h")?;
                        for nibble in NibbleValue::from_vec(literal).iter().rev() {
                            write!(f, "{}", nibble.to_char())?
                        }
                    }
                }
                Ok(())
            },
            SignalValue::Symbol(symbol) => {
                write!(f, "{}", symbol)?;
                Ok(())
            }
        }
    }
}

impl PartialEq for SignalValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SignalValue::Literal(self_l, _), SignalValue::Literal(other_l, _)) => {
                for i in 0.. {
                    match (self_l.get(i), other_l.get(i)) {
                        (Some(l), Some(r)) if *l != *r => return false,
                        (Some(x), None) | (None, Some(x)) if *x != BitValue::Low => return false,
                        (None, None) => return true,
                        _ => continue,
                    }
                }
                false
            },
            (SignalValue::Symbol(self_s), SignalValue::Symbol(other_s)) => self_s == other_s,
            _ => false
        }
    }
}

impl Eq for SignalValue {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn signal_eq() {
        assert_eq!(SignalValue::new(0), SignalValue::from_str("000").unwrap());
        assert_eq!(SignalValue::new(42), SignalValue::from_str("000000101010").unwrap());
    }
}
