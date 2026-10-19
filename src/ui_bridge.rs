//! Connects the Slint UI to the model: pushes state into the `Bridge` global and handles the
//! callbacks it raises. Rust owns the data; after every change the affected views are re-sent.
//! Rust sends data rather than wording, so all user-visible text can be translated in Slint.

mod convert;
mod i18n;
mod push;

pub(crate) use i18n::select_language;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Model, SharedString, Timer, Weak};

use crate::mock_data;
use crate::model::{
    Device, DeviceId, DeviceKind, Network, Point, Positions, StatusFilter, tree_layout,
};
use crate::ui::{AppWindow, Bridge};
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
        }
    }

    fn edit_selected(&mut self, edit: impl FnOnce(&mut Device)) {
        if let Some(device) = self.selected.and_then(|id| self.network.device_mut(id)) {
            edit(device);
        }
    }

    fn choose_role(&mut self, index: usize) {
        if let Some(role) = self.network.roles().into_iter().nth(index) {
            self.edit_selected(|d| d.role = role);
        }
    }

    /// Assigns the role named `name`, adding it as a custom role shown with `icon` unless it
    /// matches one of the roles `shown` in the dropdown (which are translated, so only the UI
    /// knows their names). A matching role keeps its own icon.
    fn add_role(&mut self, name: &str, icon: DeviceKind, shown: &[SharedString]) {
        let lower = name.trim().to_lowercase();
        if let Some(index) = shown.iter().position(|r| r.trim().to_lowercase() == lower) {
            self.choose_role(index);
        } else if let Some(role) = self.network.add_custom_role(name, icon) {
            self.edit_selected(|d| d.role = role);
        }
    }

    /// Renames custom role `old`, unless `new` is the name of another role `shown` in the
    /// dropdown, such as a translated standard role.
    fn rename_role(&mut self, old: &str, new: &str, shown: &[SharedString]) {
        let lower = new.trim().to_lowercase();
        let clash = shown
            .iter()
            .any(|r| r.as_str() != old && r.trim().to_lowercase() == lower);
        if !clash {
            self.network.rename_custom_role(old, new);
        }
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
    connect_role_edits(ui, state);

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

    push(ui, &state.borrow());
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
        st.choose_role(usize::try_from(index).unwrap_or(usize::MAX));
    }));
    let add_role = handler(
        ui,
        state,
        |st, (name, icon, shown): (SharedString, DeviceKind, Vec<SharedString>)| {
            st.add_role(&name, icon, &shown);
        },
    );
    let ui_weak = ui.as_weak();
    bridge.on_add_role(move |name, icon| {
        add_role((name, convert::kind_from_ui(icon), shown_roles(&ui_weak)));
    });
    bridge.on_set_icon(handler(ui, state, |st, icon| {
        st.edit_selected(|d| d.icon = Some(convert::kind_from_ui(icon)));
    }));
    let reset_icon = handler(ui, state, |st, ()| st.edit_selected(|d| d.icon = None));
    bridge.on_reset_icon(move || reset_icon(()));
    let forget = handler(ui, state, |st, ()| st.forget_selected());
    bridge.on_forget(move || forget(()));
}

/// Renaming, re-iconing and deleting custom roles.
fn connect_role_edits(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let rename = handler(
        ui,
        state,
        |st, (old, new, shown): (SharedString, SharedString, Vec<SharedString>)| {
            st.rename_role(&old, &new, &shown);
        },
    );
    let ui_weak = ui.as_weak();
    bridge.on_rename_role(move |old, new| rename((old, new, shown_roles(&ui_weak))));
    let set_icon = handler(ui, state, |st, (name, icon): (SharedString, DeviceKind)| {
        st.network.set_custom_role_icon(&name, icon);
    });
    bridge.on_set_role_icon(move |name, icon| set_icon((name, convert::kind_from_ui(icon))));
    bridge.on_delete_role(handler(ui, state, |st, name: SharedString| {
        st.network.remove_custom_role(&name);
    }));
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
    use crate::model::Role;

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
    fn adding_a_role_that_is_already_shown_selects_it_instead() {
        let mut state = state_with(10);
        let shown = shown_roles(&state);
        let before = state.network.roles().len();
        state.add_role(" serveur ", DeviceKind::Tv, &shown);
        assert_eq!(selected_role(&state), Some(Role::Server));
        assert_eq!(state.network.roles().len(), before, "nothing added");
    }

    #[test]
    fn adding_a_custom_role_gives_the_device_that_role_and_its_icon() {
        let mut state = state_with(10);
        let shown = shown_roles(&state);
        state.add_role("Media", DeviceKind::Server, &shown);
        assert_eq!(selected_role(&state), Some(Role::Custom("Media".into())));
        assert_eq!(
            state.network.roles().last(),
            Some(&Role::Custom("Media".into()))
        );
        let device = state.network.device(DeviceId(10)).unwrap();
        assert_eq!(state.network.icon(device), DeviceKind::Server);
    }

    #[test]
    fn a_custom_role_cannot_take_a_standard_role_name() {
        let mut state = state_with(10);
        let shown = shown_roles(&state);
        state.rename_role("TV", "imprimante", &shown);
        assert_eq!(selected_role(&state), Some(Role::Custom("TV".into())));
        state.rename_role("TV", "tv", &shown);
        assert_eq!(
            selected_role(&state),
            Some(Role::Custom("tv".into())),
            "own name"
        );
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
        state.add_role("Media", DeviceKind::Tv, &[]);
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
