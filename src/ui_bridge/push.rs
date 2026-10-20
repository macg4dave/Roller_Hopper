//! Sends the state to the window's `Bridge` global after every change.

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use super::{AppState, convert, lines};
use crate::model::{Device, Ghost, Network, Role, StatusFilter};
use crate::ui::{self, AppWindow, Bridge, Labels};

/// Sends the whole state to the `Bridge` global. Cheap for a home network, and keeps every view
/// consistent; models are updated row by row so nodes being dragged are not recreated.
pub(super) fn push(ui: &AppWindow, state: &AppState) {
    let bridge = ui.global::<Bridge<'_>>();
    let labels = ui.global::<Labels<'_>>();
    let (network, icons) = (&state.network, &state.icons);
    // Rows carry the settled spot, which changes only when the layout does, so the labels are
    // not redone on every frame of motion; where each device is drawn goes in `places`.
    let settled = |id| state.positions.get(&id).copied().unwrap_or_default();
    let view = |d: &Device| {
        let role = convert::role_label(&labels, network, &d.role);
        convert::device_view(network, icons, d, role, settled(d.id))
    };

    let node = |d: &Device| {
        let shown = state.filter.admits(d.status) && network.matches(d, &state.query);
        ui::DeviceView {
            dimmed: !shown,
            ..view(d)
        }
    };
    let nodes = network.devices().iter().map(node);
    if let Some(model) = sync(&bridge.get_devices(), nodes.collect()) {
        bridge.set_devices(model);
    }
    let ghosts = state.ghosts();
    let ghost_views = ghosts.iter().filter_map(|g| {
        let device = network.device(g.interface.device)?;
        Some(convert::ghost_view(node(device), g))
    });
    if let Some(model) = sync(&bridge.get_ghosts(), ghost_views.collect()) {
        bridge.set_ghosts(model);
    }
    push_map(&bridge, state, &ghosts);
    let list = network.filtered(state.filter, &state.query);
    if let Some(model) = sync(&bridge.get_list(), list.map(view).collect()) {
        bridge.set_list(model);
    }
    push_roles(&bridge, &labels, state);
    bridge.set_filter(convert::filter_to_ui(state.filter));
    bridge.set_count_all(count(network, StatusFilter::All));
    bridge.set_count_online(count(network, StatusFilter::Online));
    bridge.set_count_offline(count(network, StatusFilter::Offline));
    bridge.set_scanning(state.scanning);
    bridge.set_scans(i32::try_from(state.scans).unwrap_or(i32::MAX));
    push_selection(&bridge, &labels, state);
}

/// Where each device is drawn right now, the lines between them and to the `ghosts`, and the
/// area they take up.
fn push_map(bridge: &Bridge<'_>, state: &AppState, ghosts: &[Ghost]) {
    let on_map = state.motion.shown();
    let trunks = state.trunks();
    let places = state.network.devices().iter().map(|d| {
        let at = on_map.get(&d.id).copied().unwrap_or_default();
        let root = trunks
            .iter()
            .find(|(id, _)| *id == d.id)
            .map(|(_, [_, root])| root);
        ui::NodePlace {
            x: at.x,
            y: at.y,
            has_root: root.is_some(),
            root_y: root.map_or(0.0, |r| r.y),
        }
    });
    if let Some(model) = sync(&bridge.get_places(), places.collect()) {
        bridge.set_places(model);
    }

    let highlighted = state.dragging.or(state.selected);
    let lines = lines::lines(&state.network, &state.links(), &trunks, highlighted);
    bridge.set_strand(convert::strand(
        state.strand().as_ref(),
        state.motion.bend(),
    ));
    if let Some(model) = sync(&bridge.get_lines(), lines) {
        bridge.set_lines(model);
    }
    bridge.set_jump(lines::jumps(on_map, ghosts, highlighted).into());
    let nodes_at = on_map.values().copied().chain(ghosts.iter().map(|g| g.at));
    bridge.set_bounds(convert::bounds(nodes_at));
}

/// The role dropdown's names, the role list's rows and the groups they can be in.
fn push_roles(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let network = &state.network;
    let roles = network.roles();
    let names: Vec<SharedString> = roles
        .iter()
        .map(|r| convert::role_label(labels, network, r))
        .collect();
    let rows = roles
        .iter()
        .zip(&names)
        .map(|(role, name)| convert::role_view(network, &state.icons, role, name.clone()))
        .collect();
    if let Some(model) = sync(&bridge.get_role_list(), rows) {
        bridge.set_role_list(model);
    }
    if let Some(model) = sync(&bridge.get_roles(), names) {
        bridge.set_roles(model);
    }
    bridge.set_roles_restorable(network.standard_roles_changed());

    let groups = convert::group_views(network);
    let choices = std::iter::once(labels.invoke_no_group())
        .chain(groups.iter().map(|g| g.name.clone()))
        .collect();
    if let Some(model) = sync(&bridge.get_group_choices(), choices) {
        bridge.set_group_choices(model);
    }
    if let Some(model) = sync(&bridge.get_groups(), groups) {
        bridge.set_groups(model);
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
        let role = convert::role_label(labels, &state.network, &device.role);
        bridge.set_details(convert::details(&state.network, &state.icons, device, role));
        let roles = state.network.roles();
        let index = roles
            .iter()
            .position(|r: &Role| *r == device.role)
            .unwrap_or_default();
        bridge.set_role_index(i32::try_from(index).unwrap_or_default());
    }
    let ports = selected.map(convert::port_views).unwrap_or_default();
    if let Some(model) = sync(&bridge.get_ports(), ports) {
        bridge.set_ports(model);
    }
    let addresses = selected
        .map(|d| convert::other_addresses(&state.network, d))
        .unwrap_or_default();
    if let Some(model) = sync(&bridge.get_other_addresses(), addresses) {
        bridge.set_other_addresses(model);
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
