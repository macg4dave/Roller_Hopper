//! A best guess at a device's operating system, and the clues behind it, as data: the UI words
//! the family and the clues. It is a hint: a host can run anything behind any of them.

use serde::{Deserialize, Serialize};

/// The operating system families a scan can tell apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum OsFamily {
    Windows,
    MacOs,
    Linux,
    Ios,
    /// Routers, switches, printers and other embedded systems.
    Network,
}

/// Something a scan saw that points at an operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum OsClue {
    /// The model the device announced over mDNS, such as `MacBookPro18,3`.
    DeviceModel,
    /// TCP 62078, which iPhones and iPads open for syncing.
    AppleSyncPort,
    /// The SSH server's greeting, such as `OpenSSH_9.6p1 Ubuntu-3ubuntu13`.
    SshBanner,
    /// A web server's `Server` header, such as `Apache/2.4.58 (Ubuntu)`.
    ServerHeader,
    /// TCP 135 and 445 together: Windows' RPC and file sharing.
    WindowsPorts,
    /// The time-to-live of its ping reply, which each family starts at its own value.
    Ttl(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OsHint {
    pub(crate) family: OsFamily,
    /// More precise, when a clue said, such as `Ubuntu`; as the device gave it.
    pub(crate) detail: Option<String>,
    /// The clues that point to `family`, strongest first.
    pub(crate) clues: Vec<OsClue>,
}
