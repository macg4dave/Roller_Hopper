//! Asking the local multicast group a question out of one adapter and hearing who answers: the
//! shape SSDP and WS-Discovery share. The listen is bounded in time, replies and packet size.

use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, Socket, Type};

const LISTEN: Duration = Duration::from_secs(3);
const RECEIVE_SLICE: Duration = Duration::from_millis(200);
const MAX_REPLIES: usize = 128;
const MAX_PACKET: usize = 16 * 1024;

/// Sends each of `messages` to `group` out of the adapter at `local` (any adapter when
/// unspecified), then listens for replies, each with the address it came from.
pub(super) fn ask(
    local: Ipv4Addr,
    group: SocketAddrV4,
    messages: &[&[u8]],
) -> Vec<(Ipv4Addr, Vec<u8>)> {
    match send(local, group, messages) {
        Some(socket) => receive(&socket),
        None => Vec::new(),
    }
}

/// Binding to the adapter's address sets only the source address: the operating system sends
/// multicast out of the adapter with the cheapest multicast route, often a VPN tunnel, unless
/// the socket names the interface.
fn send(local: Ipv4Addr, group: SocketAddrV4, messages: &[&[u8]]) -> Option<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).ok()?;
    socket.bind(&SocketAddr::from((local, 0)).into()).ok()?;
    socket.set_multicast_if_v4(&local).ok()?;
    socket.set_multicast_ttl_v4(4).ok()?;
    socket.set_read_timeout(Some(RECEIVE_SLICE)).ok()?;
    for message in messages {
        socket
            .send_to(message, &SocketAddr::V4(group).into())
            .ok()?;
    }
    Some(socket.into())
}

fn receive(socket: &UdpSocket) -> Vec<(Ipv4Addr, Vec<u8>)> {
    let started = Instant::now();
    let mut replies = Vec::new();
    let mut packet = vec![0_u8; MAX_PACKET];
    while started.elapsed() < LISTEN && replies.len() < MAX_REPLIES {
        match socket.recv_from(&mut packet) {
            Ok((size, source)) => {
                let IpAddr::V4(ip) = source.ip() else {
                    continue;
                };
                if let Some(data) = packet.get(..size) {
                    replies.push((ip, data.to_vec()));
                }
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
    }
    replies
}
