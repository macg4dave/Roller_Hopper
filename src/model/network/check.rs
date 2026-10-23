//! Checks a network read from a file before it is used: every ID once, every reference to
//! something that exists, no loops, and roles and groups as the editors keep them.

use std::collections::{HashMap, HashSet};

use super::{CardLink, Network};
use crate::model::card::CardId;
use crate::model::device::{DeviceId, Role};

/// Why a network read from a file cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Invalid {
    /// Two devices, or two cards, share an ID.
    DuplicateId,
    /// A device has no network card.
    NoCard,
    /// A link's card, a device's role or a role's group does not exist.
    MissingReference,
    /// A card is plugged in twice, or into its own device.
    BadLink,
    /// Links form a ring.
    Loop,
    /// Roles or groups listed twice, a blank or repeated name, or Other missing.
    BadRoles,
}

impl Network {
    /// The network as read from a file, once it holds together.
    pub(crate) fn checked(self) -> Result<Self, Invalid> {
        let owners = self.card_owners()?;
        check_links(&self.links, &owners)?;
        self.check_roles()?;
        Ok(self)
    }

    /// The device each card is on, with every device and card ID used once.
    fn card_owners(&self) -> Result<HashMap<CardId, DeviceId>, Invalid> {
        let mut devices = HashSet::new();
        let mut owners = HashMap::new();
        for device in &self.devices {
            if !devices.insert(device.id) {
                return Err(Invalid::DuplicateId);
            }
            if device.cards.is_empty() {
                return Err(Invalid::NoCard);
            }
            for card in &device.cards {
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
}

/// Every link joins two existing cards on different devices, each card is plugged in once, and
/// following the links upstream always ends.
fn check_links(links: &[CardLink], owners: &HashMap<CardId, DeviceId>) -> Result<(), Invalid> {
    let mut upstream = HashMap::new();
    for link in links {
        let (Some(card), Some(up)) = (owners.get(&link.card), owners.get(&link.upstream)) else {
            return Err(Invalid::MissingReference);
        };
        if card == up || upstream.insert(link.card, link.upstream).is_some() {
            return Err(Invalid::BadLink);
        }
    }
    no_loops(upstream.keys().copied(), |card| upstream.get(card).copied())
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
                };
                n.links = vec![link(a, b), link(b, a)];
            }),
            Some(Invalid::Loop)
        );
        assert_eq!(broken(|n| n.links.push(n.links[0])), Some(Invalid::BadLink));
    }
}
