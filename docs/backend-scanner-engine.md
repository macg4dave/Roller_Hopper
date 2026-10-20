# Upstream netscli-core integration

Updated 20 October 2026. Documentation only; no application source, manifest or lockfile
changed, and no scanner installed or run. Published crate sources were downloaded through
`cargo info` and inspected. An upstream build attempt was cancelled after the user clarified
the documentation-only scope; it produced no compatibility result. No application gate,
GUI or live-network acceptance is claimed.

## Direction confirmed in this discussion

Use **upstream netscli-core as a normal Cargo library dependency**, updated through ordinary Cargo
dependency upgrades and `Cargo.lock`. The user rejected maintaining a fork. Do not copy its
source into Roller Hopper, maintain local patches, add a Git submodule or create a replacement
scanner. The backend should retain supported options beyond those exposed by the initial GUI.
This replaces nrev and selects the integration approach, not proof of every capability.
The latest clarification reiterates Windows, Linux and macOS support with platform
compatibility maintained by upstream dependencies. Keep integration shared wherever practical;
explain any unavoidable platform-specific need and wait for explicit user approval before
implementing it. The comparison below is supporting research, not competing engine selections.

The user clarified: everything must work by running the app, with no external software,
services or servers. Dependencies compiled into the app or packaged as its libraries are
acceptable; a Cargo-installable command is not automatically an embeddable library.
No plan may depend on users installing Nmap, Npcap, a collector service or another runtime.
Bundling an executable or silently installing a driver does not satisfy that boundary.

## Keep the application's responsibilities small

Use netscli-core's existing scanning, probes and result types. Tokio supplies the runtime around the
engine; it is not a reason to implement our own probe scheduler, retry engine, TCP scanner or
service fingerprinting. Supplement an essential missing protocol only through an established
library after confirming that netscli-core does not already provide it.

The actual app has no real scanner to preserve. Its
[scan options](../src/model/discovery.rs) are three booleans, and its
[discovery bridge](../src/ui_bridge/discovery.rs) reveals fixture answers with a timer.
[Services](../src/model/services.rs) currently distinguish port numbers but not transport.
Keep the map, editing and review workflow; do not adopt these mock limitations as the engine
contract. The existing documents' in-process requirement remains applicable.

## Normal Cargo usage after compatibility is established

The inspected published library is **netscli-core 0.3.4**, MIT. It has empty default features;
packet capture, database support and mDNS are separately selectable. Keep capture disabled
under the no-extra-driver requirement. Select and lock an appropriate upstream release after
qualification, then update deliberately through Cargo. No dependency was added in this task.
Users need neither Cargo nor a separate NetsCLI application, CLI or server installation.

Use existing library operations such as `PortScanner`, `DiscoverEngine` and `SweepEngine`
on an owned background runtime. Convert their results into the pure network model through the Rust
bridge. Keep `Rc<RefCell<AppState>>` and Slint model mutation on the UI thread. Runtime type
names describe existing upstream APIs; they are not instructions to add another adapter
framework or a replacement inventory. The app's integration should only validate settings,
control the run, interpret coverage and admit observations while protecting manual edits.

Use the full configuration supported by the qualified upstream release where applicable.
The GUI's three switches must not become the entire backend contract. Do not invent unused
engine options or promise features that the selected release cannot supply.

For defects, record a minimal reproduction for upstream; do not maintain a patched release
in this product. Reporting an issue or publishing any upstream contribution requires an
explicit instruction. An upstream fix becomes available through a normal dependency upgrade.
If a required capability cannot work through the shared upstream approach, explain the
concrete need, alternatives and maintenance/permission/packaging impact for user approval.
Do not install a driver, maintain a fork or silently replace the engine.

Sources: [published package](https://docs.rs/crate/netscli-core/0.3.4),
[library exports](https://docs.rs/crate/netscli-core/0.3.4/source/src/lib.rs),
[scanners](https://docs.rs/crate/netscli-core/0.3.4/source/src/scan/mod.rs),
[Cargo dependency requirements](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html),
[Cargo.lock](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html).

## Confirmed Windows dependency issue in rejected nrev

Published sources inspected through `cargo info nrev`, `cargo info nex@0.26.0` and
`cargo info nex-datalink@0.26.0` show the chain:

`nrev 0.7.0 → nex 0.26.0 → nex-datalink 0.26.0 → Windows Packet library`.

The Windows backend and bindings are selected by `cfg(windows)`. In
`nex-datalink/src/bindings/windows.rs`, `#[link(name = "Packet")]` declares the native link.
The crate's optional `pcap` feature controls a separate backend; disabling it does not remove
the normal Windows WinPcap/Npcap bindings. nrev 0.7.0 declares no feature to exclude that
dependency. Its TCP connector uses ordinary Tokio sockets, but choosing TCP at runtime does
not change the Cargo dependency graph or remove native link metadata.

This is a **confirmed source-level packaging obstacle**, not a reproduced linker failure or
proof of the exact DLL imports retained by a final application. Standalone Windows linking
and normally launched packaged-app behaviour still need compatibility evidence. Do not mark
stock nrev 0.7.0 as qualified for a machine without Npcap, or offer an unverified feature-flag
workaround. The user's no-extra-software and no-fork requirements both remain in force.

Sources: [nrev manifest](https://docs.rs/crate/nrev/0.7.0/source/Cargo.toml),
[nex manifest](https://docs.rs/crate/nex/0.26.0/source/Cargo.toml),
[datalink features](https://docs.rs/crate/nex-datalink/0.26.0/source/Cargo.toml),
[Windows backend selection](https://docs.rs/crate/nex-datalink/0.26.0/source/src/lib.rs),
[native bindings](https://docs.rs/crate/nex-datalink/0.26.0/source/src/bindings/windows.rs).

### Extra drivers are not a universal scanner requirement

The nrev dependency issue must not be described as a requirement of all network scanning.
A scanner can use the operating system's ordinary sockets and supported native APIs.

| Capability on Windows | Extra capture driver intrinsically required? |
| --- | --- |
| TCP connect scanning, including selected ranges or all TCP ports | No; the OS establishes connections and returns outcomes |
| Service interrogation over ordinary TCP/UDP, such as HTTP/TLS or DNS | No; protocol responses supply evidence without arbitrary packet capture |
| ICMP echo through Windows `IcmpSendEcho2` | No; Windows supplies the API, although a particular cross-platform library may use a different mechanism |
| On-link IPv4 MAC resolution through Windows `SendARP` | No; Windows supplies the API. This does not reveal MAC addresses across routers |
| Raw TCP SYN probing, packet-level OS fingerprints or arbitrary link-layer capture | Typically needs a capture/injection driver such as Npcap on Windows; ordinary Winsock raw sockets have restrictions |

Therefore a capable driver-free mapper is possible, but that does not establish that the
selected scanner library packages or implements those paths appropriately. Port connections and
service replies also cannot replace every observation available from raw packets. Keep
engine packaging, administrator permissions and method coverage as separate questions.

Sources: [TCP connect scan semantics](https://nmap.org/book/man-port-scanning-techniques.html),
[Windows raw-socket restrictions](https://learn.microsoft.com/en-us/windows/win32/winsock/tcp-ip-raw-sockets-2),
[Windows ICMP API](https://learn.microsoft.com/en-us/windows/win32/api/icmpapi/nf-icmpapi-icmpsendecho2),
[Windows ARP API](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-sendarp).

## Cross-platform alternatives under review

Research refreshed against published sources on 20 October 2026. No candidate was built,
tested or run in this follow-up. `cargo info` downloaded sources for inspection only.
Advertised platforms and CI configuration are evidence of upstream intent, not proof of
Roller Hopper compatibility. A published library is preferable to extracting an app's engine.

| Alternative | Reuse and platform evidence | Fit and remaining maintenance burden |
| --- | --- | --- |
| **netscli-core 0.3.4** | MIT, published Rust library expressly for consumers; TCP/UDP scanners, discovery, sweep, DNS, service probes, OUI and optional mDNS. Upstream CI defines Windows/Linux/macOS jobs. Capture and SQLite are optional, with empty default features. | Closest broad alternative in this pass. **Not a drop-in selection:** published MSRV is Rust 1.96 versus our protected 1.94. macOS ARP reading invokes `arp`; traceroute invokes system tools; Windows reverse-name fallback can invoke `ping`. These are built-in tools, not extra installations, but they still violate the current no-child-program boundary. A qualified subset might fit without patches; verify call paths before claiming this, including discovery. |
| **qscan 0.6.0** | Published Rust library for async TCP-connect and ICMP scans, with batch, timeout and retry controls. Ordinary TCP uses Tokio rather than a capture driver. | Much narrower than the requested discovery/identification engine. Inspected repository HEAD is dated December 2022 and its manifest uses older DNS/ICMP dependencies; no three-platform acceptance evidence established here. GPL-3.0-or-later requires a distribution/licence decision. Not recommended as the main engine on present evidence. |
| **zond-engine 0.19.0** | Broad published Rust engine for discovery, protocol parsing, service/OS fingerprints; current README lists Linux, macOS and Windows. Declared Rust 1.93. | Current README requires libpcap for builds and Npcap on Windows. AGPL-3.0-or-later. Older 0.7.0 documentation excluded Windows; do not repeat that stale limit, but current support does not remove capture dependencies. Does not satisfy the no-extra-software requirement as documented. |
| **portmap 2.0.0 / pmap** | MIT, Rust library target plus CLI; cross-platform TCP-connect scanner with targets, port ranges and timing controls. | Manifest includes a non-optional `pcap` dependency despite the portable TCP path. The same dependency-audit problem recurs, and it is a port scanner rather than the complete identification/discovery engine. Not qualified. |
| **lsnet / Happy IP Scanner** | Rust applications advertising Windows/Linux/macOS. lsnet combines several device-identification protocols and avoids Windows Npcap through native APIs; Happy IP Scanner offers IP/port discovery. | Inspected manifests/source expose applications, with no normal public library entry point found. Copying their engine or maintaining a repackaging fork would create the maintenance the user rejected; invoking the application is excluded. Useful examples, not direct dependencies. lsnet is GPL-3.0-or-later; Happy IP Scanner is MIT. |
| **mdns-sd plus specific protocol libraries** | mdns-sd explicitly provides cross-platform, in-process DNS-SD/mDNS without Bonjour/Avahi installation. Existing crate research covers DNS, SSDP and SNMP libraries. | Useful supplementary APIs, not a complete network scanner. Combining them with socket code still leaves Roller Hopper owning scan orchestration and missing methods. Do not describe this as equivalent to adopting a maintained scanner engine. |

Primary evidence:

- netscli-core: [published manifest](https://docs.rs/crate/netscli-core/0.3.4/source/Cargo.toml),
  [library API](https://docs.rs/netscli-core/0.3.4/netscli_core/),
  [ARP platform code](https://docs.rs/crate/netscli-core/0.3.4/source/src/arp/platform/table.rs),
  [reverse-name fallback](https://docs.rs/crate/netscli-core/0.3.4/source/src/dns/reverse.rs),
  [traceroute](https://docs.rs/crate/netscli-core/0.3.4/source/src/trace.rs),
  [upstream CI](https://github.com/fstubner/netscli/blob/40dde387dc47646c938917de58b492dcb4dc2b20/.github/workflows/ci.yml).
- qscan: [library and features](https://github.com/0xor0ne/qscan/tree/dd3e8bd0421d05ab43a01b823d3d0a96c6b1ad3e/qscan),
  [manifest](https://github.com/0xor0ne/qscan/blob/dd3e8bd0421d05ab43a01b823d3d0a96c6b1ad3e/qscan/Cargo.toml).
- zond-engine: [current published README](https://docs.rs/crate/zond-engine/0.19.0/source/README.md),
  [manifest](https://docs.rs/crate/zond-engine/0.19.0/source/Cargo.toml).
- portmap: [published manifest](https://docs.rs/crate/portmap/2.0.0/source/Cargo.toml).
- App examples: [lsnet](https://github.com/sanford/lsnet),
  [Happy IP Scanner](https://github.com/nikhilakki/happy-ip-scanner).
- [mdns-sd platform and dependency description](https://docs.rs/mdns-sd/latest/mdns_sd/).

`cargo info netscli-core` selected 0.2.6 for the current compiler, while an explicit
`cargo info netscli-core@0.3.4` confirmed the newer release and its Rust 1.96 minimum.
Do not silently pin 0.2.6 as a solution: its Windows/macOS ARP paths also use commands, and
the older dependency graph and behaviour are not qualified. No toolchain change is approved.

### Programs, commercial SDKs and device APIs

Nmap remains a strong standalone cross-platform scanner, but its recommended executable
integration is excluded. Angry IP Scanner is a Java/SWT application rather than a Rust
library. Choosing a standalone program or embedding another runtime moves the packaging
problem; it does not establish an in-process Rust solution. See
[Nmap integration guidance](https://nmap.org/oem/) and
[Angry IP Scanner source](https://github.com/angryip/ipscan).

Fing's inspected mobile SDK documentation describes iOS/Android integration, cloud-based
recognition and internet licence validation each session. It does not establish an offline
Windows/Linux/macOS SDK suitable for this app. This is a finding about that documented
offering, not a claim that no other commercial arrangement exists.
[Publisher SDK documentation](https://get.fing.com/fing-business/devrecog/documentation/Fing_Mobile_SDK.pdf)

Read-only SNMP or a router/controller's local API can supply inventory/topology independently
of the scanning computer's OS. They depend on equipment, credentials and exposed information;
they cannot be the universal scanner for arbitrary unmanaged networks. Keep them optional
sources for selected equipment, not a required external collector/server or a new vendor
integration maintenance programme.

The user has selected **stock netscli-core**. Qualify the chosen release/toolchain and call
paths without creating a local fork or silently enabling capture. No toolchain change is
approved. Bring a genuine platform-specific need to the user with its alternatives and
maintenance implications before implementation. Upstream owns platform implementations;
Roller Hopper still needs package and integration acceptance on all three OSes.

## Remaining qualification gaps

The following questions must be resolved through source/dependency review and, in a separately
authorised implementation exercise, actual compatibility checks:

1. Can the embedded ordinary TCP/UDP/service paths build and run on Windows, Linux and macOS
   without Npcap, helper executables or extra services? Merely turning off SYN in settings does
   not prove that native linking/loading or scanner initialisation is independent of capture.
2. Can results arrive incrementally and can Stop/close stop owned work within an agreed bound?
   Qualify netscli-core's progress callbacks and task lifetime for the chosen release.
   Neither a progress callback nor dropping an await proves pause or cancellation of every
   owned operation. Do not invent a percentage or label abandoned work cancelled.
3. Are concurrency, retries, response sizes and time budgets bounded across all selected
   probes? Audit the actual code paths, not only the configuration field names.
4. Are bundled probe/signature datasets distributable, maintained and useful? Check data
   licences separately from the Rust package licence. Pin the qualified revision and assets.
5. Does normal packaged-app launch work with OS network permissions, including denied access?
   A developer terminal run cannot establish the user's installation experience.

Stock netscli-core is not yet demonstrated to provide all desired capabilities under the
no-external-software boundary. Raw SYN scanning and active TCP/IP OS fingerprints must not be
promised across platforms. Reused service protocols can still provide rich identification.
Additional libraries such as mdns-sd can complement the engine where it lacks an essential
discovery method; that should not become a custom probing framework.

## Broad backend, initially small GUI

After engine qualification, use its typed configuration and results behind a thin Rust
integration. The GUI selects a useful subset; it must not define the maximum backend capability.
Avoid creating empty options, a plugin system or several interchangeable engines in advance.

| Configuration area | Capabilities to retain when the chosen engine supports them |
| --- | --- |
| Targets | Multiple explicit addresses/CIDRs, exclusions, routed targets and interface/source selection |
| Discovery | Select supported host-discovery methods; permit port scans when ping receives no reply |
| Ports | Explicit TCP lists/ranges and full TCP range; independently selected named UDP protocols |
| Identification | Selected service probes, banners, TLS details and engine-provided evidence; distinguish observations from guesses |
| Work limits | Concurrency, pacing where available, connect/probe timeouts, retries and overall workload bounds |
| Results | Transport plus port, typed outcome, timestamps, method/source, partial coverage and errors |

Preserve the already recorded TCP-range plus named-UDP scope. An engine supporting blanket UDP
scans, IPv6, OS fingerprints or other modes does not automatically enable them in this product.
Unavailable capabilities need an explicit reason rather than a silent approximation.

Our application code still owns scope validation, run lifetime, result admission, identity
reconciliation and protection of manual edits. The engine owns implemented scanning/probing.
Keep endpoint observations under the correct child interface and its parent device. All
collection is user-started: no automatic refresh or lingering discovery probes between runs.
Identification may go as deep as the user selects and the adopted tools support, with
necessary platform exceptions brought back for approval. Save all data and restorable state;
do not invent Forget or Reset workflows.
Map relationships remain a separate inference: finding an endpoint or a route does not prove
a direct physical link. A router/controller API can enrich results where equipment supports
it, but an off-device API is not a replacement for locally reaching private network targets.

The dependency approach is settled: ordinary upstream Cargo usage. Driver-free packaging
and run control remain qualification gaps. This task is documentation only; no source
integration, protected-guide amendment or implementation backlog is authorised by this note.
