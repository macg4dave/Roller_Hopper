//! One-step map-edit undo. Matching the resulting network prevents restoring a snapshot over
//! later edits or discovery imports; notes remain independent of topology changes.

use crate::model::{Arrangement, Network};
use crate::ui_bridge::AppState;

/// A bounded snapshot before and after the latest drag or automatic arrangement.
pub(in crate::ui_bridge) struct MapUndo {
    before_network: Network,
    before_arrangement: Arrangement,
    after_network: Network,
    after_arrangement: Arrangement,
}

impl AppState {
    pub(in crate::ui_bridge) fn remember_map_edit(
        &mut self,
        network: Network,
        arrangement: Arrangement,
    ) {
        if network != self.network || arrangement != self.arrangement {
            self.map_undo = Some(MapUndo {
                before_network: network,
                before_arrangement: arrangement,
                after_network: self.network.clone(),
                after_arrangement: self.arrangement.clone(),
            });
        }
    }

    pub(in crate::ui_bridge) fn can_undo_map_edit(&self) -> bool {
        self.dragging.is_none()
            && self.bubble_gesture.is_none()
            && self.map_undo.as_ref().is_some_and(|undo| {
                self.network == undo.after_network && self.arrangement == undo.after_arrangement
            })
    }

    pub(in crate::ui_bridge) fn undo_map_edit(&mut self) {
        if !self.can_undo_map_edit() {
            return;
        }
        let Some(undo) = self.map_undo.take() else {
            return;
        };
        self.network = undo.before_network;
        self.arrangement = undo.before_arrangement;
        self.auto_map = None;
        self.auto_map_undo = None;
        self.lay_out_at_rest();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::{CardId, DeviceId, Point};

    fn disconnect(state: &mut AppState, card: CardId) {
        state.reconnect_cards = true;
        state.drag_to(
            card,
            Point {
                x: -1000.0,
                y: -1000.0,
            },
        );
        state.drop_dragged();
    }

    #[test]
    fn undo_restores_connections_arrangement_and_positions() {
        let mut state = AppState::new(mock_data::network());
        let before = (
            state.network.clone(),
            state.arrangement.clone(),
            state.positions.clone(),
        );
        disconnect(&mut state, CardId(5));
        assert!(state.network.uplink(CardId(5)).is_none());
        assert!(state.can_undo_map_edit());
        state.undo_map_edit();
        assert_eq!(
            (
                state.network.clone(),
                state.arrangement.clone(),
                state.positions.clone()
            ),
            before
        );
        assert_eq!(state.motion.shown(), &state.positions);
        assert!(!state.can_undo_map_edit());
    }

    #[test]
    fn another_network_edit_prevents_undo_from_overwriting_it() {
        let mut state = AppState::new(mock_data::network());
        disconnect(&mut state, CardId(5));
        state.network.device_mut(DeviceId(5)).unwrap().name = "Edited workstation".into();
        let edited = state.network.clone();
        assert!(!state.can_undo_map_edit());
        state.undo_map_edit();
        assert_eq!(state.network, edited);
    }

    #[test]
    fn undo_restores_a_reordered_tier_without_discarding_notes() {
        let mut state = AppState::new(mock_data::network());
        state.reconnect_cards = true;
        let network = state.network.clone();
        let arrangement = state.arrangement.clone();
        let switch = state.positions[&CardId(2)];
        let access_point = state.positions[&CardId(3)];
        state.drag_to(
            CardId(4),
            Point {
                x: f32::midpoint(switch.x, access_point.x),
                y: switch.y,
            },
        );
        state.drop_dragged();
        assert_eq!(state.network, network);
        assert_ne!(state.arrangement, arrangement);
        state.notes.add(40, 60);
        let notes = state.notes.clone();
        state.undo_map_edit();
        assert_eq!(state.arrangement, arrangement);
        assert_eq!(state.network, network);
        assert_eq!(state.notes, notes);
    }

    #[test]
    fn an_undone_move_survives_save_and_reopen_without_its_history() {
        let mut state = AppState::new(mock_data::network());
        let before = state.network.clone();
        disconnect(&mut state, CardId(5));
        state.undo_map_edit();
        let project = crate::project::Project {
            network: state.network,
            arrangement: state.arrangement,
            notes: state.notes,
        };
        let path = std::env::temp_dir().join(format!(
            "rhopper-drag-undo-{}.rhopper.json",
            std::process::id()
        ));
        crate::project::write(&path, &project).unwrap();
        let reopened = AppState::with_project(crate::project::read(&path).unwrap(), None);
        std::fs::remove_file(path).unwrap();
        assert_eq!(reopened.network, before);
        assert_eq!(reopened.arrangement, project.arrangement);
        assert!(!reopened.can_undo_map_edit());
    }

    #[test]
    fn cancelling_a_second_drag_preserves_the_previous_undo() {
        let mut state = AppState::new(mock_data::network());
        let network = state.network.clone();
        disconnect(&mut state, CardId(5));
        state.drag_to(CardId(6), Point::default());
        assert!(!state.can_undo_map_edit());
        state.undo_map_edit();
        assert_eq!(state.dragging, Some(CardId(6)));
        state.cancel_drag();
        assert!(state.can_undo_map_edit());
        state.undo_map_edit();
        assert_eq!(state.network, network);
    }

    #[test]
    fn undo_is_one_step_and_new_projects_do_not_inherit_it() {
        let mut state = AppState::new(mock_data::network());
        disconnect(&mut state, CardId(5));
        let after_first = state.network.clone();
        disconnect(&mut state, CardId(6));
        state.undo_map_edit();
        assert_eq!(state.network, after_first);
        state.undo_map_edit();
        assert_eq!(state.network, after_first);
        assert!(!AppState::new(mock_data::network()).can_undo_map_edit());
    }
}
