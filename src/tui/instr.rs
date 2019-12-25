// SPDX-License-Identifier: MIT
use std::fmt;
use std::io;
use std::io::prelude::*;

/// TUI Instruction
#[derive(Clone)]
pub enum TuiInstr {
    /// Tell the TUI to display a signal.
    Signal(String),
    /// Tell the TUI to display the result of a search expression.
    Search(String),
    /// Tell the TUI to display an error message.
    Error(String, String),
}

impl TuiInstr {
    pub fn height(&self) -> usize {
        match self {
            TuiInstr::Signal(_) => 3,
            TuiInstr::Search(_) => 1,
            TuiInstr::Error(_, _) => 1,
        }
    }

    pub fn total_height(instrs: &[TuiInstr]) -> usize {
        let mut h = 0;
        for instr in instrs {
            h += instr.height()
        }
        h
    }

    fn parse_line(line: &str) -> TuiInstr {
        let v: Vec<&str> = line.splitn(2, ' ').collect();
        if v.len() != 2 {
            return TuiInstr::Error(line.to_string(), "Syntax Error".to_string());
        }
        let instr = v.first().unwrap();
        let arg = v.last().unwrap().to_string();
        match *instr {
            "signal" => TuiInstr::Signal(arg),
            "search" => TuiInstr::Search(arg),
            _ => TuiInstr::Error(line.to_string(), format!("Unknown command '{}'", instr)),
        }
    }

    pub fn parse<I: BufRead>(input: I) -> Vec<TuiInstr> {
        let mut instrs = Vec::new();
        for line in input.lines() {
            let s = line.unwrap();

            if let Some('#') = s.chars().next() {
                continue;
            }

            if !s.is_empty() {
                instrs.push(TuiInstr::parse_line(&s))
            }
        }
        instrs
    }

    pub fn format_instrs<O: io::Write>(instrs: &[TuiInstr], output: &mut O) {
        for instr in instrs {
            let _ = writeln!(output, "{}", instr);
        }
    }
}

impl fmt::Display for TuiInstr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TuiInstr::Signal(s) => write!(f, "signal {}", s),
            TuiInstr::Search(s) => write!(f, "search {}", s),
            TuiInstr::Error(s, _) => write!(f, "{}", s),
        }?;
        Ok(())
    }
}
