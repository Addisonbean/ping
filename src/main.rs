use clap::{Arg, App};

use dns_lookup::lookup_host;

use pnet::packet::icmp::{
    echo_request::MutableEchoRequestPacket,
    IcmpTypes,
};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::util::checksum;
use pnet::packet::Packet;
use pnet::transport::{icmp_packet_iter, TransportChannelType::Layer4, TransportProtocol, TransportSender, transport_channel};

use std::io;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use std::thread::sleep;

fn make_ping_request<'a>(data: &'a mut [u8]) -> MutableEchoRequestPacket<'a> {
    let mut req = MutableEchoRequestPacket::new(data).unwrap();
    req.set_icmp_type(IcmpTypes::EchoRequest);

    req.set_identifier(0);
    req.set_sequence_number(0);

    req.set_checksum(0);
    let cs = checksum(req.packet(), 1);
    req.set_checksum(cs);

    req
}

fn send_ping(addr: IpAddr, data: &mut [u8], sender: &mut TransportSender) -> io::Result<usize> {
    let req = make_ping_request(data);
    sender.send_to(req, addr)
}

#[derive(Clone, Copy, Default)]
struct PingStats {
    num_sent: u64,
    num_received: u64,
    total_rtt: u128,
}

impl PingStats {
    fn avg_rtt(self) -> f64 {
        if self.num_received != 0 {
            self.total_rtt as f64 / self.num_received as f64
        } else {
            0.0
        }
    }

    fn total_percent_loss(self) -> f64 {
        1.0 - self.num_received as f64 / self.num_sent as f64
    }
}

fn print_stats_for_rtt(rtt: u128, stats: PingStats) {
    println!("Response received: {}ms rtt, {:.2} average rtt, {}% total loss",
        rtt,
        stats.avg_rtt(),
        stats.total_percent_loss() as u64 * 100,
    );
}

fn print_stats_for_timeout(stats: PingStats) {
    println!("Response timed out: {:.2} average rtt, {}% total loss",
        stats.avg_rtt(),
        stats.total_percent_loss() as u64 * 100,
    );
}

fn main() -> io::Result<()> {
    let matches = App::new("ping")
        .arg(Arg::with_name("address")
            .takes_value(true)
            .required(true)
        )
        .get_matches();

    let addr = matches.value_of("address").unwrap();

    let ip = match lookup_host(addr)?.iter().find(|a| matches!(a, IpAddr::V4(_)) ) {
        Some(&a) => a,
        None => panic!("ugh idk"), // TODO: what to do?
    };

    let protocol = Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Icmp));
    let (mut sender, mut receiver) = transport_channel(4096, protocol)?;
    sender.set_ttl(64)?;

    let mut packet_iter = icmp_packet_iter(&mut receiver);
    let mut data = [0; MutableEchoRequestPacket::minimum_packet_size()];

    let mut stats = PingStats::default();

    loop {
        send_ping(ip, &mut data, &mut sender)?;
        let time_sent = Instant::now();
        stats.num_sent += 1;

        // Unwrap? match? `?`?
        let res = packet_iter.next_with_timeout(Duration::from_millis(1000)).unwrap();
        let rtt = Instant::now().duration_since(time_sent).as_millis();

        match res {
            Some(_) => {
                stats.total_rtt += rtt;
                stats.num_received += 1;
                print_stats_for_rtt(rtt, stats);
            },
            None => print_stats_for_timeout(stats),
        }

        sleep(Duration::from_millis(500));
    }
}
