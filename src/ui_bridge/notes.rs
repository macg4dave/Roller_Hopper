//! Canvas annotation callbacks and their presentation model.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use super::{AppState, convert, handler, view::sync};
use crate::model::{NOTE_HEIGHT, NOTE_WIDTH, NoteId};
use crate::ui::{self, AppWindow, Bridge};

pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let add = handler(ui, state, |st, (x, y): (i32, i32)| {
        st.notes.add(
            x.saturating_sub(i32::from(NOTE_WIDTH) / 2),
            y.saturating_sub(i32::from(NOTE_HEIGHT) / 2),
        );
    });
    bridge.on_add_map_note(move |x, y| add((x, y)));
    let edit = handler(ui, state, |st, (id, text): (i32, SharedString)| {
        if let Ok(id) = u32::try_from(id) {
            st.notes.edit(NoteId(id), &text);
        }
    });
    bridge.on_edit_map_note(move |id, text| edit((id, text)));
    let move_to = handler(ui, state, |st, (id, x, y)| {
        if let Ok(id) = u32::try_from(id) {
            st.notes.move_to(NoteId(id), x, y);
        }
    });
    bridge.on_move_map_note(move |id, x, y| move_to((id, x, y)));
    bridge.on_delete_map_note(handler(ui, state, |st, id| {
        if let Ok(id) = u32::try_from(id) {
            st.notes.remove(NoteId(id));
        }
    }));
    bridge.on_colour_map_note(handler(ui, state, |st, id| {
        if let Ok(id) = u32::try_from(id) {
            st.notes.recolour(NoteId(id));
        }
    }));
}

pub(super) fn push(bridge: &Bridge<'_>, state: &AppState) {
    let rows = state
        .notes
        .iter()
        .map(|note| ui::MapNote {
            id: i32::try_from(note.id.0).unwrap_or_default(),
            text: note.text.as_str().into(),
            x: note.x,
            y: note.y,
            width: i32::from(NOTE_WIDTH),
            height: i32::from(NOTE_HEIGHT),
            colour: convert::colour(Some(note.colour)),
        })
        .collect();
    if let Some(model) = sync(&bridge.get_map_notes(), rows) {
        bridge.set_map_notes(model);
    }
}
