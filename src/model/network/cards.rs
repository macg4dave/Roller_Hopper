//! Editing a device's network cards: adding a blank one, filling in its IPv4 settings, making
//! it a child of another device, and removing it. Plugging cards in is in [`super::tree`].

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

use super::{ConnectionKind, Network};
use crate::model::card::MacAddr;
use crate::model::card::{CardId, NetworkCard};
use crate::model::device::DeviceId;
use crate::model::fresh_ids;

impl Network {
    /// Whether a card other than `except` already has `address`.
    pub(crate) fn address_taken(&self, address: Ipv4Addr, except: Option<CardId>) -> bool {
        let mut cards = self.devices.iter().flat_map(|d| &d.cards);
        cards.any(|card| {
            Some(card.id) != except && card.address.is_some_and(|a| a.addr() == address)
        })
    }

    pub(super) fn next_card_id(&self) -> Option<CardId> {
        let cards = self.devices.iter().flat_map(|d| &d.cards);
        fresh_ids(cards.map(|card| card.id.0), 1)
            .and_then(|mut ids| ids.pop())
            .map(CardId)
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
        let id = self.next_card_id()?;
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

    /// Applies a newly observed lease to the existing DHCP card with `mac`. Manual addresses
    /// are never changed, and an address already used by another card is refused.
    pub(crate) fn refresh_dhcp_address(&mut self, mac: MacAddr, address: Ipv4Net) -> bool {
        let card = self
            .devices
            .iter()
            .flat_map(|device| &device.cards)
            .find(|card| card.mac == Some(mac) && card.dhcp)
            .map(|card| card.id);
        let Some(card) = card else {
            return false;
        };
        self.set_card_address(card, Some(address))
    }

    /// The device `card` is the only card of, which moving the card to another device removes.
    pub(crate) fn only_card_of(&self, card: CardId) -> Option<DeviceId> {
        let owner = self.card_owner(card)?;
        (owner.cards.len() == 1).then_some(owner.id)
    }

    /// Whether `card` may become a child of a device other than its own.
    pub(crate) fn can_move_card(&self, card: CardId, to: DeviceId) -> bool {
        let Some(owner) = self.card_owner(card) else {
            return false;
        };
        self.device(to).is_some() && owner.id != to
    }

    /// Makes `card` the last card of `to`, keeping its id, settings and links, apart from those
    /// to `to`'s other cards, as a device's cards are never plugged into each other. A device
    /// left with no card is removed. Refused (false) where
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
        self.links.retain(|l| !between(l));
        if let Some(gone) = emptied {
            self.devices.retain(|d| d.id != gone);
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
        card.id = self.next_card_id()?;
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
        self.links.retain(|l| l.card != card && l.upstream != card);
        self.relations
            .retain(|relation| relation.card != card && relation.upstream != card);
        self.memberships
            .retain(|membership| membership.card != card);
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
    fn a_card_plugged_in_hangs_there() {
        let mut network = mock_data::network();
        let card = network.add_card(PC).unwrap();
        assert!(network.plug_card(card, Some(LAB_ROUTER_CARD)));
        let uplink = network.uplink(card);
        assert_eq!(uplink.map(|c| c.upstream), Some(LAB_ROUTER_CARD));

        assert!(!network.plug_card(card, Some(PC_CARD)), "its own device");
        assert!(!network.plug_card(SWITCH_CARD, Some(PC_CARD)), "loop");
        assert!(
            network.plug_card(DOCKER_HOME, Some(SWITCH_CARD)),
            "a virtual device's too"
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
    fn only_a_dhcp_card_takes_a_new_address_for_its_mac() {
        let mut network = mock_data::network();
        let card = PC_CARD;
        let mac = network.card(card).and_then(|card| card.mac).unwrap();
        let original = network.card(card).and_then(|card| card.address);
        let lease = net("192.168.1.240/24");
        assert!(!network.refresh_dhcp_address(mac, lease));
        assert_eq!(network.card(card).and_then(|card| card.address), original);
        network.card_mut(card).unwrap().dhcp = true;
        assert!(network.refresh_dhcp_address(mac, lease));
        assert_eq!(
            network.card(card).and_then(|card| card.address),
            Some(lease)
        );
    }

    #[test]
    fn a_card_is_removed_with_the_links_into_it_but_never_the_last() {
        let mut network = mock_data::network();
        assert!(network.plug_card(PC_CARD, Some(DOCKER_LAB)));
        assert!(!network.remove_card(CardId(999)));
        assert!(!network.remove_card(PC_CARD), "last");
        assert!(network.remove_card(DOCKER_LAB));
        assert!(network.uplink(PC_CARD).is_none(), "unplugged");
        assert!(!network.remove_card(DOCKER_HOME), "now the last");
    }
}
