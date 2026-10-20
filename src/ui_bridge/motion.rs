//! The map's springy motion: devices glide to where the layout puts them, overshoot a little and
//! settle, like a goo structure taking a new piece; the strand to a dragged device swings and
//! wobbles. Purely visual; the layout itself never moves.

use std::collections::HashMap;

use crate::model::{Node, Point, Positions, Snap};

/// Spring stiffness (per second²) and damping (per second): devices settle in about a third of
/// a second, overshooting by under a tenth of the way, so the map feels alive but stays calm.
const DEVICE_SPRING: Spring = Spring {
    stiffness: 360.0,
    damping: 24.0,
};
/// The strand's bend is softer and livelier, so it trails the pointer and wobbles like goo.
const STRAND_SPRING: Spring = Spring {
    stiffness: 220.0,
    damping: 13.0,
};
/// How hard the devices on a line are pushed down when another joins it, in map units per
/// second; they dip a few units and bob back.
const JIGGLE: f32 = 150.0;
/// How far a slack strand sags under its own weight, as a share of its length.
const SAG: f32 = 0.3;
/// How fast a strand that has just caught flicks down into its sag, in map units per second.
const FLICK: f32 = 260.0;
/// Nearer than this to its spot, and slower than this per second, a thing has settled.
const REST: f32 = 0.1;
/// How far the devices on each side of a dragged device move aside to make room for it, nearest
/// first, in map units; those further along stay put. Like icons in a dock, by order rather than
/// distance, as devices with wide branches stand far apart.
pub(super) const PARTING: [f32; 3] = [36.0, 18.0, 8.0];

#[derive(Debug, Clone, Copy)]
struct Spring {
    stiffness: f32,
    damping: f32,
}

/// Where each device is shown, and how fast it is moving there; likewise the strand's bend.
#[derive(Debug, Default)]
pub(super) struct Motion {
    shown: Positions,
    velocity: HashMap<Node, Point>,
    /// The strand's bend point and its velocity, while there is a strand.
    bend: Option<(Point, Point)>,
}

impl Motion {
    /// Everything shown at rest at `at`.
    pub(super) fn new(at: Positions) -> Self {
        Self {
            shown: at,
            ..Self::default()
        }
    }

    /// Where the strand bends, if there is one and it has started to swing.
    pub(super) fn bend(&self) -> Option<Point> {
        self.bend.map(|(at, _)| at)
    }

    /// Swings the strand's bend `dt` seconds on towards where its sag puts it. A strand that has
    /// just caught starts straight and flicks down; with no strand there is no bend.
    pub(super) fn step_strand(&mut self, strand: Option<&Snap>, dt: f32) {
        let Some(snap) = strand else {
            self.bend = None;
            return;
        };
        let [from, to] = snap.line;
        let straight = Point {
            x: f32::midpoint(from.x, to.x),
            y: f32::midpoint(from.y, to.y),
        };
        let sag = Point {
            y: straight.y + (to.x - from.x).hypot(to.y - from.y) * SAG * (1.0 - snap.pull),
            ..straight
        };
        let (at, velocity) = self
            .bend
            .get_or_insert((straight, Point { x: 0.0, y: FLICK }));
        if spring(at, velocity, sag, STRAND_SPRING, dt) {
            *velocity = Point::default();
        }
    }

    pub(super) fn shown(&self) -> &Positions {
        &self.shown
    }

    /// Puts a device straight at `at`, so a dragged device follows the pointer exactly.
    pub(super) fn place(&mut self, id: Node, at: Point) {
        self.shown.insert(id, at);
        self.velocity.remove(&id);
    }

    /// Nudges the devices down, so they bob as if taking a weight.
    pub(super) fn jiggle(&mut self, ids: impl IntoIterator<Item = Node>) {
        for id in ids {
            self.velocity.entry(id).or_default().y += JIGGLE;
        }
    }

    /// Whether every device is shown at rest at its spot in `targets`, and the strand, if any,
    /// has stopped swinging.
    pub(super) fn settled(&self, targets: &Positions) -> bool {
        let strand_still = self
            .bend
            .is_none_or(|(_, velocity)| velocity == Point::default());
        strand_still && self.velocity.is_empty() && self.shown == *targets
    }

    /// Moves every device `dt` seconds on towards its spot in `targets`.
    pub(super) fn step(&mut self, targets: &Positions, dt: f32) {
        self.shown.retain(|id, _| targets.contains_key(id));
        for (&id, &target) in targets {
            let shown = self.shown.entry(id).or_insert(target);
            let velocity = self.velocity.entry(id).or_default();
            if spring(shown, velocity, target, DEVICE_SPRING, dt) {
                self.velocity.remove(&id);
            }
        }
    }
}

/// Moves `at` with `velocity` `dt` seconds on towards `goal`. Once near and slow enough it is put
/// exactly at `goal` and true is returned: it has settled.
fn spring(at: &mut Point, velocity: &mut Point, goal: Point, spring: Spring, dt: f32) -> bool {
    for (position, speed, target) in [
        (&mut at.x, &mut velocity.x, goal.x),
        (&mut at.y, &mut velocity.y, goal.y),
    ] {
        // Semi-implicit Euler: stable for these springs at any frame rate the UI runs at.
        *speed += (spring.stiffness * (target - *position) - spring.damping * *speed) * dt;
        *position += *speed * dt;
    }
    let near = (at.x - goal.x).hypot(at.y - goal.y) < REST;
    let settled = near && velocity.x.hypot(velocity.y) < REST;
    if settled {
        *at = goal;
    }
    settled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DeviceId;

    const FRAME: f32 = 1.0 / 60.0;
    const ID: Node = Node::Device(DeviceId(1));

    fn at(x: f32, y: f32) -> Positions {
        Positions::from([(ID, Point { x, y })])
    }

    /// Steps until settled, returning how many frames that took and the furthest x reached.
    fn run(motion: &mut Motion, targets: &Positions) -> (usize, f32) {
        let mut furthest = f32::MIN;
        for frame in 0..600 {
            if motion.settled(targets) {
                return (frame, furthest);
            }
            motion.step(targets, FRAME);
            furthest = furthest.max(motion.shown()[&ID].x);
        }
        panic!("never settled");
    }

    #[test]
    fn a_device_glides_to_its_spot_overshooting_a_little() {
        let mut motion = Motion::new(at(0.0, 0.0));
        let target = at(100.0, 0.0);
        let (frames, furthest) = run(&mut motion, &target);
        assert!(frames < 60, "settles within a second: {frames} frames");
        assert!(
            furthest > 100.0 && furthest < 110.0,
            "a slight overshoot: {furthest}"
        );
        assert_eq!(motion.shown(), &target);
    }

    #[test]
    fn a_jiggled_device_dips_and_comes_back() {
        let rest = at(0.0, 0.0);
        let mut motion = Motion::new(rest.clone());
        motion.jiggle([ID]);
        assert!(!motion.settled(&rest));
        motion.step(&rest, FRAME);
        assert!(motion.shown()[&ID].y > 0.0, "pushed down");
        run(&mut motion, &rest);
        assert_eq!(motion.shown(), &rest);
    }

    #[test]
    fn a_placed_device_is_shown_exactly_there() {
        let mut motion = Motion::new(at(0.0, 0.0));
        let pointer = Point { x: 40.0, y: 25.0 };
        motion.place(ID, pointer);
        let targets = Positions::from([(ID, pointer)]);
        assert!(motion.settled(&targets));
    }

    /// A slack strand 100 units across, as the snapper would give.
    fn strand() -> Snap {
        let network = crate::mock_data::network();
        let arrangement = crate::model::Arrangement::default();
        let positions = crate::model::tree_layout(&network, &arrangement);
        let (pc, nas) = (Node::Device(DeviceId(5)), Node::Device(DeviceId(4)));
        let any =
            crate::model::snap_target(&network, &arrangement, &positions, pc, positions[&nas]);
        Snap {
            line: [Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }],
            pull: 0.0,
            ..any.unwrap()
        }
    }

    #[test]
    fn a_strand_flicks_down_wobbles_and_hangs_in_its_sag() {
        let (snap, rest) = (strand(), Positions::new());
        let mut motion = Motion::default();
        motion.step_strand(Some(&snap), FRAME);
        assert!(!motion.settled(&rest), "swinging");
        let mut deepest = f32::MIN;
        for _ in 0..300 {
            motion.step_strand(Some(&snap), FRAME);
            deepest = deepest.max(motion.bend().unwrap().y);
        }
        let sag = 100.0 * SAG;
        assert!(deepest > sag, "swings past its sag: {deepest}");
        assert!(motion.settled(&rest));
        assert_eq!(motion.bend(), Some(Point { x: 50.0, y: sag }));

        motion.step_strand(None, FRAME);
        assert_eq!(motion.bend(), None);
    }
}
