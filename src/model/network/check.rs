//! Checks a network read from a file before it is used: every ID once, every reference to
//! something that exists, no loops, and roles and groups as the editors keep them.

use std::collections::{HashMap, HashSet};

use super::{CardLink, Network, children};
use crate::model::card::CardId;
use crate::model::device::{DeviceId, Role};

/// Why a network read from a file cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Invalid {
    /// Two devices, or two cards, share an ID.
    DuplicateId,
    /// A device has no network card.
    NoCard,
    /// A guest's host, a link's card, a device's role or a role's group does not exist.
    MissingReference,
    /// A card is plugged in twice, or into its own device.
    BadLink,
    /// Guests run on each other in a ring, or links do.
    Loop,
    /// Roles or groups listed twice, a blank or repeated name, or Other missing.
    BadRoles,
}

impl Network {
    /// The network as read from a file, once it holds together; its devices are put in list
    /// order (guests after their host).
    pub(crate) fn checked(mut self) -> Result<Self, Invalid> {
        let owners = self.card_owners()?;
        self.check_guests()?;
        for links in [&self.links, &self.detected] {
            check_links(links, &owners)?;
        }
        self.check_roles()?;
        self.devices = children::hosts_first(self.devices);
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

    fn check_guests(&self) -> Result<(), Invalid> {
        let hosts: HashMap<DeviceId, Option<DeviceId>> = self
            .devices
            .iter()
            .map(|d| (d.id, d.guest.map(|g| g.host)))
            .collect();
        for &host in hosts.values().flatten() {
            if !hosts.contains_key(&host) {
                return Err(Invalid::MissingReference);
            }
        }
        let host_of = |id: &DeviceId| hosts.get(id).copied().flatten();
        no_loops(hosts.keys().copied(), host_of)
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
    use crate::model::{ConnectionKind, Guest, GuestKind};

    #[test]
    fn the_sample_network_holds_together_and_its_guests_follow_their_hosts() {
        let mut shuffled = mock_data::network();
        shuffled.devices.reverse();
        let network = shuffled.checked().unwrap();
        let place = |id| network.devices.iter().position(|d| d.id == id);
        for device in &network.devices {
            if let Some(guest) = device.guest {
                assert!(place(guest.host) < place(device.id), "{}", device.name);
            }
        }
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
                let guest = Guest {
                    host: DeviceId(999),
                    kind: GuestKind::Container,
                };
                n.devices[0].guest = Some(guest);
            }),
            Some(Invalid::MissingReference)
        );
        assert_eq!(
            broken(|n| {
                let (a, b) = (n.devices[0].id, n.devices[1].id);
                let kind = GuestKind::VirtualMachine;
                n.devices[0].guest = Some(Guest { host: b, kind });
                n.devices[1].guest = Some(Guest { host: a, kind });
            }),
            Some(Invalid::Loop)
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
