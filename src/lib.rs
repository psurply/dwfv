// SPDX-License-Identifier: MIT

#[macro_use]
extern crate lalrpop_util;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate termion;
extern crate tui as tuirs;

mod search;
pub mod signaldb;
pub mod tui;
mod vcd;
