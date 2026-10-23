//! A device's open ports, each with a short description the user can edit.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum ServiceProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ServiceSource {
    #[default]
    Manual,
    Detected,
    Registry,
    Open,
}

/// An open port and what runs on it, such as a protocol name; the description may be empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Service {
    pub(crate) port: u16,
    pub(crate) protocol: ServiceProtocol,
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) source: ServiceSource,
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.description.is_empty() {
            f.write_str(&port_label(self.port, self.protocol))
        } else {
            write!(
                f,
                "{} ({})",
                port_label(self.port, self.protocol),
                self.description
            )
        }
    }
}

/// A device's open ports, kept in port order with each port listed once. Read from a file, they
/// are put in that order, and a port listed again is dropped.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "Vec<Service>")]
pub(crate) struct Services(Vec<Service>);

impl From<Vec<Service>> for Services {
    fn from(mut services: Vec<Service>) -> Self {
        services.sort_by_key(|s| (s.port, s.protocol));
        services.dedup_by_key(|s| (s.port, s.protocol));
        Self(services)
    }
}

impl Services {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Service> {
        self.0.iter()
    }

    fn position(&self, port: u16, protocol: ServiceProtocol) -> Result<usize, usize> {
        self.0.binary_search_by_key(&(port, protocol), |service| {
            (service.port, service.protocol)
        })
    }

    /// Lists a port in its place by number. Refused, returning false, if it is already listed.
    pub(crate) fn add(&mut self, port: u16, description: &str) -> bool {
        self.add_protocol(port, ServiceProtocol::Tcp, description)
    }

    pub(crate) fn add_protocol(
        &mut self,
        port: u16,
        protocol: ServiceProtocol,
        description: &str,
    ) -> bool {
        self.add_sourced(port, protocol, description, ServiceSource::Manual)
    }

    pub(crate) fn add_sourced(
        &mut self,
        port: u16,
        protocol: ServiceProtocol,
        description: &str,
        source: ServiceSource,
    ) -> bool {
        let Err(at) = self.position(port, protocol) else {
            return false;
        };
        let description = description.trim().to_owned();
        self.0.insert(
            at,
            Service {
                port,
                protocol,
                description,
                source,
            },
        );
        true
    }

    /// Changes the number of the port at `index`, unless another entry already has it.
    pub(crate) fn set_port(&mut self, index: usize, port: u16) {
        let Some(protocol) = self.0.get(index).map(|service| service.protocol) else {
            return;
        };
        if self.position(port, protocol).is_ok() {
            return;
        }
        if let Some(service) = self.0.get_mut(index) {
            service.port = port;
            service.source = ServiceSource::Manual;
            self.0.sort_by_key(|s| (s.port, s.protocol));
        }
    }

    pub(crate) fn set_description(&mut self, index: usize, description: &str) {
        if let Some(service) = self.0.get_mut(index) {
            description.trim().clone_into(&mut service.description);
            service.source = ServiceSource::Manual;
        }
    }

    pub(crate) fn remove(&mut self, index: usize) {
        if index < self.0.len() {
            self.0.remove(index);
        }
    }
}

fn port_label(port: u16, protocol: ServiceProtocol) -> String {
    match protocol {
        ServiceProtocol::Tcp => port.to_string(),
        ServiceProtocol::Udp => format!("{port}/udp"),
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
        assert!(
            services
                .iter()
                .all(|service| service.source == ServiceSource::Manual)
        );
    }

    #[test]
    fn scanned_provenance_survives_loading_but_an_edit_becomes_manual() {
        let mut services = Services::default();
        services.add_sourced(22, ServiceProtocol::Tcp, "OpenSSH", ServiceSource::Detected);
        let json = serde_json::to_string(&services).unwrap();
        let mut loaded: Services = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded.iter().next().unwrap().source,
            ServiceSource::Detected
        );
        loaded.set_description(0, "SSH");
        assert_eq!(loaded.iter().next().unwrap().source, ServiceSource::Manual);
    }
}
