//! The map's tree. Its nodes are the network cards, each drawn with its device's icon and
//! name; devices themselves are not on the map. A card hangs from the card it is plugged into,
//! and a card plugged in nowhere heads a tree of its own.

use super::{CardLink, ConnectionKind, Network};
use crate::model::card::CardId;
use crate::model::device::Device;

/// A line of the map's tree, from a card towards the router (`upstream`) to a card plugged
/// into it (`downstream`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Connection {
    pub(crate) upstream: CardId,
    pub(crate) downstream: CardId,
    pub(crate) kind: ConnectionKind,
}

impl From<&CardLink> for Connection {
    fn from(link: &CardLink) -> Self {
        Self {
            upstream: link.upstream,
            downstream: link.card,
            kind: link.kind,
        }
    }
}

impl Network {
    /// The device a network card belongs to.
    pub(crate) fn card_owner(&self, card: CardId) -> Option<&Device> {
        let mut devices = self.devices.iter();
        devices.find(|d| d.cards.iter().any(|c| c.id == card))
    }

    /// The link of `card`, if it is plugged in.
    pub(crate) fn link(&self, card: CardId) -> Option<&CardLink> {
        self.links.iter().find(|l| l.card == card)
    }

    /// Every card on the map, in device order.
    pub(crate) fn nodes(&self) -> Vec<CardId> {
        let cards = self.devices.iter().flat_map(|d| &d.cards);
        cards.map(|c| c.id).collect()
    }

    /// The line leading from `card` to the card it is plugged into, if any.
    pub(crate) fn uplink(&self, card: CardId) -> Option<Connection> {
        let link = self.link(card)?;
        self.card_owner(link.upstream).map(|_| link.into())
    }

    /// The lines of the map's tree, in the order the links were found or made, so a new link
    /// comes last.
    pub(crate) fn connections(&self) -> Vec<Connection> {
        let exists = |card| self.card_owner(card).is_some();
        let links = self
            .links
            .iter()
            .filter(|l| exists(l.card) && exists(l.upstream));
        links.map(Connection::from).collect()
    }

    /// The cards plugged straight into `card`, in the order of [`Self::connections`].
    pub(crate) fn behind(&self, card: CardId) -> impl Iterator<Item = CardId> {
        let lines = self.connections().into_iter();
        lines
            .filter(move |c| c.upstream == card)
            .map(|c| c.downstream)
    }

    /// Whether `card` is `branch` itself or hangs somewhere behind it. A chain of links longer
    /// than the number of cards has a cycle, and is cut short there.
    pub(crate) fn is_within(&self, card: CardId, branch: CardId) -> bool {
        let chain = std::iter::successors(Some(card), |&at| Some(self.uplink(at)?.upstream));
        chain.take(self.nodes().len() + 1).any(|at| at == branch)
    }

    /// Whether `card` may be plugged into `target`: not into another card of its own device,
    /// and not into one behind it, which would form a loop.
    pub(crate) fn can_plug(&self, card: CardId, target: CardId) -> bool {
        let owner = |c| self.card_owner(c).map(|d| d.id);
        owner(card).is_some()
            && owner(target).is_some()
            && owner(card) != owner(target)
            && !self.is_within(target, card)
    }

    /// Plugs `card` into `upstream`, as a link the user made by dragging it there, or unplugs
    /// it on `None`, so it heads a tree of its own; the cards behind it go with it. Refused
    /// where [`Self::can_plug`] does not allow it. A card that stays plugged into the same card
    /// keeps the link it had.
    pub(crate) fn connect(&mut self, card: CardId, upstream: Option<CardId>) {
        if self.link(card).map(|l| l.upstream) != upstream {
            self.plug_card(card, upstream);
        }
    }

    /// Plugs `card` into `target`, or unplugs it on `None`, as a known link. Refused (false)
    /// where [`Self::can_plug`] does not allow it.
    pub(crate) fn plug_card(&mut self, card: CardId, target: Option<CardId>) -> bool {
        let refused = match target {
            Some(target) => !self.can_plug(card, target),
            None => self.card_owner(card).is_none(),
        };
        if refused {
            return false;
        }
        self.links.retain(|l| l.card != card);
        if let Some(upstream) = target {
            self.links.push(CardLink {
                upstream,
                card,
                kind: ConnectionKind::Known,
            });
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    // Each sample device's first card has the device's id.
    const ROUTER_LAN: CardId = CardId(1);
    const ROUTER_WAN: CardId = CardId(101);
    const SWITCH: CardId = CardId(2);
    const ACCESS_POINT: CardId = CardId(3);
    const NAS: CardId = CardId(4);
    const PC: CardId = CardId(5);
    const LINUX_SERVER: CardId = CardId(6);
    const DOCKER_HOST: CardId = CardId(19);
    const DOCKER_LAB_CARD: CardId = CardId(100);
    const LAB_ROUTER: CardId = CardId(22);

    #[test]
    fn every_card_is_on_the_map_hanging_from_where_it_is_plugged_in() {
        let network = mock_data::network();
        let cards: usize = network.devices().iter().map(|d| d.cards.len()).sum();
        assert_eq!(network.nodes().len(), cards);
        let behind = |card| network.behind(card).collect::<Vec<_>>();
        assert_eq!(behind(ROUTER_LAN), [NAS, SWITCH, ACCESS_POINT]);
        assert_eq!(behind(ROUTER_WAN), [] as [CardId; 0]);
        assert!(network.uplink(ROUTER_WAN).is_none(), "heads its own tree");
    }

    #[test]
    fn an_ordinary_virtual_devices_cards_use_normal_links() {
        let network = mock_data::network();
        let up = |card| network.uplink(card).map(|c| c.upstream);
        assert_eq!(up(DOCKER_HOST), Some(LINUX_SERVER));
        assert_eq!(up(DOCKER_LAB_CARD), Some(LAB_ROUTER));
    }

    #[test]
    fn a_card_moves_to_another_line_with_its_branch() {
        let mut network = mock_data::network();
        network.connect(PC, Some(ROUTER_LAN));
        let link = network.uplink(PC);
        assert_eq!(
            link.map(|l| (l.upstream, l.kind)),
            Some((ROUTER_LAN, ConnectionKind::Known))
        );
        network.connect(SWITCH, Some(ACCESS_POINT));
        assert!(!network.is_within(PC, SWITCH), "the PC left the switch");
        assert!(
            network.is_within(LINUX_SERVER, ACCESS_POINT),
            "the switch kept its own"
        );
        assert!(
            network.is_within(DOCKER_HOST, ACCESS_POINT),
            "and downstream virtual devices"
        );
    }

    #[test]
    fn a_new_link_comes_last_and_an_unchanged_one_stays_as_found() {
        let mut network = mock_data::network();
        network.connect(PC, Some(ROUTER_LAN));
        let behind: Vec<_> = network.behind(ROUTER_LAN).collect();
        assert_eq!(behind, [NAS, SWITCH, ACCESS_POINT, PC]);

        let phone = CardId(10);
        let found = network.uplink(phone);
        network.connect(phone, Some(ACCESS_POINT));
        assert_eq!(network.uplink(phone), found);
        assert_eq!(found.map(|l| l.kind), Some(ConnectionKind::Inferred));
    }

    #[test]
    fn nothing_plugs_into_its_own_branch_or_its_own_device() {
        let mut network = mock_data::network();
        let before = network.connections();
        network.connect(SWITCH, Some(PC));
        network.connect(SWITCH, Some(SWITCH));
        network.connect(SWITCH, Some(CardId(999)));
        network.connect(ROUTER_WAN, Some(ROUTER_LAN));
        network.connect(DOCKER_LAB_CARD, Some(DOCKER_HOST));
        assert_eq!(network.connections(), before);
    }

    #[test]
    fn a_card_unplugged_heads_its_own_tree_with_its_branch() {
        let mut network = mock_data::network();
        network.connect(SWITCH, None);
        assert!(network.uplink(SWITCH).is_none());
        assert!(network.is_within(PC, SWITCH));
    }

    #[test]
    fn the_trees_are_headed_by_the_routers_cards() {
        let network = mock_data::network();
        let tops: Vec<_> = network
            .nodes()
            .into_iter()
            .filter(|&c| network.uplink(c).is_none())
            .collect();
        assert_eq!(tops, [ROUTER_LAN, ROUTER_WAN, LAB_ROUTER]);
    }
}
