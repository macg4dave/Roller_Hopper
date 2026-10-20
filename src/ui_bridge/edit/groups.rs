//! The group list on the edit page, and choosing a role's group in the role list. Groups are
//! identified by their index in the group list.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use crate::model::{GroupId, Network};
use crate::ui::{AppWindow, Bridge};
use crate::ui_bridge::{AppState, convert, handler};

impl AppState {
    fn group(&self, index: usize) -> Option<GroupId> {
        self.network.groups().iter().nth(index).map(|(id, _)| id)
    }

    /// Puts the role at `index` in the group at `choice` in the group dropdown, where 0 is none.
    fn set_role_group(&mut self, index: usize, choice: usize) {
        let group = match choice.checked_sub(1) {
            None => None,
            Some(group) => match self.group(group) {
                Some(id) => Some(id),
                None => return,
            },
        };
        self.edit_role(index, |network, role| network.set_role_group(role, group));
    }

    fn edit_group(&mut self, index: usize, edit: impl FnOnce(&mut Network, GroupId)) {
        if let Some(group) = self.group(index) {
            edit(&mut self.network, group);
        }
    }
}

/// Adding, renaming, colouring, styling and deleting groups, and putting roles in them.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let set_group = handler(ui, state, |st, (role, choice)| {
        st.set_role_group(convert::index(role), convert::index(choice));
    });
    bridge.on_set_role_group(move |role, choice| set_group((role, choice)));
    bridge.on_add_group(handler(ui, state, |st, name: SharedString| {
        st.network.add_group(&name);
    }));
    let rename = handler(ui, state, |st, (index, new): (i32, SharedString)| {
        st.edit_group(convert::index(index), |n, group| {
            n.rename_group(group, &new);
        });
    });
    bridge.on_rename_group(move |index, new| rename((index, new)));
    let set_colour = handler(ui, state, |st, (index, colour)| {
        let colour = convert::colour_from_ui(colour);
        st.edit_group(convert::index(index), |n, group| {
            n.set_group_colour(group, colour);
        });
    });
    bridge.on_set_group_colour(move |index, colour| set_colour((index, colour)));
    let set_style = handler(ui, state, |st, (index, choice)| {
        let Some(style) = convert::line_style_from_ui(choice) else {
            return;
        };
        st.edit_group(convert::index(index), |n, group| {
            n.set_group_line_style(group, style);
        });
    });
    bridge.on_set_group_line_style(move |index, choice| set_style((index, choice)));
    bridge.on_delete_group(handler(ui, state, |st, index| {
        st.edit_group(convert::index(index), Network::remove_group);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::Role;

    #[test]
    fn a_role_is_put_in_a_group_by_dropdown_choice() {
        let mut state = AppState::new(mock_data::network());
        let printer = state.role_index(&Role::Printer);
        let third = state.group(2);
        assert!(third.is_some());
        assert_ne!(state.network.role_group(&Role::Printer), third);
        state.set_role_group(printer, 3);
        assert_eq!(state.network.role_group(&Role::Printer), third);
        state.set_role_group(printer, 999);
        assert_eq!(
            state.network.role_group(&Role::Printer),
            third,
            "out of range"
        );
        state.set_role_group(printer, 0);
        assert_eq!(state.network.role_group(&Role::Printer), None);
    }

    #[test]
    fn deleting_a_group_leaves_its_roles_in_none() {
        let mut state = AppState::new(mock_data::network());
        let printer = state.role_index(&Role::Printer);
        state.set_role_group(printer, 1);
        state.edit_group(0, Network::remove_group);
        assert_eq!(state.network.role_group(&Role::Printer), None);
        assert!(
            state
                .network
                .groups()
                .iter()
                .all(|(_, n)| n != "Network core")
        );
    }
}
