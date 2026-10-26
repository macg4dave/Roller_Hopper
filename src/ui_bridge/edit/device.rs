//! Editing the selected device's name, notes, role, icon and the device it runs on;
//! forgetting it; and adding a blank ordinary device.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use crate::model::{DeviceId, Icon};
use crate::ui::{AppWindow, Bridge};
use crate::ui_bridge::{AppState, Selection, convert, handler};

impl AppState {
    /// Gives device `id` its own icon.
    fn set_icon(&mut self, id: DeviceId, icon: Icon) {
        if let Some(device) = self.network.device_mut(id) {
            device.icon = Some(icon);
        }
    }

    fn choose_role(&mut self, index: usize) {
        if let Some(role) = self.role(index) {
            self.edit_selected(|d| d.role = role);
        }
    }

    /// The devices the selected device can run on, in dropdown order after "none": every
    /// other device, in device order, apart from those that run on it.
    pub(in crate::ui_bridge) fn host_choices(&self) -> Vec<DeviceId> {
        let Some(selected) = self.selected_device() else {
            return Vec::new();
        };
        let devices = self.network.devices().iter().map(|d| d.id);
        devices
            .filter(|&host| self.network.can_host(selected, host))
            .collect()
    }

    /// What the selected device runs on, as its index in the dropdown: 0 for none.
    pub(in crate::ui_bridge) fn host_choice(&self) -> usize {
        let selected = self
            .selected_device()
            .and_then(|id| self.network.device(id));
        let host = selected.and_then(|d| d.host);
        let at = host.and_then(|host| self.host_choices().iter().position(|&d| d == host));
        at.map_or(0, |at| at + 1)
    }

    /// Makes the selected device run on the device at `choice` in the dropdown, where 0 is none.
    fn set_host(&mut self, choice: usize) {
        let host = match choice.checked_sub(1) {
            None => None,
            Some(at) => match self.host_choices().get(at) {
                Some(&host) => Some(host),
                None => return,
            },
        };
        if let Some(device) = self.selected_device() {
            self.network.set_host(device, host);
        }
    }

    /// Forgets the selected device (or the device of the selected card) until restart and lays
    /// the map out again without its cards.
    fn forget_selected(&mut self) {
        if let Some(id) = self.selected_device() {
            self.selected = None;
            self.network.remove_device(id);
            self.lay_out_at_rest();
        }
    }

    /// Adds a blank device and selects it so it can be filled in.
    fn add_device(&mut self) {
        if let Some(id) = self.network.add_device() {
            self.selected = Some(Selection::Device(id));
            self.lay_out_at_rest();
        }
    }
}

/// Name, notes, role, host, icon and forgetting for the selected device, and adding devices.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    bridge.on_rename(handler(ui, state, |st, name: SharedString| {
        st.edit_selected(|d| d.name = name.into());
    }));
    bridge.on_edit_notes(handler(ui, state, |st, notes: SharedString| {
        st.edit_selected(|d| d.notes = notes.into());
    }));
    bridge.on_choose_role(handler(ui, state, |st, index| {
        st.choose_role(convert::index(index));
    }));
    bridge.on_choose_host(handler(ui, state, |st, choice| {
        st.set_host(convert::index(choice));
    }));
    let set_icon = handler(ui, state, |st, (id, icon): (i32, SharedString)| {
        if let Some(id) = convert::device_id(id) {
            st.set_icon(id, convert::icon_from_ui(&icon));
        }
    });
    bridge.on_set_icon(move |id, icon| set_icon((id, icon)));
    let reset_icon = handler(ui, state, |st, ()| st.edit_selected(|d| d.icon = None));
    bridge.on_reset_icon(move || reset_icon(()));
    let forget = handler(ui, state, |st, ()| st.forget_selected());
    bridge.on_forget(move || forget(()));
    let add = handler(ui, state, |st, ()| st.add_device());
    bridge.on_add_device(move || add(()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::{CardId, Role};

    #[test]
    fn choosing_a_role_by_index_assigns_it_to_the_selection() {
        let mut state = AppState::with_selected(5);
        state.choose_role(0);
        assert_eq!(state.selected_role(), Some(Role::Router));
        state.choose_role(999);
        assert_eq!(
            state.selected_role(),
            Some(Role::Router),
            "out of range is ignored"
        );
    }

    #[test]
    fn a_device_is_given_a_host_from_the_others_that_do_not_run_on_it() {
        let (server, vm) = (DeviceId(6), DeviceId(19));
        let mut state = AppState::with_selected(19);
        let choices = state.host_choices();
        assert!(!choices.contains(&vm), "not itself");
        let at = choices.iter().position(|&d| d == server).unwrap();
        state.set_host(at + 1);
        assert_eq!(state.host_choice(), at + 1);
        assert_eq!(state.network.device(vm).and_then(|d| d.host), Some(server));
        state.selected = Some(Selection::Device(server));
        assert!(!state.host_choices().contains(&vm), "would form a ring");
        state.set_host(999);
        assert_eq!(state.host_choice(), 0, "out of range is ignored");
        state.selected = Some(Selection::Device(vm));
        state.set_host(0);
        assert_eq!(state.host_choice(), 0);
    }

    #[test]
    fn forgetting_clears_the_selection_and_the_places_of_the_device() {
        let mut state = AppState::with_selected(19);
        state.forget_selected();
        assert_eq!(state.selected, None);
        assert!(state.network.device(DeviceId(19)).is_none());
        assert!(!state.positions.contains_key(&CardId(19)));
        assert!(state.network.device(DeviceId(24)).is_some());
        assert!(!state.positions.contains_key(&CardId(100)), "its lab card");
    }

    #[test]
    fn a_device_takes_its_own_icon_without_changing_the_selection() {
        let mut state = AppState::with_selected(19);
        let web_server = DeviceId(24);
        let lamp = Icon::new("Smart home/Smart light");
        let icon_of = |st: &AppState, id| st.network.device(id).and_then(|d| d.icon.clone());
        let host_icon = icon_of(&state, DeviceId(19));
        state.set_icon(web_server, lamp.clone());
        assert_eq!(icon_of(&state, web_server), Some(lamp));
        assert_eq!(
            icon_of(&state, DeviceId(19)),
            host_icon,
            "the host keeps its own"
        );
        assert_eq!(state.selected, Some(Selection::Device(DeviceId(19))));
        let before = state.network.clone();
        state.set_icon(DeviceId(999), Icon::new("Network/Router"));
        assert_eq!(state.network, before, "no such device");
    }

    #[test]
    fn edits_without_a_selection_change_nothing() {
        let mut state = AppState::new(mock_data::network());
        let before = state.network.clone();
        state.edit_selected(|d| d.name = "Changed".into());
        state.add_role("Media", Icon::new("Entertainment/Smart TV"), &[]);
        state.forget_selected();
        assert_eq!(state.network.devices(), before.devices());
    }

    #[test]
    fn a_device_is_added_selected_and_placed_on_the_map() {
        let mut state = AppState::new(mock_data::network());
        state.add_device();
        let added = state.selected_device().unwrap();
        let card = state.network.device(added).unwrap().cards[0].id;
        assert!(state.positions.contains_key(&card), "on the map");
    }
}
