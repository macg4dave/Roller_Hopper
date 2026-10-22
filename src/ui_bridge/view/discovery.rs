//! Sends the discovery run to the UI: its progress and options, the found hosts the filter lets
//! through, each with its network cards, the selected one's details, and the log.

use std::time::Duration;

use slint::{ModelRc, SharedString, VecModel};

use super::sync;
use crate::model::{
    Decision, Finding, Found, FoundFilter, FoundSort, LogEntry, LogEvent, Method, Parent, Phase,
    ScanMode, TargetError,
};
use crate::ui::{self, Bridge, Labels};
use crate::ui_bridge::{AppState, convert};

pub(super) fn push(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let discovery = &state.discovery;
    let next = discovery.next_target();
    // While a run is busy, the subnet it scans, whatever the box says meanwhile.
    let target = if discovery.busy() {
        discovery.target()
    } else {
        next.ok()
    };
    let (swept, addresses) = discovery.progress();
    let (probed, planned) = discovery.ports();
    let count = |filter| int(discovery.count(filter));
    bridge.set_discovery(ui::DiscoveryView {
        phase: phase(discovery.phase()),
        target: target.map(|t| t.to_string()).unwrap_or_default().into(),
        target_error: target_error(next.err()),
        mode: scan_mode(discovery.options.mode),
        swept: int(swept),
        addresses: int(addresses),
        probed: int(probed),
        probes: int(planned),
        seconds: seconds(discovery.elapsed()),
        filter: filter(discovery.filter),
        sort: sort(discovery.sort),
        sort_descending: discovery.sort_descending,
        count_all: count(FoundFilter::All),
        count_new: count(FoundFilter::New),
        count_unidentified: count(FoundFilter::Unidentified),
        count_duplicates: count(FoundFilter::Duplicates),
        importable: int(discovery.importable()),
    });

    push_found(bridge, labels, state);

    let log = discovery
        .log()
        .iter()
        .rev()
        .map(|entry| log_view(state, entry));
    if let Some(model) = sync(&bridge.get_discovery_log(), log.collect()) {
        bridge.set_discovery_log(model);
    }
    bridge.set_scanning(discovery.busy());
}

/// The results, each host followed by its cards, and the host or card shown in the details.
fn push_found(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let discovery = &state.discovery;
    let shown = discovery.shown().into_iter().filter_map(|(index, depth)| {
        let found = discovery.found().get(index)?;
        let row = ui::FoundView {
            depth: int(depth),
            ..found_view(state, labels, index, found)
        };
        let cards = card_rows(state, &row, found);
        Some(std::iter::once(row).chain(cards))
    });
    if let Some(model) = sync(&bridge.get_found(), shown.flatten().collect()) {
        bridge.set_found(model);
    }
    let selected = discovery
        .selected
        .and_then(|index| Some((index, discovery.found().get(index)?)));
    let details = selected.map(|(index, f)| found_view(state, labels, index, f));
    let cards = selected
        .zip(details.as_ref())
        .map(|((_, f), row)| card_rows(state, row, f));
    if let Some(model) = sync(&bridge.get_found_cards(), cards.unwrap_or_default()) {
        bridge.set_found_cards(model);
    }
    let card = selected.and_then(|(_, f)| selected_card(state, f));
    bridge.set_selected_found_card(card.map_or(-1, int));
    let ports = selected
        .zip(card)
        .and_then(|((_, found), at)| {
            let device = found
                .on_map()
                .and_then(|id| state.network.device(id))
                .unwrap_or(&found.device);
            device.cards.get(at)
        })
        .map(convert::port_views)
        .unwrap_or_default();
    if let Some(model) = sync(&bridge.get_found_ports(), ports) {
        bridge.set_found_ports(model);
    }
    let parents = state.found_parent_choices();
    let own = selected.map(|(index, _)| Parent::Found(index));
    let at = parents.iter().position(|&p| Some(p) == own);
    bridge.set_found_card_parent_index(at.map_or(-1, int));
    let names = parents.into_iter().map(|p| parent_name(labels, state, p));
    if let Some(model) = sync(&bridge.get_found_parent_choices(), names.collect()) {
        bridge.set_found_parent_choices(model);
    }
    bridge.set_found_details(details.unwrap_or_default());
    bridge.set_selected_found(selected.map_or(-1, |(index, _)| int(index)));
}

/// A card's parent as the lists name it: a found host's name, or a device's.
pub(super) fn parent_name(labels: &Labels<'_>, state: &AppState, parent: Parent) -> SharedString {
    match parent {
        Parent::Found(index) => {
            let found = state.discovery.found().get(index);
            let name = found.map(|f| f.device.name.trim()).unwrap_or_default();
            labels.invoke_found_name(name.into())
        }
        Parent::Device(id) => {
            let name = state.network.device(id).map(|d| d.name.trim());
            labels.invoke_device_name(name.unwrap_or_default().into())
        }
    }
}

fn int(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// A time into a run, in whole seconds.
fn seconds(time: Duration) -> i32 {
    i32::try_from(time.as_secs()).unwrap_or(i32::MAX)
}

fn found_view(state: &AppState, labels: &Labels<'_>, index: usize, found: &Found) -> ui::FoundView {
    let network = &state.network;
    let device = &found.device;
    // Not once it has been forgotten from the map.
    let on_map = found.on_map().filter(|&id| network.device(id).is_some());
    let spot = on_map
        .and_then(|id| network.device(id)?.primary_card())
        .and_then(|card| state.positions.get(&card.id));
    let at = spot.copied().unwrap_or_default();
    let role_index = network.roles().iter().position(|r| *r == device.role);
    let duplicate_of = match found.finding {
        Finding::Duplicate(id) => network.device(id).map(|d| d.name.trim()),
        Finding::Known(_) | Finding::New => None,
    };
    let card = device
        .cards
        .iter()
        .find(|c| c.address.is_some_and(|a| a.addr() == found.ip));
    let text = |value: Option<&str>| SharedString::from(value.unwrap_or_default());
    let methods: Vec<_> = found.methods.iter().map(|&m| method(m)).collect();
    let (os_family, os_detail) = convert::os(device);
    ui::FoundView {
        index: int(index),
        card_index: -1,
        seen: true,
        device_id: on_map.map_or(-1, convert::ui_id),
        map_x: at.x,
        map_y: at.y,
        name: device.name.trim().into(),
        ip: found.ip.to_string().into(),
        mac: found.mac.map(|m| m.to_string()).unwrap_or_default().into(),
        hostname: text(device.hostname.as_deref()),
        manufacturer: text(device.manufacturer.as_deref()),
        model: text(device.model.as_deref()),
        kind: convert::kind(device.kind),
        role: convert::role_label(labels, network, &device.role),
        role_index: role_index.map_or(-1, int),
        colour: convert::colour(network.colour(device)),
        icon: state.icons.choice(&network.icon(device)),
        custom_icon: device.icon.is_some(),
        notes: device.notes.as_str().into(),
        services: card
            .map(|c| c.services.to_string())
            .unwrap_or_default()
            .into(),
        methods: ModelRc::new(VecModel::from(methods)),
        os_family,
        os_detail,
        os_clues: ModelRc::new(VecModel::from(convert::os_clues(device))),
        mdns_services: convert::mdns_services(device),
        unidentified: found.unidentified(),
        finding: finding(found.finding),
        duplicate_of: text(duplicate_of),
        decision: decision(found.decision),
        // Of a card's row; see `card_rows`.
        ..ui::FoundView::default()
    }
}

/// The place among `found`'s cards of the one shown in the details. For a host on the map, that
/// is the card selected there, which its editor may have moved on from.
fn selected_card(state: &AppState, found: &Found) -> Option<usize> {
    let network = &state.network;
    match found.on_map().and_then(|id| network.device(id)) {
        Some(device) => {
            let card = state.selected_card()?;
            device.cards.iter().position(|c| c.id == card)
        }
        None => state.discovery.selected_card,
    }
}

/// The row of each of `found`'s cards, made from `row`, the row of the host: all its cards, as
/// in the device list, with whether the run saw each. A host on the map shows its device's
/// cards there, which may have changed since, as cards moved to it.
fn card_rows(state: &AppState, row: &ui::FoundView, found: &Found) -> Vec<ui::FoundView> {
    let on_map = found.on_map().and_then(|id| state.network.device(id));
    let cards = &on_map.unwrap_or(&found.device).cards;
    let rows = cards.iter().enumerate().map(|(at, card)| ui::FoundView {
        card_index: int(at),
        card_name: card.name.trim().into(),
        card_icon: state.icons.choice(&card.icon),
        seen: found.seen(card),
        last: at + 1 == cards.len(),
        ip: convert::ip(card),
        mask: convert::mask(card),
        gateway: convert::gateway(card),
        mac: convert::mac(card),
        vendor: convert::vendor(card),
        services: card.services.to_string().into(),
        ..row.clone()
    });
    rows.collect()
}

fn log_view(state: &AppState, entry: &LogEntry) -> ui::LogView {
    let base = ui::LogView {
        seconds: seconds(entry.at),
        warning: entry.event.is_warning(),
        ..ui::LogView::default()
    };
    match &entry.event {
        LogEvent::Started { target } => ui::LogView {
            event: ui::LogEvent::Started,
            text: target.to_string().into(),
            ..base
        },
        LogEvent::Sweep { addresses } => ui::LogView {
            event: ui::LogEvent::Sweep,
            count: int(*addresses),
            ..base
        },
        LogEvent::Discovered { name, ip } => ui::LogView {
            event: ui::LogEvent::Discovered,
            text: name.as_str().into(),
            ip: ip.to_string().into(),
            ..base
        },
        LogEvent::Unidentified { ip, mac } => ui::LogView {
            event: ui::LogEvent::Unidentified,
            ip: ip.to_string().into(),
            mac: mac.map(|m| m.to_string()).unwrap_or_default().into(),
            ..base
        },
        LogEvent::PossibleDuplicate { ip, of } => ui::LogView {
            event: ui::LogEvent::Duplicate,
            ip: ip.to_string().into(),
            text: state
                .network
                .device(*of)
                .map(|d| d.name.trim())
                .unwrap_or_default()
                .into(),
            ..base
        },
        LogEvent::Failed { reason } => ui::LogView {
            event: ui::LogEvent::Failed,
            text: reason.as_str().into(),
            ..base
        },
        LogEvent::Stopped { found } => ui::LogView {
            event: ui::LogEvent::Stopped,
            count: int(*found),
            ..base
        },
        LogEvent::Finished { found } => ui::LogView {
            event: ui::LogEvent::Finished,
            count: int(*found),
            ..base
        },
    }
}

fn phase(phase: Phase) -> ui::DiscoveryPhase {
    match phase {
        Phase::Idle => ui::DiscoveryPhase::Idle,
        Phase::Running => ui::DiscoveryPhase::Running,
        Phase::Finished => ui::DiscoveryPhase::Finished,
    }
}

fn target_error(error: Option<TargetError>) -> ui::TargetError {
    match error {
        None => ui::TargetError::None,
        Some(TargetError::Empty) => ui::TargetError::Empty,
        Some(TargetError::Invalid) => ui::TargetError::Invalid,
        Some(TargetError::TooWide) => ui::TargetError::TooWide,
    }
}

fn filter(filter: FoundFilter) -> ui::FoundFilter {
    match filter {
        FoundFilter::All => ui::FoundFilter::All,
        FoundFilter::New => ui::FoundFilter::New,
        FoundFilter::Unidentified => ui::FoundFilter::Unidentified,
        FoundFilter::Duplicates => ui::FoundFilter::Duplicates,
    }
}

fn scan_mode(mode: ScanMode) -> ui::ScanMode {
    match mode {
        ScanMode::Quick => ui::ScanMode::Quick,
        ScanMode::Full => ui::ScanMode::Full,
        ScanMode::Everything => ui::ScanMode::Everything,
    }
}

fn sort(sort: FoundSort) -> ui::FoundSort {
    match sort {
        FoundSort::Name => ui::FoundSort::Name,
        FoundSort::Ip => ui::FoundSort::Ip,
        FoundSort::Mac => ui::FoundSort::Mac,
        FoundSort::Role => ui::FoundSort::Role,
    }
}

fn method(method: Method) -> ui::DiscoveryMethod {
    match method {
        Method::Arp => ui::DiscoveryMethod::Arp,
        Method::Ping => ui::DiscoveryMethod::Ping,
        Method::Mdns => ui::DiscoveryMethod::Mdns,
    }
}

fn finding(finding: Finding) -> ui::FoundFinding {
    match finding {
        Finding::Known(_) => ui::FoundFinding::Known,
        Finding::New => ui::FoundFinding::New,
        Finding::Duplicate(_) => ui::FoundFinding::Duplicate,
    }
}

fn decision(decision: Decision) -> ui::FoundDecision {
    match decision {
        Decision::Pending => ui::FoundDecision::Pending,
        Decision::Added(_) => ui::FoundDecision::Added,
        Decision::Ignored => ui::FoundDecision::Ignored,
    }
}
