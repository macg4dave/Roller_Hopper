//! Adding and forgetting ordinary devices, and the device each runs on.

use super::{ConnectionKind, Network};
use crate::model::card::{CardId, NetworkCard};
use crate::model::device::{Device, DeviceId, DeviceKind, LastSeen, Role, Status};
use crate::model::fresh_ids;

impl Network {
    /// Adds a blank device with one blank network card and returns it. It is added last and
    /// plugged in nowhere, ready for the user to fill in.
    pub(crate) fn add_device(&mut self) -> Option<DeviceId> {
        let device = Device {
            id: DeviceId(0),
            name: String::new(),
            kind: DeviceKind::Unknown,
            role: Role::Other,
            icon: None,
            status: Status::Unknown,
            host: None,
            cards: vec![NetworkCard::blank(CardId(0))],
            manufacturer: None,
            model: None,
            hostname: None,
            os: None,
            mdns_services: Vec::new(),
            ssdp_services: Vec::new(),
            ssdp_server: None,
            probe_log: Vec::new(),
            last_seen: LastSeen::default(),
            notes: String::new(),
        };
        self.insert_device(device, None)
    }

    /// Adds `device` under fresh identifiers and optionally plugs its first card into `uplink`.
    pub(crate) fn insert_device(
        &mut self,
        mut device: Device,
        uplink: Option<(CardId, ConnectionKind)>,
    ) -> Option<DeviceId> {
        let device_id = fresh_ids(self.devices.iter().map(|existing| existing.id.0), 1)?.pop()?;
        let card_ids = fresh_ids(
            self.devices
                .iter()
                .flat_map(|existing| &existing.cards)
                .map(|card| card.id.0),
            device.cards.len(),
        )?;
        device.id = DeviceId(device_id);
        for (card, id) in device.cards.iter_mut().zip(card_ids) {
            card.id = CardId(id);
        }
        let (id, card) = (device.id, device.primary_card().map(|c| c.id));
        self.devices.push(device);
        if let Some(card) = card {
            self.plug_found(card, uplink);
        }
        Some(id)
    }

    pub(super) fn plug_found(&mut self, card: CardId, uplink: Option<(CardId, ConnectionKind)>) {
        if let Some((upstream, kind)) = uplink
            && self.plug_card(card, Some(upstream))
        {
            self.links
                .iter_mut()
                .filter(|link| link.card == card)
                .for_each(|link| link.kind = kind);
        }
    }

    /// Forgets one device and its links. Downstream cards remain as separate map branches.
    pub(crate) fn remove_device(&mut self, id: DeviceId) -> Vec<DeviceId> {
        let cards = self
            .device(id)
            .map(|device| device.cards.iter().map(|card| card.id).collect::<Vec<_>>())
            .unwrap_or_default();
        self.devices.retain(|device| device.id != id);
        self.links
            .retain(|link| !cards.contains(&link.upstream) && !cards.contains(&link.card));
        self.relations
            .retain(|link| !cards.contains(&link.upstream) && !cards.contains(&link.card));
        self.memberships
            .retain(|membership| !cards.contains(&membership.card));
        for guest in self.devices.iter_mut().filter(|d| d.host == Some(id)) {
            guest.host = None;
        }
        (!cards.is_empty()).then_some(id).into_iter().collect()
    }

    /// Whether `device` may run on `host`: both exist, and `host` is not `device` itself or
    /// something that runs on it, which would form a ring.
    pub(crate) fn can_host(&self, device: DeviceId, host: DeviceId) -> bool {
        let host_of = |at: &DeviceId| self.device(*at)?.host;
        let mut chain = std::iter::successors(Some(host), host_of).take(self.devices.len() + 1);
        self.device(device).is_some()
            && self.device(host).is_some()
            && !chain.any(|at| at == device)
    }

    /// Records the device `device` runs on, or none. Refused (false) where [`Self::can_host`]
    /// does not allow it.
    pub(crate) fn set_host(&mut self, device: DeviceId, host: Option<DeviceId>) -> bool {
        if host.is_some_and(|host| !self.can_host(device, host)) {
            return false;
        }
        let Some(guest) = self.device_mut(device) else {
            return false;
        };
        guest.host = host;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    #[test]
    fn forgetting_a_device_drops_only_it_and_its_links() {
        let mut network = mock_data::network();
        let id = DeviceId(19);
        assert_eq!(network.remove_device(id), [id]);
        assert!(network.device(id).is_none());
        assert!(network.device(DeviceId(24)).is_some());
        assert!(network.uplink(CardId(24)).is_none());
    }

    #[test]
    fn a_device_runs_on_a_host_without_a_ring() {
        let mut network = mock_data::network();
        let (server, vm, other) = (DeviceId(6), DeviceId(19), DeviceId(5));
        assert!(network.set_host(vm, Some(server)));
        assert_eq!(network.device(vm).and_then(|d| d.host), Some(server));
        assert!(!network.set_host(server, Some(vm)), "a ring");
        assert!(!network.set_host(vm, Some(vm)), "itself");
        assert!(!network.set_host(vm, Some(DeviceId(u32::MAX))), "missing");
        assert!(network.set_host(other, Some(vm)), "a chain is fine");
        network.remove_device(server);
        assert_eq!(network.device(vm).and_then(|d| d.host), None);
        assert!(network.set_host(other, None));
    }

    #[test]
    fn a_device_is_added_blank_last_and_plugged_in_nowhere() {
        let mut network = mock_data::network();
        let id = network.add_device().unwrap();
        let added = network.devices().last().unwrap();
        assert_eq!(added.id, id);
        assert_eq!(added.cards, [NetworkCard::blank(added.cards[0].id)]);
        assert!(network.uplink(added.cards[0].id).is_none());
    }

    #[test]
    fn adding_falls_back_to_free_ids_below_the_ui_limit() {
        let mut network = mock_data::network();
        network.devices[0].id = DeviceId(crate::model::MAX_MODEL_ID);
        network.devices[1].id = DeviceId(2);
        network.devices[0].cards[0].id = CardId(crate::model::MAX_MODEL_ID);
        network.devices[1].cards[0].id = CardId(2);
        let id = network.add_device().unwrap();
        let card = network.device(id).unwrap().cards[0].id;
        assert!(id.0 <= crate::model::MAX_MODEL_ID);
        assert!(card.0 <= crate::model::MAX_MODEL_ID);
        assert_ne!(id, DeviceId(crate::model::MAX_MODEL_ID));
        assert_ne!(card, CardId(crate::model::MAX_MODEL_ID));
    }
}
