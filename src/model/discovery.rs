//! A simulated discovery run over one subnet: it checks the addresses in steps, reveals the
//! hosts that answer as it reaches them, flags those not on the map, and adds the ones the user
//! picks. Which hosts answer is handed in (sample data); nothing here touches a real network.

mod found;
mod moves;

pub(crate) use found::{Answer, Decision, Finding, Found, FoundFilter, Method};
pub(crate) use moves::Parent;

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

use super::card::{CardId, MacAddr, NetworkCard};
use super::device::{Device, DeviceId, DeviceKind, LastSeen, Status};
use super::network::{ConnectionKind, Network};

/// Steps a whole run takes, whatever the size of the subnet, so every run is short.
pub(crate) const STEPS_PER_RUN: u32 = 40;

/// What a run looks for. Without service detection, a host's make, model, hostname and the
/// methods that tell what it is stay unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScanOptions {
    pub(crate) ping_sweep: bool,
    pub(crate) port_scan: bool,
    pub(crate) service_detection: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            ping_sweep: true,
            port_scan: true,
            service_detection: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Phase {
    #[default]
    Idle,
    Running,
    Paused,
    Finished,
}

/// What a run did and found, as data: the UI words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LogEvent {
    Started { target: Ipv4Net },
    Sweep { addresses: usize },
    Discovered { name: String, ip: Ipv4Addr },
    Unidentified { ip: Ipv4Addr, mac: Option<MacAddr> },
    PossibleDuplicate { ip: Ipv4Addr, of: DeviceId },
    Paused,
    Resumed,
    Stopped { found: usize },
    Finished { found: usize },
}

impl LogEvent {
    /// Something the user may want to look at.
    pub(crate) fn is_warning(&self) -> bool {
        matches!(
            self,
            Self::Unidentified { .. } | Self::PossibleDuplicate { .. }
        )
    }
}

/// A log line, at the step of the run it happened in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogEntry {
    pub(crate) step: u32,
    pub(crate) event: LogEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Discovery {
    pub(crate) options: ScanOptions,
    /// The subnet to scan next, or being scanned.
    pub(crate) target: Option<Ipv4Net>,
    pub(crate) filter: FoundFilter,
    /// The found host shown in the details, by its place in [`Self::found`].
    pub(crate) selected: Option<usize>,
    /// The card of that host shown instead, by its place among the host's cards.
    pub(crate) selected_card: Option<usize>,
    phase: Phase,
    /// Every host the run will find, in address order; the first `revealed` have been reached.
    hosts: Vec<Found>,
    revealed: usize,
    /// Addresses checked so far, of `addresses`.
    swept: usize,
    addresses: usize,
    steps: u32,
    log: Vec<LogEntry>,
}

impl Discovery {
    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }

    /// Running or paused.
    pub(crate) fn busy(&self) -> bool {
        matches!(self.phase, Phase::Running | Phase::Paused)
    }

    /// The hosts found so far, in address order.
    pub(crate) fn found(&self) -> &[Found] {
        self.hosts.get(..self.revealed).unwrap_or_default()
    }

    /// Addresses checked so far, and in all.
    pub(crate) fn progress(&self) -> (usize, usize) {
        (self.swept, self.addresses)
    }

    pub(crate) fn steps(&self) -> u32 {
        self.steps
    }

    pub(crate) fn log(&self) -> &[LogEntry] {
        &self.log
    }

    pub(crate) fn count(&self, filter: FoundFilter) -> usize {
        self.found().iter().filter(|f| filter.admits(f)).count()
    }

    /// The found hosts the filter lets through, by their place in [`Self::found`], each with how
    /// many of them it runs within: as in the device list, a guest follows its host, indented
    /// under it, and stands on its own when its host is not shown.
    pub(crate) fn shown(&self) -> Vec<(usize, usize)> {
        let found = self.found();
        let admitted: Vec<usize> = found
            .iter()
            .enumerate()
            .filter(|(_, f)| self.filter.admits(f))
            .map(|(index, _)| index)
            .collect();
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

    /// Starts a run over `target`, in which the hosts of `answers` on it answer.
    pub(crate) fn start(&mut self, target: Ipv4Net, answers: Vec<Answer>, network: &Network) {
        let options = self.options;
        let mut hosts: Vec<Found> = answers
            .into_iter()
            .filter_map(|answer| Found::detect(answer, target, options, network))
            .collect();
        hosts.sort_by_key(|f| f.ip);
        *self = Self {
            options,
            target: Some(target),
            filter: self.filter,
            phase: Phase::Running,
            hosts,
            addresses: target.hosts().count(),
            ..Self::default()
        };
        self.note(LogEvent::Started { target });
        if options.ping_sweep {
            let addresses = self.addresses;
            self.note(LogEvent::Sweep { addresses });
        }
    }

    /// Pauses a running scan, or resumes a paused one.
    pub(crate) fn toggle_pause(&mut self) {
        let (phase, event) = match self.phase {
            Phase::Running => (Phase::Paused, LogEvent::Paused),
            Phase::Paused => (Phase::Running, LogEvent::Resumed),
            Phase::Idle | Phase::Finished => return,
        };
        self.phase = phase;
        self.note(event);
    }

    /// Ends the scan where it got to: hosts it has not reached are not found. True if it was
    /// running or paused.
    pub(crate) fn stop(&mut self) -> bool {
        if !self.busy() {
            return false;
        }
        self.hosts.truncate(self.revealed);
        self.phase = Phase::Finished;
        let found = self.hosts.len();
        self.note(LogEvent::Stopped { found });
        true
    }

    /// One step of a running scan: checks the next addresses and reveals the hosts among them.
    /// True when this step finishes the run.
    pub(crate) fn step(&mut self) -> bool {
        if self.phase != Phase::Running {
            return false;
        }
        self.steps += 1;
        let stride = self.addresses.div_ceil(STEPS_PER_RUN as usize);
        self.swept = (self.swept + stride).min(self.addresses);
        let first = self.target.map_or(0, |t| u32::from(t.network()));
        while let Some(host) = self.hosts.get(self.revealed)
            && (u32::from(host.ip).saturating_sub(first) as usize) <= self.swept
        {
            let event = host.event();
            self.revealed += 1;
            if let Some(event) = event {
                self.note(event);
            }
        }
        if self.swept < self.addresses {
            return false;
        }
        self.phase = Phase::Finished;
        let found = self.hosts.len();
        self.note(LogEvent::Finished { found });
        true
    }

    fn note(&mut self, event: LogEvent) {
        let step = self.steps;
        self.log.push(LogEntry { step, event });
    }

    fn found_mut(&mut self, index: usize) -> Option<&mut Found> {
        self.hosts.get_mut(..self.revealed)?.get_mut(index)
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

    fn started(options: ScanOptions) -> (Discovery, Network) {
        let network = mock_data::network();
        let mut discovery = Discovery {
            options,
            ..Discovery::default()
        };
        discovery.start(home(), mock_data::answers(&network), &network);
        (discovery, network)
    }

    fn run_to_end(discovery: &mut Discovery) {
        for _ in 0..STEPS_PER_RUN {
            discovery.step();
        }
    }

    fn named<'a>(discovery: &'a Discovery, name: &str) -> (usize, &'a Found) {
        let mut found = discovery.found().iter().enumerate();
        found.find(|(_, f)| f.device.name == name).unwrap()
    }

    #[test]
    fn a_run_reveals_each_host_on_its_subnet_once_in_address_order() {
        let (mut discovery, _) = started(ScanOptions::default());
        assert!(discovery.found().is_empty());
        discovery.step();
        let early = discovery.found().len();
        run_to_end(&mut discovery);
        assert_eq!(discovery.phase(), Phase::Finished);
        assert_eq!(discovery.progress(), (254, 254));
        assert!(discovery.steps() <= STEPS_PER_RUN);
        let ips: Vec<_> = discovery.found().iter().map(|f| f.ip).collect();
        assert!(early < ips.len(), "hosts appear as the sweep reaches them");
        assert!(ips.windows(2).all(|w| w[0] < w[1]), "in order, once each");
        assert!(ips.iter().all(|ip| home().contains(ip)));
        assert!(!discovery.step(), "a finished run stays finished");
        let log = discovery.log();
        assert!(matches!(
            log.first().unwrap().event,
            LogEvent::Started { .. }
        ));
        let found = ips.len();
        assert_eq!(log.last().unwrap().event, LogEvent::Finished { found });
    }

    #[test]
    fn pausing_freezes_the_run_and_stopping_ends_it_with_what_was_found() {
        let (mut discovery, _) = started(ScanOptions::default());
        discovery.step();
        discovery.toggle_pause();
        let paused = discovery.progress();
        discovery.step();
        assert_eq!(discovery.progress(), paused);
        discovery.toggle_pause();
        discovery.step();
        assert!(discovery.progress().0 > paused.0);

        let found = discovery.found().len();
        assert!(discovery.stop());
        assert_eq!(discovery.phase(), Phase::Finished);
        run_to_end(&mut discovery);
        assert_eq!(discovery.found().len(), found);
        assert_eq!(
            discovery.log().last().unwrap().event,
            LogEvent::Stopped { found }
        );
        assert!(!discovery.stop());
    }

    #[test]
    fn options_limit_what_is_found_out() {
        let options = ScanOptions {
            ping_sweep: false,
            port_scan: false,
            service_detection: false,
        };
        let (mut discovery, _) = started(options);
        run_to_end(&mut discovery);
        let found = discovery.found();
        assert!(!found.is_empty());
        for f in found {
            assert!(
                f.device
                    .cards
                    .iter()
                    .all(|c| c.services.iter().count() == 0)
            );
            assert_eq!(f.device.manufacturer, None);
            assert!(
                f.methods.iter().all(|&m| m == Method::Arp),
                "{:?}",
                f.methods
            );
        }
        let sweep = discovery
            .log()
            .iter()
            .any(|e| matches!(e.event, LogEvent::Sweep { .. }));
        assert!(!sweep);
    }

    #[test]
    fn hosts_are_matched_with_the_map_by_mac_address() {
        let (mut discovery, _) = started(ScanOptions::default());
        run_to_end(&mut discovery);
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
        let (mut discovery, mut network) = started(ScanOptions::default());
        run_to_end(&mut discovery);
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
        let (mut discovery, mut network) = started(ScanOptions::default());
        run_to_end(&mut discovery);
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
        let (mut home, _) = started(ScanOptions::default());
        run_to_end(&mut home);
        assert_eq!(
            docker_seen(&home),
            [true, false],
            "its lab card is on another subnet"
        );

        let network = mock_data::network();
        let mut lab = Discovery::default();
        lab.start(
            "10.0.0.0/24".parse().unwrap(),
            mock_data::answers(&network),
            &network,
        );
        run_to_end(&mut lab);
        assert_eq!(docker_seen(&lab), [false, true]);
    }

    #[test]
    fn guests_follow_their_host_in_the_results_unless_it_is_filtered_out() {
        let (mut discovery, _) = started(ScanOptions::default());
        run_to_end(&mut discovery);
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
    fn another_subnet_finds_only_its_own_hosts() {
        let network = mock_data::network();
        let lab: Ipv4Net = "10.0.0.0/24".parse().unwrap();
        let mut discovery = Discovery::default();
        discovery.start(lab, mock_data::answers(&network), &network);
        run_to_end(&mut discovery);
        let names: Vec<_> = discovery
            .found()
            .iter()
            .map(|f| f.device.name.as_str())
            .collect();
        assert_eq!(names, ["Lab Router", "Lab Switch", "Docker Host", "Lab Pi"]);
    }
}
