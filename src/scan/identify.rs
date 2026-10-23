//! Simple, explainable rules that turn what a scan saw of a host into a guess at what kind of
//! device it is and which operating system it runs. Each guess comes from a named clue, so a
//! wrong one can be traced; the user can change the type, and the OS stays a hint.

use netscli_core::PortResult;

use crate::model::{DeviceKind, OsClue, OsFamily, OsHint};

/// What the rules look at for one host.
pub(super) struct Evidence<'a> {
    /// Its TCP ports probed, open or not.
    pub(super) ports: &'a [PortResult],
    /// The mDNS service types it announced, short (`ipp`).
    pub(super) mdns_services: &'a [String],
    /// The model it announced over mDNS.
    pub(super) model: Option<&'a str>,
    /// The time-to-live of its ping reply.
    pub(super) ttl: Option<u8>,
}

impl Evidence<'_> {
    fn open(&self, port: u16) -> bool {
        self.ports.iter().any(|p| p.open && p.port == port)
    }

    fn any_open(&self, ports: &[u16]) -> bool {
        ports.iter().any(|&port| self.open(port))
    }

    fn announces(&self, services: &[&str]) -> bool {
        services
            .iter()
            .any(|s| self.mdns_services.iter().any(|m| m == s))
    }

    fn model_is(&self, prefixes: &[&str]) -> bool {
        let model = self.model.unwrap_or_default().to_ascii_lowercase();
        prefixes.iter().any(|p| model.starts_with(p))
    }

    fn model_mentions(&self, words: &[&str]) -> bool {
        let model = self.model.unwrap_or_default().to_ascii_lowercase();
        words.iter().any(|word| model.contains(word))
    }

    fn apple_mobile(&self) -> bool {
        self.model_is(&["iphone", "ipad"])
    }

    fn mac(&self) -> bool {
        self.model_is(&["mac", "imac"])
    }
}

type Rule = fn(&Evidence<'_>) -> bool;

fn thermostat(e: &Evidence<'_>) -> bool {
    e.model_mentions(&["thermostat", "ecobee", "tado"])
}

fn smart_display(e: &Evidence<'_>) -> bool {
    e.model_mentions(&["echo show", "nest hub", "smart display"])
}

fn voice_assistant(e: &Evidence<'_>) -> bool {
    e.model_mentions(&[
        "amazon echo",
        "echo dot",
        "homepod",
        "google home",
        "nest audio",
    ])
}

fn smart_device(e: &Evidence<'_>) -> bool {
    e.announces(&[
        "hap",
        "homekit",
        "home-assistant",
        "mqtt",
        "raop",
        "spotify-connect",
    ]) || e.model_mentions(&["hue", "tuya", "shelly", "sonos", "esp32", "esp8266"])
        || e.open(1400)
}

/// The kind each rule points to, in order: the first that holds wins. Printers and Apple
/// models first, being the most certain; port-only rules last.
fn kind_rules() -> [(Rule, DeviceKind); 14] {
    [
        (
            |e| {
                e.announces(&["ipp", "ipps", "printer", "pdl-datastream"])
                    || e.any_open(&[515, 631, 9100])
            },
            DeviceKind::Printer,
        ),
        (|e| e.apple_mobile() || e.open(62078), DeviceKind::Phone),
        (|e| e.model_is(&["macbook"]), DeviceKind::Laptop),
        (|e| e.mac(), DeviceKind::Desktop),
        (thermostat, DeviceKind::Thermostat),
        (smart_display, DeviceKind::SmartDisplay),
        (voice_assistant, DeviceKind::VoiceAssistant),
        (
            |e| {
                e.model_is(&["appletv"])
                    || e.announces(&["googlecast"])
                    || e.any_open(&[8001, 8008, 8009, 8060])
            },
            DeviceKind::Tv,
        ),
        (smart_device, DeviceKind::Smart),
        (|e| e.any_open(&[554, 8554, 37777]), DeviceKind::Camera),
        (|e| e.open(8291), DeviceKind::Router),
        (
            |e| e.open(445) && (e.any_open(&[2049, 5000, 5001]) || e.announces(&["afpovertcp"])),
            DeviceKind::Nas,
        ),
        (|e| e.open(8006), DeviceKind::Server),
        (
            |e| e.open(3389) || (e.open(135) && e.open(445)),
            DeviceKind::Desktop,
        ),
    ]
}

/// What kind of device the evidence points to; Unknown when no rule holds.
pub(super) fn kind(evidence: &Evidence<'_>) -> DeviceKind {
    let mut rules = kind_rules().into_iter();
    let found = rules.find(|(holds, _)| holds(evidence));
    found.map_or(DeviceKind::Unknown, |(_, kind)| kind)
}

/// One clue to the operating system: how strong it is, where it points, and what it says
/// more precisely.
struct Pointer {
    strength: u8,
    family: OsFamily,
    detail: Option<&'static str>,
    clue: OsClue,
}

/// Linux distributions that name themselves in SSH greetings and `Server` headers.
const DISTRIBUTIONS: [(&str, &str); 6] = [
    ("ubuntu", "Ubuntu"),
    ("debian", "Debian"),
    ("raspbian", "Raspbian"),
    ("fedora", "Fedora"),
    ("centos", "CentOS"),
    ("alpine", "Alpine"),
];

/// The operating system the evidence points to most strongly, with the clues that agree;
/// `None` when nothing points anywhere.
pub(super) fn os(evidence: &Evidence<'_>) -> Option<OsHint> {
    let mut pointers = pointers(evidence);
    // Stable, so equally strong clues keep the order they were gathered in.
    pointers.sort_by_key(|p| std::cmp::Reverse(p.strength));
    let best = pointers.first()?.family;
    let agreeing: Vec<&Pointer> = pointers.iter().filter(|p| p.family == best).collect();
    let mut clues: Vec<OsClue> = Vec::new();
    for pointer in &agreeing {
        if !clues.contains(&pointer.clue) {
            clues.push(pointer.clue);
        }
    }
    Some(OsHint {
        family: best,
        detail: agreeing.iter().find_map(|p| p.detail).map(str::to_owned),
        clues,
    })
}

fn pointers(evidence: &Evidence<'_>) -> Vec<Pointer> {
    let point = |strength, family, detail, clue| Pointer {
        strength,
        family,
        detail,
        clue,
    };
    let mut found = Vec::new();
    if evidence.apple_mobile() {
        found.push(point(5, OsFamily::Ios, None, OsClue::DeviceModel));
    } else if evidence.mac() {
        found.push(point(5, OsFamily::MacOs, None, OsClue::DeviceModel));
    }
    if evidence.open(62078) {
        found.push(point(4, OsFamily::Ios, None, OsClue::AppleSyncPort));
    }
    for port in evidence.ports.iter().filter(|p| p.open) {
        let ssh = port.banner.as_deref().filter(|b| b.starts_with("SSH-"));
        if let Some((family, detail)) = ssh.and_then(from_ssh) {
            found.push(point(4, family, detail, OsClue::SshBanner));
        }
        let headers = port.http.iter().flat_map(|http| &http.headers);
        let mut servers = headers.filter(|h| h.name.eq_ignore_ascii_case("server"));
        if let Some((family, detail)) = servers.find_map(|h| from_server(&h.value)) {
            found.push(point(4, family, detail, OsClue::ServerHeader));
        }
    }
    if evidence.open(135) && evidence.open(445) {
        found.push(point(3, OsFamily::Windows, None, OsClue::WindowsPorts));
    }
    if let Some(ttl) = evidence.ttl {
        // Windows starts at 128 and network gear at 255; 64 is Linux, macOS and other Unix
        // alike, so it tells nothing apart. A hop or two on the way barely lowers it.
        let family = match ttl {
            65..=128 => Some(OsFamily::Windows),
            129..=255 => Some(OsFamily::Network),
            0..=64 => None,
        };
        if let Some(family) = family {
            found.push(point(2, family, None, OsClue::Ttl(ttl)));
        }
    }
    found
}

/// What an SSH greeting says of the OS: OpenSSH names a distribution, or Windows.
fn from_ssh(banner: &str) -> Option<(OsFamily, Option<&'static str>)> {
    let banner = banner.to_ascii_lowercase();
    if banner.contains("windows") {
        return Some((OsFamily::Windows, None));
    }
    distribution(&banner).map(|name| (OsFamily::Linux, Some(name)))
}

/// What a web server's `Server` header says of the OS.
fn from_server(header: &str) -> Option<(OsFamily, Option<&'static str>)> {
    let header = header.to_ascii_lowercase();
    if header.contains("microsoft-iis") || header.contains("microsoft-httpapi") {
        return Some((OsFamily::Windows, None));
    }
    distribution(&header).map(|name| (OsFamily::Linux, Some(name)))
}

fn distribution(text: &str) -> Option<&'static str> {
    let mut known = DISTRIBUTIONS.iter();
    known
        .find(|(key, _)| text.contains(key))
        .map(|&(_, name)| name)
}

#[cfg(test)]
mod tests {
    use netscli_core::{HttpHeader, HttpProbe, PortStatus, Protocol};

    use super::*;

    fn port(number: u16) -> PortResult {
        PortResult {
            port: number,
            protocol: Protocol::Tcp,
            open: true,
            status: PortStatus::Open,
            service: None,
            product: None,
            version: None,
            latency_ms: None,
            banner: None,
            http: None,
            tls: None,
            raw: None,
            error: None,
        }
    }

    fn ports(numbers: &[u16]) -> Vec<PortResult> {
        numbers.iter().map(|&n| port(n)).collect()
    }

    fn evidence<'a>(ports: &'a [PortResult], services: &'a [String]) -> Evidence<'a> {
        Evidence {
            ports,
            mdns_services: services,
            model: None,
            ttl: None,
        }
    }

    fn names(services: &[&str]) -> Vec<String> {
        services.iter().map(|&s| s.to_owned()).collect()
    }

    #[test]
    fn ports_and_announcements_tell_the_kind() {
        let cases: [(&[u16], &[&str], DeviceKind); 9] = [
            (&[80, 9100], &[], DeviceKind::Printer),
            (&[80], &["ipp"], DeviceKind::Printer),
            (&[8008, 8009], &[], DeviceKind::Tv),
            (&[], &["googlecast"], DeviceKind::Tv),
            (&[80, 554], &[], DeviceKind::Camera),
            (&[22, 445, 5000, 5001], &[], DeviceKind::Nas),
            (&[22, 8006], &[], DeviceKind::Server),
            (&[135, 445], &[], DeviceKind::Desktop),
            (&[22, 80], &[], DeviceKind::Unknown),
        ];
        for (open, announced, kind) in cases {
            let (open, announced) = (ports(open), names(announced));
            assert_eq!(
                super::kind(&evidence(&open, &announced)),
                kind,
                "{open:?} {announced:?}"
            );
        }
    }

    #[test]
    fn an_apple_model_tells_the_kind_and_os() {
        for (model, kind, family) in [
            ("MacBookPro18,3", DeviceKind::Laptop, OsFamily::MacOs),
            ("Macmini9,1", DeviceKind::Desktop, OsFamily::MacOs),
            ("iPhone15,2", DeviceKind::Phone, OsFamily::Ios),
        ] {
            let e = Evidence {
                model: Some(model),
                ..evidence(&[], &[])
            };
            assert_eq!(super::kind(&e), kind, "{model}");
            let hint = os(&e).unwrap();
            assert_eq!(
                (hint.family, hint.clues),
                (family, vec![OsClue::DeviceModel])
            );
        }
    }

    #[test]
    fn advertised_models_distinguish_specific_smart_devices() {
        for (model, kind) in [
            ("Ecobee SmartThermostat", DeviceKind::Thermostat),
            ("Google Nest Hub Max", DeviceKind::SmartDisplay),
            ("Amazon Echo Dot", DeviceKind::VoiceAssistant),
            ("Shelly Plus 1", DeviceKind::Smart),
        ] {
            let evidence = Evidence {
                model: Some(model),
                ..evidence(&[], &[])
            };
            assert_eq!(super::kind(&evidence), kind, "{model}");
        }
    }

    #[test]
    fn the_strongest_clue_sets_the_os_and_only_agreeing_clues_are_kept() {
        let mut open = ports(&[22, 135, 445]);
        open[0].banner = Some("SSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13".into());
        let e = Evidence {
            ttl: Some(64),
            ..evidence(&open, &[])
        };
        let hint = os(&e).unwrap();
        assert_eq!(hint.family, OsFamily::Linux, "the banner beats the ports");
        assert_eq!(hint.detail.as_deref(), Some("Ubuntu"));
        assert_eq!(hint.clues, [OsClue::SshBanner]);
    }

    #[test]
    fn ttl_and_server_headers_point_to_windows_or_network_gear() {
        let mut web = ports(&[80]);
        web[0].http = Some(HttpProbe {
            status_line: None,
            headers: vec![HttpHeader {
                name: "Server".into(),
                value: "Microsoft-IIS/10.0".into(),
            }],
        });
        let e = Evidence {
            ttl: Some(128),
            ..evidence(&web, &[])
        };
        let hint = os(&e).unwrap();
        assert_eq!(hint.family, OsFamily::Windows);
        assert_eq!(hint.clues, [OsClue::ServerHeader, OsClue::Ttl(128)]);

        let switch = Evidence {
            ttl: Some(255),
            ..evidence(&[], &[])
        };
        assert_eq!(os(&switch).map(|h| h.family), Some(OsFamily::Network));
        let unix = Evidence {
            ttl: Some(64),
            ..evidence(&[], &[])
        };
        assert_eq!(os(&unix), None, "64 does not tell Linux from macOS");
    }
}
