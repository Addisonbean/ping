use clap::{Arg, App};

use dns_lookup::lookup_host;

use std::io;
use std::process::exit;
use std::time::{Duration, Instant};
use std::thread::sleep;

mod ping;
use ping::{send_ping, packet_iter, create_channels};

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

    // Make these methods?
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
    let matches = App::new("ping")
        .arg(Arg::with_name("address")
            .takes_value(true)
            .required(true)
            .help("The ip or hostname to ping")
        )
        .get_matches();

    let addr = matches.value_of("address").unwrap();

    let ip = lookup_host(addr)?
        .get(0)
        .cloned()
        .ok_or_else(||
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("The hostname '{}' could not be found.", addr),
            )
        )?;


    let (mut sender, mut receiver) = create_channels(ip)?;

    let mut packet_iter = packet_iter(ip, &mut receiver);

    // TODO: at least make a constant or something
    let mut data = [0; 64];

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
