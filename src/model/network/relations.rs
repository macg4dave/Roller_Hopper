//! How two cards are joined, and the joins drawn on the map beside the tree. A card hangs from
//! one card in the tree, but it can also belong to a bridge or be routed to another card; those
//! extra joins do not move it.

use serde::{Deserialize, Serialize};

use super::{CardLink, ConnectionKind, Network};
use crate::model::card::{CardId, InterfaceKind};

/// What joins two cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub(crate) enum RelationshipKind {
    /// A cable between cards on different devices.
    #[default]
    Physical,
    /// A virtual cable, such as a VM's adapter and its host-side tap.
    VirtualPeer,
    /// A port of a software bridge or a switch VLAN.
    BridgeMember,
    /// Traffic passed on by routing, NAT or a firewall rather than switched.
    Routed,
}

impl RelationshipKind {
    /// Every relationship, in the order the editor lists them.
    pub(crate) const ALL: [Self; 4] = [
        Self::Physical,
        Self::VirtualPeer,
        Self::BridgeMember,
        Self::Routed,
    ];
}

impl Network {
    /// Whether two cards may be joined by `relationship`: both exist and differ, and a cable
    /// joins cards on different devices.
    pub(crate) fn can_join(
        &self,
        card: CardId,
        target: CardId,
        relationship: RelationshipKind,
    ) -> bool {
        self.card_owner(card).is_some()
            && self.card_owner(target).is_some()
            && card != target
            && (!self.same_device(card, target) || relationship != RelationshipKind::Physical)
    }

    /// Whether two cards are on the same device; false when either is missing.
    pub(super) fn same_device(&self, card: CardId, other: CardId) -> bool {
        let owner = |c| self.card_owner(c).map(|d| d.id);
        owner(card).is_some_and(|device| owner(other) == Some(device))
    }

    /// How `card` joins `target` when plugged in without saying: a bridge's members join it,
    /// virtual adapters and cards of one device are joined virtually, and others by a cable.
    pub(super) fn natural_relationship(&self, card: CardId, target: CardId) -> RelationshipKind {
        let kinds = [card, target].map(|c| self.card(c).map(|c| c.interface_kind));
        if kinds.contains(&Some(InterfaceKind::Bridge)) {
            RelationshipKind::BridgeMember
        } else if self.same_device(card, target) || kinds.contains(&Some(InterfaceKind::Virtual)) {
            RelationshipKind::VirtualPeer
        } else {
            RelationshipKind::Physical
        }
    }

    /// Joins `card` to `target` beside the tree. Refused (false) for a cable, which belongs in
    /// the tree, where [`Self::can_join`] does not allow it, or when they are joined so already.
    pub(crate) fn add_relation(
        &mut self,
        card: CardId,
        target: CardId,
        relationship: RelationshipKind,
    ) -> bool {
        if relationship == RelationshipKind::Physical || !self.can_join(card, target, relationship)
        {
            return false;
        }
        let duplicate = self.relations.iter().any(|relation| {
            relation.relationship == relationship
                && ((relation.card == card && relation.upstream == target)
                    || (relation.card == target && relation.upstream == card))
        });
        if duplicate {
            return false;
        }
        self.relations.push(CardLink {
            upstream: target,
            card,
            kind: ConnectionKind::Known,
            relationship,
        });
        true
    }

    /// The joins beside the tree that `card` is at either end of, in the order they were made.
    pub(crate) fn relations_for(&self, card: CardId) -> impl Iterator<Item = &CardLink> {
        self.relations
            .iter()
            .filter(move |relation| relation.card == card || relation.upstream == card)
    }

    /// Removes the join at `index` in [`Self::relations_for`] `card`.
    pub(crate) fn remove_relation(&mut self, card: CardId, index: usize) -> bool {
        let Some(remove) = self
            .relations
            .iter()
            .enumerate()
            .filter(|(_, relation)| relation.card == card || relation.upstream == card)
            .nth(index)
            .map(|(index, _)| index)
        else {
            return false;
        };
        self.relations.remove(remove);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    #[test]
    fn bridge_membership_can_join_interfaces_on_one_device_without_changing_the_tree() {
        let mut network = mock_data::network();
        let device = network.devices()[0].id;
        let bridge = network.add_card(device).unwrap();
        network.card_mut(bridge).unwrap().interface_kind = InterfaceKind::Bridge;
        let member = network.device(device).unwrap().cards[0].id;
        let before = network.connections();
        assert!(network.add_relation(member, bridge, RelationshipKind::BridgeMember));
        assert_eq!(network.connections(), before, "layout tree stays stable");
        assert!(
            network
                .map_connections()
                .iter()
                .any(|relation| relation.relationship == RelationshipKind::BridgeMember)
        );
        assert!(!network.add_relation(bridge, member, RelationshipKind::BridgeMember));
        assert!(!network.add_relation(member, bridge, RelationshipKind::Physical));
        assert_eq!(network.relations_for(bridge).count(), 1);
        assert!(network.remove_relation(bridge, 0));
        assert!(!network.remove_relation(bridge, 0));
    }

    #[test]
    fn a_relation_may_join_a_card_behind_it_in_the_tree() {
        let mut network = mock_data::network();
        let (router, switch) = (CardId(1), CardId(2));
        assert!(network.is_within(switch, router));
        assert!(network.add_relation(router, switch, RelationshipKind::Routed));
    }

    #[test]
    fn plugging_in_picks_the_relationship_from_the_cards() {
        let mut network = mock_data::network();
        let device = network.devices()[0].id;
        let first = network.device(device).unwrap().cards[0].id;
        let other_device = network.devices()[1].cards[0].id;
        assert_eq!(
            network.natural_relationship(first, other_device),
            RelationshipKind::Physical
        );
        let second = network.add_card(device).unwrap();
        assert_eq!(
            network.natural_relationship(second, first),
            RelationshipKind::VirtualPeer
        );
        network.card_mut(first).unwrap().interface_kind = InterfaceKind::Bridge;
        assert!(!network.plug_card(second, Some(first)), "not by a drag");
        assert!(network.attach_card(second, Some(first)));
        let link = network.link(second).map(|l| l.relationship);
        assert_eq!(link, Some(RelationshipKind::BridgeMember));
    }
}
