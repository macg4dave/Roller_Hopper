//! Adding devices, the virtual machines and containers a device hosts, and forgetting a device
//! with them.

use super::{ConnectionKind, Network};
use crate::model::card::{CardId, NetworkCard};
use crate::model::device::{Device, DeviceId, DeviceKind, Guest, LastSeen, Role, Status};

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

    /// Adds a blank device with one blank network card, and returns it. Nothing about it was
    /// detected, so the user fills it in. A `guest` comes after its host's other guests, with
    /// its card plugged into the host's first card; refused when the host is missing. Any other
    /// device comes last, plugged in nowhere.
    pub(crate) fn add_device(&mut self, guest: Option<Guest>) -> Option<DeviceId> {
        let host_card = match guest {
            Some(Guest { host, .. }) => Some(self.device(host)?.primary_card()?.id),
            None => None,
        };
        let device = Device {
            id: DeviceId(0),
            name: String::new(),
            kind: DeviceKind::Unknown,
            role: Role::Other,
            icon: None,
            status: Status::Unknown,
            cards: vec![NetworkCard::blank(CardId(0))],
            guest,
            manufacturer: None,
            model: None,
            hostname: None,
            os: None,
            mdns_services: Vec::new(),
            last_seen: LastSeen::default(),
            notes: String::new(),
        };
        let uplink = host_card.map(|card| (card, ConnectionKind::Known));
        Some(self.insert_device(device, uplink))
    }

    /// Adds `device` under a fresh id, its cards under fresh ids, and returns its id. A guest
    /// comes after its host's other guests, any other device last. Its first card is plugged
    /// into `uplink`'s card, as a link of that kind.
    pub(crate) fn insert_device(
        &mut self,
        mut device: Device,
        uplink: Option<(CardId, ConnectionKind)>,
    ) -> DeviceId {
        device.id = DeviceId(self.devices.iter().map(|d| d.id.0).max().unwrap_or(0) + 1);
        let first_card = self.next_card_id().0;
        for (card, id) in device.cards.iter_mut().zip(first_card..) {
            card.id = CardId(id);
        }
        let host = device.guest.map(|g| g.host);
        let after_guests = host.and_then(|host| {
            let last = self
                .devices
                .iter()
                .rposition(|d| self.runs_within(d.id, host));
            last.map(|at| at + 1)
        });
        let (id, card) = (device.id, device.primary_card().map(|c| c.id));
        self.devices
            .insert(after_guests.unwrap_or(self.devices.len()), device);
        if let Some(card) = card {
            self.plug_found(card, uplink);
        }
        id
    }

    /// Plugs a card just added into `uplink`'s card, as a link of that kind; where that is not
    /// allowed, it is plugged in nowhere.
    pub(super) fn plug_found(&mut self, card: CardId, uplink: Option<(CardId, ConnectionKind)>) {
        if let Some((upstream, kind)) = uplink
            && self.plug_card(card, Some(upstream))
        {
            let links = self.links.iter_mut();
            links.filter(|l| l.card == card).for_each(|l| l.kind = kind);
        }
    }

    /// Forgets a device with everything that runs on it, the links of their cards and the links
    /// into them, and returns those it forgot. Cards plugged into theirs stay, as separate branches of
    /// the map.
    pub(crate) fn remove_device(&mut self, id: DeviceId) -> Vec<DeviceId> {
        let forgotten = self.devices.iter().filter(|d| self.runs_within(d.id, id));
        let (ids, cards): (Vec<DeviceId>, Vec<Vec<CardId>>) = forgotten
            .map(|d| (d.id, d.cards.iter().map(|c| c.id).collect()))
            .unzip();
        let cards = cards.concat();
        self.devices.retain(|d| !ids.contains(&d.id));
        self.links
            .retain(|l| !cards.contains(&l.upstream) && !cards.contains(&l.card));
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::device::GuestKind;

    const SWITCH: DeviceId = DeviceId(2);
    const PC: DeviceId = DeviceId(5);
    const LINUX_SERVER: DeviceId = DeviceId(6);
    const DOCKER_HOST: DeviceId = DeviceId(19);
    const LAB_ROUTER: DeviceId = DeviceId(22);

    #[test]
    fn forgetting_a_device_drops_it_and_its_links() {
        let mut network = mock_data::network();
        assert_eq!(network.remove_device(SWITCH), [SWITCH]);
        assert!(network.device(SWITCH).is_none());
        assert!(
            network
                .connections()
                .iter()
                .all(|c| ![c.upstream, c.downstream].contains(&CardId(2)))
        );
        assert!(network.uplink(CardId(5)).is_none(), "the PC stands alone");
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
        assert_ne!(on_docker, [] as [DeviceId; 0]);
        let forgotten = network.remove_device(LINUX_SERVER);
        assert!(forgotten.contains(&DOCKER_HOST));
        assert!(on_docker.iter().all(|id| forgotten.contains(id)));
        assert!(
            network
                .connections()
                .iter()
                .all(|c| c.downstream != CardId(100)),
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
        assert!(network.uplink(lab).is_none(), "heads its own tree");
    }

    #[test]
    fn a_guest_is_added_blank_under_its_host_after_the_hosts_other_guests() {
        let mut network = mock_data::network();
        let nowhere = Guest {
            host: DeviceId(999),
            kind: GuestKind::VirtualMachine,
        };
        assert_eq!(network.add_device(Some(nowhere)), None);

        let web = network
            .add_device(Some(Guest {
                host: DOCKER_HOST,
                kind: GuestKind::Container,
            }))
            .unwrap();
        let added = network.device(web).unwrap();
        assert_eq!(added.name, "");
        assert_eq!(added.guest.map(|g| g.kind), Some(GuestKind::Container));
        assert_eq!(network.icon(added), GuestKind::Container.icon());
        assert_eq!(added.cards, [NetworkCard::blank(added.cards[0].id)]);
        assert_eq!(
            network
                .device(web)
                .and_then(|d| network.uplink(d.cards[0].id))
                .map(|c| c.upstream),
            Some(CardId(19)),
            "plugged into its host's first card"
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
    fn a_device_of_its_own_is_added_blank_last_and_plugged_in_nowhere() {
        let mut network = mock_data::network();
        let pc_card = network.device(PC).unwrap().cards[0].id;
        let pc_uplink = network.uplink(pc_card);
        let id = network.add_device(None).unwrap();
        let added = network.devices().last().unwrap();
        assert_eq!(added.id, id);
        assert_eq!((added.guest, added.icon.as_ref()), (None, None));
        assert!(network.uplink(added.cards[0].id).is_none());
        assert_eq!(network.uplink(pc_card), pc_uplink, "others stay put");
        assert_ne!(network.add_device(None), Some(id), "a fresh id each time");
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
