//! Placement regression checks: moving is independent of topology, layout is deliberate,
//! and positions survive the production project save/open path.

use super::*;
use crate::mock_data;
use crate::project::{self, Project};

fn state() -> AppState {
    AppState::new(mock_data::network())
}

fn move_card(state: &mut AppState, at: Point) {
    state.drag_to(CardId(5), at);
    state.drop_dragged();
}

#[test]
fn free_movement_preserves_every_link_and_the_other_positions() {
    let mut state = state();
    let network = state.network.clone();
    let mut expected = state.positions.clone();
    let target = Point {
        x: -450.5,
        y: -220.25,
    };
    expected.insert(CardId(5), target);
    state.drag_to(CardId(5), target);
    assert!(state.strand().is_none());
    assert_eq!(state.links().len(), network.map_connections().len());
    assert!(state.links().iter().any(|(link, route)| {
        link.downstream == CardId(5)
            && route
                .last()
                .is_some_and(|end| (end.x - target.x).abs() < f32::EPSILON)
    }));
    state.drop_dragged();
    assert_eq!(state.network, network);
    assert_eq!(state.positions, expected);
    assert_eq!(state.motion.shown(), &expected);
    assert!(state.can_undo_map_edit());
    state.undo_map_edit();
    assert_eq!(
        state.positions,
        tree_layout(&network, &crate::model::Arrangement::default())
    );
}

#[test]
fn arrangement_is_explicit_and_undo_restores_manual_positions() {
    let mut state = state();
    move_card(
        &mut state,
        Point {
            x: -350.0,
            y: 100.0,
        },
    );
    let manual = state.positions.clone();
    let network = state.network.clone();
    state.arrange_automatically();
    assert!(!state.arrangement.has_positions());
    assert_eq!(state.network, network);
    assert_ne!(state.positions, manual);
    state.undo_map_edit();
    assert_eq!(state.positions, manual);
    assert_eq!(state.network, network);
}

#[test]
fn cancelling_or_changing_mode_does_not_commit_a_move() {
    let mut state = state();
    move_card(
        &mut state,
        Point {
            x: -350.0,
            y: 100.0,
        },
    );
    let manual = state.positions.clone();
    let arrangement = state.arrangement.clone();
    state.drag_to(CardId(5), Point::default());
    state.cancel_drag();
    assert_eq!(state.positions, manual);
    assert_eq!(state.arrangement, arrangement);
    state.drag_to(CardId(5), Point::default());
    state.set_reconnect_cards(true);
    assert_eq!(state.dragging, None);
    assert_eq!(state.positions, manual);
    assert_eq!(state.arrangement, arrangement);
}

#[test]
fn invalid_positions_and_a_stationary_drag_do_not_dirty_the_project() {
    let mut state = state();
    let arrangement = state.arrangement.clone();
    for at in [
        Point {
            x: f32::NAN,
            y: 0.0,
        },
        Point {
            x: f32::INFINITY,
            y: 0.0,
        },
        Point {
            x: 0.0,
            y: 1_000_001.0,
        },
    ] {
        state.drag_to(CardId(5), at);
        assert_eq!(state.dragging, None);
    }
    let original = state.positions[&CardId(5)];
    move_card(&mut state, original);
    assert_eq!(state.arrangement, arrangement);
    assert!(!state.can_undo_map_edit());
}

#[test]
fn manual_positions_survive_save_reopen_and_view_refresh() {
    let mut state = state();
    move_card(
        &mut state,
        Point {
            x: -450.5,
            y: -220.25,
        },
    );
    let manual = state.positions.clone();
    let project = Project {
        network: state.network,
        arrangement: state.arrangement,
        notes: state.notes,
    };
    let path = std::env::temp_dir().join(format!(
        "rhopper-placement-{}.rhopper.json",
        std::process::id()
    ));
    project::write(&path, &project).unwrap();
    let mut reopened = AppState::with_project(project::read(&path).unwrap(), None);
    std::fs::remove_file(path).unwrap();
    assert_eq!(reopened.positions, manual);
    reopened.lay_out_at_rest();
    assert_eq!(reopened.positions, manual);
    assert_eq!(reopened.network, project.network);
    assert!(!reopened.can_undo_map_edit());
}
