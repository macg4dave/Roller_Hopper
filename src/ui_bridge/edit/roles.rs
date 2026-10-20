//! The role list on the edit page: adding, renaming, re-iconing, deleting and restoring roles.
//! Roles are identified by their index in the role dropdown.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Model, SharedString, Weak};

use crate::model::{Icon, Network, Role};
use crate::ui::{AppWindow, Bridge};
use crate::ui_bridge::{AppState, convert, handler};

/// A new role's name and icon, with the role names the dropdown shows.
type NewRole = (SharedString, Icon, Vec<SharedString>);

impl AppState {
    /// The role at `index` in the role dropdown.
    pub(in crate::ui_bridge) fn role(&self, index: usize) -> Option<Role> {
        self.network.roles().into_iter().nth(index)
    }

    /// Adds the role named `name`, shown with `icon`, unless it matches one of the roles `shown`
    /// in the dropdown (which may be translated, so only the UI knows their names).
    pub(super) fn add_role(&mut self, name: &str, icon: Icon, shown: &[SharedString]) {
        let lower = name.trim().to_lowercase();
        if !shown.iter().any(|r| r.trim().to_lowercase() == lower) {
            // A blank name adds nothing, which is what the user expects of it.
            self.network.add_custom_role(name, icon);
        }
    }

    /// Renames the role at `index`, unless `new` is the name of another role `shown` in the
    /// dropdown, such as a translated standard role.
    fn rename_role(&mut self, index: usize, new: &str, shown: &[SharedString]) {
        let lower = new.trim().to_lowercase();
        let clash = shown
            .iter()
            .enumerate()
            .any(|(i, r)| i != index && r.trim().to_lowercase() == lower);
        if !clash {
            self.edit_role(index, |network, role| network.rename_role(role, new));
        }
    }

    pub(super) fn edit_role(&mut self, index: usize, edit: impl FnOnce(&mut Network, &Role)) {
        if let Some(role) = self.role(index) {
            edit(&mut self.network, &role);
        }
    }
}

/// Adding, renaming, re-iconing, deleting and restoring roles.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let add = handler(ui, state, |st, (name, icon, shown): NewRole| {
        st.add_role(&name, icon, &shown);
    });
    let add_ui = ui.as_weak();
    bridge.on_add_role(move |name, icon| {
        add((name, convert::icon_from_ui(&icon), shown_roles(&add_ui)));
    });
    let rename = handler(
        ui,
        state,
        |st, (index, new, shown): (i32, SharedString, Vec<SharedString>)| {
            st.rename_role(convert::index(index), &new, &shown);
        },
    );
    let rename_ui = ui.as_weak();
    bridge.on_rename_role(move |index, new| rename((index, new, shown_roles(&rename_ui))));
    let set_icon = Rc::new(handler(
        ui,
        state,
        |st, (index, icon): (i32, Option<Icon>)| {
            st.edit_role(convert::index(index), |n, role| n.set_role_icon(role, icon));
        },
    ));
    let clear_icon = Rc::clone(&set_icon);
    bridge.on_set_role_icon(move |index, icon| {
        set_icon((index, Some(convert::icon_from_ui(&icon))));
    });
    bridge.on_clear_role_icon(move |index| clear_icon((index, None)));
    bridge.on_delete_role(handler(ui, state, |st, index| {
        st.edit_role(convert::index(index), Network::remove_role);
    }));
    let restore = handler(ui, state, |st, ()| st.network.restore_standard_roles());
    bridge.on_restore_roles(move || restore(()));
}

/// The role names in the dropdown, as the UI shows them (standard ones translated).
fn shown_roles(ui: &Weak<AppWindow>) -> Vec<SharedString> {
    ui.upgrade()
        .map(|window| window.global::<Bridge<'_>>().get_roles().iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    /// Role names as the dropdown would show them in some language.
    fn shown_roles(state: &AppState) -> Vec<SharedString> {
        let standard = [
            "Routeur",
            "Commutateur",
            "Point d'accès",
            "Serveur",
            "NAS",
            "Bureau",
        ];
        let rest = [
            "Portable",
            "Téléphone",
            "Imprimante",
            "Caméra",
            "IoT",
            "Autre",
        ];
        let custom = state.network.roles().into_iter().filter_map(|r| {
            if let Role::Custom(name) = r {
                Some(name.into())
            } else {
                None
            }
        });
        standard
            .into_iter()
            .chain(rest)
            .map(SharedString::from)
            .chain(custom)
            .collect()
    }

    #[test]
    fn adding_a_role_that_is_already_shown_adds_nothing() {
        let mut state = AppState::with_selected(10);
        let shown = shown_roles(&state);
        let before = state.network.clone();
        state.add_role(" serveur ", Icon::new("Entertainment/Smart TV"), &shown);
        assert_eq!(state.network, before);
    }

    #[test]
    fn a_new_role_comes_last_with_its_icon() {
        let mut state = AppState::with_selected(10);
        let shown = shown_roles(&state);
        let media = Role::Custom("Media".into());
        let server = Icon::new("Computers/Server");
        state.add_role("Media", server.clone(), &shown);
        assert_eq!(state.network.roles().last(), Some(&media));
        assert_eq!(state.network.role_icon(&media), Some(&server));
        assert_eq!(
            state.selected_role(),
            Some(Role::Custom("TV".into())),
            "unchanged"
        );
    }

    #[test]
    fn a_role_cannot_take_the_name_of_another_shown_role() {
        let mut state = AppState::with_selected(10);
        let shown = shown_roles(&state);
        let tv = state.role_index(&Role::Custom("TV".into()));
        state.rename_role(tv, "imprimante", &shown);
        assert_eq!(state.selected_role(), Some(Role::Custom("TV".into())));
        state.rename_role(tv, "tv", &shown);
        assert_eq!(
            state.selected_role(),
            Some(Role::Custom("tv".into())),
            "own name"
        );
        let router = state.role_index(&Role::Router);
        state.rename_role(router, "Serveur", &shown);
        assert_eq!(state.network.role_name(&Role::Router), None);
        state.rename_role(router, "Passerelle", &shown);
        assert_eq!(state.network.role_name(&Role::Router), Some("Passerelle"));
    }

    #[test]
    fn role_edits_out_of_range_change_nothing() {
        let mut state = AppState::new(mock_data::network());
        let before = state.network.clone();
        state.edit_role(999, Network::remove_role);
        state.rename_role(999, "Anything", &[]);
        assert_eq!(state.network, before);
    }
}
