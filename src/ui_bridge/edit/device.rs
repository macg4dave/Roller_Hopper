//! Editing the selected device: its name, notes, role and icon, what a guest runs as, and
//! forgetting it; and adding a blank device or guest. A guest is then a device of its own:
//! edited, moved among its host's guests and forgotten like any other.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use crate::model::{DeviceId, Guest, GuestKind, Icon};
use crate::ui::{AppWindow, Bridge};
use crate::ui_bridge::{AppState, Selection, convert, handler};

impl AppState {
    /// Gives device `id` its own icon: the selected device, or one of its guests from their row.
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

    /// Forgets the selected device (or the device of the selected card), with what runs on it,
    /// until restart, and lays the map out again without their cards.
    fn forget_selected(&mut self) {
        if let Some(id) = self.selected_device() {
            self.selected = None;
            self.network.remove_device(id);
            self.lay_out_at_rest();
        }
    }

    /// Adds a blank device and selects it, so it can be filled in, as a new card is: a guest of
    /// `kind` on the selected device, or with no kind a device of its own.
    fn add_device(&mut self, kind: Option<GuestKind>) {
        let guest = match kind {
            None => None,
            Some(kind) => match self.selected_device() {
                Some(host) => Some(Guest { host, kind }),
                None => return,
            },
        };
        if let Some(id) = self.network.add_device(guest) {
            self.selected = Some(Selection::Device(id));
            self.lay_out_at_rest();
        }
    }

    /// Changes what the selected guest runs as. Its icon follows unless it or its role has one
    /// (see [`crate::model::Network::icon`]).
    fn set_guest_kind(&mut self, kind: GuestKind) {
        self.edit_selected(|device| {
            if let Some(guest) = &mut device.guest {
                guest.kind = kind;
            }
        });
    }
}

/// Name, notes, role, icon and forgetting for the selected device; adding devices and guests,
/// and what a guest runs as.
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
    bridge.on_add_device(handler(ui, state, |st, kind| {
        st.add_device(convert::guest_kind_from_ui(kind));
    }));
    bridge.on_set_guest_kind(handler(ui, state, |st, kind| {
        if let Some(kind) = convert::guest_kind_from_ui(kind) {
            st.set_guest_kind(kind);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::{CardId, Role};

    const PC: DeviceId = DeviceId(5);
    const DOCKER_HOST: DeviceId = DeviceId(19);

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
    fn forgetting_clears_the_selection_and_the_places_of_the_device_and_its_guests() {
        let mut state = AppState::with_selected(19);
        state.forget_selected();
        assert_eq!(state.selected, None);
        for gone in [19, 24, 25] {
            assert!(state.network.device(DeviceId(gone)).is_none(), "{gone}");
            assert!(!state.positions.contains_key(&CardId(gone)), "{gone}");
        }
        assert!(!state.positions.contains_key(&CardId(100)), "its lab card");
    }

    #[test]
    fn a_guest_takes_its_own_icon_while_its_host_stays_selected() {
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
    fn a_virtual_machine_is_added_to_the_selection_selected_and_placed_under_it() {
        let mut state = AppState::new(mock_data::network());
        let before = state.network.clone();
        state.add_device(Some(GuestKind::VirtualMachine));
        assert_eq!(state.network, before, "nothing selected to run on");

        state.selected = Some(Selection::Card(CardId(DOCKER_HOST.0)));
        state.add_device(Some(GuestKind::VirtualMachine));
        let vm = state.selected_device().unwrap();
        assert_eq!(state.selected, Some(Selection::Device(vm)), "selected");
        assert_eq!(state.network.host(vm), Some(DOCKER_HOST));
        let vm = state.network.device(vm).unwrap().cards[0].id;
        let host = CardId(DOCKER_HOST.0);
        assert!(state.positions[&vm].y > state.positions[&host].y);
        assert_eq!(state.motion.shown(), &state.positions, "laid out at rest");
    }

    #[test]
    fn a_device_of_its_own_is_added_and_selected_with_or_without_a_selection() {
        let mut state = AppState::new(mock_data::network());
        state.add_device(None);
        let added = state.selected_device().unwrap();
        assert_eq!(state.network.host(added), None);
        let card = state.network.device(added).unwrap().cards[0].id;
        assert!(state.positions.contains_key(&card), "on the map");
    }

    #[test]
    fn a_guest_runs_as_another_kind_and_its_icon_follows_unless_picked() {
        let mut state = AppState::new(mock_data::network());
        state.selected = Some(Selection::Device(DOCKER_HOST));
        state.add_device(Some(GuestKind::VirtualMachine));
        let guest = state.selected_device().unwrap();
        let of = |st: &AppState| {
            let d = st.network.device(guest).unwrap();
            (d.guest.map(|g| g.kind), st.network.icon(d))
        };
        let vm = Some(GuestKind::VirtualMachine);
        assert_eq!(of(&state), (vm, GuestKind::VirtualMachine.icon()));
        state.set_guest_kind(GuestKind::Container);
        let container = Some(GuestKind::Container);
        assert_eq!(of(&state), (container, GuestKind::Container.icon()));

        let fish = Icon::new("My icons/Fish tank");
        state.edit_selected(|d| d.icon = Some(fish.clone()));
        state.set_guest_kind(GuestKind::VirtualMachine);
        assert_eq!(of(&state), (vm, fish), "picked icon kept");

        state.selected = Some(Selection::Device(PC));
        state.set_guest_kind(GuestKind::Container);
        assert_eq!(state.network.device(PC).unwrap().guest, None, "not a guest");
    }
}
