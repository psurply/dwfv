// SPDX-License-Identifier: MIT
use dwfv::signaldb::{AsyncSignalDB, SignalDB};
use dwfv::tui::Tui;
use gumdrop::Options;
use std::env;
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
    #[options(help_flag, short = "V")]
    version: bool,

    /// Layout file to use in the TUI
    #[options()]
    layout: Option<String>,

    /// Shows stats about the VCD file
    #[options()]
    stats: bool,

    /// Displays the time periods when the specified expression is true
    #[options(meta = "EXPR")]
    when: Option<String>,

    /// Displays states of the signals at a given timestamp
    #[options(meta = "TIMESTAMP")]
    at: Option<i64>,

    /// Value Change Dump (VCD) file to parse
    #[options(free, required)]
    file: String,
}

/// Available subcommands
fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let file = File::open(args.file)?;
    let buf_reader = BufReader::new(file);

    if let Some(timestamp) = args.at {
        let db = SignalDB::from_vcd_with_limit(buf_reader, Some(timestamp))?;
        db.format_values_at(&mut io::stdout(), timestamp)
    } else if let Some(expr) = args.when {
        let mut db = SignalDB::from_vcd(buf_reader)?;
        db.search_all(&mut io::stdout(), &expr)?
    } else if args.stats {
        let db = SignalDB::from_vcd(buf_reader)?;
        db.format_stats(&mut io::stdout())
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
    let args = env::args().collect::<Vec<_>>();

    let opts = Args::parse_args_default(&args[1..]).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        eprintln!("Usage: {} [options] file", args[0]);
        process::exit(2);
    });

    if opts.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }

    if opts.help_requested() {
        println!("Usage: {} [options] file", args[0]);
        println!();
        println!("{}", Args::usage());
        return;
    }

    if let Err(e) = run(opts) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
