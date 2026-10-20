//! Editing a device's network cards: adding a blank one, filling in its IPv4 settings, plugging
//! it into a card of another device, making it a child of another device, and removing it.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

use super::{ConnectionKind, Network, children};
use crate::model::card::{CardId, NetworkCard};
use crate::model::device::DeviceId;

impl Network {
    /// Whether a card other than `except` already has `address`.
    pub(crate) fn address_taken(&self, address: Ipv4Addr, except: Option<CardId>) -> bool {
        let mut cards = self.devices.iter().flat_map(|d| &d.cards);
        cards.any(|card| {
            Some(card.id) != except && card.address.is_some_and(|a| a.addr() == address)
        })
    }

    /// The subnets the cards are on, each once, in device order.
    pub(crate) fn subnets(&self) -> Vec<Ipv4Net> {
        let mut subnets = Vec::new();
        let cards = self.devices.iter().flat_map(|d| &d.cards);
        for subnet in cards.filter_map(|card| card.address.map(|a| a.trunc())) {
            if !subnets.contains(&subnet) {
                subnets.push(subnet);
            }
        }
        subnets
    }

    pub(super) fn next_card_id(&self) -> CardId {
        let cards = self.devices.iter().flat_map(|d| &d.cards);
        CardId(cards.map(|card| card.id.0).max().unwrap_or(0) + 1)
    }

    pub(crate) fn card(&self, card: CardId) -> Option<&NetworkCard> {
        let mut cards = self.devices.iter().flat_map(|d| &d.cards);
        cards.find(|c| c.id == card)
    }

    /// For editing a card's name, gateway and MAC address. Its address is set with
    /// [`Self::set_card_address`], which keeps addresses unique.
    pub(crate) fn card_mut(&mut self, card: CardId) -> Option<&mut NetworkCard> {
        let mut cards = self.devices.iter_mut().flat_map(|d| &mut d.cards);
        cards.find(|c| c.id == card)
    }

    /// Adds a blank network card to `device`, plugged in nowhere (so it heads a tree of its own
    /// on the map), and returns it.
    pub(crate) fn add_card(&mut self, device: DeviceId) -> Option<CardId> {
        let id = self.next_card_id();
        self.device_mut(device)?.cards.push(NetworkCard::blank(id));
        Some(id)
    }

    /// Gives `card` its address and subnet, or clears them on `None`. Refused (false) when
    /// another card has that address.
    pub(crate) fn set_card_address(&mut self, card: CardId, address: Option<Ipv4Net>) -> bool {
        if address.is_some_and(|a| self.address_taken(a.addr(), Some(card))) {
            return false;
        }
        let Some(edited) = self.card_mut(card) else {
            return false;
        };
        edited.address = address;
        true
    }

    /// Plugs `card` into the card `upstream`, or into none, as a link that is kept when the
    /// layout is reset. Refused (false) into a card of its own device or one behind it.
    pub(crate) fn plug_card(&mut self, card: CardId, upstream: Option<CardId>) -> bool {
        self.plug(card, upstream, true)
    }

    /// The device `card` is the only card of, which moving the card to another device removes.
    pub(crate) fn only_card_of(&self, card: CardId) -> Option<DeviceId> {
        let owner = self.card_owner(card)?;
        (owner.cards.len() == 1).then_some(owner.id)
    }

    /// Whether `card` may become a child of `to`: a device other than its own and, when it is
    /// its device's only card, not one running on that device, which then runs on `to`.
    pub(crate) fn can_move_card(&self, card: CardId, to: DeviceId) -> bool {
        let Some(owner) = self.card_owner(card) else {
            return false;
        };
        self.device(to).is_some()
            && owner.id != to
            && !(owner.cards.len() == 1 && self.runs_within(to, owner.id))
    }

    /// Makes `card` the last card of `to`, keeping its id, settings and links, apart from those
    /// to `to`'s other cards, as a device's cards are never plugged into each other. A device
    /// left with no card is removed, and what ran on it runs on `to`. Refused (false) where
    /// [`Self::can_move_card`] does not allow it.
    pub(crate) fn move_card(&mut self, card: CardId, to: DeviceId) -> bool {
        if !self.can_move_card(card, to) {
            return false;
        }
        let emptied = self.only_card_of(card);
        let owner = self.devices.iter_mut().find_map(|d| {
            let at = d.cards.iter().position(|c| c.id == card)?;
            Some(d.cards.remove(at))
        });
        let (Some(moved), Some(target)) = (owner, self.devices.iter_mut().find(|d| d.id == to))
        else {
            return false;
        };
        let siblings: Vec<CardId> = target.cards.iter().map(|c| c.id).collect();
        target.cards.push(moved);
        let between = |l: &super::CardLink| {
            (l.card == card && siblings.contains(&l.upstream))
                || (l.upstream == card && siblings.contains(&l.card))
        };
        for links in [&mut self.links, &mut self.detected] {
            links.retain(|l| !between(l));
        }
        if let Some(gone) = emptied {
            let guests = self.devices.iter_mut().filter_map(|d| d.guest.as_mut());
            guests.filter(|g| g.host == gone).for_each(|g| g.host = to);
            self.devices.retain(|d| d.id != gone);
            self.devices = children::hosts_first(std::mem::take(&mut self.devices));
        }
        true
    }

    /// Adds `card`, as a scan found it, to `to` as its last card under a fresh id, plugged into
    /// `uplink`'s card as in [`Self::insert_device`], and returns it. Refused (`None`) when `to`
    /// is missing or another card has its address.
    pub(crate) fn adopt_card(
        &mut self,
        to: DeviceId,
        mut card: NetworkCard,
        uplink: Option<(CardId, ConnectionKind)>,
    ) -> Option<CardId> {
        if card
            .address
            .is_some_and(|a| self.address_taken(a.addr(), None))
        {
            return None;
        }
        card.id = self.next_card_id();
        let id = card.id;
        self.device_mut(to)?.cards.push(card);
        self.plug_found(id, uplink);
        Some(id)
    }

    /// Removes a network card, its link, and the links of the cards plugged into it, which are
    /// left standing alone. Refused for a device's last card.
    pub(crate) fn remove_card(&mut self, card: CardId) -> bool {
        let owner = self
            .devices
            .iter_mut()
            .find(|d| d.cards.iter().any(|c| c.id == card));
        let Some(owner) = owner.filter(|d| d.cards.len() > 1) else {
            return false;
        };
        owner.cards.retain(|c| c.id != card);
        for links in [&mut self.links, &mut self.detected] {
            links.retain(|l| l.card != card && l.upstream != card);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    const PC: DeviceId = DeviceId(5);
    // Each sample device's first card has the device's id.
    const PC_CARD: CardId = CardId(5);
    const SWITCH_CARD: CardId = CardId(2);
    const LAB_ROUTER_CARD: CardId = CardId(22);
    const DOCKER_HOME: CardId = CardId(19);
    const DOCKER_LAB: CardId = CardId(100);

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    #[test]
    fn a_new_card_is_blank_and_plugged_in_nowhere() {
        let mut network = mock_data::network();
        assert_eq!(network.add_card(DeviceId(999)), None);
        let card = network.add_card(PC).unwrap();
        assert_eq!(network.card(card), Some(&NetworkCard::blank(card)));
        assert!(network.nodes().contains(&card));
        assert!(network.uplink(card).is_none());
        assert_eq!(
            network.uplink(PC_CARD).map(|c| c.upstream),
            Some(SWITCH_CARD),
            "the PC's first card stays where it was"
        );
    }

    #[test]
    fn an_address_is_given_only_where_no_other_card_has_it() {
        let mut network = mock_data::network();
        let card = network.add_card(PC).unwrap();
        let router_ip = network.device(DeviceId(1)).unwrap().cards[0].address;
        assert!(!network.set_card_address(card, router_ip), "taken");
        assert!(network.set_card_address(card, Some(net("10.0.0.7/16"))));
        assert!(
            network.set_card_address(card, Some(net("10.0.0.7/24"))),
            "its own"
        );
        assert_eq!(
            network.card(card).and_then(|c| c.address),
            Some(net("10.0.0.7/24"))
        );
        assert!(network.set_card_address(card, None));
        assert!(!network.set_card_address(CardId(999), None));
    }

    #[test]
    fn a_card_plugged_in_hangs_there_and_is_kept_on_reset() {
        let mut network = mock_data::network();
        let card = network.add_card(PC).unwrap();
        assert!(network.plug_card(card, Some(LAB_ROUTER_CARD)));
        let uplink = network.uplink(card);
        assert_eq!(uplink.map(|c| c.upstream), Some(LAB_ROUTER_CARD));
        network.restore_connections();
        assert_eq!(network.uplink(card), uplink, "kept on reset");

        assert!(!network.plug_card(card, Some(PC_CARD)), "its own device");
        assert!(!network.plug_card(SWITCH_CARD, Some(PC_CARD)), "loop");
        assert!(
            network.plug_card(DOCKER_HOME, Some(SWITCH_CARD)),
            "a guest's too"
        );
        assert!(network.plug_card(card, None));
        assert!(network.uplink(card).is_none());
    }

    #[test]
    fn a_devices_only_card_moved_elsewhere_keeps_its_settings_and_removes_the_device() {
        let mut network = mock_data::network();
        let router = DeviceId(1);
        let before = network.card(PC_CARD).cloned();
        assert_eq!(network.only_card_of(PC_CARD), Some(PC));
        assert!(network.move_card(PC_CARD, router));
        assert!(network.device(PC).is_none(), "removed");
        let cards = &network.device(router).unwrap().cards;
        assert_eq!(cards.last(), before.as_ref(), "its next card, as it was");
        assert_eq!(
            network.uplink(PC_CARD).map(|c| c.upstream),
            Some(SWITCH_CARD),
            "still plugged in"
        );
    }

    #[test]
    fn a_moved_card_is_unplugged_from_its_new_siblings() {
        let mut network = mock_data::network();
        let docker = DeviceId(19);
        assert_eq!(network.only_card_of(DOCKER_LAB), None);
        assert!(network.move_card(DOCKER_LAB, DeviceId(22)));
        assert_eq!(network.device(docker).unwrap().cards.len(), 1, "kept");
        assert!(network.uplink(DOCKER_LAB).is_none(), "was the lab router's");
        network.restore_connections();
        assert!(network.uplink(DOCKER_LAB).is_none(), "for good");
    }

    #[test]
    fn what_ran_on_a_removed_device_runs_where_its_card_went() {
        let mut network = mock_data::network();
        let (server, docker, router) = (DeviceId(6), DeviceId(19), DeviceId(1));
        let server_card = CardId(6);
        assert!(!network.move_card(server_card, docker), "runs on it");
        assert!(!network.move_card(server_card, DeviceId(6)), "its own");
        assert!(!network.move_card(server_card, DeviceId(999)));
        assert!(!network.move_card(CardId(999), router));
        assert!(network.move_card(server_card, router));
        assert!(network.device(server).is_none());
        assert_eq!(network.host(docker), Some(router));
        let devices = network.devices();
        for (at, device) in devices.iter().enumerate().skip(1) {
            if let Some(host) = network.host(device.id) {
                let before = &devices[at - 1];
                assert!(network.runs_within(before.id, host), "{}", device.name);
            }
        }
    }

    #[test]
    fn a_found_card_is_adopted_under_a_fresh_id_unless_its_address_is_taken() {
        let mut network = mock_data::network();
        let taken = network.card(PC_CARD).cloned().unwrap();
        assert_eq!(network.adopt_card(DeviceId(1), taken, None), None);
        let card = NetworkCard {
            address: Some(net("192.168.1.240/24")),
            ..NetworkCard::blank(CardId(0))
        };
        let uplink = Some((SWITCH_CARD, ConnectionKind::Inferred));
        assert_eq!(
            network.adopt_card(DeviceId(999), card.clone(), uplink),
            None
        );
        let id = network.adopt_card(PC, card, uplink).unwrap();
        assert_eq!(network.card_owner(id).map(|d| d.id), Some(PC));
        let link = network.uplink(id).map(|c| (c.upstream, c.kind));
        assert_eq!(link, Some((SWITCH_CARD, ConnectionKind::Inferred)));
    }

    #[test]
    fn a_card_is_removed_with_the_links_into_it_but_never_the_last() {
        let mut network = mock_data::network();
        assert!(network.plug_card(PC_CARD, Some(DOCKER_LAB)));
        assert!(!network.remove_card(CardId(999)));
        assert!(!network.remove_card(PC_CARD), "last");
        assert!(network.remove_card(DOCKER_LAB));
        assert!(network.uplink(PC_CARD).is_none(), "unplugged");
        network.restore_connections();
        assert!(network.uplink(PC_CARD).is_none(), "gone for good");
        assert!(!network.remove_card(DOCKER_HOME), "now the last");
    }
}
