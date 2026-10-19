//! Connects the Slint UI to the model: pushes state into the `Bridge` global and handles the
//! callbacks it raises. Rust owns the data; after every change the affected views are re-sent.

mod convert;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, VecModel, Weak};

use crate::mock_data;
use crate::model::{DeviceId, Network, Point, Positions, Role, StatusFilter, tree_layout};
use crate::ui::{AppWindow, Bridge};

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

    fn edit_selected(&mut self, edit: impl FnOnce(&mut crate::model::Device)) {
        if let Some(device) = self.selected.and_then(|id| self.network.device_mut(id)) {
            edit(device);
        }
    }

    fn choose_role(&mut self, index: usize) {
        if let Some(role) = self.network.roles().into_iter().nth(index) {
            self.edit_selected(|d| d.role = role);
        }
    }

    fn add_role(&mut self, name: &str) {
        if let Some(role) = self.network.add_custom_role(name) {
            self.edit_selected(|d| d.role = role);
        }
    }

    fn finish_scan(&mut self) {
        mock_data::simulate_scan(&mut self.network);
        self.scanning = false;
        self.scans += 1;
    }

    fn status_text(&self) -> String {
        let total = self.network.count(StatusFilter::All);
        let offline = self.network.count(StatusFilter::Offline);
        let summary = format!("{total} devices, {offline} offline");
        match (self.scanning, self.scans) {
            (true, _) => "Simulating a scan of the sample network…".to_owned(),
            (false, 0) => format!("Sample network loaded · {summary}"),
            (false, _) => format!("Simulated scan complete · {summary}"),
        }
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
    bridge.on_rename(handler(ui, state, |st, name: SharedString| {
        st.edit_selected(|d| d.name = name.into());
    }));
    bridge.on_edit_notes(handler(ui, state, |st, notes: SharedString| {
        st.edit_selected(|d| d.notes = notes.into());
    }));
    bridge.on_choose_role(handler(ui, state, |st, index| {
        st.choose_role(usize::try_from(index).unwrap_or(usize::MAX));
    }));
    bridge.on_add_role(handler(ui, state, |st, name: SharedString| {
        st.add_role(&name);
    }));
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

/// Sends the whole state to the `Bridge` global. Cheap for a home network, and keeps every view
/// consistent; models are updated row by row so nodes being dragged are not recreated.
fn push(ui: &AppWindow, state: &AppState) {
    let bridge = ui.global::<Bridge<'_>>();
    let network = &state.network;
    let at = |id| state.positions.get(&id).copied().unwrap_or_default();

    let nodes = network.devices().iter().map(|d| {
        let shown = state.filter.admits(d.status) && d.matches(&state.query);
        convert::device_view(d, at(d.id), !shown)
    });
    if let Some(model) = sync(&bridge.get_devices(), nodes.collect()) {
        bridge.set_devices(model);
    }
    let list = network.filtered(state.filter, &state.query);
    let list = list
        .map(|d| convert::device_view(d, at(d.id), false))
        .collect();
    if let Some(model) = sync(&bridge.get_list(), list) {
        bridge.set_list(model);
    }
    let roles: Vec<SharedString> = network
        .roles()
        .iter()
        .map(|r| r.to_string().into())
        .collect();
    if let Some(model) = sync(&bridge.get_roles(), roles) {
        bridge.set_roles(model);
    }

    bridge.set_lines(convert::lines(network, &state.positions, state.selected));
    bridge.set_bounds(convert::bounds(&state.positions));
    bridge.set_filter(convert::filter_to_ui(state.filter));
    bridge.set_count_all(count(network, StatusFilter::All));
    bridge.set_count_online(count(network, StatusFilter::Online));
    bridge.set_count_offline(count(network, StatusFilter::Offline));
    bridge.set_scanning(state.scanning);
    bridge.set_status_text(state.status_text().into());
    push_selection(&bridge, state);
}

fn push_selection(bridge: &Bridge<'_>, state: &AppState) {
    let selected = state.selected.and_then(|id| state.network.device(id));
    let id = selected.map_or(-1, |d| convert::ui_id(d.id));
    if let Some(device) = selected {
        // Edit buffers are reset only when the selection changes, so typing is never disturbed.
        if bridge.get_selected_id() != id {
            bridge.set_name_draft(device.name.as_str().into());
            bridge.set_notes_draft(device.notes.as_str().into());
        }
        bridge.set_details(convert::details(&state.network, device));
        let roles = state.network.roles();
        let index = roles
            .iter()
            .position(|r: &Role| *r == device.role)
            .unwrap_or_default();
        bridge.set_role_index(i32::try_from(index).unwrap_or_default());
    }
    bridge.set_selected_id(id);
}

fn count(network: &Network, filter: StatusFilter) -> i32 {
    i32::try_from(network.count(filter)).unwrap_or(i32::MAX)
}

/// Updates a `VecModel` in place, touching only rows that changed. Returns a new model to set
/// when the property does not hold a `VecModel` yet (its initial empty value).
fn sync<T: Clone + PartialEq + 'static>(model: &ModelRc<T>, rows: Vec<T>) -> Option<ModelRc<T>> {
    let Some(vec) = model.as_any().downcast_ref::<VecModel<T>>() else {
        return Some(ModelRc::new(VecModel::from(rows)));
    };
    if vec.row_count() == rows.len() {
        for (i, row) in rows.into_iter().enumerate() {
            if vec.row_data(i).as_ref() != Some(&row) {
                vec.set_row_data(i, row);
            }
        }
    } else {
        vec.set_vec(rows);
    }
    None
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

    #[test]
    fn adding_a_custom_role_selects_it_for_the_device() {
        let mut state = state_with(10);
        state.add_role("Media");
        assert_eq!(selected_role(&state), Some(Role::Custom("Media".into())));
        assert_eq!(
            state.network.roles().last(),
            Some(&Role::Custom("Media".into()))
        );
    }

    #[test]
    fn edits_without_a_selection_change_nothing() {
        let mut state = AppState::new(mock_data::network());
        let before = state.network.clone();
        state.edit_selected(|d| d.name = "Changed".into());
        state.add_role("Media");
        assert_eq!(state.network.devices(), before.devices());
    }

    #[test]
    fn scan_status_never_claims_a_real_scan() {
        let mut state = AppState::new(mock_data::network());
        state.scanning = true;
        assert!(state.status_text().starts_with("Simulating"));
        state.finish_scan();
        assert!(!state.scanning);
        assert!(state.status_text().starts_with("Simulated scan complete"));
    }
}
