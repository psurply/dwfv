// SPDX-License-Identifier: MIT
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Representation of a point in time
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(i64);

/// Description of a time period
#[derive(Debug, Copy, Clone)]
pub enum TimeDescr {
    /// Representation of a point in time
    Point(Timestamp),
    /// Representation of a period of time
    Period(Timestamp, Timestamp),
}

impl Timestamp {
    pub fn new(value: i64) -> Timestamp {
        Timestamp(value)
    }

    pub fn get_value(self) -> i64 {
        self.0
    }
}

impl AddAssign for Timestamp {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other
    }
}

impl Add for Timestamp {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for Timestamp {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl SubAssign for Timestamp {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_value())
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
