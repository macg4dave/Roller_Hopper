//! This computer's own subnet, which the network box starts with: the IPv4 subnet of a network
//! connection that is up, preferring the one the default route uses. And its own MAC addresses,
//! which a scan that finds it cannot learn from the neighbour table.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use ipnet::{IpNet, Ipv4Net};
use netscli_core::{Host, NetworkManager};

/// The narrowest subnet suggested; a /31 or /32 links to one other host, or none.
const NARROWEST: u8 = 30;
/// The widest subnet a run may scan (65,536 addresses). A wider one is narrowed to the
/// [`NARROWED`] block around this computer's address.
const WIDEST: u8 = 16;
const NARROWED: u8 = 24;

/// One usable IPv4 address on an active adapter and the safe subnet derived from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Adapter {
    pub(crate) name: String,
    pub(crate) address: Ipv4Addr,
    pub(crate) subnet: Ipv4Net,
}

impl Adapter {
    /// Adapter identity and address as one language-neutral dropdown value.
    pub(crate) fn label(&self) -> String {
        format!(
            "{} — {}/{}",
            self.name,
            self.address,
            self.subnet.prefix_len()
        )
    }
}

/// Active non-loopback adapters with usable IPv4 subnets. The adapter used by the default route
/// is first, so it is the initial primary adapter.
pub(crate) fn adapters() -> Vec<Adapter> {
    let (routers, interfaces) = interfaces();
    adapter_list(routed_address(), &routers, interfaces)
}

pub(crate) fn local_subnet() -> Option<Ipv4Net> {
    adapters().first().map(|adapter| adapter.subnet)
}

fn adapter_list(
    routed: Option<Ipv4Addr>,
    router_adapters: &HashSet<String>,
    interfaces: Vec<netscli_core::InterfaceInfo>,
) -> Vec<Adapter> {
    let mut found = Vec::new();
    for interface in interfaces
        .into_iter()
        .filter(|interface| interface.is_up && !interface.is_loopback)
    {
        for address in interface.ips {
            let IpNet::V4(address) = address else {
                continue;
            };
            let Some(subnet) = subnet(address) else {
                continue;
            };
            found.push(Adapter {
                name: interface.name.clone(),
                address: address.addr(),
                subnet,
            });
        }
    }
    found.sort_by_key(|adapter| {
        (
            !router_adapters.contains(&adapter.name),
            routed != Some(adapter.address),
        )
    });
    found
}

/// The names of the adapters with a router (gateway) address, and every adapter with its
/// addresses. Windows exposes gateways directly; elsewhere the routed local address remains
/// the portable default-route signal.
///
/// On Windows the adapters are read here rather than through netscli-core, which gives each
/// address the longest prefix Windows lists for it: always the address's own /32 host route,
/// which no subnet can be derived from.
#[cfg(windows)]
fn interfaces() -> (HashSet<String>, Vec<netscli_core::InterfaceInfo>) {
    use ipconfig::{IfType, OperStatus};

    // Without the adapter list there is nothing to offer; the network can still be typed.
    let Ok(adapters) = ipconfig::get_adapters() else {
        return (HashSet::new(), Vec::new());
    };
    let routers = adapters
        .iter()
        .filter(|adapter| {
            adapter
                .gateways()
                .iter()
                .any(|gateway| matches!(gateway, IpAddr::V4(ip) if !ip.is_unspecified()))
        })
        .map(|adapter| adapter.friendly_name().to_owned())
        .collect();
    let interfaces = adapters
        .iter()
        .map(|adapter| netscli_core::InterfaceInfo {
            name: adapter.friendly_name().to_owned(),
            mac: None,
            ips: adapter
                .ip_addresses()
                .iter()
                .filter_map(|ip| match ip {
                    IpAddr::V4(ip) => with_prefix(*ip, adapter.prefixes()).map(IpNet::V4),
                    IpAddr::V6(_) => None,
                })
                .collect(),
            is_up: adapter.oper_status() == OperStatus::IfOperStatusUp,
            is_loopback: adapter.if_type() == IfType::SoftwareLoopback,
        })
        .collect();
    (routers, interfaces)
}

#[cfg(not(windows))]
fn interfaces() -> (HashSet<String>, Vec<netscli_core::InterfaceInfo>) {
    (HashSet::new(), NetworkManager::get_interfaces())
}

/// `ip` with the prefix of the subnet it is on. Windows lists, for each address, its subnet,
/// its own /32 host route and its subnet's broadcast address, so the subnet is the longest
/// listed prefix short of a host route that holds it. One with no such prefix stays a /32.
#[cfg(windows)]
fn with_prefix(ip: Ipv4Addr, prefixes: &[(IpAddr, u32)]) -> Option<Ipv4Net> {
    let subnet = prefixes
        .iter()
        .filter_map(|(net, len)| match net {
            IpAddr::V4(net) => Some((*net, u8::try_from(*len).ok()?)),
            IpAddr::V6(_) => None,
        })
        .filter(|(net, len)| {
            (1..32).contains(len) && Ipv4Net::new(*net, *len).is_ok_and(|n| n.contains(&ip))
        })
        .map(|(_, len)| len)
        .max();
    Ipv4Net::new(ip, subnet.unwrap_or(32)).ok()
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

    fn interface(name: &str, addresses: &[&str]) -> netscli_core::InterfaceInfo {
        netscli_core::InterfaceInfo {
            name: name.into(),
            mac: None,
            ips: addresses.iter().map(|text| text.parse().unwrap()).collect(),
            is_up: true,
            is_loopback: false,
        }
    }

    #[test]
    fn the_default_route_adapter_is_first_and_every_adapter_remains_available() {
        let interfaces = vec![
            interface("Hyper-V", &["172.30.64.1/20"]),
            interface("VPN", &["10.188.4.98/16"]),
            interface("Ethernet", &["192.168.20.105/24"]),
        ];
        let routed = "10.188.4.98".parse().ok();
        let routers = HashSet::from(["Ethernet".into()]);
        let found = adapter_list(routed, &routers, interfaces);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].name, "Ethernet");
        assert_eq!(found[0].subnet, "192.168.20.0/24".parse().unwrap());
        assert!(found.iter().any(|adapter| adapter.name == "Hyper-V"));
        assert!(found.iter().any(|adapter| adapter.name == "VPN"));
    }

    #[test]
    fn a_subnet_wider_than_a_16_narrows_to_the_24_around_this_computer() {
        let found = adapter_list(
            None,
            &HashSet::new(),
            vec![interface("LAN", &["10.1.2.3/8"])],
        );
        assert_eq!(found[0].subnet, "10.1.2.0/24".parse().unwrap());
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
        let found = adapter_list(
            None,
            &HashSet::new(),
            vec![interface(
                "Unusable",
                &[
                    "169.254.7.1/16",
                    "127.0.0.1/8",
                    "10.9.0.2/32",
                    "10.9.0.4/31",
                ],
            )],
        );
        assert_eq!(found, Vec::<Adapter>::new());
    }

    #[cfg(windows)]
    #[test]
    fn an_address_takes_its_subnet_not_its_own_host_route() {
        let prefix = |net: &str, len| (net.parse().unwrap(), len);
        let listed = [
            prefix("192.168.20.0", 24),
            prefix("192.168.20.105", 32),
            prefix("192.168.20.255", 32),
            prefix("224.0.0.0", 4),
            prefix("255.255.255.255", 32),
            prefix("0.0.0.0", 0),
        ];
        let lan = "192.168.20.105".parse().unwrap();
        assert_eq!(with_prefix(lan, &listed), "192.168.20.105/24".parse().ok());
        let tunnel = "10.9.0.2".parse().unwrap();
        let alone = [prefix("10.9.0.2", 32)];
        assert_eq!(with_prefix(tunnel, &alone), "10.9.0.2/32".parse().ok());
    }
}
