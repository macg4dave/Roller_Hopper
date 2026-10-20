//! Editing a device's network cards: adding a blank one, filling in its IPv4 settings, plugging
//! it into a card of another device, and removing it.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

use super::Network;
use crate::model::card::{CardId, NetworkCard};
use crate::model::device::DeviceId;

impl Network {
    /// Whether a card other than `except` already has `address`.
    pub(super) fn address_taken(&self, address: Ipv4Addr, except: Option<CardId>) -> bool {
        let mut cards = self.devices.iter().flat_map(|d| &d.cards);
        cards.any(|card| {
            Some(card.id) != except && card.address.is_some_and(|a| a.addr() == address)
        })
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

    /// Adds a blank network card to `device`, plugged in nowhere, and returns it.
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
    /// layout is reset. Refused (false) where it would form a loop, and for a guest's first
    /// card, which reaches the network through its host.
    pub(crate) fn plug_card(&mut self, card: CardId, upstream: Option<CardId>) -> bool {
        self.plug(card, upstream, true)
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
    use crate::model::network::Node;

    const SWITCH: DeviceId = DeviceId(2);
    const PC: DeviceId = DeviceId(5);
    const DOCKER_HOST: DeviceId = DeviceId(19);
    const LAB_ROUTER: DeviceId = DeviceId(22);

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    #[test]
    fn a_new_card_is_blank_and_hangs_under_its_device_as_a_ghost() {
        let mut network = mock_data::network();
        assert_eq!(network.add_card(DeviceId(999)), None);
        let card = network.add_card(PC).unwrap();
        assert_eq!(network.card(card), Some(&NetworkCard::blank(card)));
        assert!(network.has_ghost(card));
        let uplink = network.uplink(Node::Card(card));
        assert_eq!(uplink.map(|c| c.upstream), Some(Node::Device(PC)));
        assert_eq!(
            network.uplink(Node::Device(PC)).map(|c| c.upstream),
            Some(Node::Device(SWITCH)),
            "the PC stays where it was"
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
    fn a_card_plugged_into_another_device_hangs_there_and_is_kept_on_reset() {
        let mut network = mock_data::network();
        let card = network.add_card(PC).unwrap();
        let lab_router = network.socket(Node::Device(LAB_ROUTER));
        assert!(network.plug_card(card, lab_router));
        let uplink = network.uplink(Node::Card(card));
        assert_eq!(uplink.map(|c| c.upstream), Some(Node::Device(LAB_ROUTER)));
        network.restore_connections();
        assert_eq!(network.uplink(Node::Card(card)), uplink, "kept on reset");

        let own = network.socket(Node::Device(PC));
        assert!(!network.plug_card(card, own), "into its own device");
        let switch = network.socket(Node::Device(SWITCH));
        let pc_card = network.device(PC).unwrap().cards[0].id;
        let switch_card = network.device(SWITCH).unwrap().cards[0].id;
        assert!(!network.plug_card(switch_card, Some(pc_card)), "loop");
        assert!(network.plug_card(card, switch));
        let docker_home = network.device(DOCKER_HOST).unwrap().cards[0].id;
        assert!(
            !network.plug_card(docker_home, switch),
            "a guest's first card"
        );
    }

    #[test]
    fn a_card_is_removed_with_the_links_into_it_but_never_the_last() {
        let mut network = mock_data::network();
        let docker = network.device(DOCKER_HOST).unwrap().clone();
        let [home, lab] = [0, 1].map(|at| docker.cards[at].id);
        let pc = network.device(PC).unwrap().cards[0].id;
        assert!(network.plug_card(pc, Some(lab)));
        assert!(!network.remove_card(CardId(999)));
        assert!(!network.remove_card(pc), "last");
        assert!(network.remove_card(lab));
        assert!(network.uplink(Node::Device(PC)).is_none(), "unplugged");
        network.restore_connections();
        assert!(network.uplink(Node::Device(PC)).is_none(), "gone for good");
        assert!(!network.remove_card(home), "now the last");
    }
}
