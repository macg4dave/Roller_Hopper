//! Dragging a network card, on the map or out of the lists: it follows the pointer, previews
//! the line it would join, and on dropping is plugged in there, or unplugged away from every
//! line. A drag taken off the map again is cancelled.

use crate::model::{CardId, Point, Snap, SnapKind, snap_target};
use crate::ui_bridge::AppState;

/// What a network card dragged out of the lists carries, before its id.
const CARD_PAYLOAD: &str = "network-mapper-card:";
/// What a card's own row carries instead, so only it can be dropped on another device's group.
const CHILD_PAYLOAD: &str = "network-mapper-child:";

impl AppState {
    /// Drags card `id` so its icon is centred on `at`, on the map or from the lists. Ignored for
    /// a card that is gone.
    pub(super) fn drag_to(&mut self, id: CardId, at: Point) {
        if self.network.card(id).is_none() {
            return;
        }
        self.positions.insert(id, at);
        self.motion.place(id, at);
        self.dragging = Some(id);
    }

    /// Ends a drag without dropping, as when a card from the lists is taken off the map again:
    /// it glides back to its spot, plugged in where it was.
    pub(super) fn cancel_drag(&mut self) {
        if self.dragging.take().is_some() {
            self.lay_out();
        }
    }

    /// Where the dragged card `id` would go if dropped now, judged by the map as shown: on the
    /// trunk it is near, where it is on that trunk; `None` to stand alone.
    fn snap(&self, id: CardId) -> Option<Snap> {
        let shown = self.motion.shown();
        let at = shown.get(&id).copied()?;
        snap_target(&self.network, &self.arrangement, shown, id, at)
    }

    /// While a device is dragged near a line, how it would join it.
    pub(super) fn strand(&self) -> Option<Snap> {
        self.dragging.and_then(|id| self.snap(id))
    }

    /// Ends a drag: the device hangs from the trunk it was dropped on, where it was dropped, or
    /// stands alone, and the map is laid out again around the change. The devices on the trunk
    /// it joined bob as it settles in.
    pub(super) fn drop_dragged(&mut self) {
        let Some(id) = self.dragging.take() else {
            return;
        };
        let snap = self.snap(id);
        if let Some(snap) = snap.filter(|snap| snap.kind == SnapKind::Head) {
            self.network.connect_above(id, snap.upstream);
        } else {
            self.network.connect(id, snap.map(|snap| snap.upstream));
        }
        match snap {
            Some(snap) if snap.kind == SnapKind::Child => {
                let arrangement = &mut self.arrangement;
                arrangement.place(&self.network, id, snap.upstream, snap.slot);
                let others = self.network.behind(snap.upstream).filter(|&d| d != id);
                self.motion.jiggle(others.chain([snap.upstream]));
            }
            Some(_) | None => self.arrangement.remove(id),
        }
        self.lay_out();
    }
}

/// The drag payload of the card with UI id `id`, dragged by its own row (`child`) or its
/// device's.
pub(super) fn card_payload(id: i32, child: bool) -> String {
    let prefix = if child { CHILD_PAYLOAD } else { CARD_PAYLOAD };
    format!("{prefix}{id}")
}

/// The UI id of the card a drag payload carries, by either row; `None` for anything else.
pub(super) fn payload_card(text: &str) -> Option<i32> {
    let id = text.strip_prefix(CARD_PAYLOAD);
    id.or_else(|| text.strip_prefix(CHILD_PAYLOAD))?
        .parse()
        .ok()
}

/// The UI id of the card a drag payload carries, only when dragged by its own row.
pub(in crate::ui_bridge) fn payload_child(text: &str) -> Option<i32> {
    text.strip_prefix(CHILD_PAYLOAD)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::{tree_layout, trunk};

    /// The router's LAN card, which the home network hangs from.
    const ROUTER_LAN: CardId = CardId(1);

    #[test]
    fn a_virtual_devices_card_dragged_away_is_unplugged_like_any_other() {
        let mut state = AppState::new(mock_data::network());
        let docker_host = CardId(19);
        let far = Point {
            x: -1000.0,
            y: -1000.0,
        };
        state.drag_to(docker_host, far);
        state.drop_dragged();
        assert!(state.network.uplink(docker_host).is_none());
        assert_eq!(
            state.positions,
            tree_layout(&state.network, &state.arrangement)
        );
    }

    #[test]
    fn a_dropped_device_joins_the_line_it_was_previewing_on() {
        let mut state = AppState::new(mock_data::network());
        // The NAS hangs left of the router's LAN trunk; the switch and access point to its right.
        let (router, switch, access_point, nas) = (ROUTER_LAN, CardId(2), CardId(3), CardId(4));
        state.drop_dragged();
        assert_eq!(
            state.positions,
            tree_layout(&state.network, &state.arrangement),
            "no drag, no change"
        );

        // Dropped between the main switch and the access point.
        let x = |shown: &AppState, id| shown.positions[&id].x;
        let gap = |shown: &AppState| x(shown, access_point) - x(shown, switch);
        let gap_before = gap(&state);
        let between = Point {
            x: f32::midpoint(x(&state, switch), x(&state, access_point)),
            y: state.positions[&switch].y,
        };
        state.drag_to(nas, between);
        assert!(
            state.links().iter().all(|(c, _)| c.downstream != nas),
            "old line hidden"
        );
        assert!(state.strand().is_some());

        state.drop_dragged();
        assert_eq!(state.dragging, None);
        assert_eq!(state.network.uplink(nas).map(|c| c.upstream), Some(router));
        assert_eq!(
            state.positions,
            tree_layout(&state.network, &state.arrangement),
            "tidied again"
        );
        assert!(x(&state, switch) < x(&state, nas) && x(&state, nas) < x(&state, access_point));
        assert!(gap(&state) > gap_before, "pushed apart");

        assert!(!state.step_motion(super::super::FRAME), "glides into place");
        assert!(
            (0..600).any(|_| state.step_motion(super::super::FRAME)),
            "and settles"
        );
        assert_eq!(state.motion.shown(), &state.positions);
    }

    #[test]
    fn a_device_dropped_far_from_every_line_stands_alone() {
        let mut state = AppState::new(mock_data::network());
        let pc = CardId(5);
        state.drag_to(
            pc,
            Point {
                x: -1000.0,
                y: -1000.0,
            },
        );
        assert!(state.links().iter().all(|(c, _)| c.downstream != pc));
        assert_eq!(state.strand(), None);
        state.drop_dragged();
        assert!(state.network.uplink(pc).is_none());
    }

    #[test]
    fn a_device_dropped_above_its_tree_becomes_the_head() {
        let mut state = AppState::new(mock_data::network());
        let (router, switch, pc) = (CardId(1), CardId(2), CardId(5));
        let above_router = Point {
            x: state.positions[&router].x,
            y: state.positions[&router].y - 50.0,
        };
        state.drag_to(pc, above_router);
        assert!(
            state
                .strand()
                .is_some_and(|snap| snap.kind == SnapKind::Head)
        );

        state.drop_dragged();

        assert!(state.network.uplink(pc).is_none());
        assert_eq!(
            state.network.uplink(switch).map(|link| link.upstream),
            Some(pc)
        );
        assert_eq!(
            state.network.uplink(router).map(|link| link.upstream),
            Some(switch)
        );
        assert!(state.positions[&pc].y < state.positions[&switch].y);
        assert!(state.positions[&switch].y < state.positions[&router].y);
    }

    #[test]
    fn a_separate_device_dropped_above_a_root_connects_on_top() {
        let mut state = AppState::new(mock_data::network());
        let (router, lab_root) = (CardId(1), CardId(22));
        let above_router = Point {
            x: state.positions[&router].x + 120.0,
            y: state.positions[&router].y - 80.0,
        };
        state.drag_to(lab_root, above_router);
        assert!(
            state
                .strand()
                .is_some_and(|snap| snap.kind == SnapKind::Head && snap.upstream == router)
        );

        state.drop_dragged();

        assert!(state.network.uplink(lab_root).is_none());
        assert_eq!(
            state.network.uplink(router).map(|link| link.upstream),
            Some(lab_root)
        );
        assert!(state.positions[&lab_root].y < state.positions[&router].y);
    }

    #[test]
    fn a_new_top_device_makes_the_lower_device_its_child_instead_of_swapping() {
        let mut state = AppState::new(mock_data::network());
        let (router, switch, lab_root) = (CardId(1), CardId(2), CardId(22));
        let above_switch = Point {
            x: state.positions[&switch].x,
            y: state.positions[&switch].y - 70.0,
        };
        state.drag_to(lab_root, above_switch);
        assert!(
            state
                .strand()
                .is_some_and(|snap| { snap.kind == SnapKind::Head && snap.upstream == switch })
        );

        state.drop_dragged();

        assert_eq!(
            state.network.uplink(lab_root).map(|link| link.upstream),
            Some(router)
        );
        assert_eq!(
            state.network.uplink(switch).map(|link| link.upstream),
            Some(lab_root),
            "the device below is now the direct child"
        );
        assert!(state.positions[&router].y < state.positions[&lab_root].y);
        assert!(state.positions[&lab_root].y < state.positions[&switch].y);
    }

    #[test]
    fn a_device_dropped_on_a_trunk_below_a_row_hangs_below_it() {
        let mut state = AppState::new(mock_data::network());
        let (lab_root, nas) = (CardId(22), CardId(4));
        let lowest_of_root_row = |shown: &AppState| {
            let behind = shown.network.behind(lab_root).filter(|&d| d != nas);
            behind
                .map(|d| shown.positions[&d].y)
                .fold(f32::MIN, f32::max)
        };
        let row = lowest_of_root_row(&state);

        // Just left of the root's trunk, below its row.
        let below_left = Point {
            x: state.positions[&lab_root].x - 10.0,
            y: row + 90.0,
        };
        state.drag_to(nas, below_left);
        state.drop_dragged();
        assert_eq!(
            state.network.uplink(nas).map(|c| c.upstream),
            Some(lab_root)
        );
        assert!(state.positions[&nas].y > lowest_of_root_row(&state));
        assert!(state.positions[&nas].x < state.positions[&lab_root].x);
        assert!(
            state.trunks().iter().any(|(id, _)| *id == nas),
            "with a trunk of its own"
        );
    }

    #[test]
    fn a_card_dropped_on_a_non_root_trunk_becomes_its_child() {
        let mut state = AppState::new(mock_data::network());
        let (lab_card, lab_pi) = (CardId(100), CardId(23));
        let [_, root] = trunk(&state.network, &state.positions, lab_pi).unwrap();
        state.drag_to(
            lab_card,
            Point {
                y: root.y + 20.0,
                ..root
            },
        );
        state.drop_dragged();
        assert_eq!(
            state.network.uplink(lab_card).map(|c| c.upstream),
            Some(lab_pi),
            "the closest device remains valid even though it has a parent"
        );
    }

    #[test]
    fn a_virtual_device_held_over_a_line_previews_it_and_goes_back_if_taken_off() {
        let mut state = AppState::new(mock_data::network());
        // A container's first card has the container's id, like every sample device's.
        let (container, lab_root) = (CardId(24), CardId(22));
        let host = state.network.uplink(container).map(|c| c.upstream);
        let laid_out = state.positions.clone();
        let [_, root] = trunk(&state.network, &state.positions, lab_root).unwrap();
        let on_root = Point {
            y: root.y + 20.0,
            ..root
        };
        state.drag_to(container, on_root);
        assert_eq!(
            state.strand().map(|s| s.upstream),
            Some(lab_root),
            "previewed"
        );

        state.cancel_drag();
        assert_eq!(state.dragging, None);
        assert_eq!(state.network.uplink(container).map(|c| c.upstream), host);
        assert_eq!(state.positions, laid_out, "back in its spot");
        assert!(
            (0..600).any(|_| state.step_motion(super::super::FRAME)),
            "and glides there"
        );

        state.drag_to(container, on_root);
        state.drop_dragged();
        let plugged = state.network.uplink(container).map(|c| c.upstream);
        assert_eq!(plugged, Some(lab_root), "dropped where previewed");
    }

    #[test]
    fn only_a_card_that_exists_can_be_dragged_and_cancelling_nothing_changes_nothing() {
        let mut state = AppState::new(mock_data::network());
        state.drag_to(CardId(999), Point::default());
        assert_eq!(state.dragging, None);
        let laid_out = state.positions.clone();
        state.cancel_drag();
        assert_eq!(state.positions, laid_out);
    }

    #[test]
    fn a_drag_payload_carries_a_card_and_nothing_else() {
        assert_eq!(payload_card(&card_payload(42, false)), Some(42));
        assert_eq!(
            payload_card(&card_payload(42, true)),
            Some(42),
            "its own row"
        );
        assert_eq!(payload_child(&card_payload(42, true)), Some(42));
        assert_eq!(
            payload_child(&card_payload(42, false)),
            None,
            "its device's"
        );
        assert_eq!(payload_card("42"), None, "text from another app");
        assert_eq!(payload_card(&format!("{CARD_PAYLOAD}x")), None);
    }
}
