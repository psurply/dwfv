// SPDX-License-Identifier: MIT

#[macro_use] extern crate lalrpop_util;
#[macro_use] extern crate lazy_static;
extern crate regex;
extern crate tui as tuirs;
extern crate termion;

pub mod signaldb;
mod vcd;
mod search;
pub mod tui;
