//! SSDP discovery and bounded `UPnP` description lookup. It asks the local multicast group once,
//! keeps only replies from the scanned host, and follows only an HTTP description URL whose
//! literal host is that reply's IPv4 address.

use std::collections::HashSet;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use super::multicast;

pub(super) const PORT: u16 = 1900;
const GROUP: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), PORT);
const HTTP_TIMEOUT: Duration = Duration::from_millis(750);
const MAX_DESCRIPTIONS: usize = 16;
const MAX_XML_BYTES: u64 = 256 * 1024;

const SEARCH: &[u8] = b"M-SEARCH * HTTP/1.1\r\n\
HOST: 239.255.255.250:1900\r\n\
MAN: \"ssdp:discover\"\r\n\
MX: 2\r\n\
ST: ssdp:all\r\n\r\n";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Evidence {
    pub(super) services: Vec<String>,
    pub(super) server: Option<String>,
    pub(super) manufacturer: Option<String>,
    pub(super) model: Option<String>,
    /// Complete response packets, retained only for the discovery debug view.
    pub(super) responses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Reply {
    ip: Ipv4Addr,
    service: Option<String>,
    server: Option<String>,
    location: Option<String>,
    raw: String,
}

/// Asks for every SSDP target and enriches a bounded number of unique local description URLs.
pub(super) async fn browse(adapter: Option<Ipv4Addr>) -> Vec<(Ipv4Addr, Evidence)> {
    tokio::task::spawn_blocking(move || browse_blocking(adapter))
        .await
        .unwrap_or_default()
}

fn browse_blocking(adapter: Option<Ipv4Addr>) -> Vec<(Ipv4Addr, Evidence)> {
    let local = adapter.unwrap_or(Ipv4Addr::UNSPECIFIED);
    let replies = multicast::ask(local, GROUP, &[SEARCH]);
    combine(
        replies
            .iter()
            .filter_map(|(ip, packet)| parse_reply(*ip, packet))
            .collect(),
    )
}

fn parse_reply(ip: Ipv4Addr, packet: &[u8]) -> Option<Reply> {
    let text = std::str::from_utf8(packet).ok()?;
    let mut lines = text.lines();
    lines
        .next()?
        .to_ascii_uppercase()
        .starts_with("HTTP/1.1 200")
        .then_some(())?;
    let header = |wanted: &str| {
        text.lines().skip(1).find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case(wanted)
                .then(|| value.trim().to_owned())
        })
    };
    Some(Reply {
        ip,
        service: header("ST").or_else(|| header("NT")),
        server: header("SERVER"),
        location: header("LOCATION"),
        raw: text.trim().to_owned(),
    })
}

fn combine(replies: Vec<Reply>) -> Vec<(Ipv4Addr, Evidence)> {
    let mut found: Vec<(Ipv4Addr, Evidence)> = Vec::new();
    let mut fetched = HashSet::new();
    for reply in replies {
        let position = found.iter().position(|(ip, _)| *ip == reply.ip);
        let index = position.unwrap_or_else(|| {
            found.push((reply.ip, Evidence::default()));
            found.len() - 1
        });
        let Some((_, evidence)) = found.get_mut(index) else {
            continue;
        };
        evidence.responses.push(reply.raw);
        if let Some(service) = reply.service.filter(|value| safe_text(value)) {
            evidence.services.push(service);
        }
        if evidence.server.is_none() {
            evidence.server = reply.server.filter(|value| safe_text(value));
        }
        let description = reply.location.filter(|location| {
            fetched.len() < MAX_DESCRIPTIONS
                && local_http_location(location, reply.ip)
                && fetched.insert(location.clone())
        });
        if let Some(description) = description.and_then(|location| fetch_description(&location)) {
            evidence.services.extend(description.services);
            evidence.manufacturer = evidence.manufacturer.take().or(description.manufacturer);
            evidence.model = evidence.model.take().or(description.model);
        }
    }
    for (_, evidence) in &mut found {
        evidence.services.sort_unstable();
        evidence.services.dedup();
    }
    found
}

fn local_http_location(location: &str, ip: Ipv4Addr) -> bool {
    let prefix = format!("http://{ip}");
    location
        .strip_prefix(&prefix)
        .is_some_and(|rest| rest.starts_with('/') || rest.starts_with(':'))
}

fn fetch_description(location: &str) -> Option<Evidence> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(2)))
        .timeout_connect(Some(HTTP_TIMEOUT))
        .timeout_recv_response(Some(HTTP_TIMEOUT))
        .build()
        .into();
    let mut response = agent.get(location).call().ok()?;
    let mut xml = String::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_XML_BYTES)
        .read_to_string(&mut xml)
        .ok()?;
    parse_description(&xml)
}

fn parse_description(xml: &str) -> Option<Evidence> {
    let document = roxmltree::Document::parse(xml).ok()?;
    let values = |name: &str| {
        document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == name)
            .filter_map(|node| node.text().map(str::trim))
            .filter(|value| safe_text(value))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let mut services = values("deviceType");
    services.extend(values("serviceType"));
    services.sort_unstable();
    services.dedup();
    Some(Evidence {
        services,
        server: None,
        manufacturer: values("manufacturer").into_iter().next(),
        model: values("modelName").into_iter().next(),
        responses: Vec::new(),
    })
}

fn safe_text(value: &str) -> bool {
    !value.is_empty() && value.chars().count() <= 253 && !value.chars().any(char::is_control)
}

pub(super) fn of_host(all: &[(Ipv4Addr, Evidence)], ip: IpAddr) -> Evidence {
    let IpAddr::V4(ip) = ip else {
        return Evidence::default();
    };
    all.iter()
        .find(|(address, _)| *address == ip)
        .map(|(_, evidence)| evidence.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_keeps_declared_fields() {
        let reply = parse_reply(
            Ipv4Addr::new(192, 168, 1, 1),
            b"HTTP/1.1 200 OK\r\nST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\nSERVER: RouterOS/1 UPnP/1.1\r\nLOCATION: http://192.168.1.1:80/root.xml\r\n\r\n",
        )
        .unwrap();
        assert_eq!(reply.server.as_deref(), Some("RouterOS/1 UPnP/1.1"));
        assert!(
            reply
                .raw
                .contains("ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1")
        );
        assert!(local_http_location(
            reply.location.as_deref().unwrap(),
            reply.ip
        ));
        assert!(!local_http_location(
            "http://example.com/root.xml",
            reply.ip
        ));
    }

    #[test]
    fn a_description_keeps_identity_and_capabilities() {
        let xml = r"<root><device><deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType><manufacturer>Acme</manufacturer><modelName>Box 2</modelName><serviceList><service><serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType></service></serviceList></device></root>";
        let evidence = parse_description(xml).unwrap();
        assert_eq!(evidence.manufacturer.as_deref(), Some("Acme"));
        assert_eq!(evidence.model.as_deref(), Some("Box 2"));
        assert_eq!(evidence.services.len(), 2);
    }
}
