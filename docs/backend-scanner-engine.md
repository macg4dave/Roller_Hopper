# Upstream nrev integration

Updated 20 October 2026. Documentation only; no application source, manifest or lockfile
changed, and no scanner installed or run. Published crate sources were downloaded through
`cargo info` and inspected. An upstream build attempt was cancelled after the user clarified
the documentation-only scope; it produced no compatibility result. No application gate,
GUI or live-network acceptance is claimed.

## Direction confirmed in this discussion

Use **upstream nrev as a normal Cargo library dependency**, updated through ordinary Cargo
dependency upgrades and `Cargo.lock`. The user rejected maintaining a fork. Do not copy its
source into Roller Hopper, maintain local patches, add a Git submodule or create a replacement
scanner. The backend should retain supported options beyond those exposed by the initial GUI.
This decision selects the integration approach, not proof that nrev meets every requirement.

The user clarified: everything must work by running the app, with no external software,
services or servers. Dependencies compiled into the app or packaged as its libraries are
acceptable; a Cargo-installable command is not automatically an embeddable library.
No plan may depend on users installing Nmap, Npcap, a collector service or another runtime.
Bundling an executable or silently installing a driver does not satisfy that boundary.

## Keep the application's responsibilities small

Use nrev's existing scanning, probes and result types. Tokio supplies the runtime around the
engine; it is not a reason to implement our own probe scheduler, retry engine, TCP scanner or
service fingerprinting. Supplement an essential missing protocol only through an established
library after confirming that nrev does not already provide it.

The actual app has no real scanner to preserve. Its
[scan options](../src/model/discovery.rs) are three booleans, and its
[discovery bridge](../src/ui_bridge/discovery.rs) reveals fixture answers with a timer.
[Services](../src/model/services.rs) currently distinguish port numbers but not transport.
Keep the map, editing and review workflow; do not adopt these mock limitations as the engine
contract. The existing documents' in-process requirement remains applicable.

## Normal Cargo usage after compatibility is established

The inspected published release is **nrev 0.7.0**, MIT, with an actual library target as well
as a CLI target. The intended dependency declaration is `nrev = "0.7.0"` under the app's
existing `[dependencies]`. This is a plan, not a manifest edit made in this task. Cargo's
version requirement permits compatible 0.7 updates; the application lockfile records the
resolved release and dependency graph. Update deliberately and rerun the required gate.
Users need neither Cargo nor a separate nrev installation.

Call `Scanner` with `ScanConfig`, `DataRegistry` and `SocketConnector` on an owned background
runtime. Convert its results into the existing pure network model through the current Rust
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
If a required capability cannot work in stock upstream, report the gap for a product decision
rather than installing a driver, creating a fork or silently replacing the engine.

Sources: [published package](https://docs.rs/crate/nrev/0.7.0),
[library exports](https://docs.rs/crate/nrev/0.7.0/source/src/lib.rs),
[scanner](https://docs.rs/crate/nrev/0.7.0/source/src/scanner.rs),
[configuration](https://docs.rs/crate/nrev/0.7.0/source/src/config.rs),
[Cargo dependency requirements](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html),
[Cargo.lock](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html).

## Confirmed Windows dependency issue

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
chosen nrev release packages or implements those paths appropriately. Port connections and
service replies also cannot replace every observation available from raw packets. Keep
engine packaging, administrator permissions and method coverage as separate questions.

Sources: [TCP connect scan semantics](https://nmap.org/book/man-port-scanning-techniques.html),
[Windows raw-socket restrictions](https://learn.microsoft.com/en-us/windows/win32/winsock/tcp-ip-raw-sockets-2),
[Windows ICMP API](https://learn.microsoft.com/en-us/windows/win32/api/icmpapi/nf-icmpapi-icmpsendecho2),
[Windows ARP API](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-sendarp).

## Remaining qualification gaps

The following questions must be resolved through source/dependency review and, in a separately
authorised implementation exercise, actual compatibility checks:

1. Can the embedded ordinary TCP/UDP/service paths build and run on Windows, Linux and macOS
   without Npcap, helper executables or extra services? Merely turning off SYN in settings does
   not prove that native linking/loading or scanner initialisation is independent of capture.
2. Can results arrive incrementally and can Stop/close stop owned work within an agreed bound?
   Inspected nrev `ScanEvent` reports phase starts/completions; its scanner returns a final
   report. This is not evidence of per-finding streaming, pause support or cooperative
   cancellation. Do not invent a progress percentage or label abandoned work cancelled.
3. Are concurrency, retries, response sizes and time budgets bounded across all selected
   probes? Audit the actual code paths, not only the configuration field names.
4. Are bundled probe/signature datasets distributable, maintained and useful? Check data
   licences separately from the Rust package licence. Pin the qualified revision and assets.
5. Does normal packaged-app launch work with OS network permissions, including denied access?
   A developer terminal run cannot establish the user's installation experience.

Stock nrev is not yet demonstrated to provide all desired capabilities under the
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
Map relationships remain a separate inference: finding an endpoint or a route does not prove
a direct physical link. A router/controller API can enrich results where equipment supports
it, but an off-device API is not a replacement for locally reaching private network targets.

The dependency approach is settled: ordinary upstream Cargo usage. Driver-free packaging
and run control remain qualification gaps. This task is documentation only; no source
integration, protected-guide amendment or implementation backlog is authorised by this note.
