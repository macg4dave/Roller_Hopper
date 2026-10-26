//! Checks a network read from a file before it is used: every ID once, every reference to
//! something that exists, no loops, and roles and groups as the editors keep them.

use std::collections::{HashMap, HashSet};

use super::{CardLink, Network, RelationshipKind};
use crate::model::MAX_MODEL_ID;
use crate::model::card::CardId;
use crate::model::device::{DeviceId, Role};

/// Why a network read from a file cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Invalid {
    /// Two devices, cards or segments share an ID, or a segment name is blank or repeated.
    DuplicateId,
    /// An ID cannot cross the signed model boundary used by the UI.
    UnsupportedId,
    /// A device has no network card.
    NoCard,
    /// A link's card, a device's host or role, a role's group or a segment does not exist.
    MissingReference,
    /// A card is plugged in twice, or cabled to its own device.
    BadLink,
    /// Links or hosts form a ring.
    Loop,
    /// Roles or groups listed twice, a blank or repeated name, or Other missing.
    BadRoles,
}

impl Network {
    /// The network as read from a file, once it holds together.
    pub(crate) fn checked(self) -> Result<Self, Invalid> {
        let owners = self.card_owners()?;
        check_links(&self.links, &owners)?;
        check_relations(&self.relations, &owners)?;
        check_segments(&self, &owners)?;
        self.check_bridge_groups()?;
        self.check_hosts()?;
        self.check_roles()?;
        Ok(self)
    }

    /// Every device runs on an existing device, and following hosts always ends.
    fn check_hosts(&self) -> Result<(), Invalid> {
        let hosts: HashMap<DeviceId, DeviceId> = self
            .devices
            .iter()
            .filter_map(|d| Some((d.id, d.host?)))
            .collect();
        if hosts.values().any(|host| self.device(*host).is_none()) {
            return Err(Invalid::MissingReference);
        }
        no_loops(hosts.keys().copied(), |device| hosts.get(device).copied())
    }

    /// The device each card is on, with every device and card ID used once.
    fn card_owners(&self) -> Result<HashMap<CardId, DeviceId>, Invalid> {
        let mut devices = HashSet::new();
        let mut owners = HashMap::new();
        for device in &self.devices {
            if device.id.0 > MAX_MODEL_ID {
                return Err(Invalid::UnsupportedId);
            }
            if !devices.insert(device.id) {
                return Err(Invalid::DuplicateId);
            }
            if device.cards.is_empty() {
                return Err(Invalid::NoCard);
            }
            for card in &device.cards {
                if card.id.0 > MAX_MODEL_ID {
                    return Err(Invalid::UnsupportedId);
                }
                if owners.insert(card.id, device.id).is_some() {
                    return Err(Invalid::DuplicateId);
                }
            }
        }
        Ok(owners)
    }

    fn check_roles(&self) -> Result<(), Invalid> {
        if !self.roles.consistent() || !self.groups.consistent() {
            return Err(Invalid::BadRoles);
        }
        let roles: Vec<&Role> = self.roles.roles().collect();
        let groups = roles.iter().filter_map(|r| self.roles.group(r));
        let mut used = self.devices.iter().map(|d| &d.role);
        if !used.all(|r| roles.contains(&r)) || !groups.into_iter().all(|g| self.groups.contains(g))
        {
            return Err(Invalid::MissingReference);
        }
        Ok(())
    }

    fn check_bridge_groups(&self) -> Result<(), Invalid> {
        let mut names = HashSet::new();
        if self
            .bridge_groups
            .iter()
            .any(|group| group.trim().is_empty() || !names.insert(group.to_lowercase()))
        {
            return Err(Invalid::DuplicateId);
        }
        if self
            .devices
            .iter()
            .flat_map(|device| &device.cards)
            .any(|card| {
                card.bridge_group
                    .as_ref()
                    .is_some_and(|group| !self.bridge_groups.contains(group))
            })
        {
            return Err(Invalid::MissingReference);
        }
        Ok(())
    }
}

/// Every join beside the tree is between two existing cards, is not a cable, and is listed once.
fn check_relations(
    relations: &[CardLink],
    owners: &HashMap<CardId, DeviceId>,
) -> Result<(), Invalid> {
    let mut unique = HashSet::new();
    for relation in relations {
        if relation.relationship == RelationshipKind::Physical
            || relation.card == relation.upstream
            || !owners.contains_key(&relation.card)
            || !owners.contains_key(&relation.upstream)
        {
            return Err(Invalid::BadLink);
        }
        let ends = if relation.card < relation.upstream {
            (relation.card, relation.upstream)
        } else {
            (relation.upstream, relation.card)
        };
        if !unique.insert((ends, relation.relationship)) {
            return Err(Invalid::BadLink);
        }
    }
    Ok(())
}

/// Every link joins two existing cards, cabled ones on different devices; each card is plugged
/// in once, and following the links upstream always ends.
fn check_links(links: &[CardLink], owners: &HashMap<CardId, DeviceId>) -> Result<(), Invalid> {
    let mut upstream = HashMap::new();
    for link in links {
        let (Some(card), Some(up)) = (owners.get(&link.card), owners.get(&link.upstream)) else {
            return Err(Invalid::MissingReference);
        };
        let same_device = card == up;
        if (same_device && link.relationship == RelationshipKind::Physical)
            || link.card == link.upstream
            || upstream.insert(link.card, link.upstream).is_some()
        {
            return Err(Invalid::BadLink);
        }
    }
    no_loops(upstream.keys().copied(), |card| upstream.get(card).copied())
}

/// Every segment has its own ID and name, and every card is in at most one existing segment.
fn check_segments(network: &Network, owners: &HashMap<CardId, DeviceId>) -> Result<(), Invalid> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for segment in &network.segments {
        if segment.id.0 > MAX_MODEL_ID
            || !ids.insert(segment.id)
            || segment.name.trim().is_empty()
            || !names.insert(segment.name.to_lowercase())
        {
            return Err(Invalid::DuplicateId);
        }
    }
    let mut cards = HashSet::new();
    for membership in &network.memberships {
        if !owners.contains_key(&membership.card)
            || !ids.contains(&membership.segment)
            || !cards.insert(membership.card)
        {
            return Err(Invalid::MissingReference);
        }
    }
    Ok(())
}

/// Following `next` from each start ends within as many steps as there are starts.
fn no_loops<T: Copy>(
    starts: impl Iterator<Item = T> + Clone,
    next: impl Fn(&T) -> Option<T>,
) -> Result<(), Invalid> {
    let steps = starts.clone().count() + 1;
    for start in starts {
        let chain = std::iter::successors(Some(start), &next);
        if chain.take(steps + 1).count() > steps {
            return Err(Invalid::Loop);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::ConnectionKind;

    #[test]
    fn the_sample_network_holds_together() {
        assert!(mock_data::network().checked().is_ok());
    }

    #[test]
    fn repeated_ids_missing_references_and_loops_are_refused() {
        let broken = |edit: fn(&mut Network)| {
            let mut network = mock_data::network();
            edit(&mut network);
            network.checked().err()
        };
        assert_eq!(
            broken(|n| n.devices[1].id = n.devices[0].id),
            Some(Invalid::DuplicateId)
        );
        assert_eq!(
            broken(|n| n.devices[1].cards[0].id = n.devices[0].cards[0].id),
            Some(Invalid::DuplicateId)
        );
        assert_eq!(
            broken(|n| n.devices[0].cards.clear()),
            Some(Invalid::NoCard)
        );
        assert_eq!(
            broken(|n| n.devices[0].role = Role::Custom("Gone".into())),
            Some(Invalid::MissingReference)
        );
        assert_eq!(
            broken(|n| {
                let (a, b) = (n.devices[0].cards[0].id, n.devices[1].cards[0].id);
                let link = |card, upstream| CardLink {
                    card,
                    upstream,
                    kind: ConnectionKind::Known,
                    relationship: RelationshipKind::Physical,
                };
                n.links = vec![link(a, b), link(b, a)];
            }),
            Some(Invalid::Loop)
        );
        assert_eq!(broken(|n| n.links.push(n.links[0])), Some(Invalid::BadLink));
        assert_eq!(
            broken(|n| n.devices[0].host = Some(DeviceId(u32::MAX))),
            Some(Invalid::MissingReference)
        );
        assert_eq!(
            broken(|n| {
                let (a, b) = (n.devices[0].id, n.devices[1].id);
                n.devices[0].host = Some(b);
                n.devices[1].host = Some(a);
            }),
            Some(Invalid::Loop)
        );
        assert_eq!(
            broken(|n| n.devices[0].id = DeviceId(u32::MAX)),
            Some(Invalid::UnsupportedId)
        );
    }
}
