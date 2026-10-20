//! Sends the state to the window's `Bridge` global after every change: the lists, the roles and
//! groups, the selection and its network cards, a card move waiting to be confirmed, and the
//! discovery page. The map sends its own
//! part (see [`super::map::push`]); the network cards' views are built in [`cards`], and the
//! discovery page's in [`discovery`].

mod cards;
mod discovery;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use super::edit::CardMove;
use super::{AppState, convert, map};
use crate::model::{CardId, Device, Network, Parent, Role, StatusFilter};
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
    map::push(&bridge, state);
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
    discovery::push(&bridge, &labels, state);
    bridge.set_scans(i32::try_from(state.scans).unwrap_or(i32::MAX));
    push_selection(&bridge, &labels, state);
    push_card(&bridge, &labels, state);
    push_move_question(&bridge, &labels, state);
}

/// Builds the rows of the lists and the nodes of the map.
struct Views<'a> {
    state: &'a AppState,
    labels: &'a Labels<'a>,
}

impl Views<'_> {
    /// The row of `device`, carrying the settled spot of `card` on the map, which changes only
    /// when the layout does, so the labels are not redone on every frame of motion; where each
    /// card is drawn goes in `places`.
    fn view(&self, device: &Device, card: Option<CardId>) -> ui::DeviceView {
        let network = &self.state.network;
        let at = card.and_then(|c| self.state.positions.get(&c));
        let at = at.copied().unwrap_or_default();
        let role = convert::role_label(self.labels, network, &device.role);
        convert::device_view(network, &self.state.icons, device, role, at)
    }

    /// The row of each card of `device`.
    fn card_rows(&self, device: &Device) -> Vec<ui::DeviceView> {
        let icons = &self.state.icons;
        let rows = device
            .cards
            .iter()
            .map(|card| cards::card_row(icons, &self.view(device, Some(card.id)), device, card));
        rows.collect()
    }

    /// The map's nodes, one per card in the order of [`crate::model::Network::nodes`], faded
    /// where the search or filter hides their device.
    fn nodes(&self) -> Vec<ui::DeviceView> {
        let state = self.state;
        let nodes = state.network.devices().iter().flat_map(|device| {
            let shown = state.filter.admits(device.status) && device.matches(&state.query);
            let rows = self.card_rows(device).into_iter();
            rows.map(move |row| ui::DeviceView {
                dimmed: !shown,
                ..row
            })
        });
        nodes.collect()
    }

    /// Each of `devices` as the head of its group, followed by its network cards unless the
    /// group is collapsed, as the lists show them. A device's own row centres the map on its
    /// first card.
    fn rows<'d>(&self, devices: impl Iterator<Item = &'d Device>) -> Vec<ui::DeviceView> {
        let rows = devices.flat_map(|device| {
            let expanded = !self.state.collapsed.contains(&device.id);
            let row = ui::DeviceView {
                expanded,
                ..self.view(device, device.primary_card().map(|c| c.id))
            };
            let cards = if expanded {
                self.card_rows(device)
            } else {
                Vec::new()
            };
            std::iter::once(row).chain(cards)
        });
        rows.collect()
    }
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

/// The selected device (that of the selected card, if a card is selected), its cards and its
/// guests.
fn push_selection(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let selected = state
        .selected_device()
        .and_then(|id| state.network.device(id));
    let id = selected.map_or(-1, |d| convert::ui_id(d.id));
    if let Some(device) = selected {
        // The notes buffer is reset only when the selection changes, so typing is never
        // disturbed.
        if bridge.get_selected_id() != id {
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
    let cards = selected
        .map(|d| cards::card_views(&state.network, &state.icons, d))
        .unwrap_or_default();
    if let Some(model) = sync(&bridge.get_cards(), cards) {
        bridge.set_cards(model);
    }
    let guests = selected
        .map(|d| cards::guest_views(&state.network, &state.icons, d.id))
        .unwrap_or_default();
    if let Some(model) = sync(&bridge.get_guests(), guests) {
        bridge.set_guests(model);
    }
    bridge.set_selected_id(id);
}

/// The selected card: its view, its open ports, and what it can be plugged into.
fn push_card(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let network = &state.network;
    let card = state.selected_card();
    let id = card.map_or(-1, convert::ui_card_id);
    let view = card.and_then(|card| {
        let device = network.card_owner(card)?;
        let mut views = cards::card_views(network, &state.icons, device).into_iter();
        views.find(|v| v.id == id)
    });
    bridge.set_card(view.unwrap_or_default());
    let ports = card.and_then(|c| network.card(c)).map(convert::port_views);
    if let Some(model) = sync(&bridge.get_ports(), ports.unwrap_or_default()) {
        bridge.set_ports(model);
    }
    let sockets = state.upstream_choices().into_iter();
    let names = sockets.map(|socket| cards::socket_name(labels, network, socket));
    let choices = std::iter::once(labels.invoke_no_link()).chain(names);
    if let Some(model) = sync(&bridge.get_upstream_choices(), choices.collect()) {
        bridge.set_upstream_choices(model);
    }
    bridge.set_card_plug_index(i32::try_from(state.plug_choice()).unwrap_or_default());
    push_card_parents(bridge, labels, state);
    bridge.set_selected_card(id);
}

/// The devices the selected card can be a child of, and the one it is a child of now.
fn push_card_parents(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let network = &state.network;
    let parents = state.parent_choices();
    let owner = state.selected_card().and_then(|c| network.card_owner(c));
    let at = parents.iter().position(|&p| Some(p) == owner.map(|d| d.id));
    bridge.set_card_parent_index(at.map_or(-1, |at| i32::try_from(at).unwrap_or_default()));
    let devices = parents.iter().filter_map(|&p| network.device(p));
    let names = devices.map(|d| labels.invoke_device_name(d.name.trim().into()));
    if let Some(model) = sync(&bridge.get_parent_choices(), names.collect()) {
        bridge.set_parent_choices(model);
    }
}

/// The move waiting for the user to confirm removing the parent its card leaves, if any.
fn push_move_question(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let network = &state.network;
    let card_name = |name: &str, at: usize| {
        labels.invoke_card_name(name.trim().into(), i32::try_from(at).unwrap_or_default())
    };
    let question = state.pending_move.and_then(|request| match request {
        CardMove::Map { card, to } => {
            let owner = network.card_owner(card)?;
            let at = owner.cards.iter().position(|c| c.id == card)?;
            Some(ui::MoveQuestion {
                shown: true,
                card: card_name(&owner.cards.get(at)?.name, at),
                from: labels.invoke_device_name(owner.name.trim().into()),
                to: discovery::parent_name(labels, state, Parent::Device(to)),
                found: false,
            })
        }
        CardMove::Found { host, card, to } => {
            let found = state.discovery.found().get(host)?;
            Some(ui::MoveQuestion {
                shown: true,
                card: card_name(&found.device.cards.get(card)?.name, card),
                from: labels.invoke_found_name(found.device.name.trim().into()),
                to: discovery::parent_name(labels, state, to),
                found: true,
            })
        }
    });
    bridge.set_move_question(question.unwrap_or_default());
}

fn count(network: &Network, filter: StatusFilter) -> i32 {
    i32::try_from(network.count(filter)).unwrap_or(i32::MAX)
}

/// Updates a `VecModel` in place, touching only rows that changed. Returns a new model to set
/// when the property does not hold a `VecModel` yet (its initial empty value).
pub(super) fn sync<T: Clone + PartialEq + 'static>(
    model: &ModelRc<T>,
    rows: Vec<T>,
) -> Option<ModelRc<T>> {
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
