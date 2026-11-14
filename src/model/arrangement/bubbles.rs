//! Spatial groups reuse the existing group identity, name and colour. Membership is per
//! device and never changes a role, host, network link or Layer 2 segment.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{Arrangement, valid_position};
use crate::model::{DeviceId, GroupId, Network, Point, Positions};

const MIN_WIDTH: f32 = 520.0;
const MIN_HEIGHT: f32 = 320.0;
const PAD: f32 = 90.0;
const TOP: f32 = 140.0;
const STEP: f32 = 150.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Bubble {
    pub(crate) group: GroupId,
    pub(crate) origin: Point,
    pub(crate) size: Point,
    pub(crate) members: Vec<DeviceId>,
}

impl Bubble {
    pub(crate) const INITIAL_SIZE: Point = Point {
        x: MIN_WIDTH,
        y: MIN_HEIGHT,
    };

    pub(crate) fn corners(&self) -> [Point; 2] {
        [
            self.origin,
            Point {
                x: self.origin.x + self.size.x,
                y: self.origin.y + self.size.y,
            },
        ]
    }

    fn valid(&self) -> bool {
        self.size.x >= MIN_WIDTH
            && self.size.y >= MIN_HEIGHT
            && self.corners().into_iter().all(valid_position)
    }

    fn points<'a>(
        &'a self,
        network: &'a Network,
        positions: &'a Positions,
    ) -> impl Iterator<Item = Point> + 'a {
        self.members
            .iter()
            .filter_map(|id| network.device(*id))
            .flat_map(|d| &d.cards)
            .filter_map(|card| positions.get(&card.id).copied())
    }

    /// Expand rather than evicting members when their cards are moved individually.
    fn fit(&mut self, network: &Network, positions: &Positions) {
        let mut left = self.origin.x;
        let mut top = self.origin.y;
        let mut right = left + self.size.x;
        let mut bottom = top + self.size.y;
        for at in self.points(network, positions) {
            left = left.min(at.x - PAD);
            top = top.min(at.y - TOP);
            right = right.max(at.x + PAD);
            bottom = bottom.max(at.y + PAD);
        }
        let origin = Point { x: left, y: top };
        let size = Point {
            x: right - left,
            y: bottom - top,
        };
        if valid_position(origin)
            && valid_position(Point {
                x: right,
                y: bottom,
            })
        {
            self.origin = origin;
            self.size = size;
        }
    }
}

impl Arrangement {
    pub(crate) fn bubbles(&self) -> &[Bubble] {
        &self.bubbles
    }

    pub(crate) fn add_bubble(&mut self, group: GroupId, origin: Point) {
        if self.bubbles.iter().any(|b| b.group == group) {
            return;
        }
        let bubble = Bubble {
            group,
            origin,
            size: Bubble::INITIAL_SIZE,
            members: Vec::new(),
        };
        if bubble.valid() {
            self.bubbles.push(bubble);
        }
    }

    pub(crate) fn remove_bubble(&mut self, group: GroupId) {
        self.bubbles.retain(|b| b.group != group);
    }

    pub(super) fn retain_bubbles(&mut self, network: &Network) {
        let devices: HashSet<_> = network.devices().iter().map(|d| d.id).collect();
        let mut groups = HashSet::new();
        let mut members = HashSet::new();
        self.bubbles.retain_mut(|bubble| {
            if !network.groups().contains(bubble.group)
                || !bubble.valid()
                || !groups.insert(bubble.group)
            {
                return false;
            }
            bubble
                .members
                .retain(|id| devices.contains(id) && members.insert(*id));
            true
        });
    }

    pub(crate) fn fit_bubbles(&mut self, network: &Network, positions: &Positions) {
        for bubble in &mut self.bubbles {
            bubble.fit(network, positions);
        }
    }

    /// Move all cards of an explicitly selected device into a compact grid in the bubble.
    /// Leaving places them beside its right edge. Other devices keep their positions.
    pub(crate) fn set_bubble_member(
        &mut self,
        network: &Network,
        positions: &mut Positions,
        device: DeviceId,
        target: Option<GroupId>,
    ) {
        let Some(bubble) = self
            .bubbles
            .iter()
            .find(|b| target.map_or_else(|| b.members.contains(&device), |group| b.group == group))
        else {
            return;
        };
        let join = target.is_some();
        let group = bubble.group;
        if bubble.members.contains(&device) == join {
            return;
        }
        let Some(device) = network.device(device) else {
            return;
        };
        let start = if join {
            Point {
                x: bubble.origin.x + PAD,
                y: bubble
                    .points(network, positions)
                    .map(|p| p.y + STEP)
                    .fold(bubble.origin.y + TOP, f32::max),
            }
        } else {
            Point {
                x: bubble.origin.x + bubble.size.x + PAD,
                y: bubble.origin.y + TOP,
            }
        };
        let placed: Option<Vec<_>> = device
            .cards
            .iter()
            .enumerate()
            .map(|(index, card)| {
                let column = f32::from(u16::try_from(index % 3).ok()?);
                let row = f32::from(u16::try_from(index / 3).ok()?);
                let at = Point {
                    x: start.x + column * STEP,
                    y: start.y + row * STEP,
                };
                valid_position(Point {
                    x: at.x + PAD,
                    y: at.y + PAD,
                })
                .then_some((card.id, at))
            })
            .collect();
        let Some(placed) = placed else { return };
        for other in &mut self.bubbles {
            other.members.retain(|id| *id != device.id);
            if other.group == group && join {
                other.members.push(device.id);
            }
        }
        positions.extend(placed);
        self.save_positions(positions);
        self.fit_bubbles(network, positions);
    }

    /// A group move is a translation, preserving every member's relative placement.
    pub(crate) fn move_bubble(
        &mut self,
        network: &Network,
        positions: &mut Positions,
        group: GroupId,
        origin: Point,
    ) {
        let Some(bubble) = self.bubbles.iter_mut().find(|b| b.group == group) else {
            return;
        };
        let delta = Point {
            x: origin.x - bubble.origin.x,
            y: origin.y - bubble.origin.y,
        };
        let moved = Bubble {
            origin,
            ..bubble.clone()
        };
        let translate = |p: Point| Point {
            x: p.x + delta.x,
            y: p.y + delta.y,
        };
        if !moved.valid()
            || !bubble
                .points(network, positions)
                .all(|p| valid_position(translate(p)))
        {
            return;
        }
        let members: HashSet<_> = bubble.members.iter().copied().collect();
        for device in network.devices().iter().filter(|d| members.contains(&d.id)) {
            for card in &device.cards {
                if let Some(at) = positions.get_mut(&card.id) {
                    *at = translate(*at);
                }
            }
        }
        bubble.origin = origin;
        self.save_positions(positions);
    }

    pub(crate) fn resize_bubble(
        &mut self,
        network: &Network,
        positions: &Positions,
        group: GroupId,
        size: Point,
    ) {
        let Some(bubble) = self.bubbles.iter_mut().find(|b| b.group == group) else {
            return;
        };
        let previous = bubble.clone();
        bubble.size = Point {
            x: size.x.max(MIN_WIDTH),
            y: size.y.max(MIN_HEIGHT),
        };
        bubble.fit(network, positions);
        if !size.x.is_finite() || !size.y.is_finite() || !bubble.valid() {
            *bubble = previous;
        }
    }

    /// Automatic layout leaves explicitly grouped devices and their frames in place.
    pub(crate) fn reset_layout(&mut self, network: &Network) {
        let members: HashSet<_> = self
            .bubbles
            .iter()
            .flat_map(|b| b.members.iter().copied())
            .collect();
        let cards: HashSet<_> = network
            .devices()
            .iter()
            .filter(|d| members.contains(&d.id))
            .flat_map(|d| d.cards.iter().map(|c| c.id))
            .collect();
        self.positions.retain(|id, _| cards.contains(id));
        self.tiers.clear();
    }
}
