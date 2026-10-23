//! Every valid TCP port a scan probes on each host, and the IANA names for open ports the
//! scanner itself does not name.

use std::collections::HashMap;

use crate::model::{ScanMode, ServiceProtocol};

/// The most ports netscli-core accepts in one call.
pub(super) const BATCH_SIZE: usize = 4_096;

/// The IANA service name registry's TCP entries: `{"ports": [{"port", "service", …}]}`.
const IANA_TCP: &str = include_str!("../data_lists/TCP_UDP_Ports/tcp_ports.json");
const IANA_UDP: &str = include_str!("../data_lists/TCP_UDP_Ports/udp_ports.json");

const QUICK_TCP: &[u16] = &[
    21, 22, 23, 25, 53, 80, 81, 88, 110, 111, 135, 139, 143, 389, 443, 445, 465, 515, 548, 554,
    587, 631, 636, 873, 993, 995, 1080, 1400, 1433, 1521, 1723, 1883, 2049, 2375, 2376, 3000, 3001,
    3128, 3260, 3306, 3389, 5000, 5001, 5060, 5357, 5432, 5555, 5601, 5900, 5985, 5986, 6379, 6443,
    7000, 7100, 8000, 8001, 8006, 8008, 8009, 8060, 8080, 8081, 8086, 8088, 8123, 8181, 8200, 8291,
    8443, 8554, 8880, 8883, 8888, 9000, 9080, 9090, 9091, 9100, 9200, 9443, 9999, 10000, 10001,
    11211, 27017, 32400, 37777, 49152, 62078,
];

pub(super) struct PortPlan {
    pub(super) tcp: Vec<u16>,
    pub(super) udp: Vec<u16>,
}

impl PortPlan {
    pub(super) fn for_mode(mode: ScanMode, registry: &PortRegistry) -> Self {
        match mode {
            ScanMode::Quick => Self {
                tcp: QUICK_TCP.to_vec(),
                udp: Vec::new(),
            },
            ScanMode::Full => Self {
                tcp: registry.tcp.ports.clone(),
                udp: registry.udp.ports.clone(),
            },
            ScanMode::Everything => {
                let all = || (1..=u16::MAX).collect();
                Self {
                    tcp: all(),
                    udp: all(),
                }
            }
        }
    }

    /// How many probes each host gets, over both transports.
    pub(super) fn probes(&self) -> usize {
        self.tcp.len() + self.udp.len()
    }
}

/// The valid ports and first service name in both shipped IANA registries. Each JSON source is
/// parsed once per scan and serves both planning and naming.
#[derive(Debug, Default)]
pub(super) struct PortRegistry {
    tcp: RegistryTransport,
    udp: RegistryTransport,
}

#[derive(Debug, Default)]
struct RegistryTransport {
    ports: Vec<u16>,
    names: HashMap<u16, String>,
}

impl PortRegistry {
    /// Reads each shipped registry once. Tests verify the embedded sources, while malformed
    /// application data degrades to empty plans and names rather than crashing a scan.
    pub(super) fn load() -> Self {
        Self {
            tcp: RegistryTransport::load(IANA_TCP),
            udp: RegistryTransport::load(IANA_UDP),
        }
    }

    pub(super) fn get(&self, protocol: ServiceProtocol, port: u16) -> Option<&str> {
        let names = match protocol {
            ServiceProtocol::Tcp => &self.tcp.names,
            ServiceProtocol::Udp => &self.udp.names,
        };
        names.get(&port).map(String::as_str)
    }
}

impl RegistryTransport {
    fn load(source: &str) -> Self {
        let registry: serde_json::Value = serde_json::from_str(source).unwrap_or_default();
        let entries = registry.get("ports").and_then(serde_json::Value::as_array);
        let mut names = HashMap::new();
        let mut ports = Vec::new();
        for entry in entries.into_iter().flatten() {
            let port = entry
                .get("port")
                .and_then(serde_json::Value::as_u64)
                .and_then(|port| u16::try_from(port).ok())
                .filter(|&port| port != 0);
            let name = entry.get("service").and_then(serde_json::Value::as_str);
            if let Some(port) = port {
                ports.push(port);
                if let Some(name) = name.filter(|name| !name.is_empty()) {
                    names.entry(port).or_insert_with(|| name.to_owned());
                }
            }
        }
        ports.sort_unstable();
        ports.dedup();
        Self { ports, names }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_has_valid_distinct_batches() {
        for mode in [ScanMode::Quick, ScanMode::Full, ScanMode::Everything] {
            let registry = PortRegistry::load();
            let plan = PortPlan::for_mode(mode, &registry);
            for ports in [&plan.tcp, &plan.udp] {
                assert!(ports.windows(2).all(|pair| pair[0] < pair[1]));
                for batch in ports.chunks(BATCH_SIZE) {
                    netscli_core::validate_ports(batch).unwrap();
                }
            }
        }
        let everything = PortPlan::for_mode(ScanMode::Everything, &PortRegistry::load());
        assert_eq!(everything.tcp.len(), usize::from(u16::MAX));
        assert_eq!(everything.udp.len(), usize::from(u16::MAX));
    }

    #[test]
    fn the_shipped_registry_reads_and_names_well_known_ports() {
        let names = PortRegistry::load();
        assert_eq!(names.get(ServiceProtocol::Tcp, 22), Some("ssh"));
        assert_eq!(names.get(ServiceProtocol::Udp, 53), Some("domain"));
        assert!(names.tcp.names.len() > 1000, "{}", names.tcp.names.len());
        assert!(names.udp.names.len() > 1000, "{}", names.udp.names.len());
    }
}
