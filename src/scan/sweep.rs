//! Finds the hosts on a subnet. netscli-core's sweep pings each address once and adds what the
//! neighbour table holds, read straight after the pings: a device that is asleep or drops pings
//! misses its one ping, and one still being looked up is not in the table yet, so a rescan would
//! find devices the first run missed. Here each address that stayed silent gets a second chance:
//! a TCP knock on ports common devices answer, where even a refusal shows a device is there and
//! has the system look up its MAC address; then a ping with longer to answer; then the neighbour
//! table is read again.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use ipnet::Ipv4Net;
use netscli_core::{
    DiscoverEngine, DiscoverPhase, DiscoverProgress, FoundBy, Host, NetworkManager, PingScanner,
    PortScanner, PortStatus,
};
use tokio::task::JoinSet;

use super::{CONCURRENCY, Counts, DNS_TIMEOUT_MS, PING_TIMEOUT_MS, subnets};

/// Pings in flight at once: fewer than the port probes, so a busy Wi-Fi access point or a
/// device that limits its replies drops none.
const SWEEP_CONCURRENCY: usize = 64;
const RETRY_PING_TIMEOUT_MS: u64 = 2000;
const KNOCK_TIMEOUT_MS: u64 = 1000;
/// SMB, which Windows answers while its firewall drops pings; the web ports; and the port
/// iPhones and iPads sync on.
const KNOCK_PORTS: [u16; 4] = [445, 80, 443, 62078];

/// What the neighbour table says of an address.
struct Neighbour {
    mac: String,
    /// The maker of the card, by its MAC address.
    vendor: Option<String>,
}

/// The hosts on `subnet`, this computer with its own MAC address among them, counting the
/// addresses checked into `counts`: the first pass makes up the first half of the count, the
/// second chances the rest.
pub(super) async fn hosts(
    subnet: Ipv4Net,
    counts: &Arc<Counts>,
) -> netscli_core::Result<Vec<Host>> {
    let addresses = subnet.hosts().count();
    let first_counts = Arc::clone(counts);
    let progress: Arc<dyn Fn(DiscoverProgress) + Send + Sync> = Arc::new(move |p| {
        if matches!(p.phase, DiscoverPhase::Ping) {
            first_counts.swept.store(p.completed / 2, Ordering::Relaxed);
        }
    });
    let sweep =
        DiscoverEngine::new_with_timeouts(SWEEP_CONCURRENCY, PING_TIMEOUT_MS, DNS_TIMEOUT_MS);
    let mut hosts = sweep
        .scan_subnet_with_progress(subnet, false, Some(progress))
        .await?;
    let found: HashSet<IpAddr> = hosts.iter().map(|h| h.ip).collect();
    let silent: Vec<IpAddr> = subnet
        .hosts()
        .map(IpAddr::V4)
        .filter(|ip| !found.contains(ip))
        .collect();
    let answered = second_chances(&silent, addresses, counts).await;
    let table = neighbours().await;
    let late = silent
        .iter()
        .filter_map(|&ip| late_host(ip, answered.contains(&ip), table.get(&ip)));
    hosts.extend(late);
    subnets::add_own_macs(&mut hosts, &subnets::own_macs());
    Ok(hosts)
}

/// Knocks on each of the `silent` addresses, then pings it with longer to answer, and returns
/// those that answered either way. Counts each into `counts` as the second half of the
/// `addresses`.
async fn second_chances(silent: &[IpAddr], addresses: usize, counts: &Counts) -> HashSet<IpAddr> {
    let ping = PingScanner::new(SWEEP_CONCURRENCY);
    let ports = PortScanner::new(CONCURRENCY);
    let mut pending = JoinSet::new();
    for &ip in silent {
        let (ping, ports) = (ping.clone(), ports.clone());
        pending.spawn(async move {
            let answered =
                knock(&ports, ip).await || ping.ping(ip, RETRY_PING_TIMEOUT_MS).await.alive;
            (ip, answered)
        });
    }
    let half = addresses / 2;
    let mut answered = HashSet::new();
    let mut done = 0;
    while let Some(chance) = pending.join_next().await {
        done += 1;
        let swept = half + done * (addresses - half) / silent.len();
        counts.swept.store(swept, Ordering::Relaxed);
        // A chance that could not run found nothing.
        if let Ok((ip, true)) = chance {
            answered.insert(ip);
        }
    }
    answered
}

/// Whether a TCP connection to any of the knock ports got an answer, even a refusal.
async fn knock(ports: &PortScanner, ip: IpAddr) -> bool {
    let results = ports
        .scan_host(ip, KNOCK_PORTS.to_vec(), KNOCK_TIMEOUT_MS)
        .await;
    // Only a port list it refuses fails, and these ports are all valid.
    let results = results.unwrap_or_default();
    let answer = |status| matches!(status, PortStatus::Open | PortStatus::Closed);
    results.iter().any(|result| answer(result.status))
}

/// The neighbour table by address, without multicast addresses, which are no device; empty
/// when it cannot be read. Read off the runtime's threads, as the sweep reads it.
async fn neighbours() -> HashMap<IpAddr, Neighbour> {
    let table = tokio::task::spawn_blocking(NetworkManager::get_arp_table).await;
    // A table that cannot be read adds no hosts.
    let entries = table.ok().and_then(Result::ok).unwrap_or_default();
    let unicast = entries.into_iter().filter(|entry| {
        let [first, ..] = entry.mac.bytes();
        first & 1 == 0
    });
    let neighbour = |entry: netscli_core::ArpEntry| {
        let mac = entry.mac.to_string();
        let vendor = entry.vendor.or_else(|| netscli_core::lookup_vendor(&mac));
        (entry.ip, Neighbour { mac, vendor })
    };
    unicast.map(neighbour).collect()
}

/// The host at a silent `ip` after its second chance: found if it `answered`, or if the
/// neighbour table now lists it.
fn late_host(ip: IpAddr, answered: bool, neighbour: Option<&Neighbour>) -> Option<Host> {
    let found_by = match (answered, neighbour) {
        (true, _) => FoundBy::Probe,
        (false, Some(_)) => FoundBy::Neighbor,
        (false, None) => return None,
    };
    Some(Host {
        ip,
        hostname: None,
        mac: neighbour.map(|n| n.mac.clone()),
        vendor: neighbour.and_then(|n| n.vendor.clone()),
        rtt_ms: None,
        found_by,
        hostname_source: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_address_counts_once_it_answers_or_the_neighbour_table_lists_it() {
        let ip: IpAddr = "192.168.1.30".parse().unwrap();
        let phone = Neighbour {
            mac: "3C:22:FB:01:02:03".into(),
            vendor: Some("Apple, Inc.".into()),
        };
        assert!(late_host(ip, false, None).is_none(), "nothing heard");

        let knocked = late_host(ip, true, None).unwrap();
        assert_eq!((knocked.found_by, &knocked.mac), (FoundBy::Probe, &None));

        let listed = late_host(ip, false, Some(&phone)).unwrap();
        assert_eq!(listed.found_by, FoundBy::Neighbor);
        assert_eq!(listed.mac.as_deref(), Some("3C:22:FB:01:02:03"));
        assert_eq!(listed.vendor.as_deref(), Some("Apple, Inc."));

        let both = late_host(ip, true, Some(&phone)).unwrap();
        assert_eq!(both.found_by, FoundBy::Probe);
        assert_eq!(both.mac.as_deref(), Some("3C:22:FB:01:02:03"));
    }
}
