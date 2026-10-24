//! Where a dragged network card would join the map: the nearest trunk, root circle or line, and the
//! place on that trunk it was dropped at.

use std::collections::HashMap;

use super::arrangement::{Arrangement, Level, Side, Slot, Tier};
use super::card::CardId;
use super::layout::{LINE_TOP_GAP, Point, Positions, ROW_HEIGHT, link_route, trunk};
use super::network::Network;

/// How near a line a dragged device's own line must end for it to join that line.
const SNAP_DISTANCE: f32 = 72.0;
/// The same reach to either side, larger as columns are far apart: a device brought up beside
/// a trunk joins it well before it covers it.
const SNAP_REACH_SIDEWAYS: f32 = 160.0;
/// An existing connection holds on beyond the range where a new one would catch. This
/// hysteresis makes detaching deliberate and lets the strand stretch like a rubber band.
const DETACH_DISTANCE: f32 = 128.0;
const DETACH_REACH_SIDEWAYS: f32 = 400.0;
/// A top connection is an intentional, open drop target, so it reaches farther than the denser
/// lines within a tree.
const HEAD_SNAP_DISTANCE: f32 = 96.0;
const HEAD_REACH_SIDEWAYS: f32 = 180.0;

/// Where a dragged device would join the map if dropped now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Snap {
    /// The card whose trunk it would hang from, or the ancestor it would become head above.
    pub(crate) upstream: CardId,
    pub(crate) kind: SnapKind,
    /// Where on that trunk.
    pub(crate) slot: Slot,
    /// A short line from the existing tree's join point to the dragged icon's nearest edge.
    pub(crate) line: [Point; 2],
    /// How far that line is stretched towards the edge of the reach, from 0 (no gap) to 1 (about
    /// to let go).
    pub(crate) pull: f32,
}

/// Which side of the existing tree a dragged card will join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapKind {
    Child,
    Head,
}

/// Where `dragged` would join if dropped with its icon centred on `at`: on the trunk, root circle
/// or lines of the nearest card it can be plugged into (see [`Network::can_plug`]), at the place
/// on that trunk where it was dropped. `None` when nothing is near.
pub(crate) fn snap_target(
    network: &Network,
    arrangement: &Arrangement,
    positions: &Positions,
    dragged: CardId,
    at: Point,
) -> Option<Snap> {
    let head = head_target(network, positions, dragged, at);
    // The map without the dragged branch, whose lines and trunks move with it.
    let others: Positions = positions
        .iter()
        .filter(|&(&id, _)| !network.is_within(id, dragged))
        .map(|(&id, &p)| (id, p))
        .collect();
    // Where the dragged card's own line would end.
    let end = Point {
        x: at.x,
        y: at.y - LINE_TOP_GAP,
    };
    let inner = arrangement.inner_neighbours(network, Some(dragged));
    let current = network.uplink(dragged).map(|link| link.upstream);
    let candidates = others
        .keys()
        .copied()
        .filter(|&id| network.can_plug(dragged, id));
    let near = candidates.filter_map(|id| {
        let lines = Lines {
            network,
            positions: &others,
            inner: &inner,
        };
        let joins = lines.nearest_on(id, end)?;
        let retained = current == Some(id);
        let (limit, reach) = if retained {
            (DETACH_DISTANCE, DETACH_REACH_SIDEWAYS)
        } else {
            (SNAP_DISTANCE, SNAP_REACH_SIDEWAYS)
        };
        let stretched = scaled_distance(end, joins, limit, reach);
        let proximity = distance(end, joins);
        (stretched <= limit).then_some((id, joins, proximity, stretched, limit))
    });
    let child =
        near.min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(upstream, joins, _, stretched, limit)| {
                let tiers = arrangement.tiers_without(network, upstream, dragged);
                let trunk_x = others.get(&upstream).map_or(at.x, |p| p.x);
                Snap {
                    upstream,
                    kind: SnapKind::Child,
                    slot: slot(&tiers, &others, trunk_x, at),
                    line: [joins, end],
                    pull: stretched / limit,
                }
            });
    closest(head, child)
}

/// The physically closest valid connection wins, regardless of whether it is above a node or
/// on a trunk. Extended detach reach only keeps a candidate available; it does not prioritise it.
fn closest(a: Option<Snap>, b: Option<Snap>) -> Option<Snap> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let length = |snap: &Snap| distance(snap.line[0], snap.line[1]);
            Some(if length(&a) <= length(&b) { a } else { b })
        }
        (Some(snap), None) | (None, Some(snap)) => Some(snap),
        (None, None) => None,
    }
}

/// A card held above another device is inserted directly above it.
fn head_target(
    network: &Network,
    positions: &Positions,
    dragged: CardId,
    at: Point,
) -> Option<Snap> {
    let end = Point {
        x: at.x,
        y: at.y + LINE_TOP_GAP,
    };
    network
        .nodes()
        .into_iter()
        .filter(|&candidate| network.can_connect_above(dragged, candidate))
        .filter_map(|candidate| {
            let ancestor = *positions.get(&candidate)?;
            let joins = Point {
                x: ancestor.x,
                y: ancestor.y - LINE_TOP_GAP,
            };
            (end.y <= joins.y && head_distance(end, joins) <= HEAD_SNAP_DISTANCE)
                .then_some((candidate, joins))
        })
        .min_by(|a, b| head_distance(end, a.1).total_cmp(&head_distance(end, b.1)))
        .map(|(upstream, joins)| Snap {
            upstream,
            kind: SnapKind::Head,
            slot: Slot {
                level: Level::NewTier(0),
                side: Side::Right,
                before: None,
            },
            line: [joins, end],
            pull: head_distance(end, joins) / HEAD_SNAP_DISTANCE,
        })
}

/// The trunks and lines of the map without the dragged branch, as drawn.
struct Lines<'a> {
    network: &'a Network,
    positions: &'a Positions,
    inner: &'a HashMap<CardId, CardId>,
}

impl Lines<'_> {
    /// The point nearest `end` on what belongs to `id`: its trunk down to its root circle, and
    /// its lines to the cards plugged into it.
    fn nearest_on(&self, id: CardId, end: Point) -> Option<Point> {
        let [top, root] = trunk(self.network, self.positions, id)?;
        let lines = self
            .network
            .behind(id)
            .filter_map(|other| link_route(self.positions, self.inner, id, other))
            .flat_map(|[a, b, c]| [(a, b), (b, c)]);
        let segments = lines.chain([(top, root)]);
        segments
            .map(|(a, b)| nearest(end, a, b))
            .min_by(|a, b| distance(end, *a).total_cmp(&distance(end, *b)))
    }
}

/// Where a device dropped at `at` goes among the `tiers` hanging off a trunk at `trunk_x`: level
/// with a tier, it joins that tier where it was dropped; otherwise it starts a new tier at that
/// height. It hangs on the side of the trunk it was dropped on.
fn slot(tiers: &[Tier], positions: &Positions, trunk_x: f32, at: Point) -> Slot {
    let side = if at.x < trunk_x {
        Side::Left
    } else {
        Side::Right
    };
    let heights: Vec<f32> = tiers
        .iter()
        .map(|t| {
            let ys = t.devices().filter_map(|d| positions.get(&d)).map(|p| p.y);
            ys.fold(f32::INFINITY, f32::min)
        })
        .collect();
    let level = heights
        .iter()
        .position(|&y| (at.y - y).abs() <= ROW_HEIGHT / 2.0)
        .map_or_else(
            || Level::NewTier(heights.iter().filter(|&&y| y < at.y).count()),
            Level::Tier,
        );
    let before = match level {
        Level::Tier(index) => tiers.get(index).and_then(|tier| {
            let mut on_side = tier.side(side).iter().copied();
            on_side.find(|d| positions.get(d).is_some_and(|p| p.x > at.x))
        }),
        Level::NewTier(_) => None,
    };
    Slot {
        level,
        side,
        before,
    }
}

/// The point of the horizontal or vertical segment from `start` to `end` nearest to `point`.
fn nearest(point: Point, start: Point, end: Point) -> Point {
    Point {
        x: point.x.clamp(start.x.min(end.x), start.x.max(end.x)),
        y: point.y.clamp(start.y.min(end.y), start.y.max(end.y)),
    }
}

/// How far apart two points are for snapping, with distances to the side counting for less
/// (see [`SNAP_REACH_SIDEWAYS`]). The nearest point on a horizontal or vertical segment is the
/// same under this measure as under the usual one.
fn distance(a: Point, b: Point) -> f32 {
    scaled_distance(a, b, SNAP_DISTANCE, SNAP_REACH_SIDEWAYS)
}

fn head_distance(a: Point, b: Point) -> f32 {
    scaled_distance(a, b, HEAD_SNAP_DISTANCE, HEAD_REACH_SIDEWAYS)
}

fn scaled_distance(a: Point, b: Point, vertical: f32, sideways: f32) -> f32 {
    ((a.x - b.x) * vertical / sideways).hypot(a.y - b.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::tree_layout;

    // Each sample device's first card has its id; the router's first is its LAN card.
    const ROUTER: CardId = CardId(1);
    const SWITCH: CardId = CardId(2);
    const ACCESS_POINT: CardId = CardId(3);
    const NAS: CardId = CardId(4);
    const PC: CardId = CardId(5);

    fn map() -> (Network, Arrangement, Positions) {
        let network = mock_data::network();
        let arrangement = Arrangement::default();
        let positions = tree_layout(&network, &arrangement);
        (network, arrangement, positions)
    }

    #[test]
    fn a_device_dropped_on_a_tiers_line_joins_it_where_dropped() {
        let (network, arrangement, positions) = map();
        // Between the switch and the access point, both right of the router's trunk.
        let between = Point {
            x: f32::midpoint(positions[&SWITCH].x, positions[&ACCESS_POINT].x),
            y: positions[&SWITCH].y,
        };
        let snap = snap_target(&network, &arrangement, &positions, PC, between).unwrap();
        assert_eq!(snap.upstream, ROUTER);
        let expected = Slot {
            level: Level::Tier(0),
            side: Side::Right,
            before: Some(ACCESS_POINT),
        };
        assert_eq!(snap.slot, expected);
        assert_eq!(snap.kind, SnapKind::Child);
        // It joins the line straight above it, not back at the router.
        let [joins, end] = snap.line;
        assert_eq!((joins.x, end.x), (between.x, between.x));
        assert!(joins.y < end.y);
        let far = Point {
            x: -1000.0,
            y: -1000.0,
        };
        assert_eq!(
            snap_target(&network, &arrangement, &positions, PC, far),
            None
        );
    }

    #[test]
    fn a_device_dropped_on_a_root_circle_goes_behind_that_device() {
        let (network, arrangement, positions) = map();
        let lab_root = CardId(22);
        let [_, root] = trunk(&network, &positions, lab_root).unwrap();
        let on_root = Point {
            x: root.x,
            y: root.y + LINE_TOP_GAP,
        };
        let snap = snap_target(&network, &arrangement, &positions, PC, on_root).unwrap();
        assert_eq!((snap.upstream, snap.kind), (lab_root, SnapKind::Child));
    }

    #[test]
    fn a_device_with_a_parent_can_still_offer_the_closest_child_connection() {
        let (network, arrangement, positions) = map();
        let switch = positions[&SWITCH];
        let row = network
            .behind(SWITCH)
            .map(|d| positions[&d].y)
            .fold(0.0, f32::max);
        // Just left of the switch's trunk, below its row of devices, above its root circle.
        let below_left = Point {
            x: switch.x - 10.0,
            y: row + ROW_HEIGHT * 0.6,
        };
        let snap = snap_target(&network, &arrangement, &positions, NAS, below_left).unwrap();
        assert_eq!((snap.upstream, snap.kind), (SWITCH, SnapKind::Child));
    }

    #[test]
    fn a_dragged_device_never_joins_its_own_branch() {
        let (network, arrangement, positions) = map();
        for device in network.behind(SWITCH) {
            let target = snap_target(
                &network,
                &arrangement,
                &positions,
                SWITCH,
                positions[&device],
            );
            assert!(
                !target.is_some_and(|t| network.is_within(t.upstream, SWITCH)),
                "{device:?}"
            );
        }
    }

    #[test]
    fn a_descendant_held_above_an_ancestor_previews_becoming_the_head() {
        let (network, arrangement, positions) = map();
        let router = positions[&ROUTER];
        let above = Point {
            x: router.x,
            y: router.y - 2.0 * LINE_TOP_GAP,
        };
        let snap = snap_target(&network, &arrangement, &positions, PC, above).unwrap();
        assert_eq!(snap.upstream, ROUTER);
        assert_eq!(snap.kind, SnapKind::Head);
    }

    #[test]
    fn a_separate_device_can_reach_the_open_top_of_a_root_from_farther_away() {
        let (network, arrangement, positions) = map();
        let lab_root = CardId(22);
        let router = positions[&ROUTER];
        let above_and_to_the_side = Point {
            x: router.x + 140.0,
            y: router.y - 90.0,
        };
        let snap = snap_target(
            &network,
            &arrangement,
            &positions,
            lab_root,
            above_and_to_the_side,
        )
        .unwrap();
        assert_eq!((snap.upstream, snap.kind), (ROUTER, SnapKind::Head));
        assert_eq!(snap.line[0].y, router.y - LINE_TOP_GAP);
        assert_eq!(snap.line[1].y, above_and_to_the_side.y + LINE_TOP_GAP);
    }

    #[test]
    fn a_separate_device_can_preview_insertion_above_a_non_root() {
        let (network, arrangement, positions) = map();
        let lab_root = CardId(22);
        let switch = positions[&SWITCH];
        let above = Point {
            x: switch.x,
            y: switch.y - 70.0,
        };
        let snap = snap_target(&network, &arrangement, &positions, lab_root, above).unwrap();
        assert_eq!((snap.upstream, snap.kind), (SWITCH, SnapKind::Head));
    }

    #[test]
    fn the_closest_link_wins_when_a_node_and_trunk_are_beside_each_other() {
        let (network, arrangement, positions) = map();
        let lab_root = CardId(22);
        let switch = positions[&SWITCH];
        let router = positions[&ROUTER];
        let between = Point {
            x: f32::midpoint(router.x, switch.x),
            y: switch.y - 70.0,
        };
        let nearer_node = snap_target(
            &network,
            &arrangement,
            &positions,
            lab_root,
            Point {
                x: switch.x,
                ..between
            },
        )
        .unwrap();
        assert_eq!(
            (nearer_node.upstream, nearer_node.kind),
            (SWITCH, SnapKind::Head)
        );

        let nearer_trunk = snap_target(
            &network,
            &arrangement,
            &positions,
            lab_root,
            Point {
                x: router.x,
                ..between
            },
        )
        .unwrap();
        assert_eq!(
            (nearer_trunk.upstream, nearer_trunk.kind),
            (ROUTER, SnapKind::Child)
        );
    }

    #[test]
    fn a_card_joins_a_non_root_but_not_its_own_device() {
        let (network, arrangement, positions) = map();
        let on_root = |id| {
            let [_, root] = trunk(&network, &positions, id).unwrap();
            Point {
                x: root.x,
                y: root.y + LINE_TOP_GAP,
            }
        };
        let lab_card = CardId(100);
        let target = |id| snap_target(&network, &arrangement, &positions, lab_card, on_root(id));
        let (lab_pi, docker_home) = (CardId(23), CardId(19));
        assert_eq!(target(lab_pi).map(|s| s.upstream), Some(lab_pi));
        assert_ne!(
            target(docker_home).map(|s| s.upstream),
            Some(docker_home),
            "its own device's other card"
        );
    }

    #[test]
    fn a_non_root_trunk_attracts_a_new_child_when_it_is_closest() {
        let (network, arrangement, positions) = map();
        let [top, root] = trunk(&network, &positions, SWITCH).unwrap();
        // Beside the bare stretch of trunk below its row of devices, clear of their lines.
        let beside = Point {
            x: top.x + 100.0,
            y: root.y - ROW_HEIGHT / 2.0 + LINE_TOP_GAP,
        };
        let joined = snap_target(&network, &arrangement, &positions, NAS, beside);
        assert_eq!(joined.map(|s| s.upstream), Some(SWITCH));
        let as_far_below = Point {
            x: root.x,
            y: root.y + LINE_TOP_GAP + 100.0,
        };
        let target = snap_target(&network, &arrangement, &positions, NAS, as_far_below);
        assert_ne!(target.map(|s| s.upstream), Some(SWITCH));
    }

    #[test]
    fn a_new_connection_catches_before_the_device_has_to_touch_the_line() {
        let (network, arrangement, positions) = map();
        let lab_root = CardId(22);
        let [top, _] = trunk(&network, &positions, lab_root).unwrap();
        let within_wider_reach = Point {
            x: top.x,
            y: top.y + LINE_TOP_GAP + 60.0,
        };
        let snap =
            snap_target(&network, &arrangement, &positions, NAS, within_wider_reach).unwrap();
        assert_eq!((snap.upstream, snap.kind), (lab_root, SnapKind::Child));
    }

    #[test]
    fn an_existing_parent_holds_on_like_a_stretched_rubber_band() {
        let (mut network, arrangement, _) = map();
        let removed: Vec<_> = network
            .devices()
            .iter()
            .map(|device| device.id)
            .filter(|id| ![crate::model::DeviceId(2), crate::model::DeviceId(5)].contains(id))
            .collect();
        for device in removed {
            network.remove_device(device);
        }
        let positions = tree_layout(&network, &arrangement);
        let [top, _] = trunk(&network, &positions, SWITCH).unwrap();
        let deliberate_pull = Point {
            x: top.x + 250.0,
            y: top.y + LINE_TOP_GAP + 60.0,
        };
        let snap = snap_target(&network, &arrangement, &positions, PC, deliberate_pull).unwrap();
        assert_eq!((snap.upstream, snap.kind), (SWITCH, SnapKind::Child));
        assert!(
            distance(snap.line[0], snap.line[1]) > SNAP_DISTANCE,
            "held beyond the range where a new connection would catch"
        );
        assert!(snap.pull > 0.0, "the strand shows the pull");
    }
}
