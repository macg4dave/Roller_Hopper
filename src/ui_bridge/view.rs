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
use crate::model::{AutoMapConfidence, AutoMapReason, CardId, Device, Parent, Role};
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
    push_auto_map(&bridge, state);
    let list = views.rows(network.filtered(&state.query));
    if let Some(model) = sync(&bridge.get_list(), list) {
        bridge.set_list(model);
    }
    push_roles(&bridge, &labels, state);
    push_bridge_groups(&bridge, &labels, state);
    bridge.set_count_all(i32::try_from(network.devices().len()).unwrap_or(i32::MAX));
    discovery::push(&bridge, &labels, state);
    bridge.set_scans(i32::try_from(state.scans).unwrap_or(i32::MAX));
    push_selection(&bridge, &labels, state);
    push_card(&bridge, &labels, state);
    push_move_question(&bridge, &labels, state);
    super::project::push(&bridge, state);
}

fn push_auto_map(bridge: &Bridge<'_>, state: &AppState) {
    let suggestions = state.auto_map.as_ref().map_or_else(Vec::new, |proposal| {
        proposal
            .suggestions
            .iter()
            .map(|suggestion| ui::AutoMapSuggestion {
                downstream: card_owner_name(state, suggestion.downstream).into(),
                upstream: card_owner_name(state, suggestion.upstream).into(),
                confidence: match suggestion.confidence {
                    AutoMapConfidence::Strong => ui::AutoMapConfidence::Strong,
                    AutoMapConfidence::Likely => ui::AutoMapConfidence::Likely,
                    AutoMapConfidence::Tentative => ui::AutoMapConfidence::Tentative,
                },
                reason: match suggestion.reason {
                    AutoMapReason::Gateway => ui::AutoMapReason::Gateway,
                    AutoMapReason::Wireless => ui::AutoMapReason::Wireless,
                    AutoMapReason::Wired => ui::AutoMapReason::Wired,
                    AutoMapReason::Subnet => ui::AutoMapReason::Subnet,
                },
            })
            .collect()
    });
    if let Some(model) = sync(&bridge.get_auto_map_suggestions(), suggestions) {
        bridge.set_auto_map_suggestions(model);
    }
    bridge.set_auto_map_open(state.auto_map.is_some());
    bridge.set_auto_map_unresolved(state.auto_map.as_ref().map_or(0, |proposal| {
        i32::try_from(proposal.unresolved).unwrap_or(i32::MAX)
    }));
    bridge.set_can_undo_auto_map(state.auto_map_undo.is_some());
}

fn card_owner_name(state: &AppState, card: CardId) -> &str {
    state
        .network
        .card_owner(card)
        .map_or("", |device| device.name.as_str())
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
    /// where the search hides their device.
    fn nodes(&self) -> Vec<ui::DeviceView> {
        let state = self.state;
        let nodes = state.network.devices().iter().flat_map(|device| {
            let shown = device.matches(&state.query);
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

    bridge.set_groups_restorable(network.standard_groups_missing());
    let groups = convert::group_views(labels, network);
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

/// Every ordinary network card and the software bridge it belongs to, for map setup.
fn push_bridge_groups(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let network = &state.network;
    let choices = std::iter::once(labels.invoke_no_link()).chain(
        network
            .bridge_groups()
            .iter()
            .map(|name| SharedString::from(name.as_str())),
    );
    if let Some(model) = sync(&bridge.get_bridge_group_choices(), choices.collect()) {
        bridge.set_bridge_group_choices(model);
    }
    let rows = network.nodes().into_iter().map(|card| ui::BridgeGroupView {
        card: convert::ui_card_id(card),
        name: cards::socket_name(labels, network, card),
        bridge_index: i32::try_from(state.bridge_choice(card)).unwrap_or_default(),
    });
    if let Some(model) = sync(&bridge.get_bridge_group_cards(), rows.collect()) {
        bridge.set_bridge_group_cards(model);
    }
}

/// The selected device (that of the selected card, if a card is selected) and its cards.
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
    push_hosts(bridge, labels, state);
    let cards = selected
        .map(|d| cards::card_views(&state.network, &state.icons, d, state.featured_card()))
        .unwrap_or_default();
    if let Some(model) = sync(&bridge.get_cards(), cards) {
        bridge.set_cards(model);
    }
    bridge.set_selected_id(id);
}

/// The devices the selected device can run on, after "none", and the one it runs on now.
fn push_hosts(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let network = &state.network;
    let devices = state.host_choices().into_iter();
    let names = devices
        .filter_map(|id| network.device(id))
        .map(|d| labels.invoke_device_name(d.name.trim().into()));
    let choices = std::iter::once(labels.invoke_no_host()).chain(names);
    if let Some(model) = sync(&bridge.get_host_choices(), choices.collect()) {
        bridge.set_host_choices(model);
    }
    bridge.set_host_index(i32::try_from(state.host_choice()).unwrap_or_default());
}

/// The selected card: its view, its open ports, what it can be plugged into or joined to, and
/// the segment it is in.
fn push_card(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let network = &state.network;
    let card = state.selected_card();
    let id = card.map_or(-1, convert::ui_card_id);
    let view = card.and_then(|card| {
        let device = network.card_owner(card)?;
        let mut views = cards::card_views(network, &state.icons, device, None).into_iter();
        views.find(|v| v.id == id)
    });
    bridge.set_card(view.unwrap_or_default());
    let relations = card.into_iter().flat_map(|selected| {
        network.relations_for(selected).map(move |relation| {
            let other = if relation.card == selected {
                relation.upstream
            } else {
                relation.card
            };
            ui::RelationView {
                endpoint: cards::socket_name(labels, network, other),
                relationship: convert::relationship(relation.relationship),
            }
        })
    });
    if let Some(model) = sync(&bridge.get_card_relations(), relations.collect()) {
        bridge.set_card_relations(model);
    }
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
    // Segment names are the user's own words.
    let segments = network.segments().iter().map(|s| s.name.as_str().into());
    let segment_names = std::iter::once(labels.invoke_no_segment()).chain(segments);
    if let Some(model) = sync(&bridge.get_segment_choices(), segment_names.collect()) {
        bridge.set_segment_choices(model);
    }
    push_card_parents(bridge, labels, state);
    bridge.set_selected_card(id);
    bridge.set_featured_card(state.featured_card().map_or(-1, convert::ui_card_id));
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

/// Updates a `VecModel` in place, touching only rows that changed. Returns a new model to set
/// when the property does not hold a `VecModel` yet (its initial empty value).
pub(crate) fn sync<T: Clone + PartialEq + 'static>(
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
