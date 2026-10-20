//! Automatic tree layout of the topology, and the routing of its lines and trunks.
//!
//! Every device has a trunk: a line from below its labels down to a small root circle. The
//! devices behind it hang off the trunk in tiers, to its left and right (see [`Arrangement`]).
//! Positions are in map units (1 unit = 1 logical pixel at 100 % zoom) and mark the centre of a
//! node's icon; its labels sit below.

use std::collections::HashMap;

use super::arrangement::{Arrangement, Side};
use super::device::DeviceId;
use super::network::Network;

const COLUMN_WIDTH: f32 = 150.0;
pub(super) const ROW_HEIGHT: f32 = 150.0;
/// Lines stop short of the icon below a line and leave below the labels of the device above.
pub(super) const LINE_TOP_GAP: f32 = 28.0;
const LINE_BOTTOM_GAP: f32 = 66.0;
/// A line runs across just above the devices it leads to, well below the root circles of the
/// tier above, so a line passing under a device never looks as if it hangs from that device.
const BUS_GAP: f32 = 14.0;
/// A root circle sits this far below the labels of the lowest device in its branch.
const ROOT_GAP: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Point {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

pub(crate) type Positions = HashMap<DeviceId, Point>;

/// Lays the network out as top-down trees, side by side, starting from devices with no uplink.
pub(crate) fn tree_layout(network: &Network, arrangement: &Arrangement) -> Positions {
    let tree = Tree {
        network,
        arrangement,
        max_depth: network.devices().len(),
    };
    let mut positions = Positions::new();
    let mut left = 0.0;
    let roots = network.devices().iter().map(|d| d.id);
    for root in roots.filter(|&id| network.uplink(id).is_none()) {
        let shape = tree.shape(root, 0);
        let spot = Spot {
            trunk: left + shape.left,
            row: 0,
            depth: 0,
        };
        tree.place(root, spot, &mut positions);
        left += shape.left + shape.right;
    }
    positions
}

/// The line to `downstream` from `from_x`: across just above it, then down to it. It starts at
/// the upstream device's trunk, which runs past every tier, or at the line of the device next to
/// it nearer the trunk (see [`Arrangement::inner_neighbours`]), so the lines on one side of a
/// tier join into one and each device adds only its own stretch.
fn route(from_x: f32, downstream: Point) -> [Point; 3] {
    let end = Point {
        x: downstream.x,
        y: downstream.y - LINE_TOP_GAP,
    };
    let bus_y = end.y - BUS_GAP;
    [
        Point {
            x: from_x,
            y: bus_y,
        },
        Point { x: end.x, y: bus_y },
        end,
    ]
}

/// The line from `upstream` to `downstream` as shown at `positions`, branching off the line of
/// the device `inner` gives as next to it nearer the trunk, if that one is on the map (see
/// [`route`]). `None` while either end is off the map.
pub(crate) fn link_route(
    positions: &Positions,
    inner: &HashMap<DeviceId, DeviceId>,
    upstream: DeviceId,
    downstream: DeviceId,
) -> Option<[Point; 3]> {
    let neighbour = inner.get(&downstream).and_then(|n| positions.get(n));
    let from = neighbour.or_else(|| positions.get(&upstream))?;
    Some(route(from.x, *positions.get(&downstream)?))
}

/// The trunk of `id`: from below its labels down to its root circle, which sits below the lowest
/// device in its branch, so the trunk runs past every tier hanging off it.
pub(crate) fn trunk(network: &Network, positions: &Positions, id: DeviceId) -> Option<[Point; 2]> {
    let top = positions.get(&id)?;
    let start = Point {
        x: top.x,
        y: top.y + LINE_BOTTOM_GAP,
    };
    let root = Point {
        x: top.x,
        y: lowest(network, positions, id) + LINE_BOTTOM_GAP + ROOT_GAP,
    };
    Some([start, root])
}

/// How far down the branch of `id` reaches: the lowest of its devices.
fn lowest(network: &Network, positions: &Positions, id: DeviceId) -> f32 {
    reach(network, positions, id, network.devices().len())
}

/// [`lowest`], going no more than `depth` devices deeper, which guards against cycles in the
/// connection data: no real tree is deeper than its size.
fn reach(network: &Network, positions: &Positions, id: DeviceId, depth: usize) -> f32 {
    let Some(top) = positions.get(&id) else {
        return f32::MIN;
    };
    let behind = network.behind(id).filter(|_| depth > 0);
    behind
        .map(|d| reach(network, positions, d, depth - 1))
        .fold(top.y, f32::max)
}

/// How far a device's branch reaches to each side of its trunk, in columns, and how many rows it
/// takes up, its own included.
#[derive(Debug, Clone, Copy)]
struct Shape {
    left: f32,
    right: f32,
    rows: usize,
}

struct Tree<'a> {
    network: &'a Network,
    arrangement: &'a Arrangement,
    /// Guards against cycles in the connection data: no real tree is deeper than its size.
    max_depth: usize,
}

impl Tree<'_> {
    fn shape(&self, id: DeviceId, depth: usize) -> Shape {
        let mut shape = Shape {
            left: 0.5,
            right: 0.5,
            rows: 1,
        };
        if depth >= self.max_depth {
            return shape;
        }
        for tier in self.arrangement.tiers(self.network, id) {
            let (left, left_rows) = self.side_shape(&tier.left, depth + 1);
            let (right, right_rows) = self.side_shape(&tier.right, depth + 1);
            shape.left = shape.left.max(left);
            shape.right = shape.right.max(right);
            shape.rows += left_rows.max(right_rows);
        }
        shape
    }

    /// The width of devices side by side, and the rows the deepest of them takes.
    fn side_shape(&self, ids: &[DeviceId], depth: usize) -> (f32, usize) {
        let shapes = ids.iter().map(|&id| self.shape(id, depth));
        shapes.fold((0.0, 0), |(width, rows), s| {
            (width + s.left + s.right, rows.max(s.rows))
        })
    }

    /// Places `id` at `spot`, and its tiers below, each below the deepest branch of the one above.
    fn place(&self, id: DeviceId, spot: Spot, out: &mut Positions) {
        let at = Point {
            x: spot.trunk * COLUMN_WIDTH,
            y: units(spot.row) * ROW_HEIGHT,
        };
        out.insert(id, at);
        if spot.depth >= self.max_depth {
            return;
        }
        let mut tier = Spot {
            row: spot.row + 1,
            depth: spot.depth + 1,
            ..spot
        };
        for devices in self.arrangement.tiers(self.network, id) {
            let left = self.place_side(Side::Left, &devices.left, tier, out);
            let right = self.place_side(Side::Right, &devices.right, tier, out);
            tier.row += left.max(right);
        }
    }

    /// Places one side of a tier, side by side going out from the trunk at `tier`, and returns
    /// the rows the deepest of them takes.
    fn place_side(&self, side: Side, ids: &[DeviceId], tier: Spot, out: &mut Positions) -> usize {
        let (outward, direction): (Vec<DeviceId>, f32) = match side {
            Side::Left => (ids.iter().rev().copied().collect(), -1.0),
            Side::Right => (ids.to_vec(), 1.0),
        };
        let mut cursor = tier.trunk;
        let mut rows = 0;
        for id in outward {
            let shape = self.shape(id, tier.depth);
            let (near, far) = match side {
                Side::Left => (shape.right, shape.left),
                Side::Right => (shape.left, shape.right),
            };
            let trunk = cursor + direction * near;
            self.place(id, Spot { trunk, ..tier }, out);
            cursor += direction * (near + far);
            rows = rows.max(shape.rows);
        }
        rows
    }
}

/// Where a device goes: the column of its trunk, its row, and how deep in the tree it is.
#[derive(Debug, Clone, Copy)]
struct Spot {
    trunk: f32,
    row: usize,
    depth: usize,
}

/// Converts a small count of rows to map units without a lossy cast.
fn units(count: usize) -> f32 {
    u16::try_from(count).map_or(f32::from(u16::MAX), f32::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::arrangement::{Level, Slot};

    const ROUTER: DeviceId = DeviceId(1);
    const SWITCH: DeviceId = DeviceId(2);
    const PC: DeviceId = DeviceId(5);

    fn layout(network: &Network) -> Positions {
        tree_layout(network, &Arrangement::default())
    }

    #[test]
    fn every_device_gets_its_own_spot() {
        let network = mock_data::network();
        let positions = layout(&network);
        assert_eq!(positions.len(), network.devices().len());
        let spots: std::collections::HashSet<_> = positions
            .values()
            .map(|p| (p.x.to_bits(), p.y.to_bits()))
            .collect();
        assert_eq!(spots.len(), positions.len());
    }

    #[test]
    fn downstream_devices_sit_below_their_uplink() {
        let network = mock_data::network();
        let positions = layout(&network);
        for c in network.connections() {
            assert!(
                positions[&c.downstream].y > positions[&c.upstream].y,
                "{c:?}"
            );
        }
    }

    #[test]
    fn devices_hang_either_side_of_the_trunk_in_one_row() {
        let network = mock_data::network();
        let positions = layout(&network);
        let tiers = Arrangement::default().tiers(&network, SWITCH);
        let switch = positions[&SWITCH];
        let tier = tiers.first().unwrap();
        assert!(tier.left.iter().all(|d| positions[d].x < switch.x));
        assert!(tier.right.iter().all(|d| positions[d].x > switch.x));
        let ys: std::collections::HashSet<_> =
            tier.devices().map(|d| positions[&d].y.to_bits()).collect();
        assert_eq!(ys.len(), 1, "one row");
    }

    #[test]
    fn a_lower_tier_hangs_below_the_branches_of_the_one_above() {
        let mut network = mock_data::network();
        let mut arrangement = Arrangement::default();
        let below = Slot {
            level: Level::NewTier(1),
            side: Side::Left,
            before: None,
        };
        network.connect(PC, Some(ROUTER));
        arrangement.place(&network, PC, ROUTER, below);
        let positions = tree_layout(&network, &arrangement);
        // The switch's devices are a row below it, so the PC goes a row below them.
        let switch_devices = network.behind(SWITCH).map(|d| positions[&d].y);
        let lowest = switch_devices.fold(f32::MIN, f32::max);
        assert!(positions[&PC].y > lowest);
        assert!(positions[&PC].x < positions[&ROUTER].x, "on the left");
    }

    #[test]
    fn a_trunk_reaches_below_its_whole_branch() {
        let network = mock_data::network();
        let positions = layout(&network);
        let [top, root] = trunk(&network, &positions, ROUTER).unwrap();
        let lowest = positions.values().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(top.y < root.y && root.y > lowest);
        let [_, leaf_root] = trunk(&network, &positions, PC).unwrap();
        assert!(
            leaf_root.y > positions[&PC].y,
            "a device with none behind it has one too"
        );
    }

    #[test]
    fn a_route_runs_across_just_above_a_device_then_down() {
        let [a, b, c] = route(0.0, Point { x: 300.0, y: 140.0 });
        assert_eq!((a.x, a.y, b.x, c.x), (0.0, b.y, 300.0, 300.0));
        assert!(b.y < c.y && c.y < 140.0);
    }
}
