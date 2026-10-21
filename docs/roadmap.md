# Roadmap: first working backend

A living plan. Agents take their next task from here and delete tasks once done; blockers and
bigger needs found along the way go in the backlog (`docs/.backlog.md`). The rules for both
are in the workflow guide (`docs/agents/workflow.md`, "Planning"). Requirements live in the
product brief (`docs/agents/product.md`); this file says how and in what order to meet them.

Goal: a usable, locally runnable backend on Windows, Linux and macOS with complete project
files, real IPv4 scanning, reliable inventory updates and an offline AI model the user installs.
Installers, signing and public release are outside this plan.

Live scans, protected-file changes and publication still need the user's explicit approval.

## Findings

What the research and code review established on 20 October 2026. Versions are the ones
inspected then; recheck the release, licence, MSRV, features and platform support before
adding any dependency. Nothing below was built, run or measured: candidates are not choices
until their qualification task is done.

### Starting point in the code

- Startup builds sample data into one UI-owned `Rc<RefCell<AppState>>`. The model in
  `src/model/` has no Slint dependency, and `ui_bridge/view.rs` keeps `VecModel`s and updates
  only changed rows. Extend this: workers return owned data into the same state; no second
  inventory.
- Map nodes are interface cards (`CardId`) drawn with their device's icon; a device can own
  several cards and nested VMs/containers. The discovery page, pre-Add editing of name, role,
  icon and notes, mapped-device editing, Add/Ignore and Show on map all work with sample data.
- The simulated scan (`ui_bridge/discovery.rs`, `mock_data/answers.rs`) refreshes every sample
  status at Start, then reveals precomputed answers on a 150 ms timer over about forty steps.
  Methods, confidence and elapsed time are made up.
- Gaps before real data:
  - Runtime metadata is `Option<&'static str>`; a card has one optional IPv4/prefix and gateway.
  - Services are keyed by port number alone, with no transport (TCP vs UDP).
  - IP uniqueness is global. Discovery matches on MAC+IP, flags changed-IP/same-MAC as a
    possible duplicate, and treats a missing MAC as new.
  - IDs are "current maximum plus one", so they can be reused. The bridge uses `i32` with
    `-1` for no selection, while the IDs are `u32`.
  - Status and relative last-seen are device-wide; empty values also mean "not collected".
  - Stop becomes Finished, with no cancelled, failed or partial outcome. Discovery results
    and actions use list indices.
  - Targets come from mapped prefixes, so an empty project cannot bootstrap a scan.
  - `Network::new` trusts its inputs, and mutable access bypasses address validation. Layout
    starts from rootless cards, so an imported rootless cycle would get no positions.
  - `Found::detect` keeps the whole fixture device. Add copies it, and insertion connects its
    first card without rechecking address conflicts.
  - A newcomer added to the map can be found twice on the next simulated run, because
    mapped answers and fixed newcomers overlap. This was found by reading, not reproduced.
  - Every update and motion tick rebuilds view vectors. Measure before adding caches.
- Map placement: a drop calls `Network::connect`, updates `Arrangement` and re-runs layout,
  so display reparenting creates a network link. `lay_out` replaces all positions and startup
  begins with an empty arrangement. During a drag, `AppState.positions` holds preview
  positions. Pan, zoom and auto-fit live in Slint.
- Pinned: Rust 1.99.1 (protected; raised from 1.94 for the scanner, with the user's approval),
  Slint/slint-build 1.18.1 with `renderer-skia`, ipnet 2.12.2.
  Slint's generic software renderer does not support the canvas's scaling; Skia software
  rendering is a different option, untested.

### What the prototype UI settles

The mock GUI is the interaction specification: real data flows through its pages and
controls, changed only where the brief or an agreed decision says. It already defines:

- Main view: search by name, IP or MAC; All / Online / Offline filters with counts; a device
  list with collapsible network cards; Add device. The map has zoom, a percentage, Fit, Reset
  layout and a View menu (names, roles, IP addresses, MAC addresses, hostnames, status dots).
  Details show what a guest runs on, each card with the link it is plugged into (known or
  inferred), type, manufacturer, model, hostname, last seen, open ports and notes.
- Edit page ("Devices, roles and groups"): device name, role, icon and notes; network cards
  with name, icon, IPv4/prefix, mask, gateway and MAC, each with an inline hint; manual ports
  (number and description); VM/container guests. Roles can be added, renamed, given an icon or
  group, deleted (their devices become Other), set to follow detected icons, and the standard
  set restored. Groups have a name, colour and line style.
- Move network card: moving a card to another device merges them, confirmed when it is the
  source device's only card; its guests follow. On the discovery page a found card can move
  onto a mapped device, which is how a possible duplicate is resolved by hand.
- Discovery: a Network choice; Ping sweep, Port scan and Service detection options; Start or
  Scan again, Pause/Resume and Stop; "n of m addresses checked" with elapsed time and a
  progress bar. Results show name, found-by method chips, confidence and the flags New,
  Duplicate?, Added and Ignored, with "Same MAC as"; a mapped card outside the scanned network
  shows Not seen. A mapped host gets its normal editor and Show on map; others get a pre-Add
  form, Add to map and Ignore. The log has Info and Warning lines, and the header shows
  Scanning… while a run continues on another page.
- Icons come from an `icons` folder beside the program (the source tree as fallback), with
  subfolders as categories, re-read whenever the picker opens.

Gaps the UI shows against the plan:

- No New, Open, Save or Save As, project name or unsaved mark; the status bar says "Sample
  data only". Settings holds a pointer to the View menu and opens the AI settings window:
  install Qwen3-4B, LFM2-700M, Qwen3-0.6B or SmolLM2-135M, or list the user's own GGUF files and
  safetensors folders from the models folder; Load or Unload one with Candle (CPU, or GPU in a `cuda` build, with a
  CUDA light), and Run it on
  an editable prompt and sample file (an nmap-style NAS) with adjustable answer settings. The
  settings and edited texts are not saved. Its output is shown raw, not checked against the
  schema; it fills no device fields; both automation switches stay disabled, saying why.
- The Network choice lists mapped prefixes; the three option checkboxes predate the agreed
  depth and pace presets; method labels include SNMP, ONVIF and LLDP, which are not offered;
  Confidence is a made-up percentage.
- The card IP hint promises an address "no other card has" (global uniqueness); manual ports
  have no transport; last seen is relative ("At last scan", "n minutes ago").
- Forget device comes back on restart, and Ignore lasts one run. Custom icons are referenced
  in the folder, not embedded.

### Scanner: netscli-core

- **Selected approach: stock netscli-core as a normal Cargo dependency.** No fork, copied
  source, local patch, submodule or replacement scanner. The app owns scope, run lifetime,
  admission and manual authority; the library owns probing, retries and fingerprints. Defects
  get a minimal reproduction for upstream (reporting it needs the user's go-ahead).
- **Chosen release: 0.3.4** with `default-features = false, features = ["mdns"]`; never
  `pcap` or `db`. It needs Rust 1.96 for real (its `sysinfo` 0.39 uses `cfg_select!`), so the
  toolchain moved to 1.99.1. Checked on Windows: builds with and without `mdns`. Linux and
  macOS builds are unchecked here; upstream CI covers them.
- Licences: the 240 runtime packages (all targets, `mdns` on) are MIT, Apache-2.0, ISC,
  BSD-3-Clause or Unicode-3.0; LGPL appears only as one choice of an OR. The service probes
  are its own, not nmap's database. The IEEE OUI registry is embedded (about 400 KB); it
  prefers `NETSCLI_OUI_PATH` or an `oui.min.json.gz` beside the program.
- API: `PingScanner`, `PortScanner`, `DiscoverEngine`, `SweepEngine`, DNS, OUI and mDNS, with
  progress callbacks. Only packet capture has a cancellation token: a scan stops when its
  future is dropped, and neighbour-table reads run in `spawn_blocking`. There is no pause.
  `scan_host` takes at most 4,096 ports (`MAX_PORTS_PER_SCAN`) and returns a host's ports at
  the end; its callback reports each port as it finishes. Concurrency caps at 1,024.
- **Child processes (static trace of 0.3.4):** every `DiscoverEngine`/`SweepEngine` scan
  reads the neighbour table, which runs `arp -an` on macOS (Windows uses `GetIpNetTable2`,
  Linux `/proc/net/arp`); `reverse_lookup_best_effort_timeout` runs `ping -a` on Windows
  (reached by discovery with names on, `inspect` and `Ops`); `trace_route` and ARP-table
  changes run system tools. Compliant route: drive `PingScanner`, `PortScanner` and
  `reverse_lookup_timeout` (hickory) directly, and never call trace or ARP changes. That
  leaves macOS without neighbour MACs unless a safe in-process source is qualified or the
  user approves an exception.
- **ICMP:** Windows uses `IcmpSendEcho`, unprivileged. Linux and macOS open a raw Layer 3
  socket, which needs root or `CAP_NET_RAW`; otherwise `PingScanner` silently uses a TCP
  connect probe (`method` reports `tcp-connect`). That must show as ICMP unavailable, not as
  a ping.
- Use the library's full typed configuration where it is useful (targets, exclusions,
  source interface, discovery methods, TCP lists/ranges, named UDP, probes, concurrency,
  timeouts, retries). The first GUI shows a subset, but don't add empty options.
- **Rejected alternatives:**
  - nrev 0.7.0: nex-datalink 0.26.0 links the Windows `Packet` (Npcap) library whatever its
    features. Turning off pcap or choosing TCP at runtime doesn't remove that.
  - zond-engine 0.19.0: needs libpcap/Npcap and is AGPL.
  - qscan 0.6.0: too narrow, unmaintained since 2022 and GPL.
  - portmap 2.0.0: has a non-optional pcap dependency.
  - lsnet and Happy IP Scanner: applications with no library entry point.
  - Nmap and other executables: excluded.
  - Fing's SDK: mobile only, with cloud recognition and online licensing.
- Without a capture driver: TCP connect (including the full range), ordinary TCP/UDP service
  queries, and the native Windows ICMP (`IcmpSendEcho2`) and on-link ARP (`SendARP`) APIs all
  work. Raw SYN probes, packet-level OS fingerprints and link-layer capture need a driver, so
  don't promise them.

### Discovery methods and platform limits

- Interfaces and routes start a scan, even with an empty project. Show the source interface
  and VPN/virtual adapters. A mapped prefix or route is not permission to scan. Routed or
  VLAN targets are reached only through existing routes; never configure routes or devices.
- Neighbour caches may be stale, and ARP cannot see the MAC of a routed host. A missing ping
  reply must not stop TCP or service checks. Refused, timed out, denied and not tested are
  different outcomes, and silence does not prove a device is offline.
- DNS, mDNS/DNS-SD, SSDP and OUI give hints with a source and time, not proof of identity. A
  network card's vendor is not the device's type or maker. Don't count copies of one name
  from DNS and mDNS as independent confirmation.
- mdns-sd 0.21.6: creating the daemon doesn't mean multicast works, so read `monitor()`
  events. A full browse channel ends the subscription, so an unexpected disconnect means
  partial coverage. Stop subscriptions at run end, with bounded retries and waits.
- No maintained safe neighbour-table/ICMP wrapper has been qualified: crossnet 0.1.1 is early
  stage, and net-lattice is broad and still changing. `windows`/`windows-sys` bindings are not
  safe wrappers. Don't hide FFI in a helper crate or weaken `unsafe_code = deny`.
- macOS 15+ local-network privacy applies to ordinary sockets. `cargo run` or Terminal runs
  don't show packaged-app behaviour, so check a normally launched app with access denied and
  then granted.
- SNMP, LLDP, controller APIs and ONVIF are later, optional integrations. A forwarding-table
  entry shows reachability, not a direct cable.
- Treat replies as untrusted. Bound text and body size, parser depth, time and redirects;
  check each redirect's destination. Replies and AI output never widen scope, fetch
  arbitrary URLs or run commands.
- Scan cost grows with subnet size × port count, not host count. Limit simultaneous probes
  and new starts per second separately; don't create one task per host and port.

### Architecture

- One authoritative document stays in `AppState` on the UI thread. Background workers on an
  owned Tokio runtime do network I/O, file serialisation, parsing, import validation and
  inference. They send owned messages and read immutable snapshots.
- A cross-thread `Send` closure cannot capture `Rc<RefCell<AppState>>`. Slint's `spawn_local`
  is not a Tokio runtime. A started `spawn_blocking` task cannot be aborted. Drain a bounded
  receiver from the existing UI timer and reuse `view::sync`, rather than posting one closure
  per reply. Coalesce progress (about 5–10 UI batches a second is a starting point to measure).
- Stop needs its own control path, so a full result queue cannot block it. Supervise every
  owned task and send one terminal outcome. Apply all accepted results before that outcome.
  Ignore late results using run and document generations.
- Measure reconciliation, snapshot and projection cost at about 250 devices, counting cards and
  services, before moving ownership or adding an actor, store, index or graph framework.
- Identity matching order:
  1. a user-confirmed association in the same network context;
  2. a consistent stable identifier from a trusted source;
  3. compatible MAC evidence on the same link, checked against conflicts and history;
  4. names, OUI, services and AI as hints only.

  DHCP changes can continue a card's history. IP reuse, MAC randomisation (e.g. Apple's
  rotating private Wi-Fi addresses) and overlapping VPN prefixes go to review. Never infer
  VM/container ownership from IP or vendor.
- IDs are document-local integers with a saved allocator that never reuses them. Map them to
  bounded `i32` Slint handles. UUIDs would only matter for file merging, which is out of scope.
- Field precedence: manual value, then supported observation, then labelled inference, then
  unknown. A conflict shows "your value" beside "detected value" with its source and time, as
  a persistent marker rather than repeated dialogs. "Keep mine" acknowledges that particular
  value, so a different detected value alerts again. "Use detected" counts as a manual edit.

### Map

- Keep three relationships separate: device ownership, network connections (observed,
  inferred or manual, with their evidence) and a display forest that references card IDs.
  Snapping, reparenting, branch moves and tree lines use the display forest. Network evidence
  may contain loops; the display forest may not.
- Save display parentage, explicit root order, ordered tiers and left/right siblings,
  committed icon-centre coordinates in absolute map units, placement mode and layout
  parameters. Coordinates restore the picture; tree order drives an explicit layout action.
- A drop commits parentage, order and coordinates together, and Cancel restores the previous
  state. Saving during a drag or animation stores the last committed state. Never save
  springs, hover, pointer capture or generated SVG paths.
- Restore the view centre (in map units) and zoom after the viewport exists; the initial fit
  must not override a saved view.
- Keep Slint and the current layout, snapping and motion code. petgraph, rstar, ELK, egui with
  egui-snarl, and Iced were compared; none justifies an addition or a rewrite.

### Project files

- Serde + serde_json; one versioned `.rhopper.json` document. No SQLite alongside it.
- Open: parse into a separate candidate, bound size, counts and strings, validate references,
  ownership, display cycles, placement and finite coordinates, and apply supported
  migrations. Only after handling unsaved changes does it replace the project. Reject unknown
  newer versions, remote references and secret-bearing fields. Saved runs reopen as history.
- Save: write a complete sibling temporary file, then replace via one vetted route. Write
  one file at a time per destination. Each receipt carries document generation, revision and
  path, so an old save cannot clear newer edits or rename a newly opened project. Report
  external changes and competing writers before replacing.
- Embed custom icons with validated content and built-in fallbacks; never depend on an
  absolute development path. Save the AI model's identity, not the weights.

### Candidate crates

| Need | Candidate (inspected) | Notes |
| --- | --- | --- |
| Runtime, channels, cancellation | tokio 1.54.0, tokio-util 0.7.20 | MIT, MSRV 1.85. Minimal features, not `full`. Bounded `mpsc`, `watch` for progress, `JoinSet`, `CancellationToken`. |
| Interfaces and gateways | netdev 0.46.3 | MIT, three OSes. Only if netscli-core lacks it. Doesn't pick per-destination routes. |
| mDNS/DNS-SD | mdns-sd 0.21.6 | MIT/Apache, MSRV 1.71, in process. Not zeroconf (needs Bonjour/Avahi). |
| DNS | hickory-resolver 0.26.3 | MSRV 1.88. System resolver policy; no default public DNS/DoH. Check split DNS over VPN. |
| JSON | serde 1.0.229, serde_json 1.0.150 | Needs separate bounds and domain validation. |
| Safe save | **Chosen: atomic-write-file 0.3.1** (BSD-3-Clause, MSRV 1.85; deps MIT/Apache) | Temp file in the same folder, `sync_all`, then rename; on Unix also `fsync`s the folder and keeps mode and owner (best effort). Dropping an uncommitted file deletes the temp and leaves the old file. It replaces a symlink rather than its target, so canonicalise the path first. Windows: no folder sync and permissions come from the folder; rename fails if another program holds the file open. Its 21 tests pass on Windows. Not tempfile: `persist` doesn't sync or keep the mode. Don't promise power-loss safety. |
| File dialogs | **Chosen: rfd 0.17.2** with `default-features = false`; `gtk3` on Linux (all 60 packages MIT/Apache) | Without `gtk3`, Linux uses the portal with a `zenity` fallback, which runs another program; with it, the zenity module isn't compiled, but the system GTK3 library is required. Use `AsyncFileDialog` on `slint::spawn_local`, parented through Slint's default `raw-window-handle-06`. Builds on Windows; no dialog has been shown yet. |
| Preferences path | directories 6.0.0 | Moved to Codeberg; maintenance not confirmed. |
| ICMP | surge-ping 0.9.1 | Permissions differ by OS; no unprivileged Windows support established. Native Windows ICMP is an alternative. |
| SSDP, OUI | ssdp-client 2.1.0; mac_oui 0.4.11 / oui-data 0.2.1 | Data licence and freshness unchecked. |
| Not now | rusqlite, SQLx, uuid, pnet/pcap, snmp2/async-snmp, oxvif | No current need, or a driver requirement. |

### AI model

- **First candidate, measured and failing: Qwen3-0.6B Q8_0 GGUF on Candle 0.11.0's
  `quantized_qwen3`.** Artifact: `Qwen/Qwen3-0.6B-GGUF` revision `23749fe`, 639,446,688
  bytes, SHA-256 `9465e63a…b031`, Apache-2.0; tokenizer from `Qwen/Qwen3-0.6B` `c1899de`.
  Candle's own `quantized-qwen3` example built on Rust 1.99.1 and ran offline from local
  files on the user's Ryzen 5 7600X with 3 threads; the runtime graph's licences are all
  permissive, and tokenizers' `onig` feature compiles a C library in.
  - Speed: needs an AVX2 build (`target-cpu=x86-64-v3`). Without
    it a 188-token prompt took 19 s; with it 4.9 s including a 1.2 s load (about 190 prompt
    and 67 generated tokens a second).
  - RAM: about 1.65 GB peak working set and 2.3 GB private bytes for the model process
    alone.
  - Usefulness, zero-shot JSON prompt, four cases: "Router" with High confidence for a
    Windows RDP PC, a Linux xrdp host and an unmistakable Brother printer; three of four
    replies were not valid JSON. With a worked example it copied the example's camera and
    invented evidence. Only "Unknown"/"Linux" OS values were right.
- **The user's choice, to qualify next: Qwen3-4B Q4_K_M GGUF**, installed by Install AI
  rather than bundled. Artifact: `Qwen/Qwen3-4B-GGUF` revision `bc64014`, 2,497,280,256
  bytes, SHA-256 `7485fe6f…fdf5`, Apache-2.0; tokenizer from `Qwen/Qwen3-4B` `1cfa9a7`
  (pinned in `src/ai.rs`). Install AI downloads and checks it on the user's PC.
- **In the app (Candle 0.11 `quantized_qwen3`, greedy, thinking off, schema in the prompt,
  no constrained decoding).** On the user's PC (Ryzen 5 7600X, RTX 5060 8 GB, sm_120),
  Qwen3-0.6B Q8_0 answers the identification prompt and sample: 154 s on the CPU in a plain
  release build, 24.5 s with `target-cpu=x86-64-v3`, 8.9 s on the GPU (CUDA 13.3, warm;
  the first run also JIT-compiles kernels). Qwen3-4B not yet timed or scored.
- **Runtime now Candle 0.11**, shared by the app and evaluation tool: direct GGUF and
  Hugging Face safetensors loading with local tokenizers. Both formats now use one small,
  synchronous inference path. The timings above need re-measuring in the integrated path.
- Earlier alternatives (a replacement, never a second model):
  - SmolLM2-360M-Instruct Q8_0 (about 386 MB).
  - DeBERTa-v3-xsmall zero-shot quantized ONNX (about 87 MB). It needs ort 2.0.0-rc.15
    (MSRV 1.92; official ONNX Runtime builds can send telemetry by default) or tract 0.23.9
    (operator support depends on the model).
  - llama.cpp is acceptable only as an in-process library, but the `llama-cpp-2` binding
    carries an unsafety disclaimer.
  - MiniLM embeddings support search, not identification.
  - External runners (Ollama, servers) are excluded.
- Flow:
  1. Send a bounded evidence snapshot with evidence IDs, source and age.
  2. Allow only named fields and categories plus Unknown.
  3. Reject invented evidence IDs, malformed or oversized output, and instructions hidden in
     banners.
  4. Cache by evidence fingerprint and model version.
  5. Before applying, recheck generation, evidence, model, policy and whether the field has
     become manual.
- Example: hostname `DESKTOP-13123` plus an RDP reply on TCP 3389 suggests a likely Windows
  PC. Linux xrdp also speaks RDP, and neither clue gives OS version or manufacturer. Keep
  identity confidence, suggestion score and link certainty separate. An unevaluated score
  must be labelled "uncalibrated".
- Evaluate against a simple explainable baseline using held-out cases: Linux RDP, renamed or
  generic hosts, shared NICs, routed hosts without a MAC, spoofed banners and conflicting
  evidence. Report false positives and abstentions for each field. Automation needs a
  stricter bar.

## Agreed decisions

Agreed with the user on 20 October 2026. Later tasks meet these; changing one needs the user's
approval. Scan capabilities are as requested: BE-0007 and BE-0008 establish what netscli-core
supports, and anything unsupported is shown as unavailable, never faked.

### Workload and test machine

- Acceptance workload: 250 devices, about 400 cards (most devices one, NAS/servers two to
  four), 30 VMs/containers, about 2,000 services (≈8 per device), 100 saved runs and about
  50,000 log lines. It must stay responsive, also during a scan, and Stop and closing must
  not hang; there are no fixed hardware, memory or timing figures.
- Viewports: 1366×768 at 100% (minimum), 1920×1080 at 125% and 2560×1440 at 150%.
- Test machine: the user's Windows 11 Pro PC is the target for every test that needs one.
  Linux and macOS machines are still to be named (BLK-009).

### Scan scope and methods

- Targets and exclusions: comma- or line-separated IPv4 addresses, CIDR blocks from /16 to
  /32 (including /31 and /32) and ranges (`10.0.5.10-10.0.5.40`, short form
  `192.168.2.1-50`). At most 65,536 target addresses per run; invalid or oversized input is
  rejected before Start.
- TCP ports: lists and ranges (`22,80,443,8000-8100`) with Common and All (1–65535) presets.
- Source interface: chosen per target from the OS routing table and shown before Start; the
  user can pin one. VPN and virtual adapters are listed and labelled, and their subnets are
  never suggested as targets automatically.
- Named UDP methods: mDNS/DNS-SD (5353) and SSDP/UPnP (1900) only. NetBIOS, DNS and NTP are
  not offered, and there is no blanket UDP sweep.
- Identification depth: Basic (open ports), Standard (adds banners and service probes; the
  default) and Deep (every netscli-core probe).
- Pace presets: Gentle (32 probes in flight, 25 new a second, 3 s timeout, 2 retries), Normal
  (the default: 128, 100/s, 1.5 s, 1 retry) and Fast (512, 400/s, 1 s, 1 retry). No numeric
  advanced controls.
- A method the OS denies is shown as unavailable, with its reason, before Start and skipped;
  the rest of the run goes ahead. The app never asks for elevation.

### Map placement and links

- One map-wide Automatic / Free toggle in the canvas controls, Automatic by default and saved
  with the project. In Automatic, a drag snaps into the tree as now. In Free, a drop leaves
  the whole branch where it lands, with simple connectors. Free → Automatic is an explicit
  layout action and rearranges with animation; Automatic → Free moves nothing. Arrange always
  tidies the whole map.
- Line pattern stays the group's choice. Network links: observed at full strength, inferred
  faded, manual with a small square end mark. Organisational (display-only) lines are thin,
  light grey and unmarked. A network link the display forest cannot show is a thin curved
  line. The map key explains all four.

### AI

- Inference runs in the background and leaves the app responsive; it is measured on the
  user's PC, with no fixed hardware, memory or timing figures.
- Fields: type (the app's device kinds or Unknown), OS family (Windows, macOS, Linux, iOS,
  Android, network/embedded or Unknown) and a description of at most 120 characters citing
  evidence IDs. Below the confidence cut-off a field stays Unknown.
- Quality on held-out cases: shown suggestions are right at least 90% of the time for type
  and 85% for OS family, and beat the rule-based baseline; descriptions contain no invented
  facts. Abstaining is allowed.
- Automation: both controls are off by default, set per network context and never imported
  as on. They act only above a cut-off measured at 97% precision or better, only on empty,
  non-manual fields and on unambiguous, non-duplicate new hosts. Each automatic change is
  labelled AI with its model and run, and can be undone per run or per field; an auto-added
  device can be removed.

### File limits and conflicts

- Open refuses files over 256 MB, more than 10,000 devices, 50,000 cards or 200,000
  services, names over 4 KB, notes over 64 KB, icons over 256 KB each or more than 500 icons.
- Schema: one integer version. Older supported versions are migrated; newer ones are refused.
- Before replacing, compare size, modified time and hash with the last Open or Save; if the
  file changed, ask Overwrite / Save As / Cancel. No lock file.
- A symlinked project is saved through to its target. Unix keeps the file mode; Windows takes
  permissions from the folder, a documented limit.
- History is never deleted automatically: warn once the saved file passes 100 MB; past
  256 MB the save fails clearly and the previous file stays.

### Projects and review

These extend the product brief's wording, which needs updating first (BLK-010).

- Launch opens an empty, unsaved project. New, Open, Save and Save As sit in the header with
  the project name and an unsaved mark; the sample network leaves the production path. The
  empty map points to Scan and Add device.
- Remove device replaces Forget device: after confirmation it removes the device with its
  cards and guests from the project and keeps its scan history; a later scan may find it
  again as New.
- Ignore is saved and applies to the same identity in the same network context on later
  runs. Ignored hosts have an Ignored filter, are never auto-added and can be un-ignored.
- Result confidence is Strong, Some or Weak, worked out from the evidence (a MAC on the same
  link plus a name is Strong; an IP alone is Weak), with the evidence shown on hover. No
  percentage until one is measured.
- Restrained choices, not asked: the AI automation controls live in the AI settings window, per
  network context; Arrange and the Automatic/Free toggle replace Reset layout beside Fit.

## IDs and maintenance

| Item | Format | Next unused ID |
| --- | --- | --- |
| Milestone | `M-001` | `M-021` |
| Task or backlog need | `BE-0001` | `BE-0111` |
| Blocker (in the backlog) | `BLK-001` | `BLK-015` |

Take the next ID from this table and advance it in the same edit. Never reuse an ID, even after
its item is deleted. Splitting a task keeps the original ID for what remains.

`Depends on` lists the tasks that must be done first; `—` means none remain. Milestone entry
dependencies apply to every task in the milestone. Rows are in execution order. A task is
ready when its `Depends on` is `—` and no backlog blocker holds it. Delete a done task, remove
its ID from every `Depends on`, and delete a milestone once it is empty. Keep the Findings
section true: correct or delete a finding when a task proves it wrong or makes it obsolete.

If a required capability cannot meet the contract, add a backlog blocker with the failed check
and alternatives and ask for the specific decision. Do not silently reduce the first working
backend to one OS, omit AI, build a custom scanner or add recovery storage.

## Delivery sequence

Project foundation before real scanning, reliable updates before AI, then integrated
acceptance. Early qualification exposes scanner, file and model blockers before work depends
on them. M-002, M-003 and independent map or model work can proceed in parallel (each agent
in its own worktree). A failed model check blocks the AI milestones but not inventory work.

| Milestone | Outcome | Entry dependencies |
| --- | --- | --- |
| [M-002](#m-002-qualify-critical-dependencies) | Qualified scanner, file lifecycle and model/runtime | None |
| [M-003](#m-003-durable-inventory-and-network-contexts) | Owned metadata, durable identity, scoped endpoints | None |
| [M-004](#m-004-observations-and-manual-authority) | Provenance, manual precedence, truthful status | M-003 |
| [M-005](#m-005-separate-map-relationships) | Ownership, connectivity and display forest separated | M-003 |
| [M-006](#m-006-committed-placement-and-view-state) | Automatic/free placement and restorable view | M-005 |
| [M-007](#m-007-complete-versioned-project-format) | Complete validated JSON document | M-004, M-006 |
| [M-008](#m-008-native-open-save-and-save-as) | Native file lifecycle and dirty handling | M-007, BE-0010 |
| [M-009](#m-009-project-foundation-checkpoint) | Useful manual project before live scanning | M-008 |
| [M-010](#m-010-explicit-scan-scope-and-capabilities) | Validated scope and honest capability preview | M-009, BE-0007–BE-0009 |
| [M-011](#m-011-background-run-lifecycle) | Responsive bounded run control | M-010 |
| [M-012](#m-012-connect-real-scanner-observations) | Real engine results replace simulation | M-011 |
| [M-013](#m-013-identity-continuity-and-admission) | Repeat runs reconcile safely | M-012 |
| [M-014](#m-014-real-discovery-review-workflow) | Existing review/edit/Add flow with real data | M-013 |
| [M-015](#m-015-reliable-updates-history-and-conflicts) | Saved, traceable updates preserve user work | M-014 |
| [M-016](#m-016-offline-ai-suggestions) | One model gives bounded reviewable suggestions | M-015, BE-0011 |
| [M-017](#m-017-explicit-ai-automation-controls) | Opt-in field filling and auto-add | M-016 |
| [M-018](#m-018-integrated-correctness-and-responsiveness) | Combined workflows withstand races and load | M-017 |
| [M-019](#m-019-self-contained-builds-on-three-platforms) | Runnable bundles with every required asset | M-018, M-002 |
| [M-020](#m-020-first-working-backend-acceptance) | Evidence-backed handover | M-019 |

## M-002: Qualify critical dependencies

Goal: find hard integration problems before committing to a scanner, save path or model
runtime. Use minimal real integration checks in the app or `tools/`, not disposable scripts.
Live checks need user-approved targets.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0007 | Confirm netscli-core 0.3.4 (`default-features = false`, `mdns`) builds and links on Linux and macOS with Rust 1.99.1, with no native capture library. | — |
| BE-0008 | Turn the static trace (Findings) into a per-OS method matrix confirmed on each OS. Qualify only necessary supplementary crates: unprivileged ICMP on Linux/macOS, the macOS neighbour table and per-destination routes/source interface. Bring the macOS MAC gap to the user as a decision if no safe in-process route exists. | BE-0007 |
| BE-0009 | Exercise incremental results, pause (the app withholds new host and ≤4,096-port calls; upstream has none), stop/close by dropping futures with task accounting, timeouts and queue pressure against the selected upstream API. Do not substitute a custom scanner or retry engine for missing control. | BE-0008 |
| BE-0010 | Confirm the chosen rfd (`gtk3` on Linux) and atomic-write-file on each OS: a dialog parented to the Slint window opens and cancels without blocking the UI (approved GUI check), GTK3 runs in process alongside Slint's Linux backend, and the Unix mode/owner and macOS metadata survive a replace. Failed and locked replacements are tested in BE-0041. | — |
| BE-0011 | Qualify one pretrained model with its tokenizer/template and CPU runtime: safety, toolchain fit, offline loading, licences, weight allowance and first usefulness/resource measurements. Qwen3-0.6B on Candle failed usefulness and RAM (Findings); qualify the user's choice, Qwen3-4B Q4_K_M (Findings), on the same cases on the CPU and with CUDA, recording its RAM and speed on the user's PC. | — |
| BE-0012 | Consolidate the qualified versions, features, assets and approvals; confirm each M-002 blocker in the backlog is resolved or still honestly open. | BE-0007, BE-0008, BE-0009, BE-0010, BE-0011 |

Exit: every selected dependency has a compliant route. A successful build alone does not
qualify OS permissions, file integrity, scanner cancellation or AI usefulness.

## M-003: Durable inventory and network contexts

Goal: extend the current pure model rather than create a second inventory.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0013 | Replace static-only metadata with owned runtime values while preserving existing device/card/edit behaviour. | — |
| BE-0014 | Non-reused device/card/run/result keys with a saved allocator; bounded Slint handles kept separate. Check exhaustion and save/reopen continuity. | BE-0013 |
| BE-0015 | Named network contexts and scoped address associations; the card editor's uniqueness rule and hint become per context. Equal private IPs in different contexts stay distinct; a missing remote MAC or prefix is valid. | BE-0014 |
| BE-0016 | Keep device → card and VM/container ownership with validated references; reject cycles. Move network card (guests following) uses the same validation. Never infer guest ownership from vendor or IP. | BE-0015 |
| BE-0017 | Service endpoints keyed by context/address/transport/port with typed outcomes (not tested, denied, no response…). TCP and UDP on one port stay distinct; the edit page's manual ports gain a transport (TCP by default) and keep their description. | BE-0015 |
| BE-0018 | Route manual mutations through model validation; test duplicate keys, invalid references, address edits and ownership changes. | BE-0016, BE-0017 |

Exit: runtime data fits the existing model, stays free of Slint, and cannot bypass invariants
through an unchecked editor or imported record.

## M-004: Observations and manual authority

Goal: make evidence and user intent distinguishable before network results arrive.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0019 | Store field/source/context/time evidence separately from manual values and suggestions; keep meaningful unavailable/unknown states. | BE-0018 |
| BE-0020 | Apply manual → observation → labelled inference → unknown precedence through shared edit/reconciliation logic. Clearing an override falls back explicitly. | BE-0019 |
| BE-0021 | Record last success separately from the latest method outcome; show the last-observed time in place of the relative "At last scan"/"n minutes ago", without treating a timeout or old evidence as Offline; a card outside the scanned scope stays Not seen. | BE-0019 |
| BE-0022 | Represent manual/detected conflicts with both values, source/time and acknowledgement keyed to the differing value. | BE-0020 |
| BE-0023 | Test manual edits during delayed observations, conflict acknowledgement/reappearance, missing data and transport-specific outcomes, with no live traffic. | BE-0021, BE-0022 |

Exit: neither collection nor AI can silently overwrite manual fields; a failed method cannot
erase facts or invent fresh reachability.

## M-005: Separate map relationships

Goal: reuse tree and arrangement behaviour without coupling it to network claims.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0024 | Separate evidence-backed network relationships from ownership and a saved display forest over existing card IDs. Keep network edges the forest cannot show. | BE-0018 |
| BE-0025 | Drive traversal, `Arrangement` and snapping from display parentage, explicit root order and ordered tiers/siblings. | BE-0024 |
| BE-0026 | Make display reparenting one validated presentation edit (no `Network::connect`); reject display cycles and multiple parents. | BE-0025 |
| BE-0027 | Distinguish observed, inferred, manual and organisational links on the canvas and in its key. | BE-0026 |
| BE-0028 | Test a branch whose display parent differs from its network uplink, multi-card devices and guest ownership; changing one relationship leaves the others intact. | BE-0027 |

Exit: dragging an icon under another cannot create a network connection or change ownership.

## M-006: Committed placement and view state

Goal: keep the user's map across edits, scans and file operations.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0029 | Store committed icon centres, arrangement, placement mode and layout parameters in Rust; keep pointer previews and spring state transient. | BE-0028 |
| BE-0030 | Free positioning with simple derived connectors; one committed move for a displayed parent and all descendants; Cancel restores the branch. | BE-0029 |
| BE-0031 | Keep automatic tree/snap/order/animation; add the agreed Automatic/Free toggle and an Arrange action beside zoom and Fit, replacing Reset layout. | BE-0030 |
| BE-0032 | Place new cards in a clear area without moving established positions; refresh and ordinary edits keep placement and display parentage. | BE-0031 |
| BE-0033 | Capture view centre, zoom, panel widths, list filter and search, collapsed card groups, selection and the View menu's display options into the document; restore after the viewport exists without the initial fit overriding them. | BE-0032 |

Exit: branch offsets survive movement, and a snapshot taken mid-drag holds the last committed
placement.

## M-007: Complete versioned project format

Goal: the complete save contract before native file controls.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0034 | Define the `.rhopper.json` schema/version and complete-state checklist (IDs, contexts, inventory, cards with name/icon, guests, manual ports, roles with icons/groups/detected-icon choice and deleted standard roles, groups with colour/line style, relationships, layout/view, manual data, ignored identities, scan settings/results/history/logs, AI state/model identity). | BE-0023, BE-0033 |
| BE-0035 | Embed custom SVG icons chosen from the `icons` folder with stable identifiers, validated size/content and built-in fallbacks, so a project opens without that folder; no absolute paths. | BE-0034 |
| BE-0036 | Serialise immutable revisioned snapshots off the UI thread; keep all history while bounding working buffers; clear failure past an approved file limit. | BE-0035 |
| BE-0037 | Parse Open into a separate candidate and validate schema, counts/strings, references, ownership/display cycles, placement and finite coordinates/zoom before installing it. | BE-0036 |
| BE-0038 | Supported migrations; reject unknown newer versions and failed migrations without touching the current project. | BE-0037 |
| BE-0039 | Round-trip complete state; reject corrupt/hostile files, remote references and secret-bearing fields. | BE-0038 |

Exit: every saved category has schema coverage and round-trip tests; invalid input never
replaces the active project.

## M-008: Native Open, Save and Save As

Goal: explicit saves that protect the previous file.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0040 | Add New, Open, Save and Save As to the header and connect the qualified native dialogs; cancelling a picker changes neither document nor path. | BE-0039, BE-0010 |
| BE-0041 | Write a complete sibling temporary file, commit via the qualified replacement route, keep the old file on failure and report errors visibly. | BE-0040 |
| BE-0042 | One writer per file; detect external changes before replacing, as agreed. Test races and access failures. | BE-0041 |
| BE-0043 | Serialise writes per destination with generation/revision/path receipts; Save As changes the active path only on success. | BE-0042 |
| BE-0044 | Project name and unsaved mark in the header, replacing the "Sample data only" status; Save/Discard/Cancel before close, New or Open; invalid Open keeps the project; a document switch invalidates old callbacks. | BE-0043 |
| BE-0045 | Restore full state with motion at rest and old observations as last-known; a saved active run reopens as history; nothing starts traffic or AI on Open. | BE-0044 |

Exit: save failure, picker cancel, external edits and stale receipts all keep the right file
and project.

## M-009: Project foundation checkpoint

Goal: a useful persistent manual project before real scanning.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0046 | Launch opens an empty, unsaved project whose empty map points to Scan and Add device; it supports manual devices, cards, contexts, roles/groups and icons without sample data. | BE-0045 |
| BE-0107 | Replace Forget device with the agreed Remove device: confirmation, removes the device with its cards and guests, keeps its scan history, saved on the next Save. Test guests, selection and undo by not saving. | BE-0046 |
| BE-0047 | List/map selection, search, filters, editors, guest/card edits, card moves and custom roles all work against the one document, keeping drafts and selection. | BE-0107 |
| BE-0048 | Demonstrate save/reopen, free branch moves and automatic arrangement with several cards and contexts; report source checks separately from approved visual checks. | BE-0047 |
| BE-0049 | Remove simulation-only persistence assumptions; check the foundation against the product brief. | BE-0048 |

## M-010: Explicit scan scope and capabilities

Goal: truthful run settings before any network traffic.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0050 | Enumerate interfaces, addresses and routes through qualified APIs so an empty project can scan; the discovery page's Network choice offers these, not mapped prefixes; distinguish VPN/virtual adapters and network contexts. | BE-0049, BE-0008 |
| BE-0051 | Validate IPv4 addresses/CIDRs/exclusions lazily, including /31 and /32; show the source interface; mapped prefixes never grant permission. | BE-0050 |
| BE-0052 | Replace the Ping sweep / Port scan / Service detection checkboxes with the agreed TCP port field and presets, named UDP methods, depth and pace; TCP/service checks run without a ping reply; no blanket UDP sweep. | BE-0051 |
| BE-0053 | Show available/unavailable methods with reasons, estimated work and limits before Start; reject invalid or oversized work without probing. | BE-0052, BE-0009 |
| BE-0054 | Freeze context, targets, exclusions, ports, methods and limits into an immutable run configuration. | BE-0053 |

## M-011: Background run lifecycle

Goal: real run states without blocking the event loop.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0055 | One active scan with its runtime/tasks off the UI thread, sending owned data into the existing document. | BE-0054 |
| BE-0056 | Bounded non-blocking command/result flow, coalesced progress and UI drain budgets; Stop independent of a full queue; report coverage loss rather than dropping findings. | BE-0055 |
| BE-0057 | Preparing/running/pausing/paused/cancelling and completed/cancelled/failed/partial outcomes, using upstream pause/cancel rather than a new scheduler. | BE-0056, BE-0009 |
| BE-0058 | Reject stale run/generation events; apply accepted observations before the terminal outcome; Stop keeps valid partial results. | BE-0057 |
| BE-0059 | Runs survive navigation, with the header showing Scanning…; close or confirmed project replacement cancels and accounts for owned work. | BE-0058, BE-0044 |
| BE-0060 | Test monotonic elapsed time, stage counters, indeterminate work, partial method failure, full queues and bounded shutdown. | BE-0059 |

## M-012: Connect real scanner observations

Goal: replace fabricated answers with the qualified library's probes.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0061 | Integrate the qualified netscli-core with capture off and only approved features; remove the fake status refresh and timer answers from the real Start path. | BE-0060, BE-0007 |
| BE-0062 | Convert incremental TCP and identification results into scoped endpoint observations with typed outcomes, provenance and timestamps; never copy whole fixture devices. | BE-0061 |
| BE-0063 | Add qualified named UDP/discovery methods (and supplementary crates only where the engine lacks them); stop subscriptions at run end; record overload/disconnect/permission failures. Method chips name only offered methods (drop SNMP, ONVIF and LLDP). | BE-0062, BE-0008 |
| BE-0064 | Bound untrusted reply sizes, parser depth, timeouts and description URL redirects; redact credentials; replies cannot run commands or widen targets. | BE-0063 |
| BE-0065 | Test local and routed/no-MAC cases, denied optional methods and partial failure with deterministic fixtures; a live smoke run only with approved targets. | BE-0064 |

## M-013: Identity continuity and admission

Goal: repeat scans update the right card and device without overriding the user.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0066 | Reconcile scoped evidence in the agreed matching order; IP, name, OUI or AI alone never establish identity; ambiguous candidates stay in review. A manual card move between devices is a user-confirmed association later runs honour. | BE-0065, BE-0023 |
| BE-0067 | Keep identity across DHCP changes; isolate IP reuse, MAC randomisation, overlapping contexts and conflicting evidence. | BE-0066 |
| BE-0068 | Upsert repeated findings and services under stable keys, including multi-card devices; missing MAC/prefix stays unknown. | BE-0067 |
| BE-0069 | Revalidate review/Add against the latest revision and selected endpoint/context; admit only supported cards; reject stale candidates. | BE-0068 |
| BE-0070 | Update mapped devices through shared reconciliation without touching manual fields, guest ownership or placement. | BE-0069 |
| BE-0071 | Test repeat-run deduplication, changed/reused IPs, routed hosts, multi-card ownership and delayed events, including the fixture-copy and newcomer-duplication risks. | BE-0070 |

## M-014: Real discovery review workflow

Goal: reuse existing pages and forms for real results.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0072 | Results, counts/filters, stable result selection and details driven by real evidence, labelled with method, age, the agreed Strong/Some/Weak confidence and possible duplicates ("Same MAC as"). | BE-0071 |
| BE-0073 | Keep pre-Add name/role/icon/notes editing and Add/leave-unadded; moving a found card onto a mapped device resolves a duplicate; Ignore is saved per identity and context, with an Ignored filter and a way to stop ignoring. Pre-Add edits become protected manual values. | BE-0072 |
| BE-0074 | Reuse mapped-device editing and Show on map; discovery and inventory share the device; drafts survive result batches. | BE-0073 |
| BE-0075 | Real progress, pause/in-flight, terminal/partial states and per-method issues in the existing controls and log; remove "simulated" wording from the real path. | BE-0074 |
| BE-0076 | Check start/stop/navigation, review/edit/Add, keyboard focus and translations; record approved GUI checks separately from source checks. | BE-0075 |

## M-015: Reliable updates, history and conflicts

Goal: trustworthy repeated scan → update → save cycles before AI.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0077 | Conflict comparison in device/card details with Keep my value / Use detected value; deduplicate repeats; alert again on a changed value. | BE-0076, BE-0022 |
| BE-0078 | Keep run history, address/evidence changes, issues and logs without automatic deletion; bounded visible windows only. | BE-0077 |
| BE-0079 | Save real run settings/results/history with absolute times; reopened runs are historical. | BE-0078, BE-0045 |
| BE-0080 | Test editing/moving during scans, saving while results arrive, Save As/Open with pending work and repeated refresh. | BE-0079 |
| BE-0081 | Demonstrate manual scan → review → Add/update → conflict → save/reopen with no AI and no automatic probes. | BE-0080 |

## M-016: Offline AI suggestions

Goal: one qualified model useful inside the existing discovery and details workflows.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0082 | Load an installed GGUF file or Hugging Face safetensors model folder with Candle (using embedded GGUF metadata or adjacent Hugging Face configuration and tokenizer), with the schema in the prompt as the evaluation tool does, pinned identity and notices; the AI settings window's model list picks it; bound threads, admission, context, output and shutdown. | BE-0081, BE-0011 |
| BE-0109 | Harden Install AI: check free space before starting, resume a broken download, fail a stalled read, remove an installed model; test against a local server, not the internet. | BE-0082 |
| BE-0083 | Bounded evidence snapshot with IDs/source/age; validate output fields/categories, evidence references, reasons and Unknown; treat banners and output as untrusted. | BE-0082 |
| BE-0084 | Show type/OS-family/description suggestions with evidence, reasons and labelled confidence; explicit accept/edit/reject; suggestions never become observations. | BE-0083 |
| BE-0085 | Evaluate held-out cases against the evidence baseline; measure per-field false positives/abstention, peak RAM, CPU, cold load and latency on the user's PC. | BE-0084 |
| BE-0086 | Discard stale generation/evidence/model/policy results; protect edits made during inference; missing or corrupt assets leave manual workflows working, with no automatic download or substitute. | BE-0085 |

## M-017: Explicit AI automation controls

Goal: the agreed opt-ins without giving AI authority over user data.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0087 | Enable the AI settings window's "Fill missing inferred fields" and "Automatically add new devices" switches, per network context, both off by default. | BE-0086 |
| BE-0088 | Apply missing-field suggestions only after revision/policy/manual checks, keeping source/model identity and a way to reverse them. | BE-0087 |
| BE-0089 | Auto-add only with valid scoped evidence, resolved identity and the agreed thresholds; ambiguous or possible-duplicate findings stay in review. | BE-0088 |
| BE-0090 | Save policy, suggestions, accepted provenance and model identity; Open runs nothing and does not trust imported consent. | BE-0089, BE-0079 |
| BE-0091 | Test on/off combinations, policy changes mid-inference, pre-Add edits, ambiguity, reversal and thresholds; if thresholds fail, keep automation blocked. | BE-0090, BE-0085 |

## M-018: Integrated correctness and responsiveness

Goal: combined behaviour under realistic load.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0092 | Cross-feature deterministic tests: hostile files/replies, ownership/display cycles, wrong-context identity, stale results, save receipts. | BE-0091 |
| BE-0093 | Stress queues and work limits; Stop stays responsive and late results are handled while AI or file work is busy; nothing collects after close. | BE-0092 |
| BE-0094 | Measure about 250 devices by real card/service/history counts, including animation, projection, save snapshots and AI memory; optimise only measured bottlenecks. | BE-0093 |
| BE-0095 | Keyboard/focus/error feedback, readable provenance and conflicts, no dead controls and complete translations across map, discovery, files and AI. | BE-0094 |
| BE-0096 | Remove remaining production simulation paths and the sample network (no demo mode); update the README. | BE-0095 |

## M-019: Self-contained builds on three platforms

Goal: runnable local app bundles with no extra installs. Not installers or a public release.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0097 | Windows, Linux and macOS bundles with native libraries, runtime icons and translations (the model is installed by Install AI, not bundled). | BE-0096, BE-0012 |
| BE-0098 | Audit bundled dependencies, load paths, model/data licences and notices; nothing relies on repository paths or an external scanner, driver or service. | BE-0097 |
| BE-0099 | Launch each bundle normally on a clean machine; check dialogs, writable locations and network permission deny/grant (macOS privacy, Linux UI/render dependencies). | BE-0098 |
| BE-0100 | Install AI on a clean machine, then inference with the internet off: no downloads but the requested install, no telemetry. | BE-0099 |
| BE-0101 | Reopen a complete project on all three OSes, including icons, ownership, placement/view, history and AI state; disclose platform limits. | BE-0100 |

## M-020: First working backend acceptance

Goal: close the roadmap with demonstrated behaviour.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0102 | Confirm machines, networks, local/routed/VLAN targets, ports/methods and live-test approval with the user. | BE-0101 |
| BE-0103 | On each OS: real local/routed IPv4 runs, chosen TCP ranges/named UDP/depth, pause/resume/stop, navigation, permission degradation and bounded close, keeping partial results. | BE-0102 |
| BE-0104 | Repeat-run identity, conflicts, manual precedence and map preservation; full Open/Save/Save As, failed-save and external-change handling; cross-OS reopening. | BE-0103 |
| BE-0105 | Offline AI suggestions and both opt-ins against the agreed criteria, including ambiguity, spoofed evidence and a missing model. | BE-0104, BE-0091 |
| BE-0106 | Final gates; evidence for source, GUI, live, file, AI and platform checks recorded separately; known limits documented. | BE-0105 |

Exit: every requirement evidenced on all three OSes. Unit tests alone do not complete the
roadmap.

## Evidence

Every task runs the ordered quality gate (`cargo tool checks gate --scope app`, plus
`--scope tools` when tooling changes). The gate shows source health; deterministic tests show
the logic they exercise; GUI checks show the interaction observed; a live run shows only its
approved targets and methods. Report what each piece of evidence does not establish. Keep
credentials and private network data out of the repository.

## Outside this roadmap

Not planned unless the user asks; record a genuine need in the backlog:

- Public installers, signing/notarisation, release pipelines and publication.
- IPv6 probing, managed physical topology (SNMP/LLDP/switch ports), authenticated vendor or
  router integrations, exports, semantic search and a history comparison view.
- Multiple models, training, model downloads, cloud sync, background monitoring, scheduled
  scans, device configuration, remote control and vulnerability testing.
- SQLite, file merging, extra named map views or a UI rewrite.
