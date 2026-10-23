//! Groups the user sorts roles into. A role is in at most one group; groups are optional.
//!
//! A new project starts with the standard groups, each with its own colour and line style,
//! which are named in the UI's language until the user renames them. Any other group is named
//! by the user, and the names the user typed are kept unique ignoring case. A group may have a
//! colour, which its roles' devices are shown in, and a line style their lines are drawn in.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::device::Role;

/// Identifies a group, so renaming it does not lose the roles that are in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct GroupId(u32);

/// The colours a group can be given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum GroupColour {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
}

/// How the lines from a group's devices are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub(crate) enum LineStyle {
    #[default]
    Solid,
    Thick,
    Dashed,
    LongDashes,
    Dotted,
    DashDot,
    Zigzag,
    Wavy,
}

impl LineStyle {
    /// Every style, in the order the style dropdown lists them.
    pub(crate) const ALL: [Self; 8] = [
        Self::Solid,
        Self::Thick,
        Self::Dashed,
        Self::LongDashes,
        Self::Dotted,
        Self::DashDot,
        Self::Zigzag,
        Self::Wavy,
    ];
}

/// The groups a new project starts with, one for each kind of standard role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum StandardGroup {
    NetworkCore,
    Switching,
    WiFi,
    Mobile,
    Workstations,
    Printing,
    Servers,
    Storage,
    Security,
    SmartHome,
}

impl StandardGroup {
    /// Every standard group, in the order a new project lists them.
    pub(crate) const ALL: [Self; 10] = [
        Self::NetworkCore,
        Self::Switching,
        Self::WiFi,
        Self::Mobile,
        Self::Workstations,
        Self::Printing,
        Self::Servers,
        Self::Storage,
        Self::Security,
        Self::SmartHome,
    ];

    /// The standard group `role` starts in; none for Other and the user's own roles.
    pub(crate) fn of(role: &Role) -> Option<Self> {
        match role {
            Role::Router => Some(Self::NetworkCore),
            Role::Switch => Some(Self::Switching),
            Role::AccessPoint => Some(Self::WiFi),
            Role::Laptop | Role::Phone => Some(Self::Mobile),
            Role::Desktop => Some(Self::Workstations),
            Role::Printer => Some(Self::Printing),
            Role::Server => Some(Self::Servers),
            Role::Nas => Some(Self::Storage),
            Role::Camera => Some(Self::Security),
            Role::Iot => Some(Self::SmartHome),
            Role::Other | Role::Custom(_) => None,
        }
    }

    /// How it starts out looking, each pairing its own: the router, the switch and the access
    /// point in colours of their own, what hangs off Wi-Fi wavy or dotted.
    fn looks(self) -> (GroupColour, LineStyle) {
        use GroupColour::{Blue, Green, Orange, Purple, Red, Yellow};
        use LineStyle::{DashDot, Dotted, LongDashes, Solid, Thick, Wavy, Zigzag};
        match self {
            Self::NetworkCore => (Purple, Thick),
            Self::Switching => (Blue, Solid),
            Self::WiFi => (Green, Wavy),
            Self::Mobile => (Green, Dotted),
            Self::Workstations => (Yellow, Solid),
            Self::Printing => (Yellow, DashDot),
            Self::Servers => (Red, Thick),
            Self::Storage => (Orange, LongDashes),
            Self::Security => (Red, Zigzag),
            Self::SmartHome => (Orange, Zigzag),
        }
    }
}

/// What a group is called. A plain name is saved as a string, as every group was before there
/// were standard ones, so older project files still open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum GroupName {
    /// The user's own group.
    Named(String),
    /// A standard group, named in the UI's language unless the user `renamed` it.
    Standard {
        standard: StandardGroup,
        renamed: Option<String>,
    },
}

impl GroupName {
    /// The name the user typed, if any; `None` means the UI shows the standard group's name.
    pub(crate) fn typed(&self) -> Option<&str> {
        match self {
            Self::Named(name) => Some(name),
            Self::Standard { renamed, .. } => renamed.as_deref(),
        }
    }

    fn standard(&self) -> Option<StandardGroup> {
        match self {
            Self::Named(_) => None,
            Self::Standard { standard, .. } => Some(*standard),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Group {
    id: GroupId,
    name: GroupName,
    colour: Option<GroupColour>,
    line_style: LineStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct Groups {
    list: Vec<Group>,
    next_id: u32,
}

impl Groups {
    /// Every group, in the order it was added.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (GroupId, &GroupName)> {
        self.list.iter().map(|g| (g.id, &g.name))
    }

    /// The group with a name the user typed that is `name`, ignoring case. Standard names are
    /// translated in the UI, so they are not compared here.
    fn named_ignoring_case(&self, name: &str) -> Option<GroupId> {
        let lower = name.to_lowercase();
        self.iter()
            .find(|(_, n)| n.typed().is_some_and(|n| n.to_lowercase() == lower))
            .map(|(id, _)| id)
    }

    /// The standard group, unless it was deleted.
    pub(crate) fn standard(&self, standard: StandardGroup) -> Option<GroupId> {
        self.iter()
            .find(|(_, n)| n.standard() == Some(standard))
            .map(|(id, _)| id)
    }

    fn push(
        &mut self,
        name: GroupName,
        colour: Option<GroupColour>,
        line_style: LineStyle,
    ) -> Option<GroupId> {
        let id = GroupId(self.next_id);
        self.next_id = self.next_id.checked_add(1)?;
        self.list.push(Group {
            id,
            name,
            colour,
            line_style,
        });
        Some(id)
    }

    /// Adds back each standard group that is missing, looking as it first did.
    pub(crate) fn restore_standard(&mut self) {
        for standard in StandardGroup::ALL {
            if self.standard(standard).is_none() {
                let (colour, style) = standard.looks();
                let name = GroupName::Standard {
                    standard,
                    renamed: None,
                };
                if self.push(name, Some(colour), style).is_none() {
                    break;
                }
            }
        }
    }

    /// Whether any standard group was deleted.
    pub(crate) fn standard_missing(&self) -> bool {
        StandardGroup::ALL
            .into_iter()
            .any(|s| self.standard(s).is_none())
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
        let name = GroupName::Named(name.to_owned());
        self.push(name, None, LineStyle::default())
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
        let Some(group) = self.list.iter_mut().find(|g| g.id == id) else {
            return;
        };
        match &mut group.name {
            GroupName::Named(name) => new.clone_into(name),
            GroupName::Standard { renamed, .. } => *renamed = Some(new.to_owned()),
        }
    }

    pub(crate) fn contains(&self, id: GroupId) -> bool {
        self.list.iter().any(|g| g.id == id)
    }

    /// As the methods here keep them: IDs unique and below the next one to give out, each
    /// standard group once, and the names the user typed unique ignoring case and never blank.
    pub(crate) fn consistent(&self) -> bool {
        let mut ids = HashSet::new();
        let mut standards = HashSet::new();
        let mut names = HashSet::new();
        self.list.iter().all(|g| {
            let name = g.name.typed().map(|n| n.trim().to_lowercase());
            let fresh_name = name.is_none_or(|n| !n.is_empty() && names.insert(n));
            let fresh_standard = g.name.standard().is_none_or(|s| standards.insert(s));
            g.id.0 < self.next_id && ids.insert(g.id) && fresh_name && fresh_standard
        })
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

    /// The group's line style; solid for a group that does not exist.
    pub(crate) fn line_style(&self, id: GroupId) -> LineStyle {
        self.list
            .iter()
            .find(|g| g.id == id)
            .map(|g| g.line_style)
            .unwrap_or_default()
    }

    pub(crate) fn set_line_style(&mut self, id: GroupId, style: LineStyle) {
        if let Some(group) = self.list.iter_mut().find(|g| g.id == id) {
            group.line_style = style;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(groups: &Groups) -> Vec<&str> {
        groups.iter().filter_map(|(_, name)| name.typed()).collect()
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
        assert_eq!(names(&groups), ["Work", "Home"]);
        assert_eq!(groups.iter().next().map(|(id, _)| id), Some(office));
    }

    #[test]
    fn the_standard_groups_each_look_their_own_and_come_back_once_deleted() {
        let mut groups = Groups::default();
        assert!(groups.standard_missing());
        groups.restore_standard();
        assert!(!groups.standard_missing());
        assert!(groups.consistent());
        let looks: HashSet<_> = groups
            .iter()
            .map(|(id, _)| (groups.colour(id), groups.line_style(id)))
            .collect();
        assert_eq!(looks.len(), StandardGroup::ALL.len());

        let wifi = groups.standard(StandardGroup::WiFi).unwrap();
        groups.rename(wifi, "Wireless");
        groups.set_colour(wifi, None);
        let printing = groups.standard(StandardGroup::Printing).unwrap();
        groups.remove(printing);
        groups.restore_standard();
        assert_eq!(groups.iter().count(), StandardGroup::ALL.len());
        assert_eq!(groups.standard(StandardGroup::WiFi), Some(wifi), "kept");
        assert_eq!(names(&groups), ["Wireless"]);
        assert_eq!(groups.colour(wifi), None, "as the user left it");
        let restored = groups.standard(StandardGroup::Printing).unwrap();
        assert_ne!(restored, printing, "a new group");
        assert_eq!(groups.colour(restored), Some(GroupColour::Yellow));
    }

    #[test]
    fn a_typed_name_never_clashes_with_a_standard_one() {
        let mut groups = Groups::default();
        groups.restore_standard();
        let servers = groups.standard(StandardGroup::Servers).unwrap();
        let added = groups.add("Servers");
        assert_ne!(added, Some(servers), "a group of its own");
        groups.rename(servers, "servers");
        assert_eq!(names(&groups), ["Servers"], "but typed names stay unique");
    }

    #[test]
    fn plain_and_standard_names_are_saved_so_both_read_back() {
        let mut groups = Groups::default();
        groups.add("Office");
        groups.restore_standard();
        let text = serde_json::to_string(&groups).unwrap();
        assert_eq!(serde_json::from_str::<Groups>(&text).unwrap(), groups);

        let older =
            r#"{"list":[{"id":0,"name":"Office","colour":null,"line_style":"Solid"}],"next_id":1}"#;
        let older: Groups = serde_json::from_str(older).unwrap();
        assert_eq!(names(&older), ["Office"]);
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
    fn a_group_starts_solid_and_keeps_the_style_it_is_given() {
        let mut groups = Groups::default();
        let office = groups.add("Office").unwrap();
        let home = groups.add("Home").unwrap();
        assert_eq!(groups.line_style(office), LineStyle::Solid);
        groups.set_line_style(office, LineStyle::Wavy);
        assert_eq!(groups.line_style(office), LineStyle::Wavy);
        assert_eq!(
            groups.line_style(home),
            LineStyle::Solid,
            "others unchanged"
        );
    }

    #[test]
    fn a_removed_group_is_gone_and_its_id_is_not_reused() {
        let mut groups = Groups::default();
        let office = groups.add("Office").unwrap();
        groups.remove(office);
        assert_eq!(groups.iter().count(), 0);
        assert_ne!(groups.add("Office"), Some(office));
    }

    #[test]
    fn an_exhausted_saved_id_counter_refuses_new_groups_without_overflowing() {
        let mut groups = Groups {
            next_id: u32::MAX,
            ..Groups::default()
        };
        assert_eq!(groups.add("Office"), None);
        groups.restore_standard();
        assert!(groups.list.is_empty());
    }
}
