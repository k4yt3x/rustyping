#[macro_use]
extern crate slog;
use std::{process, sync::Mutex};

use clap::{value_t_or_exit, Arg};
use rustyping::{run, Config};
use slog::Drain;

/// parse command line arguments into a Config struct
///
/// # Errors
///
/// anything that implements the Error trait
fn parse() -> Option<Config>
{
    // parse command line arguments
    let matches = clap::App::new("rustyping")
        .version("2.2.0")
        .author("K4YT3X <k4yt3x@k4yt3x.com>")
        .about("A prettier lightweight colored ping utility written in Rust")
        .arg(
            Arg::with_name("destination")
                .value_name("DESTINATION")
                .help("dns name or ip address")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("count")
                .short("c")
                .long("count")
                .value_name("COUNT")
                .help("stop after <count> replies")
                .default_value("0")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .value_name("INTERVAL")
                .help("seconds between sending each packet")
                .default_value("1.0")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("timeout")
                .short("W")
                .long("timeout")
                .value_name("TIMEOUT")
                .help("time to wait for response")
                .default_value("2.0")
                .takes_value(true),
        )
        .get_matches();

    // assign command line values to variables
    Config::new(
        {
            let decorator = slog_term::TermDecorator::new().build();
            let drain = Mutex::new(slog_term::FullFormat::new(decorator).build()).fuse();
            slog::Logger::root(drain, o!())
        },
        matches.value_of("destination").unwrap().to_owned(),
        value_t_or_exit!(matches.value_of("count"), u16),
        value_t_or_exit!(matches.value_of("interval"), f64),
        value_t_or_exit!(matches.value_of("timeout"), f64),
    )
}

/// program entry point
fn main()
{
    // parse command line arguments into Config
    if let Some(config) = parse() {
        // run ping with config
        process::exit(match run(config) {
            Ok(_) => 0,
            Err(_) => 1,
        })
    }
    else {
        process::exit(1);
    }
}
