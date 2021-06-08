#[macro_use]
extern crate slog;
extern crate nix;
use ansi_term::Color;
use clap::{value_t_or_exit, Arg};
use fastping_rs::PingResult::{Idle, Receive};
use fastping_rs::Pinger;
use slog::Drain;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

fn paint_rtt(rtt: u64) -> String {
    let color = if rtt < 50 {
        Color::Green
    } else if rtt < 100 {
        Color::Yellow
    } else {
        Color::Red
    };
    return color.paint(rtt.to_string()).to_string();
}

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

    let destination = matches.value_of("destination").unwrap().to_owned();
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
    pinger.add_ipaddr(&destination);
    pinger.run_pinger();

    let mut seq = 0;
    let mut total_rtt = 0;
    let mut transmitted = 0;
    let mut received = 0;
    let mut min: Option<u64> = None;
    let mut max: Option<u64> = None;

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("error setting Ctrl-C handler");

    while running.load(Ordering::SeqCst) && (count == 0 || seq < count) {
        match results.recv() {
            Ok(result) => match result {
                Idle { addr } => {
                    warn!(log, "no answer from {} seq={}", addr, seq);
                    transmitted += 1;
                    seq += 1;
                }
                Receive { addr, rtt } => {
                    let millis = rtt.as_millis() as u64;

                    // if min is not initialized, set min=millis
                    // else compare and set accordingly
                    if let Some(current_min) = min {
                        if millis < current_min {
                            min = Some(millis)
                        }
                    } else {
                        min = Some(millis)
                    }

                    // if max is not initialized, set max=millis
                    // else compare and set accordingly
                    if let Some(current_max) = max {
                        if millis > current_max {
                            max = Some(millis)
                        }
                    } else {
                        max = Some(millis)
                    }

                    total_rtt += millis;

                    info!(
                        log,
                        "answer from {} seq={} rtt={}ms",
                        addr,
                        seq,
                        paint_rtt(millis)
                    );
                    received += 1;
                    transmitted += 1;
                    seq += 1;
                }
            },
            Err(_) => panic!("an error occurred in the worker thread"),
        }
    }

    if let Some(final_min) = min {
        if let Some(final_max) = max {
            if seq > 0 && transmitted > 0 {
                // reinitialize the logger since it cannot be copied
                let decorator = slog_term::TermDecorator::new().build();
                let drain = Mutex::new(slog_term::FullFormat::new(decorator).build()).fuse();
                let log = slog::Logger::root(drain, o!());
                info!(
                    log,
                    "{}",
                    Color::Purple.paint(format!("{} ping statistics", destination))
                );
                info!(
                    log,
                    "{}",
                    Color::Purple.paint(format!(
                        "transmitted={} received={} loss={:.4}%",
                        transmitted,
                        received,
                        ((transmitted - received) as f64 / transmitted as f64) * 100.0
                    ))
                );
                info!(
                    log,
                    "{}",
                    Color::Purple.paint(format!(
                        "min={}ms max={}ms avg={}ms",
                        final_min,
                        final_max,
                        total_rtt / seq
                    ))
                );
            }
        } else {
            std::process::exit(0)
        }
    } else {
        std::process::exit(0)
    }
}
