//! The network as a whole: devices, how they connect, and the roles on offer.

use super::device::{Device, DeviceId, Role, Status};

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
    custom_roles: Vec<String>,
}

impl Network {
    pub(crate) fn new(devices: Vec<Device>, connections: Vec<Connection>) -> Self {
        Self {
            devices,
            connections,
            custom_roles: Vec::new(),
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
            .chain(self.custom_roles.iter().cloned().map(Role::Custom))
            .collect()
    }

    /// Adds a custom role and returns it. An existing role with the same name (ignoring case) is
    /// returned instead of a duplicate; a blank name is rejected.
    pub(crate) fn add_custom_role(&mut self, name: &str) -> Option<Role> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        if let Some(existing) = self
            .roles()
            .into_iter()
            .find(|r| r.to_string().eq_ignore_ascii_case(name))
        {
            return Some(existing);
        }
        self.custom_roles.push(name.to_owned());
        Some(Role::Custom(name.to_owned()))
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
        assert_eq!(edited.display_name(), "Gateway");
        assert_eq!(edited.role, Role::Server);
        assert_eq!(edited.notes, "Replace in spring");
        assert_eq!(network.filtered(StatusFilter::All, "gateway").count(), 1);
    }

    #[test]
    fn custom_roles_are_added_once_and_listed_after_standard_ones() {
        let mut network = Network::default();
        assert_eq!(
            network.add_custom_role("  Media server "),
            Some(Role::Custom("Media server".into()))
        );
        assert_eq!(
            network.add_custom_role("media SERVER"),
            Some(Role::Custom("Media server".into()))
        );
        assert_eq!(network.add_custom_role("nas"), Some(Role::Nas));
        assert_eq!(network.add_custom_role("   "), None);

        let roles = network.roles();
        assert_eq!(roles.len(), Role::STANDARD.len() + 1);
        assert_eq!(roles.last(), Some(&Role::Custom("Media server".into())));
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
