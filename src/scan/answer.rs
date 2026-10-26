//! Turns what the scan found out about a host into an [`Answer`]: the device it looks like and
//! how it was found, with only what the scan's options asked for. Names, models and port
//! descriptions come from the host or the network, so they are untrusted text: one with control
//! characters is dropped and the rest are bounded.

use std::net::{IpAddr, Ipv4Addr};

use ipnet::Ipv4Net;
use netscli_core::{FoundBy, Host, MdnsService, NameSource, PortResult, Protocol};

use super::identify::{self, Evidence};
use super::mdns;
use super::ports::PortRegistry;
use super::ssdp;
use crate::model::{
    Answer, CardId, Device, DeviceId, DeviceKind, Icon, InterfaceKind, LastSeen, MacAddr, Method,
    NetworkCard, ProbeProtocol, ProbeRecord, ScanOptions, ServiceProtocol, ServiceSource, Services,
    Status,
};

/// The longest name kept, a DNS name's limit.
const MAX_NAME: usize = 253;
/// The longest description kept: a product and its version, a model, a maker.
const MAX_DESCRIPTION: usize = 60;
/// The card icons, among those that ship with the app, for a virtual adapter and for Wi-Fi.
const VIRTUAL_ICON: &str = "Network cards/Virtual";
const WIRELESS_ICON: &str = "Network cards/Wi-Fi";

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
    /// What it declared through SSDP and its `UPnP` description.
    pub(super) ssdp: ssdp::Evidence,
    /// Where it stands to the scanning computer's default gateway.
    pub(super) route: Route,
}

/// Where a host stands to the scanning computer's default gateway on the scanned subnet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Route {
    /// Any other host, or the gateway is not known.
    #[default]
    Other,
    /// It is the gateway.
    Gateway,
    /// It is the scanning computer, which sends through this gateway.
    ThisComputer(Ipv4Addr),
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
    let announced = announced(options.mdns_services, &probed.mdns);
    let (mdns_name, reverse_name) = heard_names(probed, announced, options.service_detection);
    let mdns_services = mdns::service_names(announced);
    let empty_ssdp = ssdp::Evidence::default();
    let ssdp = selected_ssdp(options.mdns_services, &probed.ssdp, &empty_ssdp);
    let mac = host
        .mac
        .as_deref()
        .and_then(|mac| mac.parse::<MacAddr>().ok());
    let heard_mdns = mdns_name.is_some() || !mdns_services.is_empty();
    let heard_ssdp = !ssdp.services.is_empty() || ssdp.server.is_some();
    let methods = methods(mac.is_some(), host.found_by, heard_mdns, heard_ssdp);
    let card = network_card(probed, subnet, options, names, mac);
    let model = mdns::model(announced)
        .or(ssdp.model.as_deref())
        .and_then(|m| clean(m, MAX_DESCRIPTION));
    let manufacturer = manufacturer(announced, ssdp);
    let (hostname, suggested_name) = name_from(mdns_name, reverse_name, model.as_deref(), mac, ip);
    let evidence = Evidence {
        ports: &probed.ports,
        mdns_services: &mdns_services,
        ssdp_services: &ssdp.services,
        model: model.as_deref(),
        ttl: probed.ttl,
        hostname: hostname.as_deref(),
        gateway: probed.route == Route::Gateway,
    };
    let (kind, kind_clue) = if options.device_type {
        identify::kind(&evidence)
    } else {
        (DeviceKind::Unknown, None)
    };
    let os = options.os_family.then(|| identify::os(&evidence)).flatten();
    let device = Device {
        kind,
        kind_clue,
        role: kind.default_role(),
        manufacturer,
        model,
        os,
        mdns_services,
        ssdp_services: ssdp.services.clone(),
        ssdp_server: ssdp.server.clone(),
        probe_log: ssdp_probe_log(ssdp),
        ..device(hostname, suggested_name, card)
    };
    Some(Answer { device, methods })
}

/// The names `probed` goes by over mDNS and in reverse DNS, when `detect` asks for names.
fn heard_names(
    probed: &Probed,
    announced: &[MdnsService],
    detect: bool,
) -> (Option<String>, Option<String>) {
    let host = &probed.host;
    let mdns_name = host
        .hostname
        .as_deref()
        .filter(|_| host.hostname_source == Some(NameSource::Mdns))
        .or_else(|| mdns::hostname(announced))
        .filter(|_| detect)
        .and_then(|name| clean(name, MAX_NAME));
    let reverse_name = probed.reverse_name.as_deref().filter(|_| detect);
    (
        mdns_name,
        reverse_name.and_then(|name| clean(name, MAX_NAME)),
    )
}

fn ssdp_probe_log(evidence: &ssdp::Evidence) -> Vec<ProbeRecord> {
    evidence
        .responses
        .iter()
        .map(|text| ProbeRecord {
            protocol: ProbeProtocol::Ssdp,
            port: ssdp::PORT,
            text: text.clone(),
        })
        .collect()
}

fn manufacturer(announced: &[MdnsService], ssdp: &ssdp::Evidence) -> Option<String> {
    mdns::maker(announced)
        .or(ssdp.manufacturer.as_deref())
        .and_then(|maker| clean(maker, MAX_DESCRIPTION))
}

fn selected_ssdp<'a>(
    enabled: bool,
    evidence: &'a ssdp::Evidence,
    empty: &'a ssdp::Evidence,
) -> &'a ssdp::Evidence {
    if enabled { evidence } else { empty }
}

fn network_card(
    probed: &Probed,
    subnet: Ipv4Net,
    options: ScanOptions,
    names: &PortRegistry,
    mac: Option<MacAddr>,
) -> NetworkCard {
    let vendor = probed
        .host
        .vendor
        .as_deref()
        .filter(|_| options.card_vendor);
    let address = match probed.host.ip {
        IpAddr::V4(ip) => Ipv4Net::new(ip, subnet.prefix_len()).ok(),
        IpAddr::V6(_) => None,
    };
    let gateway = match probed.route {
        Route::ThisComputer(gateway) => Some(gateway),
        Route::Gateway | Route::Other => None,
    };
    let (interface_kind, icon) = connection(mac);
    NetworkCard {
        interface_kind,
        address,
        gateway,
        mac,
        vendor: vendor.and_then(|value| clean(value, MAX_DESCRIPTION)),
        services: services(&probed.ports, options.service_detection, names),
        icon,
        ..NetworkCard::blank(CardId(0))
    }
}

/// What its MAC address says of a card: a hypervisor's or container engine's prefix makes it
/// a virtual adapter, and a randomised address is what phones and laptops use on Wi-Fi.
fn connection(mac: Option<MacAddr>) -> (InterfaceKind, Icon) {
    match mac {
        Some(mac) if mac.is_virtual() => (InterfaceKind::Virtual, Icon::new(VIRTUAL_ICON)),
        Some(mac) if mac.is_randomised() => (InterfaceKind::Physical, Icon::new(WIRELESS_ICON)),
        Some(_) | None => (
            InterfaceKind::Physical,
            Icon::new(NetworkCard::DEFAULT_ICON),
        ),
    }
}

fn announced(enabled: bool, services: &[MdnsService]) -> &[MdnsService] {
    if enabled { services } else { &[] }
}

fn name_from(
    mdns: Option<String>,
    reverse: Option<String>,
    model: Option<&str>,
    mac: Option<MacAddr>,
    ip: Ipv4Addr,
) -> (Option<String>, String) {
    let hostname = mdns.or(reverse);
    let name = suggested_name(hostname.as_deref(), model, mac, ip);
    (hostname, name)
}

/// How a host was found: its MAC known from the neighbour table, a ping reply, mDNS.
fn methods(mac: bool, found_by: FoundBy, mdns: bool, ssdp: bool) -> Vec<Method> {
    let found = [
        (mac, Method::Arp),
        (found_by == FoundBy::Probe, Method::Ping),
        (mdns, Method::Mdns),
        (ssdp, Method::Ssdp),
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
        kind_clue: None,
        role: DeviceKind::Unknown.default_role(),
        icon: None,
        status: Status::Online,
        host: None,
        cards: vec![card],
        manufacturer: None,
        model: None,
        hostname,
        os: None,
        mdns_services: Vec::new(),
        ssdp_services: Vec::new(),
        ssdp_server: None,
        probe_log: Vec::new(),
        last_seen: LastSeen::default(),
        notes: String::new(),
    }
}

fn suggested_name(
    hostname: Option<&str>,
    model: Option<&str>,
    mac: Option<MacAddr>,
    ip: Ipv4Addr,
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
            ssdp: ssdp::Evidence::default(),
            route: Route::Other,
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
    fn the_gateway_is_a_router_and_this_computer_keeps_its_own_gateway() {
        let mut gateway = probed(host(FoundBy::Probe, None), None);
        gateway.route = Route::Gateway;
        let router = answer(&gateway, home(), all(), &none()).unwrap().device;
        assert_eq!(
            (router.kind, router.kind_clue),
            (
                DeviceKind::Router,
                Some(crate::model::KindClue::DefaultGateway)
            )
        );
        assert_eq!(router.cards[0].gateway, None, "its own upstream is unknown");

        let mut this_computer = probed(host(FoundBy::Probe, None), None);
        this_computer.route = Route::ThisComputer(Ipv4Addr::new(192, 168, 1, 1));
        let device = answer(&this_computer, home(), all(), &none())
            .unwrap()
            .device;
        assert_eq!(device.cards[0].gateway, Some(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(device.kind_clue, None);
    }

    #[test]
    fn a_virtual_or_randomised_mac_tells_how_the_card_connects() {
        for (mac, kind, icon) in [
            ("52:54:00:77:01:10", InterfaceKind::Virtual, VIRTUAL_ICON),
            ("82:C5:00:39:61:21", InterfaceKind::Physical, WIRELESS_ICON),
            (
                "00:17:88:6E:6C:5C",
                InterfaceKind::Physical,
                NetworkCard::DEFAULT_ICON,
            ),
        ] {
            let mut found = host(FoundBy::Probe, None);
            found.mac = Some(mac.into());
            let device = answer(&probed(found, None), home(), all(), &none())
                .unwrap()
                .device;
            let card = &device.cards[0];
            assert_eq!(
                (card.interface_kind, card.icon.0.as_str()),
                (kind, icon),
                "{mac}"
            );
        }
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
    fn ssdp_and_upnp_give_identity_capabilities_and_method() {
        let mut router = probed(host(FoundBy::Probe, None), None);
        router.ssdp = ssdp::Evidence {
            services: vec!["urn:schemas-upnp-org:device:InternetGatewayDevice:1".into()],
            server: Some("RouterOS UPnP/1.1".into()),
            manufacturer: Some("Acme Networks".into()),
            model: Some("Home Gateway".into()),
            responses: vec!["HTTP/1.1 200 OK\r\nST: upnp:rootdevice".into()],
        };
        let answer = answer(&router, home(), all(), &none()).unwrap();
        assert!(answer.methods.contains(&Method::Ssdp));
        assert_eq!(answer.device.kind, DeviceKind::Router);
        assert_eq!(answer.device.name, "Home Gateway");
        assert_eq!(answer.device.manufacturer.as_deref(), Some("Acme Networks"));
        assert_eq!(
            answer.device.ssdp_server.as_deref(),
            Some("RouterOS UPnP/1.1")
        );
        assert_eq!(answer.device.ssdp_services, router.ssdp.services);
        assert_eq!(answer.device.probe_log.len(), router.ssdp.responses.len());
        assert_eq!(answer.device.probe_log[0].protocol, ProbeProtocol::Ssdp);
        assert_eq!(answer.device.probe_log[0].port, ssdp::PORT);
        assert_eq!(answer.device.probe_log[0].text, router.ssdp.responses[0]);
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
