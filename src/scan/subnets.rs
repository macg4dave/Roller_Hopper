//! This computer's own subnet, which the network box starts with: the IPv4 subnet of a network
//! connection that is up, preferring the one the default route uses.

use ipnet::{IpNet, Ipv4Net};
use netscli_core::NetworkManager;

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
    best(netscli_core::detect_default_ipv4_subnet(), addresses)
}

/// The subnet to suggest for this computer's `addresses` (each with its prefix length): the
/// first within the `default` route's subnet, or else the first at all.
fn best(default: Option<Ipv4Net>, addresses: impl Iterator<Item = Ipv4Net>) -> Option<Ipv4Net> {
    let subnets: Vec<Ipv4Net> = addresses.filter_map(subnet).collect();
    let routed = subnets
        .iter()
        .find(|s| default.is_some_and(|d| d.contains(*s)));
    routed
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
    fn the_default_routes_subnet_is_suggested_before_virtual_ones() {
        let addresses = nets(&["172.30.64.1/20", "192.168.20.105/24", "172.21.240.1/20"]);
        let default = "192.168.20.0/24".parse().ok();
        let best = best(default, addresses.clone().into_iter());
        assert_eq!(best, "192.168.20.0/24".parse().ok());
        let first = "172.30.64.0/20".parse().ok();
        assert_eq!(
            super::best(None, addresses.into_iter()),
            first,
            "no default route"
        );
    }

    #[test]
    fn the_default_route_is_the_fallback_when_interfaces_have_no_ipv4_address() {
        let default = "192.168.20.0/24".parse().ok();
        assert_eq!(best(default, std::iter::empty()), default);
    }

    #[test]
    fn a_subnet_wider_than_a_16_narrows_to_the_24_around_this_computer() {
        let addresses = nets(&["10.1.2.3/8"]);
        let default = "10.0.0.0/8".parse().ok();
        assert_eq!(
            best(default, addresses.into_iter()),
            "10.1.2.0/24".parse().ok()
        );
    }

    #[test]
    fn link_local_loopback_and_point_to_point_addresses_suggest_nothing() {
        let addresses = nets(&[
            "169.254.7.1/16",
            "127.0.0.1/8",
            "10.9.0.2/32",
            "10.9.0.4/31",
        ]);
        assert_eq!(best(None, addresses.into_iter()), None);
    }
}
