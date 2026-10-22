//! Every valid TCP port a scan probes on each host, and the IANA names for open ports the
//! scanner itself does not name.

use std::collections::{BTreeSet, HashMap};

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
    pub(super) fn for_mode(mode: ScanMode) -> Self {
        match mode {
            ScanMode::Quick => Self {
                tcp: QUICK_TCP.to_vec(),
                udp: Vec::new(),
            },
            ScanMode::Full => Self {
                tcp: registry_ports(IANA_TCP),
                udp: registry_ports(IANA_UDP),
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

/// The service name IANA registers for each TCP port, such as `rtsp` for 554.
#[derive(Debug, Default)]
pub(super) struct PortNames {
    tcp: HashMap<u16, String>,
    udp: HashMap<u16, String>,
}

impl PortNames {
    /// Reads the registry; the first name listed for a port is kept. The file ships with the
    /// app and its tests check that it reads, so a fault leaves the names empty rather than
    /// stopping a scan.
    pub(super) fn load() -> Self {
        Self {
            tcp: registry_names(IANA_TCP),
            udp: registry_names(IANA_UDP),
        }
    }

    pub(super) fn get(&self, protocol: ServiceProtocol, port: u16) -> Option<&str> {
        let names = match protocol {
            ServiceProtocol::Tcp => &self.tcp,
            ServiceProtocol::Udp => &self.udp,
        };
        names.get(&port).map(String::as_str)
    }
}

fn registry_names(source: &str) -> HashMap<u16, String> {
    let registry: serde_json::Value = serde_json::from_str(source).unwrap_or_default();
    let entries = registry.get("ports").and_then(serde_json::Value::as_array);
    let mut names = HashMap::new();
    for entry in entries.into_iter().flatten() {
        let port = entry.get("port").and_then(serde_json::Value::as_u64);
        let port = port.and_then(|p| u16::try_from(p).ok());
        let name = entry.get("service").and_then(serde_json::Value::as_str);
        if let (Some(port), Some(name)) = (port, name.filter(|n| !n.is_empty())) {
            names.entry(port).or_insert_with(|| name.to_owned());
        }
    }
    names
}

fn registry_ports(source: &str) -> Vec<u16> {
    let registry: serde_json::Value = serde_json::from_str(source).unwrap_or_default();
    let entries = registry.get("ports").and_then(serde_json::Value::as_array);
    entries
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("port")?.as_u64())
        .filter_map(|port| u16::try_from(port).ok())
        .filter(|&port| port != 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_has_valid_distinct_batches() {
        for mode in [ScanMode::Quick, ScanMode::Full, ScanMode::Everything] {
            let plan = PortPlan::for_mode(mode);
            for ports in [&plan.tcp, &plan.udp] {
                assert!(ports.windows(2).all(|pair| pair[0] < pair[1]));
                for batch in ports.chunks(BATCH_SIZE) {
                    netscli_core::validate_ports(batch).unwrap();
                }
            }
        }
        let everything = PortPlan::for_mode(ScanMode::Everything);
        assert_eq!(everything.tcp.len(), usize::from(u16::MAX));
        assert_eq!(everything.udp.len(), usize::from(u16::MAX));
    }

    #[test]
    fn the_shipped_registry_reads_and_names_well_known_ports() {
        let names = PortNames::load();
        assert_eq!(names.get(ServiceProtocol::Tcp, 22), Some("ssh"));
        assert_eq!(names.get(ServiceProtocol::Udp, 53), Some("domain"));
        assert!(names.tcp.len() > 1000, "{}", names.tcp.len());
        assert!(names.udp.len() > 1000, "{}", names.udp.len());
    }
}
