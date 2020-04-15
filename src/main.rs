use clap::{Arg, App};
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
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default)]
struct PingStats {
    num_sent: u64,
    num_received: u64,
    total_rtt: u128,
}

fn make_ping_request<'a>(data: &'a mut [u8]) -> MutableEchoRequestPacket<'a> {
    let mut req = MutableEchoRequestPacket::new(data).unwrap();
    req.set_icmp_type(IcmpTypes::EchoRequest);

    // req.set_identifier(0);
    // req.set_sequence_number(0);

    req.set_checksum(0);
    let cs = checksum(req.packet(), 1);
    req.set_checksum(cs);

    req
}

fn send_ping(hostname: IpAddr, data: &mut [u8], sender: &mut TransportSender) -> io::Result<usize> {
    let req = make_ping_request(data);
    sender.send_to(req, hostname)
}


fn print_stats_for_rtt(rtt: u128, stats: PingStats) {
    let avg_rtt = stats.total_rtt as f64 / stats.num_received as f64;
    let total_loss = 1.0 - stats.num_received as f64 / stats.num_sent as f64;
    println!(
        "Response received: {}ms rtt, {:.2} average rtt, {:.2}% total loss",
        rtt,
        avg_rtt,
        total_loss,
    );
}

fn main() -> io::Result<()> {
    let matches = App::new("ping")
        .arg(Arg::with_name("hostname")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("ip")
            .takes_value(true)
            .required(true)
        )
        .get_matches();

    let hostname = matches.value_of("hostname").unwrap();
    let ip = matches.value_of("ip").unwrap();

    println!("Hostname: {}", hostname);
    println!("Ip: {}", ip);

    let protocol = Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Icmp));
    let (mut sender, mut receiver) = transport_channel(4096, protocol)?;
    sender.set_ttl(64)?;

    let stop_signal_sent = Arc::new(AtomicBool::new(false));
    let stop_signal_sent_handler = Arc::clone(&stop_signal_sent);
    ctrlc::set_handler(move || {
        stop_signal_sent_handler.store(true, atomic::Ordering::SeqCst);
    }).expect("Could not set SIGINT/SIGTERM handler.");

    let mut packet_iter = icmp_packet_iter(&mut receiver);
    let mut data = [0; MutableEchoRequestPacket::minimum_packet_size()];

    let mut stats = PingStats::default();

    // TODO: check for failed parse
    let hostname = hostname.parse().unwrap();
    loop {
        if stop_signal_sent.load(atomic::Ordering::SeqCst) { break }

        send_ping(hostname, &mut data, &mut sender)?;
        let time_sent = Instant::now();
        stats.num_sent += 1;

        // The unix ping program can be terminated here...
        // It doesn't wait for a response to terminate and print the summary
        if stop_signal_sent.load(atomic::Ordering::SeqCst) { break }

        let res = packet_iter.next();
        stats.num_received += 1;
        let rtt = Instant::now().duration_since(time_sent).as_millis();
        stats.total_rtt += rtt;

        print_stats_for_rtt(rtt, stats);

        // TODO: can I allow it to be interrupted from sleeping and break immediately?
        thread::sleep(Duration::from_millis(500));
    }

    // -- Print summary

    // println!("Num sent: {}", num_sent);
    // println!("Num recv: {}", num_received);

    Ok(())
}
