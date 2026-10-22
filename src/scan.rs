//! Scans a subnet for real, off the UI thread: netscli-core's ping sweep, which adds the hosts
//! in the operating system's neighbour table and names from mDNS, with a browse of the services
//! hosts announce over mDNS alongside; then, for each host found, TCP connect probes of the
//! chosen ports, a reverse DNS lookup and a ping for its time-to-live, as the options ask.
//! Each host comes back as a model [`Answer`] through a channel the UI drains; dropping the
//! [`Scan`] stops it.
//!
//! Only probes that need no capture driver, no elevation and no other program are used: the
//! sweep runs with its own name lookup off (on Windows that runs `ping -a`), and names come from
//! the in-process resolver instead.

mod answer;
mod identify;
mod mdns;
mod ports;
mod subnets;

pub(crate) use subnets::local_subnet;

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryIter};

use ipnet::Ipv4Net;
use netscli_core::{
    DiscoverEngine, DiscoverPhase, DiscoverProgress, Host, MdnsService, PingScanner,
    PortScanProgress, PortScanner, UdpScanner,
};
use tokio::runtime::{Builder, Runtime};
use tokio::task::{JoinHandle, JoinSet};

use crate::model::{Answer, ScanOptions, ScanProgress};
use answer::Probed;
use ports::{PortNames, PortPlan};

/// Probes in flight at once, in the sweep and across the port probes of all hosts.
const CONCURRENCY: usize = 128;
const PING_TIMEOUT_MS: u64 = 1000;
const PORT_TIMEOUT_MS: u64 = 1000;
const DNS_TIMEOUT_MS: u64 = 1500;

/// What a running scan reports, besides how far it has got.
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
    counts: Arc<Counts>,
    events: Receiver<Event>,
}

/// How far a scan has got, counted on its threads and read on the UI's.
#[derive(Default)]
struct Counts {
    swept: AtomicUsize,
    probed: AtomicUsize,
    probes: AtomicUsize,
}

impl Scan {
    /// Starts scanning `subnet` with `options`. Fails only if the threads cannot be started.
    pub(crate) fn start(subnet: Ipv4Net, options: ScanOptions) -> io::Result<Self> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("scan")
            .enable_all()
            .build()?;
        let counts = Arc::new(Counts::default());
        let (sender, events) = mpsc::channel();
        let task = runtime.spawn(run(subnet, options, Arc::clone(&counts), sender));
        Ok(Self {
            runtime: Some(runtime),
            task,
            counts,
            events,
        })
    }

    pub(crate) fn progress(&self) -> ScanProgress {
        let counts = &self.counts;
        ScanProgress {
            swept: counts.swept.load(Ordering::Relaxed),
            probed: counts.probed.load(Ordering::Relaxed),
            probes: counts.probes.load(Ordering::Relaxed),
        }
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

async fn run(subnet: Ipv4Net, options: ScanOptions, counts: Arc<Counts>, events: Sender<Event>) {
    let sweep_counts = Arc::clone(&counts);
    let progress: Arc<dyn Fn(DiscoverProgress) + Send + Sync> = Arc::new(move |p| {
        if matches!(p.phase, DiscoverPhase::Ping) {
            sweep_counts.swept.store(p.completed, Ordering::Relaxed);
        }
    });
    let browse = options.mdns_services.then(|| tokio::spawn(mdns::browse()));
    let sweep = DiscoverEngine::new_with_timeouts(CONCURRENCY, PING_TIMEOUT_MS, DNS_TIMEOUT_MS);
    let swept = sweep
        .scan_subnet_with_progress(subnet, false, Some(progress))
        .await;
    let end = match swept {
        Ok(hosts) => {
            let announced = match browse {
                // A browse that failed heard nothing.
                Some(task) => task.await.unwrap_or_default(),
                None => Vec::new(),
            };
            let run = Run {
                announced: &announced,
                names: &PortNames::load(),
                subnet,
                options,
            };
            probe_all(hosts, &run, counts, &events).await
        }
        Err(error) => Event::Failed(error.to_string()),
    };
    // Fails only once the UI has dropped the receiver, when no one is left to tell.
    events.send(end).ok();
}

/// The probes every host shares, so their limits on probes in flight hold across hosts, and
/// the ports they probe on each.
struct Probes {
    ports: PortScanner,
    udp: UdpScanner,
    ping: PingScanner,
    plan: PortPlan,
    /// Counts each port probe as it ends, whatever it found.
    counted: Arc<dyn Fn(PortScanProgress) + Send + Sync>,
}

/// What every host's answer is made with, once the sweep is done.
struct Run<'a> {
    /// Every service announced over mDNS during the sweep.
    announced: &'a [MdnsService],
    names: &'a PortNames,
    subnet: Ipv4Net,
    options: ScanOptions,
}

/// Probes every host the sweep found, reporting each as soon as it is done, and says how the
/// scan ended. A probe that fails ends it: the hosts still being probed would go unreported.
async fn probe_all(
    hosts: Vec<Host>,
    run: &Run<'_>,
    counts: Arc<Counts>,
    events: &Sender<Event>,
) -> Event {
    let plan = PortPlan::for_mode(run.options.mode);
    counts
        .probes
        .store(hosts.len() * plan.probes(), Ordering::Relaxed);
    let shared = Arc::new(Probes {
        ports: PortScanner::new(CONCURRENCY),
        udp: UdpScanner::new(CONCURRENCY),
        ping: PingScanner::new(CONCURRENCY),
        plan,
        counted: Arc::new(move |_| {
            counts.probed.fetch_add(1, Ordering::Relaxed);
        }),
    });
    let mut pending = JoinSet::new();
    for host in hosts {
        let own = mdns::of_host(run.announced, host.ip);
        pending.spawn(probe(host, own, Arc::clone(&shared), run.options));
    }
    while let Some(done) = pending.join_next().await {
        let probed = match done {
            Ok(probed) => probed,
            Err(error) => return Event::Failed(error.to_string()),
        };
        let Some(answer) = answer::answer(&probed, run.subnet, run.options, run.names) else {
            continue;
        };
        if events.send(Event::Host(Box::new(answer))).is_err() {
            // The UI has gone; the rest would go unread.
            break;
        }
    }
    Event::Finished
}

/// Finds out what `options` allow about one host beyond what the sweep did; `mdns` is what it
/// announced.
async fn probe(
    host: Host,
    mdns: Vec<MdnsService>,
    probes: Arc<Probes>,
    options: ScanOptions,
) -> Probed {
    let mut open_ports = scan_tcp(&probes, host.ip).await;
    open_ports.extend(scan_udp(&probes, host.ip).await);
    let unnamed = host.hostname.is_none() && mdns::hostname(&mdns).is_none();
    let reverse_name = if options.service_detection && unnamed {
        // A failed or timed-out lookup means the network has no name for it.
        let name = netscli_core::dns::reverse_lookup_timeout(host.ip, DNS_TIMEOUT_MS).await;
        name.ok().flatten()
    } else {
        None
    };
    // The sweep's own ping keeps no time-to-live, so the OS family needs one more.
    let ttl = if options.os_family {
        probes.ping.ping(host.ip, PING_TIMEOUT_MS).await.ttl
    } else {
        None
    };
    Probed {
        host,
        reverse_name,
        ports: open_ports,
        ttl,
        mdns,
    }
}

async fn scan_tcp(probes: &Probes, host: std::net::IpAddr) -> Vec<netscli_core::PortResult> {
    let mut open = Vec::new();
    for batch in probes.plan.tcp.chunks(ports::BATCH_SIZE) {
        let counted = Some(Arc::clone(&probes.counted));
        let found = probes
            .ports
            .scan_host_with_progress(host, batch.to_vec(), PORT_TIMEOUT_MS, counted)
            .await
            .unwrap_or_default();
        open.extend(found.into_iter().filter(|result| result.open));
    }
    open
}

async fn scan_udp(probes: &Probes, host: std::net::IpAddr) -> Vec<netscli_core::PortResult> {
    let mut open = Vec::new();
    for batch in probes.plan.udp.chunks(ports::BATCH_SIZE) {
        let counted = Some(Arc::clone(&probes.counted));
        let found = probes
            .udp
            .scan_host_with_progress(host, batch.to_vec(), PORT_TIMEOUT_MS, counted)
            .await
            .unwrap_or_default();
        open.extend(found.into_iter().filter(|result| result.open));
    }
    open
}
