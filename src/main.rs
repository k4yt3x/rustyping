#[macro_use]
extern crate slog;
use std::{process, sync::Mutex};

use clap::Parser;
use rustyping::{run, Config};
use slog::Drain;

/// A prettier lightweight colored ping utility written in Rust
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args
{
    /// dns name or ip address
    #[arg(index = 1)]
    destination: String,

    /// stop after <count> replies
    #[arg(short = 'c', long, default_value_t = 0)]
    count: u16,

    /// seconds between sending each packet
    #[arg(short = 'i', long, default_value_t = 1.0)]
    interval: f64,

    /// time to wait for response
    #[arg(short = 'W', long, default_value_t = 2.0)]
    timeout: f64,
}

/// parse command line arguments into a Config struct
///
/// # Errors
///
/// anything that implements the Error trait
fn parse() -> Option<Config>
{
    let args = Args::parse();

    // assign command line values to variables
    Config::new(
        {
            let decorator = slog_term::TermDecorator::new().build();
            let drain = Mutex::new(slog_term::FullFormat::new(decorator).build()).fuse();
            slog::Logger::root(drain, o!())
        },
        args.destination,
        args.count,
        args.interval,
        args.timeout,
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
