//! Conversion of model data into the Slint structs declared in `ui/bridge.slint`.
//! Only data crosses over; wording is added on the Slint side so it can be translated.

use slint::SharedString;

use super::icons::Icons;
use crate::model::{
    Connection, ConnectionKind, Device, DeviceId, DeviceKind, GroupColour, Icon, Network, Point,
    Positions, Role, Snap, Status, StatusFilter, route,
};
use crate::ui::{self, Labels};

/// How far a slack strand sags under its own weight, as a share of its length.
const SAG: f32 = 0.3;
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

/// A list index from the UI; a negative one is out of range of every list.
pub(super) fn index(index: i32) -> usize {
    usize::try_from(index).unwrap_or(usize::MAX)
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

/// An icon picked in the UI, by its id; an empty id is the Unknown icon.
pub(super) fn icon_from_ui(id: &str) -> Icon {
    if id.is_empty() {
        DeviceKind::Unknown.icon()
    } else {
        Icon::new(id)
    }
}

fn status(status: Status) -> ui::DeviceStatus {
    match status {
        Status::Online => ui::DeviceStatus::Online,
        Status::Offline => ui::DeviceStatus::Offline,
        Status::Unknown => ui::DeviceStatus::Unknown,
    }
}

/// The role as shown to the user: as typed if the user named it, otherwise translated.
pub(super) fn role_label(labels: &Labels<'_>, network: &Network, role: &Role) -> SharedString {
    if let Some(name) = network.role_name(role) {
        return name.into();
    }
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

fn colour(colour: Option<GroupColour>) -> ui::GroupColour {
    match colour {
        None => ui::GroupColour::None,
        Some(GroupColour::Red) => ui::GroupColour::Red,
        Some(GroupColour::Orange) => ui::GroupColour::Orange,
        Some(GroupColour::Yellow) => ui::GroupColour::Yellow,
        Some(GroupColour::Green) => ui::GroupColour::Green,
        Some(GroupColour::Blue) => ui::GroupColour::Blue,
        Some(GroupColour::Purple) => ui::GroupColour::Purple,
    }
}

/// A colour picked in the UI; `None` takes the group's colour away.
pub(super) fn colour_from_ui(colour: ui::GroupColour) -> Option<GroupColour> {
    match colour {
        ui::GroupColour::None => None,
        ui::GroupColour::Red => Some(GroupColour::Red),
        ui::GroupColour::Orange => Some(GroupColour::Orange),
        ui::GroupColour::Yellow => Some(GroupColour::Yellow),
        ui::GroupColour::Green => Some(GroupColour::Green),
        ui::GroupColour::Blue => Some(GroupColour::Blue),
        ui::GroupColour::Purple => Some(GroupColour::Purple),
    }
}

/// A device as drawn at `at`; the caller dims it when the search or filter hides it.
pub(super) fn device_view(
    network: &Network,
    icons: &Icons,
    device: &Device,
    role: SharedString,
    at: Point,
) -> ui::DeviceView {
    ui::DeviceView {
        id: ui_id(device.id),
        // Blank names are shown with a translated placeholder by the UI.
        name: device.name.trim().into(),
        ip: device.ip.to_string().into(),
        role,
        icon: icons.image(&network.icon(device)),
        colour: colour(network.colour(device)),
        status: status(device.status),
        x: at.x,
        y: at.y,
        dimmed: false,
    }
}

/// A role for the role list, with `label` as shown in the role dropdown.
pub(super) fn role_view(
    network: &Network,
    icons: &Icons,
    role: &Role,
    label: SharedString,
) -> ui::RoleView {
    let icon = network.role_icon(role);
    // Choice 0 in the group dropdown is "no group"; the groups follow in order.
    let group = network.role_group(role).and_then(|group| {
        let position = network.groups().iter().position(|(id, _)| id == group)?;
        i32::try_from(position + 1).ok()
    });
    ui::RoleView {
        name: label,
        icon: icon.map(|icon| icons.choice(icon)).unwrap_or_default(),
        has_icon: icon.is_some(),
        deletable: *role != Role::Other,
        group: group.unwrap_or(0),
    }
}

pub(super) fn details(
    network: &Network,
    icons: &Icons,
    device: &Device,
    role: SharedString,
) -> ui::DeviceDetails {
    let text = |value: Option<&str>| SharedString::from(value.unwrap_or_default());
    let uplink = network.uplink(device.id).and_then(|link| {
        let upstream = network.device(link.upstream)?;
        Some((upstream.name.trim(), link.kind == ConnectionKind::Inferred))
    });
    ui::DeviceDetails {
        kind: kind(device.kind),
        icon: icons.choice(&network.icon(device)),
        custom_icon: device.icon.is_some(),
        colour: colour(network.colour(device)),
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
        services: device.services.to_string().into(),
    }
}

/// The device's open ports, as rows to edit.
pub(super) fn port_views(device: &Device) -> Vec<ui::PortView> {
    let views = device.services.iter().map(|s| ui::PortView {
        port: i32::from(s.port),
        description: s.description.as_str().into(),
    });
    views.collect()
}

/// A port number as typed; anything but a whole number from 1 to 65535 is refused.
pub(super) fn port(text: &str) -> Option<u16> {
    text.trim().parse().ok().filter(|&port| port != 0)
}

/// The group list's rows, in order.
pub(super) fn group_views(network: &Network) -> Vec<ui::GroupView> {
    let groups = network.groups();
    groups
        .iter()
        .map(|(id, name)| ui::GroupView {
            name: name.into(),
            colour: colour(groups.colour(id)),
        })
        .collect()
}

/// How a line is drawn: its parent's group colour, dashed or not, highlighted or not.
type LineStyle = (Option<GroupColour>, ConnectionKind, bool);

/// SVG path commands for the `links`, one set per line style, highlighted sets last.
pub(super) fn lines(
    network: &Network,
    links: &[Connection],
    positions: &Positions,
    selected: Option<DeviceId>,
) -> Vec<ui::LineSet> {
    let at = |id| positions.get(&id).copied().unwrap_or_default();
    let mut sets: Vec<(LineStyle, Vec<String>)> = Vec::new();
    for link in links {
        let points = route(at(link.upstream), at(link.downstream));
        let highlighted = selected.is_some_and(|id| id == link.upstream || id == link.downstream);
        // A line takes the group colour of the device it hangs from, unless it is highlighted.
        let parent = network.device(link.upstream);
        let colour = parent
            .filter(|_| !highlighted)
            .and_then(|d| network.colour(d));
        let path = match link.kind {
            ConnectionKind::Known => solid(&points),
            ConnectionKind::Inferred => dashed(&points),
        };
        let style = (colour, link.kind, highlighted);
        match sets.iter_mut().find(|(s, _)| *s == style) {
            Some((_, paths)) => paths.push(path),
            None => sets.push((style, vec![path])),
        }
    }
    sets.sort_by_key(|((_, _, highlighted), _)| *highlighted);
    let sets = sets
        .into_iter()
        .map(|((group, kind, highlighted), paths)| ui::LineSet {
            commands: paths.join(" ").into(),
            colour: colour(group),
            inferred: kind == ConnectionKind::Inferred,
            selected: highlighted,
        });
    sets.collect()
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

/// The stretchy strand from the line a dragged device would join to the device: it sags when
/// slack and pulls straight as it stretches. No commands when there is none.
pub(super) fn strand(snap: Option<&Snap>) -> ui::Strand {
    let Some(snap) = snap else {
        return ui::Strand::default();
    };
    let [from, to] = snap.line;
    let sag = (to.x - from.x).hypot(to.y - from.y) * SAG * (1.0 - snap.pull);
    let bend = Point {
        x: f32::midpoint(from.x, to.x),
        y: f32::midpoint(from.y, to.y) + sag,
    };
    let commands = format!(
        "M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1}",
        from.x, from.y, bend.x, bend.y, to.x, to.y
    );
    ui::Strand {
        commands: commands.into(),
        pull: snap.pull,
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
    fn only_real_port_numbers_are_accepted() {
        assert_eq!(port(" 8080 "), Some(8080));
        assert_eq!(port("65535"), Some(65535));
        for refused in ["0", "65536", "-1", "80a", ""] {
            assert_eq!(port(refused), None, "{refused:?}");
        }
    }

    #[test]
    fn a_strand_sags_when_slack_and_pulls_straight_when_stretched() {
        let snap = |pull| Snap {
            upstream: DeviceId(1),
            before: None,
            line: [Point { x: 0.0, y: 0.0 }, Point { x: 40.0, y: 0.0 }],
            pull,
        };
        assert_eq!(
            strand(Some(&snap(0.5))).commands,
            "M0.0 0.0 Q20.0 6.0 40.0 0.0"
        );
        assert_eq!(
            strand(Some(&snap(1.0))).commands,
            "M0.0 0.0 Q20.0 0.0 40.0 0.0"
        );
        assert_eq!(strand(None).commands, "");
    }

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

    /// Whether some line set has the given look.
    fn has(sets: &[ui::LineSet], colour: ui::GroupColour, inferred: bool, selected: bool) -> bool {
        sets.iter()
            .any(|s| (s.colour, s.inferred, s.selected) == (colour, inferred, selected))
    }

    #[test]
    fn selection_highlights_its_links_and_draws_them_last() {
        let network = crate::mock_data::network();
        let positions = crate::model::tree_layout(&network);
        let plain = lines(&network, network.connections(), &positions, None);
        assert!(plain.iter().all(|s| !s.selected));

        let access_point = DeviceId(3);
        let highlighted = lines(
            &network,
            network.connections(),
            &positions,
            Some(access_point),
        );
        let none = ui::GroupColour::None;
        assert!(has(&highlighted, none, false, true), "uplink to the router");
        assert!(has(&highlighted, none, true, true), "wireless clients");
        let first_highlighted = highlighted.iter().position(|s| s.selected);
        let last_plain = highlighted.iter().rposition(|s| !s.selected);
        assert!(last_plain < first_highlighted);
    }

    #[test]
    fn lines_take_the_colour_of_the_device_they_hang_from() {
        let mut network = crate::mock_data::network();
        let positions = crate::model::tree_layout(&network);
        // A colour of its own, so other groups' lines cannot be mistaken for the router's.
        let group = network.add_group("Gateway").unwrap();
        network.set_group_colour(group, Some(GroupColour::Red));
        network.set_role_group(&Role::Router, Some(group));

        let sets = lines(&network, network.connections(), &positions, None);
        let red = sets
            .iter()
            .find(|s| s.colour == ui::GroupColour::Red && !s.inferred);
        let router_links = network
            .connections()
            .iter()
            .filter(|c| c.upstream == DeviceId(1));
        let expected = router_links
            .filter(|c| c.kind == ConnectionKind::Known)
            .count();
        let drawn = red.map(|s| s.commands.matches('M').count());
        assert_eq!(drawn, Some(expected), "the router's links, and only those");
    }
}
