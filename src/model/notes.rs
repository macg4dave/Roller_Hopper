//! Free-standing annotations saved with the map, independently of its topology.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{GroupColour, Point, fresh_ids};

pub(crate) const NOTE_WIDTH: i16 = 280;
pub(crate) const NOTE_HEIGHT: i16 = 220;
const MAX_NOTES: usize = 1_000;
const MAX_TEXT: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct NoteId(pub(crate) u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Note {
    pub(crate) id: NoteId,
    pub(crate) text: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) colour: GroupColour,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Notes(Vec<Note>);

impl Notes {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Note> {
        self.0.iter()
    }

    pub(crate) fn add(&mut self, x: i32, y: i32) {
        if self.0.len() >= MAX_NOTES {
            return;
        }
        if let Some(id) =
            fresh_ids(self.0.iter().map(|n| n.id.0), 1).and_then(|ids| ids.first().copied())
        {
            self.0.push(Note {
                id: NoteId(id),
                text: String::new(),
                x,
                y,
                colour: GroupColour::Yellow,
            });
        }
    }

    pub(crate) fn edit(&mut self, id: NoteId, text: &str) {
        if let Some(note) = self.0.iter_mut().find(|n| n.id == id) {
            text.clone_into(&mut note.text);
        }
    }

    pub(crate) fn move_to(&mut self, id: NoteId, x: i32, y: i32) {
        if let Some(note) = self.0.iter_mut().find(|n| n.id == id) {
            note.x = x;
            note.y = y;
        }
    }

    pub(crate) fn recolour(&mut self, id: NoteId) {
        if let Some(note) = self.0.iter_mut().find(|n| n.id == id) {
            note.colour = match note.colour {
                GroupColour::Yellow => GroupColour::Blue,
                GroupColour::Blue => GroupColour::Purple,
                GroupColour::Purple => GroupColour::Green,
                GroupColour::Green | GroupColour::Red | GroupColour::Orange => GroupColour::Yellow,
            };
        }
    }

    pub(crate) fn remove(&mut self, id: NoteId) {
        self.0.retain(|note| note.id != id);
    }

    pub(crate) fn valid(&self) -> bool {
        let mut ids = HashSet::new();
        self.0.len() <= MAX_NOTES
            && self.0.iter().all(|note| {
                note.id.0 <= super::MAX_MODEL_ID
                    && ids.insert(note.id)
                    && note.text.len() <= MAX_TEXT
            })
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "The map renders coordinates as f32; rounding very distant notes to the nearest representable map unit is acceptable"
    )]
    pub(crate) fn corners(&self) -> impl Iterator<Item = Point> + '_ {
        self.0.iter().flat_map(|note| {
            let x = note.x as f32;
            let y = note.y as f32;
            [
                Point { x, y },
                Point {
                    x: x + f32::from(NOTE_WIDTH),
                    y: y + f32::from(NOTE_HEIGHT),
                },
            ]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_can_be_edited_moved_coloured_and_removed_independently() {
        let mut notes = Notes::default();
        notes.add(10, 20);
        notes.add(30, 40);
        let id = notes.0[0].id;
        notes.edit(id, "First line\nSecond line");
        notes.move_to(id, -50, 60);
        notes.recolour(id);
        let note = &notes.0[0];
        assert_eq!(
            (note.text.as_str(), note.x, note.y, note.colour),
            ("First line\nSecond line", -50, 60, GroupColour::Blue)
        );
        notes.edit(id, &"x".repeat(MAX_TEXT + 1));
        assert!(
            !notes.valid(),
            "oversized text is kept so Save can report the limit"
        );
        notes.remove(id);
        assert_eq!(notes.0.len(), 1);
        assert_eq!(notes.0[0].text, "");
        assert!(notes.valid());
    }
}
