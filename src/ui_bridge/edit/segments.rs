//! Editing Layer 2 segments on the edit page, and putting the selected card in one.

use std::cell::RefCell;
use std::rc::Rc;

use ipnet::Ipv4Net;
use slint::{ComponentHandle, SharedString};

use super::cards::optional;
use crate::model::{MembershipMode, Segment, SegmentId};
use crate::ui::{self, AppWindow, Bridge};
use crate::ui_bridge::{AppState, convert, handler, update};

/// The VLAN IDs a switch can use; 0 and 4095 are reserved.
const VLANS: std::ops::RangeInclusive<u16> = 1..=4094;

impl AppState {
    /// The segment at `index` in the segment list.
    fn segment_at(&self, index: usize) -> Option<SegmentId> {
        self.network.segments().get(index).map(|segment| segment.id)
    }

    /// Changes the segment at `index` in the list; false when there is none.
    fn edit_segment(&mut self, index: usize, edit: impl FnOnce(&mut Segment)) -> bool {
        let segment = self.segment_at(index);
        segment
            .and_then(|id| self.network.segment_mut(id))
            .map(edit)
            .is_some()
    }

    /// Puts the selected card in the segment at `choice` in the dropdown, where 0 is none. A
    /// card moved between segments keeps its mode.
    fn set_card_segment(&mut self, choice: usize) {
        let Some(card) = self.selected_card() else {
            return;
        };
        let segment = choice.checked_sub(1).and_then(|at| self.segment_at(at));
        let mode = self.network.segment_of(card).map(|(_, mode)| mode);
        self.network
            .set_membership(card, segment, mode.unwrap_or_default());
    }

    /// Sets how the selected card carries its segment, by index in [`MembershipMode::ALL`].
    fn set_membership_mode(&mut self, choice: usize) {
        let Some(&mode) = MembershipMode::ALL.get(choice) else {
            return;
        };
        let Some(card) = self.selected_card() else {
            return;
        };
        if let Some((segment, _)) = self.network.segment_of(card) {
            self.network.set_membership(card, Some(segment), mode);
        }
    }

    /// Sets a segment's VLAN as typed: blank for none, or a usable VLAN ID.
    fn set_segment_vlan(&mut self, index: usize, text: &str) -> bool {
        match optional::<u16>(text) {
            Ok(vlan) if vlan.is_none_or(|vlan| VLANS.contains(&vlan)) => {
                self.edit_segment(index, |segment| segment.vlan = vlan)
            }
            _ => false,
        }
    }

    /// Sets a segment's subnet as typed (`a.b.c.d/nn`), or none when blank.
    fn set_segment_subnet(&mut self, index: usize, text: &str) -> bool {
        optional::<Ipv4Net>(text).is_ok_and(|subnet| {
            let subnet = subnet.map(|subnet| subnet.trunc());
            self.edit_segment(index, |segment| segment.subnet = subnet)
        })
    }
}

/// The segment list's editing callbacks and the selected card's segment.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    bridge.on_set_card_segment(handler(ui, state, |st, choice| {
        st.set_card_segment(convert::index(choice));
    }));
    bridge.on_set_membership_mode(handler(ui, state, |st, choice| {
        st.set_membership_mode(convert::index(choice));
    }));
    bridge.on_add_segment(handler(ui, state, |st, name: SharedString| {
        st.network.add_segment(&name);
    }));
    let rename = handler(ui, state, |st, (index, name): (i32, SharedString)| {
        if let Some(id) = st.segment_at(convert::index(index)) {
            st.network.rename_segment(id, &name);
        }
    });
    bridge.on_rename_segment(move |index, name| rename((index, name)));
    let recolour = handler(ui, state, |st, (index, colour): (i32, ui::GroupColour)| {
        if let Some(colour) = convert::colour_from_ui(colour) {
            st.edit_segment(convert::index(index), |segment| segment.colour = colour);
        }
    });
    bridge.on_set_segment_colour(move |index, colour| recolour((index, colour)));
    bridge.on_remove_segment(handler(ui, state, |st, index| {
        if let Some(id) = st.segment_at(convert::index(index)) {
            st.network.remove_segment(id);
        }
    }));
    let (vlan_ui, vlan_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_set_segment_vlan(move |index, text| {
        let mut accepted = false;
        update(&vlan_ui, &vlan_state, |st| {
            accepted = st.set_segment_vlan(convert::index(index), &text);
        });
        accepted
    });
    let (subnet_ui, subnet_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_set_segment_subnet(move |index, text| {
        let mut accepted = false;
        update(&subnet_ui, &subnet_state, |st| {
            accepted = st.set_segment_subnet(convert::index(index), &text);
        });
        accepted
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::CardId;
    use crate::ui_bridge::Selection;

    fn with_segments() -> AppState {
        let mut state = AppState::new(mock_data::network());
        state.network.add_segment("LAN").unwrap();
        state.network.add_segment("SMART").unwrap();
        state
    }

    #[test]
    fn a_vlan_is_a_usable_id_or_blank() {
        let mut state = with_segments();
        assert!(state.set_segment_vlan(0, " 20 "));
        assert_eq!(state.network.segments()[0].vlan, Some(20));
        for refused in ["0", "4095", "vlan", "-1"] {
            assert!(!state.set_segment_vlan(0, refused), "{refused:?}");
        }
        assert_eq!(state.network.segments()[0].vlan, Some(20), "unchanged");
        assert!(state.set_segment_vlan(0, ""));
        assert_eq!(state.network.segments()[0].vlan, None);
        assert!(!state.set_segment_vlan(9, "20"), "no such segment");
    }

    #[test]
    fn a_subnet_is_kept_as_its_network() {
        let mut state = with_segments();
        assert!(state.set_segment_subnet(1, "192.168.20.7/24"));
        let subnet = state.network.segments()[1].subnet.map(|s| s.to_string());
        assert_eq!(subnet.as_deref(), Some("192.168.20.0/24"));
        assert!(!state.set_segment_subnet(1, "192.168.20.0"));
    }

    #[test]
    fn a_card_keeps_its_mode_when_moved_to_another_segment() {
        let mut state = with_segments();
        let card = CardId(1);
        state.selected = Some(Selection::Card(card));
        state.set_membership_mode(2);
        assert_eq!(state.network.segment_of(card), None, "in no segment yet");
        state.set_card_segment(1);
        state.set_membership_mode(2);
        state.set_card_segment(2);
        let smart = state.network.segments()[1].id;
        assert_eq!(
            state.network.segment_of(card),
            Some((smart, MembershipMode::Native))
        );
        state.set_card_segment(0);
        assert_eq!(state.network.segment_of(card), None);
    }
}
