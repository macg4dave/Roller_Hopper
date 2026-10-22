//! A discovery run over one subnet: it takes the hosts the scanner reports as they come, flags
//! those not on the map, and adds the ones the user picks. The scanning itself is elsewhere
//! (`crate::scan`); nothing here touches a network, so runs can be replayed in tests.

mod found;
mod moves;
mod target;

pub(crate) use found::{Answer, Decision, Finding, Found, FoundFilter, FoundSort, Method};
pub(crate) use moves::Parent;
pub(crate) use target::TargetError;

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use ipnet::Ipv4Net;

use super::card::{CardId, MacAddr, NetworkCard};
use super::device::{Device, DeviceId, DeviceKind, LastSeen, Role, Status};
use super::network::{ConnectionKind, Network};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ScanMode {
    #[default]
    Quick,
    Full,
    Everything,
}

/// What a run finds out beyond which addresses answer. The UI runs every available method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent on/off choices, one per tick box"
)]
pub(crate) struct ScanOptions {
    pub(crate) mode: ScanMode,
    /// Names from reverse DNS, and the software that names itself on open ports.
    pub(crate) service_detection: bool,
    /// The services hosts announce over mDNS, with the model and maker they give.
    pub(crate) mdns_services: bool,
    /// A guess at what kind of device each host is.
    pub(crate) device_type: bool,
    /// A guess at each host's operating system family; adds a ping for its time-to-live.
    pub(crate) os_family: bool,
    /// Who made each network card, from its MAC address.
    pub(crate) card_vendor: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            mode: ScanMode::Quick,
            service_detection: true,
            mdns_services: true,
            device_type: true,
            os_family: true,
            card_vendor: true,
        }
    }
}

/// How far a running scan has got: first the sweep of the subnet's addresses, then the port
/// probes of the hosts it found, of which there are none planned until the sweep ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ScanProgress {
    pub(crate) swept: usize,
    pub(crate) probed: usize,
    pub(crate) probes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Phase {
    #[default]
    Idle,
    Running,
    Finished,
}

/// What a run did and found, as data: the UI words it. When it `Failed`, the `reason` is the
/// system's or scanner's own message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LogEvent {
    Started { target: Ipv4Net },
    Sweep { addresses: usize },
    Discovered { name: String, ip: Ipv4Addr },
    Unidentified { ip: Ipv4Addr, mac: Option<MacAddr> },
    PossibleDuplicate { ip: Ipv4Addr, of: DeviceId },
    Failed { reason: String },
    Stopped { found: usize },
    Finished { found: usize },
}

impl LogEvent {
    /// Something the user may want to look at.
    pub(crate) fn is_warning(&self) -> bool {
        matches!(
            self,
            Self::Unidentified { .. } | Self::PossibleDuplicate { .. } | Self::Failed { .. }
        )
    }
}

/// A log line, at the time into the run it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogEntry {
    pub(crate) at: Duration,
    pub(crate) event: LogEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Discovery {
    pub(crate) options: ScanOptions,
    /// The network to scan next, as the user typed it. Nothing fills it from the map: mapping
    /// a network is no permission to scan it.
    pub(crate) target_text: String,
    /// The subnet being scanned, or last scanned.
    target: Option<Ipv4Net>,
    pub(crate) filter: FoundFilter,
    pub(crate) sort: FoundSort,
    pub(crate) sort_descending: bool,
    /// The found host shown in the details, by its place in [`Self::found`].
    pub(crate) selected: Option<usize>,
    /// The card of that host shown instead, by its place among the host's cards.
    pub(crate) selected_card: Option<usize>,
    phase: Phase,
    /// The hosts found, in the order they were reported; [`Self::shown`] puts them in address
    /// order. Their places here never change while they are listed.
    hosts: Vec<Found>,
    /// How far the scan has got; its `swept` is of `addresses`.
    reached: ScanProgress,
    addresses: usize,
    /// When the run started, and how long it took once it ended.
    started: Option<Instant>,
    took: Option<Duration>,
    log: Vec<LogEntry>,
}

impl Discovery {
    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }

    pub(crate) fn busy(&self) -> bool {
        self.phase == Phase::Running
    }

    /// The subnet the next run scans, or why the typed text is not one.
    pub(crate) fn next_target(&self) -> Result<Ipv4Net, TargetError> {
        target::parse_target(&self.target_text)
    }

    /// The subnet being scanned, or last scanned.
    pub(crate) fn target(&self) -> Option<Ipv4Net> {
        self.target
    }

    /// The hosts found so far, in the order they were reported.
    pub(crate) fn found(&self) -> &[Found] {
        &self.hosts
    }

    /// Addresses checked so far, and in all.
    pub(crate) fn progress(&self) -> (usize, usize) {
        (self.reached.swept, self.addresses)
    }

    /// Port probes done so far, and in all; none are planned until the sweep ends.
    pub(crate) fn ports(&self) -> (usize, usize) {
        (self.reached.probed, self.reached.probes)
    }

    /// How long the run has been going, or took.
    pub(crate) fn elapsed(&self) -> Duration {
        let running = self.started.map(|s| s.elapsed());
        self.took.or(running).unwrap_or_default()
    }

    pub(crate) fn log(&self) -> &[LogEntry] {
        &self.log
    }

    pub(crate) fn count(&self, filter: FoundFilter) -> usize {
        self.found().iter().filter(|f| filter.admits(f)).count()
    }

    /// The found hosts the filter lets through, in the selected column order, by their place in
    /// [`Self::found`], each with how many of them it runs within: as in the device list, a
    /// guest follows its host, indented under it, and stands on its own when its host is not
    /// shown.
    pub(crate) fn shown(&self) -> Vec<(usize, usize)> {
        let found = self.found();
        let mut admitted: Vec<usize> = found
            .iter()
            .enumerate()
            .filter(|(_, f)| self.filter.admits(f))
            .map(|(index, _)| index)
            .collect();
        admitted.sort_by(|&left, &right| {
            let order = match (found.get(left), found.get(right)) {
                (Some(left), Some(right)) => self.compare_found(left, right),
                _ => left.cmp(&right),
            };
            if self.sort_descending {
                order.reverse()
            } else {
                order
            }
        });
        let host = |index: usize| {
            let host = found.get(index)?.device.guest?.host;
            let mut hosts = admitted.iter().copied();
            hosts.find(|&at| found.get(at).and_then(Found::on_map) == Some(host))
        };
        let tops = admitted
            .iter()
            .rev()
            .filter(|&&index| host(index).is_none());
        let mut pending: Vec<(usize, usize)> = tops.map(|&index| (index, 0)).collect();
        let mut shown = Vec::with_capacity(admitted.len());
        while let Some((index, depth)) = pending.pop() {
            shown.push((index, depth));
            let guests = admitted.iter().rev().filter(|&&g| host(g) == Some(index));
            pending.extend(guests.map(|&g| (g, depth + 1)));
        }
        // Guests in a loop of hosts reach no top; they stand on their own at the end.
        let looped: Vec<usize> = admitted
            .into_iter()
            .filter(|&index| shown.iter().all(|&(at, _)| at != index))
            .collect();
        shown.extend(looped.into_iter().map(|index| (index, 0)));
        shown
    }

    fn compare_found(&self, left: &Found, right: &Found) -> std::cmp::Ordering {
        let text = |value: &str| value.to_lowercase();
        match self.sort {
            FoundSort::Name => text(&left.device.name).cmp(&text(&right.device.name)),
            FoundSort::Ip => left.ip.cmp(&right.ip),
            FoundSort::Mac => left.mac.map(|mac| mac.0).cmp(&right.mac.map(|mac| mac.0)),
            FoundSort::Role => role_key(&left.device.role).cmp(&role_key(&right.device.role)),
        }
        .then_with(|| left.ip.cmp(&right.ip))
    }

    pub(crate) fn set_sort(&mut self, sort: FoundSort) {
        if self.sort == sort {
            self.sort_descending = !self.sort_descending;
        } else {
            self.sort = sort;
            self.sort_descending = false;
        }
    }

    /// Starts a run over `target`, forgetting the last one's results. Refused (false) while a
    /// run is busy.
    pub(crate) fn start(&mut self, target: Ipv4Net) -> bool {
        if self.busy() {
            return false;
        }
        let addresses = target.hosts().count();
        *self = Self {
            options: self.options,
            target_text: std::mem::take(&mut self.target_text),
            target: Some(target),
            filter: self.filter,
            sort: self.sort,
            sort_descending: self.sort_descending,
            phase: Phase::Running,
            addresses,
            started: Some(Instant::now()),
            ..Self::default()
        };
        self.note(LogEvent::Started { target });
        self.note(LogEvent::Sweep { addresses });
        true
    }

    /// Notes how far the running scan has got.
    pub(crate) fn reach(&mut self, progress: ScanProgress) {
        if self.busy() {
            self.reached = ScanProgress {
                swept: progress.swept.min(self.addresses),
                ..progress
            };
        }
    }

    /// Takes a host the running scan found; one not on the subnet scanned is no answer to it.
    pub(crate) fn answer(&mut self, answer: Answer, network: &Network) {
        let Some(target) = self.target.filter(|_| self.busy()) else {
            return;
        };
        if let Some(found) = Found::detect(answer, target, network) {
            if let Some(event) = found.event() {
                self.note(event);
            }
            self.hosts.push(found);
        }
    }

    /// Ends the scan once every address is checked and every host probed. True if it was
    /// running.
    pub(crate) fn finish(&mut self) -> bool {
        self.reached.swept = self.addresses;
        self.reached.probed = self.reached.probes;
        self.end(|found| LogEvent::Finished { found })
    }

    /// Ends the scan where it got to, keeping the hosts found so far. True if it was running.
    pub(crate) fn stop(&mut self) -> bool {
        self.end(|found| LogEvent::Stopped { found })
    }

    /// Ends the scan when it cannot go on, keeping the hosts found so far. True if it was
    /// running.
    pub(crate) fn fail(&mut self, reason: String) -> bool {
        self.end(|_| LogEvent::Failed { reason })
    }

    fn end(&mut self, event: impl FnOnce(usize) -> LogEvent) -> bool {
        if !self.busy() {
            return false;
        }
        self.took = Some(self.elapsed());
        self.phase = Phase::Finished;
        let found = self.hosts.len();
        self.note(event(found));
        true
    }

    fn note(&mut self, event: LogEvent) {
        let at = self.elapsed();
        self.log.push(LogEntry { at, event });
    }

    fn found_mut(&mut self, index: usize) -> Option<&mut Found> {
        self.hosts.get_mut(index)
    }

    /// Edits a host not on the map (its name, role, icon or notes), which it will be added
    /// with. A host on the map is edited there instead.
    pub(crate) fn edit(&mut self, index: usize, edit: impl FnOnce(&mut Device)) {
        if let Some(found) = self.found_mut(index).filter(|f| f.addable()) {
            edit(&mut found.device);
        }
    }

    /// Edits the card at `card` among the cards of a host not on the map (its name or icon),
    /// which it will be added with.
    pub(crate) fn edit_card(
        &mut self,
        index: usize,
        card: usize,
        edit: impl FnOnce(&mut NetworkCard),
    ) {
        self.edit(index, |device| {
            if let Some(card) = device.cards.get_mut(card) {
                edit(card);
            }
        });
    }

    /// Leaves a host off the map; it can still be added later.
    pub(crate) fn ignore(&mut self, index: usize) {
        if let Some(found) = self.found_mut(index).filter(|f| f.addable()) {
            found.decision = Decision::Ignored;
        }
    }

    /// Adds a host that is not on the map to it, seen just now, with an inferred link to the
    /// router on its subnet. Returns the new device, or `None` if it cannot be added.
    pub(crate) fn add(&mut self, index: usize, network: &mut Network) -> Option<DeviceId> {
        let found = self.found_mut(index).filter(|f| f.addable())?;
        let uplink = gateway(network, found.ip).map(|card| (card, ConnectionKind::Inferred));
        let mut device = found.device.clone();
        device.status = Status::Online;
        device.last_seen = LastSeen::default();
        let id = network.insert_device(device, uplink);
        found.decision = Decision::Added(id);
        Some(id)
    }
}

fn role_key(role: &Role) -> (u8, String) {
    match role {
        Role::AccessPoint => (0, "access point".into()),
        Role::Camera => (0, "camera".into()),
        Role::Desktop => (0, "desktop".into()),
        Role::Iot => (0, "iot".into()),
        Role::Laptop => (0, "laptop".into()),
        Role::Nas => (0, "nas".into()),
        Role::Other => (0, "other".into()),
        Role::Phone => (0, "phone".into()),
        Role::Printer => (0, "printer".into()),
        Role::Router => (0, "router".into()),
        Role::Server => (0, "server".into()),
        Role::Switch => (0, "switch".into()),
        Role::Custom(name) => (1, name.to_lowercase()),
    }
}

/// The card of a router on the subnet of `ip`, which a host found there is most likely behind.
fn gateway(network: &Network, ip: Ipv4Addr) -> Option<CardId> {
    let routers = network
        .devices()
        .iter()
        .filter(|d| d.kind == DeviceKind::Router);
    let mut cards = routers.flat_map(|d| &d.cards);
    let card = cards.find(|c| c.address.is_some_and(|a| a.contains(&ip)))?;
    Some(card.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::Role;

    fn home() -> Ipv4Net {
        "192.168.1.0/24".parse().unwrap()
    }

    /// A run of `target` that every sample answer has reached, still running.
    fn answered(target: Ipv4Net, network: &Network) -> Discovery {
        let mut discovery = Discovery::default();
        assert!(discovery.start(target));
        for answer in mock_data::answers(network) {
            discovery.answer(answer, network);
        }
        discovery
    }

    /// A finished run of the home network, and the sample network it ran against.
    fn finished() -> (Discovery, Network) {
        let network = mock_data::network();
        let mut discovery = answered(home(), &network);
        assert!(discovery.finish());
        (discovery, network)
    }

    fn named<'a>(discovery: &'a Discovery, name: &str) -> (usize, &'a Found) {
        let mut found = discovery.found().iter().enumerate();
        found.find(|(_, f)| f.device.name == name).unwrap()
    }

    #[test]
    fn a_run_counts_its_sweep_then_its_port_probes_up_to_the_end() {
        let mut discovery = Discovery::default();
        discovery.start(home());
        discovery.reach(ScanProgress {
            swept: 100,
            ..ScanProgress::default()
        });
        assert_eq!(discovery.ports(), (0, 0), "no probes before the sweep ends");
        discovery.reach(ScanProgress {
            swept: 300,
            probed: 12,
            probes: 30,
        });
        assert_eq!(discovery.progress(), (254, 254), "never past the end");
        assert_eq!(discovery.ports(), (12, 30));
        assert!(discovery.finish());
        assert_eq!(
            discovery.ports(),
            (30, 30),
            "every probe done once finished"
        );
        discovery.reach(ScanProgress::default());
        assert_eq!(
            discovery.ports(),
            (30, 30),
            "a finished run keeps its count"
        );
    }

    #[test]
    fn a_run_takes_the_hosts_on_its_subnet_and_shows_them_in_address_order() {
        let network = mock_data::network();
        let mut discovery = Discovery::default();
        discovery.start(home());
        assert_eq!(
            (discovery.found(), discovery.progress()),
            (&[][..], (0, 254))
        );
        discovery.reach(ScanProgress {
            swept: 100,
            ..ScanProgress::default()
        });
        assert_eq!(discovery.progress(), (100, 254));
        let mut answers = mock_data::answers(&network);
        answers.reverse();
        for answer in answers {
            discovery.answer(answer, &network);
        }
        assert!(discovery.finish());
        assert_eq!(discovery.phase(), Phase::Finished);
        assert_eq!(discovery.progress(), (254, 254));
        let found = discovery.found();
        let ips: Vec<_> = discovery
            .shown()
            .iter()
            .map(|&(i, _)| found[i].ip)
            .collect();
        assert_eq!(ips.len(), found.len());
        assert!(ips.windows(2).all(|w| w[0] < w[1]), "in order, once each");
        assert!(ips.iter().all(|ip| home().contains(ip)));

        assert!(!discovery.finish(), "a finished run stays finished");
        let late = mock_data::answers(&network).swap_remove(0);
        discovery.answer(late, &network);
        assert_eq!(discovery.found().len(), ips.len(), "nothing after the end");
        let log = discovery.log();
        assert!(matches!(
            log.first().unwrap().event,
            LogEvent::Started { .. }
        ));
        let end = LogEvent::Finished { found: ips.len() };
        assert_eq!(log.last().unwrap().event, end);
    }

    #[test]
    fn stopping_ends_the_run_with_what_was_found() {
        let network = mock_data::network();
        let mut discovery = Discovery::default();
        discovery.start(home());
        for answer in mock_data::answers(&network).into_iter().take(3) {
            discovery.answer(answer, &network);
        }
        assert!(!discovery.start(home()), "a busy run is not restarted");
        let found = discovery.found().len();
        assert!(discovery.stop());
        assert_eq!(discovery.phase(), Phase::Finished);
        assert_eq!(
            discovery.log().last().unwrap().event,
            LogEvent::Stopped { found }
        );
        assert!(!discovery.stop());
        assert!(discovery.start(home()), "a new run starts afresh");
        assert_eq!(discovery.found(), []);
    }

    #[test]
    fn a_failed_run_keeps_its_hosts_and_says_why() {
        let network = mock_data::network();
        let mut discovery = answered(home(), &network);
        let found = discovery.found().len();
        assert!(discovery.fail("no route".into()));
        assert_eq!(discovery.found().len(), found);
        let last = &discovery.log().last().unwrap().event;
        assert_eq!(
            *last,
            LogEvent::Failed {
                reason: "no route".into()
            }
        );
        assert!(last.is_warning());
        assert!(!discovery.fail("again".into()));
    }

    #[test]
    fn the_next_target_is_what_was_typed_and_is_kept_across_runs() {
        let mut discovery = Discovery::default();
        assert_eq!(discovery.next_target(), Err(TargetError::Empty));
        discovery.target_text = "192.168.1.7/24".into();
        assert_eq!(discovery.next_target(), Ok(home()));
        assert_eq!(discovery.target(), None, "nothing scanned yet");
        discovery.start(home());
        assert_eq!(discovery.target(), Some(home()));
        assert_eq!(
            discovery.target_text, "192.168.1.7/24",
            "kept for the next run"
        );
    }

    #[test]
    fn hosts_are_matched_with_the_map_by_mac_address() {
        let (discovery, _) = finished();
        let camera = DeviceId(12);
        let finding = |name| named(&discovery, name).1.finding;
        assert_eq!(finding("IP Camera Front"), Finding::Known(camera));
        assert_eq!(finding("IP Camera"), Finding::Duplicate(camera));
        assert_eq!(finding("Kitchen Speaker"), Finding::New);
        let warnings = discovery.log().iter().filter(|e| e.event.is_warning());
        assert_eq!(
            warnings.count(),
            2,
            "the duplicate and the unidentified host"
        );
        assert_eq!(discovery.count(FoundFilter::Duplicates), 1);
        assert_eq!(
            discovery.count(FoundFilter::Unidentified),
            2,
            "one is on the map"
        );
        let all = discovery.found().len();
        assert_eq!(discovery.count(FoundFilter::All), all);
    }

    #[test]
    fn a_new_host_is_added_once_as_edited_behind_the_router() {
        let (mut discovery, mut network) = finished();
        let (index, _) = named(&discovery, "Kitchen Speaker");
        discovery.edit(index, |d| {
            d.name = "Kitchen".into();
            d.role = Role::Other;
            d.notes = "On the fridge".into();
        });
        discovery.edit_card(index, 0, |c| c.name = "Wi-Fi".into());
        discovery.edit_card(index, 9, |c| c.name = "Nowhere".into());
        let id = discovery.add(index, &mut network).unwrap();
        let added = network.device(id).unwrap();
        let names: Vec<_> = added.cards.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Wi-Fi"], "its card as edited");
        assert_eq!(
            (added.name.as_str(), &added.role, added.notes.as_str()),
            ("Kitchen", &Role::Other, "On the fridge")
        );
        discovery.edit(index, |d| d.name = "Changed".into());
        assert_eq!(
            discovery.found()[index].device.name,
            "Kitchen",
            "on the map, it is edited there"
        );
        assert!(added.cards[0].services.iter().count() > 0);
        let uplink = network.uplink(added.cards[0].id).unwrap();
        assert_eq!(uplink.upstream, CardId(1), "the router's LAN card");
        assert_eq!(uplink.kind, ConnectionKind::Inferred);
        assert_eq!(discovery.found()[index].on_map(), Some(id));
        assert_eq!(discovery.add(index, &mut network), None, "only once");
        discovery.edit_card(index, 0, |c| c.name = "Changed".into());
        assert_eq!(discovery.found()[index].device.cards[0].name, "Wi-Fi");

        let (known, _) = named(&discovery, "NAS");
        let before = network.devices().len();
        assert_eq!(
            discovery.add(known, &mut network),
            None,
            "already on the map"
        );
        assert_eq!(network.devices().len(), before);
    }

    #[test]
    fn an_ignored_host_stays_off_the_map_until_added() {
        let (mut discovery, mut network) = finished();
        let before = network.clone();
        let (index, _) = named(&discovery, "Guest Laptop");
        discovery.ignore(index);
        assert_eq!(discovery.found()[index].decision, Decision::Ignored);
        assert_eq!(network, before);
        assert!(discovery.add(index, &mut network).is_some());
    }

    #[test]
    fn a_host_keeps_all_its_cards_and_only_those_on_the_subnet_are_seen() {
        let docker_seen = |discovery: &Discovery| {
            let (_, docker) = named(discovery, "Docker Host");
            let cards = docker.device.cards.iter();
            cards.map(|c| docker.seen(c)).collect::<Vec<_>>()
        };
        let (home, network) = finished();
        assert_eq!(
            docker_seen(&home),
            [true, false],
            "its lab card is on another subnet"
        );
        let lab = answered("10.0.0.0/24".parse().unwrap(), &network);
        assert_eq!(docker_seen(&lab), [false, true]);
    }

    #[test]
    fn guests_follow_their_host_in_the_results_unless_it_is_filtered_out() {
        let (mut discovery, _) = finished();
        let shown = discovery.shown();
        assert_eq!(shown.len(), discovery.found().len(), "each host once");
        let place = |name| {
            let (index, _) = named(&discovery, name);
            let at = shown.iter().position(|&(i, _)| i == index).unwrap();
            (at, shown[at].1)
        };
        let (server, server_depth) = place("Linux Server");
        let (docker, docker_depth) = place("Docker Host");
        assert_eq!((server_depth, docker_depth), (0, 1));
        assert!(server < docker, "a guest after its host");
        let guests = shown.iter().skip(server + 1);
        assert!(
            guests.take_while(|&&(_, d)| d > 0).count() > 1,
            "the server's guests, and theirs, straight after it"
        );

        discovery.filter = FoundFilter::Unidentified;
        let unidentified = discovery.shown();
        assert_eq!(
            unidentified.len(),
            discovery.count(FoundFilter::Unidentified)
        );
        assert!(unidentified.iter().all(|&(_, depth)| depth == 0));
    }

    #[test]
    fn result_columns_sort_both_ways() {
        let (mut discovery, _) = finished();
        discovery.filter = FoundFilter::Unidentified;
        discovery.set_sort(FoundSort::Name);

        let names = |state: &Discovery| {
            state
                .shown()
                .into_iter()
                .map(|(index, _)| state.found()[index].device.name.to_lowercase())
                .collect::<Vec<_>>()
        };
        let ascending = names(&discovery);
        assert!(ascending.windows(2).all(|pair| pair[0] <= pair[1]));

        discovery.set_sort(FoundSort::Name);
        let descending = names(&discovery);
        assert!(descending.windows(2).all(|pair| pair[0] >= pair[1]));
        assert_eq!(descending, ascending.into_iter().rev().collect::<Vec<_>>());
    }

    #[test]
    fn another_subnet_finds_only_its_own_hosts() {
        let network = mock_data::network();
        let discovery = answered("10.0.0.0/24".parse().unwrap(), &network);
        let found = discovery.found();
        let names: Vec<_> = discovery
            .shown()
            .iter()
            .map(|&(i, _)| found[i].device.name.as_str())
            .collect();
        assert_eq!(names, ["Lab Router", "Lab Switch", "Docker Host", "Lab Pi"]);
    }
}
