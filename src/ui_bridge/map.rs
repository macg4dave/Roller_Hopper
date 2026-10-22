//! The topology map: where each network card is laid out and shown, the lines between them, and
//! the motion that carries cards to their spots. Dragging a card is in [`drag`], drawing the
//! lines in [`lines`] and the springy motion in [`motion`].

mod drag;
mod lines;
mod motion;

pub(super) use drag::payload_child;
pub(super) use motion::Motion;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel, Weak};

use super::view::sync;
use super::{AppState, Selection, convert, handler, update};
use crate::model::{
    CardId, Connection, Level, Point, Positions, Snap, link_route, tree_layout, trunk,
};
use crate::ui::{self, AppWindow, Bridge};

/// One frame of the map's motion, at about 60 frames a second.
const FRAME: Duration = Duration::from_millis(16);

impl AppState {
    /// The cards lit up on the map: the selected card, or every card of the selected device.
    fn selected_cards(&self) -> Vec<CardId> {
        match self.selected {
            Some(Selection::Card(card)) => vec![card],
            Some(Selection::Device(id)) => {
                let cards = self
                    .network
                    .device(id)
                    .map(|d| d.cards.iter().map(|c| c.id));
                cards.into_iter().flatten().collect()
            }
            None => Vec::new(),
        }
    }

    /// While a device is selected, the dotted ties between its cards as shown on the map.
    fn ties(&self) -> Vec<[Point; 2]> {
        let Some(Selection::Device(id)) = self.selected else {
            return Vec::new();
        };
        let device = self.network.device(id);
        device.map_or_else(Vec::new, |d| lines::ties(d, self.motion.shown()))
    }

    /// The trunk of every node as shown. A dragged branch's trunks are hidden until it is
    /// dropped, and the other trunks do not stretch after it.
    /// In node order, so the lines sent to the UI only change when the map does.
    fn trunks(&self) -> Vec<(CardId, [Point; 2])> {
        let shown = self.motion.shown();
        let dragged = |id| self.dragging.is_some_and(|d| self.network.is_within(id, d));
        let still: Positions = shown
            .iter()
            .filter(|&(&id, _)| !dragged(id))
            .map(|(&id, &p)| (id, p))
            .collect();
        let ids = self.network.nodes().into_iter();
        ids.filter_map(|id| Some((id, trunk(&self.network, &still, id)?)))
            .collect()
    }

    /// The connections to draw: all but the dragged card's uplink, which [`Self::strand`]
    /// stands in for.
    /// Each runs along its route as shown, branching off its neighbour's line, with the dragged
    /// device left out of the rows it was in.
    fn links(&self) -> Vec<(Connection, [Point; 3])> {
        let shown = self.motion.shown();
        let inner = self
            .arrangement
            .inner_neighbours(&self.network, self.dragging);
        let links = self.network.connections().into_iter();
        links
            .filter(|c| Some(c.downstream) != self.dragging)
            .filter_map(|c| Some((c, link_route(shown, &inner, c.upstream, c.downstream)?)))
            .collect()
    }

    fn lay_out(&mut self) {
        self.positions = tree_layout(&self.network, &self.arrangement);
    }

    /// Lays the map out again at rest: it is out of sight on the edit page, where the network
    /// is edited.
    pub(super) fn lay_out_at_rest(&mut self) {
        self.lay_out();
        self.motion = Motion::new(self.positions.clone());
    }

    /// Moves the map one frame on; true once everything has settled.
    fn step_motion(&mut self) -> bool {
        let dt = FRAME.as_secs_f32();
        let strand = self.strand();
        let targets = self.making_room(strand.as_ref());
        self.motion.step(&targets, dt);
        self.motion.step_strand(strand.as_ref(), dt);
        self.motion.settled(&targets)
    }

    /// Where the motion takes each device: its spot in the layout, except that while a dragged
    /// device is over a row it would join, the devices in that row move aside to make room
    /// for it, each with what hangs behind it. Only the motion sees this; dropping lays out anew.
    fn making_room(&self, snap: Option<&Snap>) -> Positions {
        let mut targets = self.positions.clone();
        let (Some(dragged), Some(snap)) = (self.dragging, snap) else {
            return targets;
        };
        let (Level::Tier(index), Some(&pointer)) = (snap.slot.level, self.positions.get(&dragged))
        else {
            return targets;
        };
        let tiers = self
            .arrangement
            .tiers_without(&self.network, snap.upstream, dragged);
        let Some(row) = tiers.get(index) else {
            return targets;
        };
        // The row's devices on each side of the pointer, nearest first.
        let mut beside: Vec<(CardId, f32)> = row
            .devices()
            .filter_map(|id| Some((id, self.positions.get(&id)?.x - pointer.x)))
            .collect();
        beside.sort_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
        let (left, right): (Vec<_>, Vec<_>) = beside.into_iter().partition(|&(_, dx)| dx < 0.0);
        let away = |side: Vec<(CardId, f32)>, sign: f32| {
            let parted = side.into_iter().zip(motion::PARTING);
            parted.map(move |((id, _), shift)| (id, sign * shift))
        };
        for (neighbour, shift) in away(left, -1.0).chain(away(right, 1.0)) {
            let branch = targets.iter_mut().filter(|&(&id, _)| {
                self.network.is_within(id, neighbour) && !self.network.is_within(id, dragged)
            });
            for (_, at) in branch {
                at.x += shift;
            }
        }
        targets
    }
}

/// Dragging (on the map or from the lists), dropping and cancelling, each followed
/// by the motion that settles it.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    // The style previews never change, so they are sent once.
    bridge.set_line_styles(ModelRc::new(VecModel::from(lines::samples())));

    let timer = Rc::new(Timer::default());
    let settle = {
        let (ui, state) = (ui.as_weak(), Rc::clone(state));
        Rc::new(move || animate(&ui, &state, &timer))
    };

    // A card is dragged the same way on the map and from the lists. While dragging, the strand
    // swings after the pointer.
    let move_card = handler(ui, state, |st, (id, x, y)| {
        if let Some(card) = convert::card_id(id) {
            st.drag_to(card, Point { x, y });
        }
    });
    let move_settle = Rc::clone(&settle);
    bridge.on_move_card(move |id, x, y| {
        move_card((id, x, y));
        move_settle();
    });
    let drop_card = handler(ui, state, |st, ()| st.drop_dragged());
    let drop_settle = Rc::clone(&settle);
    bridge.on_drop_card(move || {
        drop_card(());
        drop_settle();
    });
    let cancel = handler(ui, state, |st, ()| st.cancel_drag());
    bridge.on_cancel_drag(move || {
        cancel(());
        settle();
    });

    // A card dragged out of the lists carries its id as text, as Slint's drag payload is
    // opaque to Slint code. The prefix keeps text dragged in from other apps from passing for
    // a card.
    bridge.on_card_transfer(|id, child| SharedString::from(drag::card_payload(id, child)).into());
    bridge.on_transfer_card(|data| {
        let text = data.plain_text().unwrap_or_default();
        drag::payload_card(&text).unwrap_or(-1)
    });
}

/// Steps the map's motion every frame until everything has settled (or the window closed).
fn animate(ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>, timer: &Rc<Timer>) {
    if timer.running() {
        return;
    }
    let (ui, state, this) = (ui.clone(), Rc::clone(state), Rc::downgrade(timer));
    timer.start(TimerMode::Repeated, FRAME, move || {
        let mut settled = true;
        update(&ui, &state, |st| settled = st.step_motion());
        if let Some(running) = this.upgrade().filter(|_| settled) {
            running.stop();
        }
    });
}

/// Where each card is drawn right now, the lines between them, and the area they take up.
pub(super) fn push(bridge: &Bridge<'_>, state: &AppState) {
    let on_map = state.motion.shown();
    let trunks = state.trunks();
    let places = state.network.nodes().into_iter().map(|node| {
        let at = on_map.get(&node).copied().unwrap_or_default();
        let root = trunks
            .iter()
            .find(|(id, _)| *id == node)
            .map(|(_, [_, root])| root);
        ui::NodePlace {
            x: at.x,
            y: at.y,
            has_root: root.is_some(),
            root_y: root.map_or(0.0, |r| r.y),
        }
    });
    if let Some(model) = sync(&bridge.get_places(), places.collect()) {
        bridge.set_places(model);
    }

    let lit = state
        .dragging
        .map_or_else(|| state.selected_cards(), |card| vec![card]);
    let lines = lines::lines(&state.network, &state.links(), &trunks, &lit, &state.ties());
    bridge.set_strand(convert::strand(
        state.strand().as_ref(),
        state.motion.bend(),
    ));
    if let Some(model) = sync(&bridge.get_lines(), lines) {
        bridge.set_lines(model);
    }
    bridge.set_bounds(convert::bounds(on_map.values().copied()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::DeviceId;

    #[test]
    fn a_row_makes_room_for_a_device_dragged_over_it_and_closes_after() {
        let mut state = AppState::new(mock_data::network());
        let (switch, access_point, nas, pc) = (CardId(2), CardId(3), CardId(4), CardId(5));
        let laid_out = state.positions.clone();
        let between = Point {
            x: f32::midpoint(laid_out[&switch].x, laid_out[&access_point].x),
            y: laid_out[&switch].y,
        };
        state.drag_to(nas, between);
        for _ in 0..60 {
            state.step_motion();
        }
        let moved = |st: &AppState, id| st.motion.shown()[&id].x - laid_out[&id].x;
        assert!(moved(&state, switch) < -1.0, "the switch moves left");
        assert!(moved(&state, access_point) > 1.0, "the access point right");
        assert!(
            (moved(&state, pc) - moved(&state, switch)).abs() < 0.5,
            "with what hangs behind it"
        );
        assert_eq!(
            state.motion.shown()[&nas],
            between,
            "the dragged one stays put"
        );

        // Far from every line, the row closes up again.
        state.drag_to(
            nas,
            Point {
                x: -5000.0,
                y: -5000.0,
            },
        );
        assert!((0..600).any(|_| state.step_motion()));
        assert!(moved(&state, switch).abs() < f32::EPSILON);
    }

    #[test]
    fn a_selected_card_lights_up_alone_and_a_selected_device_ties_its_cards() {
        let mut state = AppState::new(mock_data::network());
        state.selected = Some(Selection::Card(CardId(101)));
        assert_eq!(state.selected_device(), Some(DeviceId(1)));
        assert_eq!(state.selected_cards(), [CardId(101)]);
        assert_eq!(state.ties(), [] as [[Point; 2]; 0]);
        state.edit_selected(|d| d.name = "Gateway".into());
        assert_eq!(
            state.network.device(DeviceId(1)).map(|d| d.name.as_str()),
            Some("Gateway"),
            "edits go to its device"
        );
        state.selected = Some(Selection::Device(DeviceId(1)));
        assert_eq!(state.selected_cards(), [CardId(1), CardId(101)]);
        assert_eq!(state.ties().len(), 1, "LAN to WAN");
        state.selected = Some(Selection::Device(DeviceId(5)));
        assert!(state.ties().is_empty(), "the PC has one card");
    }
}
