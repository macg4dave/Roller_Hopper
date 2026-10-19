//! The roles the user added, each with the icon picked for it.
//!
//! Standard role names are translated in the UI, so clashes with those are checked there; this
//! list only keeps custom names unique among themselves, ignoring case.

use super::device::{DeviceKind, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CustomRole {
    name: String,
    icon: DeviceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CustomRoles(Vec<CustomRole>);

impl CustomRoles {
    /// In the order they were added.
    pub(crate) fn roles(&self) -> impl Iterator<Item = Role> + '_ {
        self.0.iter().map(|r| Role::Custom(r.name.clone()))
    }

    pub(crate) fn icon(&self, name: &str) -> Option<DeviceKind> {
        self.0.iter().find(|r| r.name == name).map(|r| r.icon)
    }

    fn find_ignoring_case(&self, name: &str) -> Option<&CustomRole> {
        let lower = name.to_lowercase();
        self.0.iter().find(|r| r.name.to_lowercase() == lower)
    }

    /// Adds a role shown with `icon`, and returns it. An existing role with the same name is
    /// returned, with its own icon, instead of a duplicate; a blank name is rejected.
    pub(crate) fn add(&mut self, name: &str, icon: DeviceKind) -> Option<Role> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        if let Some(existing) = self.find_ignoring_case(name) {
            return Some(Role::Custom(existing.name.clone()));
        }
        self.0.push(CustomRole {
            name: name.to_owned(),
            icon,
        });
        Some(Role::Custom(name.to_owned()))
    }

    /// Renames the role `old` and returns its new name. A blank name, or one another role
    /// already has, is rejected.
    pub(crate) fn rename(&mut self, old: &str, new: &str) -> Option<String> {
        let new = new.trim();
        let taken = self
            .find_ignoring_case(new)
            .is_some_and(|other| other.name != old);
        if new.is_empty() || taken {
            return None;
        }
        let role = self.0.iter_mut().find(|r| r.name == old)?;
        new.clone_into(&mut role.name);
        Some(role.name.clone())
    }

    pub(crate) fn set_icon(&mut self, name: &str, icon: DeviceKind) {
        if let Some(role) = self.0.iter_mut().find(|r| r.name == name) {
            role.icon = icon;
        }
    }

    /// Removes the role; returns whether it existed.
    pub(crate) fn remove(&mut self, name: &str) -> bool {
        let before = self.0.len();
        self.0.retain(|r| r.name != name);
        self.0.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles() -> CustomRoles {
        let mut roles = CustomRoles::default();
        roles.add("TV", DeviceKind::Tv);
        roles.add("Media", DeviceKind::Server);
        roles
    }

    #[test]
    fn names_stay_unique_ignoring_case() {
        let mut roles = roles();
        assert_eq!(
            roles.add(" media ", DeviceKind::Phone),
            Some(Role::Custom("Media".into()))
        );
        assert_eq!(roles.icon("Media"), Some(DeviceKind::Server), "icon kept");
        assert_eq!(roles.add("  ", DeviceKind::Phone), None);
        assert_eq!(roles.roles().count(), 2);
    }

    #[test]
    fn renaming_rejects_blank_and_taken_names() {
        let mut roles = roles();
        assert_eq!(roles.rename("Media", "   "), None);
        assert_eq!(roles.rename("Media", "tv"), None);
        assert_eq!(roles.rename("Missing", "Other name"), None);
        assert_eq!(roles.rename("Media", " MEDIA "), Some("MEDIA".into()));
        assert_eq!(roles.rename("MEDIA", "Streaming"), Some("Streaming".into()));
        assert_eq!(roles.icon("Streaming"), Some(DeviceKind::Server));
    }

    #[test]
    fn icons_can_change_and_roles_can_be_removed() {
        let mut roles = roles();
        roles.set_icon("TV", DeviceKind::Console);
        assert_eq!(roles.icon("TV"), Some(DeviceKind::Console));
        assert!(roles.remove("TV"));
        assert!(!roles.remove("TV"));
        assert_eq!(roles.icon("TV"), None);
    }
}
