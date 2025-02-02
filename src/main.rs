use clap::{App, Arg};

use dns_lookup::lookup_host;

use std::io;
use std::net::IpAddr;
use std::process::exit;
use std::thread::sleep;
use std::time::{Duration, Instant};

mod ping;
use ping::{create_channels, PACKET_DATA_SIZE, packet_iter, send_ping};

const DEFAULT_TTL: u8 = 64;
const DEFAULT_WAIT: u64 = 2;

#[derive(Clone, Copy, Debug, Default)]
struct PingStats {
    num_sent: u64,
    num_received: u64,
    total_rtt: u128,
}

impl PingStats {
    pub fn avg_rtt(self) -> u128 {
        if self.num_received != 0 {
            self.total_rtt / self.num_received as u128
        } else {
            0
        }
    }

    pub fn total_percent_loss(self) -> f64 {
        1.0 - self.num_received as f64 / self.num_sent as f64
    }

    pub fn total_lost(self) -> u64 {
        self.num_sent - self.num_received
    }

    fn print_stats_for_rtt(self, rtt: u128) {
        println!("Response received: {}ms rtt, {} average rtt, {}/{} lost ({:.2}%)",
            rtt,
            self.avg_rtt(),
            self.total_lost(),
            self.num_sent,
            self.total_percent_loss() * 100.0,
        );
    }

    fn print_stats_for_timeout(self) {
        println!("Response timed out: {} average rtt, {}/{} lost ({:.2}%)",
            self.avg_rtt(),
            self.total_lost(),
            self.num_sent,
            self.total_percent_loss() * 100.0,
        );
    }
}

fn ping_app() -> io::Result<()> {
    let ttl_help = format!("The time to live for the icmp echo request, in seconds. Default is {}.", DEFAULT_TTL);
    let timeout_help = format!("The number of seconds to wait for a reply. Default is {}.", DEFAULT_WAIT);

    let config = App::new("ping")
        .arg(Arg::with_name("address")
            .takes_value(true)
            .required(true)
            .help("The ip or hostname to ping")
        )
        .arg(Arg::with_name("ttl")
            .takes_value(true)
            .required(false)
            .help(&ttl_help)
            .short("t")
            .long("ttl")
        )
        .arg(Arg::with_name("ipv4")
            .takes_value(false)
            .required(false)
            .help("Force ping to use IPv4.")
            .short("4")
            .conflicts_with("ipv6")
        )
        .arg(Arg::with_name("ipv6")
            .takes_value(false)
            .required(false)
            .help("Force ping to use IPv6.")
            .short("6")
        )
        .arg(Arg::with_name("timeout")
            .takes_value(true)
            .required(false)
            .help(&timeout_help)
            .short("W")
            .long("wait")
        )
        .arg(Arg::with_name("packet_count")
            .takes_value(true)
            .required(false)
            .help("Stop sending packets after <packet_count> packets have been sent.")
            .short("c")
            .long("count")
        )
        .get_matches();

    let host = config.value_of("address").unwrap();
    let addrs = lookup_host(host)?;
    let addr =
        if config.is_present("ipv4") {
            addrs.into_iter().find(IpAddr::is_ipv4)
        } else if config.is_present("ipv6") {
            addrs.into_iter().find(IpAddr::is_ipv6)
        } else {
            addrs.get(0).cloned()
        }
        .ok_or_else(||
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("The hostname '{}' could not be found.", host),
            )
        )?;

    let ttl = config.value_of("ttl")
        .map(str::parse)
        .unwrap_or(Ok(DEFAULT_TTL))
        .map_err(|_|
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "The value for the 'ttl' flag must be an integer between 0 and 255",
            )
        )?;

    let timeout = config.value_of("timeout")
        .map(str::parse)
        .unwrap_or(Ok(DEFAULT_WAIT))
        .map_err(|_|
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "The value for the 'timeout' flag must be a positive integer.",
            )
        )?;

    let packets_to_send = config.value_of("packet_count")
        .map(str::parse::<u64>)
        .transpose()
        .map_err(|_|
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "The value for the 'packet_count' flag must be a positive integer.",
            )
        )?;

    println!("Sending pings to {}...", addr);

    start_pings(addr, ttl, timeout, packets_to_send)
}

fn start_pings(addr: IpAddr, ttl: u8, timeout: u64, packets_to_send: Option<u64>) -> io::Result<()> {
    let (mut sender, mut receiver) = create_channels(addr, ttl)?;
    let mut packets = packet_iter(addr, &mut receiver);

    let mut data = [0; PACKET_DATA_SIZE];
    let mut stats = PingStats::default();

    loop {
        if packets_to_send.map(|c| stats.num_sent >= c).unwrap_or(false) {
            break;
        }

        send_ping(addr, &mut data, &mut sender)?;

        let time_sent = Instant::now();
        stats.num_sent += 1;

        let success = packets.next_with_timeout(Duration::from_secs(timeout))?;
        let rtt = Instant::now().duration_since(time_sent).as_millis();

        if success {
            stats.total_rtt += rtt;
            stats.num_received += 1;
            stats.print_stats_for_rtt(rtt);
        } else {
            stats.print_stats_for_timeout();
        }

        sleep(Duration::from_millis(500));
    }

    Ok(())
}

fn main() {
    if let Err(e) = ping_app() {
        eprintln!("Error: {}", e);
        exit(1);
    }
}
