//! Adding and forgetting ordinary devices.

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
        (!cards.is_empty()).then_some(id).into_iter().collect()
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
