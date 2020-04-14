use clap::{Arg, App};
use pnet::packet::icmp::{
    echo_request::MutableEchoRequestPacket,
    IcmpTypes,
};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::util::checksum;
use pnet::packet::Packet;
use pnet::transport::{icmp_packet_iter, TransportChannelType::Layer4, TransportProtocol, transport_channel};

use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic;
use std::thread;

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

    // -- Create channels --

    // TODO: why 4096? At least put it in a constant...
    let protocol = Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Icmp));
    let (mut sender, mut receiver) = transport_channel(4096, protocol)?;
    sender.set_ttl(64)?;

    // -- Create packet --

    let mut buffer = [0; MutableEchoRequestPacket::minimum_packet_size()];
    let mut req = MutableEchoRequestPacket::new(&mut buffer).unwrap();
    req.set_icmp_type(IcmpTypes::EchoRequest);
    // req.set_identifier(0);
    // req.set_sequence_number(0);

    req.set_checksum(0);
    let cs = checksum(req.packet(), 1);
    req.set_checksum(cs);

    // -- Send it

    // TODO: check for failed parse
    sender.send_to(req, hostname.parse().unwrap())?;

    let mut num_received = Arc::new(AtomicU64::new(0));

    let mut thread_num_received = Arc::clone(&num_received);
    let tmp = thread::spawn(move || {
        let mut iter = icmp_packet_iter(&mut receiver);

        thread_num_received.fetch_add(1, atomic::Ordering::SeqCst);

        println!("{:?}", iter.next());
    });

    tmp.join().unwrap();

    // -- Print summary

    println!("Num received: {}", num_received.load(atomic::Ordering::SeqCst));

    Ok(())
}
