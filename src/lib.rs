#[macro_use]
extern crate slog;
use std::{
    error::Error,
    net::{IpAddr, ToSocketAddrs},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use ansi_term::Color;
use hsl::HSL;
use pnet::{
    packet::{
        icmp::{echo_reply, echo_request, IcmpTypes},
        icmpv6::{Icmpv6Types, MutableIcmpv6Packet},
        ip::IpNextHeaderProtocols,
        Packet,
    },
    transport::{
        icmp_packet_iter, icmpv6_packet_iter, transport_channel,
        TransportChannelType::Layer4,
        TransportProtocol::{Ipv4, Ipv6},
        TransportReceiver, TransportSender,
    },
};
use rand::random;

/// configs passed to the run function
pub struct Config
{
    logger: slog::Logger,
    destination: IpAddr,
    count: u16,
    interval: f64,
    timeout: f64,
}

impl Config
{
    pub fn new(
        logger: slog::Logger,
        destination: String,
        count: u16,
        mut interval: f64,
        timeout: f64,
    ) -> Option<Config>
    {
        if interval < 0.0 {
            crit!(logger, "the value of 'interval' cannot be negative");
            return None;
        }

        if timeout < 0.0 {
            crit!(logger, "the value of 'timeout' cannot be negative");
            return None;
        }

        // check if interval is below 0.2
        if (interval < 0.2 && nix::unistd::getuid().as_raw() != 0)
            && !cfg!(feature = "unrestricted")
        {
            warn!(
                logger,
                "cannot flood; minimal interval allowed for user is 200ms"
            );
            warn!(logger, "interval will be set to 200ms");
            interval = 0.2
        }

        // resolve destination String into IpAddr
        let destination = match Config::resolve_hostname(destination) {
            Ok(destination) => destination,
            Err(error) => {
                crit!(logger, "{}", error);
                return None;
            }
        };

        Some(Config {
            logger,
            destination,
            count,
            interval,
            timeout,
        })
    }

    /// resolve hostname String into IpAddr
    ///
    /// # Arguments
    ///
    /// * `hostname` - hostname to resolve
    ///
    /// # Errors
    ///
    /// resolution errors
    ///
    /// # Examples
    ///
    /// ```
    /// let ip = resolve_hostname("evnk.io")?;
    /// ```
    fn resolve_hostname(hostname: String) -> Result<IpAddr, Box<dyn Error>>
    {
        // check if destination is a valid IP address
        match hostname.parse::<IpAddr>() {
            // address is valid, use this address
            Ok(address) => Ok(address),

            // address is invalid, try to resolve destination into IpAddr
            Err(_) => match (hostname, 0).to_socket_addrs() {
                // hostname has been resolved successfully
                Ok(mut resolve_result) => {
                    if let Some(resolve) = resolve_result.next() {
                        // final result
                        Ok(resolve.ip())
                    }
                    // empty resolution result
                    else {
                        Err("the resolver has returned an invalid result".into())
                    }
                }
                // failed to resolve
                Err(_) => Err("unable to resolve destination hostname".into()),
            },
        }
    }
}

/// use ansi_term to color the rtt value and returns
/// the colored value as a string
///
/// # Arguments
///
/// * `rtt` - the round trip time value to be painted
///
/// # Examples
///
/// ```
/// paint_rtt(30_u128)
/// ```
fn paint_rtt(rtt: u128) -> String
{
    // calculate hue value from latency
    // 0ms == 0° (green), 100ms == 100° (red)
    let hue = (100.0 - rtt as f64 / (1000.0 * 100.0) * 100.0) as f64;
    let hsl = HSL {
        h: if hue < 0.0 { 0.0 } else { hue },
        s: 1.0,
        l: 0.5,
    };

    // convert HSL color space into RGB
    let (red, green, blue) = hsl.to_rgb();
    let color = Color::RGB(red, green, blue);

    // if RTT is less than 1ms, show three digits after the decimal point
    if rtt < 1000 {
        color
            .paint(format!("{:.5}", (rtt as f64 / 1000.0).to_string()))
            .to_string()
    }
    else {
        color.paint((rtt / 1000).to_string()).to_string()
    }
}

/// send ICMP/ICMPv6 echo request to an address and return the RTT if a response is received
/// if no responses are received, return Ok(None)
///
/// # Arguments
///
/// * `address` - IPv4 or IPv6 address to ping
/// * `timeout` - ICMP echo receival timeout
/// * `size` - ICMP echo data size
/// * `sequence` - ICMP echo sequence number
/// * `identifier` - ICMP echo identifier
///
/// # Errors
///
/// std::io::Error if packets cannot be sent
///
/// # Examples
///
/// ```
/// ping(
///     std::net::Ipv4Addr::new(1, 1, 1, 1),
///     time::Duration::new(1, 0),
///     64,
///     rand::random::<u16>(),
///     random::<u16>(),
/// )
/// ```
fn ping(
    address: IpAddr,
    timeout: f64,
    size: usize,
    sequence: u16,
    identifier: u16,
) -> Result<Option<Duration>, std::io::Error>
{
    // allocate space for packet
    let mut packet_buffer: Vec<u8> = vec![0; size];
    let mut sender: TransportSender;
    let mut receiver: TransportReceiver;

    // if the target address is an IPv4 address
    if address.is_ipv4() {
        let mut packet =
            echo_request::MutableEchoRequestPacket::new(&mut packet_buffer[..]).unwrap();
        packet.set_icmp_type(IcmpTypes::EchoRequest);
        packet.set_sequence_number(sequence);
        packet.set_identifier(identifier);
        packet.set_checksum(pnet::util::checksum(&packet.packet(), 1));
        (sender, receiver) = transport_channel(size, Layer4(Ipv4(IpNextHeaderProtocols::Icmp)))?;
        sender.send_to(packet, address)?;

    // if the target address is an IPv6 address
    }
    else {
        let mut packet = MutableIcmpv6Packet::new(&mut packet_buffer[..]).unwrap();
        packet.set_icmpv6_type(Icmpv6Types::EchoRequest);
        (sender, receiver) = transport_channel(size, Layer4(Ipv6(IpNextHeaderProtocols::Icmpv6)))?;
        sender.send_to(packet, address)?;
    }

    // start timer
    let sent_time = Instant::now();
    let mut loop_timeout = Duration::from_secs_f64(timeout);

    // ICMP
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
                        }
                        else if reply.get_identifier() == identifier
                            && reply.get_sequence_number() >= sequence
                        {
                            panic!("got impossible sequence number")
                        }
                    }
                }
            }

            // if the amount of time elapsed has yet exceeded the specified timeout
            // set (timeout = timeout - elapsed time) and listen for another packet
            if Instant::now().duration_since(sent_time) > Duration::from_secs_f64(timeout) {
                return Ok(None);
            }
            else {
                loop_timeout =
                    Duration::from_secs_f64(timeout) - Instant::now().duration_since(sent_time)
            }
        }

    // ICMPv6
    }
    else {
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
            }
            else {
                loop_timeout =
                    Duration::from_secs_f64(timeout) - Instant::now().duration_since(sent_time)
            }
        }
    }
}

/// send ping requests in a loop and print the stats
///
/// # Arguments
///
/// * `config` - configs saved in a Config struct
///
/// # Errors
///
/// any error that implements the Error trait
///
/// # Examples
///
/// ```
/// run(Config::new(
///     {
///         let decorator = slog_term::TermDecorator::new().build();
///         let drain = Mutex::new(slog_term::FullFormat::new(decorator).build()).fuse();
///         slog::Logger::root(drain, o!())
///     },
///     "1.1.1.1",
///     4_u16,
///     1.0_f64,
///     1.0_f64,
/// ))
/// ```
pub fn run(config: Config) -> Result<(), Box<dyn Error>>
{
    // declare/initialize internal metric variables for the ping summary
    let identifier = random::<u16>();
    let mut sequence: u16 = 0;
    let mut total_rtt = Duration::new(0, 0);
    let mut transmitted = 0;
    let mut received = 0;
    let mut min: Option<Duration> = None;
    let mut max: Option<Duration> = None;

    // an atomic boolean value that acts as the running flag
    // this is used to stop the ping cycle when ^C is pressed
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // upon receiving ^C, set running to false
    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("error setting Ctrl-C handler");

    // keep sending pings until ^C is pressed or count is reached
    while running.load(Ordering::SeqCst) && (config.count == 0 || sequence < config.count) {
        // this timer is used to calculate interval
        let cycle_begin_time = Instant::now();

        // send one echo request and get the RTT value
        let rtt = match ping(config.destination, config.timeout, 64, sequence, identifier) {
            Ok(rtt) => rtt,
            Err(error) => {
                crit!(config.logger, "{}", error);
                return Err(error.into());
            }
        };

        match rtt {
            None => {
                warn!(
                    config.logger,
                    "no answer from {} seq={}", config.destination, sequence
                );
            }
            Some(rtt) => {
                // if min is not initialized, set min=millis
                // else compare and set accordingly
                if let Some(current_min) = min {
                    if rtt < current_min {
                        min = Some(rtt)
                    }
                }
                else {
                    min = Some(rtt)
                }

                // if max is not initialized, set max=millis
                // else compare and set accordingly
                if let Some(current_max) = max {
                    if rtt > current_max {
                        max = Some(rtt)
                    }
                }
                else {
                    max = Some(rtt)
                }

                info!(
                    config.logger,
                    "answer from {} seq={} rtt={}ms",
                    config.destination,
                    sequence,
                    paint_rtt(rtt.as_micros())
                );

                total_rtt += rtt;
                received += 1;
            }
        }
        transmitted += 1;
        sequence += 1;

        // if current time - elapsed time < interval, wait until interval is reached
        if Instant::now().duration_since(cycle_begin_time)
            < Duration::from_secs_f64(config.interval)
        {
            thread::sleep(
                Duration::from_secs_f64(config.interval)
                    - Instant::now().duration_since(cycle_begin_time),
            )
        }
    }

    // print final statistics
    info!(
        config.logger,
        "{}",
        Color::Fixed(240)
            .bold()
            .paint(format!("{} ping statistics", config.destination))
    );

    // calculate %loss
    let loss = if transmitted == 0 {
        100.0
    }
    else {
        ((transmitted - received) as f64 / transmitted as f64) * 100.0
    };

    info!(
        config.logger,
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
    }
    else {
        total_rtt.as_micros() / sequence as u128
    };

    info!(
        config.logger,
        "{}{}{}{}{}{}{}",
        Color::Fixed(240).bold().paint("min="),
        paint_rtt(final_min.as_micros()),
        Color::Fixed(240).bold().paint("ms max="),
        paint_rtt(final_max.as_micros()),
        Color::Fixed(240).bold().paint("ms avg="),
        paint_rtt(avg),
        Color::Fixed(240).bold().paint("ms")
    );

    // return an error if no successful responses were received
    if transmitted > 0 && received == 0 {
        return Err("no responses have been received".into());
    }

    Ok(())
}
