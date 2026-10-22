//! Import with AI's identification: the evidence a found host and its network cards give the
//! model, and its answer read back into the device fields the scan and the user left empty.
//! The answer must hold the default prompt's fields with their allowed values; a field the
//! model is not confident about stays empty, and anything else in the answer is ignored.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::response;
use crate::model::{Device, DeviceKind, OsClue, OsFamily, OsHint, ServiceProtocol};

/// The most open ports listed for one card, and of those the most with what runs on them, so a
/// host with thousands open still fits the model's context.
const MAX_PORTS: usize = 64;
const MAX_SERVICES: usize = 16;
/// The longest description kept (the agreed AI fields).
const MAX_DESCRIPTION: usize = 120;
/// How old every fact is: all come from the run just finished.
const AGE: &str = "this scan";

/// The evidence about `device` for the default prompt: a JSON array of numbered facts.
pub(crate) fn evidence(device: &Device) -> String {
    let mut facts = Vec::new();
    if !device.name.trim().is_empty() {
        facts.push(("name", json!({ "name": device.name })));
    }
    if let Some(hostname) = &device.hostname {
        facts.push(("reverse-dns", json!({ "ptr": hostname })));
    }
    if let Some(announced) = announced(device) {
        facts.push(("mdns", announced));
    }
    for card in &device.cards {
        let address = card.address.map(|a| a.to_string());
        let mac = card.mac.map(|m| m.to_string());
        let link = json!({ "address": address, "mac": mac, "vendor": card.vendor });
        facts.push(("arp", link));
        let ports: Vec<_> = card.services.iter().take(MAX_PORTS).collect();
        if !ports.is_empty() {
            let open: Vec<String> = ports.iter().map(|s| port(s.port, s.protocol)).collect();
            facts.push(("port-scan", json!({ "open": open })));
        }
        let described = ports.iter().filter(|s| !s.description.is_empty());
        for service in described.take(MAX_SERVICES) {
            let port = port(service.port, service.protocol);
            facts.push((
                "service-detection",
                json!({ "port": port, "service": service.description }),
            ));
        }
    }
    if let Some(os) = &device.os {
        let fact = json!({ "family": family_name(os.family), "detail": os.detail });
        facts.push(("os-fingerprint", fact));
    }
    let items: Vec<Value> = facts
        .into_iter()
        .enumerate()
        .map(|(at, (source, fact))| {
            json!({ "id": format!("E{}", at + 1), "source": source, "age": AGE, "fact": fact })
        })
        .collect();
    // Plain values always serialise; an empty array is still evidence of nothing.
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_owned())
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

fn port(number: u16, protocol: ServiceProtocol) -> String {
    match protocol {
        ServiceProtocol::Tcp => format!("{number}/tcp"),
        ServiceProtocol::Udp => format!("{number}/udp"),
    }
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

/// What the model made of a host, keeping only what it is confident (high or medium) about.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Identity {
    kind: Option<DeviceKind>,
    os: Option<OsFamily>,
    /// Kept only with a confident type or OS, and only if it is plain text within the limit.
    description: Option<String>,
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
            .filter(|text| (kind.is_some() || os.is_some()) && plain(text))
            .map(str::to_owned);
        Some(Self {
            kind,
            os,
            description,
        })
    }

    /// Fills the fields of `device` that are still empty: an unknown type (with the role and
    /// icon that follow it, unless a role was picked), no OS family, and empty notes. What the
    /// scan found or the user entered is never changed.
    pub(crate) fn fill(&self, device: &mut Device) {
        if let Some(kind) = self.kind.filter(|_| device.kind == DeviceKind::Unknown) {
            if device.role == DeviceKind::Unknown.default_role() {
                device.role = kind.default_role();
            }
            device.kind = kind;
        }
        if device.os.is_none() {
            device.os = self.os.map(|family| OsHint {
                family,
                detail: None,
                clues: vec![OsClue::Ai],
            });
        }
        if let Some(text) = self
            .description
            .as_ref()
            .filter(|_| device.notes.trim().is_empty())
        {
            device.notes.clone_from(text);
        }
    }
}

/// A description fit for the notes: not empty, within the limit, one line, and citing no
/// evidence IDs, which the prompt forbids there.
fn plain(text: &str) -> bool {
    let cites = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter_map(|word| word.strip_prefix('E'))
        .any(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()));
    !text.is_empty()
        && text.chars().count() <= MAX_DESCRIPTION
        && !text.chars().any(char::is_control)
        && !cites
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CardId, DeviceId, LastSeen, MacAddr, NetworkCard, Role, Status};

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

    #[test]
    fn the_evidence_numbers_each_fact_about_the_host_and_its_cards() {
        let evidence: Value = serde_json::from_str(&evidence(&found())).unwrap();
        let items = evidence.as_array().unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["E1", "E2", "E3", "E4", "E5", "E6"]);
        let text = evidence.to_string();
        for fact in [
            "diskstation.home.arpa",
            "00:11:32:0A:0B:0C",
            "Synology",
            "445/tcp",
        ] {
            assert!(text.contains(fact), "{fact}");
        }
        assert_eq!(items[4]["fact"]["open"], json!(["445/tcp", "5001/tcp"]));
        assert_eq!(items[5]["fact"]["service"], "SMB");
    }

    #[test]
    fn a_confident_answer_fills_only_empty_fields() {
        let answer = reply("Nas", "high", "Linux", "Synology storage sharing files.");
        let identity = Identity::read(&answer).unwrap();
        let mut device = found();
        identity.fill(&mut device);
        assert_eq!(device.kind, DeviceKind::Nas);
        assert_eq!(device.role, Role::Nas);
        let os = device.os.as_ref().unwrap();
        assert_eq!(
            (os.family, os.clues.as_slice()),
            (OsFamily::Linux, &[OsClue::Ai][..])
        );
        assert_eq!(device.notes, "Synology storage sharing files.");

        let mut edited = found();
        edited.kind = DeviceKind::Server;
        edited.role = Role::Custom("Backups".into());
        edited.notes = "Mine".into();
        identity.fill(&mut edited);
        assert_eq!(
            (edited.kind, edited.role, edited.notes.as_str()),
            (DeviceKind::Server, Role::Custom("Backups".into()), "Mine")
        );
        let mut picked = found();
        picked.role = Role::Laptop;
        identity.fill(&mut picked);
        assert_eq!((picked.kind, picked.role), (DeviceKind::Nas, Role::Laptop));
    }

    #[test]
    fn unsure_unknown_and_unfit_answers_fill_nothing_doubtful() {
        let low = Identity::read(&reply("Nas", "low", "Linux", "A NAS.")).unwrap();
        assert_eq!(low, Identity::default());
        let unknown = Identity::read(&reply("Unknown", "medium", "Android", "Phone.")).unwrap();
        assert_eq!(unknown, Identity::default());
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
