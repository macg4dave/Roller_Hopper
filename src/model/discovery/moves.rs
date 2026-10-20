//! Making a network card of a found host not on the map a child of another parent: another such
//! host, which it is then added to the map with, or a device on the map, which takes it at
//! once. A host left with no card was only ever that card, so it leaves the results.

use super::{Discovery, Found, gateway};
use crate::model::device::DeviceId;
use crate::model::network::{ConnectionKind, Network};

/// Where a card of a found host not on the map can go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Parent {
    /// A found host not on the map, by its place in [`Discovery::found`].
    Found(usize),
    Device(DeviceId),
}

impl Discovery {
    /// The parent the found host at `index` stands for: its device, once on the map, or the host
    /// itself while it can still be added. `None` for one added and since forgotten.
    pub(crate) fn parent(&self, index: usize, network: &Network) -> Option<Parent> {
        let found = self.found().get(index)?;
        match found.on_map() {
            Some(id) => network.device(id).map(|_| Parent::Device(id)),
            None => Some(Parent::Found(index)),
        }
    }

    /// Every parent a card of a found host can have, in the order the "Child of" dropdown lists
    /// them: the found hosts not on the map, in address order, then the devices on the map.
    pub(crate) fn parents(&self, network: &Network) -> Vec<Parent> {
        let found = self.found().iter().enumerate();
        let hosts = found
            .filter(|(_, f)| f.addable())
            .map(|(at, _)| Parent::Found(at));
        let devices = network.devices().iter().map(|d| Parent::Device(d.id));
        hosts.chain(devices).collect()
    }

    /// Whether the host at `index` has just the one card, which moving away drops the host.
    pub(crate) fn only_card(&self, index: usize) -> bool {
        self.found()
            .get(index)
            .is_some_and(|f| f.device.cards.len() == 1)
    }

    /// Whether the card at `card` among those of the host at `index`, not on the map, may go to
    /// `to`: another found host not on the map, or a device on the map where no card has its
    /// address.
    pub(crate) fn can_move_card(
        &self,
        index: usize,
        card: usize,
        to: Parent,
        network: &Network,
    ) -> bool {
        let source = self.found().get(index).filter(|f| f.addable());
        let Some(moved) = source.and_then(|f| f.device.cards.get(card)) else {
            return false;
        };
        match to {
            Parent::Found(target) => {
                target != index && self.found().get(target).is_some_and(Found::addable)
            }
            Parent::Device(id) => {
                network.device(id).is_some()
                    && !moved
                        .address
                        .is_some_and(|a| network.address_taken(a.addr(), None))
            }
        }
    }

    /// Moves the card at `card` among those of the host at `index`, not on the map, to `to`: after
    /// another found host's cards, or onto the map after a device's cards, plugged into the
    /// router on its subnet by an inferred link. A host left with no card leaves the results.
    /// Refused (false) where [`Self::can_move_card`] does not allow it.
    pub(crate) fn move_card(
        &mut self,
        index: usize,
        card: usize,
        to: Parent,
        network: &mut Network,
    ) -> bool {
        if !self.can_move_card(index, card, to, network) {
            return false;
        }
        let Some(moved) = self.found_mut(index).map(|f| f.device.cards.remove(card)) else {
            return false;
        };
        match to {
            Parent::Found(target) => {
                if let Some(found) = self.found_mut(target) {
                    found.device.cards.push(moved);
                }
            }
            Parent::Device(id) => {
                let ip = moved.address.map(|a| a.addr());
                let router = ip.and_then(|ip| gateway(network, ip));
                let uplink = router.map(|socket| (socket, ConnectionKind::Inferred));
                network.adopt_card(id, moved, uplink);
            }
        }
        if self
            .found()
            .get(index)
            .is_some_and(|f| f.device.cards.is_empty())
        {
            self.drop_host(index);
        }
        true
    }

    /// Takes the host at `index` out of the results, keeping the selection on the same host.
    fn drop_host(&mut self, index: usize) {
        if index >= self.revealed {
            return;
        }
        self.hosts.remove(index);
        self.revealed -= 1;
        self.selected = match self.selected {
            Some(at) if at == index => None,
            Some(at) if at > index => Some(at - 1),
            other => other,
        };
        if self.selected.is_none() {
            self.selected_card = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::card::CardId;
    use crate::model::discovery::STEPS_PER_RUN;

    fn finished() -> (Discovery, Network) {
        let network = mock_data::network();
        let mut discovery = Discovery::default();
        let home = "192.168.1.0/24".parse().unwrap();
        discovery.start(home, mock_data::answers(&network), &network);
        for _ in 0..STEPS_PER_RUN {
            discovery.step();
        }
        (discovery, network)
    }

    fn at(discovery: &Discovery, name: &str) -> usize {
        let mut found = discovery.found().iter();
        found.position(|f| f.device.name == name).unwrap()
    }

    #[test]
    fn a_found_hosts_only_card_joins_a_device_on_the_map_and_the_host_goes() {
        let (mut discovery, mut network) = finished();
        let speaker = at(&discovery, "Kitchen Speaker");
        let card = discovery.found()[speaker].device.cards[0].clone();
        let router = DeviceId(1);
        assert!(discovery.only_card(speaker));
        let count = discovery.found().len();
        discovery.selected = Some(count - 1);
        assert!(discovery.move_card(speaker, 0, Parent::Device(router), &mut network));
        assert_eq!(discovery.found().len(), count - 1, "out of the results");
        assert_eq!(discovery.selected, Some(count - 2), "still the same host");
        let adopted = network.device(router).unwrap().cards.last().unwrap();
        assert_eq!((adopted.address, adopted.mac), (card.address, card.mac));
        assert!(
            network.uplink(adopted.id).is_none(),
            "not into the router's own card"
        );
    }

    #[test]
    fn a_found_card_moves_between_hosts_not_on_the_map() {
        let (mut discovery, mut network) = finished();
        let (speaker, laptop) = (
            at(&discovery, "Kitchen Speaker"),
            at(&discovery, "Guest Laptop"),
        );
        let before = network.clone();
        assert!(!discovery.move_card(speaker, 0, Parent::Found(speaker), &mut network));
        assert!(!discovery.move_card(speaker, 9, Parent::Found(laptop), &mut network));
        let nas = at(&discovery, "NAS");
        assert!(
            !discovery.move_card(speaker, 0, Parent::Found(nas), &mut network),
            "on the map: its device instead"
        );
        assert!(!discovery.move_card(nas, 0, Parent::Found(speaker), &mut network));
        assert!(discovery.only_card(laptop));
        assert!(discovery.move_card(laptop, 0, Parent::Found(speaker), &mut network));
        let merged = at(&discovery, "Kitchen Speaker");
        assert_eq!(discovery.found()[merged].device.cards.len(), 2);
        assert_eq!(network, before, "nothing on the map yet");
        let names = discovery.found().iter().map(|f| f.device.name.as_str());
        assert!(names.into_iter().all(|name| name != "Guest Laptop"), "gone");
    }

    #[test]
    fn a_found_card_is_refused_where_its_address_is_already_on_the_map() {
        let (mut discovery, mut network) = finished();
        let speaker = at(&discovery, "Kitchen Speaker");
        let taken = network.card(CardId(5)).and_then(|c| c.address);
        discovery.edit_card(speaker, 0, |c| c.address = taken);
        let to = Parent::Device(DeviceId(1));
        assert!(!discovery.can_move_card(speaker, 0, to, &network));
        assert!(!discovery.move_card(speaker, 0, to, &mut network));
    }

    #[test]
    fn a_host_stands_for_its_device_once_on_the_map() {
        let (discovery, mut network) = finished();
        let (nas, speaker) = (at(&discovery, "NAS"), at(&discovery, "Kitchen Speaker"));
        let nas_id = discovery.found()[nas].on_map().unwrap();
        assert_eq!(
            discovery.parent(nas, &network),
            Some(Parent::Device(nas_id))
        );
        assert_eq!(
            discovery.parent(speaker, &network),
            Some(Parent::Found(speaker))
        );
        network.remove_device(nas_id);
        assert_eq!(discovery.parent(nas, &network), None, "forgotten");
        let parents = discovery.parents(&network);
        assert!(parents.contains(&Parent::Found(speaker)));
        assert!(!parents.contains(&Parent::Found(nas)));
        assert_eq!(
            parents
                .iter()
                .filter(|p| matches!(p, Parent::Device(_)))
                .count(),
            network.devices().len()
        );
    }
}
