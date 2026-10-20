//! Automatic tree layout of the topology and the routing of its connection lines.
//!
//! Positions are in map units (1 unit = 1 logical pixel at 100 % zoom) and mark the centre of a
//! node's icon; its labels sit below.

use std::collections::HashMap;

use super::device::DeviceId;
use super::network::Network;

const COLUMN_WIDTH: f32 = 150.0;
const ROW_HEIGHT: f32 = 150.0;
/// Leaf devices under one parent wrap onto a new row after this many. Kept even so the line
/// down to later rows runs between nodes, and small so the map stays close to square.
const MAX_COLUMNS: usize = 2;
/// Lines stop short of the icon above a node and leave below the labels of its parent.
const LINE_TOP_GAP: f32 = 28.0;
const LINE_BOTTOM_GAP: f32 = 66.0;
/// How near a line a dragged device's own line must end for it to join that line, above or
/// below it. Rows are close together, so this stays small.
const SNAP_DISTANCE: f32 = 48.0;
/// The same reach to either side, larger as columns are far apart: a device brought up beside
/// a vertical line joins it well before it covers it.
const SNAP_REACH_SIDEWAYS: f32 = 110.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Point {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

pub(crate) type Positions = HashMap<DeviceId, Point>;

/// Lays the network out as a top-down tree, starting from devices with no uplink.
pub(crate) fn tree_layout(network: &Network) -> Positions {
    let tree = Tree {
        network,
        max_depth: network.devices().len(),
    };
    let mut positions = Positions::new();
    let mut left = 0;
    for root in network
        .devices()
        .iter()
        .filter(|d| network.uplink(d.id).is_none())
    {
        tree.place(root.id, left, 0, &mut positions);
        left += tree.width(root.id, 0);
    }
    positions
}

/// An orthogonal route between two nodes: down from the upstream node, across, then down.
pub(crate) fn route(upstream: Point, downstream: Point) -> [Point; 4] {
    let start = Point {
        x: upstream.x,
        y: upstream.y + LINE_BOTTOM_GAP,
    };
    let end = Point {
        x: downstream.x,
        y: downstream.y - LINE_TOP_GAP,
    };
    let bus_y = end.y - (ROW_HEIGHT - LINE_TOP_GAP - LINE_BOTTOM_GAP) / 2.0;
    [
        start,
        Point {
            x: start.x,
            y: bus_y,
        },
        Point { x: end.x, y: bus_y },
        end,
    ]
}

/// Where a dragged device would join the map if dropped now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Snap {
    /// The device whose line it would join.
    pub(crate) upstream: DeviceId,
    /// The device on that line it would go just before, as it was dropped between it and the one
    /// before; `None` to go last.
    pub(crate) before: Option<DeviceId>,
    /// A short line showing the join: from the nearest point of the line it would join to the
    /// top of its icon.
    pub(crate) line: [Point; 2],
}

/// Where `dragged` would join if dropped with its icon centred on `at`: the nearest of the lines
/// a device has, or of the spots under a device for a first line, at the place among that
/// line's devices where it was dropped. Never the dragged device or one behind it, which would
/// form a loop. `None` when no line is near.
pub(crate) fn snap_target(
    network: &Network,
    positions: &Positions,
    dragged: DeviceId,
    at: Point,
) -> Option<Snap> {
    // Where the dragged device's own line would end.
    let end = Point {
        x: at.x,
        y: at.y - LINE_TOP_GAP,
    };
    let candidates = network.devices().iter().map(|d| d.id);
    candidates
        .filter(|&id| !network.is_within(id, dragged))
        .filter_map(|id| {
            let origin = positions.get(&id).copied()?;
            let first_line = Point {
                x: origin.x,
                y: origin.y + LINE_BOTTOM_GAP,
            };
            let segments = others_behind(network, id, dragged)
                .filter_map(|other| positions.get(&other))
                .flat_map(|&p| {
                    let [a, b, c, d] = route(origin, p);
                    [(a, b), (b, c), (c, d)]
                });
            let joins = segments
                .map(|(a, b)| nearest(end, a, b))
                .min_by(|a, b| distance(end, *a).total_cmp(&distance(end, *b)))
                .filter(|&p| distance(end, p) < distance(end, first_line))
                .unwrap_or(first_line);
            Some((id, joins))
        })
        .filter(|&(_, joins)| distance(end, joins) <= SNAP_DISTANCE)
        .min_by(|a, b| distance(end, a.1).total_cmp(&distance(end, b.1)))
        .map(|(upstream, joins)| Snap {
            upstream,
            // They are laid out in reading order, so the first one after `at` follows it.
            before: others_behind(network, upstream, dragged)
                .find(|other| positions.get(other).is_some_and(|&p| reads_after(p, at))),
            line: [joins, end],
        })
}

/// The devices connected straight behind `upstream`, in layout order, other than `dragged`.
fn others_behind(
    network: &Network,
    upstream: DeviceId,
    dragged: DeviceId,
) -> impl Iterator<Item = DeviceId> {
    let links = network.connections().iter();
    links
        .filter(move |c| c.upstream == upstream && c.downstream != dragged)
        .map(|c| c.downstream)
}

/// Whether a node at `p` comes after `at` reading the map by rows, top to bottom, left to right.
fn reads_after(p: Point, at: Point) -> bool {
    let rows_below = p.y - at.y;
    let half_row = ROW_HEIGHT / 2.0;
    rows_below > half_row || (rows_below.abs() <= half_row && p.x > at.x)
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

struct Tree<'a> {
    network: &'a Network,
    /// Guards against cycles in the connection data: no real tree is deeper than its size.
    max_depth: usize,
}

impl Tree<'_> {
    fn children(&self, id: DeviceId) -> Vec<DeviceId> {
        let connections = self.network.connections().iter();
        connections
            .filter(|c| c.upstream == id)
            .map(|c| c.downstream)
            .collect()
    }

    fn is_leaf(&self, id: DeviceId) -> bool {
        self.children(id).is_empty()
    }

    /// Width of the subtree in columns.
    fn width(&self, id: DeviceId, depth: usize) -> usize {
        let children = self.children(id);
        if depth >= self.max_depth || children.is_empty() {
            1
        } else if children.iter().all(|&c| self.is_leaf(c)) {
            children.len().min(MAX_COLUMNS)
        } else {
            children.iter().map(|&c| self.width(c, depth + 1)).sum()
        }
    }

    fn place(&self, id: DeviceId, left: usize, depth: usize, out: &mut Positions) {
        let width = self.width(id, depth);
        let x = column_centre(units(left) + units(width) / 2.0 - 0.5);
        out.insert(
            id,
            Point {
                x,
                y: row_top(depth),
            },
        );
        if depth >= self.max_depth {
            return;
        }
        let children = self.children(id);
        if children.iter().all(|&c| self.is_leaf(c)) {
            for (i, &child) in children.iter().enumerate() {
                let (row, column) = (i / MAX_COLUMNS, i % MAX_COLUMNS);
                let in_row = (children.len() - row * MAX_COLUMNS).min(MAX_COLUMNS);
                let centring = units(width.saturating_sub(in_row)) / 2.0;
                let child_x = column_centre(units(left) + centring + units(column));
                out.insert(
                    child,
                    Point {
                        x: child_x,
                        y: row_top(depth + 1 + row),
                    },
                );
            }
        } else {
            let mut cursor = left;
            for child in children {
                self.place(child, cursor, depth + 1, out);
                cursor += self.width(child, depth + 1);
            }
        }
    }
}

fn column_centre(column: f32) -> f32 {
    (column + 0.5) * COLUMN_WIDTH
}

fn row_top(row: usize) -> f32 {
    units(row) * ROW_HEIGHT
}

/// Converts a small count of columns or rows to map units without a lossy cast.
fn units(count: usize) -> f32 {
    u16::try_from(count).map_or(f32::from(u16::MAX), f32::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::{Connection, ConnectionKind};

    #[test]
    fn every_device_gets_its_own_spot() {
        let network = mock_data::network();
        let positions = tree_layout(&network);
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
        let positions = tree_layout(&network);
        for c in network.connections() {
            assert!(
                positions[&c.downstream].y > positions[&c.upstream].y,
                "{c:?}"
            );
        }
    }

    #[test]
    fn a_dragged_device_joins_the_nearest_line() {
        let network = mock_data::network();
        let positions = tree_layout(&network);
        let (router, switch, access_point, pc) =
            (DeviceId(1), DeviceId(2), DeviceId(3), DeviceId(5));
        // Dropped between the switch and the access point, its line ends on the router's line.
        let between = Point {
            x: f32::midpoint(positions[&switch].x, positions[&access_point].x),
            y: positions[&switch].y,
        };
        let target = snap_target(&network, &positions, pc, between).unwrap();
        assert_eq!(
            (target.upstream, target.before),
            (router, Some(access_point))
        );
        // It joins the line straight above it, not back at the router.
        let [joins, end] = target.line;
        assert_eq!((joins.x, end.x), (between.x, between.x));
        assert!(joins.y < end.y && end.y < between.y);
        let far = Point {
            x: -1000.0,
            y: -1000.0,
        };
        assert_eq!(snap_target(&network, &positions, pc, far), None);
    }

    #[test]
    fn a_device_with_no_line_yet_offers_one_below_it() {
        let network = mock_data::network();
        let positions = tree_layout(&network);
        let (pc, leaf) = (DeviceId(5), DeviceId(4));
        assert!(!network.connections().iter().any(|c| c.upstream == leaf));
        let below = Point {
            x: positions[&leaf].x,
            y: positions[&leaf].y + LINE_BOTTOM_GAP + LINE_TOP_GAP,
        };
        let target = snap_target(&network, &positions, pc, below).unwrap();
        assert_eq!((target.upstream, target.before), (leaf, None));
        let spot = Point {
            x: below.x,
            y: positions[&leaf].y + LINE_BOTTOM_GAP,
        };
        assert_eq!(target.line, [spot, spot]);
    }

    #[test]
    fn a_dragged_device_never_joins_its_own_branch() {
        let network = mock_data::network();
        let positions = tree_layout(&network);
        let switch = DeviceId(2);
        for c in network
            .connections()
            .iter()
            .filter(|c| c.upstream == switch)
        {
            let target = snap_target(&network, &positions, switch, positions[&c.downstream]);
            assert!(
                !target.is_some_and(|t| network.is_within(t.upstream, switch)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn a_line_reaches_further_to_the_side_than_up_or_down() {
        // A device with one other stacked below it, and a third being dragged.
        let (top, below, dragged) = (DeviceId(1), DeviceId(2), DeviceId(3));
        let devices = mock_data::network().devices().get(..3).unwrap().to_vec();
        let link = Connection {
            upstream: top,
            downstream: below,
            kind: ConnectionKind::Known,
        };
        let network = Network::new(devices, vec![link]);
        let mut positions = tree_layout(&network);
        positions.remove(&dragged);
        let line_x = positions[&below].x;
        assert!((positions[&top].x - line_x).abs() < f32::EPSILON, "stacked");

        let beside = Point {
            x: line_x + 100.0,
            y: positions[&below].y,
        };
        let joined = snap_target(&network, &positions, dragged, beside);
        assert_eq!(joined.map(|s| s.upstream), Some(top));
        let as_far_above = Point {
            x: line_x,
            y: positions[&top].y - 100.0,
        };
        assert_eq!(
            snap_target(&network, &positions, dragged, as_far_above),
            None
        );
    }

    #[test]
    fn a_device_dropped_in_a_wrapped_row_goes_in_reading_order() {
        let network = mock_data::network();
        let positions = tree_layout(&network);
        let (switch, pc) = (DeviceId(2), DeviceId(5));
        let others: Vec<_> = others_behind(&network, switch, pc).collect();
        // Just left of the third device on the line, which starts a new row.
        let third = others[2];
        let at = Point {
            x: positions[&third].x - 10.0,
            y: positions[&third].y,
        };
        let target = snap_target(&network, &positions, pc, at);
        assert_eq!(target.map(|t| t.before), Some(Some(third)));
    }

    #[test]
    fn route_is_orthogonal_and_joins_both_nodes() {
        let [a, b, c, d] = route(Point { x: 0.0, y: 0.0 }, Point { x: 300.0, y: 140.0 });
        assert_eq!((a.x, b.x, c.y, d.x), (0.0, 0.0, b.y, 300.0));
        assert!(a.y < b.y && c.y < d.y);
    }
}
