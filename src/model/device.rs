//! A single device on the network and the closed sets that describe it.
//!
//! User-visible wording for these types lives in the UI (`ui/labels.slint`) so it can be
//! translated; only language-neutral formats (such as the MAC address) are produced here.

use std::fmt;
use std::net::Ipv4Addr;

use super::services::Services;

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

/// A MAC address, shown in the usual colon-separated upper-case form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MacAddr(pub(crate) [u8; 6]);

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let octets: Vec<String> = self.0.iter().map(|octet| format!("{octet:02X}")).collect();
        f.write_str(&octets.join(":"))
    }
}

/// How long before the latest (simulated) scan the device was last seen.
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
    pub(crate) ip: Ipv4Addr,
    pub(crate) mac: MacAddr,
    pub(crate) manufacturer: Option<&'static str>,
    pub(crate) model: Option<&'static str>,
    pub(crate) hostname: Option<&'static str>,
    pub(crate) last_seen: LastSeen,
    pub(crate) services: Services,
    pub(crate) notes: String,
}

impl Device {
    /// Case-insensitive match on name, IP address or MAC address.
    pub(crate) fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.name.to_lowercase().contains(&query)
            || self.ip.to_string().contains(&query)
            || self.mac.to_string().to_lowercase().contains(&query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> Device {
        Device {
            id: DeviceId(1),
            name: "Living Room TV".into(),
            kind: DeviceKind::Tv,
            role: Role::Other,
            icon: None,
            status: Status::Online,
            ip: Ipv4Addr::new(192, 168, 1, 50),
            mac: MacAddr([0x8c, 0x79, 0xf5, 0x1a, 0x2b, 0x3c]),
            manufacturer: None,
            model: None,
            hostname: None,
            last_seen: LastSeen::default(),
            services: Services::default(),
            notes: String::new(),
        }
    }

    #[test]
    fn search_matches_name_ip_and_mac_ignoring_case() {
        let d = device();
        assert!(d.matches("living"));
        assert!(d.matches("1.50"));
        assert!(d.matches("8c:79"));
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
