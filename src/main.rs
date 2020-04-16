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

    fn print_stats_for_rtt(self, rtt: u128) {
        println!("Response received: {}ms rtt, {} average rtt, {:.2}% total loss",
            rtt,
            self.avg_rtt(),
            self.total_percent_loss() * 100.0,
        );
    }

    fn print_stats_for_timeout(self) {
        println!("Response timed out: {} average rtt, {:.2}% total loss",
            self.avg_rtt(),
            self.total_percent_loss() * 100.0,
        );
    }
}

fn ping_app() -> io::Result<()> {
    let config = App::new("ping")
        .arg(Arg::with_name("address")
            .takes_value(true)
            .required(true)
            .help("The ip or hostname to ping")
        )
        .arg(Arg::with_name("ttl")
            .takes_value(true)
            .required(false)
            .help("The time to live for the icmp echo request, in seconds")
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
        .get_matches();

    let addr = config.value_of("address").unwrap();
    let ttl = config.value_of("ttl")
        .map(str::parse::<u8>)
        .unwrap_or(Ok(DEFAULT_TTL))
        .map_err(|_|
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "The value for the 'ttl' flag must be an integer between 0 and 255",
            )
        )?;

    let ips = lookup_host(addr)?;
    let ip =
        if config.is_present("ipv4") {
            ips.into_iter().find(IpAddr::is_ipv4)
        } else if config.is_present("ipv6") {
            ips.into_iter().find(IpAddr::is_ipv6)
        } else {
            ips.get(0).cloned()
        }
        .ok_or_else(||
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("The hostname '{}' could not be found.", addr),
            )
        )?;

    println!("Sending pings to {}...", ip);

    let (mut sender, mut receiver) = create_channels(ip, ttl)?;
    let mut packet_iter = packet_iter(ip, &mut receiver);

    let mut data = [0; PACKET_DATA_SIZE];
    let mut stats = PingStats::default();

    loop {
        send_ping(ip, &mut data, &mut sender)?;

        let time_sent = Instant::now();
        stats.num_sent += 1;

        let success = packet_iter.next_with_timeout(Duration::from_millis(1000))?;
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
}

fn main() {
    if let Err(e) = ping_app() {
        eprintln!("Error: {}", e);
        exit(1);
    }
}
