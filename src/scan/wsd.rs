//! WS-Discovery: a multicast Probe on UDP 3702, answered by Windows computers, WSD printers and
//! scanners, and ONVIF cameras and recorders. Each `ProbeMatch` is kept as the endpoint, types,
//! scopes and transport addresses it declared, for the debug view; nothing it names is fetched.

use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::time::{SystemTime, UNIX_EPOCH};

use super::multicast;

pub(super) const PORT: u16 = 3702;
const GROUP: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), PORT);
/// An empty Probe matches every device; many ONVIF cameras answer only one naming their type.
const TYPES: [&str; 2] = [
    "",
    r#"<d:Types xmlns:dn="http://www.onvif.org/ver10/network/wsdl">dn:NetworkVideoTransmitter</d:Types>"#,
];
/// The longest declared value kept; scopes and addresses are URIs.
const MAX_VALUE: usize = 512;

/// Probes once for every device and once for cameras, and keeps each distinct match by the
/// address it came from.
pub(super) async fn browse(adapter: Option<Ipv4Addr>) -> Vec<(Ipv4Addr, String)> {
    tokio::task::spawn_blocking(move || browse_blocking(adapter))
        .await
        .unwrap_or_default()
}

fn browse_blocking(adapter: Option<Ipv4Addr>) -> Vec<(Ipv4Addr, String)> {
    let probes: Vec<String> = TYPES
        .iter()
        .zip(0_u32..)
        .map(|(types, salt)| probe(types, &message_id(salt)))
        .collect();
    let messages: Vec<&[u8]> = probes.iter().map(String::as_bytes).collect();
    let local = adapter.unwrap_or(Ipv4Addr::UNSPECIFIED);
    let mut found: Vec<(Ipv4Addr, String)> = Vec::new();
    for (ip, packet) in multicast::ask(local, GROUP, &messages) {
        for record in matches(&packet) {
            if !found.iter().any(|(at, seen)| *at == ip && *seen == record) {
                found.push((ip, record));
            }
        }
    }
    found
}

fn probe(types: &str, id: &str) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" "#,
            r#"xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" "#,
            r#"xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery">"#,
            "<s:Header><a:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</a:To>",
            "<a:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</a:Action>",
            "<a:MessageID>{id}</a:MessageID></s:Header>",
            "<s:Body><d:Probe>{types}</d:Probe></s:Body></s:Envelope>"
        ),
        id = id,
        types = types
    )
}

/// A message id in UUID form; devices only echo it, so it needs to be unique, not random.
fn message_id(salt: u32) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let hex = format!("{:032x}", nanos ^ (u128::from(salt) << 96));
    let part = |from: usize, to: usize| hex.get(from..to).unwrap_or_default();
    let (a, b, c, d, e) = (part(0, 8), part(8, 12), part(12, 16), part(16, 20), part(20, 32));
    format!("urn:uuid:{a}-{b}-{c}-{d}-{e}")
}

/// Each `ProbeMatch` in a reply, as `field: value` lines in WS-Discovery's element names.
fn matches(packet: &[u8]) -> Vec<String> {
    let Ok(text) = std::str::from_utf8(packet) else {
        return Vec::new();
    };
    let Ok(document) = roxmltree::Document::parse(text) else {
        return Vec::new();
    };
    document
        .descendants()
        .filter(|node| node.tag_name().name() == "ProbeMatch")
        .map(|found| {
            let value = |name: &str| {
                found
                    .descendants()
                    .find(|node| node.tag_name().name() == name)
                    .and_then(|node| node.text())
                    .map(clean)
                    .filter(|value| !value.is_empty())
            };
            let mut lines = Vec::new();
            if let Some(address) = value("Address") {
                lines.push(format!("EndpointReference: {address}"));
            }
            for name in ["Types", "XAddrs", "MetadataVersion"] {
                if let Some(text) = value(name) {
                    lines.push(format!("{name}: {text}"));
                }
            }
            if let Some(scopes) = value("Scopes") {
                lines.push("Scopes:".to_owned());
                lines.extend(scopes.split_whitespace().map(|scope| format!("  {scope}")));
            }
            lines.join("\n")
        })
        .filter(|record| !record.is_empty())
        .collect()
}

/// Network text, with runs of white space and control characters made single spaces, bounded.
fn clean(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    words
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_VALUE)
        .collect()
}

pub(super) fn of_host(all: &[(Ipv4Addr, String)], ip: IpAddr) -> Vec<String> {
    let IpAddr::V4(ip) = ip else {
        return Vec::new();
    };
    all.iter()
        .filter(|(address, _)| *address == ip)
        .map(|(_, record)| record.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probe_is_well_formed_and_carries_its_id_and_types() {
        let id = message_id(1);
        assert!(id.starts_with("urn:uuid:") && id.len() == 45, "{id}");
        assert_ne!(id, message_id(2));
        for types in TYPES {
            let message = probe(types, &id);
            let document = roxmltree::Document::parse(&message).unwrap();
            let probe = document.descendants().find(|n| n.has_tag_name("Probe"));
            assert!(probe.is_some());
            assert!(message.contains(&id));
        }
    }

    #[test]
    fn a_probe_match_keeps_its_declared_fields() {
        let reply = br#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery">
<soap:Body><wsd:ProbeMatches><wsd:ProbeMatch>
<wsa:EndpointReference><wsa:Address>urn:uuid:1234</wsa:Address></wsa:EndpointReference>
<wsd:Types>dn:NetworkVideoTransmitter tds:Device</wsd:Types>
<wsd:Scopes>onvif://www.onvif.org/name/Camera
 onvif://www.onvif.org/hardware/DS-2CD</wsd:Scopes>
<wsd:XAddrs>http://192.168.1.64/onvif/device_service</wsd:XAddrs>
<wsd:MetadataVersion>1</wsd:MetadataVersion>
</wsd:ProbeMatch></wsd:ProbeMatches></soap:Body></soap:Envelope>"#;
        let found = matches(reply);
        assert_eq!(
            found,
            ["EndpointReference: urn:uuid:1234\n\
              Types: dn:NetworkVideoTransmitter tds:Device\n\
              XAddrs: http://192.168.1.64/onvif/device_service\n\
              MetadataVersion: 1\n\
              Scopes:\n  onvif://www.onvif.org/name/Camera\n  onvif://www.onvif.org/hardware/DS-2CD"]
        );
        assert_eq!(matches(b"not xml"), Vec::<String>::new());
    }
}
