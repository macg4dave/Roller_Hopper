# Reuse a scanner engine

Research: 20 October 2026. Documentation only; no dependencies added, builds, tests,
scanner installations or network scans performed. This is a focused engine-selection
addendum to [the discovery strategy](backend-discovery.md), not an approved implementation
roadmap. Existing planning documents had uncommitted edits and were left untouched.

## Direction confirmed in this discussion

Reuse an existing scanner implementation from a crate, library or repository. Roller Hopper
should provide a powerful scan backend with more options than the initial GUI exposes.
Do not build a new scanner out of socket primitives simply because the GUI is small.

The user clarified: everything must work by running the app, with no external software,
services or servers. Dependencies compiled into the app or packaged as its libraries are
acceptable; a Cargo-installable command is not automatically an embeddable library.
No plan may depend on users installing Nmap, Npcap, a collector service or another runtime.
Bundling an executable or silently installing a driver does not satisfy that boundary.

## What the present plan would leave us building

The current [crate shortlist](backend-crates.md) supplies Tokio, interface enumeration,
DNS and individual discovery protocols. These are useful components, but Tokio is an I/O
runtime, not a complete scanner. Using them alone still leaves us owning probe scheduling,
port-state interpretation, retries and service interrogation. That is substantially more
scanner implementation than the user wants.

The actual app has no real scanner to preserve. Its
[scan options](../src/model/discovery.rs) are three booleans, and its
[discovery bridge](../src/ui_bridge/discovery.rs) reveals fixture answers with a timer.
[Services](../src/model/services.rs) currently distinguish port numbers but not transport.
Keep the map, editing and review workflow; do not adopt these mock limitations as the engine
contract. The existing documents' in-process requirement remains applicable.

## Engine comparison

These are researched candidates, not validated integrations. Repository manifests and source
were inspected as well as project descriptions. Versions identify inspected artifacts;
repository HEAD and published crates can differ.

| Candidate | Reusable functionality | Fit under the confirmed packaging requirement |
| --- | --- | --- |
| **nrev**, repository 0.7.0 | Public Rust library modules as well as CLI; TCP/UDP/SYN/QUIC scanning, service observations, host discovery, neighbour discovery, traceroute, configurable probes and structured results | Strongest first candidate from this pass for breadth plus an embedded API. **Conditional**, not selected: raw/capture operations require privileges and Npcap on Windows. Establish whether ordinary socket paths and their complete dependency graph run without that installation. Its source includes configuration, a scanner API and a socket connector, so evaluation need not start by copying the CLI. |
| **netscan**, repository 0.30.0 | Actual Rust library; TCP connect/SYN scans, ICMP/TCP/UDP host probes and service detection; settings for targets, interface, concurrency and timing | Smaller fallback candidate. README documents raw privileges and Windows Npcap/WinPcap; the TCP-connect source has a distinct ordinary-socket path, but that does not prove a clean driver-free Windows build/runtime. Less broad than nrev; no assumption of full Nmap-equivalent service identification. |
| **pistol**, published documentation 4.0.18 | Rust library with many discovery/port-scan methods, OS detection and service/version detection | Broad API, but documented Windows capture dependencies conflict with the requirement. It also ships Nmap-derived databases requiring separate provenance/licence examination. Repository HEAD inspected separately identifies itself as 5.0.0; do not combine that manifest with 4.x API assumptions. Not the first choice here. |
| **Nmap** | Established broad host/port discovery, service/version detection, OS fingerprinting and substantial configuration | Capability benchmark. Its publisher recommends executable invocation with XML output; that approach is excluded here. Incorporating source is discussed by the publisher, but is not evidence of a supported drop-in Rust library or a small integration. Redistribution terms also need qualification. Do not promise embedded Nmap as an easy solution. |
| **RustScan** | Existing fast port scanner with configurable inputs and scripting/integration | Its documented broader workflow delegates to Nmap or other scripts. It does not remove the packaging problem or supply the complete embedded discovery/identification backend wanted here. |

Primary sources:

- nrev: [overview](https://github.com/shellrow/nrev),
  [inspected library exports](https://github.com/shellrow/nrev/blob/d2d4e231a0fb5228c5294d29d318d88249d6ccac/src/lib.rs),
  [scanner API](https://github.com/shellrow/nrev/blob/d2d4e231a0fb5228c5294d29d318d88249d6ccac/src/scanner.rs),
  [configuration](https://github.com/shellrow/nrev/blob/d2d4e231a0fb5228c5294d29d318d88249d6ccac/src/config.rs),
  [transport implementation](https://github.com/shellrow/nrev/blob/d2d4e231a0fb5228c5294d29d318d88249d6ccac/src/transport.rs),
  [usage](https://github.com/shellrow/nrev/blob/d2d4e231a0fb5228c5294d29d318d88249d6ccac/docs/USAGE.md).
- netscan: [overview and Windows requirements](https://github.com/shellrow/netscan),
  [settings](https://github.com/shellrow/netscan/blob/361dc17f2a2999259a0ac0f7b9e6788af2fb3bc7/src/scan/setting.rs),
  [scanner implementation](https://github.com/shellrow/netscan/blob/361dc17f2a2999259a0ac0f7b9e6788af2fb3bc7/src/scan/scanner.rs).
- pistol: [published 4.0.18 documentation and database statement](https://docs.rs/crate/pistol/4.0.18),
  [repository](https://github.com/rikonaka/pistol-rs).
- Nmap: [capabilities](https://nmap.org/book/man.html),
  [publisher integration guidance](https://nmap.org/oem/),
  [licence](https://nmap.org/npsl/), [Npcap distribution](https://npcap.com/oem/).
- RustScan: [maintainer README](https://github.com/bee-san/RustScan).

Maintenance evidence is limited: inspected nrev HEAD is a 0.7.0 release commit dated
4 April 2026, and netscan HEAD is a 0.30.0 release commit dated 15 February 2026.
These are smaller projects than Nmap; release activity and an advertised platform list do
not establish reliability or compatibility with this app's Rust 1.94 toolchain.

## Recommendation and qualification gaps

Evaluate **nrev's embedded library first**, with netscan as the narrower alternative. Neither
is approved as a driver-free solution by this review. Reject any candidate whose usable path
requires external software; do not turn rejection into permission to write our own engine.
A small upstream improvement may be preferable to a permanent fork, but maintaining a large
fork would defeat the reuse objective.

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

No inspected candidate is yet demonstrated to provide all desired capabilities under the
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

The next design decision is the engine's qualified capability set, especially driver-free
packaging and run control. Do not approve a socket-by-socket implementation roadmap before
that decision, and do not expand the GUI merely to demonstrate every engine option.
