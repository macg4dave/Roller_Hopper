//! A host a discovery run found: the device as detected, how it was found, and how it compares
//! with the devices on the map.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

use super::LogEvent;
use crate::model::card::{MacAddr, NetworkCard};
use crate::model::device::{Device, DeviceId, DeviceKind};
use crate::model::network::Network;

/// A way a host was found or named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Method {
    /// The operating system's neighbour table had its MAC address.
    Arp,
    Ping,
    /// It announced its name over mDNS.
    Mdns,
}

/// A host that answered a scan, as the scanner reports it: the device it looks like, with only
/// what the scan's options found out, and how it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answer {
    pub(crate) device: Device,
    pub(crate) methods: Vec<Method>,
}

/// How a found host compares with the devices on the map, matched by MAC address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Finding {
    /// On the map already, with the same MAC and address.
    Known(DeviceId),
    New,
    /// At a new address, but with the MAC address of this device on the map.
    Duplicate(DeviceId),
}

/// What the user did with a host that is not on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decision {
    Pending,
    Added(DeviceId),
    Ignored,
}

/// The filter over the results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FoundFilter {
    #[default]
    All,
    New,
    Unidentified,
    Duplicates,
}

/// The discovery-results column that orders host groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FoundSort {
    Name,
    #[default]
    Ip,
    Mac,
    Role,
}

impl FoundFilter {
    pub(crate) fn admits(self, found: &Found) -> bool {
        match self {
            Self::All => true,
            Self::New => found.finding == Finding::New,
            Self::Unidentified => found.unidentified(),
            Self::Duplicates => matches!(found.finding, Finding::Duplicate(_)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    /// As detected, with all its cards.
    pub(crate) device: Device,
    /// The subnet the run found it on; its cards elsewhere were not seen.
    pub(crate) subnet: Ipv4Net,
    /// The address it answered at, and that card's MAC address.
    pub(crate) ip: Ipv4Addr,
    pub(crate) mac: Option<MacAddr>,
    pub(crate) methods: Vec<Method>,
    pub(crate) finding: Finding,
    pub(crate) decision: Decision,
}

impl Found {
    /// What a scan of `target` finds of `answer`: nothing unless one of its cards is on it.
    pub(super) fn detect(answer: Answer, target: Ipv4Net, network: &Network) -> Option<Self> {
        let Answer { device, methods } = answer;
        let on_target = device.cards.iter().find_map(|card| {
            let address = card.address.filter(|a| target.contains(&a.addr()))?;
            Some((address.addr(), card.mac))
        });
        let (ip, mac) = on_target?;
        Some(Self {
            finding: finding(network, ip, mac),
            decision: Decision::Pending,
            device,
            subnet: target,
            ip,
            mac,
            methods,
        })
    }

    /// Whether the run saw `card`, one of its device's: it is on the subnet scanned.
    pub(crate) fn seen(&self, card: &NetworkCard) -> bool {
        card.address
            .is_some_and(|a| self.subnet.contains(&a.addr()))
    }

    /// Nothing told what it is.
    pub(crate) fn unidentified(&self) -> bool {
        self.device.kind == DeviceKind::Unknown
    }

    /// It can still be added to the map.
    pub(crate) fn addable(&self) -> bool {
        !matches!(self.finding, Finding::Known(_)) && !matches!(self.decision, Decision::Added(_))
    }

    /// The device on the map it is: the known device, or the one it was added as.
    pub(crate) fn on_map(&self) -> Option<DeviceId> {
        if let Finding::Known(id) = self.finding {
            return Some(id);
        }
        match self.decision {
            Decision::Added(id) => Some(id),
            Decision::Pending | Decision::Ignored => None,
        }
    }

    /// What the log says when the scan finds it; nothing for a device already on the map. A
    /// host that told no name is worth a look; one of unknown kind alone is not, since most
    /// real hosts are.
    pub(super) fn event(&self) -> Option<LogEvent> {
        let ip = self.ip;
        match self.finding {
            Finding::Known(_) => None,
            Finding::Duplicate(of) => Some(LogEvent::PossibleDuplicate { ip, of }),
            Finding::New if self.device.name.trim().is_empty() => {
                Some(LogEvent::Unidentified { ip, mac: self.mac })
            }
            Finding::New => Some(LogEvent::Discovered {
                name: self.device.name.clone(),
                ip,
            }),
        }
    }
}

/// How a host at `ip` with `mac` compares with the devices on the map. The same MAC at another
/// address is a possible duplicate: a copied MAC, or a device that moved.
fn finding(network: &Network, ip: Ipv4Addr, mac: Option<MacAddr>) -> Finding {
    let Some(mac) = mac else {
        return Finding::New;
    };
    let cards = network
        .devices()
        .iter()
        .flat_map(|d| d.cards.iter().map(move |card| (d.id, card)));
    let mut duplicate = None;
    for (id, card) in cards.filter(|(_, card)| card.mac == Some(mac)) {
        if card.address.is_some_and(|a| a.addr() == ip) {
            return Finding::Known(id);
        }
        duplicate.get_or_insert(id);
    }
    duplicate.map_or(Finding::New, Finding::Duplicate)
}
