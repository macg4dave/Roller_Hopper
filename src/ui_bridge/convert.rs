//! Conversion of model data into the Slint structs declared in `ui/bridge.slint`.
//! Only data crosses over; wording is added on the Slint side so it can be translated.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;
use slint::SharedString;

use super::icons::Icons;
use crate::model::{
    ConnectionKind, Device, DeviceId, DeviceKind, GroupColour, GuestKind, Icon, LineStyle, Network,
    NetworkCard, Point, Role, Snap, Status, StatusFilter,
};
use crate::ui::{self, Labels};

/// Room around the outermost icons for their labels and root circles, in map units; below them
/// there is room for every label line the View menu can turn on.
const PAD_SIDE: f32 = 80.0;
const PAD_TOP: f32 = 40.0;
const PAD_BOTTOM: f32 = 115.0;
/// The subnet prefix of an address typed without one: that of most home networks.
const TYPED_PREFIX: u8 = 24;

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

pub(super) fn colour(colour: Option<GroupColour>) -> ui::GroupColour {
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

/// A line style picked in the UI, by its index in the style dropdown.
pub(super) fn line_style_from_ui(choice: i32) -> Option<LineStyle> {
    LineStyle::ALL.get(index(choice)).copied()
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

/// A device whose settled spot is `at`; the caller dims it when the search or filter hides it.
pub(super) fn device_view(
    network: &Network,
    icons: &Icons,
    device: &Device,
    role: SharedString,
    at: Point,
) -> ui::DeviceView {
    let primary = device.primary_card();
    ui::DeviceView {
        id: ui_id(device.id),
        // Blank names are shown with a translated placeholder by the UI.
        name: device.name.trim().into(),
        ip: primary
            .map(|card| card.address.addr().to_string())
            .unwrap_or_default()
            .into(),
        mac: primary.map(mac).unwrap_or_default(),
        hostname: device.hostname.unwrap_or_default().into(),
        role,
        icon: icons.image(&network.icon(device)),
        colour: colour(network.colour(device)),
        status: status(device.status),
        x: at.x,
        y: at.y,
        dimmed: false,
        depth: i32::try_from(network.depth(device.id)).unwrap_or(i32::MAX),
    }
}

/// The MAC address as shown; empty for a card the user added.
fn mac(card: &NetworkCard) -> SharedString {
    card.mac
        .map(|mac| mac.to_string())
        .unwrap_or_default()
        .into()
}

/// The network cards of `device`, each with the name of the device it is plugged into.
pub(super) fn card_views(network: &Network, device: &Device) -> Vec<ui::CardView> {
    let views = device.cards.iter().map(|card| {
        let link = network.card_links().iter().find(|l| l.card == card.id);
        let via = link.and_then(|l| network.device(l.upstream));
        ui::CardView {
            address: card.address.to_string().into(),
            mac: mac(card),
            via_name: via.map(|d| d.name.trim()).unwrap_or_default().into(),
            inferred: link.is_some_and(|l| l.kind == ConnectionKind::Inferred),
        }
    });
    views.collect()
}

pub(super) fn guest_kind_from_ui(kind: ui::GuestKind) -> Option<GuestKind> {
    match kind {
        ui::GuestKind::None => None,
        ui::GuestKind::VirtualMachine => Some(GuestKind::VirtualMachine),
        ui::GuestKind::Container => Some(GuestKind::Container),
    }
}

fn guest_kind(kind: Option<GuestKind>) -> ui::GuestKind {
    match kind {
        None => ui::GuestKind::None,
        Some(GuestKind::VirtualMachine) => ui::GuestKind::VirtualMachine,
        Some(GuestKind::Container) => ui::GuestKind::Container,
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
    let name_of = |id| network.device(id).map(|d| d.name.trim());
    // A guest connects through its host, which is shown as where it runs instead.
    let uplink = network.uplink(device.id).filter(|_| device.guest.is_none());
    let uplink = uplink.and_then(|link| {
        Some((
            name_of(link.upstream)?,
            link.kind == ConnectionKind::Inferred,
        ))
    });
    let host = device.guest.and_then(|guest| name_of(guest.host));
    ui::DeviceDetails {
        kind: kind(device.kind),
        icon: icons.choice(&network.icon(device)),
        custom_icon: device.icon.is_some(),
        colour: colour(network.colour(device)),
        status: status(device.status),
        name: device.name.trim().into(),
        role,
        notes: device.notes.as_str().into(),
        guest: guest_kind(device.guest.map(|guest| guest.kind)),
        host_name: text(host),
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

/// An IPv4 address and subnet prefix as typed, such as `10.0.0.5/24`; without a prefix, the
/// usual home one.
pub(super) fn address(text: &str) -> Option<Ipv4Net> {
    let text = text.trim();
    match text.parse::<Ipv4Addr>() {
        Ok(ip) => Ipv4Net::new(ip, TYPED_PREFIX).ok(),
        Err(_) => text.parse().ok(),
    }
}

/// The group list's rows, in order.
pub(super) fn group_views(network: &Network) -> Vec<ui::GroupView> {
    let groups = network.groups();
    groups
        .iter()
        .map(|(id, name)| {
            let style = groups.line_style(id);
            let choice = LineStyle::ALL.iter().position(|&s| s == style);
            ui::GroupView {
                name: name.into(),
                colour: colour(groups.colour(id)),
                line_style: choice.and_then(|c| i32::try_from(c).ok()).unwrap_or(0),
            }
        })
        .collect()
}

/// The map area holding every node at `points`, and its labels.
pub(super) fn bounds(points: impl IntoIterator<Item = Point>) -> ui::MapBounds {
    let mut points = points.into_iter();
    let Some(first) = points.next() else {
        return ui::MapBounds {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
    };
    let (min, max) = points.fold((first, first), |(min, max), p| {
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

/// The stretchy strand from the line a dragged device would join to the device, curving through
/// `bend` as it swings (see [`super::motion`]). No commands when there is none.
pub(super) fn strand(snap: Option<&Snap>, bend: Option<Point>) -> ui::Strand {
    let Some(snap) = snap else {
        return ui::Strand::default();
    };
    let [from, to] = snap.line;
    // Straight until the bend starts to swing.
    let bend = bend.unwrap_or(Point {
        x: f32::midpoint(from.x, to.x),
        y: f32::midpoint(from.y, to.y),
    });
    let commands = format!(
        "M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1}",
        from.x, from.y, bend.x, bend.y, to.x, to.y
    );
    ui::Strand {
        commands: commands.into(),
        pull: snap.pull,
        join_x: from.x,
        join_y: from.y,
    }
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
    fn only_ipv4_addresses_are_accepted_with_the_home_prefix_by_default() {
        let typed = |text: &str| address(text).map(|net| net.to_string());
        assert_eq!(typed(" 10.0.0.5 "), Some("10.0.0.5/24".into()));
        assert_eq!(typed("172.17.0.9/16"), Some("172.17.0.9/16".into()));
        for refused in ["10.0.0", "10.0.0.256", "10.0.0.5/33", "fe80::1", ""] {
            assert_eq!(address(refused), None, "{refused:?}");
        }
    }

    #[test]
    fn a_strand_bends_where_it_swings_and_marks_where_it_joins() {
        let network = crate::mock_data::network();
        let arrangement = crate::model::Arrangement::default();
        let positions = crate::model::tree_layout(&network, &arrangement);
        let (pc, nas) = (DeviceId(5), DeviceId(4));
        let any =
            crate::model::snap_target(&network, &arrangement, &positions, pc, positions[&nas]);
        let snap = Snap {
            line: [Point { x: 10.0, y: 0.0 }, Point { x: 50.0, y: 0.0 }],
            ..any.unwrap()
        };
        let swinging = strand(Some(&snap), Some(Point { x: 30.0, y: 6.0 }));
        assert_eq!(swinging.commands, "M10.0 0.0 Q30.0 6.0 50.0 0.0");
        assert_eq!((swinging.join_x, swinging.join_y), (10.0, 0.0));
        let caught = strand(Some(&snap), None);
        assert_eq!(
            caught.commands, "M10.0 0.0 Q30.0 0.0 50.0 0.0",
            "straight at first"
        );
        assert_eq!(strand(None, None).commands, "");
    }

    #[test]
    fn bounds_leave_room_for_labels_around_every_node() {
        let b = bounds([Point { x: 100.0, y: 0.0 }, Point { x: 400.0, y: 300.0 }]);
        assert_eq!((b.x, b.y), (100.0 - PAD_SIDE, -PAD_TOP));
        assert_eq!(
            (b.width, b.height),
            (300.0 + 2.0 * PAD_SIDE, 300.0 + PAD_TOP + PAD_BOTTOM)
        );
    }

    #[test]
    fn a_line_style_is_picked_by_its_place_in_the_dropdown() {
        assert_eq!(line_style_from_ui(0), Some(LineStyle::Solid));
        assert_eq!(line_style_from_ui(7), Some(LineStyle::Wavy));
        assert_eq!(line_style_from_ui(8), None);
        assert_eq!(line_style_from_ui(-1), None);
    }
}
