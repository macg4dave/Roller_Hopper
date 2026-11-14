//! Two-group acceptance and lifecycle checks use the same state operations as the UI.

use super::*;
use crate::mock_data;
use crate::model::{DeviceId, tree_layout};
use crate::ui_bridge::Selection;

fn state() -> (AppState, i32, i32) {
    let mut state = AppState::new(mock_data::network());
    state.add_bubble(
        "Lab",
        Point {
            x: -940.0,
            y: 160.0,
        },
    );
    state.add_bubble(
        "Office",
        Point {
            x: 260.0,
            y: -640.0,
        },
    );
    let index = |name| {
        i32::try_from(
            state
                .network
                .groups()
                .iter()
                .position(|(_, n)| n.typed() == Some(name))
                .unwrap(),
        )
        .unwrap()
    };
    let (lab, office) = (index("Lab"), index("Office"));
    (state, lab, office)
}

fn select(state: &mut AppState, id: u32) {
    state.selected = Some(Selection::Device(DeviceId(id)));
}

#[test]
fn names_reuse_existing_groups_and_renaming_keeps_membership() {
    let (mut state, lab, _) = state();
    let count = state.network.groups().iter().count();
    let before = state.arrangement.clone();
    state.add_bubble("lab", Point::default());
    state.add_bubble("  ", Point::default());
    assert_eq!(state.network.groups().iter().count(), count);
    assert_eq!(state.arrangement, before);
    select(&mut state, 1);
    state.set_bubble_member(lab, true);
    let group = state.bubble_group(lab).unwrap();
    let arrangement = state.arrangement.clone();
    state.edit_bubbles(|st| st.network.rename_group(group, "Research"));
    assert_eq!(state.arrangement, arrangement);
    assert!(
        state
            .network
            .groups()
            .iter()
            .any(|(id, name)| id == group && name.typed() == Some("Research"))
    );
    state.undo_map_edit();
    assert_eq!(state.arrangement, arrangement);
    assert!(
        state
            .network
            .groups()
            .iter()
            .any(|(id, name)| id == group && name.typed() == Some("Lab"))
    );
}

#[test]
fn two_bubbles_keep_their_external_links_and_move_every_member_card_together() {
    let (mut state, lab, office) = state();
    let network = state.network.clone();
    select(&mut state, 1);
    state.set_bubble_member(lab, true);
    select(&mut state, 2);
    state.set_bubble_member(office, true);
    let before = state.positions.clone();
    let arrangement = state.arrangement.clone();
    state.change_bubble_geometry(
        lab,
        Point {
            x: -1100.0,
            y: 80.0,
        },
        false,
    );
    state.change_bubble_geometry(
        lab,
        Point {
            x: -1000.0,
            y: 160.0,
        },
        false,
    );
    assert!(!state.can_undo_map_edit());
    state.finish_bubble_gesture();
    for device in network.devices() {
        for card in &device.cards {
            let mut expected = before[&card.id];
            if device.id == DeviceId(1) {
                expected.x += 200.0;
                expected.y += 160.0;
            }
            assert_eq!(state.positions[&card.id], expected);
        }
    }
    assert_eq!(state.network, network);
    state.undo_map_edit();
    assert_eq!(state.positions, before);
    assert_eq!(state.arrangement, arrangement);
}

#[test]
fn transfer_between_bubbles_is_explicit_and_never_changes_host_or_role() {
    let (mut state, lab, office) = state();
    let network = state.network.clone();
    select(&mut state, 24);
    state.set_bubble_member(lab, true);
    state.set_bubble_member(office, true);
    let bubbles = state.arrangement.bubbles();
    assert!(!bubbles[0].members.contains(&DeviceId(24)));
    assert_eq!(bubbles[1].members, [DeviceId(24)]);
    state.set_bubble_member(office, false);
    assert!(
        state
            .arrangement
            .bubbles()
            .iter()
            .all(|b| b.members.is_empty())
    );
    assert_eq!(state.network, network);
    state.undo_map_edit();
    assert_eq!(state.arrangement.bubbles()[1].members, [DeviceId(24)]);
}

#[test]
fn resize_contains_members_and_cancelling_a_gesture_restores_the_entire_layout() {
    let (mut state, lab, _) = state();
    select(&mut state, 1);
    state.set_bubble_member(lab, true);
    let before = state.arrangement.clone();
    state.change_bubble_geometry(lab, Point { x: 900.0, y: 600.0 }, true);
    state.finish_bubble_gesture();
    assert_eq!(
        state.arrangement.bubbles()[0].size,
        Point { x: 900.0, y: 600.0 }
    );
    state.undo_map_edit();
    assert_eq!(state.arrangement, before);
    state.change_bubble_geometry(
        lab,
        Point {
            x: -800.0,
            y: 200.0,
        },
        false,
    );
    state.cancel_bubble_gesture();
    assert_eq!(state.arrangement, before);
    state.change_bubble_geometry(lab, Point { x: 1.0, y: 1.0 }, true);
    state.finish_bubble_gesture();
    let bubble = &state.arrangement.bubbles()[0];
    assert!(bubble.size.x >= 520.0 && bubble.size.y >= 320.0);
    for card in &state.network.device(DeviceId(1)).unwrap().cards {
        let at = state.positions[&card.id];
        assert!(at.x > bubble.origin.x && at.x < bubble.origin.x + bubble.size.x);
        assert!(at.y > bubble.origin.y && at.y < bubble.origin.y + bubble.size.y);
    }
}

#[test]
fn deleting_a_bubble_keeps_devices_and_automatic_arrangement_preserves_other_bubbles() {
    let (mut state, lab, office) = state();
    select(&mut state, 1);
    state.set_bubble_member(lab, true);
    let group = state.bubble_group(office).unwrap();
    let network = state.network.clone();
    let before = state.arrangement.clone();
    state.edit_bubbles(|st| st.arrangement.remove_bubble(group));
    assert_eq!(state.arrangement.bubbles().len(), 1);
    assert_eq!(state.network, network);
    state.undo_map_edit();
    assert_eq!(state.arrangement, before);
    let bubbles = state.arrangement.bubbles().to_vec();
    state.arrangement.reset_layout(&state.network);
    assert_eq!(state.arrangement.bubbles(), bubbles);
    let laid_out = tree_layout(&state.network, &state.arrangement);
    for card in &state.network.device(DeviceId(1)).unwrap().cards {
        assert_eq!(laid_out[&card.id], state.positions[&card.id]);
    }
}

#[test]
fn bubbles_and_members_round_trip_through_production_project_io() {
    let (mut state, lab, office) = state();
    select(&mut state, 1);
    state.set_bubble_member(lab, true);
    select(&mut state, 2);
    state.set_bubble_member(office, true);
    let project = crate::project::Project {
        network: state.network,
        arrangement: state.arrangement,
        notes: state.notes,
    };
    let path = std::env::temp_dir().join(format!(
        "rhopper-bubbles-{}.rhopper.json",
        std::process::id()
    ));
    crate::project::write(&path, &project).unwrap();
    let read = crate::project::read(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(read, project);
    let reopened = AppState::with_project(read, None);
    assert_eq!(reopened.arrangement.bubbles().len(), 2);
    assert!(!reopened.can_undo_map_edit());
}

#[test]
fn stale_duplicate_and_invalid_bubble_records_are_cleaned_on_load() {
    let (mut state, lab, _) = state();
    select(&mut state, 1);
    state.set_bubble_member(lab, true);
    let mut json = serde_json::to_value(&state.arrangement).unwrap();
    let duplicate = json["bubbles"][0].clone();
    json["bubbles"].as_array_mut().unwrap().push(duplicate);
    json["bubbles"][1]["members"] = serde_json::json!([1, 1, 999_999]);
    let mut arrangement: crate::model::Arrangement = serde_json::from_value(json).unwrap();
    arrangement.retain(&state.network);
    assert_eq!(arrangement.bubbles().len(), 2);
    assert_eq!(arrangement.bubbles()[0].members, [DeviceId(1)]);
    assert_eq!(arrangement.bubbles()[1].members, [] as [DeviceId; 0]);
    let mut invalid = serde_json::to_value(&arrangement).unwrap();
    invalid["bubbles"][0]["size"]["x"] = serde_json::json!(1e100);
    let mut cleaned: crate::model::Arrangement = serde_json::from_value(invalid).unwrap();
    cleaned.retain(&state.network);
    assert_eq!(cleaned.bubbles().len(), 1);
}
