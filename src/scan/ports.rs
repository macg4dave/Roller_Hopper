//! The TCP ports a port scan probes on each host (the Simple list, or the Deep one), and the
//! names IANA registers for TCP ports, for open ports the scanner itself does not name.

use std::collections::HashMap;

use crate::model::PortList;

/// The IANA service name registry's TCP entries: `{"ports": [{"port", "service", …}]}`.
const IANA_TCP: &str = include_str!("../data_lists/TCP_UDP_Ports/tcp_ports.json");

/// The service name IANA registers for each TCP port, such as `rtsp` for 554.
#[derive(Debug, Default)]
pub(super) struct PortNames(HashMap<u16, String>);

impl PortNames {
    /// Reads the registry; the first name listed for a port is kept. The file ships with the
    /// app and its tests check that it reads, so a fault leaves the names empty rather than
    /// stopping a scan.
    pub(super) fn load() -> Self {
        let registry: serde_json::Value = serde_json::from_str(IANA_TCP).unwrap_or_default();
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
        Self(names)
    }

    pub(super) fn get(&self, port: u16) -> Option<&str> {
        self.0.get(&port).map(String::as_str)
    }
}

/// File transfer and sharing, remote access, mail, DNS, web pages, printing, cameras,
/// databases, home automation and the consoles of common NAS, hypervisor and media servers.
const SIMPLE: &[u16] = &[
    21, 22, 23, 25, 53, 80, 81, 139, 443, 445, 515, 548, 554, 631, 1883, 3306, 3389, 5000, 5001,
    5900, 8000, 8006, 8080, 8123, 8443, 8883, 9000, 9100, 9443, 32400, 62078,
];

/// The Simple list, and also: mail and directory services, Windows RPC and remote management,
/// more databases and caches, NFS and iSCSI, VPN and proxy servers, Docker and Kubernetes,
/// casting and TV control (Chromecast, Roku, Samsung, Apple TV), Sonos, more cameras, router and
/// access point consoles (Winbox, Ubiquiti), media and download servers, and universal plug and
/// play.
const DEEP: &[u16] = &[
    21, 22, 23, 25, 53, 80, 81, 88, 110, 111, 135, 139, 143, 389, 443, 445, 465, 515, 548, 554,
    587, 631, 636, 873, 993, 995, 1080, 1400, 1433, 1521, 1723, 1883, 2049, 2375, 2376, 3000, 3001,
    3128, 3260, 3306, 3389, 5000, 5001, 5060, 5357, 5432, 5555, 5601, 5900, 5985, 5986, 6379, 6443,
    7000, 7100, 8000, 8001, 8006, 8008, 8009, 8060, 8080, 8081, 8086, 8088, 8123, 8181, 8200, 8291,
    8443, 8554, 8880, 8883, 8888, 9000, 9080, 9090, 9091, 9100, 9200, 9443, 9999, 10000, 10001,
    11211, 27017, 32400, 37777, 49152, 62078,
];

pub(super) fn ports(list: PortList) -> &'static [u16] {
    match list {
        PortList::Simple => SIMPLE,
        PortList::Deep => DEEP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_lists_are_valid_with_each_port_once_and_deep_takes_in_simple() {
        for list in [SIMPLE, DEEP] {
            netscli_core::validate_ports(list).unwrap();
            assert!(list.windows(2).all(|w| w[0] < w[1]), "sorted, each once");
        }
        assert!(SIMPLE.iter().all(|port| DEEP.contains(port)));
        assert!(DEEP.len() > 2 * SIMPLE.len());
    }

    #[test]
    fn the_shipped_registry_reads_and_names_well_known_ports() {
        let names = PortNames::load();
        assert_eq!(names.get(22), Some("ssh"));
        assert_eq!(names.get(554), Some("rtsp"));
        assert!(names.0.len() > 1000, "{}", names.0.len());
    }
}
