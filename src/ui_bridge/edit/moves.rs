//! Making a network card a child of another parent: a card on the map of another device, from
//! the card editor's "Child of" dropdown or by dropping its row on that device's group on the
//! edit page; a card of a found host not on the map of another found host or of a device on the
//! map, likewise from the discovery page. A move that would leave the parent with no card
//! removes that parent, so it waits for the user to confirm.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, DataTransfer, SharedString};

use crate::model::{CardId, DeviceId, Found, Parent};
use crate::ui::{AppWindow, Bridge};
use crate::ui_bridge::{AppState, Selection, convert, handler, map};

/// What a card of a found host carries when dragged, before the host's and the card's places.
const FOUND_PAYLOAD: &str = "network-mapper-found:";

/// A network card becoming a child of another parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui_bridge) enum CardMove {
    /// A card on the map, to another device.
    Map { card: CardId, to: DeviceId },
    /// The card at `card` among those of the found host at `host`, which is not on the map.
    Found {
        host: usize,
        card: usize,
        to: Parent,
    },
}

impl AppState {
    fn can_move(&self, request: CardMove) -> bool {
        match request {
            CardMove::Map { card, to } => self.network.can_move_card(card, to),
            CardMove::Found { host, card, to } => {
                self.discovery.can_move_card(host, card, to, &self.network)
            }
        }
    }

    /// Moves a card, or holds the move for the user to confirm when it would remove the parent
    /// the card leaves.
    fn request_move(&mut self, request: CardMove) {
        if !self.can_move(request) {
            return;
        }
        let empties = match request {
            CardMove::Map { card, .. } => self.network.only_card_of(card).is_some(),
            CardMove::Found { host, .. } => self.discovery.only_card(host),
        };
        if empties {
            self.pending_move = Some(request);
        } else {
            self.apply_move(request);
        }
    }

    fn confirm_move(&mut self) {
        if let Some(request) = self.pending_move.take() {
            self.apply_move(request);
        }
    }

    /// Moves the card and shows it where it went: a card on the map stays selected, and the
    /// discovery details show the parent a found card went to, when it is in the results.
    fn apply_move(&mut self, request: CardMove) {
        match request {
            CardMove::Map { card, to } => {
                if self.network.move_card(card, to) {
                    self.selected = Some(Selection::Card(card));
                    self.lay_out_at_rest();
                }
            }
            CardMove::Found { host, card, to } => {
                let emptied = self.discovery.only_card(host);
                if !self.discovery.move_card(host, card, to, &mut self.network) {
                    return;
                }
                let shown = match to {
                    Parent::Found(at) if emptied && at > host => Some(at - 1),
                    Parent::Found(at) => Some(at),
                    Parent::Device(id) => {
                        self.lay_out_at_rest();
                        let mut found = self.discovery.found().iter();
                        found.position(|f| f.on_map() == Some(id))
                    }
                };
                self.select_found(shown, None);
            }
        }
    }

    /// The devices the selected card can be a child of, for the card editor's dropdown: its own,
    /// and every other one it can move to, in device order.
    pub(in crate::ui_bridge) fn parent_choices(&self) -> Vec<DeviceId> {
        let Some(card) = self.selected_card() else {
            return Vec::new();
        };
        let owner = self.network.card_owner(card).map(|d| d.id);
        let devices = self.network.devices().iter().map(|d| d.id);
        let choices =
            devices.filter(|&id| Some(id) == owner || self.network.can_move_card(card, id));
        choices.collect()
    }

    /// The parents the selected card of a found host not on the map can have, for its dropdown:
    /// its own host, and every other one it can move to, in [`crate::model::Discovery::parents`]
    /// order. Empty when no such card is selected.
    pub(in crate::ui_bridge) fn found_parent_choices(&self) -> Vec<Parent> {
        let discovery = &self.discovery;
        let (Some(host), Some(card)) = (discovery.selected, discovery.selected_card) else {
            return Vec::new();
        };
        if !discovery.found().get(host).is_some_and(Found::addable) {
            return Vec::new();
        }
        let parents = discovery.parents(&self.network).into_iter();
        let movable = |&to: &Parent| {
            to == Parent::Found(host) || discovery.can_move_card(host, card, to, &self.network)
        };
        parents.filter(movable).collect()
    }

    /// The move a found card's payload dropped on the found host at `index` asks for.
    fn found_drop(&self, data: &DataTransfer, index: i32) -> Option<CardMove> {
        let (host, card) = payload_found(data)?;
        let index = usize::try_from(index).ok()?;
        let to = self.discovery.parent(index, &self.network)?;
        Some(CardMove::Found { host, card, to })
    }
}

/// The text a drag payload carries. Data without text, such as a file dragged in from another
/// app, is not one of our cards, so why it has none does not matter.
fn payload_text(data: &DataTransfer) -> Option<SharedString> {
    data.plain_text().ok()
}

/// Which card of which found host a drag payload carries.
fn payload_found(data: &DataTransfer) -> Option<(usize, usize)> {
    let text = payload_text(data)?;
    let (host, card) = text.strip_prefix(FOUND_PAYLOAD)?.split_once(':')?;
    Some((host.parse().ok()?, card.parse().ok()?))
}

/// The move a card row's payload dropped on the group of the device with UI id `device` asks
/// for.
fn map_drop(data: &DataTransfer, device: i32) -> Option<CardMove> {
    let text = payload_text(data)?;
    let card = convert::card_id(map::payload_child(&text)?)?;
    let to = convert::device_id(device)?;
    Some(CardMove::Map { card, to })
}

/// The "Child of" dropdowns, dropping card rows on other groups, and confirming a move.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    bridge.on_set_card_parent(handler(ui, state, |st, choice| {
        let to = st.parent_choices().get(convert::index(choice)).copied();
        if let (Some(card), Some(to)) = (st.selected_card(), to) {
            st.request_move(CardMove::Map { card, to });
        }
    }));
    let can_state = Rc::clone(state);
    bridge.on_can_move_card(move |data, device| {
        let request = map_drop(&data, device);
        request.is_some_and(|r| can_state.borrow().can_move(r))
    });
    let move_card = handler(ui, state, |st, (data, device): (DataTransfer, i32)| {
        if let Some(request) = map_drop(&data, device) {
            st.request_move(request);
        }
    });
    bridge.on_move_card_to(move |data, device| move_card((data, device)));
    let confirm = handler(ui, state, |st, ()| st.confirm_move());
    bridge.on_confirm_move(move || confirm(()));
    let cancel = handler(ui, state, |st, ()| st.pending_move = None);
    bridge.on_cancel_move(move || cancel(()));
    connect_found(ui, state);
}

/// The same for the cards of found hosts not on the map.
fn connect_found(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    bridge.on_found_card_transfer(|host, card| {
        SharedString::from(format!("{FOUND_PAYLOAD}{host}:{card}")).into()
    });
    let can_state = Rc::clone(state);
    bridge.on_can_move_found_card(move |data, index| {
        let st = can_state.borrow();
        st.found_drop(&data, index).is_some_and(|r| st.can_move(r))
    });
    let move_card = handler(ui, state, |st, (data, index): (DataTransfer, i32)| {
        if let Some(request) = st.found_drop(&data, index) {
            st.request_move(request);
        }
    });
    bridge.on_move_found_card(move |data, index| move_card((data, index)));
    let set_parent = handler(ui, state, |st, (host, card, choice): (i32, i32, i32)| {
        let to = st
            .found_parent_choices()
            .get(convert::index(choice))
            .copied();
        if let Some(to) = to {
            let (host, card) = (convert::index(host), convert::index(card));
            st.request_move(CardMove::Found { host, card, to });
        }
    });
    bridge.on_set_found_card_parent(move |host, card, choice| set_parent((host, card, choice)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    const PC: DeviceId = DeviceId(5);
    const ROUTER: DeviceId = DeviceId(1);

    #[test]
    fn moving_a_devices_only_card_waits_for_confirmation() {
        let mut state = AppState::with_selected(PC.0);
        let card = state.network.device(PC).unwrap().cards[0].id;
        state.selected = Some(Selection::Card(card));
        let to = state.parent_choices().iter().position(|&d| d == ROUTER);
        let request = CardMove::Map { card, to: ROUTER };
        assert!(to.is_some());
        state.request_move(request);
        assert_eq!(state.pending_move, Some(request), "asked first");
        assert!(state.network.device(PC).is_some());
        state.pending_move = None;
        assert!(state.network.device(PC).is_some(), "cancelled");
        state.request_move(request);
        state.confirm_move();
        assert!(state.network.device(PC).is_none());
        assert_eq!(
            state.selected_device(),
            Some(ROUTER),
            "the card stays selected"
        );
        assert!(state.positions.contains_key(&card), "still on the map");
        assert_eq!(state.pending_move, None);
    }

    #[test]
    fn moving_one_of_several_cards_happens_at_once() {
        let mut state = AppState::new(mock_data::network());
        let docker = DeviceId(19);
        let lab = state.network.device(docker).unwrap().cards[1].id;
        state.request_move(CardMove::Map {
            card: lab,
            to: DeviceId(22),
        });
        assert_eq!(state.pending_move, None);
        assert_eq!(
            state.network.card_owner(lab).map(|d| d.id),
            Some(DeviceId(22))
        );
        state.request_move(CardMove::Map {
            card: lab,
            to: DeviceId(999),
        });
        assert_eq!(state.pending_move, None, "refused, not asked");
    }

    /// The sample network after a finished scan of the home subnet.
    fn scanned() -> AppState {
        let mut state = AppState::new(mock_data::network());
        let home = "192.168.1.0/24".parse().unwrap();
        let answers = mock_data::answers(&state.network);
        state.discovery.start(home, answers, &state.network);
        while state.discovery.busy() {
            state.discovery.step();
        }
        state
    }

    #[test]
    fn a_found_hosts_only_card_dropped_on_a_device_asks_then_shows_that_device() {
        let mut state = scanned();
        let found = state.discovery.found();
        let at = |name| found.iter().position(|f| f.device.name == name).unwrap();
        let (speaker, nas) = (at("Kitchen Speaker"), at("NAS"));
        let nas_id = found[nas].on_map().unwrap();
        let data: DataTransfer = SharedString::from(format!("{FOUND_PAYLOAD}{speaker}:0")).into();
        let request = state.found_drop(&data, i32::try_from(nas).unwrap());
        assert_eq!(
            request,
            Some(CardMove::Found {
                host: speaker,
                card: 0,
                to: Parent::Device(nas_id)
            })
        );
        state.request_move(request.unwrap());
        state.confirm_move();
        let shown = state
            .discovery
            .selected
            .and_then(|i| state.discovery.found().get(i));
        assert_eq!(shown.and_then(Found::on_map), Some(nas_id));
        assert_eq!(state.network.device(nas_id).unwrap().cards.len(), 2);
    }

    #[test]
    fn a_found_cards_choices_lead_with_the_hosts_not_on_the_map() {
        let mut state = scanned();
        assert!(state.found_parent_choices().is_empty(), "no card selected");
        let speaker = state
            .discovery
            .found()
            .iter()
            .position(|f| f.device.name == "Kitchen Speaker");
        state.discovery.selected = speaker;
        state.discovery.selected_card = Some(0);
        let choices = state.found_parent_choices();
        assert!(
            choices.contains(&Parent::Found(speaker.unwrap())),
            "its own"
        );
        let first_device = choices.iter().position(|p| matches!(p, Parent::Device(_)));
        let hosts = choices.iter().take_while(|p| matches!(p, Parent::Found(_)));
        assert_eq!(Some(hosts.count()), first_device);
    }

    #[test]
    fn only_a_cards_own_row_drops_on_another_group() {
        let row: DataTransfer = SharedString::from("network-mapper-child:5").into();
        let device_row: DataTransfer = SharedString::from("network-mapper-card:5").into();
        assert!(map_drop(&row, 1).is_some());
        assert!(map_drop(&device_row, 1).is_none());
    }
}
