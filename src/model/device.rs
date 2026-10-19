//! A single device on the network and the closed sets that describe it.

use std::fmt;
use std::net::Ipv4Addr;

/// Stable identifier of a device; not an index into any list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct DeviceId(pub(crate) u32);

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

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Router => "Router",
            Self::Switch => "Switch",
            Self::AccessPoint => "Access point",
            Self::Nas => "NAS",
            Self::Desktop => "Desktop",
            Self::Laptop => "Laptop",
            Self::Server => "Server",
            Self::Printer => "Printer",
            Self::Phone => "Phone",
            Self::Tv => "Smart TV",
            Self::Console => "Games console",
            Self::Camera => "Camera",
            Self::Smart => "Smart device",
            Self::Unknown => "Unknown",
        })
    }
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

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Online => "Online",
            Self::Offline => "Offline",
            Self::Unknown => "Unknown",
        })
    }
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
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Router => "Router",
            Self::Switch => "Switch",
            Self::AccessPoint => "Access point",
            Self::Server => "Server",
            Self::Nas => "NAS",
            Self::Desktop => "Desktop",
            Self::Laptop => "Laptop",
            Self::Phone => "Phone",
            Self::Printer => "Printer",
            Self::Camera => "Camera",
            Self::Iot => "IoT",
            Self::Other => "Other",
            Self::Custom(name) => name,
        })
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

/// An open port and the service usually found on it.
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

impl fmt::Display for LastSeen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let plural = |n: u32| if n == 1 { "" } else { "s" };
        match self.minutes_ago {
            0 => f.write_str("At last scan"),
            m @ 1..60 => write!(f, "{m} minute{} ago", plural(m)),
            m @ 60..1440 => write!(f, "{} hour{} ago", m / 60, plural(m / 60)),
            m => write!(f, "{} day{} ago", m / 1440, plural(m / 1440)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Device {
    pub(crate) id: DeviceId,
    pub(crate) name: String,
    pub(crate) kind: DeviceKind,
    pub(crate) role: Role,
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
    /// The name to show; never blank, so a cleared name still leaves something to click.
    pub(crate) fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            "Unnamed device"
        } else {
            &self.name
        }
    }

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
    fn blank_name_falls_back_to_placeholder() {
        let mut d = device();
        d.name = "   ".into();
        assert_eq!(d.display_name(), "Unnamed device");
    }

    #[test]
    fn last_seen_reads_naturally() {
        let at = |minutes_ago| LastSeen { minutes_ago }.to_string();
        assert_eq!(at(0), "At last scan");
        assert_eq!(at(1), "1 minute ago");
        assert_eq!(at(125), "2 hours ago");
        assert_eq!(at(1440), "1 day ago");
    }
}
