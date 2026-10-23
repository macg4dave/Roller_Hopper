//! Conversion of model data into the Slint structs declared in `ui/bridge.slint`.
//! Only data crosses over; wording is added on the Slint side so it can be translated.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;
use slint::SharedString;

use super::icons::Icons;
use crate::model::{
    CardId, Device, DeviceId, DeviceKind, GroupColour, GroupName, Icon, LineStyle, Network,
    NetworkCard, OsClue, OsFamily, Point, Role, ServiceSource, Snap, StandardGroup, TargetError,
};
use crate::ui::{self, Labels};

/// Room around the outermost icons for their labels and root circles, in map units; below them
/// there is room for every label line the View menu can turn on.
const PAD_SIDE: f32 = 80.0;
const PAD_TOP: f32 = 40.0;
const PAD_BOTTOM: f32 = 115.0;
/// The subnet prefix of an address typed without one: that of most home networks.
pub(super) const TYPED_PREFIX: u8 = 24;

/// Why the typed network cannot be scanned, if it cannot; for the discovery page and the AI
/// settings window's scan.
pub(crate) fn target_error(error: Option<TargetError>) -> ui::TargetError {
    match error {
        None => ui::TargetError::None,
        Some(TargetError::Empty) => ui::TargetError::Empty,
        Some(TargetError::Invalid) => ui::TargetError::Invalid,
        Some(TargetError::TooWide) => ui::TargetError::TooWide,
    }
}

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

pub(super) fn ui_card_id(id: CardId) -> i32 {
    // As with devices, -1 means "no card" to the UI.
    i32::try_from(id.0).unwrap_or(-1)
}

pub(super) fn card_id(id: i32) -> Option<CardId> {
    u32::try_from(id).ok().map(CardId)
}

/// The card's address without its prefix, as shown; empty when it has none.
pub(super) fn ip(card: &NetworkCard) -> SharedString {
    let ip = card.address.map(|net| net.addr().to_string());
    ip.unwrap_or_default().into()
}

/// The subnet mask of the card's address, dotted; empty when it has no address.
pub(super) fn mask(card: &NetworkCard) -> SharedString {
    let mask = card.address.map(|net| net.netmask().to_string());
    mask.unwrap_or_default().into()
}

pub(super) fn gateway(card: &NetworkCard) -> SharedString {
    let gateway = card.gateway.map(|g| g.to_string());
    gateway.unwrap_or_default().into()
}

/// Who made the card; empty when unknown.
pub(super) fn vendor(card: &NetworkCard) -> SharedString {
    card.vendor.as_deref().unwrap_or_default().into()
}

/// The device's operating system family as guessed, and its detail (empty when none).
pub(super) fn os(device: &Device) -> (ui::OsFamily, SharedString) {
    let Some(os) = &device.os else {
        return (ui::OsFamily::Unknown, SharedString::new());
    };
    let family = match os.family {
        OsFamily::Windows => ui::OsFamily::Windows,
        OsFamily::MacOs => ui::OsFamily::Macos,
        OsFamily::Linux => ui::OsFamily::Linux,
        OsFamily::Ios => ui::OsFamily::Ios,
        OsFamily::Network => ui::OsFamily::Network,
    };
    (family, os.detail.as_deref().unwrap_or_default().into())
}

/// The clues behind the device's OS guess, strongest first.
pub(super) fn os_clues(device: &Device) -> Vec<ui::OsClueView> {
    let clues = device.os.iter().flat_map(|os| &os.clues);
    let view = |kind, ttl| ui::OsClueView { kind, ttl };
    clues
        .map(|clue| match *clue {
            OsClue::DeviceModel => view(ui::OsClueKind::DeviceModel, 0),
            OsClue::AppleSyncPort => view(ui::OsClueKind::AppleSyncPort, 0),
            OsClue::SshBanner => view(ui::OsClueKind::SshBanner, 0),
            OsClue::ServerHeader => view(ui::OsClueKind::ServerHeader, 0),
            OsClue::WindowsPorts => view(ui::OsClueKind::WindowsPorts, 0),
            OsClue::Ttl(ttl) => view(ui::OsClueKind::Ttl, i32::from(ttl)),
            OsClue::Ai => view(ui::OsClueKind::Ai, 0),
        })
        .collect()
}

/// The mDNS service types the device announced, as one line such as `http, ipp`.
pub(super) fn mdns_services(device: &Device) -> SharedString {
    device.mdns_services.join(", ").into()
}

/// The MAC address as shown; empty when there is none.
pub(super) fn mac(card: &NetworkCard) -> SharedString {
    let mac = card.mac.map(|mac| mac.to_string());
    mac.unwrap_or_default().into()
}

pub(super) fn kind(kind: DeviceKind) -> ui::DeviceKind {
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
        DeviceKind::Thermostat => ui::DeviceKind::Thermostat,
        DeviceKind::SmartDisplay => ui::DeviceKind::SmartDisplay,
        DeviceKind::VoiceAssistant => ui::DeviceKind::VoiceAssistant,
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

/// A device whose settled spot is `at`; the caller dims it when the search or filter hides it,
/// and makes each card's row and map node from it.
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
        card_id: -1,
        card_index: 0,
        card_name: SharedString::new(),
        drag_card: primary.map_or(-1, |c| ui_card_id(c.id)),
        card_icon: ui::IconChoice::default(),
        // Blank names are shown with a translated placeholder by the UI.
        name: device.name.trim().into(),
        ip: primary.map(ip).unwrap_or_default(),
        dhcp: primary.is_some_and(|card| card.dhcp),
        mac: primary.map(mac).unwrap_or_default(),
        hostname: device.hostname.as_deref().unwrap_or_default().into(),
        role,
        icon: icons.image(&network.icon(device)),
        colour: colour(network.colour(device)),
        x: at.x,
        y: at.y,
        dimmed: false,
        depth: 0,
        children: i32::try_from(device.cards.len()).unwrap_or(i32::MAX),
        expanded: true,
        last: false,
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
    let (os_family, os_detail) = os(device);
    ui::DeviceDetails {
        kind: kind(device.kind),
        icon: icons.choice(&network.icon(device)),
        custom_icon: device.icon.is_some(),
        colour: colour(network.colour(device)),
        name: device.name.trim().into(),
        role,
        notes: device.notes.as_str().into(),
        manufacturer: text(device.manufacturer.as_deref()),
        model: text(device.model.as_deref()),
        hostname: text(device.hostname.as_deref()),
        os_family,
        os_detail,
        mdns_services: mdns_services(device),
        last_seen_minutes: i32::try_from(device.last_seen.minutes_ago).unwrap_or(i32::MAX),
    }
}

/// The card's open ports, as rows to edit.
pub(super) fn port_views(card: &NetworkCard) -> Vec<ui::PortView> {
    let views = card.services.iter().map(|s| ui::PortView {
        port: i32::from(s.port),
        udp: s.protocol == crate::model::ServiceProtocol::Udp,
        description: s.description.as_str().into(),
        source: match s.source {
            ServiceSource::Manual => "manual",
            ServiceSource::Detected => "detected",
            ServiceSource::Registry => "registry",
            ServiceSource::Open => "open",
        }
        .into(),
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

/// The group as shown to the user: as typed if the user named it, otherwise translated.
fn group_label(labels: &Labels<'_>, name: &GroupName) -> SharedString {
    let standard = match name {
        GroupName::Named(name)
        | GroupName::Standard {
            renamed: Some(name),
            ..
        } => return name.as_str().into(),
        GroupName::Standard { standard, .. } => standard,
    };
    labels.invoke_standard_group(match standard {
        StandardGroup::NetworkCore => ui::StandardGroup::NetworkCore,
        StandardGroup::Switching => ui::StandardGroup::Switching,
        StandardGroup::WiFi => ui::StandardGroup::WiFi,
        StandardGroup::Mobile => ui::StandardGroup::Mobile,
        StandardGroup::Workstations => ui::StandardGroup::Workstations,
        StandardGroup::Printing => ui::StandardGroup::Printing,
        StandardGroup::Servers => ui::StandardGroup::Servers,
        StandardGroup::Storage => ui::StandardGroup::Storage,
        StandardGroup::Security => ui::StandardGroup::Security,
        StandardGroup::SmartHome => ui::StandardGroup::SmartHome,
    })
}

/// The group list's rows, in order.
pub(super) fn group_views(labels: &Labels<'_>, network: &Network) -> Vec<ui::GroupView> {
    let groups = network.groups();
    groups
        .iter()
        .map(|(id, name)| {
            let style = groups.line_style(id);
            let choice = LineStyle::ALL.iter().position(|&s| s == style);
            ui::GroupView {
                name: group_label(labels, name),
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
        let (pc, nas) = (CardId(5), CardId(4));
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
