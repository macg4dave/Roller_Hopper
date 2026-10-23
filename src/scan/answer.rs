//! Turns what the scan found out about a host into an [`Answer`]: the device it looks like and
//! how it was found, with only what the scan's options asked for. Names, models and port
//! descriptions come from the host or the network, so they are untrusted text: one with control
//! characters is dropped and the rest are bounded.

use std::net::IpAddr;

use ipnet::Ipv4Net;
use netscli_core::{FoundBy, Host, MdnsService, NameSource, PortResult, Protocol};

use super::identify::{self, Evidence};
use super::mdns;
use super::ports::PortRegistry;
use crate::model::{
    Answer, CardId, Device, DeviceId, DeviceKind, LastSeen, MacAddr, Method, NetworkCard,
    ScanOptions, ServiceProtocol, ServiceSource, Services, Status,
};

/// The longest name kept, a DNS name's limit.
const MAX_NAME: usize = 253;
/// The longest description kept: a product and its version, a model, a maker.
const MAX_DESCRIPTION: usize = 60;

/// One host as the sweep found it, and what probing it found out.
pub(super) struct Probed {
    pub(super) host: Host,
    /// Its name in reverse DNS, looked up when mDNS gave none.
    pub(super) reverse_name: Option<String>,
    /// Its TCP ports probed, open or not.
    pub(super) ports: Vec<PortResult>,
    /// The time-to-live of its ping reply, when the OS family was asked for.
    pub(super) ttl: Option<u8>,
    /// What it announced over mDNS.
    pub(super) mdns: Vec<MdnsService>,
}

/// The answer `probed` gives a scan of `subnet` with `options`: a device with one card, at its
/// address on the subnet. Open ports nothing else names take IANA's `names`. `None` for a host
/// that is not an IPv4 address on the subnet.
pub(super) fn answer(
    probed: &Probed,
    subnet: Ipv4Net,
    options: ScanOptions,
    names: &PortRegistry,
) -> Option<Answer> {
    let IpAddr::V4(ip) = probed.host.ip else {
        return None;
    };
    if !subnet.contains(&ip) {
        return None;
    }
    let host = &probed.host;
    let detect = options.service_detection;
    let announced = announced(options.mdns_services, &probed.mdns);
    let mdns_name = host
        .hostname
        .as_deref()
        .filter(|_| host.hostname_source == Some(NameSource::Mdns))
        .or_else(|| mdns::hostname(announced))
        .filter(|_| detect)
        .and_then(|name| clean(name, MAX_NAME));
    let reverse_name = probed.reverse_name.as_deref().filter(|_| detect);
    let reverse_name = reverse_name.and_then(|name| clean(name, MAX_NAME));
    let mdns_services = mdns::service_names(announced);
    let mac = host
        .mac
        .as_deref()
        .and_then(|mac| mac.parse::<MacAddr>().ok());
    let heard = mdns_name.is_some() || !mdns_services.is_empty();
    let methods = methods(mac.is_some(), host.found_by, heard);
    let vendor = host.vendor.as_deref().filter(|_| options.card_vendor);
    let card = NetworkCard {
        address: Ipv4Net::new(ip, subnet.prefix_len()).ok(),
        mac,
        vendor: vendor.and_then(|v| clean(v, MAX_DESCRIPTION)),
        services: services(&probed.ports, detect, names),
        ..NetworkCard::blank(CardId(0))
    };
    let model = mdns::model(announced).and_then(|m| clean(m, MAX_DESCRIPTION));
    let evidence = Evidence {
        ports: &probed.ports,
        mdns_services: &mdns_services,
        model: model.as_deref(),
        ttl: probed.ttl,
    };
    let kind = if options.device_type {
        identify::kind(&evidence)
    } else {
        DeviceKind::Unknown
    };
    let os = options.os_family.then(|| identify::os(&evidence)).flatten();
    let (hostname, suggested_name) = name_from(mdns_name, reverse_name, model.as_deref(), mac, ip);
    let device = Device {
        kind,
        role: kind.default_role(),
        manufacturer: mdns::maker(announced).and_then(|m| clean(m, MAX_DESCRIPTION)),
        model,
        os,
        mdns_services,
        ..device(hostname, suggested_name, card)
    };
    Some(Answer { device, methods })
}

fn announced(enabled: bool, services: &[MdnsService]) -> &[MdnsService] {
    if enabled { services } else { &[] }
}

fn name_from(
    mdns: Option<String>,
    reverse: Option<String>,
    model: Option<&str>,
    mac: Option<MacAddr>,
    ip: std::net::Ipv4Addr,
) -> (Option<String>, String) {
    let hostname = mdns.or(reverse);
    let name = suggested_name(hostname.as_deref(), model, mac, ip);
    (hostname, name)
}

/// How a host was found: its MAC known from the neighbour table, a ping reply, mDNS.
fn methods(mac: bool, found_by: FoundBy, mdns: bool) -> Vec<Method> {
    let found = [
        (mac, Method::Arp),
        (found_by == FoundBy::Probe, Method::Ping),
        (mdns, Method::Mdns),
    ];
    found
        .into_iter()
        .filter(|&(by, _)| by)
        .map(|(_, m)| m)
        .collect()
}

/// A newly found device of unknown kind with `card`, named after the first part of its
/// `hostname`. Its ids are placeholders: it gets real ones when it is added to the map.
fn device(hostname: Option<String>, name: String, card: NetworkCard) -> Device {
    Device {
        id: DeviceId(0),
        name,
        kind: DeviceKind::Unknown,
        role: DeviceKind::Unknown.default_role(),
        icon: None,
        status: Status::Online,
        cards: vec![card],
        manufacturer: None,
        model: None,
        hostname,
        os: None,
        mdns_services: Vec::new(),
        last_seen: LastSeen::default(),
        notes: String::new(),
    }
}

fn suggested_name(
    hostname: Option<&str>,
    model: Option<&str>,
    mac: Option<MacAddr>,
    ip: std::net::Ipv4Addr,
) -> String {
    if let Some(name) = hostname
        .and_then(|h| h.split('.').next())
        .filter(|n| !n.is_empty())
    {
        return name.to_owned();
    }
    if let Some(model) = model.filter(|m| !m.is_empty()) {
        return model.to_owned();
    }
    if let Some(mac) = mac {
        return format!("device-{:02X}{:02X}", mac.0[4], mac.0[5]);
    }
    format!("host-{}", ip.to_string().replace('.', "-"))
}

/// The open ports, each described by the software that named itself on it when `identify`
/// allows, or else by the service usual on that port: the scanner's name for it, or IANA's.
fn services(ports: &[PortResult], identify: bool, names: &PortRegistry) -> Services {
    let mut services = Services::default();
    for port in ports.iter().filter(|p| p.open) {
        let protocol = match port.protocol {
            Protocol::Tcp => ServiceProtocol::Tcp,
            Protocol::Udp => ServiceProtocol::Udp,
        };
        let software = identify.then(|| port.product_and_version()).flatten();
        let usual = port
            .service
            .as_deref()
            .or_else(|| names.get(protocol, port.port));
        let source = if software.is_some() {
            ServiceSource::Detected
        } else if usual.is_some() {
            ServiceSource::Registry
        } else {
            ServiceSource::Open
        };
        let description = software.or_else(|| usual.map(str::to_owned));
        let description = description.and_then(|d| clean(&d, MAX_DESCRIPTION));
        services.add_sourced(
            port.port,
            protocol,
            description.as_deref().unwrap_or_default(),
            source,
        );
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
            ttl: None,
            mdns: Vec::new(),
        }
    }

    /// No IANA names, so ports keep only the scanner's.
    fn none() -> PortRegistry {
        PortRegistry::default()
    }

    /// Every option on.
    fn all() -> ScanOptions {
        ScanOptions::default()
    }

    /// Only which ports are open.
    fn bare() -> ScanOptions {
        ScanOptions {
            service_detection: false,
            mdns_services: false,
            device_type: false,
            os_family: false,
            card_vendor: false,
            ..ScanOptions::default()
        }
    }

    fn printer_announcement() -> MdnsService {
        MdnsService {
            full_name: "Brother._ipp._tcp.local.".into(),
            hostname: "brother.local.".into(),
            service_type: "_ipp._tcp.local.".into(),
            addresses: vec!["192.168.1.20".parse().unwrap()],
            port: 631,
            properties: [
                ("ty".to_owned(), "Brother HL-L2350DW series".to_owned()),
                ("usb_MFG".to_owned(), "Brother".to_owned()),
            ]
            .into(),
        }
    }

    #[test]
    fn a_host_answers_with_its_address_mac_name_and_open_ports() {
        let lamp = probed(host(FoundBy::Probe, Some("lamp.local.")), None);
        let answer = answer(&lamp, home(), all(), &none()).unwrap();
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
        assert_eq!(card.vendor.as_deref(), Some("Philips Lighting BV"));
        assert_eq!(card.services.to_string(), "22 (OpenSSH 9.6), 80 (http)");
    }

    #[test]
    fn announcements_give_the_model_maker_services_and_kind() {
        let mut brother = probed(host(FoundBy::Probe, None), None);
        brother.mdns = vec![printer_announcement()];
        brother.ttl = Some(255);
        let answer = answer(&brother, home(), all(), &none()).unwrap();
        assert!(answer.methods.contains(&Method::Mdns));
        let device = &answer.device;
        assert_eq!(device.mdns_services, ["ipp"]);
        assert_eq!(device.model.as_deref(), Some("Brother HL-L2350DW series"));
        assert_eq!(device.manufacturer.as_deref(), Some("Brother"));
        assert_eq!(device.kind, DeviceKind::Printer);
        assert_eq!(device.role, DeviceKind::Printer.default_role());
        let os = device.os.as_ref().map(|os| os.family);
        assert_eq!(os, Some(crate::model::OsFamily::Network), "TTL 255");
    }

    #[test]
    fn each_detail_is_found_out_only_when_its_option_is_on() {
        let mut brother = probed(host(FoundBy::Probe, Some("brother.local")), Some("b.home"));
        brother.mdns = vec![printer_announcement()];
        brother.ttl = Some(255);
        let answer = answer(&brother, home(), bare(), &none()).unwrap();
        assert_eq!(answer.methods, [Method::Arp, Method::Ping]);
        let device = &answer.device;
        assert_eq!(
            (device.name.as_str(), &device.hostname),
            ("device-6C5C", &None)
        );
        assert_eq!((&device.model, &device.manufacturer), (&None, &None));
        assert_eq!((device.kind, &device.os), (DeviceKind::Unknown, &None));
        assert_eq!(device.mdns_services, Vec::<String>::new());
        let card = &device.cards[0];
        assert_eq!(card.vendor, None);
        assert_eq!(
            card.services.to_string(),
            "22 (ssh), 80 (http)",
            "the usual service only"
        );
    }

    #[test]
    fn a_host_the_sweep_did_not_name_takes_the_name_it_announced_over_reverse_dns() {
        let mut brother = probed(host(FoundBy::Probe, None), Some("b.home"));
        brother.mdns = vec![printer_announcement()];
        let answer = answer(&brother, home(), all(), &none()).unwrap();
        assert_eq!(answer.device.name, "brother");
        assert_eq!(answer.device.hostname.as_deref(), Some("brother.local"));
    }

    #[test]
    fn a_neighbour_that_did_not_answer_is_named_by_reverse_dns() {
        let nas = probed(host(FoundBy::Neighbor, None), Some("nas.home.arpa"));
        let answer = answer(&nas, home(), all(), &none()).unwrap();
        assert_eq!(answer.methods, [Method::Arp]);
        assert_eq!(answer.device.name, "nas");
    }

    #[test]
    fn untrusted_names_and_descriptions_are_dropped_or_bounded() {
        let mut sneaky = probed(host(FoundBy::Probe, Some("evil\u{1b}[2Khost")), None);
        let long = "x".repeat(500);
        sneaky.ports = vec![port(8080, true, "http-alt", Some(&long))];
        let answer = answer(&sneaky, home(), all(), &none()).unwrap();
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
        let lab = "10.0.0.0/24".parse().unwrap();
        assert!(answer(&elsewhere, lab, all(), &none()).is_none());
        let mut v6 = host(FoundBy::Probe, None);
        v6.ip = "fd00::5".parse().unwrap();
        assert!(answer(&probed(v6, None), home(), all(), &none()).is_none());
    }

    #[test]
    fn an_open_port_the_scanner_does_not_name_takes_its_iana_name() {
        let mut host = probed(host(FoundBy::Probe, None), None);
        let mut database = port(5432, true, "", None);
        database.service = None;
        host.ports = vec![database];
        let answer = answer(&host, home(), all(), &PortRegistry::load()).unwrap();
        let services = answer.device.cards[0].services.to_string();
        assert_eq!(services, "5432 (postgres)");
        assert_eq!(
            answer.device.cards[0]
                .services
                .iter()
                .next()
                .unwrap()
                .source,
            ServiceSource::Registry
        );
    }

    #[test]
    fn names_and_port_sources_preserve_what_the_scan_actually_knew() {
        let unnamed = probed(host(FoundBy::Probe, None), None);
        let answer = answer(&unnamed, home(), all(), &none()).unwrap();
        assert_eq!(answer.device.name, "device-6C5C");
        let sources: Vec<_> = answer.device.cards[0]
            .services
            .iter()
            .map(|service| service.source)
            .collect();
        assert_eq!(sources, [ServiceSource::Detected, ServiceSource::Registry]);
    }
}
