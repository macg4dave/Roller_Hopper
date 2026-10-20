//! Network cards in the UI. A card is a child of its device: a row under it in the lists, and a
//! node on the map drawn with the device's icon and name. This module builds their views and
//! handles editing the selected card (its IPv4 settings, where it is plugged in, removing it),
//! adding cards, listing the virtual machines and containers on the selected device, and
//! adding devices and guests.

use std::cell::RefCell;
use std::net::Ipv4Addr;
use std::rc::Rc;

use ipnet::Ipv4Net;
use slint::{ComponentHandle, SharedString};

use super::icons::Icons;
use super::{AppState, Selection, convert, handler, update};
use crate::model::{
    CardId, ConnectionKind, Device, DeviceId, Guest, GuestKind, Icon, MacAddr, Network, NetworkCard,
};
use crate::ui::{self, AppWindow, Bridge, Labels};

/// The card's address without its prefix, as shown; empty when it has none.
pub(super) fn ip(card: &NetworkCard) -> SharedString {
    let ip = card.address.map(|net| net.addr().to_string());
    ip.unwrap_or_default().into()
}

/// The MAC address as shown; empty when there is none.
pub(super) fn mac(card: &NetworkCard) -> SharedString {
    let mac = card.mac.map(|mac| mac.to_string());
    mac.unwrap_or_default().into()
}

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
        ip: ip(card),
        mac: mac(card),
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
            ip: ip(card),
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
            mac: mac(card),
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
            ip: device.primary_card().map(ip).unwrap_or_default(),
            icon: icons.choice(&network.icon(device)),
            colour: convert::colour(network.colour(device)),
        })
    });
    guests.collect()
}

/// A subnet prefix as typed: a mask such as `255.255.255.0`, or a length such as `24` or `/24`.
fn prefix(text: &str) -> Option<u8> {
    let text = text.trim();
    let length = text.strip_prefix('/').unwrap_or(text).parse::<u8>();
    match length {
        Ok(length) => Some(length).filter(|&l| l <= 32),
        Err(_) => ipnet::ipv4_mask_to_prefix(text.parse().ok()?).ok(),
    }
}

/// An optional value as typed: empty is none; anything else must parse.
fn optional<T: std::str::FromStr>(text: &str) -> Result<Option<T>, T::Err> {
    let text = text.trim();
    if text.is_empty() {
        Ok(None)
    } else {
        text.parse().map(Some)
    }
}

impl AppState {
    /// The cards the selected card can be plugged into, in dropdown order after "none": the
    /// cards of the other devices, in device order, apart from those behind it, which would
    /// form a loop.
    pub(super) fn upstream_choices(&self) -> Vec<CardId> {
        let Some(selected) = self.selected_card() else {
            return Vec::new();
        };
        let cards = self.network.nodes().into_iter();
        cards
            .filter(|&card| self.network.can_plug(selected, card))
            .collect()
    }

    /// Where the selected card is plugged in, as its index in the dropdown: 0 for none.
    pub(super) fn plug_choice(&self) -> usize {
        let link = self.selected_card().and_then(|c| self.network.link(c));
        let at = link.and_then(|l| {
            self.upstream_choices()
                .iter()
                .position(|&c| c == l.upstream)
        });
        at.map_or(0, |at| at + 1)
    }

    /// Adds a blank card to the selected device and selects it, so it can be filled in.
    fn add_card(&mut self) {
        let Some(device) = self.selected_device() else {
            return;
        };
        if let Some(card) = self.network.add_card(device) {
            self.selected = Some(Selection::Card(card));
            self.lay_out_at_rest();
        }
    }

    /// Gives `card` its own icon, from the card editor.
    fn set_card_icon(&mut self, card: CardId, icon: Icon) {
        if let Some(card) = self.network.card_mut(card) {
            card.icon = icon;
        }
    }

    /// Removes the selected card, unless it is its device's last, and selects its device.
    fn remove_card(&mut self) {
        let (Some(card), Some(device)) = (self.selected_card(), self.selected_device()) else {
            return;
        };
        if self.network.remove_card(card) {
            self.selected = Some(Selection::Device(device));
            self.lay_out_at_rest();
        }
    }

    /// Sets one field of the selected card as typed. False if refused, so the UI can keep what
    /// was typed and say so. An address typed without a prefix keeps the card's subnet, and a
    /// subnet mask needs an address.
    fn edit_card(&mut self, field: ui::CardField, text: &str) -> bool {
        let Some(card) = self.selected_card() else {
            return false;
        };
        let current = self.network.card(card).and_then(|c| c.address);
        match field {
            ui::CardField::Name => self
                .network
                .card_mut(card)
                .map(|c| text.clone_into(&mut c.name))
                .is_some(),
            ui::CardField::Address => {
                let prefix = current.map_or(convert::TYPED_PREFIX, |n| n.prefix_len());
                let address = match optional::<Ipv4Addr>(text) {
                    Ok(ip) => ip.and_then(|ip| Ipv4Net::new(ip, prefix).ok()),
                    Err(_) => match convert::address(text) {
                        Some(net) => Some(net),
                        None => return false,
                    },
                };
                self.network.set_card_address(card, address)
            }
            ui::CardField::Mask => {
                let net = current
                    .zip(prefix(text))
                    .and_then(|(n, p)| Ipv4Net::new(n.addr(), p).ok());
                net.is_some_and(|net| self.network.set_card_address(card, Some(net)))
            }
            ui::CardField::Gateway => optional::<Ipv4Addr>(text)
                .ok()
                .and_then(|gateway| self.network.card_mut(card).map(|c| c.gateway = gateway))
                .is_some(),
            ui::CardField::Mac => optional::<MacAddr>(text)
                .ok()
                .and_then(|mac| self.network.card_mut(card).map(|c| c.mac = mac))
                .is_some(),
        }
    }

    /// Plugs the selected card into the card at `choice` in the dropdown, where 0 is none.
    fn plug_card(&mut self, choice: usize) {
        let upstream = match choice.checked_sub(1) {
            None => None,
            Some(at) => match self.upstream_choices().get(at) {
                Some(&card) => Some(card),
                None => return,
            },
        };
        let Some(card) = self.selected_card() else {
            return;
        };
        if self.network.plug_card(card, upstream) {
            self.lay_out_at_rest();
        }
    }

    /// Adds a blank device and selects it, so it can be filled in, as a new card is: a guest of
    /// `kind` on the selected device, or with no kind a device of its own.
    fn add_device(&mut self, kind: Option<GuestKind>) {
        let guest = match kind {
            None => None,
            Some(kind) => match self.selected_device() {
                Some(host) => Some(Guest { host, kind }),
                None => return,
            },
        };
        if let Some(id) = self.network.add_device(guest) {
            self.selected = Some(Selection::Device(id));
            self.lay_out_at_rest();
        }
    }

    /// Changes what the selected guest runs as. Its icon follows unless it or its role has one
    /// (see [`Network::icon`]).
    fn set_guest_kind(&mut self, kind: GuestKind) {
        self.edit_selected(|device| {
            if let Some(guest) = &mut device.guest {
                guest.kind = kind;
            }
        });
    }
}

/// Adding, editing, plugging in and removing network cards, and adding devices and guests of
/// the selected device. A guest is then a device of its own: edited, moved among its host's
/// guests and forgotten like any other.
pub(super) fn connect_child_edits(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let add_card = handler(ui, state, |st, ()| st.add_card());
    bridge.on_add_card(move || add_card(()));
    let remove_card = handler(ui, state, |st, ()| st.remove_card());
    bridge.on_remove_card(move || remove_card(()));
    let set_icon = handler(ui, state, |st, (id, icon): (i32, SharedString)| {
        if let Some(card) = convert::card_id(id) {
            st.set_card_icon(card, convert::icon_from_ui(&icon));
        }
    });
    bridge.on_set_card_icon(move |id, icon| set_icon((id, icon)));
    let (edit_ui, edit_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_edit_card(move |field, text| {
        let mut accepted = false;
        update(&edit_ui, &edit_state, |st| {
            accepted = st.edit_card(field, &text);
        });
        accepted
    });
    bridge.on_plug_card(handler(ui, state, |st, choice| {
        st.plug_card(convert::index(choice));
    }));
    bridge.on_add_device(handler(ui, state, |st, kind| {
        st.add_device(convert::guest_kind_from_ui(kind));
    }));
    bridge.on_set_guest_kind(handler(ui, state, |st, kind| {
        if let Some(kind) = convert::guest_kind_from_ui(kind) {
            st.set_guest_kind(kind);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    const PC: DeviceId = DeviceId(5);
    const DOCKER_HOST: DeviceId = DeviceId(19);
    const LAB_ROUTER: DeviceId = DeviceId(22);

    /// A state with a blank card just added to the PC, and selected.
    fn with_new_card() -> (AppState, CardId) {
        let mut state = AppState::new(mock_data::network());
        state.selected = Some(Selection::Device(PC));
        state.add_card();
        let card = state.selected_card().unwrap();
        (state, card)
    }

    #[test]
    fn adding_a_card_selects_it_and_puts_it_on_the_map() {
        let (state, card) = with_new_card();
        assert_eq!(state.selected_device(), Some(PC));
        assert_eq!(state.network.card(card), Some(&NetworkCard::blank(card)));
        assert!(state.positions.contains_key(&card), "on the map");
        assert!(
            state.network.uplink(card).is_none(),
            "plugged in nowhere yet"
        );
        assert_eq!(state.motion.shown(), &state.positions, "laid out at rest");
    }

    #[test]
    fn a_typed_address_and_mask_are_kept_only_when_valid() {
        let (mut state, card) = with_new_card();
        let edit = |st: &mut AppState, field, text: &str| st.edit_card(field, text);
        assert!(
            !edit(&mut state, ui::CardField::Mask, "255.255.0.0"),
            "no address yet"
        );
        assert!(!edit(&mut state, ui::CardField::Address, "10.0.0"));
        assert!(
            !edit(&mut state, ui::CardField::Address, "192.168.1.1"),
            "the router's"
        );
        assert!(edit(&mut state, ui::CardField::Address, " 10.0.0.7 "));
        assert!(edit(&mut state, ui::CardField::Mask, "255.255.0.0"));
        assert!(
            edit(&mut state, ui::CardField::Address, "10.0.0.8"),
            "keeps the subnet"
        );
        assert!(!edit(&mut state, ui::CardField::Mask, "255.0.255.0"));
        let address = state.network.card(card).and_then(|c| c.address);
        assert_eq!(
            address.map(|a| a.to_string()).as_deref(),
            Some("10.0.0.8/16")
        );
        assert!(edit(&mut state, ui::CardField::Address, ""), "cleared");
        assert_eq!(state.network.card(card).and_then(|c| c.address), None);
    }

    #[test]
    fn a_typed_gateway_mac_and_name_are_kept_only_when_valid() {
        let (mut state, card) = with_new_card();
        let edit = |st: &mut AppState, field, text: &str| st.edit_card(field, text);
        assert!(edit(&mut state, ui::CardField::Gateway, "10.0.0.1"));
        assert!(!edit(&mut state, ui::CardField::Gateway, "router"));
        assert!(edit(&mut state, ui::CardField::Mac, "aa-bb-cc-dd-ee-ff"));
        assert!(!edit(&mut state, ui::CardField::Mac, "aa-bb"));
        assert!(edit(&mut state, ui::CardField::Name, "Lab"));
        let edited = state.network.card(card).unwrap();
        assert_eq!(edited.gateway, Some(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(
            edited.mac.map(|m| m.to_string()).as_deref(),
            Some("AA:BB:CC:DD:EE:FF")
        );
        assert_eq!(edited.name, "Lab");
        assert!(edit(&mut state, ui::CardField::Gateway, " "), "cleared");
        assert_eq!(state.network.card(card).and_then(|c| c.gateway), None);
    }

    #[test]
    fn a_mask_is_read_as_dotted_or_as_a_length() {
        assert_eq!(prefix("255.255.255.0"), Some(24));
        assert_eq!(prefix(" /16 "), Some(16));
        assert_eq!(prefix("8"), Some(8));
        for refused in ["33", "255.0.255.0", "", "mask"] {
            assert_eq!(prefix(refused), None, "{refused:?}");
        }
    }

    #[test]
    fn a_card_is_plugged_into_a_card_chosen_away_from_its_own_device() {
        let (mut state, card) = with_new_card();
        let choices = state.upstream_choices();
        let own: Vec<_> = state
            .network
            .device(PC)
            .unwrap()
            .cards
            .iter()
            .map(|c| c.id)
            .collect();
        assert!(choices.iter().all(|c| !own.contains(c)));
        let lab = state.network.device(LAB_ROUTER).unwrap().cards[0].id;
        let at = choices.iter().position(|&c| c == lab).unwrap();
        state.plug_card(at + 1);
        assert_eq!(state.plug_choice(), at + 1);
        let uplink = state.network.uplink(card).map(|c| c.upstream);
        assert_eq!(uplink, Some(CardId(LAB_ROUTER.0)));
        state.plug_card(999);
        assert_eq!(state.plug_choice(), at + 1, "out of range");
        state.plug_card(0);
        assert_eq!(state.plug_choice(), 0);
    }

    #[test]
    fn a_card_starts_as_ethernet_and_takes_any_icon_without_being_selected() {
        let (mut state, card) = with_new_card();
        let icon_of = |st: &AppState, c| st.network.card(c).map(|c| c.icon.0.clone());
        assert_eq!(
            icon_of(&state, card).as_deref(),
            Some(NetworkCard::DEFAULT_ICON)
        );
        let other = CardId(2);
        state.set_card_icon(other, Icon::new("Network cards/5G"));
        assert_eq!(icon_of(&state, other).as_deref(), Some("Network cards/5G"));
        assert_eq!(state.selected_card(), Some(card), "the selection stays");
        state.set_card_icon(CardId(999), Icon::new("Network/Router"));
    }

    #[test]
    fn removing_a_card_selects_its_device_but_the_last_stays() {
        let (mut state, card) = with_new_card();
        state.remove_card();
        assert_eq!(state.selected, Some(Selection::Device(PC)));
        assert!(state.network.card(card).is_none());
        let only = state.network.device(PC).unwrap().cards[0].id;
        state.selected = Some(Selection::Card(only));
        state.remove_card();
        assert!(state.network.card(only).is_some(), "the last stays");
    }

    #[test]
    fn a_virtual_machine_is_added_to_the_selection_selected_and_placed_under_it() {
        let mut state = AppState::new(mock_data::network());
        let before = state.network.clone();
        state.add_device(Some(GuestKind::VirtualMachine));
        assert_eq!(state.network, before, "nothing selected to run on");

        state.selected = Some(Selection::Card(CardId(DOCKER_HOST.0)));
        state.add_device(Some(GuestKind::VirtualMachine));
        let vm = state.selected_device().unwrap();
        assert_eq!(state.selected, Some(Selection::Device(vm)), "selected");
        assert_eq!(state.network.host(vm), Some(DOCKER_HOST));
        let vm = state.network.device(vm).unwrap().cards[0].id;
        let host = CardId(DOCKER_HOST.0);
        assert!(state.positions[&vm].y > state.positions[&host].y);
        assert_eq!(state.motion.shown(), &state.positions, "laid out at rest");
    }

    #[test]
    fn a_device_of_its_own_is_added_and_selected_with_or_without_a_selection() {
        let mut state = AppState::new(mock_data::network());
        state.add_device(None);
        let added = state.selected_device().unwrap();
        assert_eq!(state.network.host(added), None);
        let card = state.network.device(added).unwrap().cards[0].id;
        assert!(state.positions.contains_key(&card), "on the map");
    }

    #[test]
    fn a_guest_runs_as_another_kind_and_its_icon_follows_unless_picked() {
        let mut state = AppState::new(mock_data::network());
        state.selected = Some(Selection::Device(DOCKER_HOST));
        state.add_device(Some(GuestKind::VirtualMachine));
        let guest = state.selected_device().unwrap();
        let of = |st: &AppState| {
            let d = st.network.device(guest).unwrap();
            (d.guest.map(|g| g.kind), st.network.icon(d))
        };
        let vm = Some(GuestKind::VirtualMachine);
        assert_eq!(of(&state), (vm, GuestKind::VirtualMachine.icon()));
        state.set_guest_kind(GuestKind::Container);
        let container = Some(GuestKind::Container);
        assert_eq!(of(&state), (container, GuestKind::Container.icon()));

        let fish = Icon::new("My icons/Fish tank");
        state.edit_selected(|d| d.icon = Some(fish.clone()));
        state.set_guest_kind(GuestKind::VirtualMachine);
        assert_eq!(of(&state), (vm, fish), "picked icon kept");

        state.selected = Some(Selection::Device(PC));
        state.set_guest_kind(GuestKind::Container);
        assert_eq!(state.network.device(PC).unwrap().guest, None, "not a guest");
    }

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
