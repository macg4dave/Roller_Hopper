//! Sends the state to the window's `Bridge` global after every change.

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use super::{AppState, cards, convert, lines};
use crate::model::{Device, Network, Node, Role, StatusFilter};
use crate::ui::{self, AppWindow, Bridge, Labels};

/// Sends the whole state to the `Bridge` global. Cheap for a home network, and keeps every view
/// consistent; models are updated row by row so nodes being dragged are not recreated.
pub(super) fn push(ui: &AppWindow, state: &AppState) {
    let bridge = ui.global::<Bridge<'_>>();
    let labels = ui.global::<Labels<'_>>();
    let network = &state.network;
    let views = Views {
        state,
        labels: &labels,
    };

    if let Some(model) = sync(&bridge.get_nodes(), views.nodes()) {
        bridge.set_nodes(model);
    }
    push_map(&bridge, state);
    let list = views.rows(network.filtered(state.filter, &state.query));
    if let Some(model) = sync(&bridge.get_list(), list) {
        bridge.set_list(model);
    }
    let rows = views.rows(network.devices().iter());
    if let Some(model) = sync(&bridge.get_rows(), rows) {
        bridge.set_rows(model);
    }
    push_roles(&bridge, &labels, state);
    bridge.set_filter(convert::filter_to_ui(state.filter));
    bridge.set_count_all(count(network, StatusFilter::All));
    bridge.set_count_online(count(network, StatusFilter::Online));
    bridge.set_count_offline(count(network, StatusFilter::Offline));
    bridge.set_scanning(state.scanning);
    bridge.set_scans(i32::try_from(state.scans).unwrap_or(i32::MAX));
    push_selection(&bridge, &labels, state);
    push_card(&bridge, &labels, state);
}

/// Builds the rows of the lists and the nodes of the map.
struct Views<'a> {
    state: &'a AppState,
    labels: &'a Labels<'a>,
}

impl Views<'_> {
    /// The row of `device`, carrying the settled spot of `node` (the device's own, or its ghost's
    /// for a card), which changes only when the layout does, so the labels are not redone on
    /// every frame of motion; where each node is drawn goes in `places`.
    fn view(&self, device: &Device, node: Node) -> ui::DeviceView {
        let network = &self.state.network;
        let at = self.state.positions.get(&node).copied().unwrap_or_default();
        let role = convert::role_label(self.labels, network, &device.role);
        convert::device_view(network, &self.state.icons, device, role, at)
    }

    /// The map's nodes, in the order of [`crate::model::Network::nodes`], faded where the search
    /// or filter hides their device.
    fn nodes(&self) -> Vec<ui::DeviceView> {
        let (network, state) = (&self.state.network, self.state);
        let node = |node: Node| {
            let device = network.node_device(node)?;
            let shown = state.filter.admits(device.status) && device.matches(&state.query);
            let row = ui::DeviceView {
                dimmed: !shown,
                ..self.view(device, node)
            };
            Some(match node {
                Node::Device(_) => row,
                Node::Card(id) => {
                    let card = device.cards.iter().find(|c| c.id == id)?;
                    cards::card_row(&row, device, card)
                }
            })
        };
        network.nodes().into_iter().filter_map(node).collect()
    }

    /// Each of `devices` followed by its network cards, as the lists show them.
    fn rows<'d>(&self, devices: impl Iterator<Item = &'d Device>) -> Vec<ui::DeviceView> {
        let network = &self.state.network;
        let rows = devices.flat_map(|device| {
            let row = self.view(device, Node::Device(device.id));
            let card_rows = device.cards.iter().map(|card| {
                let node = network
                    .node_of_card(card.id)
                    .unwrap_or(Node::Device(device.id));
                cards::card_row(&self.view(device, node), device, card)
            });
            std::iter::once(row).chain(card_rows.collect::<Vec<_>>())
        });
        rows.collect()
    }
}

/// Where each node is drawn right now, the lines between them, and the area they take up.
fn push_map(bridge: &Bridge<'_>, state: &AppState) {
    let on_map = state.motion.shown();
    let trunks = state.trunks();
    let places = state.network.nodes().into_iter().map(|node| {
        let at = on_map.get(&node).copied().unwrap_or_default();
        let root = trunks
            .iter()
            .find(|(id, _)| *id == node)
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

    let highlighted = state.dragging.or_else(|| state.selected_node());
    let lines = lines::lines(&state.network, &state.links(), &trunks, highlighted);
    bridge.set_strand(convert::strand(
        state.strand().as_ref(),
        state.motion.bend(),
    ));
    if let Some(model) = sync(&bridge.get_lines(), lines) {
        bridge.set_lines(model);
    }
    bridge.set_bounds(convert::bounds(on_map.values().copied()));
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

/// The selected device (that of the selected card, if a card is selected) and its cards.
fn push_selection(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let selected = state
        .selected_device()
        .and_then(|id| state.network.device(id));
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
    let cards = selected
        .map(|d| cards::card_views(&state.network, d))
        .unwrap_or_default();
    if let Some(model) = sync(&bridge.get_cards(), cards) {
        bridge.set_cards(model);
    }
    bridge.set_selected_id(id);
}

/// The selected card: its view, and what it can be plugged into.
fn push_card(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let network = &state.network;
    let card = state.selected_card();
    let id = card.map_or(-1, convert::ui_card_id);
    let view = card.and_then(|card| {
        let device = network.card_owner(card)?;
        let mut views = cards::card_views(network, device).into_iter();
        views.find(|v| v.id == id)
    });
    bridge.set_card(view.unwrap_or_default());
    bridge.set_selected_ghost(card.is_some_and(|c| network.has_ghost(c)));
    let sockets = state.upstream_choices().into_iter();
    let names = sockets.map(|socket| cards::socket_name(labels, network, socket));
    let choices = std::iter::once(labels.invoke_no_link()).chain(names);
    if let Some(model) = sync(&bridge.get_upstream_choices(), choices.collect()) {
        bridge.set_upstream_choices(model);
    }
    bridge.set_card_plug_index(i32::try_from(state.plug_choice()).unwrap_or_default());
    bridge.set_selected_card(id);
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
