//! The discovery page: scanning the network the user types for real, and editing the hosts
//! found and adding them to the map. The run's record is [`crate::model::Discovery`], the
//! scanning [`crate::scan`]; [`super::view`] sends the run to the UI.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use ipnet::Ipv4Net;
use slint::{ComponentHandle, SharedString, Timer, TimerMode};

use super::{AppState, Selection, convert, handler, update};
use crate::model::{Found, FoundFilter, FoundSort, ScanMode, ScanOptions, ScanProgress};
use crate::scan::{self, Event, Scan};
use crate::ui::{self, AppWindow, Bridge};

/// How often a running scan's progress and hosts are taken in and shown.
const POLL: Duration = Duration::from_millis(200);

impl AppState {
    /// Starts recording a run of the subnet typed, unless it is not one or a run is busy: what
    /// to scan, and how.
    fn begin_discovery(&mut self) -> Option<(Ipv4Net, ScanOptions)> {
        let target = self.discovery.next_target().ok()?;
        let options = self.discovery.options;
        self.discovery.start(target).then_some((target, options))
    }

    fn add_found_port(&mut self, host: usize, card: usize, port: &str, description: &str) -> bool {
        let Some(port) = convert::port(port) else {
            return false;
        };
        let mut added = false;
        self.discovery.edit_card(host, card, |network_card| {
            added = network_card.services.add(port, description);
        });
        added
    }

    /// Takes in a running scan's progress and what it reported, counting the scan once it
    /// finishes. True while it is still running.
    fn take_scan(
        &mut self,
        progress: ScanProgress,
        events: impl IntoIterator<Item = Event>,
    ) -> bool {
        let events: Vec<Event> = events.into_iter().collect();
        for event in &events {
            let Event::Host(answer) = event else { continue };
            for card in &answer.device.cards {
                if let (Some(mac), Some(address)) = (card.mac, card.address) {
                    self.network.refresh_dhcp_address(mac, address);
                }
            }
        }
        if scan::record(&mut self.discovery, progress, events, &self.network) {
            self.scans += 1;
        }
        self.discovery.busy()
    }

    fn stop_discovery(&mut self) {
        if self.discovery.stop() {
            self.scans += 1;
        }
    }

    /// Shows a found host in the details, or the card at `card` among its cards. One on the map
    /// is selected there too, or that card of it, so the details edit it on the map.
    pub(super) fn select_found(&mut self, index: Option<usize>, card: Option<usize>) {
        self.discovery.selected = index;
        self.discovery.selected_card = card;
        let found = index.and_then(|i| self.discovery.found().get(i));
        if let Some(id) = found.and_then(Found::on_map) {
            let card = card.and_then(|at| self.network.device(id)?.cards.get(at));
            let card = card.map(|c| Selection::Card(c.id));
            self.selected = Some(card.unwrap_or(Selection::Device(id)));
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

    /// Adds every new host still waiting for a decision to the map and lays them out once. The
    /// selection is taken again, so a host shown in the details is edited on the map now.
    fn import_found(&mut self) {
        if self.discovery.import(&mut self.network) > 0 {
            self.lay_out_at_rest();
            self.select_found(self.discovery.selected, self.discovery.selected_card);
        }
    }

    fn set_found_role(&mut self, index: usize, role: usize) {
        if let Some(role) = self.role(role) {
            self.discovery.edit(index, |d| d.role = role);
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

fn sort_from_ui(sort: ui::FoundSort) -> FoundSort {
    match sort {
        ui::FoundSort::Name => FoundSort::Name,
        ui::FoundSort::Ip => FoundSort::Ip,
        ui::FoundSort::Mac => FoundSort::Mac,
        ui::FoundSort::Role => FoundSort::Role,
    }
}

fn scan_mode_from_ui(mode: ui::ScanMode) -> ScanMode {
    match mode {
        ui::ScanMode::Quick => ScanMode::Quick,
        ui::ScanMode::Full => ScanMode::Full,
        ui::ScanMode::Everything => ScanMode::Everything,
    }
}

/// The run's controls and options, the result filter, and the found hosts. The network box
/// starts with this computer's own subnet, when one is found; after that it holds what the
/// user types, and Rust never rewrites it.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let own = scan::local_subnet()
        .map(|s| s.to_string())
        .unwrap_or_default();
    bridge.set_discovery_target_draft(own.as_str().into());
    state.borrow_mut().discovery.target_text = own;
    let running = Rc::new(Running::default());

    let (start_ui, start_state, start_running) =
        (ui.as_weak(), Rc::clone(state), Rc::clone(&running));
    bridge.on_start_discovery(move || {
        let mut started = None;
        update(&start_ui, &start_state, |st| {
            let Some((target, options)) = st.begin_discovery() else {
                return;
            };
            match Scan::start(target, options) {
                Ok(scan) => started = Some(scan),
                Err(error) => {
                    st.discovery.fail(error.to_string());
                }
            }
        });
        if let Some(scan) = started {
            let (watch_ui, watch_state) = (start_ui.clone(), Rc::clone(&start_state));
            start_running.watch(scan, move |scan| {
                let mut going = false;
                update(&watch_ui, &watch_state, |st| {
                    going = st.take_scan(scan.progress(), scan.events());
                });
                going
            });
        }
    });
    let stop = handler(ui, state, |st, ()| st.stop_discovery());
    bridge.on_stop_discovery(move || {
        running.end();
        stop(());
    });

    bridge.on_set_discovery_target(handler(ui, state, |st, text: SharedString| {
        st.discovery.target_text = text.into();
    }));
    bridge.on_set_scan_mode(handler(ui, state, |st, mode| {
        st.discovery.options.mode = scan_mode_from_ui(mode);
    }));
    bridge.on_set_found_filter(handler(ui, state, |st, filter| {
        st.discovery.filter = filter_from_ui(filter);
    }));
    bridge.on_sort_found(handler(ui, state, |st, sort| {
        st.discovery.set_sort(sort_from_ui(sort));
    }));
    connect_found(ui, state);
}

/// Selecting a found host or one of its cards, editing one not on the map, and adding or
/// ignoring it.
fn connect_found(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    bridge.on_select_found(handler(ui, state, |st, index| {
        st.select_found(usize::try_from(index).ok(), None);
    }));
    let select_card = handler(ui, state, |st, (index, card): (i32, i32)| {
        st.select_found(usize::try_from(index).ok(), usize::try_from(card).ok());
    });
    bridge.on_select_found_card(move |index, card| select_card((index, card)));
    connect_found_cards(ui, state);
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
    let import = handler(ui, state, |st, ()| st.import_found());
    bridge.on_import_found(move || import(()));
}

/// Renaming and re-iconing the cards of a found host not on the map.
fn connect_found_cards(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let rename = handler(
        ui,
        state,
        |st, (index, card, name): (i32, i32, SharedString)| {
            let (index, card) = (convert::index(index), convert::index(card));
            st.discovery
                .edit_card(index, card, |c| c.name = name.into());
        },
    );
    bridge.on_rename_found_card(move |index, card, name| rename((index, card, name)));
    let set_icon = handler(
        ui,
        state,
        |st, (index, card, icon): (i32, i32, SharedString)| {
            let (index, card) = (convert::index(index), convert::index(card));
            let icon = convert::icon_from_ui(&icon);
            st.discovery.edit_card(index, card, |c| c.icon = icon);
        },
    );
    bridge.on_set_found_card_icon(move |index, card, icon| set_icon((index, card, icon)));

    let (add_ui, add_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_add_found_port(move |host, card, port, description| {
        let mut added = false;
        update(&add_ui, &add_state, |st| {
            added = st.add_found_port(
                convert::index(host),
                convert::index(card),
                &port,
                &description,
            );
        });
        added
    });
    let describe = handler(
        ui,
        state,
        |st, (host, card, port, text): (i32, i32, i32, SharedString)| {
            st.discovery
                .edit_card(convert::index(host), convert::index(card), |network_card| {
                    network_card
                        .services
                        .set_description(convert::index(port), &text);
                });
        },
    );
    bridge.on_set_found_port_description(move |host, card, port, text| {
        describe((host, card, port, text));
    });
    let remove = handler(ui, state, |st, (host, card, port): (i32, i32, i32)| {
        st.discovery
            .edit_card(convert::index(host), convert::index(card), |network_card| {
                network_card.services.remove(convert::index(port));
            });
    });
    bridge.on_remove_found_port(move |host, card, port| remove((host, card, port)));
}

/// The scan running, if any, and the timer that takes in what it reports. The discovery page
/// and the AI settings window's scan each have one.
#[derive(Default)]
pub(crate) struct Running {
    scan: RefCell<Option<Scan>>,
    timer: Timer,
}

impl Running {
    /// Has `take` take in what `started` reports, every [`POLL`], until it says the scan has
    /// ended (or this is dropped).
    pub(crate) fn watch(self: &Rc<Self>, started: Scan, take: impl Fn(&Scan) -> bool + 'static) {
        self.scan.replace(Some(started));
        let this = Rc::downgrade(self);
        self.timer.start(TimerMode::Repeated, POLL, move || {
            let Some(running) = this.upgrade() else {
                return;
            };
            let going = running.scan.borrow().as_ref().is_some_and(&take);
            if !going {
                running.end();
            }
        });
    }

    /// Stops taking in the scan, and stops the scan if it is still going.
    pub(crate) fn end(&self) {
        self.timer.stop();
        self.scan.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::Phase;

    fn home() -> Ipv4Net {
        "192.168.1.0/24".parse().unwrap()
    }

    /// The sample answers, as the scan reports them.
    fn hosts(state: &AppState) -> Vec<Event> {
        let answers = mock_data::answers(&state.network).into_iter();
        answers
            .map(|answer| Event::Host(Box::new(answer)))
            .collect()
    }

    /// A scan that has swept `swept` addresses and probed no ports yet.
    fn swept(swept: usize) -> ScanProgress {
        ScanProgress {
            swept,
            ..ScanProgress::default()
        }
    }

    /// A run of the home network that every sample answer has reached.
    fn finished(state: &mut AppState) {
        state.discovery.target_text = home().to_string();
        state.begin_discovery();
        let mut events = hosts(state);
        events.push(Event::Finished);
        state.take_scan(swept(254), events);
    }

    #[test]
    fn a_run_takes_in_the_scan_and_counts_once_finished() {
        let mut state = AppState::new(mock_data::network());
        assert_eq!(state.begin_discovery(), None, "nothing typed");
        state.discovery.target_text = "192.168.1.0/8".into();
        assert_eq!(state.begin_discovery(), None, "too wide to scan");
        state.discovery.target_text = "192.168.1.0/24".into();
        let run = Some((home(), ScanOptions::default()));
        assert_eq!(state.begin_discovery(), run);
        assert_eq!(state.begin_discovery(), None, "a busy run is not restarted");
        let hosts = hosts(&state);
        assert!(state.take_scan(swept(100), hosts));
        assert_eq!(state.discovery.progress(), (100, 254));
        assert_ne!(state.discovery.found(), []);
        assert_eq!(state.scans, 0);
        assert!(!state.take_scan(swept(254), [Event::Finished]));
        assert_eq!((state.discovery.phase(), state.scans), (Phase::Finished, 1));
        assert!(
            !state.take_scan(swept(254), [Event::Finished]),
            "counted once"
        );
        assert_eq!(state.scans, 1);
    }

    #[test]
    fn a_scan_refreshes_a_dhcp_cards_address_before_matching_the_host() {
        let mut state = AppState::new(mock_data::network());
        let device = state.network.devices()[0].id;
        let (card_id, mac) = {
            let card = &state.network.device(device).unwrap().cards[0];
            (card.id, card.mac.unwrap())
        };
        state.network.device_mut(device).unwrap().cards[0].dhcp = true;
        let address: Ipv4Net = "192.168.1.250/24".parse().unwrap();
        let mut answer = mock_data::answers(&state.network)
            .into_iter()
            .find(|answer| answer.device.cards.iter().any(|card| card.mac == Some(mac)))
            .unwrap();
        answer
            .device
            .cards
            .iter_mut()
            .find(|card| card.mac == Some(mac))
            .unwrap()
            .address = Some(address);

        state.discovery.target_text = home().to_string();
        assert!(state.begin_discovery().is_some());
        assert!(state.take_scan(swept(1), [Event::Host(Box::new(answer))]));

        assert_eq!(state.network.card(card_id).unwrap().address, Some(address));
        assert_eq!(state.discovery.found()[0].on_map(), Some(device));
    }

    #[test]
    fn a_found_cards_guessed_service_can_be_edited_and_a_port_added() {
        let mut state = AppState::new(mock_data::network());
        finished(&mut state);
        let host = state
            .discovery
            .found()
            .iter()
            .position(|found| found.device.name == "Guest Laptop")
            .unwrap();

        assert!(state.add_found_port(host, 0, "8443", "Guessed service"));
        assert!(!state.add_found_port(host, 0, "8443", "Duplicate"));
        state.discovery.edit_card(host, 0, |card| {
            let at = card
                .services
                .iter()
                .position(|service| service.port == 8443)
                .unwrap();
            card.services.set_description(at, "Admin");
        });

        let services = &state.discovery.found()[host].device.cards[0].services;
        let shown = services.to_string();
        assert!(shown.contains("8443 (Admin)"));
    }

    #[test]
    fn stopping_counts_the_scan_failing_does_not_and_an_idle_stop_does_nothing() {
        let mut state = AppState::new(mock_data::network());
        state.discovery.target_text = home().to_string();
        state.stop_discovery();
        assert_eq!(state.scans, 0);
        state.begin_discovery();
        assert!(!state.take_scan(swept(0), [Event::Failed("no route".into())]));
        assert_eq!((state.discovery.phase(), state.scans), (Phase::Finished, 0));
        state.begin_discovery();
        state.stop_discovery();
        assert_eq!((state.discovery.phase(), state.scans), (Phase::Finished, 1));
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
    fn imported_hosts_are_laid_out_and_the_one_shown_is_edited_on_the_map() {
        let mut state = AppState::new(mock_data::network());
        finished(&mut state);
        let mut hosts = state.discovery.found().iter();
        let laptop = hosts.position(|f| f.device.name == "Guest Laptop");
        state.select_found(laptop, None);
        state.import_found();
        let found = state.discovery.found();
        let laptop_id = found[laptop.unwrap()].on_map().unwrap();
        assert_eq!(state.selected, Some(Selection::Device(laptop_id)));
        let added = found.iter().filter_map(Found::on_map);
        let mut cards = added.filter_map(|id| state.network.device(id)?.primary_card());
        assert!(cards.all(|c| state.positions.contains_key(&c.id)));
    }

    #[test]
    fn a_found_host_on_the_map_selects_its_device_and_one_off_it_keeps_the_selection() {
        let mut state = AppState::new(mock_data::network());
        finished(&mut state);
        let found = state.discovery.found();
        let at = |name| found.iter().position(|f| f.device.name == name);
        let (nas, laptop) = (at("NAS"), at("Guest Laptop"));
        let nas_id = found[nas.unwrap()].on_map().unwrap();
        state.select_found(nas, None);
        assert_eq!(state.selected, Some(Selection::Device(nas_id)));
        state.select_found(laptop, None);
        assert_eq!(state.discovery.selected, laptop);
        assert_eq!(state.selected, Some(Selection::Device(nas_id)));
    }

    #[test]
    fn a_found_card_of_a_host_on_the_map_selects_that_card_there() {
        let mut state = AppState::new(mock_data::network());
        finished(&mut state);
        let found = state.discovery.found();
        let docker = found.iter().position(|f| f.device.name == "Docker Host");
        let id = found[docker.unwrap()].on_map().unwrap();
        let lab = state.network.device(id).unwrap().cards[1].id;
        state.select_found(docker, Some(1));
        assert_eq!(state.selected, Some(Selection::Card(lab)));
        assert_eq!(state.discovery.selected_card, Some(1));
        state.select_found(docker, Some(9));
        assert_eq!(
            state.selected,
            Some(Selection::Device(id)),
            "no such card: the host"
        );
        state.select_found(docker, None);
        assert_eq!(state.discovery.selected_card, None);
    }
}
