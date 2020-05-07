// SPDX-License-Identifier: MIT
extern crate clap;
extern crate dwfv;

use clap::{crate_description, crate_version, App, Arg, ArgMatches, SubCommand};
use dwfv::signaldb::{AsyncSignalDB, SignalDB, Timestamp};
use dwfv::tui::Tui;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufReader};
use std::process;

fn run(args: ArgMatches) -> Result<(), Box<dyn Error>> {
    let file = File::open(args.value_of("INPUT").unwrap())?;
    let buf_reader = BufReader::new(file);

    if args.subcommand_matches("stats").is_some() {
        let db = SignalDB::from_vcd(buf_reader)?;
        db.format_stats(&mut io::stdout());
    } else if let Some(matches) = args.subcommand_matches("at") {
        let timestamp = matches
            .value_of("TIMESTAMP")
            .unwrap()
            .parse()
            .map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "TIMESTAMP must be an integer")
            })?;
        let timestamp = Timestamp::new(timestamp);
        let db = SignalDB::from_vcd_with_limit(buf_reader, Some(timestamp))?;
        db.format_values_at(&mut io::stdout(), timestamp);
    } else if let Some(matches) = args.subcommand_matches("when") {
        let expr = matches.value_of("EXPR").unwrap();
        let mut db = SignalDB::from_vcd(buf_reader)?;
        db.search_all(&mut io::stdout(), expr)?
    } else {
        let mut adb = AsyncSignalDB::new();
        adb.parse_vcd(buf_reader);

        adb.sync_db.wait_until_initialized()?;
        let mut tui = Tui::new(adb)?;
        if let Some(layout) = args.value_of("layout") {
            tui.update_layout(layout)?
        }
        tui.run()?
    }

    Ok(())
}

fn main() {
    let matches = App::new("dwfv")
        .version(crate_version!())
        .about(crate_description!())
        .arg(
            Arg::with_name("INPUT")
                .help("Value Change Dump (VCD) file to parse")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("layout")
                .short("l")
                .value_name("LAYOUT")
                .help("Layout file to use in the TUI")
                .takes_value(true),
        )
        .subcommand(SubCommand::with_name("stats").about("Shows stats about the input"))
        .subcommand(
            SubCommand::with_name("at")
                .about("Displays states of the signals at a given timestamp")
                .arg(Arg::with_name("TIMESTAMP").required(true)),
        )
        .subcommand(
            SubCommand::with_name("when")
                .about("Displays the time periods when the specified expression is true")
                .arg(Arg::with_name("EXPR").required(true)),
        )
        .get_matches();

    if let Err(e) = run(matches) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
