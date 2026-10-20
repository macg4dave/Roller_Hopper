//! Connects the Slint UI to the model: pushes state into the `Bridge` global and handles the
//! callbacks it raises. Rust owns the data; after every change the affected views are re-sent.
//! Rust sends data rather than wording, so all user-visible text can be translated in Slint.

mod convert;
mod i18n;
mod icons;
mod lines;
mod motion;
mod push;

pub(crate) use i18n::select_language;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel, Weak};

use crate::mock_data;
use crate::model::{
    Arrangement, Connection, Device, DeviceId, GroupId, Icon, Network, Point, Positions, Role,
    Snap, StatusFilter, link_route, snap_target, tree_layout, trunk,
};
use crate::ui::{AppWindow, Bridge};
use icons::Icons;
use motion::Motion;
use push::push;

/// How long the simulated scan pretends to take, so the refresh is visible.
const SCAN_DURATION: Duration = Duration::from_millis(1500);
/// One frame of the map's motion, at about 60 frames a second.
const FRAME: Duration = Duration::from_millis(16);

/// Everything the window shows, owned by `main` and shared with the callbacks.
pub(crate) struct AppState {
    network: Network,
    /// Where the layout puts each device, and the dragged one at the pointer.
    positions: Positions,
    /// Where each device is shown, on its way to its spot in `positions`.
    motion: Motion,
    /// How the user arranged devices down each trunk, until the layout is reset.
    arrangement: Arrangement,
    /// The device being dragged on the map, until it is dropped.
    dragging: Option<DeviceId>,
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
        let arrangement = Arrangement::default();
        let positions = tree_layout(&network, &arrangement);
        Self {
            network,
            motion: Motion::new(positions.clone()),
            positions,
            arrangement,
            dragging: None,
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
            self.motion.forget(id);
        }
    }

    /// Drags device `id` so its icon is centred on `at`.
    fn drag_to(&mut self, id: DeviceId, at: Point) {
        self.positions.insert(id, at);
        self.motion.place(id, at);
        self.dragging = Some(id);
    }

    /// Where the dragged device `id` would go if dropped now, judged by the map as shown: on the
    /// trunk it is near, where it is on that trunk; `None` to stand alone.
    fn snap(&self, id: DeviceId) -> Option<Snap> {
        let shown = self.motion.shown();
        let at = shown.get(&id).copied()?;
        snap_target(&self.network, &self.arrangement, shown, id, at)
    }

    /// The trunk of every device as shown. A dragged branch's trunks are hidden until it is
    /// dropped, and the other trunks do not stretch after it.
    /// In device order, so the lines sent to the UI only change when the map does.
    fn trunks(&self) -> Vec<(DeviceId, [Point; 2])> {
        let shown = self.motion.shown();
        let dragged = |id| self.dragging.is_some_and(|d| self.network.is_within(id, d));
        let still: Positions = shown
            .iter()
            .filter(|&(&id, _)| !dragged(id))
            .map(|(&id, &p)| (id, p))
            .collect();
        let ids = self.network.devices().iter().map(|d| d.id);
        ids.filter_map(|id| Some((id, trunk(&self.network, &still, id)?)))
            .collect()
    }

    /// The connections to draw: all but the dragged device's uplink, which [`Self::strand`]
    /// stands in for.
    /// Each runs along its route as shown, branching off its neighbour's line, with the dragged
    /// device left out of the rows it was in.
    fn links(&self) -> Vec<(Connection, [Point; 3])> {
        let shown = self.motion.shown();
        let inner = self
            .arrangement
            .inner_neighbours(&self.network, self.dragging);
        let links = self.network.connections().iter();
        links
            .filter(|c| Some(c.downstream) != self.dragging)
            .filter_map(|&c| Some((c, link_route(shown, &inner, c.upstream, c.downstream)?)))
            .collect()
    }

    /// While a device is dragged near a line, how it would join it.
    fn strand(&self) -> Option<Snap> {
        self.dragging.and_then(|id| self.snap(id))
    }

    /// Ends a drag: the device hangs from the trunk it was dropped on, where it was dropped, or
    /// stands alone, and the map is laid out again around the change. The devices on the trunk
    /// it joined bob as it settles in.
    fn drop_dragged(&mut self) {
        let Some(id) = self.dragging.take() else {
            return;
        };
        let snap = self.snap(id);
        self.network.connect(id, snap.map(|s| s.upstream));
        match snap {
            Some(snap) => {
                let arrangement = &mut self.arrangement;
                arrangement.place(&self.network, id, snap.upstream, snap.slot);
                let others = self.network.behind(snap.upstream).filter(|&d| d != id);
                self.motion.jiggle(others.chain([snap.upstream]));
            }
            None => self.arrangement.remove(id),
        }
        self.lay_out();
    }

    fn lay_out(&mut self) {
        self.positions = tree_layout(&self.network, &self.arrangement);
    }

    /// Back to the connections as found, laid out afresh with nothing the user arranged.
    fn reset_layout(&mut self) {
        self.network.restore_connections();
        self.arrangement.clear();
        self.lay_out();
    }

    /// Moves the map one frame on; true once everything has settled.
    fn step_motion(&mut self) -> bool {
        let dt = FRAME.as_secs_f32();
        let strand = self.strand();
        self.motion.step(&self.positions, dt);
        self.motion.step_strand(strand.as_ref(), dt);
        self.motion.settled(&self.positions)
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
    connect_map(ui, state);

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

/// Dragging, dropping and resetting on the map, each followed by the motion that settles it.
fn connect_map(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let timer = Rc::new(Timer::default());
    let settle = {
        let (ui, state) = (ui.as_weak(), Rc::clone(state));
        Rc::new(move || animate(&ui, &state, &timer))
    };

    let reset = handler(ui, state, |st, ()| st.reset_layout());
    let reset_settle = Rc::clone(&settle);
    bridge.on_reset_layout(move || {
        reset(());
        reset_settle();
    });
    let move_device = handler(ui, state, |st, (id, x, y)| {
        if let Some(id) = convert::device_id(id) {
            st.drag_to(id, Point { x, y });
        }
    });
    // While dragging, the strand swings after the pointer.
    let move_settle = Rc::clone(&settle);
    bridge.on_move_device(move |id, x, y| {
        move_device((id, x, y));
        move_settle();
    });
    let drop_device = handler(ui, state, |st, ()| st.drop_dragged());
    bridge.on_drop_device(move || {
        drop_device(());
        settle();
    });
}

/// Steps the map's motion every frame until everything has settled (or the window closed).
fn animate(ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>, timer: &Rc<Timer>) {
    if timer.running() {
        return;
    }
    let (ui, state, this) = (ui.clone(), Rc::clone(state), Rc::downgrade(timer));
    timer.start(TimerMode::Repeated, FRAME, move || {
        let mut settled = true;
        update(&ui, &state, |st| settled = st.step_motion());
        if let Some(running) = this.upgrade().filter(|_| settled) {
            running.stop();
        }
    });
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
    let set_style = handler(ui, state, |st, (index, choice)| {
        let Some(style) = convert::line_style_from_ui(choice) else {
            return;
        };
        st.edit_group(convert::index(index), |n, group| {
            n.set_group_line_style(group, style);
        });
    });
    bridge.on_set_group_line_style(move |index, choice| set_style((index, choice)));
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
    fn a_dropped_device_joins_the_line_it_was_previewing_on() {
        let mut state = AppState::new(mock_data::network());
        // The NAS hangs left of the router's trunk; the switch and access point to its right.
        let (router, switch, access_point, nas) =
            (DeviceId(1), DeviceId(2), DeviceId(3), DeviceId(4));
        state.drop_dragged();
        assert_eq!(
            state.positions,
            tree_layout(&state.network, &state.arrangement),
            "no drag, no change"
        );

        // Dropped between the main switch and the access point.
        let x = |shown: &AppState, id| shown.positions[&id].x;
        let gap = |shown: &AppState| x(shown, access_point) - x(shown, switch);
        let gap_before = gap(&state);
        let between = Point {
            x: f32::midpoint(x(&state, switch), x(&state, access_point)),
            y: state.positions[&switch].y,
        };
        state.drag_to(nas, between);
        assert!(
            state.links().iter().all(|(c, _)| c.downstream != nas),
            "old line hidden"
        );
        assert!(state.strand().is_some());

        state.drop_dragged();
        assert_eq!(state.dragging, None);
        assert_eq!(state.network.uplink(nas).map(|c| c.upstream), Some(router));
        assert_eq!(
            state.positions,
            tree_layout(&state.network, &state.arrangement),
            "tidied again"
        );
        assert!(x(&state, switch) < x(&state, nas) && x(&state, nas) < x(&state, access_point));
        assert!(gap(&state) > gap_before, "pushed apart");

        assert!(!state.step_motion(), "glides into place");
        assert!((0..600).any(|_| state.step_motion()), "and settles");
        assert_eq!(state.motion.shown(), &state.positions);
    }

    #[test]
    fn a_device_dropped_far_from_every_line_stands_alone() {
        let mut state = AppState::new(mock_data::network());
        let pc = DeviceId(5);
        state.drag_to(
            pc,
            Point {
                x: -1000.0,
                y: -1000.0,
            },
        );
        assert!(state.links().iter().all(|(c, _)| c.downstream != pc));
        assert_eq!(state.strand(), None);
        state.drop_dragged();
        assert!(state.network.uplink(pc).is_none());
    }

    #[test]
    fn a_device_dropped_on_a_trunk_below_a_row_hangs_below_it_until_reset() {
        let mut state = AppState::new(mock_data::network());
        let (router, switch, nas) = (DeviceId(1), DeviceId(2), DeviceId(4));
        let lowest_of_switch_row = |shown: &AppState| {
            let behind = shown.network.behind(switch).filter(|&d| d != nas);
            behind
                .map(|d| shown.positions[&d].y)
                .fold(f32::MIN, f32::max)
        };
        let row = lowest_of_switch_row(&state);

        // Just left of the switch's trunk, below its row.
        let below_left = Point {
            x: state.positions[&switch].x - 10.0,
            y: row + 90.0,
        };
        state.drag_to(nas, below_left);
        state.drop_dragged();
        assert_eq!(state.network.uplink(nas).map(|c| c.upstream), Some(switch));
        assert!(state.positions[&nas].y > lowest_of_switch_row(&state));
        assert!(state.positions[&nas].x < state.positions[&switch].x);
        assert!(
            state.trunks().iter().any(|(id, _)| *id == nas),
            "with a trunk of its own"
        );

        state.reset_layout();
        assert_eq!(state.network.uplink(nas).map(|c| c.upstream), Some(router));
        assert_eq!(state.arrangement, Arrangement::default());
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
