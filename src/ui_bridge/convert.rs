//! Conversion of model data into the Slint structs declared in `ui/bridge.slint`.
//! Only data crosses over; wording is added on the Slint side so it can be translated.

use slint::SharedString;

use crate::model::{
    ConnectionKind, Device, DeviceId, DeviceKind, Network, Point, Positions, Role, Status,
    StatusFilter, route,
};
use crate::ui::{self, Labels};

/// Dash and gap lengths of inferred links, in map units.
const DASH: f32 = 6.0;
const GAP: f32 = 4.0;
/// Room around the outermost icons for their labels, in map units.
const PAD_SIDE: f32 = 80.0;
const PAD_TOP: f32 = 40.0;
const PAD_BOTTOM: f32 = 80.0;

pub(super) fn ui_id(id: DeviceId) -> i32 {
    // Ids come from our own data and stay far below i32::MAX; -1 means "no device" to the UI.
    i32::try_from(id.0).unwrap_or(-1)
}

pub(super) fn device_id(id: i32) -> Option<DeviceId> {
    u32::try_from(id).ok().map(DeviceId)
}

pub(super) fn filter_from_ui(filter: ui::StatusFilter) -> StatusFilter {
    match filter {
        ui::StatusFilter::All => StatusFilter::All,
        ui::StatusFilter::Online => StatusFilter::Online,
        ui::StatusFilter::Offline => StatusFilter::Offline,
    }
}

pub(super) fn filter_to_ui(filter: StatusFilter) -> ui::StatusFilter {
    match filter {
        StatusFilter::All => ui::StatusFilter::All,
        StatusFilter::Online => ui::StatusFilter::Online,
        StatusFilter::Offline => ui::StatusFilter::Offline,
    }
}

fn kind(kind: DeviceKind) -> ui::DeviceKind {
    match kind {
        DeviceKind::Router => ui::DeviceKind::Router,
        DeviceKind::Switch => ui::DeviceKind::Switch,
        DeviceKind::AccessPoint => ui::DeviceKind::AccessPoint,
        DeviceKind::Nas => ui::DeviceKind::Nas,
        DeviceKind::Desktop => ui::DeviceKind::Desktop,
        DeviceKind::Laptop => ui::DeviceKind::Laptop,
        DeviceKind::Server => ui::DeviceKind::Server,
        DeviceKind::Printer => ui::DeviceKind::Printer,
        DeviceKind::Phone => ui::DeviceKind::Phone,
        DeviceKind::Tv => ui::DeviceKind::Tv,
        DeviceKind::Console => ui::DeviceKind::Console,
        DeviceKind::Camera => ui::DeviceKind::Camera,
        DeviceKind::Smart => ui::DeviceKind::Smart,
        DeviceKind::Unknown => ui::DeviceKind::Unknown,
    }
}

/// The icon picked for a new custom role.
pub(super) fn kind_from_ui(kind: ui::DeviceKind) -> DeviceKind {
    match kind {
        ui::DeviceKind::Router => DeviceKind::Router,
        ui::DeviceKind::Switch => DeviceKind::Switch,
        ui::DeviceKind::AccessPoint => DeviceKind::AccessPoint,
        ui::DeviceKind::Nas => DeviceKind::Nas,
        ui::DeviceKind::Desktop => DeviceKind::Desktop,
        ui::DeviceKind::Laptop => DeviceKind::Laptop,
        ui::DeviceKind::Server => DeviceKind::Server,
        ui::DeviceKind::Printer => DeviceKind::Printer,
        ui::DeviceKind::Phone => DeviceKind::Phone,
        ui::DeviceKind::Tv => DeviceKind::Tv,
        ui::DeviceKind::Console => DeviceKind::Console,
        ui::DeviceKind::Camera => DeviceKind::Camera,
        ui::DeviceKind::Smart => DeviceKind::Smart,
        ui::DeviceKind::Unknown => DeviceKind::Unknown,
    }
}

fn status(status: Status) -> ui::DeviceStatus {
    match status {
        Status::Online => ui::DeviceStatus::Online,
        Status::Offline => ui::DeviceStatus::Offline,
        Status::Unknown => ui::DeviceStatus::Unknown,
    }
}

/// The role as shown to the user: translated for standard roles, as typed for custom ones.
pub(super) fn role_label(labels: &Labels<'_>, role: &Role) -> SharedString {
    let standard = match role {
        Role::Router => ui::RoleKind::Router,
        Role::Switch => ui::RoleKind::Switch,
        Role::AccessPoint => ui::RoleKind::AccessPoint,
        Role::Server => ui::RoleKind::Server,
        Role::Nas => ui::RoleKind::Nas,
        Role::Desktop => ui::RoleKind::Desktop,
        Role::Laptop => ui::RoleKind::Laptop,
        Role::Phone => ui::RoleKind::Phone,
        Role::Printer => ui::RoleKind::Printer,
        Role::Camera => ui::RoleKind::Camera,
        Role::Iot => ui::RoleKind::Iot,
        Role::Other => ui::RoleKind::Other,
        Role::Custom(name) => return name.as_str().into(),
    };
    labels.invoke_role(standard)
}

pub(super) fn device_view(
    device: &Device,
    role: SharedString,
    icon: DeviceKind,
    at: Point,
    dimmed: bool,
) -> ui::DeviceView {
    ui::DeviceView {
        id: ui_id(device.id),
        // Blank names are shown with a translated placeholder by the UI.
        name: device.name.trim().into(),
        ip: device.ip.to_string().into(),
        role,
        icon: kind(icon),
        status: status(device.status),
        x: at.x,
        y: at.y,
        dimmed,
    }
}

/// A role for the Roles tab, with `label` as shown in the role dropdown.
pub(super) fn role_view(network: &Network, role: &Role, label: SharedString) -> ui::RoleView {
    let icon = network.role_icon(role);
    let devices = network.devices().iter().filter(|d| d.role == *role).count();
    ui::RoleView {
        name: label,
        icon: kind(icon.unwrap_or(DeviceKind::Unknown)),
        has_icon: icon.is_some(),
        devices: i32::try_from(devices).unwrap_or(i32::MAX),
        custom: matches!(role, Role::Custom(_)),
    }
}

pub(super) fn details(network: &Network, device: &Device, role: SharedString) -> ui::DeviceDetails {
    let text = |value: Option<&str>| SharedString::from(value.unwrap_or_default());
    let uplink = network.uplink(device.id).and_then(|link| {
        let upstream = network.device(link.upstream)?;
        Some((upstream.name.trim(), link.kind == ConnectionKind::Inferred))
    });
    let services: Vec<String> = device.services.iter().map(ToString::to_string).collect();
    ui::DeviceDetails {
        kind: kind(device.kind),
        icon: kind(network.icon(device)),
        custom_icon: device.icon.is_some(),
        status: status(device.status),
        name: device.name.trim().into(),
        role,
        notes: device.notes.as_str().into(),
        ip: device.ip.to_string().into(),
        mac: device.mac.to_string().into(),
        manufacturer: text(device.manufacturer),
        model: text(device.model),
        hostname: text(device.hostname),
        last_seen_minutes: i32::try_from(device.last_seen.minutes_ago).unwrap_or(i32::MAX),
        uplink_name: uplink.map(|(name, _)| name).unwrap_or_default().into(),
        uplink_inferred: uplink.is_some_and(|(_, inferred)| inferred),
        services: services.join(", ").into(),
    }
}

/// SVG path commands for every connection, grouped by line style.
pub(super) fn lines(
    network: &Network,
    positions: &Positions,
    selected: Option<DeviceId>,
) -> ui::MapLines {
    let at = |id| positions.get(&id).copied().unwrap_or_default();
    let (mut known, mut inferred) = (Vec::new(), Vec::new());
    let (mut selected_known, mut selected_inferred) = (Vec::new(), Vec::new());
    for link in network.connections() {
        let points = route(at(link.upstream), at(link.downstream));
        let highlighted = selected.is_some_and(|id| id == link.upstream || id == link.downstream);
        match (link.kind, highlighted) {
            (ConnectionKind::Known, false) => known.push(solid(&points)),
            (ConnectionKind::Known, true) => selected_known.push(solid(&points)),
            (ConnectionKind::Inferred, false) => inferred.push(dashed(&points)),
            (ConnectionKind::Inferred, true) => selected_inferred.push(dashed(&points)),
        }
    }
    ui::MapLines {
        known: known.join(" ").into(),
        inferred: inferred.join(" ").into(),
        selected_known: selected_known.join(" ").into(),
        selected_inferred: selected_inferred.join(" ").into(),
    }
}

/// The map area holding every node and its labels.
pub(super) fn bounds(positions: &Positions) -> ui::MapBounds {
    let mut points = positions.values();
    let Some(first) = points.next() else {
        return ui::MapBounds {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
    };
    let (min, max) = points.fold((*first, *first), |(min, max), p| {
        (
            Point {
                x: min.x.min(p.x),
                y: min.y.min(p.y),
            },
            Point {
                x: max.x.max(p.x),
                y: max.y.max(p.y),
            },
        )
    });
    ui::MapBounds {
        x: min.x - PAD_SIDE,
        y: min.y - PAD_TOP,
        width: max.x - min.x + 2.0 * PAD_SIDE,
        height: max.y - min.y + PAD_TOP + PAD_BOTTOM,
    }
}

/// A polyline as SVG path commands.
fn solid(points: &[Point]) -> String {
    let commands = points.iter().enumerate().map(|(i, p)| {
        let command = if i == 0 { 'M' } else { 'L' };
        format!("{command}{:.1} {:.1}", p.x, p.y)
    });
    commands.collect::<Vec<_>>().join(" ")
}

/// A polyline as a series of short dashes, each its own SVG sub-path.
fn dashed(points: &[Point]) -> String {
    let mut dashes = Vec::new();
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let length = (b.x - a.x).hypot(b.y - a.y);
        let along = |distance: f32| {
            let t = distance / length;
            Point {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
            }
        };
        let mut start = 0.0;
        while start < length {
            dashes.push(solid(&[along(start), along((start + DASH).min(length))]));
            start += DASH + GAP;
        }
    }
    dashes.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashed_line_breaks_into_dashes_that_stay_on_the_line() {
        let line = [Point { x: 0.0, y: 0.0 }, Point { x: 0.0, y: 25.0 }];
        let expected = "M0.0 0.0 L0.0 6.0 M0.0 10.0 L0.0 16.0 M0.0 20.0 L0.0 25.0";
        assert_eq!(dashed(&line), expected);
    }

    #[test]
    fn bounds_leave_room_for_labels_around_every_node() {
        let positions = Positions::from([
            (DeviceId(1), Point { x: 100.0, y: 0.0 }),
            (DeviceId(2), Point { x: 400.0, y: 300.0 }),
        ]);
        let b = bounds(&positions);
        assert_eq!((b.x, b.y), (100.0 - PAD_SIDE, -PAD_TOP));
        assert_eq!(
            (b.width, b.height),
            (300.0 + 2.0 * PAD_SIDE, 300.0 + PAD_TOP + PAD_BOTTOM)
        );
    }

    #[test]
    fn selection_moves_its_links_into_the_highlighted_styles() {
        let network = crate::mock_data::network();
        let positions = crate::model::tree_layout(&network);
        let plain = lines(&network, &positions, None);
        assert!(plain.selected_known.is_empty() && plain.selected_inferred.is_empty());

        let access_point = DeviceId(3);
        let highlighted = lines(&network, &positions, Some(access_point));
        assert!(
            !highlighted.selected_known.is_empty(),
            "uplink to the router"
        );
        assert!(
            !highlighted.selected_inferred.is_empty(),
            "wireless clients"
        );
    }
}
