//! This computer's own subnet, which the network box starts with: the IPv4 subnet of a network
//! connection that is up, preferring the one the default route uses. And its own MAC addresses,
//! which a scan that finds it cannot learn from the neighbour table.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use ipnet::{IpNet, Ipv4Net};
use netscli_core::{Host, NetworkManager};

/// The narrowest subnet suggested; a /31 or /32 links to one other host, or none.
const NARROWEST: u8 = 30;
/// The widest subnet a run may scan (65,536 addresses). A wider one is narrowed to the
/// [`NARROWED`] block around this computer's address.
const WIDEST: u8 = 16;
const NARROWED: u8 = 24;

pub(crate) fn local_subnet() -> Option<Ipv4Net> {
    let interfaces = NetworkManager::get_interfaces();
    let up = interfaces.iter().filter(|i| i.is_up && !i.is_loopback);
    let addresses = up.flat_map(|i| &i.ips).filter_map(|ip| match ip {
        IpNet::V4(v4) => Some(*v4),
        IpNet::V6(_) => None,
    });
    let default = netscli_core::detect_default_ipv4_subnet();
    best(routed_address(), default, addresses)
}

/// This computer's own addresses, each with the MAC address of the connection it is on.
pub(super) fn own_macs() -> Vec<(IpAddr, String)> {
    let interfaces = NetworkManager::get_interfaces();
    let connections = interfaces.iter().filter(|i| !i.is_loopback);
    connections
        .filter_map(|i| Some((i, i.mac?.to_string())))
        .flat_map(|(i, mac)| i.ips.iter().map(move |ip| (ip.addr(), mac.clone())))
        .collect()
}

/// Gives each of `hosts` that is this computer, at one of its `own` addresses, the MAC address
/// of its connection there. Its own neighbour table never lists itself, so the sweep finds it
/// without one, and it would not match itself on the map.
pub(super) fn add_own_macs(hosts: &mut [Host], own: &[(IpAddr, String)]) {
    for host in hosts.iter_mut().filter(|h| h.mac.is_none()) {
        let mine = own.iter().find(|(ip, _)| *ip == host.ip);
        host.mac = mine.map(|(_, mac)| mac.clone());
    }
}

/// The address this computer sends from to the internet, as its routing table says; none when
/// it has no route (offline), and the other clues decide. Connecting a UDP socket only looks
/// the route up; nothing is sent.
fn routed_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // A documentation address (TEST-NET-1): routed like any other, never reached.
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        IpAddr::V4(_) | IpAddr::V6(_) => None,
    }
}

/// The subnet to suggest for this computer's `addresses` (each with its prefix length): that of
/// the `routed` address, which outgoing traffic leaves from, else the first within the
/// `default` subnet (the first connection's, which may be a virtual or VPN adapter), or else
/// the first at all.
fn best(
    routed: Option<Ipv4Addr>,
    default: Option<Ipv4Net>,
    addresses: impl Iterator<Item = Ipv4Net>,
) -> Option<Ipv4Net> {
    let subnets: Vec<Ipv4Net> = addresses.filter_map(subnet).collect();
    let on_route = subnets
        .iter()
        .find(|s| routed.is_some_and(|ip| s.contains(&ip)));
    let in_default = || {
        subnets
            .iter()
            .find(|s| default.is_some_and(|d| d.contains(*s)))
    };
    on_route
        .or_else(in_default)
        .or(subnets.first())
        .copied()
        .or_else(|| default.and_then(subnet))
}

/// The subnet to scan for one of this computer's addresses; none for a link-local address or a
/// point-to-point link.
fn subnet(address: Ipv4Net) -> Option<Ipv4Net> {
    let ip = address.addr();
    if ip.is_link_local() || ip.is_loopback() || address.prefix_len() > NARROWEST {
        return None;
    }
    let prefix = if address.prefix_len() < WIDEST {
        NARROWED
    } else {
        address.prefix_len()
    };
    Ipv4Net::new(ip, prefix).ok().map(|net| net.trunc())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets(texts: &[&str]) -> Vec<Ipv4Net> {
        texts.iter().map(|t| t.parse().unwrap()).collect()
    }

    #[test]
    fn the_subnet_traffic_leaves_from_is_suggested_before_virtual_and_vpn_ones() {
        // As on the user's PC: Hyper-V, VPN and WSL adapters alongside the Ethernet one.
        let addresses = nets(&[
            "172.30.64.1/20",
            "10.188.4.98/16",
            "192.168.20.105/24",
            "172.21.240.1/20",
        ]);
        let lan = "192.168.20.0/24".parse().ok();
        let routed = "192.168.20.105".parse().ok();
        let vpn = "10.188.0.0/16".parse().ok();
        assert_eq!(best(routed, vpn, addresses.clone().into_iter()), lan);
        assert_eq!(
            best(None, lan, addresses.clone().into_iter()),
            lan,
            "no route: the default subnet"
        );
        let first = "172.30.64.0/20".parse().ok();
        assert_eq!(
            best(None, None, addresses.into_iter()),
            first,
            "neither found"
        );
    }

    #[test]
    fn the_default_route_is_the_fallback_when_interfaces_have_no_ipv4_address() {
        let default = "192.168.20.0/24".parse().ok();
        assert_eq!(best(None, default, std::iter::empty()), default);
    }

    #[test]
    fn a_subnet_wider_than_a_16_narrows_to_the_24_around_this_computer() {
        let addresses = nets(&["10.1.2.3/8"]);
        let default = "10.0.0.0/8".parse().ok();
        assert_eq!(
            best(None, default, addresses.into_iter()),
            "10.1.2.0/24".parse().ok()
        );
    }

    #[test]
    fn this_computer_takes_its_own_mac_and_others_keep_theirs() {
        let host = |ip: &str, mac: Option<&str>| Host {
            ip: ip.parse().unwrap(),
            hostname: None,
            mac: mac.map(str::to_owned),
            vendor: None,
            rtt_ms: None,
            found_by: netscli_core::FoundBy::Probe,
            hostname_source: None,
        };
        let mut hosts = [
            host("192.168.20.105", None),
            host("192.168.20.1", Some("00:11:22:33:44:55")),
            host("192.168.20.7", None),
        ];
        let own = [(hosts[0].ip, "AA:BB:CC:DD:EE:FF".to_owned())];
        add_own_macs(&mut hosts, &own);
        let macs: Vec<_> = hosts.iter().map(|h| h.mac.as_deref()).collect();
        let expected = [Some("AA:BB:CC:DD:EE:FF"), Some("00:11:22:33:44:55"), None];
        assert_eq!(macs, expected);
    }

    #[test]
    fn link_local_loopback_and_point_to_point_addresses_suggest_nothing() {
        let addresses = nets(&[
            "169.254.7.1/16",
            "127.0.0.1/8",
            "10.9.0.2/32",
            "10.9.0.4/31",
        ]);
        assert_eq!(best(None, None, addresses.into_iter()), None);
    }
}
