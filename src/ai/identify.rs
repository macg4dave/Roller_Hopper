//! Import with AI's identification: the evidence a found host and its network cards give the
//! model, and its answer put in place of what the scan made of the host. The answer must hold
//! the default prompt's fields with their allowed values; a field the model is not confident
//! about keeps the scan's value, and anything else in the answer is ignored.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::response;
use crate::model::{
    Device, DeviceKind, Method, NetworkCard, OsClue, OsFamily, OsHint, ServiceProtocol,
};

/// The most open ports listed for one card, so a host with thousands open still fits the
/// model's context.
const MAX_PORTS: usize = 64;
/// The longest description kept (the agreed AI fields), and the longest name (the prompt's).
const MAX_DESCRIPTION: usize = 120;
const MAX_NAME: usize = 35;

/// The evidence about `device`, found by `methods`, for the default prompt: a JSON array of
/// numbered facts, each saying where it came from and holding only what was found.
pub(crate) fn evidence(device: &Device, methods: &[Method]) -> String {
    let mut facts: Vec<(&str, Value)> = Vec::new();
    facts.extend(naming(device));
    if !methods.is_empty() {
        let answered: Vec<&str> = methods.iter().map(|&m| method_name(m)).collect();
        facts.push(("discovery", json!({ "answered": answered })));
    }
    facts.extend(announced(device).map(|fact| ("mdns", fact)));
    for card in &device.cards {
        facts.push(("network-card", card_fact(card)));
        facts.extend(open_ports(card).map(|fact| ("port-scan", fact)));
    }
    facts.extend(os_clues(device).map(|fact| ("os-fingerprint", fact)));
    let items: Vec<Value> = facts
        .into_iter()
        .enumerate()
        .map(|(at, (source, fact))| {
            json!({ "id": format!("E{}", at + 1), "source": source, "fact": fact })
        })
        .collect();
    // Plain values always serialise; an empty array is still evidence of nothing.
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_owned())
}

/// What the device is called: its hostname (from mDNS or reverse DNS), and its name only when
/// that is not just the hostname's first part.
fn naming(device: &Device) -> Option<(&'static str, Value)> {
    let name = device.name.trim();
    match &device.hostname {
        Some(hostname) => {
            let mut fact = Map::new();
            fact.insert("hostname".into(), json!(hostname));
            if !name.is_empty() && hostname.split('.').next() != Some(name) {
                fact.insert("name".into(), json!(name));
            }
            Some(("hostname", Value::Object(fact)))
        }
        None => (!name.is_empty()).then(|| ("name", json!({ "name": name }))),
    }
}

fn method_name(method: Method) -> &'static str {
    match method {
        Method::Arp => "neighbour table",
        Method::Ping => "ping",
        Method::Mdns => "mdns",
    }
}

/// What the device said about itself over mDNS, if anything.
fn announced(device: &Device) -> Option<Value> {
    let mut fact = Map::new();
    if !device.mdns_services.is_empty() {
        fact.insert("services".into(), json!(device.mdns_services));
    }
    if let Some(model) = &device.model {
        fact.insert("model".into(), json!(model));
    }
    if let Some(maker) = &device.manufacturer {
        fact.insert("manufacturer".into(), json!(maker));
    }
    (!fact.is_empty()).then_some(Value::Object(fact))
}

/// A network card's address and link details, leaving out what is unknown. A locally
/// administered MAC is usually a phone's or laptop's private (randomised) address.
fn card_fact(card: &NetworkCard) -> Value {
    let mut fact = Map::new();
    if !card.name.trim().is_empty() {
        fact.insert("name".into(), json!(card.name.trim()));
    }
    if let Some(address) = card.address {
        fact.insert("address".into(), json!(address.to_string()));
    }
    if let Some(gateway) = card.gateway {
        fact.insert("gateway".into(), json!(gateway.to_string()));
    }
    if let Some(mac) = card.mac {
        fact.insert("mac".into(), json!(mac.to_string()));
        if mac.0[0] & 0x02 != 0 {
            fact.insert("mac_randomised".into(), json!(true));
        }
    }
    if let Some(vendor) = &card.vendor {
        fact.insert("mac_vendor".into(), json!(vendor));
    }
    Value::Object(fact)
}

/// The card's open ports, each with what runs on it when known, such as `443/tcp nginx`.
fn open_ports(card: &NetworkCard) -> Option<Value> {
    let open: Vec<String> = card
        .services
        .iter()
        .take(MAX_PORTS)
        .map(|service| {
            let port = match service.protocol {
                ServiceProtocol::Tcp => format!("{}/tcp", service.port),
                ServiceProtocol::Udp => format!("{}/udp", service.port),
            };
            match service.description.trim() {
                "" => port,
                what => format!("{port} {what}"),
            }
        })
        .collect();
    if open.is_empty() {
        return None;
    }
    let address = card.address.map(|a| a.addr().to_string());
    Some(json!({ "address": address, "open": open }))
}

/// The scanner's OS guess with the clues behind it; none for a guess the AI made, which it is
/// not shown again.
fn os_clues(device: &Device) -> Option<Value> {
    let os = device.os.as_ref()?;
    if os.clues.contains(&OsClue::Ai) {
        return None;
    }
    let clues: Vec<String> = os
        .clues
        .iter()
        .filter_map(|&clue| clue_text(clue))
        .collect();
    let mut fact = Map::new();
    fact.insert("scanner_guess".into(), json!(family_name(os.family)));
    if let Some(detail) = &os.detail {
        fact.insert("detail".into(), json!(detail));
    }
    fact.insert("clues".into(), json!(clues));
    Some(Value::Object(fact))
}

/// A clue as the model reads it; the AI's own suggestion is no clue.
fn clue_text(clue: OsClue) -> Option<String> {
    Some(match clue {
        OsClue::DeviceModel => "model announced over mDNS".into(),
        OsClue::AppleSyncPort => "Apple sync port 62078 open".into(),
        OsClue::SshBanner => "SSH greeting".into(),
        OsClue::ServerHeader => "web server header".into(),
        OsClue::WindowsPorts => "TCP 135 and 445 open together".into(),
        OsClue::Ttl(ttl) => format!("ping TTL {ttl}"),
        OsClue::Ai => return None,
    })
}

/// The prompt's name for an OS family.
fn family_name(family: OsFamily) -> &'static str {
    match family {
        OsFamily::Windows => "Windows",
        OsFamily::MacOs => "macOS",
        OsFamily::Linux => "Linux",
        OsFamily::Ios => "iOS",
        OsFamily::Network => "Embedded",
    }
}

/// The default prompt's answer fields; its evidence review is not used.
#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "type")]
    kind: DeviceKind,
    type_confidence: Confidence,
    os_family: OsReply,
    os_confidence: Confidence,
    description: String,
    /// A name for the map; an answer to an older prompt without it still reads.
    #[serde(default)]
    suggested_name: String,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Deserialize, Clone, Copy)]
enum OsReply {
    Windows,
    #[serde(rename = "macOS")]
    MacOs,
    Linux,
    #[serde(rename = "iOS")]
    Ios,
    Android,
    Embedded,
    Unknown,
}

impl OsReply {
    /// The app's family for it; the app has none for Android.
    fn family(self) -> Option<OsFamily> {
        match self {
            Self::Windows => Some(OsFamily::Windows),
            Self::MacOs => Some(OsFamily::MacOs),
            Self::Linux => Some(OsFamily::Linux),
            Self::Ios => Some(OsFamily::Ios),
            Self::Embedded => Some(OsFamily::Network),
            Self::Android | Self::Unknown => None,
        }
    }
}

/// What the model made of a host, keeping only what it is confident (high or medium) about,
/// and a name that is plain text within its limit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Identity {
    kind: Option<DeviceKind>,
    os: Option<OsFamily>,
    /// Kept only with a confident type or OS, and only if it is plain text within the limit.
    description: Option<String>,
    name: Option<String>,
}

impl Identity {
    /// The identity in the model's `answer`, or `None` when it is not one JSON object with the
    /// prompt's fields and allowed values.
    pub(crate) fn read(answer: &str) -> Option<Self> {
        let reply: Reply = serde_json::from_str(response::complete_json(answer)?).ok()?;
        let sure = |confidence| confidence != Confidence::Low;
        let kind = Some(reply.kind)
            .filter(|kind| sure(reply.type_confidence) && *kind != DeviceKind::Unknown);
        let os = sure(reply.os_confidence)
            .then_some(reply.os_family)
            .and_then(OsReply::family);
        let description = Some(reply.description.trim())
            .filter(|text| (kind.is_some() || os.is_some()) && plain(text, MAX_DESCRIPTION))
            .map(str::to_owned);
        let name = Some(reply.suggested_name.trim())
            .filter(|name| plain(name, MAX_NAME) && !names_an_address(name))
            .map(str::to_owned);
        Some(Self {
            kind,
            os,
            description,
            name,
        })
    }

    /// Puts the model's answer in place of what the scan made of `device`: its type (with the
    /// role that follows it, unless another role was picked), OS family, notes and name. A
    /// field the model is unsure of keeps the scan's value, and so does a name the device's
    /// full hostname gave it (such as `pc` from `pc.local`).
    pub(crate) fn fill(&self, device: &mut Device) {
        if let Some(kind) = self.kind {
            if device.role == device.kind.default_role() {
                device.role = kind.default_role();
            }
            device.kind = kind;
        }
        if let Some(family) = self.os {
            device.os = Some(OsHint {
                family,
                detail: None,
                clues: vec![OsClue::Ai],
            });
        }
        if let Some(text) = &self.description {
            device.notes.clone_from(text);
        }
        if let Some(name) = self.name.as_ref().filter(|_| !has_full_hostname(device)) {
            device.name.clone_from(name);
        }
    }
}

/// The device has a hostname with a domain, such as `pc.local` or `xbox.lan`, which its name
/// comes from.
fn has_full_hostname(device: &Device) -> bool {
    device
        .hostname
        .as_deref()
        .and_then(|h| h.split_once('.'))
        .is_some_and(|(host, domain)| !host.is_empty() && !domain.is_empty())
}

/// Text fit for a device field: not empty, at most `max` characters, one line, and citing no
/// evidence IDs, which the prompt forbids there.
fn plain(text: &str, max: usize) -> bool {
    let cites = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter_map(|word| word.strip_prefix('E'))
        .any(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()));
    !text.is_empty() && text.chars().count() <= max && !text.chars().any(char::is_control) && !cites
}

/// The text holds an IPv4 address, which the prompt keeps out of names.
fn names_an_address(text: &str) -> bool {
    text.split_whitespace()
        .any(|word| word.parse::<std::net::Ipv4Addr>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CardId, DeviceId, LastSeen, MacAddr, Role, Services, Status};

    fn found() -> Device {
        let mut card = NetworkCard {
            address: Some("192.168.20.30/24".parse().unwrap()),
            mac: Some(MacAddr([0x00, 0x11, 0x32, 0x0a, 0x0b, 0x0c])),
            vendor: Some("Synology Incorporated".into()),
            ..NetworkCard::blank(CardId(1))
        };
        card.services.add(445, "SMB");
        card.services.add(5001, "");
        Device {
            id: DeviceId(1),
            name: "diskstation".into(),
            kind: DeviceKind::Unknown,
            role: Role::Other,
            icon: None,
            status: Status::Online,
            cards: vec![card],
            guest: None,
            manufacturer: None,
            model: None,
            hostname: Some("diskstation.home.arpa".into()),
            os: None,
            mdns_services: vec!["smb".into()],
            last_seen: LastSeen::default(),
            notes: String::new(),
        }
    }

    fn reply(kind: &str, confidence: &str, os: &str, description: &str) -> String {
        format!(
            "{{\"evidence_review\":[],\"type\":\"{kind}\",\"type_confidence\":\"{confidence}\",\
             \"os_family\":\"{os}\",\"os_confidence\":\"{confidence}\",\
             \"description\":\"{description}\"}} trailing text"
        )
    }

    fn evidence_items(device: &Device, methods: &[Method]) -> Vec<Value> {
        let evidence: Value = serde_json::from_str(&evidence(device, methods)).unwrap();
        evidence.as_array().unwrap().clone()
    }

    #[test]
    fn the_evidence_numbers_each_fact_about_the_host_and_its_cards_once() {
        let mut device = found();
        device.os = Some(OsHint {
            family: OsFamily::Linux,
            detail: Some("Ubuntu".into()),
            clues: vec![OsClue::SshBanner, OsClue::Ttl(64)],
        });
        let items = evidence_items(&device, &[Method::Arp, Method::Ping]);
        let sources: Vec<(&str, &str)> = items
            .iter()
            .map(|i| (i["id"].as_str().unwrap(), i["source"].as_str().unwrap()))
            .collect();
        assert_eq!(
            sources,
            [
                ("E1", "hostname"),
                ("E2", "discovery"),
                ("E3", "mdns"),
                ("E4", "network-card"),
                ("E5", "port-scan"),
                ("E6", "os-fingerprint"),
            ]
        );
        assert!(items.iter().all(|i| i.get("age").is_none()));
        // The name is the hostname's first part, so it is not repeated.
        assert_eq!(
            items[0]["fact"],
            json!({ "hostname": "diskstation.home.arpa" })
        );
        assert_eq!(
            items[1]["fact"]["answered"],
            json!(["neighbour table", "ping"])
        );
        assert_eq!(
            items[3]["fact"],
            json!({
                "address": "192.168.20.30/24",
                "mac": "00:11:32:0A:0B:0C",
                "mac_vendor": "Synology Incorporated"
            })
        );
        assert_eq!(
            items[4]["fact"],
            json!({ "address": "192.168.20.30", "open": ["445/tcp SMB", "5001/tcp"] })
        );
        assert_eq!(
            items[5]["fact"],
            json!({
                "scanner_guess": "Linux",
                "detail": "Ubuntu",
                "clues": ["SSH greeting", "ping TTL 64"]
            })
        );
    }

    #[test]
    fn a_private_mac_a_name_of_its_own_and_the_ais_own_guess_show_as_such() {
        let mut device = found();
        device.name = "Office NAS".into();
        if let Some(card) = device.cards.first_mut() {
            card.mac = Some(MacAddr([0xda, 0x11, 0x32, 0x0a, 0x0b, 0x0c]));
            card.services = Services::default();
        }
        device.os = Some(OsHint {
            family: OsFamily::Linux,
            detail: None,
            clues: vec![OsClue::Ai],
        });
        let items = evidence_items(&device, &[]);
        let sources: Vec<&str> = items
            .iter()
            .map(|i| i["source"].as_str().unwrap())
            .collect();
        assert_eq!(sources, ["hostname", "mdns", "network-card"]);
        assert_eq!(items[0]["fact"]["name"], "Office NAS");
        assert_eq!(items[2]["fact"]["mac_randomised"], true);
    }

    /// A confident NAS answer with `name` as its suggested name.
    fn nas_named(name: &str) -> Identity {
        let answer = reply("Nas", "high", "Linux", "Synology storage sharing files.");
        let answer = answer.replacen('{', &format!("{{\"suggested_name\":\"{name}\","), 1);
        Identity::read(&answer).unwrap()
    }

    #[test]
    fn a_confident_answer_replaces_what_the_scan_made_of_the_host() {
        // As the scan found it: no hostname, so no name, and its own guesses.
        let mut device = found();
        device.hostname = None;
        device.name = String::new();
        device.kind = DeviceKind::Server;
        device.role = Role::Server;
        device.os = Some(OsHint {
            family: OsFamily::Windows,
            detail: None,
            clues: vec![OsClue::Ttl(128)],
        });
        device.notes = "Old".into();
        nas_named("Synology NAS").fill(&mut device);
        assert_eq!((device.kind, &device.role), (DeviceKind::Nas, &Role::Nas));
        let os = device.os.as_ref().unwrap();
        assert_eq!(
            (os.family, os.clues.as_slice()),
            (OsFamily::Linux, &[OsClue::Ai][..])
        );
        assert_eq!(device.notes, "Synology storage sharing files.");
        assert_eq!(device.name, "Synology NAS");

        let mut picked = found();
        picked.role = Role::Custom("Backups".into());
        nas_named("Synology NAS").fill(&mut picked);
        assert_eq!(
            (picked.kind, picked.role),
            (DeviceKind::Nas, Role::Custom("Backups".into())),
            "a role picked by hand stays"
        );
    }

    #[test]
    fn a_name_from_a_full_hostname_stays() {
        let mut named = found();
        nas_named("Synology NAS").fill(&mut named);
        assert_eq!(named.name, "diskstation", "from diskstation.home.arpa");
        for hostname in [None, Some("NAS"), Some("nas.")] {
            let mut device = found();
            device.hostname = hostname.map(str::to_owned);
            nas_named("Synology NAS").fill(&mut device);
            assert_eq!(device.name, "Synology NAS", "{hostname:?}");
        }
    }

    #[test]
    fn unsure_unknown_and_unfit_answers_keep_the_scans_values() {
        let low = Identity::read(&reply("Nas", "low", "Linux", "A NAS.")).unwrap();
        assert_eq!(low, Identity::default());
        let unknown = Identity::read(&reply("Unknown", "medium", "Android", "Phone.")).unwrap();
        assert_eq!(unknown, Identity::default());
        for unfit in [
            "",
            "Router at 192.168.1.1",
            "NAS E3",
            &"x".repeat(MAX_NAME + 1),
        ] {
            assert_eq!(nas_named(unfit).name, None, "{unfit}");
        }
        let cited = Identity::read(&reply("Nas", "medium", "Linux", "A NAS (E3).")).unwrap();
        assert_eq!(
            (cited.kind, cited.description),
            (Some(DeviceKind::Nas), None)
        );
        let long = "x".repeat(MAX_DESCRIPTION + 1);
        let long = Identity::read(&reply("Nas", "medium", "Linux", &long)).unwrap();
        assert_eq!(long.description, None);
        for bad in [
            "no JSON",
            "{\"type\": \"Nas\"",
            &reply("Toaster", "high", "Linux", "A toaster."),
            &reply("Nas", "certain", "Linux", "A NAS."),
        ] {
            assert_eq!(Identity::read(bad), None, "{bad}");
        }
    }
}
