//! The network as a whole: devices, how they connect, the roles on offer and their groups.

use super::device::{Device, DeviceId, Icon, Role, Status};
use super::groups::{GroupColour, GroupId, Groups, LineStyle};
use super::roles::Roles;

/// Whether a link was observed directly or guessed from indirect evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ConnectionKind {
    Known,
    Inferred,
}

/// A link from a device towards the router (`upstream`) to one further away (`downstream`).
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
    devices: Vec<Device>,
    connections: Vec<Connection>,
    /// The connections as first found, which [`Self::restore_connections`] brings back.
    detected: Vec<Connection>,
    roles: Roles,
    groups: Groups,
}

impl Network {
    pub(crate) fn new(devices: Vec<Device>, connections: Vec<Connection>) -> Self {
        Self {
            devices,
            detected: connections.clone(),
            connections,
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

    pub(crate) fn connections(&self) -> &[Connection] {
        &self.connections
    }

    pub(crate) fn device(&self, id: DeviceId) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    pub(crate) fn device_mut(&mut self, id: DeviceId) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }

    /// Forgets a device and its links. Devices behind it stay, as separate branches of the map.
    pub(crate) fn remove_device(&mut self, id: DeviceId) {
        self.devices.retain(|d| d.id != id);
        for links in [&mut self.connections, &mut self.detected] {
            links.retain(|c| c.upstream != id && c.downstream != id);
        }
    }

    /// The connection leading from `id` towards the router, if any.
    pub(crate) fn uplink(&self, id: DeviceId) -> Option<&Connection> {
        self.connections.iter().find(|c| c.downstream == id)
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

    /// The devices connected straight behind `id`, in the order their links were found or made.
    pub(crate) fn behind(&self, id: DeviceId) -> impl Iterator<Item = DeviceId> {
        let links = self.connections.iter();
        links
            .filter(move |c| c.upstream == id)
            .map(|c| c.downstream)
    }

    /// Connects `id` behind `upstream`, as a link the user made, or leaves it standing alone on
    /// `None`; the devices behind it go with it. Refused when it would form a loop. A device that
    /// stays behind the same upstream keeps the link it had.
    pub(crate) fn connect(&mut self, id: DeviceId, upstream: Option<DeviceId>) {
        let unchanged = self.uplink(id).map(|link| link.upstream) == upstream;
        let refused =
            upstream.is_some_and(|up| self.device(up).is_none() || self.is_within(up, id));
        if unchanged || refused || self.device(id).is_none() {
            return;
        }
        self.connections.retain(|c| c.downstream != id);
        if let Some(upstream) = upstream {
            self.connections.push(Connection {
                upstream,
                downstream: id,
                kind: ConnectionKind::Known,
            });
        }
    }

    /// Undoes [`Self::connect`]: back to the connections as found, less any forgotten devices.
    pub(crate) fn restore_connections(&mut self) {
        self.connections.clone_from(&self.detected);
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
        assert!(
            routers
                .iter()
                .all(|d| d.ip.to_string().contains("192.168.1.1"))
        );
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

    #[test]
    fn forgetting_a_device_drops_it_and_its_links() {
        let mut network = mock_data::network();
        let switch = DeviceId(2);
        network.remove_device(switch);
        assert!(network.device(switch).is_none());
        assert!(
            network
                .connections()
                .iter()
                .all(|c| c.upstream != switch && c.downstream != switch)
        );
    }

    #[test]
    fn a_device_moves_to_another_line_with_its_branch() {
        let mut network = mock_data::network();
        let (router, switch, access_point, pc) =
            (DeviceId(1), DeviceId(2), DeviceId(3), DeviceId(5));
        network.connect(pc, Some(router));
        let link = network.uplink(pc).copied();
        assert_eq!(
            link.map(|l| (l.upstream, l.kind)),
            Some((router, ConnectionKind::Known))
        );
        assert_eq!(
            network
                .connections()
                .iter()
                .filter(|c| c.downstream == pc)
                .count(),
            1
        );

        network.connect(switch, Some(access_point));
        assert!(network.is_within(pc, router));
        assert!(!network.is_within(pc, switch), "the PC left the switch");
        assert!(
            network.is_within(DeviceId(6), access_point),
            "the switch kept its devices"
        );
    }

    #[test]
    fn a_new_link_comes_last_and_an_unchanged_one_stays_as_found() {
        let mut network = mock_data::network();
        let (router, switch, access_point, pc) =
            (DeviceId(1), DeviceId(2), DeviceId(3), DeviceId(5));
        let behind = |n: &Network| n.behind(router).collect::<Vec<_>>();
        assert_eq!(behind(&network), [DeviceId(4), switch, access_point]);
        network.connect(pc, Some(router));
        assert_eq!(behind(&network), [DeviceId(4), switch, access_point, pc]);

        let phone = DeviceId(10);
        let found = network.uplink(phone).copied();
        network.connect(phone, Some(access_point));
        assert_eq!(network.uplink(phone).copied(), found);
        assert_eq!(found.map(|l| l.kind), Some(ConnectionKind::Inferred));
    }

    #[test]
    fn a_device_cannot_join_its_own_branch() {
        let mut network = mock_data::network();
        let (switch, pc) = (DeviceId(2), DeviceId(5));
        let before = network.connections().to_vec();
        network.connect(switch, Some(pc));
        network.connect(switch, Some(switch));
        network.connect(switch, Some(DeviceId(999)));
        assert_eq!(network.connections(), before.as_slice());
    }

    #[test]
    fn a_device_left_alone_heads_its_own_branch_until_restored() {
        let mut network = mock_data::network();
        let before = network.connections().to_vec();
        let switch = DeviceId(2);
        network.connect(switch, None);
        assert!(network.uplink(switch).is_none());
        assert!(network.is_within(DeviceId(5), switch));
        network.restore_connections();
        assert_eq!(network.connections(), before.as_slice());
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
