//! Editing the selected device's network cards: adding a blank one, filling in its IPv4
//! settings, plugging it into a card of another device, giving it an icon, and removing it.

use std::cell::RefCell;
use std::net::Ipv4Addr;
use std::rc::Rc;

use ipnet::Ipv4Net;
use slint::{ComponentHandle, SharedString};

use crate::model::{CardId, Icon, MacAddr};
use crate::ui::{self, AppWindow, Bridge};
use crate::ui_bridge::{AppState, Selection, convert, handler, update};

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
    pub(in crate::ui_bridge) fn upstream_choices(&self) -> Vec<CardId> {
        let Some(selected) = self.selected_card() else {
            return Vec::new();
        };
        let cards = self.network.nodes().into_iter();
        cards
            .filter(|&card| self.network.can_plug(selected, card))
            .collect()
    }

    /// Where the selected card is plugged in, as its index in the dropdown: 0 for none.
    pub(in crate::ui_bridge) fn plug_choice(&self) -> usize {
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
}

/// Adding, editing, plugging in, re-iconing and removing network cards.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
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
    bridge.on_set_card_dhcp(handler(ui, state, |st, dhcp| {
        st.edit_selected_card(|card| card.dhcp = dhcp);
    }));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::{DeviceId, NetworkCard};

    const PC: DeviceId = DeviceId(5);
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
}
