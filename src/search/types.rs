// SPDX-License-Identifier: MIT
use super::expr::{ExprAst, ValueAst};
use crate::signaldb::{SignalDB, SignalValue, TimeDescr, Timestamp};
use std::error::Error;
use std::io;
use std::ops::{BitAnd, BitOr};

pub(crate) struct Search {
    findings: Vec<TimeDescr>,
    expr: ExprAst,
    current_period: Option<Timestamp>,
    cursor: Option<Timestamp>,
}

#[derive(Debug, Copy, Clone)]
enum ExprType {
    Transition,
    Level,
}

impl BitOr for ExprType {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        match (self, rhs) {
            (ExprType::Transition, _) | (_, ExprType::Transition) => ExprType::Transition,
            _ => ExprType::Level,
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct EvalResult {
    result: bool,
    ty: ExprType,
}

impl BitOr for EvalResult {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        let result = self.result || rhs.result;
        let ty = if result {
            if self.result == rhs.result {
                self.ty | rhs.ty
            } else if self.result {
                self.ty
            } else {
                rhs.ty
            }
        } else {
            ExprType::Level
        };
        EvalResult { result, ty }
    }
}

impl BitAnd for EvalResult {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        let result = self.result && rhs.result;
        let ty = if result {
            self.ty | rhs.ty
        } else {
            ExprType::Level
        };
        EvalResult { result, ty }
    }
}

/// Summary of findings within a time period
#[derive(Debug, PartialEq, Eq)]
pub enum FindingsSummary {
    /// No findings in the time period
    Nothing,
    /// Only one finding in the time period
    Timestamp,
    /// The expression starts to be true during the time period
    RangeBegin,
    /// The expression is true during the time period
    Range,
    /// The expression stops to be true during the time period
    RangeEnd,
    /// Many findings found in the time period
    Complex(usize),
}

impl Search {
    pub(crate) fn new(expr: &str) -> Result<Search, Box<dyn Error>> {
        let search = Search {
            expr: ExprAst::from_str(expr)?,
            findings: Vec::new(),
            current_period: None,
            cursor: Some(Timestamp::origin()),
        };
        Ok(search)
    }

    pub(crate) fn eval_value_at(
        &self,
        value: &ValueAst,
        signaldb: &SignalDB,
        timestamp: Timestamp,
    ) -> Result<SignalValue, Box<dyn Error>> {
        let res = match value {
            ValueAst::Literal(v) => v.clone(),
            ValueAst::Id(id) => signaldb.value_at(id, timestamp)?,
        };
        Ok(res)
    }

    fn eval_at(
        &self,
        expr: &ExprAst,
        signaldb: &SignalDB,
        timestamp: Timestamp,
    ) -> Result<EvalResult, Box<dyn Error>> {
        let res = match expr {
            ExprAst::Equal(id, v) => EvalResult {
                result: signaldb.value_at(id, timestamp)?
                    == self.eval_value_at(v, signaldb, timestamp)?,
                ty: ExprType::Level,
            },
            ExprAst::Transition(id, v) => EvalResult {
                result: {
                    match signaldb.event_at(id, timestamp)? {
                        Some(evt) => evt == self.eval_value_at(v, signaldb, timestamp)?,
                        None => false,
                    }
                },
                ty: ExprType::Transition,
            },
            ExprAst::AnyTransition(id) => EvalResult {
                result: signaldb.event_at(id, timestamp)?.is_some(),
                ty: ExprType::Transition,
            },
            ExprAst::And(le, re) => {
                let ler = self.eval_at(le, signaldb, timestamp)?;
                if !ler.result {
                    ler
                } else {
                    let rer = self.eval_at(re, signaldb, timestamp)?;
                    EvalResult {
                        result: ler.result && rer.result,
                        ty: ler.ty | rer.ty,
                    }
                }
            }
            ExprAst::Or(le, re) => {
                let ler = self.eval_at(le, signaldb, timestamp)?;
                if ler.result {
                    ler
                } else {
                    let rer = self.eval_at(re, signaldb, timestamp)?;
                    ler | rer
                }
            }
            ExprAst::Not(e) => {
                let er = self.eval_at(e, signaldb, timestamp)?;
                let result = !er.result;
                let ty = if result { er.ty } else { ExprType::Level };
                EvalResult { result, ty }
            }
            ExprAst::After(t) => EvalResult {
                result: timestamp > timestamp.derive(*t),
                ty: ExprType::Level,
            },
            ExprAst::Before(t) => EvalResult {
                result: timestamp < timestamp.derive(*t),
                ty: ExprType::Level,
            },
        };
        Ok(res)
    }

    pub(crate) fn search_all(&mut self, signaldb: &SignalDB) -> Result<(), Box<dyn Error>> {
        self.findings.clear();
        self.current_period = None;
        for timestamp in signaldb.get_timestamps() {
            self.search_at(signaldb, timestamp)?
        }
        self.finish();
        Ok(())
    }

    pub(crate) fn search_at(
        &mut self,
        signaldb: &SignalDB,
        timestamp: Timestamp,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(cursor) = self.cursor {
            if timestamp < cursor {
                return Ok(());
            }
        }
        let res = self.eval_at(&self.expr, signaldb, timestamp)?;
        match res.ty {
            ExprType::Transition => {
                if res.result && self.current_period.is_none() {
                    self.findings.push(TimeDescr::Point(timestamp))
                }
            }
            ExprType::Level => {
                if res.result && self.current_period.is_none() {
                    self.current_period = Some(timestamp)
                } else if !res.result && self.current_period.is_some() {
                    self.findings
                        .push(TimeDescr::Period(self.current_period.unwrap(), timestamp));
                    self.current_period = None;
                }
            }
        };
        self.cursor = Some(timestamp);
        Ok(())
    }

    pub(crate) fn finish(&mut self) {
        self.cursor = None
    }

    pub(crate) fn format_findings(&self, output: &mut dyn io::Write) {
        for timestamp in &self.findings {
            let _ = writeln!(output, "{}", timestamp);
        }
    }

    fn search_finding(&self, timestamp: Timestamp) -> Result<usize, usize> {
        self.findings.binary_search_by_key(&timestamp, |t| match t {
            TimeDescr::Point(p) => *p,
            TimeDescr::Period(begin, end) => {
                if *begin <= timestamp && timestamp < *end {
                    timestamp
                } else if timestamp <= *begin {
                    *begin
                } else {
                    *end - end.derive(1)
                }
            }
        })
    }

    pub(crate) fn findings_between(&self, begin: Timestamp, end: Timestamp) -> FindingsSummary {
        if let Some(current_period) = self.current_period {
            if begin <= current_period && current_period < end {
                return FindingsSummary::RangeBegin;
            }

            if let Some(cursor) = self.cursor {
                if current_period <= begin && end <= cursor {
                    return FindingsSummary::Range;
                }
            } else if current_period <= begin {
                return FindingsSummary::Range;
            }
        }

        let seek = (
            self.search_finding(begin - begin.derive(1)),
            self.search_finding(end - end.derive(1)),
        );

        match seek {
            (Err(bi), Err(ei)) => {
                if bi == ei {
                    FindingsSummary::Nothing
                } else if ei - bi == 1 {
                    match self.findings.get(bi).unwrap() {
                        TimeDescr::Period(_, _) => FindingsSummary::Complex(1),
                        TimeDescr::Point(_) => FindingsSummary::Timestamp,
                    }
                } else {
                    FindingsSummary::Complex(ei - bi)
                }
            }
            (Ok(bi), Err(_)) => match self.findings.get(bi).unwrap() {
                TimeDescr::Point(_) => FindingsSummary::Nothing,
                _ => FindingsSummary::RangeEnd,
            },
            (Err(_), Ok(ei)) => match self.findings.get(ei).unwrap() {
                TimeDescr::Period(b, e) => {
                    if *b == end {
                        FindingsSummary::Nothing
                    } else if *e < end {
                        FindingsSummary::RangeEnd
                    } else {
                        FindingsSummary::RangeBegin
                    }
                }
                TimeDescr::Point(_) => FindingsSummary::Timestamp,
            },
            (Ok(bi), Ok(ei)) => match self.findings.get(bi).unwrap() {
                TimeDescr::Period(b, _) => {
                    if *b == begin {
                        FindingsSummary::RangeBegin
                    } else if ei == bi {
                        FindingsSummary::Range
                    } else {
                        FindingsSummary::Complex(ei - bi)
                    }
                }
                _ => FindingsSummary::Complex(ei - bi),
            },
        }
    }

    pub(crate) fn get_next_finding(&self, from: Timestamp) -> Option<Timestamp> {
        let index = match self.search_finding(from) {
            Ok(index) => index + 1,
            Err(index) => index,
        };
        self.findings
            .get(index)
            .map(|x| match x {
                TimeDescr::Point(t) => *t,
                TimeDescr::Period(t, _) => *t,
            })
            .or_else(|| {
                self.current_period.and_then(|current_period| {
                    if current_period > from {
                        Some(current_period)
                    } else {
                        None
                    }
                })
            })
    }

    pub(crate) fn get_end_of_next_finding(&self, from: Timestamp) -> Option<Timestamp> {
        let index = match self.search_finding(from) {
            Ok(index) => index + 1,
            Err(index) => index,
        };
        self.findings.get(index).map(|x| match x {
            TimeDescr::Point(t) => *t,
            TimeDescr::Period(_, t) => *t,
        })
    }

    pub(crate) fn get_previous_finding(&self, from: Timestamp) -> Option<Timestamp> {
        let index = match self.search_finding(from - from.derive(1)) {
            Ok(index) => index,
            Err(index) => {
                if index > 0 {
                    index - 1
                } else {
                    index
                }
            }
        };
        self.findings
            .get(index)
            .map(|x| match x {
                TimeDescr::Point(t) => *t,
                TimeDescr::Period(t, _) => *t,
            })
            .or_else(|| {
                self.current_period.and_then(|current_period| {
                    if current_period < from {
                        Some(current_period)
                    } else {
                        None
                    }
                })
            })
    }

    pub(crate) fn get_first_finding(&self) -> Option<Timestamp> {
        self.findings.first().map(|x| match x {
            TimeDescr::Point(t) => *t,
            TimeDescr::Period(t, _) => *t,
        })
    }

    pub(crate) fn get_last_finding(&self) -> Option<Timestamp> {
        self.findings.last().map(|x| match x {
            TimeDescr::Point(t) => *t,
            TimeDescr::Period(_, t) => *t,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn search() {
        let mut _db = SignalDB::new();
        let mut _search = Search::new("$A");
    }
}
