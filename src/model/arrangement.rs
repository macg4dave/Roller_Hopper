//! How the cards plugged into each card are arranged down its trunk: in tiers, one below the
//! other, each split to the left and right of the trunk. This is layout only: what is plugged
//! into what is in [`Network`], and a card's place here never changes its uplink.

use std::collections::HashMap;

use super::card::CardId;
use super::network::Network;

/// Which side of a trunk a device hangs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
}

/// One level of devices hanging off a trunk, each side listed left to right.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Tier {
    pub(crate) left: Vec<CardId>,
    pub(crate) right: Vec<CardId>,
}

impl Tier {
    pub(crate) fn side(&self, side: Side) -> &[CardId] {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }

    fn side_mut(&mut self, side: Side) -> &mut Vec<CardId> {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }

    pub(crate) fn devices(&self) -> impl Iterator<Item = CardId> {
        self.left.iter().chain(&self.right).copied()
    }

    fn remove(&mut self, id: CardId) {
        self.left.retain(|&d| d != id);
        self.right.retain(|&d| d != id);
    }

    fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }
}

/// Which tier a device goes in: an existing one, or a new one put in at that index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    Tier(usize),
    NewTier(usize),
}

/// Where on a trunk a device goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) level: Level,
    pub(crate) side: Side,
    /// The device on that side it goes just left of; `None` to go rightmost.
    pub(crate) before: Option<CardId>,
}

/// The tiers the user arranged under each device, until the layout is reset.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Arrangement {
    tiers: HashMap<CardId, Vec<Tier>>,
}

impl Arrangement {
    /// The tiers under `parent`, as arranged, holding only devices still connected behind it.
    /// Devices not arranged yet go in the first tier, the first half of them on the left, so
    /// with nothing arranged a device's devices sit in one row centred under it.
    pub(crate) fn tiers(&self, network: &Network, parent: CardId) -> Vec<Tier> {
        let behind: Vec<CardId> = network.behind(parent).collect();
        let mut tiers = self.tiers.get(&parent).cloned().unwrap_or_default();
        for tier in &mut tiers {
            tier.left.retain(|d| behind.contains(d));
            tier.right.retain(|d| behind.contains(d));
        }
        tiers.retain(|t| !t.is_empty());
        let loose: Vec<CardId> = behind
            .into_iter()
            .filter(|&d| !tiers.iter().any(|t| t.devices().any(|placed| placed == d)))
            .collect();
        if !loose.is_empty() {
            if tiers.is_empty() {
                tiers.push(Tier::default());
            }
            if let Some(first) = tiers.first_mut() {
                let (left, right) = loose.split_at(loose.len() / 2);
                first.left.extend(left);
                first.right.extend(right);
            }
        }
        tiers
    }

    /// For every device with another beside it nearer its upstream device's trunk, that other
    /// device: its line branches off that one's. Leaves out `without`, as if it were not there.
    pub(crate) fn inner_neighbours(
        &self,
        network: &Network,
        without: Option<CardId>,
    ) -> HashMap<CardId, CardId> {
        let mut inner = HashMap::new();
        for parent in network.nodes() {
            let tiers = match without {
                Some(id) => self.tiers_without(network, parent, id),
                None => self.tiers(network, parent),
            };
            for tier in tiers {
                // Left of the trunk the next one inwards is to the right; right of it, the left.
                for pair in tier.left.windows(2) {
                    if let [outer, inward] = pair {
                        inner.insert(*outer, *inward);
                    }
                }
                for pair in tier.right.windows(2) {
                    if let [inward, outer] = pair {
                        inner.insert(*outer, *inward);
                    }
                }
            }
        }
        inner
    }

    /// The tiers under `parent` as they are without `id`, with any tier it leaves empty gone.
    pub(crate) fn tiers_without(&self, network: &Network, parent: CardId, id: CardId) -> Vec<Tier> {
        let mut tiers = self.tiers(network, parent);
        for tier in &mut tiers {
            tier.remove(id);
        }
        tiers.retain(|t| !t.is_empty());
        tiers
    }

    /// Puts `id`, connected behind `parent`, at `slot` among the tiers under it, judged as
    /// [`Self::tiers_without`] it; it leaves wherever it was before.
    pub(crate) fn place(&mut self, network: &Network, id: CardId, parent: CardId, slot: Slot) {
        let mut tiers = self.tiers_without(network, parent, id);
        self.remove(id);
        let index = match slot.level {
            Level::Tier(index) => index.min(tiers.len()),
            Level::NewTier(index) => {
                let index = index.min(tiers.len());
                tiers.insert(index, Tier::default());
                index
            }
        };
        if index == tiers.len() {
            tiers.push(Tier::default());
        }
        if let Some(tier) = tiers.get_mut(index) {
            let side = tier.side_mut(slot.side);
            let at = slot
                .before
                .and_then(|b| side.iter().position(|&d| d == b))
                .unwrap_or(side.len());
            side.insert(at, id);
        }
        self.tiers.insert(parent, tiers);
    }

    /// Takes `id` out of every tier, as when it is left standing alone.
    pub(crate) fn remove(&mut self, id: CardId) {
        for tier in self.tiers.values_mut().flatten() {
            tier.remove(id);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.tiers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    // Each sample device's first card has its id; the router's first is its LAN card.
    const ROUTER: CardId = CardId(1);
    const SWITCH: CardId = CardId(2);
    const ACCESS_POINT: CardId = CardId(3);
    const PC: CardId = CardId(5);

    #[test]
    fn unarranged_devices_split_either_side_in_one_tier() {
        let network = mock_data::network();
        let tiers = Arrangement::default().tiers(&network, ROUTER);
        let expected = Tier {
            left: vec![CardId(4)],
            right: vec![SWITCH, ACCESS_POINT],
        };
        assert_eq!(tiers, [expected]);
        assert_eq!(Arrangement::default().tiers(&network, PC), [] as [Tier; 0]);
    }

    #[test]
    fn a_device_put_in_a_new_tier_hangs_below_and_leaves_its_old_place() {
        let mut network = mock_data::network();
        let mut arrangement = Arrangement::default();
        network.connect(PC, Some(ROUTER));
        let below_left = Slot {
            level: Level::NewTier(1),
            side: Side::Left,
            before: None,
        };
        arrangement.place(&network, PC, ROUTER, below_left);
        let tiers = arrangement.tiers(&network, ROUTER);
        assert_eq!(tiers.len(), 2);
        assert_eq!(tiers.get(1).map(|t| t.left.clone()), Some(vec![PC]));
        assert!(
            !arrangement
                .tiers(&network, SWITCH)
                .iter()
                .any(|t| t.devices().any(|d| d == PC))
        );
    }

    #[test]
    fn a_device_joins_a_tier_where_it_was_put() {
        let network = mock_data::network();
        let mut arrangement = Arrangement::default();
        let before_switch = Slot {
            level: Level::Tier(0),
            side: Side::Right,
            before: Some(SWITCH),
        };
        arrangement.place(&network, ACCESS_POINT, ROUTER, before_switch);
        let tiers = arrangement.tiers(&network, ROUTER);
        assert_eq!(
            tiers.first().map(|t| t.right.clone()),
            Some(vec![ACCESS_POINT, SWITCH])
        );
    }

    #[test]
    fn each_device_branches_off_its_neighbour_nearer_the_trunk() {
        let network = mock_data::network();
        let arrangement = Arrangement::default();
        // The router: the NAS left of its trunk; the switch, then the access point, right of it.
        let inner = arrangement.inner_neighbours(&network, None);
        assert_eq!(inner.get(&ACCESS_POINT), Some(&SWITCH));
        assert_eq!(inner.get(&SWITCH), None, "straight off the trunk");
        assert_eq!(inner.get(&CardId(4)), None, "alone on its side");
        // The switch's first three hang left of its trunk, so the PC, leftmost, is outermost.
        assert_eq!(inner.get(&PC), Some(&CardId(6)));

        let without_switch = arrangement.inner_neighbours(&network, Some(SWITCH));
        assert_eq!(without_switch.get(&ACCESS_POINT), None);
    }

    #[test]
    fn devices_no_longer_behind_drop_out_and_empty_tiers_go() {
        let mut network = mock_data::network();
        let mut arrangement = Arrangement::default();
        let alone = Slot {
            level: Level::NewTier(1),
            side: Side::Right,
            before: None,
        };
        arrangement.place(&network, ACCESS_POINT, ROUTER, alone);
        network.connect(ACCESS_POINT, None);
        assert_eq!(arrangement.tiers(&network, ROUTER).len(), 1);
    }
}
