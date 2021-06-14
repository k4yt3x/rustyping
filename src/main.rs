#![feature(destructuring_assignment)]
#[macro_use]
extern crate slog;
use ansi_term::Color;
use clap::{value_t_or_exit, Arg};
use hsl::HSL;
use pnet::packet::icmp::{echo_reply, echo_request, IcmpTypes};
use pnet::packet::icmpv6::{Icmpv6Types, MutableIcmpv6Packet};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::Packet;
use pnet::transport::TransportChannelType::Layer4;
use pnet::transport::TransportProtocol::{Ipv4, Ipv6};
use pnet::transport::{
    icmp_packet_iter, icmpv6_packet_iter, transport_channel, TransportReceiver, TransportSender,
};
use rand::random;
use slog::Drain;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// use ansi_term to color the rtt value and returns
///   the colored value as a string
fn paint_rtt(rtt: u128) -> String {
    let hue = (100.0 - rtt as f64 / (1000.0 * 100.0) * 100.0) as f64;
    let hsl = HSL {
        h: if hue < 0.0 { 0.0 } else { hue },
        s: 1.0,
        l: 0.5,
    };
    let (red, green, blue) = hsl.to_rgb();
    let color = Color::RGB(red, green, blue);

    if rtt < 1000 {
        color
            .paint(format!("{:.5}", (rtt as f64 / 1000.0).to_string()))
            .to_string()
    } else {
        color.paint((rtt / 1000).to_string()).to_string()
    }
}

fn ping(
    address: IpAddr,
    timeout: f64,
    size: usize,
    sequence: u16,
    identifier: u16,
) -> Result<Option<Duration>, std::io::Error> {
    // allocate space for packet
    let mut packet_buffer: Vec<u8> = vec![0; size];
    let mut sender: TransportSender;
    let mut receiver: TransportReceiver;

    // construct packet content
    if address.is_ipv4() {
        let mut packet =
            echo_request::MutableEchoRequestPacket::new(&mut packet_buffer[..]).unwrap();
        packet.set_icmp_type(IcmpTypes::EchoRequest);
        packet.set_sequence_number(sequence);
        packet.set_identifier(identifier);
        packet.set_checksum(pnet::util::checksum(&packet.packet(), 1));
        (sender, receiver) =
            transport_channel(size, Layer4(Ipv4(IpNextHeaderProtocols::Icmp))).unwrap();
        sender.send_to(packet, address).unwrap();
    } else {
        let mut packet = MutableIcmpv6Packet::new(&mut packet_buffer[..]).unwrap();
        packet.set_icmpv6_type(Icmpv6Types::EchoRequest);
        (sender, receiver) =
            transport_channel(size, Layer4(Ipv6(IpNextHeaderProtocols::Icmpv6))).unwrap();
        sender.send_to(packet, address).unwrap();
    }

    let sent_time = Instant::now();
    let mut loop_timeout = Duration::from_secs_f64(timeout);

    if address.is_ipv4() {
        let mut receiver_iterator = icmp_packet_iter(&mut receiver);
        loop {
            // get data from receiver
            let data = receiver_iterator.next_with_timeout(loop_timeout).unwrap();

            match data {
                None => return Ok(None),
                Some(data) => {
                    let (received, _address) = data;
                    if received.get_icmp_type() == IcmpTypes::EchoReply {
                        let reply = echo_reply::EchoReplyPacket::new(received.packet()).unwrap();

                        if reply.get_identifier() == identifier
                            && reply.get_sequence_number() == sequence
                        {
                            // return rtt = now - start
                            return Ok(Some(Instant::now().duration_since(sent_time)));

                        // this should not happen
                        // we have not sent a packet with a greater sequence number yet
                        } else if reply.get_identifier() == identifier
                            && reply.get_sequence_number() >= sequence
                        {
                            panic!("got impossible sequence number")
                        }
                    }
                }
            }

            if Instant::now().duration_since(sent_time) > Duration::from_secs_f64(timeout) {
                return Ok(None);
            } else {
                loop_timeout =
                    Duration::from_secs_f64(timeout) - Instant::now().duration_since(sent_time)
            }
        }
    } else {
        let mut receiver_iterator = icmpv6_packet_iter(&mut receiver);
        loop {
            // get data from receiver
            let data = receiver_iterator.next_with_timeout(loop_timeout).unwrap();

            match data {
                None => return Ok(None),
                Some(data) => {
                    let (received, _address) = data;
                    if received.get_icmpv6_type() == Icmpv6Types::EchoReply {
                        return Ok(Some(Instant::now().duration_since(sent_time)));
                    }
                }
            }

            if Instant::now().duration_since(sent_time) > Duration::from_secs_f64(timeout) {
                return Ok(None);
            } else {
                loop_timeout =
                    Duration::from_secs_f64(timeout) - Instant::now().duration_since(sent_time)
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // initialize logger
    let decorator = slog_term::TermDecorator::new().build();
    let drain = Mutex::new(slog_term::FullFormat::new(decorator).build()).fuse();
    let log = slog::Logger::root(drain, o!());

    // parse command line arguments
    let matches = clap::App::new("rustping")
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

    let destination = matches.value_of("destination").unwrap();
    let count = value_t_or_exit!(matches.value_of("count"), u16);
    let mut interval = value_t_or_exit!(matches.value_of("interval"), f64);
    let timeout = value_t_or_exit!(matches.value_of("timeout"), f64);

    let address = match destination.parse::<IpAddr>() {
        // address is valid, use this address
        Ok(address) => address,

        // address is invalid, try to resolve destination into IpAddr
        Err(_e) => {
            let resolved = (destination, 0).to_socket_addrs().unwrap().next();
            match resolved {
                None => panic!("unable to resolve destination hostname"),
                Some(resolved) => resolved.ip(),
            }
        }
    };

    // check if flooding is possible
    if interval < 0.2 && nix::unistd::getuid().as_raw() != 0 {
        warn!(
            log,
            "cannot flood; minimal interval allowed for user is 200ms"
        );
        warn!(log, "interval will be set to 200ms");
        interval = 0.2
    }

    let identifier = random::<u16>();
    let mut sequence: u16 = 0;
    let mut total_rtt = Duration::new(0, 0);
    let mut transmitted = 0;
    let mut received = 0;
    let mut min: Option<Duration> = None;
    let mut max: Option<Duration> = None;

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("error setting Ctrl-C handler");

    while running.load(Ordering::SeqCst) && (count == 0 || sequence < count) {
        let cycle_begin_time = Instant::now();
        let rtt = ping(address, timeout, 64, sequence, identifier).unwrap();

        match rtt {
            None => {
                warn!(log, "no answer from {} seq={}", address, sequence);
            }
            Some(rtt) => {
                // if min is not initialized, set min=millis
                // else compare and set accordingly
                if let Some(current_min) = min {
                    if rtt < current_min {
                        min = Some(rtt)
                    }
                } else {
                    min = Some(rtt)
                }

                // if max is not initialized, set max=millis
                // else compare and set accordingly
                if let Some(current_max) = max {
                    if rtt > current_max {
                        max = Some(rtt)
                    }
                } else {
                    max = Some(rtt)
                }

                total_rtt += rtt;

                info!(
                    log,
                    "answer from {} seq={} rtt={}ms",
                    address,
                    sequence,
                    paint_rtt(rtt.as_micros())
                );
                received += 1;
            }
        }
        transmitted += 1;
        sequence += 1;

        if Instant::now().duration_since(cycle_begin_time) < Duration::from_secs_f64(interval) {
            thread::sleep(
                Duration::from_secs_f64(interval) - Instant::now().duration_since(cycle_begin_time),
            )
        }
    }

    // print final statistics
    info!(
        log,
        "{}",
        Color::Fixed(240)
            .bold()
            .paint(format!("{} ping statistics", destination))
    );

    // calculate %loss
    let loss = if transmitted == 0 {
        100.0
    } else {
        ((transmitted - received) as f64 / transmitted as f64) * 100.0
    };

    info!(
        log,
        "{}",
        Color::Fixed(240).bold().paint(format!(
            "transmitted={} received={} loss={:.4}%",
            transmitted, received, loss
        ))
    );

    let final_min = match min {
        None => Duration::new(0, 0),
        Some(min) => min,
    };

    let final_max = match max {
        None => Duration::new(0, 0),
        Some(max) => max,
    };

    let avg = if sequence == 0 {
        0
    } else {
        total_rtt.as_micros() / sequence as u128
    };

    info!(
        log,
        "{}{}{}{}{}{}{}",
        Color::Fixed(240).bold().paint("min="),
        paint_rtt(final_min.as_micros()),
        Color::Fixed(240).bold().paint("ms max="),
        paint_rtt(final_max.as_micros()),
        Color::Fixed(240).bold().paint("ms avg="),
        paint_rtt(avg),
        Color::Fixed(240).bold().paint("ms")
    );

    Ok(())
}
