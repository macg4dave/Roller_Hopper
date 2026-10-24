//! Deterministic topology suggestions from addresses, gateways, card types and device roles.
//! Manual links are evidence and remain untouched; this module only proposes inferred links.

use std::net::Ipv4Addr;

use super::{CardLink, Connection, ConnectionKind, Network};
use crate::model::{CardId, Device, NetworkCard, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoMapConfidence {
    Strong,
    Likely,
    Tentative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoMapReason {
    Gateway,
    Wireless,
    Wired,
    Subnet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoMapSuggestion {
    pub(crate) downstream: CardId,
    pub(crate) upstream: CardId,
    pub(crate) confidence: AutoMapConfidence,
    pub(crate) reason: AutoMapReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutoMapProposal {
    pub(crate) suggestions: Vec<AutoMapSuggestion>,
    pub(crate) unresolved: usize,
}

impl Network {
    /// Builds a repeatable map without changing the network. Known links are preserved, while
    /// cards with inferred links are reconsidered from the current evidence.
    pub(crate) fn auto_map_proposal(&self) -> AutoMapProposal {
        let mut suggestions = Vec::new();
        let mut unresolved = 0;
        for device in &self.devices {
            for card in &device.cards {
                if self
                    .link(card.id)
                    .is_some_and(|link| link.kind == ConnectionKind::Known)
                    || device.role == Role::Router
                {
                    continue;
                }
                match self.auto_map_card(device, card) {
                    Some(suggestion) => suggestions.push(suggestion),
                    None => unresolved += 1,
                }
            }
        }
        AutoMapProposal {
            suggestions,
            unresolved,
        }
    }

    /// Replaces inferred links with a reviewed proposal and returns the previous inferred links
    /// so the whole operation can be undone in one step.
    pub(crate) fn apply_auto_map(&mut self, proposal: &AutoMapProposal) -> Vec<Connection> {
        let previous = self
            .links
            .iter()
            .filter(|link| link.kind == ConnectionKind::Inferred)
            .map(Connection::from)
            .collect();
        self.links.retain(|link| link.kind == ConnectionKind::Known);
        for suggestion in &proposal.suggestions {
            self.plug_found(
                suggestion.downstream,
                Some((suggestion.upstream, ConnectionKind::Inferred)),
            );
        }
        previous
    }

    pub(crate) fn restore_inferred(&mut self, links: &[Connection]) {
        self.links.retain(|link| link.kind == ConnectionKind::Known);
        self.links.extend(links.iter().map(|link| CardLink {
            upstream: link.upstream,
            card: link.downstream,
            kind: ConnectionKind::Inferred,
        }));
    }

    fn auto_map_card(&self, device: &Device, card: &NetworkCard) -> Option<AutoMapSuggestion> {
        let (router, exact_gateway) = self.router_for(card)?;
        let infrastructure = self.infrastructure_for(device, card, router);
        let (upstream, confidence, reason) = match infrastructure {
            Some((upstream, wireless)) => (
                upstream,
                AutoMapConfidence::Likely,
                if wireless {
                    AutoMapReason::Wireless
                } else {
                    AutoMapReason::Wired
                },
            ),
            None if exact_gateway => (router, AutoMapConfidence::Strong, AutoMapReason::Gateway),
            None => (router, AutoMapConfidence::Tentative, AutoMapReason::Subnet),
        };
        Some(AutoMapSuggestion {
            downstream: card.id,
            upstream,
            confidence,
            reason,
        })
    }

    /// A unique router card selected first by an exact default-gateway address, otherwise by a
    /// shared subnet. More than one equally good candidate is deliberately unresolved.
    fn router_for(&self, card: &NetworkCard) -> Option<(CardId, bool)> {
        let routers = self.cards_with_role(&Role::Router);
        if let Some(gateway) = card.gateway {
            let exact: Vec<CardId> = routers
                .iter()
                .filter(|(_, candidate)| address(candidate) == Some(gateway))
                .map(|(id, _)| *id)
                .collect();
            if exact.len() == 1 {
                return exact.first().copied().map(|id| (id, true));
            }
            if !exact.is_empty() {
                return None;
            }
        }
        let same_subnet: Vec<CardId> = routers
            .into_iter()
            .filter(|(_, candidate)| shares_subnet(card, candidate))
            .map(|(id, _)| id)
            .collect();
        (same_subnet.len() == 1)
            .then(|| same_subnet.first().copied())
            .flatten()
            .map(|id| (id, false))
    }

    fn infrastructure_for(
        &self,
        device: &Device,
        card: &NetworkCard,
        router: CardId,
    ) -> Option<(CardId, bool)> {
        if matches!(device.role, Role::Switch | Role::AccessPoint) {
            return None;
        }
        let wireless = prefers_wireless(device, card);
        let role = if wireless {
            Role::AccessPoint
        } else {
            Role::Switch
        };
        let candidates: Vec<CardId> = self
            .devices
            .iter()
            .filter(|candidate| candidate.role == role && candidate.id != device.id)
            .filter_map(|candidate| best_infrastructure_card(card, candidate, router, self))
            .collect();
        (candidates.len() == 1)
            .then(|| candidates.first().copied())
            .flatten()
            .map(|id| (id, wireless))
    }

    fn cards_with_role(&self, role: &Role) -> Vec<(CardId, &NetworkCard)> {
        self.devices
            .iter()
            .filter(|device| device.role == *role)
            .flat_map(|device| device.cards.iter().map(|card| (card.id, card)))
            .collect()
    }
}

/// One card per infrastructure device, preferring the same gateway over merely sharing a
/// subnet. This keeps a dual-homed switch or access point from becoming ambiguous with itself.
fn best_infrastructure_card(
    downstream: &NetworkCard,
    device: &Device,
    router: CardId,
    network: &Network,
) -> Option<CardId> {
    if downstream.gateway.is_some() {
        let exact = device
            .cards
            .iter()
            .find(|candidate| candidate.gateway == downstream.gateway);
        if let Some(card) = exact {
            return Some(card.id);
        }
    }
    device
        .cards
        .iter()
        .find(|candidate| shares_domain(downstream, candidate, router, network))
        .map(|card| card.id)
}

fn address(card: &NetworkCard) -> Option<Ipv4Addr> {
    card.address.map(|network| network.addr())
}

fn shares_subnet(a: &NetworkCard, b: &NetworkCard) -> bool {
    match (a.address, b.address) {
        (Some(a), Some(b)) => a.contains(&b.addr()) || b.contains(&a.addr()),
        _ => false,
    }
}

fn shares_domain(
    card: &NetworkCard,
    candidate: &NetworkCard,
    router: CardId,
    network: &Network,
) -> bool {
    if card.gateway.is_some() && card.gateway == candidate.gateway {
        return true;
    }
    shares_subnet(card, candidate)
        || network
            .card(router)
            .is_some_and(|router_card| shares_subnet(candidate, router_card))
}

fn prefers_wireless(device: &Device, card: &NetworkCard) -> bool {
    card.icon.0.ends_with("/Wi-Fi")
        || matches!(
            device.role,
            Role::Phone | Role::Iot | Role::Camera | Role::Laptop
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    #[test]
    fn known_links_are_preserved_and_inferred_cards_are_reconsidered() {
        let network = mock_data::network();
        let proposal = network.auto_map_proposal();
        assert!(proposal.suggestions.iter().all(|suggestion| {
            network
                .uplink(suggestion.downstream)
                .is_none_or(|link| link.kind == ConnectionKind::Inferred)
        }));
        assert!(
            proposal
                .suggestions
                .iter()
                .any(|suggestion| { suggestion.reason == AutoMapReason::Wireless })
        );
    }

    #[test]
    fn applying_and_undoing_restores_the_inferred_topology_exactly() {
        let mut network = mock_data::network();
        let before: Vec<Connection> = network
            .connections()
            .into_iter()
            .filter(|link| link.kind == ConnectionKind::Inferred)
            .collect();
        let proposal = network.auto_map_proposal();
        let undo = network.apply_auto_map(&proposal);
        assert_eq!(undo, before);
        network.restore_inferred(&undo);
        let restored: Vec<Connection> = network
            .connections()
            .into_iter()
            .filter(|link| link.kind == ConnectionKind::Inferred)
            .collect();
        assert_eq!(restored, before);
    }

    #[test]
    fn a_second_proposal_is_identical_after_applying_the_first() {
        let mut network = mock_data::network();
        let proposal = network.auto_map_proposal();
        network.apply_auto_map(&proposal);
        assert_eq!(network.auto_map_proposal(), proposal);
    }
}
