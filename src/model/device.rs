//! A single device on the network and the closed sets that describe it.
//!
//! User-visible wording for these types lives in the UI (`ui/labels.slint`) so it can be
//! translated; only language-neutral formats (MAC address, port list) are produced here.

use std::fmt;
use std::net::Ipv4Addr;

/// Stable identifier of a device; not an index into any list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct DeviceId(pub(crate) u32);

/// What the device is, as detected. Fixed; the user edits the [`Role`] instead. Each kind has its
/// own icon, so this is also the set of icons a role can show.
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

    /// The icon a standard role gives its devices. `Other` keeps the detected icon, and a custom
    /// role's icon is stored with the network's custom roles.
    pub(crate) fn icon(&self) -> Option<DeviceKind> {
        match self {
            Self::Router => Some(DeviceKind::Router),
            Self::Switch => Some(DeviceKind::Switch),
            Self::AccessPoint => Some(DeviceKind::AccessPoint),
            Self::Server => Some(DeviceKind::Server),
            Self::Nas => Some(DeviceKind::Nas),
            Self::Desktop => Some(DeviceKind::Desktop),
            Self::Laptop => Some(DeviceKind::Laptop),
            Self::Phone => Some(DeviceKind::Phone),
            Self::Printer => Some(DeviceKind::Printer),
            Self::Camera => Some(DeviceKind::Camera),
            Self::Iot => Some(DeviceKind::Smart),
            Self::Other | Self::Custom(_) => None,
        }
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

/// An open port and the service usually found on it. The name is a protocol name, not prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Service {
    pub(crate) port: u16,
    pub(crate) name: &'static str,
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.port, self.name)
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
    pub(crate) icon: Option<DeviceKind>,
    pub(crate) status: Status,
    pub(crate) ip: Ipv4Addr,
    pub(crate) mac: MacAddr,
    pub(crate) manufacturer: Option<&'static str>,
    pub(crate) model: Option<&'static str>,
    pub(crate) hostname: Option<&'static str>,
    pub(crate) last_seen: LastSeen,
    pub(crate) services: Vec<Service>,
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
            services: Vec::new(),
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

    #[test]
    fn a_new_device_shows_its_detected_icon() {
        let kinds = [
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
        for kind in kinds {
            let icon = kind.default_role().icon().unwrap_or(kind);
            assert_eq!(icon, kind, "{kind:?}");
        }
    }
}
