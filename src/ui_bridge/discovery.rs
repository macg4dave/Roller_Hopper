//! The discovery page: a simulated scan, stepped on by a timer, and editing the hosts it finds
//! and adding them to the map. The run itself is [`crate::model::Discovery`]; [`super::view`] sends it to the UI.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use ipnet::Ipv4Net;
use slint::{ComponentHandle, SharedString, Timer, TimerMode, Weak};

use super::{AppState, Selection, convert, handler, update};
use crate::mock_data;
use crate::model::{Found, FoundFilter, Phase};
use crate::ui::{self, AppWindow, Bridge};

/// One step of a run, so a whole run takes about six seconds.
pub(super) const STEP: Duration = Duration::from_millis(150);

impl AppState {
    /// The subnet the page shows and the next run scans: the one picked, or else the first.
    pub(super) fn discovery_target(&self) -> Option<Ipv4Net> {
        let subnets = self.network.subnets();
        let picked = self.discovery.target.filter(|t| subnets.contains(t));
        picked.or_else(|| subnets.first().copied())
    }

    /// Starts a run, unless one is busy. Which devices answer it is decided as it starts, by
    /// the sample network's fake refresh.
    fn start_discovery(&mut self) {
        let Some(target) = self.discovery_target().filter(|_| !self.discovery.busy()) else {
            return;
        };
        mock_data::simulate_scan(&mut self.network);
        let answers = mock_data::answers(&self.network);
        self.discovery.start(target, answers, &self.network);
    }

    /// One step of the run, counting the scan once it finishes. True while it is running.
    fn step_discovery(&mut self) -> bool {
        if self.discovery.step() {
            self.scans += 1;
        }
        self.discovery.phase() == Phase::Running
    }

    fn stop_discovery(&mut self) {
        if self.discovery.stop() {
            self.scans += 1;
        }
    }

    /// Shows a found host in the details. One on the map is selected there too, so the details
    /// edit the device on the map.
    fn select_found(&mut self, index: Option<usize>) {
        self.discovery.selected = index;
        let found = index.and_then(|i| self.discovery.found().get(i));
        if let Some(id) = found.and_then(Found::on_map) {
            self.selected = Some(Selection::Device(id));
        }
    }

    /// Adds a found host to the map and lays it out; the map is out of sight meanwhile. It is
    /// selected, so the details go on editing it on the map.
    fn add_found(&mut self, index: usize) {
        if let Some(id) = self.discovery.add(index, &mut self.network) {
            self.selected = Some(Selection::Device(id));
            self.lay_out_at_rest();
        }
    }

    fn set_found_role(&mut self, index: usize, role: usize) {
        if let Some(role) = self.role(role) {
            self.discovery.edit(index, |d| d.role = role);
        }
    }

    fn set_discovery_option(&mut self, option: ui::DiscoveryOption, on: bool) {
        let options = &mut self.discovery.options;
        match option {
            ui::DiscoveryOption::PingSweep => options.ping_sweep = on,
            ui::DiscoveryOption::PortScan => options.port_scan = on,
            ui::DiscoveryOption::ServiceDetection => options.service_detection = on,
        }
    }
}

fn filter_from_ui(filter: ui::FoundFilter) -> FoundFilter {
    match filter {
        ui::FoundFilter::All => FoundFilter::All,
        ui::FoundFilter::New => FoundFilter::New,
        ui::FoundFilter::Unidentified => FoundFilter::Unidentified,
        ui::FoundFilter::Duplicates => FoundFilter::Duplicates,
    }
}

/// The run's controls and options, the result filter, and what is done with a found host.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let timer = Rc::new(Timer::default());
    let run = {
        let (ui, state) = (ui.as_weak(), Rc::clone(state));
        Rc::new(move || run(&ui, &state, &timer))
    };

    let start = handler(ui, state, |st, ()| st.start_discovery());
    let start_run = Rc::clone(&run);
    bridge.on_start_discovery(move || {
        start(());
        start_run();
    });
    let pause = handler(ui, state, |st, ()| st.discovery.toggle_pause());
    bridge.on_pause_discovery(move || {
        pause(());
        run();
    });
    let stop = handler(ui, state, |st, ()| st.stop_discovery());
    bridge.on_stop_discovery(move || stop(()));

    bridge.on_set_discovery_target(handler(ui, state, |st, index| {
        st.discovery.target = st.network.subnets().get(convert::index(index)).copied();
    }));
    let set_option = handler(ui, state, |st, (option, on)| {
        st.set_discovery_option(option, on);
    });
    bridge.on_set_discovery_option(move |option, on| set_option((option, on)));
    bridge.on_set_found_filter(handler(ui, state, |st, filter| {
        st.discovery.filter = filter_from_ui(filter);
    }));
    bridge.on_select_found(handler(ui, state, |st, index| {
        st.select_found(usize::try_from(index).ok());
    }));
    let set_role = handler(ui, state, |st, (index, role)| {
        st.set_found_role(convert::index(index), convert::index(role));
    });
    bridge.on_set_found_role(move |index, role| set_role((index, role)));
    let rename = handler(ui, state, |st, (index, name): (i32, SharedString)| {
        st.discovery
            .edit(convert::index(index), |d| d.name = name.into());
    });
    bridge.on_rename_found(move |index, name| rename((index, name)));
    let edit_notes = handler(ui, state, |st, (index, notes): (i32, SharedString)| {
        st.discovery
            .edit(convert::index(index), |d| d.notes = notes.into());
    });
    bridge.on_edit_found_notes(move |index, notes| edit_notes((index, notes)));
    let set_icon = handler(ui, state, |st, (index, icon): (i32, SharedString)| {
        let icon = convert::icon_from_ui(&icon);
        st.discovery
            .edit(convert::index(index), |d| d.icon = Some(icon));
    });
    bridge.on_set_found_icon(move |index, icon| set_icon((index, icon)));
    bridge.on_reset_found_icon(handler(ui, state, |st, index| {
        st.discovery.edit(convert::index(index), |d| d.icon = None);
    }));
    bridge.on_add_found(handler(ui, state, |st, index| {
        st.add_found(convert::index(index));
    }));
    bridge.on_ignore_found(handler(ui, state, |st, index| {
        st.discovery.ignore(convert::index(index));
    }));
}

/// Steps the run on until it stops running: finished, paused or stopped (or the window closed).
fn run(ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>, timer: &Rc<Timer>) {
    if timer.running() {
        return;
    }
    let (ui, state, this) = (ui.clone(), Rc::clone(state), Rc::downgrade(timer));
    timer.start(TimerMode::Repeated, STEP, move || {
        let mut running = false;
        update(&ui, &state, |st| running = st.step_discovery());
        if let Some(stepping) = this.upgrade().filter(|_| !running) {
            stepping.stop();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(state: &mut AppState) {
        state.start_discovery();
        while state.step_discovery() {}
    }

    #[test]
    fn a_run_refreshes_the_sample_network_and_counts_once_finished() {
        let mut state = AppState::new(mock_data::network());
        state.start_discovery();
        assert!(state.discovery.busy());
        assert_eq!(state.scans, 0);
        state.step_discovery();
        state.start_discovery();
        assert_eq!(state.discovery.steps(), 1, "a busy run is not restarted");
        while state.step_discovery() {}
        assert_eq!(state.discovery.phase(), Phase::Finished);
        assert_eq!(state.scans, 1);
        assert_ne!(state.network, mock_data::network(), "the fake refresh ran");
    }

    #[test]
    fn stopping_counts_the_scan_and_an_idle_stop_does_nothing() {
        let mut state = AppState::new(mock_data::network());
        state.stop_discovery();
        assert_eq!(state.scans, 0);
        state.start_discovery();
        state.stop_discovery();
        assert_eq!((state.discovery.phase(), state.scans), (Phase::Finished, 1));
    }

    #[test]
    fn the_target_is_the_one_picked_or_the_first_subnet() {
        let mut state = AppState::new(mock_data::network());
        let subnets = state.network.subnets();
        assert_eq!(state.discovery_target(), subnets.first().copied());
        state.discovery.target = subnets.get(1).copied();
        assert_eq!(state.discovery_target(), subnets.get(1).copied());
    }

    #[test]
    fn an_added_host_is_laid_out_on_the_map_with_the_role_picked() {
        let mut state = AppState::new(mock_data::network());
        finished(&mut state);
        let found = state.discovery.found();
        let index = found
            .iter()
            .position(|f| f.device.name == "Guest Laptop")
            .unwrap();
        let other = state.role_index(&crate::model::Role::Other);
        state.set_found_role(index, other);
        state.add_found(index);
        let id = state.discovery.found()[index].on_map().unwrap();
        let device = state.network.device(id).unwrap();
        assert_eq!(device.role, crate::model::Role::Other);
        assert!(state.positions.contains_key(&device.cards[0].id));
        assert_eq!(state.selected, Some(Selection::Device(id)), "edited on");
    }

    #[test]
    fn a_found_host_on_the_map_selects_its_device_and_one_off_it_keeps_the_selection() {
        let mut state = AppState::new(mock_data::network());
        finished(&mut state);
        let found = state.discovery.found();
        let at = |name| found.iter().position(|f| f.device.name == name);
        let (nas, laptop) = (at("NAS"), at("Guest Laptop"));
        let nas_id = found[nas.unwrap()].on_map().unwrap();
        state.select_found(nas);
        assert_eq!(state.selected, Some(Selection::Device(nas_id)));
        state.select_found(laptop);
        assert_eq!(state.discovery.selected, laptop);
        assert_eq!(state.selected, Some(Selection::Device(nas_id)));
    }
}
