//! Who answers a simulated discovery run: the sample devices that are not offline, and a few
//! hosts that are not on the map yet, one of them with a MAC already on it.

use super::{LAB, Spec, spec};
use crate::model::{Answer, Device, DeviceKind, Method, Network, Status};

/// The answers a run gets on `network`, on every subnet; the run keeps those on its own.
pub(crate) fn answers(network: &Network) -> Vec<Answer> {
    let known = network
        .devices()
        .iter()
        .filter(|d| d.status != Status::Offline)
        .map(|device| {
            let (methods, confidence) = evidence(device.kind);
            answer(device.clone(), methods, confidence)
        });
    known.chain(newcomers()).collect()
}

fn answer(device: Device, methods: &[Method], confidence: u8) -> Answer {
    Answer {
        device,
        methods: methods.to_vec(),
        confidence,
    }
}

/// How a device of each kind is typically found, and how sure that makes the identification.
fn evidence(kind: DeviceKind) -> (&'static [Method], u8) {
    use Method::{Arp, Lldp, Mdns, Onvif, Ping, Snmp, Upnp};
    match kind {
        DeviceKind::Router => (&[Arp, Ping, Snmp, Upnp], 98),
        DeviceKind::Switch | DeviceKind::AccessPoint => (&[Arp, Snmp, Lldp], 97),
        DeviceKind::Nas => (&[Arp, Ping, Snmp, Mdns], 97),
        DeviceKind::Server => (&[Arp, Ping, Snmp], 94),
        DeviceKind::Desktop => (&[Arp, Ping], 90),
        DeviceKind::Laptop | DeviceKind::Phone => (&[Arp, Mdns], 92),
        DeviceKind::Printer => (&[Arp, Snmp, Mdns], 96),
        DeviceKind::Tv | DeviceKind::Console | DeviceKind::Smart => (&[Arp, Upnp], 88),
        DeviceKind::Camera => (&[Arp, Onvif], 90),
        DeviceKind::Unknown => (&[Arp, Ping], 55),
    }
}

/// Hosts not on the map. Their ids are placeholders: a device gets real ones when it is added.
#[expect(
    clippy::too_many_lines,
    reason = "flat table of sample data; splitting it hides the hosts"
)]
fn newcomers() -> Vec<Answer> {
    use DeviceKind as K;
    use Method::{Arp, Lldp, Mdns, Onvif, Ping, Snmp, Upnp};
    let hosts = [
        (
            Spec {
                manufacturer: Some("Microsoft"),
                model: Some("Surface Laptop 5"),
                hostname: Some("surface-guest"),
                ..spec(
                    0,
                    "Guest Laptop",
                    K::Laptop,
                    14,
                    [0x7C, 0x1E, 0x52, 0x0D, 0x33, 0xA1],
                )
            },
            &[Arp, Ping, Mdns][..],
            88,
        ),
        (
            Spec {
                manufacturer: Some("Sonos"),
                model: Some("Era 100"),
                hostname: Some("sonos-kitchen"),
                services: &[(1400, "UPnP")],
                ..spec(
                    0,
                    "Kitchen Speaker",
                    K::Smart,
                    82,
                    [0x48, 0xA6, 0xB8, 0x2C, 0x91, 0x0E],
                )
            },
            &[Arp, Mdns, Upnp],
            93,
        ),
        // Same MAC as the IP Camera Front at .70: a copied MAC, or the camera moved address.
        (
            Spec {
                manufacturer: Some("Reolink"),
                model: Some("RLC-810A"),
                services: &[(80, "HTTP"), (554, "RTSP")],
                ..spec(
                    0,
                    "IP Camera",
                    K::Camera,
                    71,
                    [0xEC, 0x71, 0xDB, 0x2E, 0x40, 0x9D],
                )
            },
            &[Arp, Onvif],
            85,
        ),
        (
            Spec {
                services: &[(8080, "")],
                ..spec(0, "", K::Unknown, 91, [0x6A, 0x0F, 0x3E, 0x92, 0x5B, 0x17])
            },
            &[Arp, Ping],
            40,
        ),
        (
            Spec {
                manufacturer: Some("MikroTik"),
                model: Some("CRS305"),
                hostname: Some("lab-switch"),
                subnet: LAB,
                ..spec(
                    0,
                    "Lab Switch",
                    K::Switch,
                    2,
                    [0x48, 0xA9, 0x8A, 0x21, 0x5C, 0x91],
                )
            },
            &[Arp, Snmp, Lldp],
            96,
        ),
    ];
    hosts
        .into_iter()
        .map(|(spec, methods, confidence)| answer(spec.build(), methods, confidence))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::mock_data;

    #[test]
    fn newcomers_have_free_addresses_on_known_subnets_and_one_copied_mac() {
        let network = mock_data::network();
        let cards: Vec<_> = network.devices().iter().flat_map(|d| &d.cards).collect();
        let ips: HashSet<_> = cards
            .iter()
            .filter_map(|c| c.address)
            .map(|a| a.addr())
            .collect();
        let macs: HashSet<_> = cards.iter().filter_map(|c| c.mac).collect();
        let subnets = network.subnets();
        let mut copied = 0;
        for answer in newcomers() {
            let card = &answer.device.cards[0];
            let address = card.address.unwrap();
            assert!(!ips.contains(&address.addr()), "{address}");
            assert!(subnets.contains(&address.trunc()), "{address}");
            copied += usize::from(macs.contains(&card.mac.unwrap()));
        }
        assert_eq!(copied, 1, "only the camera's MAC is seen twice");
    }

    #[test]
    fn offline_devices_do_not_answer() {
        let network = mock_data::network();
        let offline = network
            .devices()
            .iter()
            .filter(|d| d.status == Status::Offline);
        let answers = answers(&network);
        for device in offline {
            assert!(
                answers.iter().all(|a| a.device.id != device.id),
                "{}",
                device.name
            );
        }
    }
}
