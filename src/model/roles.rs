//! The roles on offer: the standard ones, which the user may rename, re-icon or delete, followed
//! by the ones the user added.
//!
//! Standard role names are translated in the UI, so clashes with those are checked there; this
//! list only keeps the names the user typed unique among themselves, ignoring case.

use super::device::{DeviceKind, Role};
use super::groups::GroupId;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    role: Role,
    /// The user's name for a standard role, replacing its translated one. Unused for custom
    /// roles, whose name is part of the role itself.
    renamed: Option<String>,
    /// `None` keeps each device's detected icon.
    icon: Option<DeviceKind>,
    group: Option<GroupId>,
}

impl Entry {
    fn standard(role: Role) -> Self {
        let icon = role.default_icon();
        Self {
            role,
            renamed: None,
            icon,
            group: None,
        }
    }

    /// What restoring the standard roles resets: the group is the user's sorting, not part of
    /// the role.
    fn definition(&self) -> (&Role, Option<&str>, Option<DeviceKind>) {
        (&self.role, self.renamed.as_deref(), self.icon)
    }

    /// The name the user typed, if any; `None` means the UI shows the translated name.
    fn name(&self) -> Option<&str> {
        if let Role::Custom(name) = &self.role {
            Some(name)
        } else {
            self.renamed.as_deref()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Roles(Vec<Entry>);

impl Default for Roles {
    fn default() -> Self {
        Self(Role::STANDARD.into_iter().map(Entry::standard).collect())
    }
}

impl Roles {
    /// Standard roles first, then custom ones in the order they were added.
    pub(crate) fn roles(&self) -> impl Iterator<Item = &Role> {
        self.0.iter().map(|e| &e.role)
    }

    fn entry(&self, role: &Role) -> Option<&Entry> {
        self.0.iter().find(|e| e.role == *role)
    }

    fn entry_mut(&mut self, role: &Role) -> Option<&mut Entry> {
        self.0.iter_mut().find(|e| e.role == *role)
    }

    pub(crate) fn name(&self, role: &Role) -> Option<&str> {
        self.entry(role).and_then(Entry::name)
    }

    pub(crate) fn icon(&self, role: &Role) -> Option<DeviceKind> {
        self.entry(role).and_then(|e| e.icon)
    }

    fn named_ignoring_case(&self, name: &str) -> Option<&Entry> {
        let lower = name.to_lowercase();
        self.0
            .iter()
            .find(|e| e.name().is_some_and(|n| n.to_lowercase() == lower))
    }

    /// Adds a role shown with `icon`, and returns it. An existing role with the same name is
    /// returned, with its own icon, instead of a duplicate; a blank name is rejected.
    pub(crate) fn add(&mut self, name: &str, icon: DeviceKind) -> Option<Role> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        if let Some(existing) = self.named_ignoring_case(name) {
            return Some(existing.role.clone());
        }
        let role = Role::Custom(name.to_owned());
        self.0.push(Entry {
            role: role.clone(),
            renamed: None,
            icon: Some(icon),
            group: None,
        });
        Some(role)
    }

    /// Renames `role` and returns it as it is now identified: a custom role's name is part of
    /// it, a standard role stays the same role. A blank name, or one another role already has,
    /// is rejected.
    pub(crate) fn rename(&mut self, role: &Role, new: &str) -> Option<Role> {
        let new = new.trim();
        let taken = self
            .named_ignoring_case(new)
            .is_some_and(|other| other.role != *role);
        if new.is_empty() || taken {
            return None;
        }
        let entry = self.entry_mut(role)?;
        if let Role::Custom(name) = &mut entry.role {
            new.clone_into(name);
        } else {
            entry.renamed = Some(new.to_owned());
        }
        Some(entry.role.clone())
    }

    pub(crate) fn set_icon(&mut self, role: &Role, icon: Option<DeviceKind>) {
        if let Some(entry) = self.entry_mut(role) {
            entry.icon = icon;
        }
    }

    pub(crate) fn group(&self, role: &Role) -> Option<GroupId> {
        self.entry(role).and_then(|e| e.group)
    }

    pub(crate) fn set_group(&mut self, role: &Role, group: Option<GroupId>) {
        if let Some(entry) = self.entry_mut(role) {
            entry.group = group;
        }
    }

    /// Takes every role out of `group`, which is being deleted.
    pub(crate) fn leave_group(&mut self, group: GroupId) {
        for entry in self.0.iter_mut().filter(|e| e.group == Some(group)) {
            entry.group = None;
        }
    }

    /// Removes the role; returns whether it was removed. Other stays, as the role every device
    /// falls back to.
    pub(crate) fn remove(&mut self, role: &Role) -> bool {
        let before = self.0.len();
        self.0.retain(|e| e.role != *role || e.role == Role::Other);
        self.0.len() != before
    }

    /// Whether any standard role was renamed, re-iconed or deleted.
    pub(crate) fn standard_changed(&self) -> bool {
        let standard = self.0.iter().filter(|e| !is_custom(&e.role));
        let default = Self::default();
        !standard
            .map(Entry::definition)
            .eq(default.0.iter().map(Entry::definition))
    }

    /// Brings back the standard roles as they first were; custom roles, and the groups roles
    /// are in, are kept.
    pub(crate) fn restore_standard(&mut self) {
        let mut restored = Self::default();
        for entry in &mut restored.0 {
            entry.group = self.group(&entry.role);
        }
        restored
            .0
            .extend(self.0.drain(..).filter(|e| is_custom(&e.role)));
        *self = restored;
    }
}

fn is_custom(role: &Role) -> bool {
    matches!(role, Role::Custom(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tv() -> Role {
        Role::Custom("TV".into())
    }

    fn roles() -> Roles {
        let mut roles = Roles::default();
        roles.add("TV", DeviceKind::Tv);
        roles.add("Media", DeviceKind::Server);
        roles
    }

    #[test]
    fn names_stay_unique_ignoring_case() {
        let mut roles = roles();
        let media = Role::Custom("Media".into());
        assert_eq!(roles.add(" media ", DeviceKind::Phone), Some(media.clone()));
        assert_eq!(roles.icon(&media), Some(DeviceKind::Server), "icon kept");
        assert_eq!(roles.add("  ", DeviceKind::Phone), None);
        roles.rename(&Role::Nas, "Storage");
        assert_eq!(roles.add("storage", DeviceKind::Phone), Some(Role::Nas));
        assert_eq!(roles.roles().count(), Role::STANDARD.len() + 2);
    }

    #[test]
    fn renaming_rejects_blank_and_taken_names() {
        let mut roles = roles();
        let media = Role::Custom("Media".into());
        assert_eq!(roles.rename(&media, "   "), None);
        assert_eq!(roles.rename(&media, "tv"), None);
        assert_eq!(
            roles.rename(&Role::Custom("Missing".into()), "Other name"),
            None
        );
        let upper = Role::Custom("MEDIA".into());
        assert_eq!(roles.rename(&media, " MEDIA "), Some(upper.clone()));
        let streaming = Role::Custom("Streaming".into());
        assert_eq!(roles.rename(&upper, "Streaming"), Some(streaming.clone()));
        assert_eq!(roles.icon(&streaming), Some(DeviceKind::Server));
    }

    #[test]
    fn a_standard_role_keeps_its_identity_when_renamed() {
        let mut roles = roles();
        assert_eq!(roles.name(&Role::Router), None, "translated by the UI");
        assert_eq!(roles.rename(&Role::Router, " Gateway "), Some(Role::Router));
        assert_eq!(roles.name(&Role::Router), Some("Gateway"));
        assert_eq!(roles.rename(&tv(), "gateway"), None, "taken");
    }

    #[test]
    fn icons_can_change_or_follow_the_device() {
        let mut roles = roles();
        roles.set_icon(&tv(), Some(DeviceKind::Console));
        assert_eq!(roles.icon(&tv()), Some(DeviceKind::Console));
        roles.set_icon(&Role::Printer, None);
        assert_eq!(roles.icon(&Role::Printer), None);
        roles.set_icon(&Role::Other, Some(DeviceKind::Unknown));
        assert_eq!(roles.icon(&Role::Other), Some(DeviceKind::Unknown));
    }

    #[test]
    fn any_role_but_other_can_be_removed() {
        let mut roles = roles();
        assert!(roles.remove(&tv()));
        assert!(!roles.remove(&tv()));
        assert_eq!(roles.icon(&tv()), None);
        assert!(roles.remove(&Role::Camera));
        assert!(!roles.remove(&Role::Other));
        assert!(roles.roles().any(|r| *r == Role::Other));
    }

    #[test]
    fn restoring_resets_standard_roles_and_keeps_custom_ones() {
        let mut roles = roles();
        assert!(!roles.standard_changed());
        roles.remove(&Role::Camera);
        roles.rename(&Role::Router, "Gateway");
        roles.set_icon(&Role::Phone, None);
        roles.set_icon(&tv(), Some(DeviceKind::Console));
        assert!(roles.standard_changed());

        roles.restore_standard();
        assert!(!roles.standard_changed());
        let order: Vec<&Role> = roles.roles().collect();
        assert_eq!(order.get(9), Some(&&Role::Camera), "back in its place");
        assert_eq!(roles.name(&Role::Router), None);
        assert_eq!(roles.icon(&Role::Phone), Some(DeviceKind::Phone));
        assert_eq!(roles.icon(&tv()), Some(DeviceKind::Console), "custom kept");
    }

    #[test]
    fn groups_survive_restoring_and_are_left_when_deleted() {
        let mut groups = super::super::groups::Groups::default();
        let office = groups.add("Office").unwrap();
        let mut roles = roles();
        roles.set_group(&Role::Printer, Some(office));
        roles.set_group(&tv(), Some(office));
        assert!(
            !roles.standard_changed(),
            "a group is not a change to the role"
        );

        roles.rename(&Role::Printer, "Office printer");
        roles.restore_standard();
        assert_eq!(roles.group(&Role::Printer), Some(office));

        roles.leave_group(office);
        assert_eq!(roles.group(&Role::Printer), None);
        assert_eq!(roles.group(&tv()), None);
    }
}
