//! The network as a whole: devices, how they connect, the roles on offer and their groups.

mod children;

use super::card::CardId;
use super::device::{Device, DeviceId, Icon, Role, Status};
use super::groups::{GroupColour, GroupId, Groups, LineStyle};
use super::roles::Roles;

/// Whether a link was observed directly or guessed from indirect evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ConnectionKind {
    Known,
    Inferred,
}

/// A network card's link to the device it is plugged into, towards the router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CardLink {
    pub(crate) upstream: DeviceId,
    pub(crate) card: CardId,
    pub(crate) kind: ConnectionKind,
}

/// A line of the map's tree, from a device towards the router (`upstream`) to one further away
/// (`downstream`): through the downstream device's network card, or from a guest's host to the
/// guest, which is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Connection {
    pub(crate) upstream: DeviceId,
    pub(crate) downstream: DeviceId,
    pub(crate) kind: ConnectionKind,
}

/// The device list filter tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum StatusFilter {
    #[default]
    All,
    Online,
    Offline,
}

impl StatusFilter {
    pub(crate) fn admits(self, status: Status) -> bool {
        match self {
            Self::All => true,
            Self::Online => status == Status::Online,
            Self::Offline => status == Status::Offline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Network {
    /// Each guest comes after its host, among the host's other guests, so every list shows the
    /// guests under their host.
    devices: Vec<Device>,
    links: Vec<CardLink>,
    /// The links as first found or added, which [`Self::restore_connections`] brings back.
    detected: Vec<CardLink>,
    roles: Roles,
    groups: Groups,
}

impl Network {
    pub(crate) fn new(devices: Vec<Device>, links: Vec<CardLink>) -> Self {
        Self {
            devices: children::hosts_first(devices),
            detected: links.clone(),
            links,
            roles: Roles::default(),
            groups: Groups::default(),
        }
    }

    pub(crate) fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub(crate) fn devices_mut(&mut self) -> impl Iterator<Item = &mut Device> {
        self.devices.iter_mut()
    }

    pub(crate) fn device(&self, id: DeviceId) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    pub(crate) fn device_mut(&mut self, id: DeviceId) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }

    pub(crate) fn card_links(&self) -> &[CardLink] {
        &self.links
    }

    /// The device a network card belongs to.
    pub(crate) fn card_owner(&self, card: CardId) -> Option<&Device> {
        let mut devices = self.devices.iter();
        devices.find(|d| d.cards.iter().any(|c| c.id == card))
    }

    /// The link of the card a physical device hangs from in the tree: its first card that has
    /// one. A guest hangs from its host instead.
    fn tree_link(&self, device: &Device) -> Option<&CardLink> {
        if device.guest.is_some() {
            return None;
        }
        let mut links = device.cards.iter().map(|card| card.id);
        links.find_map(|card| self.links.iter().find(|l| l.card == card))
    }

    /// Whether `link` is one of the tree's lines rather than a card's further link.
    fn in_tree(&self, link: &CardLink) -> bool {
        let owner = self.card_owner(link.card);
        owner.is_some_and(|d| self.tree_link(d) == Some(link))
    }

    fn connection(&self, link: &CardLink) -> Option<Connection> {
        Some(Connection {
            upstream: link.upstream,
            downstream: self.card_owner(link.card)?.id,
            kind: link.kind,
        })
    }

    /// The lines of the map's tree: those through cards in the order they were found or made,
    /// then those to guests.
    pub(crate) fn connections(&self) -> Vec<Connection> {
        let tree = self.links.iter().filter(|l| self.in_tree(l));
        let through_cards = tree.filter_map(|l| self.connection(l));
        let hosted = self.devices.iter().filter_map(|d| {
            Some(Connection {
                upstream: d.guest?.host,
                downstream: d.id,
                kind: ConnectionKind::Known,
            })
        });
        through_cards.chain(hosted).collect()
    }

    /// Links of further cards, which join a device to another part of the map, often another
    /// subnet, without moving it there.
    pub(crate) fn cross_links(&self) -> Vec<Connection> {
        let cross = self.links.iter().filter(|l| !self.in_tree(l));
        cross.filter_map(|l| self.connection(l)).collect()
    }

    /// The line leading from `id` towards the router, if any: a guest's leads to its host.
    pub(crate) fn uplink(&self, id: DeviceId) -> Option<Connection> {
        let device = self.device(id)?;
        if let Some(guest) = device.guest {
            return Some(Connection {
                upstream: guest.host,
                downstream: id,
                kind: ConnectionKind::Known,
            });
        }
        self.connection(self.tree_link(device)?)
    }

    /// The device `id` runs on, for a guest.
    pub(crate) fn host(&self, id: DeviceId) -> Option<DeviceId> {
        Some(self.device(id)?.guest?.host)
    }

    /// Whether `id` is `branch` itself or a device connected behind it.
    pub(crate) fn is_within(&self, id: DeviceId, branch: DeviceId) -> bool {
        let mut current = id;
        // Guards against cycles in the connection data: no real chain is longer than the network.
        for _ in 0..=self.devices.len() {
            if current == branch {
                return true;
            }
            match self.uplink(current) {
                Some(link) => current = link.upstream,
                None => return false,
            }
        }
        false
    }

    /// The devices connected straight behind `id`, in the order of [`Self::connections`].
    pub(crate) fn behind(&self, id: DeviceId) -> impl Iterator<Item = DeviceId> {
        let links = self.connections().into_iter();
        links
            .filter(move |c| c.upstream == id)
            .map(|c| c.downstream)
    }

    /// Connects `id` behind `upstream`, as a link the user made to the card it hangs from, or
    /// leaves it standing alone on `None`; the devices behind it go with it. Refused for a
    /// guest, which stays on its host, and when it would form a loop. A device that stays behind
    /// the same upstream keeps the link it had.
    pub(crate) fn connect(&mut self, id: DeviceId, upstream: Option<DeviceId>) {
        let unchanged = self.uplink(id).map(|link| link.upstream) == upstream;
        let refused =
            upstream.is_some_and(|up| self.device(up).is_none() || self.is_within(up, id));
        let Some(device) = self.device(id).filter(|d| d.guest.is_none()) else {
            return;
        };
        let primary = device.primary_card().map(|c| c.id);
        let Some(card) = self.tree_link(device).map(|l| l.card).or(primary) else {
            return;
        };
        if unchanged || refused {
            return;
        }
        self.links.retain(|l| l.card != card);
        if let Some(upstream) = upstream {
            self.links.push(CardLink {
                upstream,
                card,
                kind: ConnectionKind::Known,
            });
        }
    }

    /// Undoes [`Self::connect`]: back to the links as found, less any forgotten devices.
    pub(crate) fn restore_connections(&mut self) {
        self.links.clone_from(&self.detected);
    }

    pub(crate) fn filtered<'a>(
        &'a self,
        filter: StatusFilter,
        query: &'a str,
    ) -> impl Iterator<Item = &'a Device> {
        self.devices
            .iter()
            .filter(move |d| filter.admits(d.status) && d.matches(query))
    }

    /// How many devices, virtual machines and containers the filter lets through.
    pub(crate) fn count(&self, filter: StatusFilter) -> usize {
        self.filtered(filter, "").count()
    }

    /// Standard roles followed by the custom ones, in the order they were added.
    pub(crate) fn roles(&self) -> Vec<Role> {
        self.roles.roles().cloned().collect()
    }

    /// The name the user gave a role; `None` for a standard role still shown translated.
    pub(crate) fn role_name(&self, role: &Role) -> Option<&str> {
        self.roles.name(role)
    }

    /// The icon a role gives its devices; `None` when they keep their detected icon.
    pub(crate) fn role_icon(&self, role: &Role) -> Option<&Icon> {
        self.roles.icon(role)
    }

    /// The icon shown for a device: the one picked for it, else its role's, else the detected one.
    pub(crate) fn icon(&self, device: &Device) -> Icon {
        device
            .icon
            .as_ref()
            .or_else(|| self.role_icon(&device.role))
            .cloned()
            .unwrap_or_else(|| device.kind.icon())
    }

    /// Adds a custom role shown with `icon`, and returns it (see [`Roles::add`]).
    pub(crate) fn add_custom_role(&mut self, name: &str, icon: Icon) -> Option<Role> {
        self.roles.add(name, icon)
    }

    /// Renames a role, on every device that has it. Blank or taken names are ignored.
    pub(crate) fn rename_role(&mut self, role: &Role, new: &str) {
        let Some(renamed) = self.roles.rename(role, new) else {
            return;
        };
        for device in self.devices.iter_mut().filter(|d| d.role == *role) {
            device.role = renamed.clone();
        }
    }

    /// Sets the icon a role gives its devices; `None` lets each keep its detected icon.
    pub(crate) fn set_role_icon(&mut self, role: &Role, icon: Option<Icon>) {
        self.roles.set_icon(role, icon);
    }

    /// Deletes a role; its devices go back to Other. Other itself cannot be deleted.
    pub(crate) fn remove_role(&mut self, role: &Role) {
        if self.roles.remove(role) {
            for device in self.devices.iter_mut().filter(|d| d.role == *role) {
                device.role = Role::Other;
            }
        }
    }

    /// Whether [`Self::restore_standard_roles`] would change anything.
    pub(crate) fn standard_roles_changed(&self) -> bool {
        self.roles.standard_changed()
    }

    /// Brings back the standard roles' names and icons, and any that were deleted. Devices moved
    /// to Other by a deletion stay there.
    pub(crate) fn restore_standard_roles(&mut self) {
        self.roles.restore_standard();
    }

    pub(crate) fn groups(&self) -> &Groups {
        &self.groups
    }

    pub(crate) fn add_group(&mut self, name: &str) -> Option<GroupId> {
        self.groups.add(name)
    }

    pub(crate) fn rename_group(&mut self, group: GroupId, new: &str) {
        self.groups.rename(group, new);
    }

    /// Deletes a group; its roles are left in no group.
    pub(crate) fn remove_group(&mut self, group: GroupId) {
        self.groups.remove(group);
        self.roles.leave_group(group);
    }

    pub(crate) fn role_group(&self, role: &Role) -> Option<GroupId> {
        self.roles.group(role)
    }

    /// Puts a role in a group, or in none.
    pub(crate) fn set_role_group(&mut self, role: &Role, group: Option<GroupId>) {
        self.roles.set_group(role, group);
    }

    pub(crate) fn set_group_colour(&mut self, group: GroupId, colour: Option<GroupColour>) {
        self.groups.set_colour(group, colour);
    }

    /// The colour a device is shown in: that of its role's group, if the group has one.
    pub(crate) fn colour(&self, device: &Device) -> Option<GroupColour> {
        self.role_group(&device.role)
            .and_then(|group| self.groups.colour(group))
    }

    pub(crate) fn set_group_line_style(&mut self, group: GroupId, style: LineStyle) {
        self.groups.set_line_style(group, style);
    }

    /// How the lines from a device are drawn: in its role's group's style, solid in no group.
    pub(crate) fn line_style(&self, device: &Device) -> LineStyle {
        self.role_group(&device.role)
            .map(|group| self.groups.line_style(group))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::DeviceKind;

    #[test]
    fn filters_combine_status_and_search() {
        let network = mock_data::network();
        let offline: Vec<_> = network.filtered(StatusFilter::Offline, "").collect();
        assert!(!offline.is_empty());
        assert!(offline.iter().all(|d| d.status == Status::Offline));

        let total = network.count(StatusFilter::All);
        assert_eq!(total, network.devices().len());
        assert!(
            network.count(StatusFilter::Online) + network.count(StatusFilter::Offline) <= total
        );

        let routers: Vec<_> = network
            .filtered(StatusFilter::Online, "192.168.1.1")
            .collect();
        assert!(!routers.is_empty());
        assert!(routers.iter().all(|d| d.matches("192.168.1.1")));
    }

    #[test]
    fn editing_a_device_is_visible_through_lookup() {
        let mut network = mock_data::network();
        let id = network.devices().first().map(|d| d.id).unwrap();
        let device = network.device_mut(id).unwrap();
        device.name = "Gateway".into();
        device.role = Role::Server;
        device.notes = "Replace in spring".into();

        let edited = network.device(id).unwrap();
        assert_eq!(edited.name, "Gateway");
        assert_eq!(edited.role, Role::Server);
        assert_eq!(edited.notes, "Replace in spring");
        assert_eq!(network.filtered(StatusFilter::All, "gateway").count(), 1);
    }

    #[test]
    fn custom_roles_are_listed_after_standard_ones() {
        let mut network = Network::default();
        network.add_custom_role("Media server", DeviceKind::Server.icon());
        let roles = network.roles();
        assert_eq!(roles.len(), Role::STANDARD.len() + 1);
        assert_eq!(roles.last(), Some(&Role::Custom("Media server".into())));
    }

    #[test]
    fn the_icon_follows_the_role_unless_one_was_picked() {
        let mut network = mock_data::network();
        let media = network
            .add_custom_role("Media", DeviceKind::Tv.icon())
            .unwrap();
        let mut device = network.devices().first().unwrap().clone();
        for (role, expected) in [
            (Role::Printer, DeviceKind::Printer.icon()),
            (Role::Iot, DeviceKind::Smart.icon()),
            (media, DeviceKind::Tv.icon()),
            (Role::Other, device.kind.icon()),
        ] {
            device.role = role;
            assert_eq!(network.icon(&device), expected, "{:?}", device.role);
        }
        let dropped_in = Icon::new("My icons/Fish tank");
        device.icon = Some(dropped_in.clone());
        assert_eq!(network.icon(&device), dropped_in);
    }

    fn roles_of(network: &Network, role: &Role) -> usize {
        network.devices().iter().filter(|d| d.role == *role).count()
    }

    #[test]
    fn renaming_a_custom_role_renames_it_on_its_devices() {
        let mut network = mock_data::network();
        let tv = Role::Custom("TV".into());
        let television = Role::Custom("Television".into());
        let users = roles_of(&network, &tv);
        assert!(users > 0);
        network.rename_role(&tv, "Television");
        assert_eq!(roles_of(&network, &tv), 0);
        assert_eq!(roles_of(&network, &television), users);
        network.rename_role(&television, "games CONSOLE");
        assert_eq!(roles_of(&network, &television), users, "taken");
    }

    #[test]
    fn renaming_a_standard_role_keeps_its_devices() {
        let mut network = mock_data::network();
        let printers = roles_of(&network, &Role::Printer);
        network.rename_role(&Role::Printer, "Office printer");
        assert_eq!(network.role_name(&Role::Printer), Some("Office printer"));
        assert_eq!(roles_of(&network, &Role::Printer), printers);
    }

    #[test]
    fn deleting_a_role_gives_its_devices_other() {
        let mut network = mock_data::network();
        let tv = Role::Custom("TV".into());
        for role in [tv, Role::Printer] {
            let others = roles_of(&network, &Role::Other);
            let users = roles_of(&network, &role);
            assert!(users > 0, "{role:?}");
            network.remove_role(&role);
            assert!(!network.roles().contains(&role));
            assert_eq!(roles_of(&network, &Role::Other), others + users);
        }
        network.remove_role(&Role::Other);
        assert!(network.roles().contains(&Role::Other));
    }

    #[test]
    fn a_device_takes_the_colour_of_its_roles_group() {
        let mut network = mock_data::network();
        let printer = network
            .devices()
            .iter()
            .find(|d| d.role == Role::Printer)
            .cloned()
            .unwrap();
        let office = network.add_group("Print room").unwrap();
        network.set_group_colour(office, Some(GroupColour::Orange));
        network.set_role_group(&Role::Printer, None);
        assert_eq!(network.colour(&printer), None, "role in no group");
        network.set_role_group(&Role::Printer, Some(office));
        assert_eq!(network.colour(&printer), Some(GroupColour::Orange));
        network.remove_group(office);
        assert_eq!(network.colour(&printer), None, "group deleted");
    }

    #[test]
    fn a_device_takes_the_line_style_of_its_roles_group() {
        let mut network = mock_data::network();
        let printer = network
            .devices()
            .iter()
            .find(|d| d.role == Role::Printer)
            .cloned()
            .unwrap();
        let office = network.add_group("Print room").unwrap();
        network.set_group_line_style(office, LineStyle::Dotted);
        network.set_role_group(&Role::Printer, None);
        assert_eq!(network.line_style(&printer), LineStyle::Solid, "no group");
        network.set_role_group(&Role::Printer, Some(office));
        assert_eq!(network.line_style(&printer), LineStyle::Dotted);
        network.remove_group(office);
        assert_eq!(network.line_style(&printer), LineStyle::Solid, "deleted");
    }

    const ROUTER: DeviceId = DeviceId(1);
    const SWITCH: DeviceId = DeviceId(2);
    const ACCESS_POINT: DeviceId = DeviceId(3);
    const PC: DeviceId = DeviceId(5);
    const LINUX_SERVER: DeviceId = DeviceId(6);
    /// A virtual machine on the Linux Server, with a further card on the lab network.
    const DOCKER_HOST: DeviceId = DeviceId(19);
    const LAB_ROUTER: DeviceId = DeviceId(22);

    #[test]
    fn a_guest_hangs_from_its_host_and_a_further_card_only_crosses_over() {
        let network = mock_data::network();
        let uplink = network.uplink(DOCKER_HOST);
        assert_eq!(uplink.map(|c| c.upstream), Some(LINUX_SERVER));
        assert!(network.behind(LINUX_SERVER).any(|d| d == DOCKER_HOST));
        assert!(network.behind(LAB_ROUTER).all(|d| d != DOCKER_HOST));
        let cross = network.cross_links();
        assert!(
            cross
                .iter()
                .any(|c| (c.upstream, c.downstream) == (LAB_ROUTER, DOCKER_HOST))
        );
        let tree = network.connections();
        assert!(cross.iter().all(|c| !tree.contains(c)));
    }

    #[test]
    fn a_device_moves_to_another_line_with_its_branch() {
        let mut network = mock_data::network();
        network.connect(PC, Some(ROUTER));
        let link = network.uplink(PC);
        assert_eq!(
            link.map(|l| (l.upstream, l.kind)),
            Some((ROUTER, ConnectionKind::Known))
        );
        let to_pc = network
            .connections()
            .into_iter()
            .filter(|c| c.downstream == PC);
        assert_eq!(to_pc.count(), 1);

        network.connect(SWITCH, Some(ACCESS_POINT));
        assert!(network.is_within(PC, ROUTER));
        assert!(!network.is_within(PC, SWITCH), "the PC left the switch");
        assert!(
            network.is_within(LINUX_SERVER, ACCESS_POINT),
            "the switch kept its devices"
        );
        assert!(
            network.is_within(DOCKER_HOST, ACCESS_POINT),
            "and their guests"
        );
    }

    #[test]
    fn a_new_link_comes_last_and_an_unchanged_one_stays_as_found() {
        let mut network = mock_data::network();
        let behind = |n: &Network| n.behind(ROUTER).collect::<Vec<_>>();
        assert_eq!(behind(&network), [DeviceId(4), SWITCH, ACCESS_POINT]);
        network.connect(PC, Some(ROUTER));
        assert_eq!(behind(&network), [DeviceId(4), SWITCH, ACCESS_POINT, PC]);

        let phone = DeviceId(10);
        let found = network.uplink(phone);
        network.connect(phone, Some(ACCESS_POINT));
        assert_eq!(network.uplink(phone), found);
        assert_eq!(found.map(|l| l.kind), Some(ConnectionKind::Inferred));
    }

    #[test]
    fn a_device_cannot_join_its_own_branch() {
        let mut network = mock_data::network();
        let before = network.connections();
        network.connect(SWITCH, Some(PC));
        network.connect(SWITCH, Some(SWITCH));
        network.connect(SWITCH, Some(DeviceId(999)));
        assert_eq!(network.connections(), before);
    }

    #[test]
    fn a_guest_stays_on_its_host() {
        let mut network = mock_data::network();
        let before = network.connections();
        network.connect(DOCKER_HOST, Some(ROUTER));
        network.connect(DOCKER_HOST, None);
        assert_eq!(network.connections(), before);
    }

    #[test]
    fn a_device_left_alone_heads_its_own_branch_until_restored() {
        let mut network = mock_data::network();
        let before = network.connections();
        network.connect(SWITCH, None);
        assert!(network.uplink(SWITCH).is_none());
        assert!(network.is_within(PC, SWITCH));
        network.restore_connections();
        assert_eq!(network.connections(), before);
    }

    #[test]
    fn uplink_points_towards_the_router() {
        let network = mock_data::network();
        let router = network
            .devices()
            .iter()
            .find(|d| network.uplink(d.id).is_none());
        assert!(router.is_some_and(|r| r.role == Role::Router));
    }
}
