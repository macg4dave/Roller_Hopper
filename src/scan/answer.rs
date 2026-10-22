//! Turns what the scan found out about a host into an [`Answer`]: the device it looks like and
//! how it was found. Names and port descriptions come from the host or the network, so they are
//! untrusted text: one with control characters is dropped and the rest are bounded.

use std::net::IpAddr;

use ipnet::Ipv4Net;
use netscli_core::{FoundBy, Host, NameSource, PortResult};

use crate::model::{
    Answer, CardId, Device, DeviceId, DeviceKind, LastSeen, MacAddr, Method, NetworkCard, Services,
    Status,
};

/// The longest name kept, a DNS name's limit.
const MAX_NAME: usize = 253;
/// The longest port description kept, such as a product and its version.
const MAX_DESCRIPTION: usize = 60;

/// One host as the sweep found it, and what probing it found out.
pub(super) struct Probed {
    pub(super) host: Host,
    /// Its name in reverse DNS, looked up when the sweep had none from mDNS.
    pub(super) reverse_name: Option<String>,
    /// Its TCP ports probed, open or not.
    pub(super) ports: Vec<PortResult>,
}

/// The answer `probed` gives a scan of `subnet`: a device of unknown kind with one card, at its
/// address on the subnet. Without `identify` (service detection) its name and the software on
/// its ports are left out. `None` for a host that is not an IPv4 address on the subnet.
pub(super) fn answer(probed: Probed, subnet: Ipv4Net, identify: bool) -> Option<Answer> {
    let Probed {
        host,
        reverse_name,
        ports,
    } = probed;
    let IpAddr::V4(ip) = host.ip else {
        return None;
    };
    if !subnet.contains(&ip) {
        return None;
    }
    let mdns_name = host
        .hostname
        .filter(|_| identify && host.hostname_source == Some(NameSource::Mdns))
        .and_then(|name| clean(&name, MAX_NAME));
    let reverse_name = reverse_name.filter(|_| identify);
    let mut methods = Vec::new();
    let mac = host.mac.and_then(|mac| mac.parse::<MacAddr>().ok());
    if mac.is_some() {
        methods.push(Method::Arp);
    }
    if host.found_by == FoundBy::Probe {
        methods.push(Method::Ping);
    }
    if mdns_name.is_some() {
        methods.push(Method::Mdns);
    }
    let hostname = mdns_name.or_else(|| clean(&reverse_name?, MAX_NAME));
    let card = NetworkCard {
        address: Ipv4Net::new(ip, subnet.prefix_len()).ok(),
        mac,
        services: services(&ports, identify),
        ..NetworkCard::blank(CardId(0))
    };
    Some(Answer {
        device: device(hostname, card),
        methods,
    })
}

/// A newly found device with `card`, named after the first part of its `hostname`. Its ids are
/// placeholders: it gets real ones when it is added to the map.
fn device(hostname: Option<String>, card: NetworkCard) -> Device {
    let name = hostname.as_deref().and_then(|h| h.split('.').next());
    Device {
        id: DeviceId(0),
        name: name.unwrap_or_default().to_owned(),
        kind: DeviceKind::Unknown,
        role: DeviceKind::Unknown.default_role(),
        icon: None,
        status: Status::Online,
        cards: vec![card],
        guest: None,
        manufacturer: None,
        model: None,
        hostname,
        last_seen: LastSeen::default(),
        notes: String::new(),
    }
}

/// The open ports, each described by the software that named itself on it when `identify`
/// allows, or else by the service usual on that port.
fn services(ports: &[PortResult], identify: bool) -> Services {
    let mut services = Services::default();
    for port in ports.iter().filter(|p| p.open) {
        let named = identify.then(|| port.product_and_version()).flatten();
        let description = named.or_else(|| port.service.clone());
        let description = description.and_then(|d| clean(&d, MAX_DESCRIPTION));
        services.add(port.port, description.as_deref().unwrap_or_default());
    }
    services
}

/// `text` trimmed, without a DNS name's final dot, and cut to `max` characters; `None` when
/// empty or carrying control characters, which no honest name or description has.
fn clean(text: &str, max: usize) -> Option<String> {
    let text = text.trim().trim_end_matches('.').trim_end();
    if text.is_empty() || text.chars().any(char::is_control) {
        return None;
    }
    Some(text.chars().take(max).collect())
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use netscli_core::{PortStatus, Protocol};

    use super::*;

    fn home() -> Ipv4Net {
        "192.168.1.0/24".parse().unwrap()
    }

    fn host(found_by: FoundBy, hostname: Option<&str>) -> Host {
        Host {
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
            hostname: hostname.map(str::to_owned),
            mac: Some("00:17:88:6E:6C:5C".into()),
            vendor: Some("Philips Lighting BV".into()),
            rtt_ms: Some(4),
            found_by,
            hostname_source: hostname.map(|_| NameSource::Mdns),
        }
    }

    fn port(port: u16, open: bool, service: &str, product: Option<&str>) -> PortResult {
        PortResult {
            port,
            protocol: Protocol::Tcp,
            open,
            status: if open {
                PortStatus::Open
            } else {
                PortStatus::Closed
            },
            service: Some(service.into()),
            product: product.map(str::to_owned),
            version: product.map(|_| "9.6".to_owned()),
            latency_ms: None,
            banner: None,
            http: None,
            tls: None,
            raw: None,
            error: None,
        }
    }

    fn probed(host: Host, reverse_name: Option<&str>) -> Probed {
        Probed {
            host,
            reverse_name: reverse_name.map(str::to_owned),
            ports: vec![
                port(22, true, "ssh", Some("OpenSSH")),
                port(80, true, "http", None),
                port(443, false, "https", None),
            ],
        }
    }

    #[test]
    fn a_host_answers_with_its_address_mac_name_and_open_ports() {
        let lamp = probed(host(FoundBy::Probe, Some("lamp.local.")), None);
        let answer = answer(lamp, home(), true).unwrap();
        assert_eq!(answer.methods, [Method::Arp, Method::Ping, Method::Mdns]);
        let device = &answer.device;
        assert_eq!(device.name, "lamp");
        assert_eq!(device.hostname.as_deref(), Some("lamp.local"));
        assert_eq!(
            (device.kind, device.manufacturer.as_deref()),
            (DeviceKind::Unknown, None),
            "a card's vendor is not the device's maker"
        );
        let card = &device.cards[0];
        assert_eq!(card.address, "192.168.1.20/24".parse().ok());
        assert_eq!(card.mac, "00:17:88:6E:6C:5C".parse().ok());
        assert_eq!(card.services.to_string(), "22 (OpenSSH 9.6), 80 (http)");
    }

    #[test]
    fn without_service_detection_names_and_software_stay_unknown() {
        let lamp = probed(host(FoundBy::Probe, Some("lamp.local")), Some("lamp.home"));
        let answer = answer(lamp, home(), false).unwrap();
        assert_eq!(answer.methods, [Method::Arp, Method::Ping]);
        assert_eq!(
            (answer.device.name.as_str(), answer.device.hostname),
            ("", None)
        );
        let services = answer.device.cards[0].services.to_string();
        assert_eq!(services, "22 (ssh), 80 (http)", "the usual service only");
    }

    #[test]
    fn a_neighbour_that_did_not_answer_is_named_by_reverse_dns() {
        let nas = probed(host(FoundBy::Neighbor, None), Some("nas.home.arpa"));
        let answer = answer(nas, home(), true).unwrap();
        assert_eq!(answer.methods, [Method::Arp]);
        assert_eq!(answer.device.name, "nas");
    }

    #[test]
    fn untrusted_names_and_descriptions_are_dropped_or_bounded() {
        let mut sneaky = probed(host(FoundBy::Probe, Some("evil\u{1b}[2Khost")), None);
        let long = "x".repeat(500);
        sneaky.ports = vec![port(8080, true, "http-alt", Some(&long))];
        let answer = answer(sneaky, home(), true).unwrap();
        assert_eq!(answer.device.hostname, None);
        assert!(
            !answer.methods.contains(&Method::Mdns),
            "its name was not used"
        );
        let services = answer.device.cards[0].services.to_string();
        assert_eq!(services.len(), "8080 ()".len() + MAX_DESCRIPTION);
    }

    #[test]
    fn a_host_off_the_subnet_or_not_on_ipv4_is_no_answer() {
        let elsewhere = probed(host(FoundBy::Probe, None), None);
        assert!(answer(elsewhere, "10.0.0.0/24".parse().unwrap(), true).is_none());
        let mut v6 = host(FoundBy::Probe, None);
        v6.ip = "fd00::5".parse().unwrap();
        assert!(answer(probed(v6, None), home(), true).is_none());
    }
}
