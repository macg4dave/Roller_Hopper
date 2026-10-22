//! A single device on the network and the closed sets that describe it.
//!
//! User-visible wording for these types lives in the UI (`ui/labels.slint`) so it can be
//! translated; only language-neutral formats (such as the MAC address) are produced here.

use super::card::NetworkCard;

/// Stable identifier of a device; not an index into any list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct DeviceId(pub(crate) u32);

/// An icon file in the icon folder, named by its path there without `.svg`, with `/` between
/// folders: `Network/Router`. The user can add icons, so this is an open set.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Icon(pub(crate) String);

impl Icon {
    pub(crate) fn new(path: &str) -> Self {
        Self(path.to_owned())
    }
}

/// What the device is, as detected. Fixed; the user edits the [`Role`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
            Self::Tv | Self::Console | Self::Unknown => Role::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Status {
    Online,
    Offline,
    Unknown,
}

/// The user-assigned purpose of a device. Custom roles are the one open set.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// What a guest runs as on its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GuestKind {
    VirtualMachine,
    Container,
}

impl GuestKind {
    /// The icon a guest of this kind is shown with when neither it nor its role has one: one of
    /// the icons that ship with the app. Their category is the one a guest's icon picker opens on.
    pub(crate) fn icon(self) -> Icon {
        Icon::new(match self {
            Self::VirtualMachine => "Virtual machines/Virtual machine",
            Self::Container => "Virtual machines/Container",
        })
    }
}

/// A device that runs on another one, which may itself be a guest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Guest {
    pub(crate) host: DeviceId,
    pub(crate) kind: GuestKind,
}

/// How long before the latest scan the device was last seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LastSeen {
    pub(crate) minutes_ago: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Device {
    pub(crate) id: DeviceId,
    pub(crate) name: String,
    pub(crate) kind: DeviceKind,
    pub(crate) role: Role,
    /// An icon picked for this device alone; `None` follows the role.
    pub(crate) icon: Option<Icon>,
    pub(crate) status: Status,
    /// Its children in the lists, and what stands for it on the map. Every device has at least
    /// one; the first is the one its address is shown from.
    pub(crate) cards: Vec<NetworkCard>,
    /// The host it runs on, for a virtual machine or container; `None` for a physical device.
    pub(crate) guest: Option<Guest>,
    pub(crate) manufacturer: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) hostname: Option<String>,
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
            guest: None,
            manufacturer: None,
            model: None,
            hostname: None,
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

    const KINDS: [DeviceKind; 14] = [
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
