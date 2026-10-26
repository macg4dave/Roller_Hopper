//! A single device on the network and the closed sets that describe it.
//!
//! User-visible wording for these types lives in the UI (`ui/labels.slint`) so it can be
//! translated; only language-neutral formats (such as the MAC address) are produced here.

use serde::{Deserialize, Serialize};

use super::card::NetworkCard;
use super::os::OsHint;
use super::probe_log::ProbeRecord;

/// Stable identifier of a device; not an index into any list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct DeviceId(pub(crate) u32);

/// An icon file in the icon folder, named by its path there without `.svg`, with `/` between
/// folders: `Network/Router`. The user can add icons, so this is an open set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Icon(pub(crate) String);

impl Icon {
    pub(crate) fn new(path: &str) -> Self {
        Self(path.to_owned())
    }
}

/// What the device is, as detected. Fixed; the user edits the [`Role`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum DeviceKind {
    Router,
    Switch,
    AccessPoint,
    Nas,
    Desktop,
    Laptop,
    Server,
    Printer,
    Phone,
    Tv,
    Console,
    Camera,
    Thermostat,
    SmartDisplay,
    VoiceAssistant,
    Smart,
    Unknown,
}

impl DeviceKind {
    /// The icon a device of this kind is detected with: one of the icons that ship with the app.
    pub(crate) fn icon(self) -> Icon {
        Icon::new(match self {
            Self::Router => "Network/Router",
            Self::Switch => "Network/Switch",
            Self::AccessPoint => "Network/Access point",
            Self::Nas => "Computers/NAS",
            Self::Desktop => "Computers/Desktop",
            Self::Laptop => "Computers/Laptop",
            Self::Server => "Computers/Server",
            Self::Printer => "Other/Printer",
            Self::Phone => "Phones and wearables/Phone",
            Self::Tv => "Entertainment/Smart TV",
            Self::Console => "Entertainment/Games console",
            Self::Camera => "Smart home/Camera",
            Self::Thermostat => "Smart home/Thermostat",
            Self::SmartDisplay => "Smart home/Smart display",
            Self::VoiceAssistant => "Smart home/Voice assistant",
            Self::Smart => "Smart home/Smart device",
            Self::Unknown => "Other/Unknown",
        })
    }

    /// The role a newly found device of this kind starts with, until the user changes it.
    pub(crate) fn default_role(self) -> Role {
        match self {
            Self::Router => Role::Router,
            Self::Switch => Role::Switch,
            Self::AccessPoint => Role::AccessPoint,
            Self::Nas => Role::Nas,
            Self::Desktop => Role::Desktop,
            Self::Laptop => Role::Laptop,
            Self::Server => Role::Server,
            Self::Printer => Role::Printer,
            Self::Phone => Role::Phone,
            Self::Camera => Role::Camera,
            Self::Smart => Role::Iot,
            Self::Tv
            | Self::Console
            | Self::Thermostat
            | Self::SmartDisplay
            | Self::VoiceAssistant
            | Self::Unknown => Role::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Status {
    Online,
    Offline,
    Unknown,
}

/// The user-assigned purpose of a device. Custom roles are the one open set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Role {
    Router,
    Switch,
    AccessPoint,
    Server,
    Nas,
    Desktop,
    Laptop,
    Phone,
    Printer,
    Camera,
    Iot,
    Other,
    Custom(String),
}

impl Role {
    pub(crate) const STANDARD: [Self; 12] = [
        Self::Router,
        Self::Switch,
        Self::AccessPoint,
        Self::Server,
        Self::Nas,
        Self::Desktop,
        Self::Laptop,
        Self::Phone,
        Self::Printer,
        Self::Camera,
        Self::Iot,
        Self::Other,
    ];

    /// The icon a standard role starts with, until the user changes it. `Other` keeps the
    /// detected icon, and a custom role's icon is picked when it is added.
    pub(crate) fn default_icon(&self) -> Option<Icon> {
        let kind = match self {
            Self::Router => DeviceKind::Router,
            Self::Switch => DeviceKind::Switch,
            Self::AccessPoint => DeviceKind::AccessPoint,
            Self::Server => DeviceKind::Server,
            Self::Nas => DeviceKind::Nas,
            Self::Desktop => DeviceKind::Desktop,
            Self::Laptop => DeviceKind::Laptop,
            Self::Phone => DeviceKind::Phone,
            Self::Printer => DeviceKind::Printer,
            Self::Camera => DeviceKind::Camera,
            Self::Iot => DeviceKind::Smart,
            Self::Other | Self::Custom(_) => return None,
        };
        Some(kind.icon())
    }
}

/// How long before the latest scan the device was last seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct LastSeen {
    pub(crate) minutes_ago: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Device {
    pub(crate) id: DeviceId,
    pub(crate) name: String,
    pub(crate) kind: DeviceKind,
    pub(crate) role: Role,
    /// An icon picked for this device alone; `None` follows the role.
    pub(crate) icon: Option<Icon>,
    pub(crate) status: Status,
    /// The device it runs on, such as a virtual machine's host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) host: Option<DeviceId>,
    /// Its children in the lists, and what stands for it on the map. Every device has at least
    /// one; the first is the one its address is shown from.
    pub(crate) cards: Vec<NetworkCard>,
    pub(crate) manufacturer: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) hostname: Option<String>,
    /// What a scan made of its operating system.
    pub(crate) os: Option<OsHint>,
    /// The mDNS service types it announced, such as `ipp` or `airplay`, in order, each once.
    pub(crate) mdns_services: Vec<String>,
    /// Device and service types declared over SSDP/UPnP.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) ssdp_services: Vec<String>,
    /// The responder's SSDP `SERVER` field, when it declared one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ssdp_server: Option<String>,
    /// What each protocol the last scan asked replied, for the current session's debug view;
    /// never saved to a project.
    #[serde(skip)]
    pub(crate) probe_log: Vec<ProbeRecord>,
    pub(crate) last_seen: LastSeen,
    pub(crate) notes: String,
}

impl Device {
    /// Case-insensitive match on name, or any card's IP or MAC address.
    pub(crate) fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.name.to_lowercase().contains(&query)
            || self.cards.iter().any(|card| card.matches(&query))
    }

    /// The card its address is shown from.
    pub(crate) fn primary_card(&self) -> Option<&NetworkCard> {
        self.cards.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::{CardId, MacAddr};

    fn device() -> Device {
        Device {
            id: DeviceId(1),
            name: "Living Room TV".into(),
            kind: DeviceKind::Tv,
            role: Role::Other,
            icon: None,
            status: Status::Online,
            host: None,
            cards: vec![
                NetworkCard {
                    address: Some("192.168.1.50/24".parse().unwrap()),
                    mac: Some(MacAddr([0x8c, 0x79, 0xf5, 0x1a, 0x2b, 0x3c])),
                    ..NetworkCard::blank(CardId(1))
                },
                NetworkCard {
                    address: Some("10.0.0.9/24".parse().unwrap()),
                    ..NetworkCard::blank(CardId(2))
                },
            ],
            manufacturer: None,
            model: None,
            hostname: None,
            os: None,
            mdns_services: Vec::new(),
            ssdp_services: Vec::new(),
            ssdp_server: None,
            probe_log: Vec::new(),
            last_seen: LastSeen::default(),
            notes: String::new(),
        }
    }

    #[test]
    fn search_matches_name_ip_and_mac_ignoring_case() {
        let d = device();
        assert!(d.matches("living"));
        assert!(d.matches("1.50"));
        assert!(d.matches("8c:79"));
        assert!(d.matches("10.0.0.9"), "any card");
        assert!(!d.matches("/24"), "not the prefix");
        assert!(d.matches("  "));
        assert!(!d.matches("printer"));
    }

    #[test]
    fn the_debug_probe_log_is_not_saved() {
        let mut device = device();
        device.probe_log = vec![ProbeRecord {
            protocol: super::super::ProbeProtocol::Ssdp,
            port: 1900,
            text: "HTTP/1.1 200 OK".into(),
        }];
        let saved = serde_json::to_value(device).unwrap();
        assert_eq!(saved.get("probe_log"), None);
    }

    const KINDS: [DeviceKind; 17] = [
        DeviceKind::Router,
        DeviceKind::Switch,
        DeviceKind::AccessPoint,
        DeviceKind::Nas,
        DeviceKind::Desktop,
        DeviceKind::Laptop,
        DeviceKind::Server,
        DeviceKind::Printer,
        DeviceKind::Phone,
        DeviceKind::Tv,
        DeviceKind::Console,
        DeviceKind::Camera,
        DeviceKind::Thermostat,
        DeviceKind::SmartDisplay,
        DeviceKind::VoiceAssistant,
        DeviceKind::Smart,
        DeviceKind::Unknown,
    ];

    #[test]
    fn a_new_device_shows_its_detected_icon() {
        for kind in KINDS {
            let icon = kind.default_role().default_icon();
            assert_eq!(icon.unwrap_or(kind.icon()), kind.icon(), "{kind:?}");
        }
    }

    #[test]
    fn every_detected_icon_ships_with_the_app() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons");
        for kind in KINDS {
            let file = folder.join(format!("{}.svg", kind.icon().0));
            assert!(file.is_file(), "{kind:?}: {}", file.display());
        }
    }
}
