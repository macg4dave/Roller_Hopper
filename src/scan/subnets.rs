//! The subnets this computer is on, which a scan may target: the IPv4 subnets of its network
//! connections that are up, with the one its default route uses first.

use ipnet::{IpNet, Ipv4Net};
use netscli_core::NetworkManager;

/// The narrowest subnet offered; a /31 or /32 links to one other host, or none.
const NARROWEST: u8 = 30;
/// The widest subnet a run may scan (65,536 addresses). A wider one is narrowed to the
/// [`NARROWED`] block around this computer's address.
const WIDEST: u8 = 16;
const NARROWED: u8 = 24;

pub(crate) fn local_subnets() -> Vec<Ipv4Net> {
    let interfaces = NetworkManager::get_interfaces();
    let up = interfaces.iter().filter(|i| i.is_up && !i.is_loopback);
    let addresses = up.flat_map(|i| &i.ips).filter_map(|ip| match ip {
        IpNet::V4(v4) => Some(*v4),
        IpNet::V6(_) => None,
    });
    subnets(netscli_core::detect_default_ipv4_subnet(), addresses)
}

/// The subnets to offer for this computer's `addresses` (each with its prefix length), each
/// once, those within the `default` route's subnet first.
fn subnets(default: Option<Ipv4Net>, addresses: impl Iterator<Item = Ipv4Net>) -> Vec<Ipv4Net> {
    let mut subnets: Vec<Ipv4Net> = addresses.filter_map(subnet).collect();
    subnets.sort_by_key(|s| !default.is_some_and(|d| d.contains(s)));
    let mut unique = Vec::with_capacity(subnets.len());
    for subnet in subnets {
        if !unique.contains(&subnet) {
            unique.push(subnet);
        }
    }
    unique
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
    fn each_connection_offers_its_subnet_once_with_the_default_first() {
        let addresses = nets(&[
            "172.20.16.1/20",
            "192.168.1.42/24",
            "192.168.1.43/24",
            "10.8.0.5/16",
        ]);
        let default = "192.168.1.0/24".parse().ok();
        assert_eq!(
            subnets(default, addresses.into_iter()),
            nets(&["192.168.1.0/24", "172.20.16.0/20", "10.8.0.0/16"])
        );
    }

    #[test]
    fn a_subnet_wider_than_a_16_narrows_to_the_24_around_this_computer() {
        let addresses = nets(&["10.1.2.3/8"]);
        let default = "10.0.0.0/8".parse().ok();
        assert_eq!(
            subnets(default, addresses.into_iter()),
            nets(&["10.1.2.0/24"])
        );
    }

    #[test]
    fn link_local_loopback_and_point_to_point_addresses_offer_nothing() {
        let addresses = nets(&[
            "169.254.7.1/16",
            "127.0.0.1/8",
            "10.9.0.2/32",
            "10.9.0.4/31",
        ]);
        assert_eq!(subnets(None, addresses.into_iter()), []);
    }
}
