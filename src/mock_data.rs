//! Test helpers backed by the same saved project used by the hidden mock-map shortcut.

mod answers;

pub(crate) use answers::answers;

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

use crate::model::{
    CardId, Device, DeviceId, DeviceKind, LastSeen, MacAddr, Network, NetworkCard, Role, Services,
    Status,
};

const HOME: [u8; 3] = [192, 168, 1];
const LAB: [u8; 3] = [10, 0, 0];

/// A small discovered host used only by the discovery tests' synthetic scan answers.
struct Spec {
    id: u32,
    name: &'static str,
    kind: DeviceKind,
    role: Role,
    status: Status,
    subnet: [u8; 3],
    host: u8,
    prefix: u8,
    mac: [u8; 6],
    manufacturer: Option<&'static str>,
    model: Option<&'static str>,
    hostname: Option<&'static str>,
    minutes_ago: u32,
    services: &'static [(u16, &'static str)],
    notes: &'static str,
}

impl Spec {
    fn build(self) -> Device {
        let mut services = Services::default();
        for &(port, description) in self.services {
            services.add(port, description);
        }
        Device {
            id: DeviceId(self.id),
            name: self.name.to_owned(),
            kind: self.kind,
            role: self.role,
            icon: None,
            status: self.status,
            host: None,
            cards: card(self.id, self.subnet, self.host, self.prefix, self.mac)
                .map(|card| NetworkCard { services, ..card })
                .into_iter()
                .collect(),
            manufacturer: self.manufacturer.map(str::to_owned),
            model: self.model.map(str::to_owned),
            hostname: self.hostname.map(str::to_owned),
            os: None,
            mdns_services: Vec::new(),
            ssdp_services: Vec::new(),
            ssdp_server: None,
            probe_log: Vec::new(),
            last_seen: LastSeen {
                minutes_ago: self.minutes_ago,
            },
            notes: self.notes.to_owned(),
        }
    }
}

fn card(id: u32, [a, b, c]: [u8; 3], host: u8, prefix: u8, mac: [u8; 6]) -> Option<NetworkCard> {
    Some(NetworkCard {
        address: Some(Ipv4Net::new(Ipv4Addr::new(a, b, c, host), prefix).ok()?),
        mac: Some(MacAddr(mac)),
        ..NetworkCard::blank(CardId(id))
    })
}

fn spec(id: u32, name: &'static str, kind: DeviceKind, host: u8, mac: [u8; 6]) -> Spec {
    Spec {
        id,
        name,
        kind,
        role: kind.default_role(),
        status: Status::Online,
        subnet: HOME,
        host,
        prefix: 24,
        mac,
        manufacturer: None,
        model: None,
        hostname: None,
        minutes_ago: 0,
        services: &[],
        notes: "",
    }
}

pub(crate) fn network() -> Network {
    let mut network = crate::project::mock()
        .expect("the checked-in mock project must stay valid")
        .network;
    // Most unit tests exercise one operation against the compact home-network branch. The
    // hidden UI shortcut opens the complete project; keeping its independent infrastructure
    // branch out of each small test avoids turning geometry tests into stress tests.
    let infrastructure: Vec<_> = network
        .devices()
        .iter()
        .filter(|device| device.id.0 >= 1_000)
        .map(|device| device.id)
        .collect();
    for id in infrastructure {
        network.remove_device(id);
    }
    network
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;

    fn shipped_icons(folder: &Path, root: &Path, icons: &mut HashSet<String>) {
        for entry in fs::read_dir(folder).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                shipped_icons(&path, root, icons);
            } else if path.extension().is_some_and(|extension| extension == "svg") {
                let relative = path.strip_prefix(root).unwrap().with_extension("");
                icons.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    #[test]
    fn infrastructure_fixture_is_large_connected_and_uses_every_shipped_icon() {
        let network = crate::project::mock().unwrap().network;
        assert!(network.devices().len() >= 500);
        assert!(network.clone().checked().is_ok());

        let used: HashSet<_> = network
            .devices()
            .iter()
            .flat_map(|device| {
                std::iter::once(network.icon(device).0)
                    .chain(device.cards.iter().map(|card| card.icon.0.clone()))
            })
            .collect();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons");
        let mut shipped = HashSet::new();
        shipped_icons(&root, &root, &mut shipped);
        assert_eq!(used.intersection(&shipped).count(), shipped.len());
    }
}
