//! Where a dragged device or ghost would join the map: the nearest trunk, root circle or line, and the
//! place on that trunk it was dropped at.

use std::collections::HashMap;

use super::arrangement::{Arrangement, Level, Side, Slot, Tier};
use super::layout::{LINE_TOP_GAP, Point, Positions, ROW_HEIGHT, link_route, trunk};
use super::network::{Network, Node};

/// How near a line a dragged device's own line must end for it to join that line, above or
/// below it. Rows are close together, so this stays small.
const SNAP_DISTANCE: f32 = 48.0;
/// The same reach to either side, larger as columns are far apart: a device brought up beside
/// a trunk joins it well before it covers it.
const SNAP_REACH_SIDEWAYS: f32 = 110.0;

/// Where a dragged device would join the map if dropped now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Snap {
    /// The node whose trunk it would hang from.
    pub(crate) upstream: Node,
    /// Where on that trunk.
    pub(crate) slot: Slot,
    /// A short line showing the join: from the nearest point of the trunk or line it would join
    /// to the top of its icon.
    pub(crate) line: [Point; 2],
    /// How far that line is stretched towards the edge of the reach, from 0 (no gap) to 1 (about
    /// to let go).
    pub(crate) pull: f32,
}

/// Where `dragged` would join if dropped with its icon centred on `at`: on the trunk, root circle
/// or lines of the nearest node it can join (see [`can_join`]), at the place on that trunk where
/// it was dropped. `None` when nothing is near.
pub(crate) fn snap_target(
    network: &Network,
    arrangement: &Arrangement,
    positions: &Positions,
    dragged: Node,
    at: Point,
) -> Option<Snap> {
    // The map without the dragged branch, whose lines and trunks move with it.
    let others: Positions = positions
        .iter()
        .filter(|&(&id, _)| !network.is_within(id, dragged))
        .map(|(&id, &p)| (id, p))
        .collect();
    // Where the dragged device's own line would end.
    let end = Point {
        x: at.x,
        y: at.y - LINE_TOP_GAP,
    };
    let inner = arrangement.inner_neighbours(network, Some(dragged));
    let candidates = others
        .keys()
        .copied()
        .filter(|&id| can_join(network, dragged, id));
    let near = candidates.filter_map(|id| {
        let lines = Lines {
            network,
            positions: &others,
            inner: &inner,
        };
        Some((id, lines.nearest_on(id, end)?))
    });
    near.filter(|&(_, joins)| distance(end, joins) <= SNAP_DISTANCE)
        .min_by(|a, b| distance(end, a.1).total_cmp(&distance(end, b.1)))
        .map(|(upstream, joins)| {
            let tiers = arrangement.tiers_without(network, upstream, dragged);
            let trunk_x = others.get(&upstream).map_or(at.x, |p| p.x);
            Snap {
                upstream,
                slot: slot(&tiers, &others, trunk_x, at),
                line: [joins, end],
                pull: distance(end, joins) / SNAP_DISTANCE,
            }
        })
}

/// Whether `dragged` may hang from `target`. A guest only ever joins its own host. A ghost may
/// go back under its own device. Anything else joins a node it can plug into, away from every
/// node of its own device, which would form a loop.
fn can_join(network: &Network, dragged: Node, target: Node) -> bool {
    let Some(owner) = network.node_device(dragged) else {
        return false;
    };
    match dragged {
        Node::Device(_) if owner.guest.is_some() => {
            network.host(owner.id).map(Node::Device) == Some(target)
        }
        Node::Card(_) if target == Node::Device(owner.id) => true,
        Node::Device(_) | Node::Card(_) => {
            network.socket(target).is_some() && !network.within_device(target, owner.id)
        }
    }
}

/// The trunks and lines of the map without the dragged branch, as drawn.
struct Lines<'a> {
    network: &'a Network,
    positions: &'a Positions,
    inner: &'a HashMap<Node, Node>,
}

impl Lines<'_> {
    /// The point nearest `end` on what belongs to `id`: its trunk down to its root circle, and
    /// its lines to the devices behind it.
    fn nearest_on(&self, id: Node, end: Point) -> Option<Point> {
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
    ((a.x - b.x) * SNAP_DISTANCE / SNAP_REACH_SIDEWAYS).hypot(a.y - b.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::tree_layout;

    use crate::model::{CardId, DeviceId};

    /// The router's LAN card, which the home network hangs from.
    const ROUTER: Node = Node::Card(CardId(1));
    const SWITCH: Node = Node::Device(DeviceId(2));
    const ACCESS_POINT: Node = Node::Device(DeviceId(3));
    const NAS: Node = Node::Device(DeviceId(4));
    const PC: Node = Node::Device(DeviceId(5));

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
        let [_, root] = trunk(&network, &positions, NAS).unwrap();
        let on_root = Point {
            x: root.x,
            y: root.y + LINE_TOP_GAP,
        };
        let snap = snap_target(&network, &arrangement, &positions, PC, on_root).unwrap();
        assert_eq!((snap.upstream, snap.slot.level), (NAS, Level::NewTier(0)));
    }

    #[test]
    fn a_device_dropped_on_a_trunk_below_a_tier_starts_a_new_one_on_that_side() {
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
        assert_eq!(snap.upstream, SWITCH);
        assert_eq!(
            (snap.slot.level, snap.slot.side),
            (Level::NewTier(1), Side::Left)
        );
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
    fn a_ghost_joins_where_its_card_can_plug_in_or_goes_back_under_its_device() {
        let (network, arrangement, positions) = map();
        let on_root = |id| {
            let [_, root] = trunk(&network, &positions, id).unwrap();
            Point {
                x: root.x,
                y: root.y + LINE_TOP_GAP,
            }
        };
        let ghost = Node::Card(CardId(100));
        let target = |id| snap_target(&network, &arrangement, &positions, ghost, on_root(id));
        let (lab_pi, docker, portainer) = (
            Node::Device(DeviceId(23)),
            Node::Device(DeviceId(19)),
            Node::Device(DeviceId(24)),
        );
        assert_eq!(target(lab_pi).map(|s| s.upstream), Some(lab_pi));
        assert_eq!(target(docker).map(|s| s.upstream), Some(docker), "its own");
        assert_ne!(
            target(portainer).map(|s| s.upstream),
            Some(portainer),
            "on its own device"
        );
    }

    #[test]
    fn a_trunk_reaches_further_to_the_side_than_up_or_down() {
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
}
