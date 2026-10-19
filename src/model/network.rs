//! The network as a whole: devices, how they connect, and the roles on offer.

use super::device::{Device, DeviceId, DeviceKind, Role, Status};
use super::roles::CustomRoles;

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
    custom_roles: CustomRoles,
}

impl Network {
    pub(crate) fn new(devices: Vec<Device>, connections: Vec<Connection>) -> Self {
        Self {
            devices,
            connections,
            custom_roles: CustomRoles::default(),
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
        self.connections
            .retain(|c| c.upstream != id && c.downstream != id);
    }

    /// The connection leading from `id` towards the router, if any.
    pub(crate) fn uplink(&self, id: DeviceId) -> Option<&Connection> {
        self.connections.iter().find(|c| c.downstream == id)
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
        Role::STANDARD
            .into_iter()
            .chain(self.custom_roles.roles())
            .collect()
    }

    /// The icon a role gives its devices; `None` when they keep their detected icon.
    pub(crate) fn role_icon(&self, role: &Role) -> Option<DeviceKind> {
        if let Role::Custom(name) = role {
            self.custom_roles.icon(name)
        } else {
            role.icon()
        }
    }

    /// The icon shown for a device: the one picked for it, else its role's, else the detected one.
    pub(crate) fn icon(&self, device: &Device) -> DeviceKind {
        device
            .icon
            .or_else(|| self.role_icon(&device.role))
            .unwrap_or(device.kind)
    }

    /// Adds a custom role shown with `icon`, and returns it (see [`CustomRoles::add`]).
    pub(crate) fn add_custom_role(&mut self, name: &str, icon: DeviceKind) -> Option<Role> {
        self.custom_roles.add(name, icon)
    }

    /// Renames a custom role on every device that has it. Blank or taken names are ignored.
    pub(crate) fn rename_custom_role(&mut self, old: &str, new: &str) {
        let Some(new) = self.custom_roles.rename(old, new) else {
            return;
        };
        let old = Role::Custom(old.to_owned());
        for device in self.devices.iter_mut().filter(|d| d.role == old) {
            device.role = Role::Custom(new.clone());
        }
    }

    pub(crate) fn set_custom_role_icon(&mut self, name: &str, icon: DeviceKind) {
        self.custom_roles.set_icon(name, icon);
    }

    /// Deletes a custom role; its devices go back to `Other` and their own icon.
    pub(crate) fn remove_custom_role(&mut self, name: &str) {
        if self.custom_roles.remove(name) {
            let old = Role::Custom(name.to_owned());
            for device in self.devices.iter_mut().filter(|d| d.role == old) {
                device.role = Role::Other;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

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
        network.add_custom_role("Media server", DeviceKind::Server);
        let roles = network.roles();
        assert_eq!(roles.len(), Role::STANDARD.len() + 1);
        assert_eq!(roles.last(), Some(&Role::Custom("Media server".into())));
    }

    #[test]
    fn the_icon_follows_the_role_unless_one_was_picked() {
        let mut network = mock_data::network();
        let media = network.add_custom_role("Media", DeviceKind::Tv).unwrap();
        let mut device = network.devices().first().unwrap().clone();
        for (role, expected) in [
            (Role::Printer, DeviceKind::Printer),
            (Role::Iot, DeviceKind::Smart),
            (media, DeviceKind::Tv),
            (Role::Other, device.kind),
        ] {
            device.role = role;
            assert_eq!(network.icon(&device), expected, "{:?}", device.role);
        }
        device.icon = Some(DeviceKind::Camera);
        assert_eq!(network.icon(&device), DeviceKind::Camera);
    }

    fn roles_of(network: &Network, role: &Role) -> usize {
        network.devices().iter().filter(|d| d.role == *role).count()
    }

    #[test]
    fn renaming_a_custom_role_renames_it_on_its_devices() {
        let mut network = mock_data::network();
        let tv = Role::Custom("TV".into());
        let users = roles_of(&network, &tv);
        assert!(users > 0);
        network.rename_custom_role("TV", "Television");
        assert_eq!(roles_of(&network, &tv), 0);
        assert_eq!(
            roles_of(&network, &Role::Custom("Television".into())),
            users
        );
        network.rename_custom_role("Television", "games CONSOLE");
        assert_eq!(
            roles_of(&network, &Role::Custom("Television".into())),
            users,
            "taken"
        );
    }

    #[test]
    fn deleting_a_custom_role_gives_its_devices_other() {
        let mut network = mock_data::network();
        let tv = Role::Custom("TV".into());
        let others = roles_of(&network, &Role::Other);
        let users = roles_of(&network, &tv);
        network.remove_custom_role("TV");
        assert!(!network.roles().contains(&tv));
        assert_eq!(roles_of(&network, &Role::Other), others + users);
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
    fn uplink_points_towards_the_router() {
        let network = mock_data::network();
        let router = network
            .devices()
            .iter()
            .find(|d| network.uplink(d.id).is_none());
        assert!(router.is_some_and(|r| r.role == Role::Router));
    }
}
