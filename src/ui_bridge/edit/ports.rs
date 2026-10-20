//! Editing the selected network card's open ports.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use crate::ui::{AppWindow, Bridge};
use crate::ui_bridge::{AppState, convert, handler, update};

impl AppState {
    /// Lists a port on the selected card. False if there is none, or the port number is refused
    /// or already listed, so the UI can keep what was typed.
    fn add_port(&mut self, port: &str, description: &str) -> bool {
        let Some(port) = convert::port(port) else {
            return false;
        };
        let mut added = false;
        self.edit_selected_card(|c| added = c.services.add(port, description));
        added
    }
}

/// Adding, renumbering, describing and removing the selected card's open ports.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let (add_ui, add_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_add_port(move |port, description| {
        let mut added = false;
        update(&add_ui, &add_state, |st| {
            added = st.add_port(&port, &description);
        });
        added
    });
    let renumber = handler(ui, state, |st, (index, port): (i32, SharedString)| {
        if let Some(port) = convert::port(&port) {
            st.edit_selected_card(|c| c.services.set_port(convert::index(index), port));
        }
    });
    bridge.on_set_port_number(move |index, port| renumber((index, port)));
    let describe = handler(ui, state, |st, (index, text): (i32, SharedString)| {
        st.edit_selected_card(|c| c.services.set_description(convert::index(index), &text));
    });
    bridge.on_set_port_description(move |index, text| describe((index, text)));
    bridge.on_remove_port(handler(ui, state, |st, index| {
        st.edit_selected_card(|c| c.services.remove(convert::index(index)));
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::{CardId, DeviceId};
    use crate::ui_bridge::Selection;

    #[test]
    fn a_port_is_added_to_the_selection_only_with_a_valid_number() {
        let mut state = AppState::new(mock_data::network());
        assert!(!state.add_port("8080", "Web"), "nothing selected");
        state.selected = Some(Selection::Device(DeviceId(10)));
        assert!(
            !state.add_port("8080", "Web"),
            "a device has no ports of its own"
        );
        state.selected = Some(Selection::Card(CardId(10)));
        assert!(!state.add_port("70000", "Web"));
        assert!(state.add_port(" 8080 ", "Web"));
        assert!(!state.add_port("8080", "Again"), "already listed");
        let card = state
            .network
            .card(CardId(10))
            .map(|c| c.services.to_string());
        assert_eq!(card.as_deref(), Some("8001 (SmartView), 8080 (Web)"));
    }
}
