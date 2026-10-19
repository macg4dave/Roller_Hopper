//! Sends the state to the window's `Bridge` global after every change.

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use super::{AppState, convert};
use crate::model::{Device, Network, Role, StatusFilter};
use crate::ui::{AppWindow, Bridge, Labels};

/// Sends the whole state to the `Bridge` global. Cheap for a home network, and keeps every view
/// consistent; models are updated row by row so nodes being dragged are not recreated.
pub(super) fn push(ui: &AppWindow, state: &AppState) {
    let bridge = ui.global::<Bridge<'_>>();
    let labels = ui.global::<Labels<'_>>();
    let network = &state.network;
    let at = |id| state.positions.get(&id).copied().unwrap_or_default();
    let view = |d: &Device, dimmed| {
        let role = convert::role_label(&labels, &d.role);
        convert::device_view(d, role, network.icon(d), at(d.id), dimmed)
    };

    let nodes = network.devices().iter().map(|d| {
        let shown = state.filter.admits(d.status) && d.matches(&state.query);
        view(d, !shown)
    });
    if let Some(model) = sync(&bridge.get_devices(), nodes.collect()) {
        bridge.set_devices(model);
    }
    let list = network.filtered(state.filter, &state.query);
    let list = list.map(|d| view(d, false)).collect();
    if let Some(model) = sync(&bridge.get_list(), list) {
        bridge.set_list(model);
    }
    push_roles(&bridge, &labels, network);

    bridge.set_lines(convert::lines(network, &state.positions, state.selected));
    bridge.set_bounds(convert::bounds(&state.positions));
    bridge.set_filter(convert::filter_to_ui(state.filter));
    bridge.set_count_all(count(network, StatusFilter::All));
    bridge.set_count_online(count(network, StatusFilter::Online));
    bridge.set_count_offline(count(network, StatusFilter::Offline));
    bridge.set_scanning(state.scanning);
    bridge.set_scans(i32::try_from(state.scans).unwrap_or(i32::MAX));
    push_selection(&bridge, &labels, state);
}

/// The role dropdown's names and the Roles tab's rows.
fn push_roles(bridge: &Bridge<'_>, labels: &Labels<'_>, network: &Network) {
    let roles = network.roles();
    let names: Vec<SharedString> = roles
        .iter()
        .map(|r| convert::role_label(labels, r))
        .collect();
    let rows = roles
        .iter()
        .zip(&names)
        .map(|(role, name)| convert::role_view(network, role, name.clone()))
        .collect();
    if let Some(model) = sync(&bridge.get_role_list(), rows) {
        bridge.set_role_list(model);
    }
    if let Some(model) = sync(&bridge.get_roles(), names) {
        bridge.set_roles(model);
    }
}

fn push_selection(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let selected = state.selected.and_then(|id| state.network.device(id));
    let id = selected.map_or(-1, |d| convert::ui_id(d.id));
    if let Some(device) = selected {
        // Edit buffers are reset only when the selection changes, so typing is never disturbed.
        if bridge.get_selected_id() != id {
            bridge.set_name_draft(device.name.as_str().into());
            bridge.set_notes_draft(device.notes.as_str().into());
        }
        let role = convert::role_label(labels, &device.role);
        bridge.set_details(convert::details(&state.network, device, role));
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
