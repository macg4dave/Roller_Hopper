//! Scans a subnet for real, off the UI thread: netscli-core's ping sweep, which adds the hosts
//! in the operating system's neighbour table and names from mDNS, then TCP connect probes of
//! the common ports and a reverse DNS lookup for each host found. Each host comes back as a
//! model [`Answer`] through a channel the UI drains; dropping the [`Scan`] stops it.
//!
//! Only probes that need no capture driver, no elevation and no other program are used: the
//! sweep runs with its own name lookup off (on Windows that runs `ping -a`), and names come from
//! the in-process resolver instead.

mod answer;
mod subnets;

pub(crate) use subnets::local_subnets;

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryIter};

use ipnet::Ipv4Net;
use netscli_core::{DiscoverEngine, DiscoverPhase, DiscoverProgress, Host, PortScanner};
use tokio::runtime::{Builder, Runtime};
use tokio::task::{JoinHandle, JoinSet};

use crate::model::{Answer, ScanOptions};
use answer::Probed;

/// Probes in flight at once, in the sweep and across the port probes of all hosts.
const CONCURRENCY: usize = 128;
const PING_TIMEOUT_MS: u64 = 1000;
const PORT_TIMEOUT_MS: u64 = 1000;
const DNS_TIMEOUT_MS: u64 = 1500;

/// The TCP ports probed on each host found: file transfer and sharing, remote access, mail,
/// DNS, web pages, printing, cameras, databases, home automation and the consoles of common
/// NAS, hypervisor and media servers.
const COMMON_TCP_PORTS: &[u16] = &[
    21, 22, 23, 25, 53, 80, 81, 139, 443, 445, 515, 548, 554, 631, 1883, 3306, 3389, 5000, 5001,
    5900, 8000, 8006, 8080, 8123, 8443, 8883, 9000, 9100, 9443, 32400, 62078,
];

/// What a running scan reports, besides how many addresses it has checked.
#[derive(Debug)]
pub(crate) enum Event {
    Host(Box<Answer>),
    /// Every address is checked and every host found is probed; nothing follows.
    Finished,
    /// The scan could not go on, with the scanner's or system's message; nothing follows.
    Failed(String),
}

/// A scan running on its own threads.
pub(crate) struct Scan {
    /// Taken only when the scan is dropped.
    runtime: Option<Runtime>,
    task: JoinHandle<()>,
    checked: Arc<AtomicUsize>,
    events: Receiver<Event>,
}

impl Scan {
    /// Starts scanning `subnet` with `options`. Fails only if the threads cannot be started.
    pub(crate) fn start(subnet: Ipv4Net, options: ScanOptions) -> io::Result<Self> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("scan")
            .enable_all()
            .build()?;
        let checked = Arc::new(AtomicUsize::new(0));
        let (sender, events) = mpsc::channel();
        let task = runtime.spawn(run(subnet, options, Arc::clone(&checked), sender));
        Ok(Self {
            runtime: Some(runtime),
            task,
            checked,
            events,
        })
    }

    /// Addresses the sweep has checked so far.
    pub(crate) fn checked(&self) -> usize {
        self.checked.load(Ordering::Relaxed)
    }

    /// What the scan has reported since last asked, without waiting.
    pub(crate) fn events(&self) -> TryIter<'_, Event> {
        self.events.try_iter()
    }
}

impl Drop for Scan {
    /// Stops the scan where it got to. Probes already waiting on the system finish on their own
    /// timeouts in the background; nothing waits for them.
    fn drop(&mut self) {
        self.task.abort();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

async fn run(
    subnet: Ipv4Net,
    options: ScanOptions,
    checked: Arc<AtomicUsize>,
    events: Sender<Event>,
) {
    let progress: Arc<dyn Fn(DiscoverProgress) + Send + Sync> = Arc::new(move |p| {
        if matches!(p.phase, DiscoverPhase::Ping) {
            checked.store(p.completed, Ordering::Relaxed);
        }
    });
    let sweep = DiscoverEngine::new_with_timeouts(CONCURRENCY, PING_TIMEOUT_MS, DNS_TIMEOUT_MS);
    let end = match sweep
        .scan_subnet_with_progress(subnet, false, Some(progress))
        .await
    {
        Ok(hosts) => probe_all(hosts, subnet, options, &events).await,
        Err(error) => Event::Failed(error.to_string()),
    };
    // Fails only once the UI has dropped the receiver, when no one is left to tell.
    events.send(end).ok();
}

/// Probes every host the sweep found, reporting each as soon as it is done, and says how the
/// scan ended. A probe that fails ends it: the hosts still being probed would go unreported.
async fn probe_all(
    hosts: Vec<Host>,
    subnet: Ipv4Net,
    options: ScanOptions,
    events: &Sender<Event>,
) -> Event {
    let scanner = Arc::new(PortScanner::new(CONCURRENCY));
    let mut pending = JoinSet::new();
    for host in hosts {
        pending.spawn(probe(host, Arc::clone(&scanner), options));
    }
    while let Some(done) = pending.join_next().await {
        let probed = match done {
            Ok(probed) => probed,
            Err(error) => return Event::Failed(error.to_string()),
        };
        let Some(answer) = answer::answer(probed, subnet, options.service_detection) else {
            continue;
        };
        if events.send(Event::Host(Box::new(answer))).is_err() {
            // The UI has gone; the rest would go unread.
            break;
        }
    }
    Event::Finished
}

/// Finds out what `options` allow about one host beyond what the sweep did.
async fn probe(host: Host, scanner: Arc<PortScanner>, options: ScanOptions) -> Probed {
    let ports = if options.port_scan {
        // Refused only for an invalid port list, and this one is fixed and valid (see the
        // tests), so an error here means no open ports were found.
        let ports = COMMON_TCP_PORTS.to_vec();
        let results = scanner.scan_host(host.ip, ports, PORT_TIMEOUT_MS).await;
        results.unwrap_or_default()
    } else {
        Vec::new()
    };
    let reverse_name = if options.service_detection && host.hostname.is_none() {
        // A failed or timed-out lookup means the network has no name for it.
        let name = netscli_core::dns::reverse_lookup_timeout(host.ip, DNS_TIMEOUT_MS).await;
        name.ok().flatten()
    } else {
        None
    };
    Probed {
        host,
        reverse_name,
        ports,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_common_ports_are_a_valid_port_list_with_each_port_once() {
        netscli_core::validate_ports(COMMON_TCP_PORTS).unwrap();
        assert!(COMMON_TCP_PORTS.windows(2).all(|w| w[0] < w[1]));
    }
}
