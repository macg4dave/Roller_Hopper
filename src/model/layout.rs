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
    fn route_is_orthogonal_and_joins_both_nodes() {
        let [a, b, c, d] = route(Point { x: 0.0, y: 0.0 }, Point { x: 300.0, y: 140.0 });
        assert_eq!((a.x, b.x, c.y, d.x), (0.0, 0.0, b.y, 300.0));
        assert!(a.y < b.y && c.y < d.y);
    }
}
