use pnet::packet::icmp::{
    echo_request::MutableEchoRequestPacket,
    IcmpTypes,
};
use pnet::packet::{
    icmpv6::{MutableIcmpv6Packet, Icmpv6Types},
    ip::IpNextHeaderProtocols,
    Packet,
    util::checksum,
};
use pnet::transport::{
    icmp_packet_iter,
    IcmpTransportChannelIterator,
    icmpv6_packet_iter,
    Icmpv6TransportChannelIterator,
    transport_channel,
    TransportChannelType::Layer4,
    TransportProtocol,
    TransportReceiver,
    TransportSender,
};

use std::io;
use std::net::IpAddr;
use std::time::Duration;

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

pub enum PacketIter<'a> {
    V4(IcmpTransportChannelIterator<'a>),
    V6(Icmpv6TransportChannelIterator<'a>),
}

impl<'a> PacketIter<'a> {
    pub fn next_with_timeout(&mut self, t: Duration) -> io::Result<bool> {
        match self {
            PacketIter::V4(iter) => iter.next_with_timeout(t).map(|v| v.is_some()),
            PacketIter::V6(iter) => iter.next_with_timeout(t).map(|v| v.is_some()),
        }
    }
}

pub fn packet_iter<'a>(addr: IpAddr, receiver: &'a mut TransportReceiver) -> PacketIter<'a> {
    match addr {
        IpAddr::V4(_) => PacketIter::V4(icmp_packet_iter(receiver)),
        IpAddr::V6(_) => PacketIter::V6(icmpv6_packet_iter(receiver)),
    }
}

pub fn create_channels(addr: IpAddr) -> io::Result<(TransportSender, TransportReceiver)> {
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

pub fn send_ping(addr: IpAddr, data: &mut [u8], sender: &mut TransportSender) -> io::Result<usize> {
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
