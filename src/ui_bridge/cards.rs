//! Editing what the selected device holds: its network cards, and the virtual machines and
//! containers it runs.

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

use super::motion::Motion;
use super::{AppState, convert, handler, update};
use crate::model::{DeviceId, GuestKind};
use crate::ui::{AppWindow, Bridge};

impl AppState {
    /// The devices a new card of the selected device can be plugged into, in dropdown order
    /// after "none": any outside its own branch, which would form a loop.
    pub(super) fn upstream_choices(&self) -> Vec<DeviceId> {
        let Some(selected) = self.selected else {
            return Vec::new();
        };
        let ids = self.network.devices().iter().map(|d| d.id);
        ids.filter(|&id| !self.network.is_within(id, selected))
            .collect()
    }

    /// Adds a card at `address` as typed to the selected device, plugged into the device at
    /// `choice` in the dropdown, where 0 is none. False if refused, so the UI can keep what was
    /// typed.
    fn add_card(&mut self, address: &str, choice: usize) -> bool {
        let upstream = match choice.checked_sub(1) {
            None => None,
            Some(at) => match self.upstream_choices().get(at) {
                Some(&id) => Some(id),
                None => return false,
            },
        };
        let (Some(device), Some(address)) = (self.selected, convert::address(address)) else {
            return false;
        };
        let added = self.network.add_card(device, address, upstream).is_some();
        if added {
            self.lay_out_at_rest();
        }
        added
    }

    /// Removes the selected device's card at `index`, unless it is the last.
    fn remove_card(&mut self, index: usize) {
        let device = self.selected.and_then(|id| self.network.device(id));
        let Some((device, card)) = device.and_then(|d| Some((d.id, d.cards.get(index)?.id))) else {
            return;
        };
        if self.network.remove_card(device, card) {
            self.lay_out_at_rest();
        }
    }

    /// Adds a guest named `name` to the selected device, with its first card at `address` as
    /// typed. False if refused, so the UI can keep what was typed.
    fn add_guest(&mut self, kind: GuestKind, name: &str, address: &str) -> bool {
        let (Some(host), Some(address)) = (self.selected, convert::address(address)) else {
            return false;
        };
        let added = self.network.add_guest(host, kind, name, address).is_some();
        if added {
            self.lay_out_at_rest();
        }
        added
    }

    /// Lays the map out again at rest: it is out of sight on the edit page, where these edits
    /// are made.
    fn lay_out_at_rest(&mut self) {
        self.lay_out();
        self.motion = Motion::new(self.positions.clone());
    }
}

/// Adding and removing the selected device's cards, and adding guests to it. A guest is then a
/// device of its own: edited, moved among its host's guests and forgotten like any other.
pub(super) fn connect_child_edits(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let (card_ui, card_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_add_card(move |address, choice| {
        let mut added = false;
        update(&card_ui, &card_state, |st| {
            added = st.add_card(&address, convert::index(choice));
        });
        added
    });
    bridge.on_remove_card(handler(ui, state, |st, index| {
        st.remove_card(convert::index(index));
    }));
    let (guest_ui, guest_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_add_guest(move |kind, name, address| {
        let Some(kind) = convert::guest_kind_from_ui(kind) else {
            return false;
        };
        let mut added = false;
        update(&guest_ui, &guest_state, |st| {
            added = st.add_guest(kind, &name, &address);
        });
        added
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    const SWITCH: DeviceId = DeviceId(2);
    const PC: DeviceId = DeviceId(5);
    const DOCKER_HOST: DeviceId = DeviceId(19);

    #[test]
    fn a_card_is_plugged_into_the_device_chosen() {
        let mut state = AppState::new(mock_data::network());
        assert!(!state.add_card("10.0.0.7", 0), "nothing selected");
        state.selected = Some(SWITCH);
        let choices = state.upstream_choices();
        assert!(!choices.contains(&SWITCH), "not into itself");
        assert!(!choices.contains(&PC), "nor into its own branch");

        state.selected = Some(PC);
        let router = 1 + state
            .upstream_choices()
            .iter()
            .position(|&id| id == DeviceId(1))
            .unwrap();
        assert!(!state.add_card("10.0.0", router));
        assert!(!state.add_card("10.0.0.7", 999));
        assert!(state.add_card(" 10.0.0.7/16 ", router));
        assert!(state.add_card("10.0.0.8", 0), "plugged into none");

        let pc = state.network.device(PC).unwrap();
        let added: Vec<_> = pc
            .cards
            .iter()
            .skip(1)
            .map(|c| c.address.to_string())
            .collect();
        assert_eq!(added, ["10.0.0.7/16", "10.0.0.8/24"]);
        let cross = state.network.cross_links();
        assert!(
            cross
                .iter()
                .any(|c| (c.upstream, c.downstream) == (DeviceId(1), PC))
        );
        assert_eq!(state.motion.shown(), &state.positions, "laid out at rest");
    }

    #[test]
    fn a_card_is_removed_by_its_index_but_never_the_last() {
        let mut state = AppState::new(mock_data::network());
        state.selected = Some(DOCKER_HOST);
        let cards = |st: &AppState| st.network.device(DOCKER_HOST).map(|d| d.cards.len());
        state.remove_card(9);
        assert_eq!(cards(&state), Some(2));
        state.remove_card(1);
        assert_eq!(cards(&state), Some(1));
        state.remove_card(0);
        assert_eq!(cards(&state), Some(1), "the last stays");
    }

    #[test]
    fn a_guest_is_added_to_the_selection_and_placed_under_it() {
        let mut state = AppState::new(mock_data::network());
        let kind = GuestKind::Container;
        assert!(
            !state.add_guest(kind, "Web", "172.17.0.9/16"),
            "nothing selected"
        );
        state.selected = Some(DOCKER_HOST);
        assert!(!state.add_guest(kind, "Web", "172.17.0"));
        assert!(!state.add_guest(kind, "  ", "172.17.0.9/16"));
        assert!(state.add_guest(kind, "Web", "172.17.0.9/16"));

        let web = state
            .network
            .devices()
            .iter()
            .find(|d| d.name == "Web")
            .unwrap()
            .id;
        assert_eq!(state.network.host(web), Some(DOCKER_HOST));
        assert!(state.positions[&web].y > state.positions[&DOCKER_HOST].y);
        assert_eq!(state.motion.shown(), &state.positions, "laid out at rest");
    }
}
