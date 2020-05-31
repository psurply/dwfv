// SPDX-License-Identifier: MIT
extern crate dwfv;
extern crate gumdrop;

use dwfv::signaldb::{AsyncSignalDB, SignalDB};
use dwfv::tui::Tui;
use gumdrop::Options;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufReader};
use std::process;

/// A simple digital waveform viewer with vi-like key bindings
#[derive(Debug, Options)]
struct Args {
    /// Prints help information
    #[options()]
    help: bool,

    /// Prints version information
    #[options(short = "V")]
    version: bool,

    /// Layout file to use in the TUI
    #[options()]
    layout: Option<String>,

    /// Value Change Dump (VCD) file to parse
    #[options()]
    input: String,

    /// Subcommands
    #[options(command)]
    command: Option<Command>,
}

/// Available subcommands
#[derive(Debug, Options)]
enum Command {
    /// Displays states of the signals at a given timestamp
    #[options()]
    At(AtArgs),

    /// Shows stats about the input
    #[options()]
    Stats(StatsArgs),

    /// Displays the time periods when the specified expression is true
    #[options()]
    When(WhenArgs),
}

/// Displays states of the signals at a given timestamp
#[derive(Debug, Options)]
struct AtArgs {
    #[options()]
    help: bool,

    #[options(free)]
    timestamp: i64,
}

/// Shows stats about the input
#[derive(Debug, Options)]
struct StatsArgs {
    #[options()]
    help: bool,
}

/// Displays the time periods when the specified expression is true
#[derive(Debug, Options)]
struct WhenArgs {
    #[options()]
    help: bool,

    #[options(free)]
    expr: String,
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let file = File::open(args.input)?;
    let buf_reader = BufReader::new(file);

    if let Some(command) = args.command {
        match command {
            Command::At(AtArgs { timestamp, .. }) => {
                let db = SignalDB::from_vcd_with_limit(buf_reader, Some(timestamp))?;
                db.format_values_at(&mut io::stdout(), timestamp);
            }
            Command::Stats(_) => {
                let db = SignalDB::from_vcd(buf_reader)?;
                db.format_stats(&mut io::stdout());
            }
            Command::When(WhenArgs { expr, .. }) => {
                let mut db = SignalDB::from_vcd(buf_reader)?;
                db.search_all(&mut io::stdout(), &expr)?
            }
        }
    } else {
        let mut adb = AsyncSignalDB::new();
        adb.parse_vcd(buf_reader);

        adb.sync_db.wait_until_initialized()?;
        let mut tui = Tui::new(adb)?;
        if let Some(layout) = args.layout {
            tui.update_layout(layout)?
        }
        tui.run()?
    }

    Ok(())
}

fn main() {
    let args = Args::parse_args_default_or_exit();

    if args.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.input.is_empty() {
        eprintln!("Error: missing required option `--input`");
        process::exit(1);
    }

    if let Err(e) = run(args) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
