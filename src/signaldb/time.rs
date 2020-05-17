// SPDX-License-Identifier: MIT
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};
use std::str::FromStr;
use std::{convert, fmt};

const MAX_RESCALE: i64 = 1 << 50;

/// Time scale
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scale {
    Femtosecond,
    Picosecond,
    Nanosecond,
    Microsecond,
    Millisecond,
    Second,
}

/// Representation of a point in time
#[derive(Debug, Copy, Clone, Eq)]
pub struct Timestamp {
    pub value: i64,
    pub scale: Scale,
}

/// Description of a time period
#[derive(Debug, Copy, Clone)]
pub enum TimeDescr {
    /// Representation of a point in time
    Point(Timestamp),
    /// Representation of a period of time
    Period(Timestamp, Timestamp),
}

impl Scale {
    fn scale_down(self) -> Option<Scale> {
        match self {
            Scale::Second => Some(Scale::Millisecond),
            Scale::Millisecond => Some(Scale::Microsecond),
            Scale::Microsecond => Some(Scale::Nanosecond),
            Scale::Nanosecond => Some(Scale::Picosecond),
            Scale::Picosecond => Some(Scale::Femtosecond),
            Scale::Femtosecond => None,
        }
    }

    fn scale_up(self) -> Option<Scale> {
        match self {
            Scale::Second => None,
            Scale::Millisecond => Some(Scale::Second),
            Scale::Microsecond => Some(Scale::Millisecond),
            Scale::Nanosecond => Some(Scale::Microsecond),
            Scale::Picosecond => Some(Scale::Nanosecond),
            Scale::Femtosecond => Some(Scale::Picosecond),
        }
    }
}

impl convert::Into<i64> for Scale {
    fn into(self) -> i64 {
        match self {
            Scale::Second => 1000_0000_0000_0000,
            Scale::Millisecond => 1_0000_0000_0000,
            Scale::Microsecond => 10_0000_0000,
            Scale::Nanosecond => 100_0000,
            Scale::Picosecond => 1000,
            Scale::Femtosecond => 1,
        }
    }
}

impl fmt::Display for Scale {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Scale::Second => write!(f, "s"),
            Scale::Millisecond => write!(f, "ms"),
            Scale::Microsecond => write!(f, "us"),
            Scale::Nanosecond => write!(f, "ns"),
            Scale::Picosecond => write!(f, "ps"),
            Scale::Femtosecond => write!(f, "fs"),
        }
    }
}

impl FromStr for Scale {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "s" => Ok(Scale::Second),
            "ms" => Ok(Scale::Millisecond),
            "us" => Ok(Scale::Microsecond),
            "ns" => Ok(Scale::Nanosecond),
            "ps" => Ok(Scale::Picosecond),
            "fs" => Ok(Scale::Femtosecond),
            _ => Err(()),
        }
    }
}

impl Timestamp {
    pub fn new(value: i64, scale: Scale) -> Timestamp {
        Timestamp { value, scale }
    }

    pub fn origin() -> Timestamp {
        Timestamp {
            value: 0,
            scale: Scale::Second,
        }
    }

    fn rescale(self, scale: Scale) -> Timestamp {
        if scale == self.scale {
            return self;
        }

        let current_scale: i64 = self.scale.into();
        let new_scale: i64 = scale.into();
        let new_value = if current_scale > new_scale {
            let rescale = current_scale / new_scale;
            if rescale > MAX_RESCALE {
                return self;
            } else {
                self.value * rescale
            }
        } else {
            self.value / (new_scale / current_scale)
        };
        Timestamp::new(new_value, scale)
    }

    pub fn auto_rescale(&mut self, max_value: i64) -> bool {
        while self.value > max_value {
            if let Some(upscaled) = self.scale_up() {
                self.value = upscaled.value;
                self.scale = upscaled.scale;
            } else {
                return false;
            }
        }
        true
    }

    fn normalize(self, other: Timestamp) -> (Timestamp, Timestamp) {
        match self.scale.cmp(&other.scale) {
            Ordering::Less => (self, other.rescale(self.scale)),
            Ordering::Greater => (self.rescale(other.scale), other),
            Ordering::Equal => (self, other),
        }
    }

    fn scale_down(self) -> Option<Timestamp> {
        if let Some(new_scale) = self.scale.scale_down() {
            Some(self.rescale(new_scale))
        } else {
            None
        }
    }

    fn scale_up(self) -> Option<Timestamp> {
        if let Some(new_scale) = self.scale.scale_up() {
            Some(self.rescale(new_scale))
        } else {
            None
        }
    }

    pub fn derive(self, value: i64) -> Timestamp {
        Timestamp {
            value,
            scale: self.scale,
        }
    }
}

impl AddAssign for Timestamp {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other
    }
}

impl Add for Timestamp {
    type Output = Self;

    /// Add two timestamps
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Scale, Timestamp};
    /// let a = Timestamp::new(21, Scale::Second);
    /// let b = Timestamp::new(21, Scale::Second);
    /// assert_eq!(a + b, Timestamp::new(42, Scale::Second));
    ///
    /// let a = Timestamp::new(21, Scale::Second);
    /// let b = Timestamp::new(21, Scale::Microsecond);
    /// assert_eq!(a + b, Timestamp::new(21000021, Scale::Microsecond));
    /// ```
    fn add(self, other: Self) -> Self {
        let (a, b) = self.normalize(other);
        Self::new(a.value + b.value, a.scale)
    }
}

impl Sub for Timestamp {
    type Output = Self;

    /// Substract two timestamps
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Scale, Timestamp};
    /// let a = Timestamp::new(21, Scale::Millisecond);
    /// let b = Timestamp::new(21, Scale::Millisecond);
    /// assert_eq!(a - b, Timestamp::new(0, Scale::Millisecond));
    ///
    /// let a = Timestamp::new(21, Scale::Second);
    /// let b = Timestamp::new(21, Scale::Microsecond);
    /// assert_eq!(a - b, Timestamp::new(20999979, Scale::Microsecond));
    /// ```
    fn sub(self, other: Self) -> Self {
        let (a, b) = self.normalize(other);
        Self::new(a.value - b.value, a.scale)
    }
}

impl SubAssign for Timestamp {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl Div for Timestamp {
    type Output = usize;

    /// Divide two timestamps
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Scale, Timestamp};
    /// let a = Timestamp::new(42, Scale::Millisecond);
    /// let b = Timestamp::new(21, Scale::Millisecond);
    /// assert_eq!(a / b, 2);
    ///
    /// let a = Timestamp::new(2, Scale::Second);
    /// let b = Timestamp::new(1, Scale::Microsecond);
    /// assert_eq!(a / b, 2000000);
    fn div(self, rhs: Self) -> usize {
        let ts = self.normalize(rhs);
        (ts.0.value / ts.1.value) as usize
    }
}

impl MulAssign<i64> for Timestamp {
    fn mul_assign(&mut self, other: i64) {
        *self = *self * other;
    }
}

impl Mul<i64> for Timestamp {
    type Output = Self;

    /// Multiply a timestamp
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Scale, Timestamp};
    /// let timestamp = Timestamp::new(500, Scale::Millisecond);
    /// assert_eq!(timestamp * 2, Timestamp::new(1, Scale::Second));
    fn mul(self, rhs: i64) -> Self {
        Timestamp::new(self.value * rhs, self.scale)
    }
}

impl DivAssign<i64> for Timestamp {
    fn div_assign(&mut self, other: i64) {
        *self = *self / other;
    }
}

impl Div<i64> for Timestamp {
    type Output = Self;

    /// Divide a timestamp
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Scale, Timestamp};
    /// let timestamp = Timestamp::new(42, Scale::Millisecond);
    /// assert_eq!(timestamp / 2, Timestamp::new(21, Scale::Millisecond));
    ///
    /// let timestamp = Timestamp::new(1, Scale::Second);
    /// assert_eq!(timestamp / 2, Timestamp::new(500, Scale::Millisecond));
    ///
    /// let timestamp = Timestamp::new(1, Scale::Femtosecond);
    /// assert_eq!(timestamp / 2, Timestamp::new(1, Scale::Femtosecond));
    fn div(self, rhs: i64) -> Self {
        let mut current = self;
        let mut result = Timestamp::new(self.value / rhs, current.scale);

        while result.value < 1 {
            if let Some(downscaled) = current.scale_down() {
                result.value = downscaled.value / rhs;
                result.scale = downscaled.scale;
                current = downscaled
            } else {
                return self;
            }
        }

        result
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Timestamp) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamp {
    /// Compare two timestamps
    ///
    /// # Example
    ///
    /// ```
    /// use dwfv::signaldb::{Scale, Timestamp};
    /// let a = Timestamp::new(1, Scale::Second);
    /// let b = Timestamp::new(1000, Scale::Millisecond);
    /// assert!(a == b);
    ///
    /// let a = Timestamp::new(1, Scale::Second);
    /// let b = Timestamp::new(999, Scale::Millisecond);
    /// assert!(a > b);
    ///
    /// let a = Timestamp::new(1, Scale::Second);
    /// let b = Timestamp::new(1001, Scale::Millisecond);
    /// assert!(a < b);
    fn cmp(&self, other: &Timestamp) -> Ordering {
        let (a, b) = self.normalize(*other);
        if a.scale == b.scale {
            a.value.cmp(&b.value)
        } else {
            a.scale.cmp(&b.scale)
        }
    }
}

impl PartialEq for Timestamp {
    fn eq(&self, other: &Timestamp) -> bool {
        let (a, b) = self.normalize(*other);
        a.value == b.value
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.scale)
    }
}

impl fmt::Display for TimeDescr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeDescr::Point(t) => write!(f, "{}", t),
            TimeDescr::Period(begin, end) => write!(f, "{}-{}", begin, end),
        }
    }
}
