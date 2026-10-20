//! Network cards and guests as the UI shows them. A card is a child of its device: a row under
//! it in the lists, and a node on the map drawn with the device's icon and name. A guest is
//! listed in its host's editor.

use slint::SharedString;

use crate::model::{CardId, ConnectionKind, Device, DeviceId, Network, NetworkCard};
use crate::ui::{self, Labels};
use crate::ui_bridge::convert;
use crate::ui_bridge::icons::Icons;

/// Where `card` sits among its device's cards, from 0, for its placeholder name.
fn index_of(device: &Device, card: CardId) -> i32 {
    let at = device.cards.iter().position(|c| c.id == card);
    at.and_then(|at| i32::try_from(at).ok()).unwrap_or_default()
}

/// The row or map node of `card`, made from `device_row`, the row of its device.
pub(super) fn card_row(
    icons: &Icons,
    device_row: &ui::DeviceView,
    device: &Device,
    card: &NetworkCard,
) -> ui::DeviceView {
    ui::DeviceView {
        card_id: convert::ui_card_id(card.id),
        card_index: index_of(device, card.id),
        card_name: card.name.trim().into(),
        drag_card: convert::ui_card_id(card.id),
        card_icon: icons.choice(&card.icon),
        ip: convert::ip(card),
        mac: convert::mac(card),
        depth: device_row.depth + 1,
        children: 0,
        last: device.cards.last().is_some_and(|c| c.id == card.id),
        ..device_row.clone()
    }
}

/// The name of the card `socket` in the "plugged into" dropdown: its device's name, with the
/// card's own when the device has several.
pub(super) fn socket_name(labels: &Labels<'_>, network: &Network, socket: CardId) -> SharedString {
    let Some(device) = network.card_owner(socket) else {
        return SharedString::new();
    };
    let card = device.cards.iter().find(|c| c.id == socket);
    let several = device.cards.len() > 1;
    let index = if several {
        index_of(device, socket)
    } else {
        -1
    };
    let name = card.map(|c| c.name.trim()).unwrap_or_default();
    labels.invoke_card_of(device.name.trim().into(), name.into(), index)
}

/// The network cards of `device`, each with where it is plugged in.
pub(super) fn card_views(network: &Network, icons: &Icons, device: &Device) -> Vec<ui::CardView> {
    let views = device.cards.iter().map(|card| {
        let link = network.link(card.id);
        let via = link.and_then(|l| Some((l.upstream, network.card_owner(l.upstream)?)));
        let via_card = via.and_then(|(id, d)| d.cards.iter().find(|c| c.id == id));
        let several = via.is_some_and(|(_, d)| d.cards.len() > 1);
        ui::CardView {
            id: convert::ui_card_id(card.id),
            index: index_of(device, card.id),
            name: card.name.trim().into(),
            ip: convert::ip(card),
            mask: card
                .address
                .map(|n| n.netmask().to_string())
                .unwrap_or_default()
                .into(),
            gateway: card
                .gateway
                .map(|g| g.to_string())
                .unwrap_or_default()
                .into(),
            mac: convert::mac(card),
            via_name: via.map(|(_, d)| d.name.trim()).unwrap_or_default().into(),
            via_card: via_card.map(|c| c.name.trim()).unwrap_or_default().into(),
            via_index: match via {
                Some((id, d)) if several => index_of(d, id),
                Some(_) | None => -1,
            },
            inferred: link.is_some_and(|l| l.kind == ConnectionKind::Inferred),
            services: card.services.to_string().into(),
            icon: icons.choice(&card.icon),
        }
    });
    views.collect()
}

/// The virtual machines and containers running straight on `host`, in device order; those on
/// them are listed with their own host.
pub(super) fn guest_views(network: &Network, icons: &Icons, host: DeviceId) -> Vec<ui::GuestView> {
    let guests = network.devices().iter().filter_map(|device| {
        let guest = device.guest.filter(|g| g.host == host)?;
        Some(ui::GuestView {
            id: convert::ui_id(device.id),
            kind: convert::guest_kind(Some(guest.kind)),
            name: device.name.trim().into(),
            ip: device.primary_card().map(convert::ip).unwrap_or_default(),
            icon: icons.choice(&network.icon(device)),
            colour: convert::colour(network.colour(device)),
        })
    });
    guests.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    const PC: DeviceId = DeviceId(5);
    const DOCKER_HOST: DeviceId = DeviceId(19);

    #[test]
    fn a_hosts_guests_are_those_running_straight_on_it() {
        let network = mock_data::network();
        let icons = Icons::default();
        let ids = |host| {
            guest_views(&network, &icons, host)
                .into_iter()
                .map(|g| g.id)
        };
        let on_docker: Vec<i32> = network
            .devices()
            .iter()
            .filter(|d| network.host(d.id) == Some(DOCKER_HOST))
            .map(|d| convert::ui_id(d.id))
            .collect();
        assert!(!on_docker.is_empty());
        assert_eq!(ids(DOCKER_HOST).collect::<Vec<_>>(), on_docker);
        let on_linux_server: Vec<i32> = ids(network.host(DOCKER_HOST).unwrap()).collect();
        assert!(on_linux_server.contains(&convert::ui_id(DOCKER_HOST)));
        assert!(
            on_docker.iter().all(|id| !on_linux_server.contains(id)),
            "not the guests of its guests"
        );
        assert_eq!(ids(PC).count(), 0);
    }
}
