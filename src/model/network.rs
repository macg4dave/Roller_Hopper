//! The network as a whole: devices, how their network cards are plugged into each other, the
//! roles on offer and their groups.

mod auto_map;
mod cards;
mod check;
mod children;
mod tree;

pub(crate) use auto_map::{AutoMapConfidence, AutoMapProposal, AutoMapReason};
pub(crate) use tree::Connection;

use serde::{Deserialize, Serialize};

use super::card::CardId;
use super::device::{Device, DeviceId, Icon, Role};
use super::groups::{GroupColour, GroupId, Groups, LineStyle, StandardGroup};
use super::roles::Roles;

/// Whether a link was observed directly or guessed from indirect evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum ConnectionKind {
    Known,
    Inferred,
}

/// A network card plugged into a card of another device, towards the router (`upstream`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct CardLink {
    pub(crate) upstream: CardId,
    pub(crate) card: CardId,
    pub(crate) kind: ConnectionKind,
}

/// Read from a file, it is only used once [`Self::checked`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Network {
    devices: Vec<Device>,
    links: Vec<CardLink>,
    roles: Roles,
    groups: Groups,
}

impl Default for Network {
    /// A new project: no devices, and the standard roles in the standard groups.
    fn default() -> Self {
        let mut network = Self {
            devices: Vec::new(),
            links: Vec::new(),
            roles: Roles::default(),
            groups: Groups::default(),
        };
        network.restore_standard_groups();
        network
    }
}

impl Network {
    pub(crate) fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub(crate) fn device(&self, id: DeviceId) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    pub(crate) fn device_mut(&mut self, id: DeviceId) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }

    pub(crate) fn filtered<'a>(&'a self, query: &'a str) -> impl Iterator<Item = &'a Device> {
        self.devices
            .iter()
            .filter(move |device| device.matches(query))
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
        let picked = device
            .icon
            .as_ref()
            .or_else(|| self.role_icon(&device.role));
        picked.cloned().unwrap_or_else(|| device.kind.icon())
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

    /// Whether [`Self::restore_standard_groups`] would add a group.
    pub(crate) fn standard_groups_missing(&self) -> bool {
        self.groups.standard_missing()
    }

    /// Adds back the standard groups that were deleted, as a new project has them, and puts each
    /// standard role that is in no group into its standard group. Groups and roles the user
    /// changed stay as they are.
    pub(crate) fn restore_standard_groups(&mut self) {
        self.groups.restore_standard();
        let groups = &self.groups;
        self.roles
            .fill_groups(|role| StandardGroup::of(role).and_then(|s| groups.standard(s)));
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
    fn search_filters_devices() {
        let network = mock_data::network();
        let routers: Vec<_> = network.filtered("192.168.1.1").collect();
        assert_ne!(routers, [] as [&Device; 0]);
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
        assert_eq!(network.filtered("gateway").count(), 1);
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
}
