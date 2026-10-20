//! Sends the discovery run to the UI: its progress and options, the found hosts the filter lets
//! through, the selected one's details, and the log.

use slint::{ModelRc, SharedString, VecModel};

use super::sync;
use crate::model::{Decision, Finding, Found, FoundFilter, LogEntry, LogEvent, Method, Phase};
use crate::ui::{self, Bridge, Labels};
use crate::ui_bridge::discovery::STEP;
use crate::ui_bridge::{AppState, convert};

pub(super) fn push(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let discovery = &state.discovery;
    let subnets = state.network.subnets();
    let names = subnets.iter().map(|s| SharedString::from(s.to_string()));
    if let Some(model) = sync(&bridge.get_discovery_targets(), names.collect()) {
        bridge.set_discovery_targets(model);
    }
    let target = state.discovery_target();
    let target_index = target.and_then(|t| subnets.iter().position(|&s| s == t));
    let (swept, addresses) = discovery.progress();
    let count = |filter| int(discovery.count(filter));
    bridge.set_discovery(ui::DiscoveryView {
        phase: phase(discovery.phase()),
        target: target.map(|t| t.to_string()).unwrap_or_default().into(),
        target_index: target_index.map_or(-1, int),
        ping_sweep: discovery.options.ping_sweep,
        port_scan: discovery.options.port_scan,
        service_detection: discovery.options.service_detection,
        swept: int(swept),
        addresses: int(addresses),
        seconds: seconds(discovery.steps()),
        filter: filter(discovery.filter),
        count_all: count(FoundFilter::All),
        count_new: count(FoundFilter::New),
        count_unidentified: count(FoundFilter::Unidentified),
        count_duplicates: count(FoundFilter::Duplicates),
    });

    let found = discovery.found().iter().enumerate();
    let shown = found.filter(|(_, f)| discovery.filter.admits(f));
    let rows = shown.map(|(index, f)| found_view(state, labels, index, f));
    if let Some(model) = sync(&bridge.get_found(), rows.collect()) {
        bridge.set_found(model);
    }
    let selected = discovery
        .selected
        .and_then(|index| Some((index, discovery.found().get(index)?)));
    let details = selected.map(|(index, f)| found_view(state, labels, index, f));
    bridge.set_found_details(details.unwrap_or_default());
    bridge.set_selected_found(selected.map_or(-1, |(index, _)| int(index)));

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

fn int(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// The time a run took to reach its `steps`th step, in whole seconds.
fn seconds(steps: u32) -> i32 {
    let millis = STEP.as_millis().saturating_mul(u128::from(steps));
    i32::try_from(millis / 1000).unwrap_or(i32::MAX)
}

fn found_view(state: &AppState, labels: &Labels<'_>, index: usize, found: &Found) -> ui::FoundView {
    let network = &state.network;
    let device = &found.device;
    let on_map = found.on_map();
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
    ui::FoundView {
        index: int(index),
        device_id: on_map.map_or(-1, convert::ui_id),
        map_x: at.x,
        map_y: at.y,
        name: device.name.trim().into(),
        ip: found.ip.to_string().into(),
        mac: found.mac.map(|m| m.to_string()).unwrap_or_default().into(),
        hostname: text(device.hostname),
        manufacturer: text(device.manufacturer),
        model: text(device.model),
        kind: convert::kind(device.kind),
        role: convert::role_label(labels, network, &device.role),
        role_index: role_index.map_or(-1, int),
        icon: state.icons.image(&network.icon(device)),
        services: card
            .map(|c| c.services.to_string())
            .unwrap_or_default()
            .into(),
        methods: ModelRc::new(VecModel::from(methods)),
        confidence: i32::from(found.confidence),
        unidentified: found.unidentified(),
        finding: finding(found.finding),
        duplicate_of: text(duplicate_of),
        decision: decision(found.decision),
    }
}

fn log_view(state: &AppState, entry: &LogEntry) -> ui::LogView {
    let base = ui::LogView {
        seconds: seconds(entry.step),
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
        LogEvent::Paused => ui::LogView {
            event: ui::LogEvent::Paused,
            ..base
        },
        LogEvent::Resumed => ui::LogView {
            event: ui::LogEvent::Resumed,
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
        Phase::Paused => ui::DiscoveryPhase::Paused,
        Phase::Finished => ui::DiscoveryPhase::Finished,
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

fn method(method: Method) -> ui::DiscoveryMethod {
    match method {
        Method::Arp => ui::DiscoveryMethod::Arp,
        Method::Ping => ui::DiscoveryMethod::Ping,
        Method::Snmp => ui::DiscoveryMethod::Snmp,
        Method::Mdns => ui::DiscoveryMethod::Mdns,
        Method::Upnp => ui::DiscoveryMethod::Upnp,
        Method::Onvif => ui::DiscoveryMethod::Onvif,
        Method::Lldp => ui::DiscoveryMethod::Lldp,
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
