//! The virtual machines and containers a device hosts, and forgetting a device with them.

use ipnet::Ipv4Net;

use super::Network;
use crate::model::card::{CardId, NetworkCard};
use crate::model::device::{
    Device, DeviceId, DeviceKind, Guest, GuestKind, LastSeen, Role, Status,
};
use crate::model::services::Services;

/// `devices` in the same order, but with each guest moved straight after its host and the
/// host's earlier guests.
pub(super) fn hosts_first(mut devices: Vec<Device>) -> Vec<Device> {
    let ids: Vec<DeviceId> = devices.iter().map(|d| d.id).collect();
    let host_of = |d: &Device| d.guest.map(|g| g.host).filter(|h| ids.contains(h));
    let mut order = Vec::with_capacity(devices.len());
    let tops = devices.iter().filter(|d| host_of(d).is_none());
    let mut pending: Vec<DeviceId> = tops.map(|d| d.id).rev().collect();
    while let Some(id) = pending.pop() {
        order.push(id);
        let guests = devices.iter().filter(|d| host_of(d) == Some(id));
        pending.extend(guests.map(|d| d.id).rev());
    }
    // Guests in a loop of hosts reach no top; they keep their order at the end.
    devices.sort_by_key(|d| {
        order
            .iter()
            .position(|&id| id == d.id)
            .unwrap_or(usize::MAX)
    });
    devices
}

impl Network {
    /// Whether `id` is `host` itself or runs on it, directly or on one of its guests.
    pub(crate) fn runs_within(&self, id: DeviceId, host: DeviceId) -> bool {
        let mut current = Some(id);
        for _ in 0..=self.devices.len() {
            match current {
                Some(at) if at == host => return true,
                Some(at) => current = self.host(at),
                None => return false,
            }
        }
        false
    }

    /// How many hosts `id` runs within: 0 for a physical device.
    pub(crate) fn depth(&self, id: DeviceId) -> usize {
        let hosts = std::iter::successors(self.host(id), |&at| self.host(at));
        hosts.take(self.devices.len()).count()
    }

    /// Adds a virtual machine or container named `name` to `host`, with one network card at
    /// `address` that reaches the network through the host, and returns it. Nothing about it
    /// was detected. Refused when the host is missing, the name is blank or the address is in
    /// use.
    pub(crate) fn add_guest(
        &mut self,
        host: DeviceId,
        kind: GuestKind,
        name: &str,
        address: Ipv4Net,
    ) -> Option<DeviceId> {
        let name = name.trim();
        let refused = self.device(host).is_none() || self.address_taken(address.addr(), None);
        if refused || name.is_empty() {
            return None;
        }
        let id = DeviceId(self.devices.iter().map(|d| d.id.0).max().unwrap_or(0) + 1);
        let guest = Device {
            id,
            name: name.to_owned(),
            kind: DeviceKind::Unknown,
            role: Role::Other,
            icon: None,
            status: Status::Unknown,
            cards: vec![NetworkCard {
                address: Some(address),
                ..NetworkCard::blank(self.next_card_id())
            }],
            guest: Some(Guest { host, kind }),
            manufacturer: None,
            model: None,
            hostname: None,
            last_seen: LastSeen::default(),
            services: Services::default(),
            notes: String::new(),
        };
        let last = self
            .devices
            .iter()
            .rposition(|d| self.runs_within(d.id, host));
        let at = last.map_or(self.devices.len(), |at| at + 1);
        self.devices.insert(at, guest);
        Some(id)
    }

    /// Forgets a device with everything that runs on it, the links of their cards and the links
    /// into them, and returns those it forgot. Devices behind them stay, as separate branches of
    /// the map.
    pub(crate) fn remove_device(&mut self, id: DeviceId) -> Vec<DeviceId> {
        let forgotten = self.devices.iter().filter(|d| self.runs_within(d.id, id));
        let (ids, cards): (Vec<DeviceId>, Vec<Vec<CardId>>) = forgotten
            .map(|d| (d.id, d.cards.iter().map(|c| c.id).collect()))
            .unzip();
        let cards = cards.concat();
        self.devices.retain(|d| !ids.contains(&d.id));
        for links in [&mut self.links, &mut self.detected] {
            links.retain(|l| !cards.contains(&l.upstream) && !cards.contains(&l.card));
        }
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::network::Node;

    const SWITCH: DeviceId = DeviceId(2);
    const PC: DeviceId = DeviceId(5);
    const LINUX_SERVER: DeviceId = DeviceId(6);
    const DOCKER_HOST: DeviceId = DeviceId(19);
    const LAB_ROUTER: DeviceId = DeviceId(22);

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    #[test]
    fn forgetting_a_device_drops_it_and_its_links() {
        let mut network = mock_data::network();
        assert_eq!(network.remove_device(SWITCH), [SWITCH]);
        assert!(network.device(SWITCH).is_none());
        assert!(
            network
                .connections()
                .iter()
                .all(|c| ![c.upstream, c.downstream].contains(&Node::Device(SWITCH)))
        );
        assert!(network.uplink(Node::Device(PC)).is_none(), "stands alone");
    }

    #[test]
    fn forgetting_a_host_forgets_its_guests_and_theirs() {
        let mut network = mock_data::network();
        let on_docker: Vec<_> = network
            .devices()
            .iter()
            .filter(|d| network.host(d.id) == Some(DOCKER_HOST))
            .map(|d| d.id)
            .collect();
        assert!(!on_docker.is_empty());
        let forgotten = network.remove_device(LINUX_SERVER);
        assert!(forgotten.contains(&DOCKER_HOST));
        assert!(on_docker.iter().all(|id| forgotten.contains(id)));
        let lab_card = Node::Card(CardId(100));
        assert!(
            network
                .connections()
                .iter()
                .all(|c| c.downstream != lab_card),
            "Docker Host's lab card"
        );
        assert!(network.device(LAB_ROUTER).is_some());
    }

    #[test]
    fn forgetting_where_a_further_card_is_plugged_in_keeps_its_device() {
        let mut network = mock_data::network();
        assert_eq!(network.remove_device(LAB_ROUTER), [LAB_ROUTER]);
        let docker = network.device(DOCKER_HOST).unwrap();
        assert_eq!(docker.cards.len(), 2);
        let lab = docker.cards[1].id;
        let uplink = network.uplink(Node::Card(lab)).map(|c| c.upstream);
        assert_eq!(
            uplink,
            Some(Node::Device(DOCKER_HOST)),
            "back under its device"
        );
    }

    #[test]
    fn a_guest_is_added_under_its_host_after_the_hosts_other_guests() {
        let mut network = mock_data::network();
        let free = net("172.17.0.9/16");
        let taken = network.device(PC).unwrap().cards[0].address.unwrap();
        let vm = GuestKind::VirtualMachine;
        assert_eq!(network.add_guest(DOCKER_HOST, vm, " ", free), None);
        assert_eq!(network.add_guest(DOCKER_HOST, vm, "Web", taken), None);
        assert_eq!(network.add_guest(DeviceId(999), vm, "Web", free), None);

        let web = network
            .add_guest(DOCKER_HOST, GuestKind::Container, " Web ", free)
            .unwrap();
        let added = network.device(web).unwrap();
        assert_eq!(added.name, "Web");
        assert_eq!(added.guest.map(|g| g.kind), Some(GuestKind::Container));
        assert_eq!(
            network.uplink(Node::Device(web)).map(|c| c.upstream),
            Some(Node::Device(DOCKER_HOST))
        );
        assert_eq!(network.depth(web), 2, "on a virtual machine");

        let ids: Vec<_> = network.devices().iter().map(|d| d.id).collect();
        let at = ids.iter().position(|&d| d == web).unwrap();
        let before = ids[at - 1];
        assert_eq!(
            network.host(before),
            Some(DOCKER_HOST),
            "after the other guests"
        );
        let next = ids.get(at + 1).copied();
        assert!(next.is_none_or(|next| !network.runs_within(next, DOCKER_HOST)));
    }

    #[test]
    fn guests_follow_their_host_in_every_list() {
        let network = mock_data::network();
        let devices = network.devices();
        for (at, device) in devices.iter().enumerate().skip(1) {
            let Some(host) = network.host(device.id) else {
                continue;
            };
            let before = &devices[at - 1];
            assert!(network.runs_within(before.id, host), "{}", device.name);
        }
    }
}
