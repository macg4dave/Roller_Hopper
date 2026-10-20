//! A device's open ports, each with a short description the user can edit.

use std::fmt;

/// An open port and what runs on it, such as a protocol name; the description may be empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Service {
    pub(crate) port: u16,
    pub(crate) description: String,
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.description.is_empty() {
            write!(f, "{}", self.port)
        } else {
            write!(f, "{} ({})", self.port, self.description)
        }
    }
}

/// A device's open ports, kept in port order with each port listed once.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Services(Vec<Service>);

impl Services {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Service> {
        self.0.iter()
    }

    fn position(&self, port: u16) -> Result<usize, usize> {
        self.0.binary_search_by_key(&port, |s| s.port)
    }

    /// Lists a port in its place by number. Refused, returning false, if it is already listed.
    pub(crate) fn add(&mut self, port: u16, description: &str) -> bool {
        let Err(at) = self.position(port) else {
            return false;
        };
        let description = description.trim().to_owned();
        self.0.insert(at, Service { port, description });
        true
    }

    /// Changes the number of the port at `index`, unless another entry already has it.
    pub(crate) fn set_port(&mut self, index: usize, port: u16) {
        if self.position(port).is_ok() {
            return;
        }
        if let Some(service) = self.0.get_mut(index) {
            service.port = port;
            self.0.sort_by_key(|s| s.port);
        }
    }

    pub(crate) fn set_description(&mut self, index: usize, description: &str) {
        if let Some(service) = self.0.get_mut(index) {
            description.trim().clone_into(&mut service.description);
        }
    }

    pub(crate) fn remove(&mut self, index: usize) {
        if index < self.0.len() {
            self.0.remove(index);
        }
    }
}

/// The ports as one line, such as `22 (SSH), 80 (HTTP)`.
impl fmt::Display for Services {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let all: Vec<String> = self.iter().map(ToString::to_string).collect();
        f.write_str(&all.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn services() -> Services {
        let mut services = Services::default();
        services.add(443, "HTTPS");
        services.add(22, " SSH ");
        services.add(80, "");
        services
    }

    #[test]
    fn ports_stay_in_order_and_are_listed_once() {
        let mut services = services();
        assert_eq!(services.to_string(), "22 (SSH), 80, 443 (HTTPS)");
        assert!(!services.add(80, "HTTP"), "already listed");
        assert_eq!(services.to_string(), "22 (SSH), 80, 443 (HTTPS)");
    }

    #[test]
    fn a_changed_port_moves_into_place_unless_taken() {
        let mut services = services();
        services.set_port(0, 8080);
        assert_eq!(services.to_string(), "80, 443 (HTTPS), 8080 (SSH)");
        services.set_port(0, 443);
        assert_eq!(services.to_string(), "80, 443 (HTTPS), 8080 (SSH)");
    }

    #[test]
    fn descriptions_can_be_edited_and_ports_removed() {
        let mut services = services();
        services.set_description(1, " HTTP ");
        services.remove(0);
        services.remove(99);
        services.set_description(99, "ignored");
        assert_eq!(services.to_string(), "80 (HTTP), 443 (HTTPS)");
    }
}
