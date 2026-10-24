//! Scans a subnet for real, off the UI thread: a sweep for its hosts (see [`sweep`]), with a
//! browse of the services hosts announce over mDNS alongside; then, for each host found, TCP
//! connect probes of the
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
mod ssdp;
mod subnets;
mod sweep;

pub(crate) use subnets::{Adapter, adapters, local_subnet};

use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryIter};

use ipnet::Ipv4Net;
use netscli_core::{Host, MdnsService, PingScanner, PortScanProgress, PortScanner, UdpScanner};
use tokio::runtime::{Builder, Runtime};
use tokio::task::{JoinError, JoinHandle, JoinSet};

use crate::model::{Answer, Discovery, Network, ScanOptions, ScanProgress};
use answer::Probed;
use ports::{PortPlan, PortRegistry};

/// Probes in flight at once across the port probes of all hosts, and the sweep's knocks.
const CONCURRENCY: usize = 128;
/// Whole-host tasks kept at once. Port scanners separately cap their socket work; this cap
/// prevents a /16 from allocating one suspended task (and its result buffers) per address.
const HOST_TASKS: usize = 32;
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
    pub(crate) fn start(
        subnet: Ipv4Net,
        adapter: Option<std::net::Ipv4Addr>,
        options: ScanOptions,
    ) -> io::Result<Self> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("scan")
            .enable_all()
            .build()?;
        let counts = Arc::new(Counts::default());
        let (sender, events) = mpsc::channel();
        let task = runtime.spawn(run(subnet, adapter, options, Arc::clone(&counts), sender));
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

/// Takes in what a running scan reported into the record of its run: how far it has got, and
/// the hosts found, compared with `network`. True if it reported finishing, which ended the run
/// (stopping or failing is not finishing).
pub(crate) fn record(
    discovery: &mut Discovery,
    progress: ScanProgress,
    events: impl IntoIterator<Item = Event>,
    network: &Network,
) -> bool {
    discovery.reach(progress);
    let mut finished = false;
    for event in events {
        match event {
            Event::Host(answer) => discovery.answer(*answer, network),
            Event::Finished => finished |= discovery.finish(),
            Event::Failed(reason) => {
                discovery.fail(reason);
            }
        }
    }
    finished
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
    adapter: Option<std::net::Ipv4Addr>,
    options: ScanOptions,
    counts: Arc<Counts>,
    events: Sender<Event>,
) {
    let browse = options.mdns_services.then(|| tokio::spawn(mdns::browse()));
    let ssdp = options
        .mdns_services
        .then(|| tokio::spawn(ssdp::browse(adapter)));
    let end = match sweep::hosts(subnet, &counts).await {
        Ok(hosts) => {
            let announced = match browse {
                // A browse that failed heard nothing.
                Some(task) => task.await.unwrap_or_default(),
                None => Vec::new(),
            };
            let ssdp = match ssdp {
                Some(task) => task.await.unwrap_or_default(),
                None => Vec::new(),
            };
            let registry = PortRegistry::load();
            let run = Run {
                announced: &announced,
                ssdp: &ssdp,
                registry: &registry,
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
    /// Every SSDP/UPnP declaration heard during the sweep, grouped by responder address.
    ssdp: &'a [(std::net::Ipv4Addr, ssdp::Evidence)],
    registry: &'a PortRegistry,
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
    let plan = PortPlan::for_mode(run.options.mode, run.registry);
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
    let hosts = hosts
        .into_iter()
        .map(|host| {
            let own = mdns::of_host(run.announced, host.ip);
            let ssdp = ssdp::of_host(run.ssdp, host.ip);
            (host, own, ssdp)
        })
        .collect::<Vec<_>>();
    let mut failure = None;
    run_bounded(
        hosts,
        HOST_TASKS,
        |(host, own, ssdp)| probe(host, own, ssdp, Arc::clone(&shared), run.options),
        |done| {
            let probed = match done {
                Ok(Ok(probed)) => probed,
                Ok(Err(error)) => {
                    failure = Some(Event::Failed(error));
                    return false;
                }
                Err(error) => {
                    failure = Some(Event::Failed(error.to_string()));
                    return false;
                }
            };
            let Some(answer) = answer::answer(&probed, run.subnet, run.options, run.registry)
            else {
                return true;
            };
            // The UI has gone if this fails; the rest would go unread.
            events.send(Event::Host(Box::new(answer))).is_ok()
        },
    )
    .await;
    failure.unwrap_or(Event::Finished)
}

/// Runs at most `limit` spawned futures at a time and handles each result as it arrives. False
/// from `done` cancels the remaining set by dropping it.
async fn run_bounded<I, F, Fut, T, D>(items: I, limit: usize, task: F, mut done: D)
where
    I: IntoIterator,
    I::Item: Send + 'static,
    F: Fn(I::Item) -> Fut,
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
    D: FnMut(Result<T, JoinError>) -> bool,
{
    let mut items = items.into_iter();
    let mut pending = JoinSet::new();
    for item in items.by_ref().take(limit.max(1)) {
        pending.spawn(task(item));
    }
    while let Some(result) = pending.join_next().await {
        if !done(result) {
            return;
        }
        if let Some(item) = items.next() {
            pending.spawn(task(item));
        }
    }
}

/// Finds out what `options` allow about one host beyond what the sweep did; `mdns` is what it
/// announced.
async fn probe(
    host: Host,
    mdns: Vec<MdnsService>,
    ssdp: ssdp::Evidence,
    probes: Arc<Probes>,
    options: ScanOptions,
) -> Result<Probed, String> {
    let mut open_ports = scan_tcp(&probes, host.ip).await?;
    open_ports.extend(scan_udp(&probes, host.ip).await?);
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
    Ok(Probed {
        host,
        reverse_name,
        ports: open_ports,
        ttl,
        mdns,
        ssdp,
    })
}

async fn scan_tcp(
    probes: &Probes,
    host: std::net::IpAddr,
) -> Result<Vec<netscli_core::PortResult>, String> {
    let mut open = Vec::new();
    for batch in probes.plan.tcp.chunks(ports::BATCH_SIZE) {
        let counted = Some(Arc::clone(&probes.counted));
        let found = probes
            .ports
            .scan_host_with_progress(host, batch.to_vec(), PORT_TIMEOUT_MS, counted)
            .await
            .map_err(|error| error.to_string())?;
        open.extend(found.into_iter().filter(|result| result.open));
    }
    Ok(open)
}

async fn scan_udp(
    probes: &Probes,
    host: std::net::IpAddr,
) -> Result<Vec<netscli_core::PortResult>, String> {
    let mut open = Vec::new();
    for batch in probes.plan.udp.chunks(ports::BATCH_SIZE) {
        let counted = Some(Arc::clone(&probes.counted));
        let found = probes
            .udp
            .scan_host_with_progress(host, batch.to_vec(), PORT_TIMEOUT_MS, counted)
            .await
            .map_err(|error| error.to_string())?;
        open.extend(found.into_iter().filter(|result| result.open));
    }
    Ok(open)
}
