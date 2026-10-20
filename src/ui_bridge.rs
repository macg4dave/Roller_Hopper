//! Connects the Slint UI to the model: pushes state into the `Bridge` global and handles the
//! callbacks it raises. Rust owns the data; after every change the affected views are re-sent.
//! Rust sends data rather than wording, so all user-visible text can be translated in Slint.

mod convert;
mod i18n;
mod icons;
mod push;

pub(crate) use i18n::select_language;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, VecModel, Weak};

use crate::mock_data;
use crate::model::{
    Device, DeviceId, GroupId, Icon, Network, Point, Positions, Role, StatusFilter, tree_layout,
};
use crate::ui::{AppWindow, Bridge};
use icons::Icons;
use push::push;

/// How long the simulated scan pretends to take, so the refresh is visible.
const SCAN_DURATION: Duration = Duration::from_millis(1500);

/// Everything the window shows, owned by `main` and shared with the callbacks.
pub(crate) struct AppState {
    network: Network,
    positions: Positions,
    selected: Option<DeviceId>,
    filter: StatusFilter,
    query: String,
    scanning: bool,
    scans: u32,
    /// Read from the icon folder by [`connect`], and again when the icon picker opens.
    icons: Icons,
}

impl AppState {
    pub(crate) fn new(network: Network) -> Self {
        let positions = tree_layout(&network);
        Self {
            network,
            positions,
            selected: None,
            filter: StatusFilter::All,
            query: String::new(),
            scanning: false,
            scans: 0,
            icons: Icons::default(),
        }
    }

    fn edit_selected(&mut self, edit: impl FnOnce(&mut Device)) {
        if let Some(device) = self.selected.and_then(|id| self.network.device_mut(id)) {
            edit(device);
        }
    }

    fn role(&self, index: usize) -> Option<Role> {
        self.network.roles().into_iter().nth(index)
    }

    fn choose_role(&mut self, index: usize) {
        if let Some(role) = self.role(index) {
            self.edit_selected(|d| d.role = role);
        }
    }

    /// Adds the role named `name`, shown with `icon`, unless it matches one of the roles `shown`
    /// in the dropdown (which may be translated, so only the UI knows their names).
    fn add_role(&mut self, name: &str, icon: Icon, shown: &[SharedString]) {
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

    fn edit_role(&mut self, index: usize, edit: impl FnOnce(&mut Network, &Role)) {
        if let Some(role) = self.role(index) {
            edit(&mut self.network, &role);
        }
    }

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

    /// Lists a port on the selected device. False if there is none, or the port number is
    /// refused or already listed, so the UI can keep what was typed.
    fn add_port(&mut self, port: &str, description: &str) -> bool {
        let Some(port) = convert::port(port) else {
            return false;
        };
        let mut added = false;
        self.edit_selected(|d| added = d.services.add(port, description));
        added
    }

    /// Forgets the selected device until restart; the rest of the map stays where it is.
    fn forget_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.network.remove_device(id);
            self.positions.remove(&id);
        }
    }

    fn finish_scan(&mut self) {
        mock_data::simulate_scan(&mut self.network);
        self.scanning = false;
        self.scans += 1;
    }
}

/// Wires every `Bridge` callback to the state and shows the initial state.
pub(crate) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    bridge.on_select(handler(ui, state, |st, id| {
        st.selected = convert::device_id(id);
    }));
    bridge.on_search(handler(ui, state, |st, text: SharedString| {
        st.query = text.into();
    }));
    bridge.on_set_filter(handler(ui, state, |st, f| {
        st.filter = convert::filter_from_ui(f);
    }));
    connect_device_edits(ui, state);
    connect_port_edits(ui, state);
    connect_role_edits(ui, state);
    connect_group_edits(ui, state);
    let (icons_ui, icons_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_refresh_icons(move || refresh_icons(&icons_ui, &icons_state));

    let reset = handler(ui, state, |st, ()| st.positions = tree_layout(&st.network));
    bridge.on_reset_layout(move || reset(()));
    let move_device = handler(ui, state, |st, (id, x, y)| {
        if let Some(id) = convert::device_id(id) {
            st.positions.insert(id, Point { x, y });
        }
    });
    bridge.on_move_device(move |id, x, y| move_device((id, x, y)));

    let start_scan = handler(ui, state, |st, ()| st.scanning = true);
    let finish_scan = Rc::new(handler(ui, state, |st, ()| st.finish_scan()));
    bridge.on_scan(move || {
        start_scan(());
        let finish_scan = Rc::clone(&finish_scan);
        Timer::single_shot(SCAN_DURATION, move || finish_scan(()));
    });

    // Shows the initial state, with the icons.
    refresh_icons(&ui.as_weak(), state);
}

/// Name, notes, role, icon and forgetting, for the selected device.
fn connect_device_edits(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
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
    bridge.on_set_icon(handler(ui, state, |st, icon: SharedString| {
        st.edit_selected(|d| d.icon = Some(convert::icon_from_ui(&icon)));
    }));
    let reset_icon = handler(ui, state, |st, ()| st.edit_selected(|d| d.icon = None));
    bridge.on_reset_icon(move || reset_icon(()));
    let forget = handler(ui, state, |st, ()| st.forget_selected());
    bridge.on_forget(move || forget(()));
}

/// Adding, renumbering, describing and removing the selected device's open ports.
fn connect_port_edits(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
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
            st.edit_selected(|d| d.services.set_port(convert::index(index), port));
        }
    });
    bridge.on_set_port_number(move |index, port| renumber((index, port)));
    let describe = handler(ui, state, |st, (index, text): (i32, SharedString)| {
        st.edit_selected(|d| d.services.set_description(convert::index(index), &text));
    });
    bridge.on_set_port_description(move |index, text| describe((index, text)));
    bridge.on_remove_port(handler(ui, state, |st, index| {
        st.edit_selected(|d| d.services.remove(convert::index(index)));
    }));
}

/// A new role's name and icon, with the role names the dropdown shows.
type NewRole = (SharedString, Icon, Vec<SharedString>);

/// The role list: adding, renaming, re-iconing, deleting and restoring roles.
fn connect_role_edits(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
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

/// The group list, and choosing a role's group in the role list.
fn connect_group_edits(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
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
    bridge.on_delete_group(handler(ui, state, |st, index| {
        st.edit_group(convert::index(index), Network::remove_group);
    }));
}

/// Reads the icon folder again and shows what is in it now. The picker's list is sent only
/// here, not on every change, so it is not rebuilt while the user scrolls it.
fn refresh_icons(ui: &Weak<AppWindow>, state: &RefCell<AppState>) {
    update(ui, state, |st| st.icons = Icons::load(&icons::folder()));
    if let Some(ui) = ui.upgrade() {
        let groups = VecModel::from(state.borrow().icons.groups());
        ui.global::<Bridge<'_>>().set_icons(ModelRc::new(groups));
    }
}

/// The role names in the dropdown, as the UI shows them (standard ones translated).
fn shown_roles(ui: &Weak<AppWindow>) -> Vec<SharedString> {
    ui.upgrade()
        .map(|window| window.global::<Bridge<'_>>().get_roles().iter().collect())
        .unwrap_or_default()
}

/// A callback that applies `change` with the callback's argument, then refreshes the window.
fn handler<A>(
    ui: &AppWindow,
    state: &Rc<RefCell<AppState>>,
    change: impl Fn(&mut AppState, A) + 'static,
) -> impl Fn(A) + 'static {
    let (ui, state) = (ui.as_weak(), Rc::clone(state));
    move |arg| update(&ui, &state, |st| change(st, arg))
}

/// Applies a change to the state, then re-sends it to the window (if it is still open).
fn update(ui: &Weak<AppWindow>, state: &RefCell<AppState>, change: impl FnOnce(&mut AppState)) {
    let Some(ui) = ui.upgrade() else { return };
    change(&mut state.borrow_mut());
    push(&ui, &state.borrow());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(selected: u32) -> AppState {
        let mut state = AppState::new(mock_data::network());
        state.selected = Some(DeviceId(selected));
        state
    }

    fn selected_role(state: &AppState) -> Option<Role> {
        state
            .selected
            .and_then(|id| state.network.device(id))
            .map(|d| d.role.clone())
    }

    #[test]
    fn choosing_a_role_by_index_assigns_it_to_the_selection() {
        let mut state = state_with(5);
        state.choose_role(0);
        assert_eq!(selected_role(&state), Some(Role::Router));
        state.choose_role(999);
        assert_eq!(
            selected_role(&state),
            Some(Role::Router),
            "out of range is ignored"
        );
    }

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
        let mut state = state_with(10);
        let shown = shown_roles(&state);
        let before = state.network.clone();
        state.add_role(" serveur ", Icon::new("Entertainment/Smart TV"), &shown);
        assert_eq!(state.network, before);
    }

    #[test]
    fn a_new_role_comes_last_with_its_icon() {
        let mut state = state_with(10);
        let shown = shown_roles(&state);
        let media = Role::Custom("Media".into());
        let server = Icon::new("Computers/Server");
        state.add_role("Media", server.clone(), &shown);
        assert_eq!(state.network.roles().last(), Some(&media));
        assert_eq!(state.network.role_icon(&media), Some(&server));
        assert_eq!(
            selected_role(&state),
            Some(Role::Custom("TV".into())),
            "unchanged"
        );
    }

    /// Where a role sits in the dropdown.
    fn index_of(state: &AppState, role: &Role) -> usize {
        state
            .network
            .roles()
            .iter()
            .position(|r| r == role)
            .unwrap()
    }

    #[test]
    fn a_role_cannot_take_the_name_of_another_shown_role() {
        let mut state = state_with(10);
        let shown = shown_roles(&state);
        let tv = index_of(&state, &Role::Custom("TV".into()));
        state.rename_role(tv, "imprimante", &shown);
        assert_eq!(selected_role(&state), Some(Role::Custom("TV".into())));
        state.rename_role(tv, "tv", &shown);
        assert_eq!(
            selected_role(&state),
            Some(Role::Custom("tv".into())),
            "own name"
        );
        let router = index_of(&state, &Role::Router);
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

    #[test]
    fn a_role_is_put_in_a_group_by_dropdown_choice() {
        let mut state = AppState::new(mock_data::network());
        let printer = index_of(&state, &Role::Printer);
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
        let printer = index_of(&state, &Role::Printer);
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

    #[test]
    fn a_port_is_added_to_the_selection_only_with_a_valid_number() {
        let mut state = AppState::new(mock_data::network());
        assert!(!state.add_port("8080", "Web"), "nothing selected");
        state.selected = Some(DeviceId(10));
        assert!(!state.add_port("70000", "Web"));
        assert!(state.add_port(" 8080 ", "Web"));
        assert!(!state.add_port("8080", "Again"), "already listed");
        let device = state
            .network
            .device(DeviceId(10))
            .map(|d| d.services.to_string());
        assert_eq!(device.as_deref(), Some("8001 (SmartView), 8080 (Web)"));
    }

    #[test]
    fn forgetting_clears_the_selection_and_its_place_on_the_map() {
        let mut state = state_with(10);
        state.forget_selected();
        assert_eq!(state.selected, None);
        assert!(state.network.device(DeviceId(10)).is_none());
        assert!(!state.positions.contains_key(&DeviceId(10)));
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
    fn finishing_a_scan_counts_it_and_clears_the_busy_state() {
        let mut state = AppState::new(mock_data::network());
        state.scanning = true;
        state.finish_scan();
        assert!(!state.scanning);
        assert_eq!(state.scans, 1);
    }
}
