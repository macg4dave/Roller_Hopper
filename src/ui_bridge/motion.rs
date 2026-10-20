//! The map's springy motion: devices glide to where the layout puts them, overshoot a little and
//! settle, like a goo structure taking a new piece. Purely visual; the layout itself never moves.

use std::collections::HashMap;

use crate::model::{DeviceId, Point, Positions};

/// Spring stiffness (per second²) and damping (per second): devices settle in about a third of
/// a second, overshooting by under a tenth of the way, so the map feels alive but stays calm.
const STIFFNESS: f32 = 360.0;
const DAMPING: f32 = 24.0;
/// How hard the devices on a line are pushed down when another joins it, in map units per
/// second; they dip a few units and bob back.
const JIGGLE: f32 = 150.0;
/// Nearer than this to its spot, and slower than this per second, a device has settled.
const REST: f32 = 0.1;

/// Where each device is shown, and how fast it is moving there.
#[derive(Debug, Default)]
pub(super) struct Motion {
    shown: Positions,
    velocity: HashMap<DeviceId, Point>,
}

impl Motion {
    /// Everything shown at rest at `at`.
    pub(super) fn new(at: Positions) -> Self {
        Self {
            shown: at,
            velocity: HashMap::new(),
        }
    }

    pub(super) fn shown(&self) -> &Positions {
        &self.shown
    }

    /// Puts a device straight at `at`, so a dragged device follows the pointer exactly.
    pub(super) fn place(&mut self, id: DeviceId, at: Point) {
        self.shown.insert(id, at);
        self.velocity.remove(&id);
    }

    pub(super) fn forget(&mut self, id: DeviceId) {
        self.shown.remove(&id);
        self.velocity.remove(&id);
    }

    /// Nudges the devices down, so they bob as if taking a weight.
    pub(super) fn jiggle(&mut self, ids: impl IntoIterator<Item = DeviceId>) {
        for id in ids {
            self.velocity.entry(id).or_default().y += JIGGLE;
        }
    }

    /// Whether every device is shown at rest at its spot in `targets`.
    pub(super) fn settled(&self, targets: &Positions) -> bool {
        self.velocity.is_empty() && self.shown == *targets
    }

    /// Moves every device `dt` seconds on towards its spot in `targets`.
    pub(super) fn step(&mut self, targets: &Positions, dt: f32) {
        self.shown.retain(|id, _| targets.contains_key(id));
        for (&id, &target) in targets {
            let shown = self.shown.entry(id).or_insert(target);
            let velocity = self.velocity.entry(id).or_default();
            for (at, speed, goal) in [
                (&mut shown.x, &mut velocity.x, target.x),
                (&mut shown.y, &mut velocity.y, target.y),
            ] {
                // Semi-implicit Euler: stable for this spring at any frame rate the UI runs at.
                *speed += (STIFFNESS * (goal - *at) - DAMPING * *speed) * dt;
                *at += *speed * dt;
            }
            let near = (shown.x - target.x).hypot(shown.y - target.y) < REST;
            if near && velocity.x.hypot(velocity.y) < REST {
                *shown = target;
                self.velocity.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;
    const ID: DeviceId = DeviceId(1);

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
}
