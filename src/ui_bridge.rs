//! Connects the Slint UI to the model: pushes state into the `Bridge` global and handles the
//! callbacks it raises. Rust owns the data; after every change the affected views are re-sent.
//! Rust sends data rather than wording, so all user-visible text can be translated in Slint.
//!
//! This module owns the state, the selection and the list's search, filter and folding. The
//! rest is split by area: [`edit`] handles editing, [`map`] the topology map, [`discovery`] the
//! discovery page's simulated scan, [`view`] sends
//! the state to the UI, [`convert`] translates between model and UI types, and [`icons`] reads
//! the icon folder.

mod convert;
mod discovery;
mod edit;
mod i18n;
mod icons;
mod map;
mod view;

pub(crate) use i18n::select_language;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString, Weak};

use crate::model::{
    Arrangement, CardId, Device, DeviceId, Discovery, Network, NetworkCard, Positions,
    StatusFilter, tree_layout,
};
use crate::ui::{AppWindow, Bridge};
use icons::Icons;
use map::Motion;

/// What is selected: a device, picked in the lists, or one of its network cards, picked in
/// the lists or on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Device(DeviceId),
    Card(CardId),
}

/// Everything the window shows, owned by `main` and shared with the callbacks.
pub(crate) struct AppState {
    network: Network,
    /// Where the layout puts each card, and the dragged one at the pointer.
    positions: Positions,
    /// Where each card is shown, on its way to its spot in `positions`.
    motion: Motion,
    /// How the user arranged cards down each trunk, until the layout is reset.
    arrangement: Arrangement,
    /// The card being dragged on the map, until it is dropped.
    dragging: Option<CardId>,
    selected: Option<Selection>,
    /// Devices whose group is folded up in the lists, hiding their cards.
    collapsed: HashSet<DeviceId>,
    filter: StatusFilter,
    query: String,
    /// The discovery page's simulated scan; its finishing runs count as scans.
    discovery: Discovery,
    scans: u32,
    /// Read from the icon folder by [`connect`], and again when the icon picker opens.
    icons: Icons,
}

impl AppState {
    pub(crate) fn new(network: Network) -> Self {
        let arrangement = Arrangement::default();
        let positions = tree_layout(&network, &arrangement);
        Self {
            network,
            motion: Motion::new(positions.clone()),
            positions,
            arrangement,
            dragging: None,
            selected: None,
            collapsed: HashSet::new(),
            filter: StatusFilter::All,
            query: String::new(),
            discovery: Discovery::default(),
            scans: 0,
            icons: Icons::default(),
        }
    }

    /// The selected device, or the device of the selected card.
    fn selected_device(&self) -> Option<DeviceId> {
        match self.selected? {
            Selection::Device(id) => Some(id),
            Selection::Card(card) => self.network.card_owner(card).map(|d| d.id),
        }
    }

    fn selected_card(&self) -> Option<CardId> {
        match self.selected? {
            Selection::Card(card) => Some(card),
            Selection::Device(_) => None,
        }
    }

    /// Edits the selected card.
    fn edit_selected_card(&mut self, edit: impl FnOnce(&mut NetworkCard)) {
        if let Some(card) = self.selected_card().and_then(|c| self.network.card_mut(c)) {
            edit(card);
        }
    }

    /// Edits the selected device, or the device of the selected card.
    fn edit_selected(&mut self, edit: impl FnOnce(&mut Device)) {
        if let Some(device) = self
            .selected_device()
            .and_then(|id| self.network.device_mut(id))
        {
            edit(device);
        }
    }

    /// Folds a device's group up in the lists, or opens it again.
    fn toggle_group(&mut self, id: DeviceId) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
    }
}

/// Wires every `Bridge` callback to the state and shows the initial state.
pub(crate) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    bridge.on_select(handler(ui, state, |st, id| {
        st.selected = convert::device_id(id).map(Selection::Device);
    }));
    bridge.on_select_card(handler(ui, state, |st, id| {
        st.selected = convert::card_id(id).map(Selection::Card);
    }));
    bridge.on_toggle_group(handler(ui, state, |st, id| {
        if let Some(id) = convert::device_id(id) {
            st.toggle_group(id);
        }
    }));
    bridge.on_search(handler(ui, state, |st, text: SharedString| {
        st.query = text.into();
    }));
    bridge.on_set_filter(handler(ui, state, |st, f| {
        st.filter = convert::filter_from_ui(f);
    }));
    edit::connect(ui, state);
    map::connect(ui, state);
    discovery::connect(ui, state);
    let (icons_ui, icons_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_refresh_icons(move || icons::refresh(&icons_ui, &icons_state));

    // Shows the initial state, with the icons.
    icons::refresh(&ui.as_weak(), state);
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
    view::push(&ui, &state.borrow());
}

/// Shared by the tests of every area.
#[cfg(test)]
impl AppState {
    /// The sample network with the device `id` selected.
    fn with_selected(id: u32) -> Self {
        let mut state = Self::new(crate::mock_data::network());
        state.selected = Some(Selection::Device(DeviceId(id)));
        state
    }

    fn selected_role(&self) -> Option<crate::model::Role> {
        let device = self
            .selected_device()
            .and_then(|id| self.network.device(id));
        device.map(|d| d.role.clone())
    }

    /// Where a role sits in the role dropdown.
    fn role_index(&self, role: &crate::model::Role) -> usize {
        let roles = self.network.roles();
        roles.iter().position(|r| r == role).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    #[test]
    fn a_group_folds_up_and_opens_again() {
        let mut state = AppState::new(mock_data::network());
        state.toggle_group(DeviceId(1));
        assert!(state.collapsed.contains(&DeviceId(1)));
        state.toggle_group(DeviceId(1));
        assert!(state.collapsed.is_empty());
    }
}
