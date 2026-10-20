//! A host a discovery run found: the device as detected, how it was found, and how it compares
//! with the devices on the map.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

use super::{LogEvent, ScanOptions};
use crate::model::card::{MacAddr, NetworkCard};
use crate::model::device::{Device, DeviceId, DeviceKind};
use crate::model::network::Network;
use crate::model::services::Services;

/// A way a host can be found or identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Method {
    Arp,
    Ping,
    Snmp,
    Mdns,
    Upnp,
    Onvif,
    Lldp,
}

impl Method {
    /// Whether it tells what a host is, beyond its address; only used with service detection.
    fn identifies(self) -> bool {
        match self {
            Self::Arp | Self::Ping => false,
            Self::Snmp | Self::Mdns | Self::Upnp | Self::Onvif | Self::Lldp => true,
        }
    }
}

/// A host that answers a scan, as the (simulated) network reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answer {
    pub(crate) device: Device,
    pub(crate) methods: Vec<Method>,
    /// How sure the identification is, in per cent.
    pub(crate) confidence: u8,
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
    /// As detected, with only what the scan's options could find out, and all its cards.
    pub(crate) device: Device,
    /// The subnet the run found it on; its cards elsewhere were not seen.
    pub(crate) subnet: Ipv4Net,
    /// The address it answered at, and that card's MAC address.
    pub(crate) ip: Ipv4Addr,
    pub(crate) mac: Option<MacAddr>,
    pub(crate) methods: Vec<Method>,
    pub(crate) confidence: u8,
    pub(crate) finding: Finding,
    pub(crate) decision: Decision,
}

impl Found {
    /// What a scan of `target` finds of `answer`: nothing unless one of its cards is on it.
    pub(super) fn detect(
        answer: Answer,
        target: Ipv4Net,
        options: ScanOptions,
        network: &Network,
    ) -> Option<Self> {
        let Answer {
            mut device,
            mut methods,
            confidence,
        } = answer;
        let on_target = device.cards.iter().find_map(|card| {
            let address = card.address.filter(|a| target.contains(&a.addr()))?;
            Some((address.addr(), card.mac))
        });
        let (ip, mac) = on_target?;
        if !options.ping_sweep {
            methods.retain(|&m| m != Method::Ping);
        }
        if !options.service_detection {
            methods.retain(|m| !m.identifies());
            (device.manufacturer, device.model, device.hostname) = (None, None, None);
        }
        if !options.port_scan {
            for card in &mut device.cards {
                card.services = Services::default();
            }
        }
        Some(Self {
            finding: finding(network, ip, mac),
            decision: Decision::Pending,
            device,
            subnet: target,
            ip,
            mac,
            methods,
            confidence,
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

    /// What the log says when the scan reaches it; nothing for a device already on the map.
    pub(super) fn event(&self) -> Option<LogEvent> {
        let ip = self.ip;
        match self.finding {
            Finding::Known(_) => None,
            Finding::Duplicate(of) => Some(LogEvent::PossibleDuplicate { ip, of }),
            Finding::New if self.unidentified() => {
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
