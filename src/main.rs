#[macro_use]
extern crate slog;
extern crate nix;
use ansi_term::Color;
use clap::{value_t_or_exit, Arg};
use fastping_rs::PingResult::{Idle, Receive};
use fastping_rs::Pinger;
use slog::Drain;
use std::sync::Mutex;

fn main() {
    // initialize logger
    let decorator = slog_term::TermDecorator::new().build();
    let drain = Mutex::new(slog_term::FullFormat::new(decorator).build()).fuse();
    let log = slog::Logger::root(drain, o!());

    // parse command line arguments
    let matches = clap::App::new("rping")
        .version("0.1.0")
        .author("K4YT3X <k4yt3x@k4yt3x.com>")
        .about("A prettier ping utility written in Rust")
        .arg(
            Arg::with_name("destination")
                .value_name("DESTINATION")
                .help("dns name or ip address")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .value_name("INTERVAL")
                .help("seconds between sending each packet")
                .default_value("1")
                .takes_value(true),
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
        .get_matches();

    let destination = matches.value_of("destination").unwrap();
    let interval = (value_t_or_exit!(matches.value_of("interval"), f64) * 1000.0) as u64;
    let count = value_t_or_exit!(matches.value_of("count"), u64);

    // check if flooding is possible
    if interval < 200 && nix::unistd::getuid().as_raw() != 0 {
        warn!(
            log,
            "cannot flood; minimal interval allowed for user is 200ms"
        );
        warn!(log, "interval will be set to 200ms")
    }

    // initialize pinger
    let (pinger, results) = match Pinger::new(Some(interval as u64), None) {
        Ok((pinger, results)) => (pinger, results),
        Err(e) => panic!("error creating pinger: {}", e),
    };

    pinger.add_ipaddr(destination);
    pinger.run_pinger();
    let mut seq = 0;

    while count == 0 || seq < count {
        match results.recv() {
            Ok(result) => match result {
                Idle { addr } => {
                    warn!(log, "no answer from {} seq={}", addr, seq);
                }
                Receive { addr, rtt } => {
                    let millis = rtt.as_millis() as u64;
                    let color = if millis < 50 {
                        Color::Green
                    } else if millis < 100 {
                        Color::Yellow
                    } else {
                        Color::Red
                    };

                    info!(
                        log,
                        "answer from {} seq={} rtt={}ms",
                        addr,
                        seq,
                        color.paint(millis.to_string())
                    );
                }
            },
            Err(_) => panic!("an error occurred in the worker thread"),
        }
        seq += 1;
    }
}
