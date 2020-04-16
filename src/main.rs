use clap::{Arg, App};

use dns_lookup::lookup_host;

use pnet::packet::icmp::{
    echo_request::MutableEchoRequestPacket,
    IcmpTypes,
};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::util::checksum;
use pnet::packet::Packet;
use pnet::packet::icmpv6::{MutableIcmpv6Packet, Icmpv6Types};
use pnet::transport::{icmp_packet_iter, icmpv6_packet_iter, TransportChannelType::Layer4, TransportProtocol, TransportSender, TransportReceiver, transport_channel, Icmpv6TransportChannelIterator, IcmpTransportChannelIterator};

use std::io;
use std::net::IpAddr;
use std::process::exit;
use std::time::{Duration, Instant};
use std::thread::sleep;

fn make_icmp_ping_request<'a>(data: &'a mut [u8]) -> MutableEchoRequestPacket<'a> {
    let mut req = MutableEchoRequestPacket::new(data).expect("Data provided to packet was too small");
    req.set_icmp_type(IcmpTypes::EchoRequest);

    req.set_identifier(0);
    req.set_sequence_number(0);

    req.set_checksum(0);
    let cs = checksum(req.packet(), 1);
    req.set_checksum(cs);

    req
}

fn make_icmpv6_ping_request<'a>(data: &'a mut [u8]) -> MutableIcmpv6Packet<'a> {
    let mut req = MutableIcmpv6Packet::new(data).expect("Data provided to packet was too small");
    req.set_icmpv6_type(Icmpv6Types::EchoRequest);

    req.set_checksum(0);
    let cs = checksum(req.packet(), 1);
    req.set_checksum(cs);

    req
}

#[derive(Clone, Copy, Debug, Default)]
struct PingStats {
    num_sent: u64,
    num_received: u64,
    total_rtt: u128,
}

impl PingStats {
    fn avg_rtt(self) -> u128 {
        if self.num_received != 0 {
            self.total_rtt / self.num_received as u128
        } else {
            0
        }
    }

    fn total_percent_loss(self) -> f64 {
        1.0 - self.num_received as f64 / self.num_sent as f64
    }
}

fn print_stats_for_rtt(rtt: u128, stats: PingStats) {
    println!("Response received: {}ms rtt, {} average rtt, {:.2}% total loss",
        rtt,
        stats.avg_rtt(),
        stats.total_percent_loss() * 100.0,
    );
}

fn print_stats_for_timeout(stats: PingStats) {
    println!("Response timed out: {} average rtt, {:.2}% total loss",
        stats.avg_rtt(),
        stats.total_percent_loss() * 100.0,
    );
}

enum PacketIter<'a> {
    V4(IcmpTransportChannelIterator<'a>),
    V6(Icmpv6TransportChannelIterator<'a>),
}

impl<'a> PacketIter<'a> {
    fn next_with_timeout(&mut self, t: Duration) -> io::Result<bool> {
        match self {
            PacketIter::V4(iter) => iter.next_with_timeout(t).map(|r| r.is_some()),
            PacketIter::V6(iter) => iter.next_with_timeout(t).map(|r| r.is_some()),
        }
    }
}

fn create_channels(addr: IpAddr) -> io::Result<(TransportSender, TransportReceiver)> {
    Ok(match addr {
        IpAddr::V4(_) => {
            let protocol = Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Icmp));
            let (mut sender, receiver) = transport_channel(4096, protocol)?;
            sender.set_ttl(64)?;

            (sender, receiver)
        },
        IpAddr::V6(_) => {
            let protocol = Layer4(TransportProtocol::Ipv6(IpNextHeaderProtocols::Icmpv6));
            let (sender, receiver) = transport_channel(4096, protocol)?;

            (sender, receiver)
        },
    })
}

fn packet_iter<'a>(addr: IpAddr, receiver: &'a mut TransportReceiver) -> PacketIter<'a> {
    match addr {
        IpAddr::V4(_) => PacketIter::V4(icmp_packet_iter(receiver)),
        IpAddr::V6(_) => PacketIter::V6(icmpv6_packet_iter(receiver)),
    }
}

fn send_ping(addr: IpAddr, data: &mut [u8], sender: &mut TransportSender) -> io::Result<usize> {
    match addr {
        IpAddr::V4(_) => {
            let req = make_icmp_ping_request(data);
            sender.send_to(req, addr)
        },
        IpAddr::V6(_) => {
            let req = make_icmpv6_ping_request(data);
            sender.send_to(req, addr)
        },
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
            print_stats_for_rtt(rtt, stats);
        } else {
            print_stats_for_timeout(stats);
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
