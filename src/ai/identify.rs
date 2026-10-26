//! Import with AI's identification: the evidence a found host and its network cards give the
//! model, and its answer put in place of what the scan made of the host. The answer must hold
//! all of the default prompt's fields with their allowed values; a field it leaves Unknown
//! keeps the scan's value.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::response;
use crate::model::{
    Device, DeviceKind, KindClue, MacAddr, NetworkCard, OsClue, OsFamily, OsHint, ServiceProtocol,
    ServiceSource,
};

/// The most open ports listed for one card, so a host with thousands open still fits the
/// model's context.
const MAX_PORTS: usize = 64;
/// The longest description kept (the agreed AI fields), and the longest name (the prompt's).
const MAX_DESCRIPTION: usize = 120;
const MAX_NAME: usize = 35;

/// Compact identity evidence for the model. Rust has already discarded raw addressing and
/// converted generated names into an unnamed state, leaving the model only ambiguous facts.
pub(crate) fn evidence(device: &Device) -> String {
    let mut fact = Map::new();
    let name = device.name.trim();
    if !name.is_empty() && !generated_name(name) {
        fact.insert("current_name".into(), json!(name));
    }
    if let Some(hostname) = &device.hostname {
        fact.insert("hostname".into(), json!(hostname));
    }
    if let Some(model) = &device.model {
        fact.insert("model".into(), json!(model));
    }
    if let Some(manufacturer) = &device.manufacturer {
        fact.insert("manufacturer".into(), json!(manufacturer));
    }
    if !device.mdns_services.is_empty() {
        fact.insert("mdns_services".into(), json!(device.mdns_services));
    }
    if !device.ssdp_services.is_empty() {
        fact.insert("ssdp_upnp_services".into(), json!(device.ssdp_services));
    }
    if let Some(server) = &device.ssdp_server {
        fact.insert("ssdp_server".into(), json!(server));
    }
    add_card_evidence(&mut fact, &device.cards);
    if let Some(kind) = kind_clue(device) {
        fact.insert("type_fingerprint".into(), kind);
    }
    if let Some(os) = os_clues(device) {
        fact.insert("fingerprint".into(), os);
    }
    // Plain values always serialise; an empty object is still evidence of nothing.
    serde_json::to_string_pretty(&fact).unwrap_or_else(|_| "{}".to_owned())
}

/// `device-XXXX` is a stable Rust-generated placeholder, not identity evidence.
fn generated_name(name: &str) -> bool {
    name.get(..7)
        .filter(|prefix| prefix.eq_ignore_ascii_case("device-"))
        .and_then(|_| name.get(7..))
        .is_some_and(|suffix| suffix.len() == 4 && suffix.chars().all(|c| c.is_ascii_hexdigit()))
}

fn add_card_evidence(fact: &mut Map<String, Value>, cards: &[NetworkCard]) {
    let mut vendors: Vec<&str> = cards
        .iter()
        .filter_map(|card| card.vendor.as_deref())
        .collect();
    vendors.sort_unstable();
    vendors.dedup();
    if !vendors.is_empty() {
        fact.insert("mac_vendors".into(), json!(vendors));
    }
    let mut macs = cards.iter().filter_map(|card| card.mac);
    if macs.clone().any(MacAddr::is_randomised) {
        fact.insert("randomised_mac".into(), json!(true));
    }
    if macs.any(MacAddr::is_virtual) {
        fact.insert("virtual_adapter".into(), json!(true));
    }
    let services: Vec<Value> = cards.iter().flat_map(open_ports).take(MAX_PORTS).collect();
    if !services.is_empty() {
        fact.insert("services".into(), json!(services));
    }
}

/// The card's open ports and whether their descriptions were observed, looked up, or edited.
fn open_ports(card: &NetworkCard) -> Vec<Value> {
    card.services
        .iter()
        .take(MAX_PORTS)
        .map(|service| {
            let protocol = match service.protocol {
                ServiceProtocol::Tcp => "tcp",
                ServiceProtocol::Udp => "udp",
            };
            let source = match service.source {
                ServiceSource::Manual => "manual",
                ServiceSource::Detected => "detected",
                ServiceSource::Registry => "registry",
                ServiceSource::Open => "open",
            };
            let mut fact = Map::new();
            fact.insert("port".into(), json!(service.port));
            fact.insert("protocol".into(), json!(protocol));
            fact.insert("source".into(), json!(source));
            if !service.description.trim().is_empty() {
                fact.insert("description".into(), json!(service.description.trim()));
            }
            Value::Object(fact)
        })
        .collect()
}

/// The scanner's type when one of its certain rules decided it, with the clue behind it.
fn kind_clue(device: &Device) -> Option<Value> {
    let clue = match device.kind_clue? {
        KindClue::DefaultGateway => "default gateway of the scanning computer",
        KindClue::SwitchModel => "hostname or model names a managed switch series",
        KindClue::FileServer => "NFS, its port mapper and SMB open together",
    };
    Some(json!({ "scanner_guess": device.kind, "clue": clue }))
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

/// The default prompt's answer: every field must be there with an allowed value, or the answer
/// is refused. The evidence review is optional (the default prompt no longer asks for it) but
/// checked when given; it and the confidences are not kept: the device has no place for them.
#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "evidence_review", default)]
    _evidence_review: Vec<Review>,
    suggested_name: String,
    #[serde(rename = "type")]
    kind: DeviceKind,
    #[serde(rename = "type_confidence")]
    _type_confidence: Confidence,
    os_family: OsReply,
    #[serde(rename = "os_confidence")]
    _os_confidence: Confidence,
    description: String,
}

/// One entry of the evidence review: the input field a clue comes from, and what it indicates.
#[derive(Deserialize)]
struct Review {
    #[serde(rename = "id")]
    _id: String,
    #[serde(rename = "indicates")]
    _indicates: String,
}

#[derive(Deserialize)]
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

/// What the model made of a host: each field it gave a value for, whatever its confidence. An
/// Unknown type or OS family gives none, and a name or description that is not plain text
/// within its limit is dropped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Identity {
    kind: Option<DeviceKind>,
    os: Option<OsFamily>,
    description: Option<String>,
    name: Option<String>,
}

impl Identity {
    /// The identity in the model's `answer`, or `None` when it is not one JSON object with all
    /// of the prompt's fields and allowed values.
    pub(crate) fn read(answer: &str) -> Option<Self> {
        let reply: Reply = serde_json::from_str(response::complete_json(answer)?).ok()?;
        let kind = Some(reply.kind).filter(|kind| *kind != DeviceKind::Unknown);
        let os = reply.os_family.family();
        let description = Some(reply.description.trim())
            .filter(|text| plain(text, MAX_DESCRIPTION))
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
    /// role and icon that follow it, unless another role was picked), OS family, notes and
    /// name. A field the answer left Unknown keeps the scan's value, and so does a name the
    /// device's hostname gave it (such as `pc` from `pc.local`).
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
        if let Some(name) = self.name.as_ref().filter(|_| !has_hostname(device)) {
            device.name.clone_from(name);
        }
    }
}

/// The scan already made the hostname's host label the device name, so an AI suggestion must
/// not replace it.
fn has_hostname(device: &Device) -> bool {
    device
        .hostname
        .as_deref()
        .is_some_and(|hostname| !hostname.trim_matches('.').trim().is_empty())
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
            kind_clue: None,
            role: Role::Other,
            icon: None,
            status: Status::Online,
            host: None,
            cards: vec![card],
            manufacturer: None,
            model: None,
            hostname: Some("diskstation.home.arpa".into()),
            os: None,
            mdns_services: vec!["smb".into()],
            ssdp_services: Vec::new(),
            ssdp_server: None,
            probe_log: Vec::new(),
            last_seen: LastSeen::default(),
            notes: String::new(),
        }
    }

    /// A complete answer, as the prompt asks for, with text after it.
    fn reply(kind: &str, confidence: &str, os: &str, description: &str, name: &str) -> String {
        json!({
            "evidence_review": [{ "id": "hostname", "indicates": "weak: a clue" }],
            "suggested_name": name,
            "type": kind,
            "type_confidence": confidence,
            "os_family": os,
            "os_confidence": confidence,
            "description": description
        })
        .to_string()
            + " trailing text"
    }

    fn evidence_of(device: &Device) -> Value {
        serde_json::from_str(&evidence(device)).unwrap()
    }

    #[test]
    fn the_evidence_is_one_object_with_each_card_and_its_ports_together() {
        let mut device = found();
        device.os = Some(OsHint {
            family: OsFamily::Linux,
            detail: Some("Ubuntu".into()),
            clues: vec![OsClue::SshBanner, OsClue::Ttl(64)],
        });
        assert_eq!(
            evidence_of(&device),
            json!({
                "current_name": "diskstation",
                "hostname": "diskstation.home.arpa",
                "mdns_services": ["smb"],
                "mac_vendors": ["Synology Incorporated"],
                "services": [
                    { "port": 445, "protocol": "tcp", "source": "manual", "description": "SMB" },
                    { "port": 5001, "protocol": "tcp", "source": "manual" }
                ],
                "fingerprint": {
                    "scanner_guess": "Linux",
                    "detail": "Ubuntu",
                    "clues": ["SSH greeting", "ping TTL 64"]
                }
            })
        );
    }

    #[test]
    fn no_name_asks_for_one_and_a_private_mac_and_the_ais_own_guess_show_as_such() {
        let mut device = found();
        device.name = String::new();
        device.hostname = None;
        device.mdns_services.clear();
        if let Some(card) = device.cards.first_mut() {
            card.mac = Some(MacAddr([0xda, 0x11, 0x32, 0x0a, 0x0b, 0x0c]));
            card.vendor = None;
            card.services = Services::default();
        }
        device.os = Some(OsHint {
            family: OsFamily::Linux,
            detail: None,
            clues: vec![OsClue::Ai],
        });
        assert_eq!(
            evidence_of(&device),
            json!({
                "randomised_mac": true
            })
        );
    }

    #[test]
    fn a_virtual_machines_card_is_not_called_randomised_and_a_certain_type_says_why() {
        let mut device = found();
        if let Some(card) = device.cards.first_mut() {
            card.mac = Some(MacAddr([0x52, 0x54, 0x00, 0x77, 0x01, 0x10]));
        }
        device.kind = DeviceKind::Nas;
        device.kind_clue = Some(KindClue::FileServer);
        let evidence = evidence_of(&device);
        assert_eq!(evidence.get("randomised_mac"), None);
        assert_eq!(evidence["virtual_adapter"], json!(true));
        assert_eq!(
            evidence["type_fingerprint"],
            json!({ "scanner_guess": "Nas", "clue": "NFS, its port mapper and SMB open together" })
        );
    }

    #[test]
    fn ssdp_and_upnp_declarations_are_identity_evidence() {
        let mut device = found();
        device.ssdp_services = vec!["urn:schemas-upnp-org:device:InternetGatewayDevice:1".into()];
        device.ssdp_server = Some("RouterOS UPnP/1.1".into());
        let evidence = evidence_of(&device);
        assert_eq!(
            evidence["ssdp_upnp_services"],
            json!(["urn:schemas-upnp-org:device:InternetGatewayDevice:1"])
        );
        assert_eq!(evidence["ssdp_server"], json!("RouterOS UPnP/1.1"));
    }

    #[test]
    fn generated_names_are_not_sent_as_identity_evidence() {
        for name in ["device-4A2B", "DEVICE-ab12"] {
            let mut device = found();
            device.name = name.into();
            device.hostname = None;
            assert_eq!(evidence_of(&device).get("current_name"), None, "{name}");
        }
        assert!(!generated_name("device-lamp"));
    }

    /// A NAS answer with `name` as its suggested name.
    fn nas_named(name: &str) -> Identity {
        let description = "Synology storage sharing files.";
        Identity::read(&reply("Nas", "high", "Linux", description, name)).unwrap()
    }

    #[test]
    fn the_current_prompts_answer_without_an_evidence_review_fills_the_device() {
        // An answer the current default prompt gave, word for word.
        let answer = r#"{
          "suggested_name": "Amazon Router",
          "type": "Router",
          "type_confidence": "medium",
          "os_family": "Embedded",
          "os_confidence": "low",
          "description": "A router with HTTPS service likely from Amazon."
        }"#;
        let mut device = found();
        device.hostname = None;
        Identity::read(answer).unwrap().fill(&mut device);
        assert_eq!(device.name, "Amazon Router");
        assert_eq!(
            (device.kind, &device.role),
            (DeviceKind::Router, &Role::Router)
        );
        assert_eq!(
            device.os.as_ref().map(|os| os.family),
            Some(OsFamily::Network)
        );
        assert_eq!(
            device.notes,
            "A router with HTTPS service likely from Amazon."
        );
    }

    #[test]
    fn the_prompts_answer_fills_every_field_of_the_device() {
        // An answer an earlier prompt gave, word for word, with an evidence review.
        let answer = r#"{
          "evidence_review": [
            { "id": "network_cards", "indicates": "MAC address vendor is Realtek, suggesting a network interface card from a common manufacturer." },
            { "id": "open", "indicates": "OpenSSH, SMB, and RDP services indicate a device that can provide remote access and file sharing." },
            { "id": "os_fingerprint", "indicates": "SSH greeting and TCP port combinations are consistent with a Windows system." }
          ],
          "suggested_name": "Windows Network Device",
          "type": "Router",
          "type_confidence": "medium",
          "os_family": "Windows",
          "os_confidence": "medium",
          "description": "A device providing remote access and file sharing services likely acting as a network gateway."
        }"#;
        let mut device = found();
        device.hostname = None;
        Identity::read(answer).unwrap().fill(&mut device);
        assert_eq!(device.name, "Windows Network Device");
        assert_eq!(
            (device.kind, &device.role),
            (DeviceKind::Router, &Role::Router)
        );
        assert_eq!(
            device.os.as_ref().map(|os| os.family),
            Some(OsFamily::Windows)
        );
        assert_eq!(
            device.notes,
            "A device providing remote access and file sharing services likely acting as a network gateway."
        );
    }

    #[test]
    fn an_answer_missing_a_field_or_with_a_value_not_allowed_is_refused() {
        let complete = reply("Nas", "low", "Linux", "A NAS.", "NAS");
        let complete: Value =
            serde_json::from_str(complete.trim_end_matches(" trailing text")).unwrap();
        let fields = complete.as_object().unwrap().clone();
        for missing in fields.keys() {
            let mut partial = fields.clone();
            partial.remove(missing);
            let partial = Value::Object(partial).to_string();
            // The default prompt no longer asks for an evidence review.
            let optional = missing == "evidence_review";
            assert_eq!(
                Identity::read(&partial).is_some(),
                optional,
                "without {missing}"
            );
        }
        for (field, bad) in [
            ("type", json!("Toaster")),
            ("type_confidence", json!("certain")),
            ("os_family", json!("BeOS")),
            ("suggested_name", json!(7)),
            ("evidence_review", json!([{ "id": "hostname" }])),
            ("evidence_review", json!("hostname")),
        ] {
            let mut wrong = fields.clone();
            wrong.insert(field.into(), bad.clone());
            let wrong = Value::Object(wrong).to_string();
            assert_eq!(Identity::read(&wrong), None, "{field}: {bad}");
        }
        for broken in ["no JSON", "{\"type\": \"Nas\""] {
            assert_eq!(Identity::read(broken), None, "{broken}");
        }
    }

    #[test]
    fn an_answer_replaces_what_the_scan_made_of_the_host() {
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
    fn a_name_from_any_hostname_stays() {
        let mut named = found();
        nas_named("Synology NAS").fill(&mut named);
        assert_eq!(named.name, "diskstation", "from diskstation.home.arpa");
        for hostname in [Some("NAS"), Some("nas.")] {
            let mut device = found();
            device.hostname = hostname.map(str::to_owned);
            nas_named("Synology NAS").fill(&mut device);
            assert_eq!(device.name, "diskstation", "{hostname:?}");
        }
        let mut unnamed = found();
        unnamed.hostname = None;
        nas_named("Synology NAS").fill(&mut unnamed);
        assert_eq!(unnamed.name, "Synology NAS", "without a hostname");
    }

    #[test]
    fn low_confidence_fills_too_but_unknown_and_unfit_text_keep_the_scans_values() {
        let low = Identity::read(&reply("Nas", "low", "Linux", "A NAS.", "NAS")).unwrap();
        assert_eq!(
            (
                low.kind,
                low.os,
                low.description.as_deref(),
                low.name.as_deref()
            ),
            (
                Some(DeviceKind::Nas),
                Some(OsFamily::Linux),
                Some("A NAS."),
                Some("NAS")
            )
        );
        let unknown = reply("Unknown", "low", "Android", "", "");
        assert_eq!(Identity::read(&unknown), Some(Identity::default()));
        for unfit in [
            "",
            "Router at 192.168.1.1",
            "NAS E3",
            &"x".repeat(MAX_NAME + 1),
        ] {
            assert_eq!(nas_named(unfit).name, None, "{unfit}");
        }
        let cited = reply("Nas", "medium", "Linux", "A NAS (E3).", "NAS");
        assert_eq!(Identity::read(&cited).unwrap().description, None);
        let long = reply(
            "Nas",
            "medium",
            "Linux",
            &"x".repeat(MAX_DESCRIPTION + 1),
            "NAS",
        );
        assert_eq!(Identity::read(&long).unwrap().description, None);
    }
}
