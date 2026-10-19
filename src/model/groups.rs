//! Groups the user sorts roles into. A role is in at most one group; groups are optional.
//!
//! Group names are the user's own words, kept unique ignoring case. A group may have a colour,
//! which its roles' devices are shown in.

/// Identifies a group, so renaming it does not lose the roles that are in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GroupId(u32);

/// The colours a group can be given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GroupColour {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Group {
    id: GroupId,
    name: String,
    colour: Option<GroupColour>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Groups {
    list: Vec<Group>,
    next_id: u32,
}

impl Groups {
    /// Every group, in the order it was added.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (GroupId, &str)> {
        self.list.iter().map(|g| (g.id, g.name.as_str()))
    }

    fn named_ignoring_case(&self, name: &str) -> Option<GroupId> {
        let lower = name.to_lowercase();
        self.iter()
            .find(|(_, n)| n.to_lowercase() == lower)
            .map(|(id, _)| id)
    }

    /// Adds a group and returns it; an existing group with the same name is returned instead of
    /// a duplicate, and a blank name is rejected.
    pub(crate) fn add(&mut self, name: &str) -> Option<GroupId> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        if let Some(existing) = self.named_ignoring_case(name) {
            return Some(existing);
        }
        let id = GroupId(self.next_id);
        self.next_id += 1;
        self.list.push(Group {
            id,
            name: name.to_owned(),
            colour: None,
        });
        Some(id)
    }

    /// Renames a group. A blank name, or one another group already has, is ignored.
    pub(crate) fn rename(&mut self, id: GroupId, new: &str) {
        let new = new.trim();
        let taken = self
            .named_ignoring_case(new)
            .is_some_and(|other| other != id);
        if new.is_empty() || taken {
            return;
        }
        if let Some(group) = self.list.iter_mut().find(|g| g.id == id) {
            new.clone_into(&mut group.name);
        }
    }

    pub(crate) fn remove(&mut self, id: GroupId) {
        self.list.retain(|g| g.id != id);
    }

    pub(crate) fn colour(&self, id: GroupId) -> Option<GroupColour> {
        self.list.iter().find(|g| g.id == id).and_then(|g| g.colour)
    }

    /// Gives a group a colour, or takes it away with `None`.
    pub(crate) fn set_colour(&mut self, id: GroupId, colour: Option<GroupColour>) {
        if let Some(group) = self.list.iter_mut().find(|g| g.id == id) {
            group.colour = colour;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(groups: &Groups) -> Vec<&str> {
        groups.iter().map(|(_, name)| name).collect()
    }

    #[test]
    fn names_stay_unique_ignoring_case_and_never_blank() {
        let mut groups = Groups::default();
        let office = groups.add(" Office ");
        assert_eq!(groups.add("OFFICE"), office);
        assert_eq!(groups.add("   "), None);
        let home = groups.add("Home");
        assert_ne!(home, office);
        assert_eq!(names(&groups), ["Office", "Home"]);
    }

    #[test]
    fn renaming_keeps_the_group_and_rejects_taken_names() {
        let mut groups = Groups::default();
        let office = groups.add("Office").unwrap();
        groups.add("Home");
        groups.rename(office, "home");
        groups.rename(office, " ");
        assert_eq!(names(&groups), ["Office", "Home"]);
        groups.rename(office, "Work");
        assert_eq!(groups.iter().next(), Some((office, "Work")));
    }

    #[test]
    fn a_group_starts_without_a_colour_and_can_lose_it_again() {
        let mut groups = Groups::default();
        let office = groups.add("Office").unwrap();
        let home = groups.add("Home").unwrap();
        assert_eq!(groups.colour(office), None);
        groups.set_colour(office, Some(GroupColour::Blue));
        assert_eq!(groups.colour(office), Some(GroupColour::Blue));
        assert_eq!(groups.colour(home), None, "other groups unchanged");
        groups.set_colour(office, None);
        assert_eq!(groups.colour(office), None);
    }

    #[test]
    fn a_removed_group_is_gone_and_its_id_is_not_reused() {
        let mut groups = Groups::default();
        let office = groups.add("Office").unwrap();
        groups.remove(office);
        assert_eq!(groups.iter().count(), 0);
        assert_ne!(groups.add("Office"), Some(office));
    }
}
