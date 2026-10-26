//! Layer 2 segments (a VLAN or another shared Ethernet network) and the cards that belong to
//! them. Membership is separate from the cabling tree: it is drawn as a coloured region.

use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};

use super::Network;
use crate::model::card::CardId;
use crate::model::fresh_ids;
use crate::model::groups::GroupColour;

/// How a card carries a segment's traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub(crate) enum MembershipMode {
    #[default]
    Untagged,
    Tagged,
    /// Untagged, and the port's default VLAN (PVID).
    Native,
}

impl MembershipMode {
    /// Every mode, in the order the editor lists them.
    pub(crate) const ALL: [Self; 3] = [Self::Untagged, Self::Tagged, Self::Native];
}

/// Stable identifier of a segment, unique across the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SegmentId(pub(crate) u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Segment {
    pub(crate) id: SegmentId,
    /// The user's name for it, unique ignoring case.
    pub(crate) name: String,
    #[serde(default = "default_segment_colour")]
    pub(crate) colour: GroupColour,
    #[serde(default)]
    pub(crate) vlan: Option<u16>,
    #[serde(default)]
    pub(crate) subnet: Option<Ipv4Net>,
}

fn default_segment_colour() -> GroupColour {
    GroupColour::Blue
}

/// A card's place in one segment; a card is in at most one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct SegmentMembership {
    pub(crate) card: CardId,
    pub(crate) segment: SegmentId,
    #[serde(default)]
    pub(crate) mode: MembershipMode,
}

impl Network {
    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub(crate) fn memberships(&self) -> &[SegmentMembership] {
        &self.memberships
    }

    pub(crate) fn segment_of(&self, card: CardId) -> Option<(SegmentId, MembershipMode)> {
        self.memberships
            .iter()
            .find(|membership| membership.card == card)
            .map(|membership| (membership.segment, membership.mode))
    }

    fn segment_name_taken(&self, name: &str, except: Option<SegmentId>) -> bool {
        self.segments
            .iter()
            .any(|segment| Some(segment.id) != except && segment.name.eq_ignore_ascii_case(name))
    }

    /// Adds a segment named `name`; `None` when the name is blank or taken.
    pub(crate) fn add_segment(&mut self, name: &str) -> Option<SegmentId> {
        let name = name.trim();
        if name.is_empty() || self.segment_name_taken(name, None) {
            return None;
        }
        let id = SegmentId(fresh_ids(self.segments.iter().map(|s| s.id.0), 1)?.pop()?);
        self.segments.push(Segment {
            id,
            name: name.to_owned(),
            colour: default_segment_colour(),
            vlan: None,
            subnet: None,
        });
        Some(id)
    }

    /// Renames a segment; refused (false) when the name is blank or taken.
    pub(crate) fn rename_segment(&mut self, id: SegmentId, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || self.segment_name_taken(name, Some(id)) {
            return false;
        }
        let Some(segment) = self.segment_mut(id) else {
            return false;
        };
        name.clone_into(&mut segment.name);
        true
    }

    /// A segment to change its colour, VLAN or subnet; its name goes through
    /// [`Self::rename_segment`], which keeps names unique.
    pub(crate) fn segment_mut(&mut self, id: SegmentId) -> Option<&mut Segment> {
        self.segments.iter_mut().find(|segment| segment.id == id)
    }

    /// Removes a segment; its cards are left in none.
    pub(crate) fn remove_segment(&mut self, id: SegmentId) -> bool {
        let before = self.segments.len();
        self.segments.retain(|segment| segment.id != id);
        self.memberships
            .retain(|membership| membership.segment != id);
        self.segments.len() != before
    }

    /// Puts `card` in `segment`, or in none, replacing any membership it had.
    pub(crate) fn set_membership(
        &mut self,
        card: CardId,
        segment: Option<SegmentId>,
        mode: MembershipMode,
    ) -> bool {
        if self.card(card).is_none()
            || segment.is_some_and(|id| self.segments.iter().all(|item| item.id != id))
        {
            return false;
        }
        self.memberships
            .retain(|membership| membership.card != card);
        if let Some(segment) = segment {
            self.memberships.push(SegmentMembership {
                card,
                segment,
                mode,
            });
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    #[test]
    fn layer_two_segments_and_membership_are_explicit() {
        let mut network = mock_data::network();
        let segment = network.add_segment("VLAN 20").unwrap();
        assert_eq!(network.add_segment(" vlan 20 "), None, "name taken");
        let card = network.nodes()[0];
        assert!(network.set_membership(card, Some(segment), MembershipMode::Native));
        assert_eq!(
            network.segment_of(card),
            Some((segment, MembershipMode::Native))
        );
        assert!(!network.set_membership(CardId(u32::MAX), Some(segment), MembershipMode::Tagged));
        assert!(network.remove_segment(segment));
        assert_eq!(network.segment_of(card), None);
    }

    #[test]
    fn a_segment_is_renamed_only_to_a_free_name() {
        let mut network = mock_data::network();
        let lan = network.add_segment("LAN").unwrap();
        network.add_segment("SMART").unwrap();
        assert!(!network.rename_segment(lan, "smart"));
        assert!(!network.rename_segment(lan, " "));
        assert!(
            network.rename_segment(lan, "lan"),
            "its own name in another case"
        );
        assert_eq!(network.segments()[0].name, "lan");
    }
}
