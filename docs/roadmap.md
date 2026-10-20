# Roadmap: first working backend

Planning baseline: 20 October 2026. Status: **not started**.

Deliver a working Rust/Slint backend with complete project saving, real IPv4 scanning,
reliable inventory updates and one bundled offline CPU AI model on Windows, Linux and macOS.
This roadmap stops at a usable, locally runnable backend. Installer polish, public release,
publication and later product enhancements are outside this plan.

The user requested this task plan and a [blank backlog](.backlog.md). Creating these documents
does not authorise backend implementation, protected-file changes, live scans or publication.
The current [product guide](agents/product.md) still limits implementation to a prototype;
BE-0001 resolves that explicit blocker before backend code is changed.

## Authority and settled requirements

[backend-results.md](backend-results.md) is the outcome and takes precedence over older
backend proposals. Use the other documents for rationale, technical risks and source findings:

| Source | Use in this roadmap |
| --- | --- |
| [Outcome](backend-results.md) | Required behaviour and delivery order |
| [Review](backend-review.md) | Existing functionality to reuse and integration defects to address |
| [Architecture](backend-architecture.md) | One document, owned worker messages, provenance, identity and file lifecycle |
| [Discovery](backend-discovery.md) | Explicit scope, method coverage, failure semantics and untrusted responses |
| [Scanner engine](backend-scanner-engine.md) | Upstream netscli-core qualification and compatibility blockers |
| [Map persistence](backend-map-persistence.md) | Ownership/connectivity/display separation and committed placement |
| [Crates](backend-crates.md) | Candidate libraries and qualification checks; no dependency is preapproved |
| [Decisions](backend-decisions.md) | Earlier answers and questions, reconciled against the outcome |
| [AI and opportunities](backend-opportunities.md) | Offline inference, quality evaluation and deferred ideas |

Older references to separate crash recovery, bounded history deletion, secure credential
storage, an undecided extension, external custom-icon references or two models are superseded:

- Explicit Open, Save and Save As use `.rhopper.json`; no autosave or crash recovery.
- Save complete project and restorable application state, embed custom icons, and retain
  history/logs without automatic deletion. Bound active queues and parsers without silently
  pruning stored history. Runtime handles, hover, unfinished drags and animation frames are
  not restorable project state.
- Never persist credentials in documents, logs or a separate credential store. Obtain them
  for a particular operation only. Managed/authenticated integrations are not required here.
- One active project can contain multiple named network contexts. Open replaces it; no file merge.
- One bundled pretrained CPU model, approximately 700 MB maximum weights; no GPU, downloads,
  telemetry, self-training, external executable, helper service, server or capture-driver setup.
- Use stock upstream netscli-core through ordinary Cargo updates. No fork, copied scanner,
  local compatibility patch or replacement scanning engine.
- Manual scans only; selected IPv4 local and reachable routed/VLAN targets, selected TCP
  lists/ranges and named UDP protocols, with user-selected supported identification depth.
- Pause launches no new probes; in-flight work may finish. Navigation leaves the run active;
  Stop/close cancels collection and preserves valid partial results. Open never starts traffic.
- Manual values win over observations and AI. Conflicts show both values with source/time.
  Identity ambiguity stays in review. AI field filling and auto-add are separate controls,
  both off by default; manual review remains usable without AI.
- Preserve parent devices and their child interfaces, guest ownership, existing editing and
  discovery workflows. Keep network relationships separate from display organisation.
- Preserve automatic layout, ordering, snapping and animation; also deliver free positioning
  with simple connectors and whole-display-branch movement. Only an explicit layout action
  rearranges established placement. No Forget or Reset workflows are planned.

Technical version/compatibility statements below describe the local research snapshot, not a
fresh qualification. Reread upstream documentation and the actual dependency graph when
executing the relevant task.

## Findings and results carried into this plan

The backend files record a documentation/source-research exercise dated 20 October 2026.
The [source review](backend-review.md) identifies its baseline as `c6d389e`; later source
changes are outside that review. Its findings below are planning inputs, not a fresh audit
of the current checkout. The scanner research inspected downloaded published sources; an
upstream build attempt was cancelled and produced no compatibility result. No scanner,
persistence implementation, AI model, packaged GUI or live-network scenario was qualified.
The review did not run an application quality gate or establish a completion percentage.

### Existing functionality and the gaps it exposes

| Reviewed finding / result | Decision and consequence for implementation | Tasks |
| --- | --- | --- |
| Startup creates sample data in UI-owned `Rc<RefCell<AppState>>`; the pure network model has no Slint dependency. `view::sync` retains `VecModel`s and updates changed rows. | Extend that authority and projection path. Workers return owned data; do not introduce a background inventory plus an editable UI copy. | BE-0013–BE-0018, BE-0055–BE-0056 |
| Map nodes are interface `CardId`s, with parent-device labels/icons; devices can own multiple interfaces and nested VMs/containers. | Preserve parent/child ownership and guest relationships. A host with two observed addresses is not automatically two devices, and hosting does not prove a network uplink. | BE-0015–BE-0018, BE-0024–BE-0028 |
| Existing inline/edit-page forms, custom roles/groups, discovery filters, Add/Ignore and Show on map already work with mock data. New findings allow name/role/icon/notes editing before Add; mapped findings reuse the device editor. | Reuse these workflows, including protected pre-admission edits, selection and drafts. Role groups are presentation categories, not VLAN contexts. | BE-0046–BE-0049, BE-0072–BE-0076 |
| Start refreshes all mock statuses, builds mapped answers plus fixed newcomers, then reveals them using a 150 ms timer over roughly forty steps. Methods/confidence and elapsed time are fabricated. | Replace answer production and progress semantics on the real path. No successful live result, throughput or elapsed-time behaviour can be inferred from the simulation. | BE-0057–BE-0062, BE-0075, BE-0096 |
| Runtime metadata is `Option<&'static str>`; a card has one optional IPv4/prefix and gateway. Services are keyed by port number alone. | Own runtime strings, scope address evidence, permit unknown remote prefix/MAC, and distinguish transport plus port and typed collection outcomes. IPv6 probing remains deferred. | BE-0013, BE-0015, BE-0017, BE-0019 |
| Address uniqueness is global; discovery matches MAC+IP, treats changed-IP/same-MAC as a possible duplicate and missing MAC as new. IDs use current maximum plus one. | Persist non-reused keys and allocator state, keep Slint handles separate, and reconcile scoped evidence before admission. IP/name/AI alone cannot transfer identity. | BE-0014–BE-0015, BE-0066–BE-0071 |
| Fixed newcomers may overlap mapped answers on the next simulated run; sorting does not deduplicate them. | Cover repeated findings and repeated admissions with stable-key upserts. This is a static risk, not a reproduced GUI failure. | BE-0068, BE-0071 |
| `Found::detect` selects a card but retains the whole fixture device; Add copies it, and insertion connects the first card without rechecking address conflicts. | Admit only supported endpoints/cards and revalidate against intervening edits, target context and the latest document revision. Do not copy off-target interfaces from a fixture. | BE-0062, BE-0069, BE-0071 |
| `Network::new` trusts its inputs; layout begins at roots, so an imported rootless cycle would receive no positions. Mutable access can bypass address validation. | Validate manual changes, imported references, ownership and display structure through shared model rules. Layout depth guards are not import validation. | BE-0018, BE-0026, BE-0037–BE-0039 |
| Stop becomes Finished; results/actions use list indices; status and relative last-seen are device-wide. | Add stable run/result keys, document generations, absolute observation times and distinct cancelled/failed/partial outcomes. Preserve prior evidence after failed methods. | BE-0014, BE-0021, BE-0057–BE-0060 |
| Target options come from mapped prefixes; every update/motion tick reconstructs view vectors. | Bootstrap from actual interfaces/routes even with no inventory. Batch results and measure card/service/history workloads before adding caches or indexes. | BE-0050–BE-0054, BE-0056, BE-0094 |

These findings come from [What exists and integration gaps](backend-review.md#what-exists).
The existing tests cover prototype model/edit/discovery/layout/icon/translation logic; they
do not establish persistence, real cancellation, network coverage or model accuracy.

### Decisions reconciled with earlier proposals

The outcome settles requirements that older files still describe as open or differently
agreed. Read the older Q references as discussion history with the following resolutions;
they do not override [the outcome](backend-results.md).

| Earlier question/proposal | Current decision | Consequence / remaining detail |
| --- | --- | --- |
| Q1, Q21, Q23a, Q25: platforms and packaging | Windows, Linux and macOS; everything runs in process and ships with the app. | No child executable, scanner/AI server, helper service or capture-driver installation. Explain and obtain approval for an unavoidable platform exception; packaging remains unqualified (BE-0007–BE-0012, BE-0097–BE-0101). |
| Q2, Q4a/Q4b, Q8, Q17: scan coverage | IPv4 local and reachable routed/VLAN targets, selected TCP lists/ranges, named UDP protocols and supported user-selected identification depth; preserve child interfaces. | No blanket UDP sweep, automatic VLAN access or invented raw-fingerprint support. Syntax, method list and workload limits remain BE-0002–BE-0003. |
| Q5, Q6a/Q6b: run lifetime | Manual starts only; pause starts no new probes; in-flight results may arrive; navigation continues; Stop/close cancels with partial results retained. | No resumed traffic on Open/restart. Finite close/cancellation limits and upstream control still need BE-0002, BE-0009 and BE-0060. |
| Q9, Q10a: manual values and conflicts | Manual values always win; show both conflicting values with source/time and explicit keep/adopt actions. | Use a persistent field comparison rather than repeated modal alerts. Acknowledging one difference does not discard contrary evidence; a changed detected value can alert again (BE-0022, BE-0077). Exact styling is implementation detail. |
| Q13a and older recovery phases | Explicit Open/Save/Save As only; **no autosave or separate crash recovery**. | Older recovery cadence/copies are superseded, including the conditional roadmap's recovery work. Failure protection and Save/Discard/Cancel remain required (BE-0041–BE-0045). |
| Q12, Q22a/Q22b: saved scope and retention | Complete versioned `.rhopper.json`, retaining history/logs without automatic deletion. | Bound working buffers, visible windows and parsers; do not prune saved history. Agree file limits and readable failure behaviour under BE-0006. No parallel SQLite inventory. |
| Q22c and older custom-asset deferral | Embed custom icons with the project. | No absolute development-path dependency; validate embedded assets and provide safe built-in fallbacks (BE-0035, BE-0039). |
| Earlier secure-store/credential representation proposal | Never persist credentials in files, logs or a separate credential store. | Obtain credentials only for a particular operation. Authenticated vendor integration is deferred; complete saving does not override this prohibition. |
| Q19: several networks and file merging | One active project may contain several named network contexts; Open replaces it. | Independent-file merging is outside scope. Durable document-local keys suffice unless a later requirement establishes a need for UUIDs. |
| Q11, Q16/Q16b/Q16c: one or two models and updates | One bundled pretrained CPU model, approximately 700 MB maximum weights; no downloads, telemetry or self-training. | One model/runtime/tokenizer/template must be qualified together. Size is not a RAM, speed or quality result; no second-model fallback is approved (BE-0005, BE-0011, BE-0085). |
| Q3, Q10b: AI automation | Review suggestions first; Fill missing inferred fields and Automatically add new devices are separate controls, both off by default. | Policy scope, reversal and measured thresholds remain BE-0005. Ambiguous identities stay in review; no automatic manual-field replacement (BE-0087–BE-0091). |
| Q14, Q26/Q26a/Q26b: map meaning | Preserve the card map and automatic placement; separate ownership, connectivity and display organisation; free moves translate the whole displayed branch. | Use simple free-position connectors, preserve established placement on Open/refresh, and rearrange only by explicit layout action. Mode switching/styling examples remain BE-0004. |
| Q15a: Forget and Reset | Neither workflow belongs in the intended product. | Old prototype controls are discrepancies to remove within BE-0096, not a requirement for deletion/suppression/recovery mechanisms. |
| Q20, Q24: implementation authority | This request expands the planning document; it does not implement the backend or amend protected rules. | BE-0001 remains open. The source-only review's earlier “no implementation tasks created” statement describes that review; this roadmap now supplies the requested tasks. |

### Scanner and platform research results

[The engine investigation](backend-scanner-engine.md) selected the **dependency approach**,
not a proven release: stock netscli-core via Cargo, no fork, copied scanner, local patch or
replacement probe engine. The app owns scope, lifetime, admission and manual authority;
upstream owns implemented scanning, retry/probe behaviour and fingerprints. Retain useful
qualified configuration beyond the first GUI's switches without speculative empty options.

| Research result | What it establishes / what it leaves open | Tasks / blockers |
| --- | --- | --- |
| Inspected netscli-core 0.3.4 is MIT, has empty defaults and optional capture/database/mDNS features; upstream CI defines all three desktop OSes. | A public embeddable library is available. CI and disabled capture are not proof that final linking, initialisation and selected calls need no external components. | BE-0007–BE-0008; BLK-002–BLK-003 |
| Its declared Rust minimum is 1.96 versus protected project Rust 1.94. `cargo info` selected 0.2.6 for the current compiler. | There is a declared toolchain mismatch. Older 0.2.6 is unqualified and also has OS-command ARP paths; neither silent downgrade nor toolchain upgrade resolves it. | BE-0007; BLK-002 |
| Inspected macOS ARP invokes `arp`; traceroute invokes system tools; Windows reverse-name fallback may invoke `ping`. | Built-in OS commands still violate the no-child-program boundary. Qualify reachable selected paths and any supported subset; do not assume runtime switches remove dependencies. | BE-0008; BLK-003 |
| Rejected nrev 0.7.0 depends on nex/nex-datalink 0.26.0 with Windows `#[link(name = "Packet")]`; its optional pcap feature controls another path. | Confirmed source-level packaging obstacle, not a reproduced linker failure. Turning off pcap or selecting TCP at runtime does not remove that dependency graph. nrev is not the fallback engine. | BE-0007–BE-0008 |
| TCP-connect and ordinary TCP/UDP service queries do not intrinsically require a capture driver; native Windows ICMP/ARP APIs exist. | Driver-free coverage is possible in principle. That does not qualify a safe cross-platform Rust wrapper or prove raw SYN/OS fingerprints available under this contract. | BE-0008; BLK-003 |
| Progress callbacks and cancellation tokens exist as library/runtime facilities. Started Tokio blocking tasks cannot be aborted by dropping their handles. | Incremental results, pause boundaries, finite Stop/close and all owned-task accounting need integration evidence. A scheduled 100% or abandoned await is not completed collection. | BE-0009, BE-0055–BE-0060 |

Other comparisons explain why engine hunting is not a substitute for qualification:
qscan was narrower, had older inspected dependencies and a GPL distribution question;
zond-engine advertised three OSes but documented libpcap/Npcap requirements and AGPL;
portmap had a non-optional pcap dependency; lsnet and Happy IP Scanner were applications
without a normal public library entry point found. Nmap executable integration is excluded.
The inspected Fing mobile SDK material did not establish an offline desktop SDK. These are
research-snapshot conclusions, not fresh claims about those projects' current releases.
No alternative is selected by listing it here.

[Discovery research](backend-discovery.md#combine-sources-without-overstating-them) also
defines how to interpret results; available evidence must not become invented topology:

| Method / platform finding | Interpretation and acceptance consequence |
| --- | --- |
| Interfaces/routes and neighbour caches | Bootstrap real targets and show source/interface/VPN context. Cache entries can be stale; ARP cannot reveal a routed host's MAC. A mapped prefix or route is not scan permission. |
| ICMP and TCP | Positive responses are evidence; no ping reply must not prevent TCP/service checks. Refused, timed out, denied and not tested are distinct; silence alone does not prove Offline. |
| DNS, mDNS/DNS-SD, SSDP and OUI | Names/advertisements/vendor assignments are hints with source/time, not authenticated identity or exhaustive inventory. NIC vendor does not establish device type/manufacturer. |
| mdns-sd 0.21.6 | Construction is not multicast readiness: consume monitor events. A full browse channel can terminate the subscription; unexpected disconnect means lost/partial coverage. Stop subscriptions at run end, with bounded retry/wait behaviour. |
| Safe neighbour/ICMP access | Native bindings alone do not satisfy the app's no-unsafe rule. No maintained safe wrapper was qualified; bespoke FFI hidden in a helper crate is not an approved workaround. |
| macOS normal app launch | Local-network privacy can affect ordinary sockets; developer Terminal/root runs do not prove packaged-app permission handling. Verify normal launch with denied and granted access. |
| SNMP/LLDP/controller/ONVIF evidence | Optional later named integrations, not mandatory first-backend collectors. FDB reachability is not a direct cable; VLAN/FDB/port mapping and guest ownership cannot be guessed. No route/VLAN/equipment configuration is implied. |
| Untrusted replies and description URLs | Bound text/body size, parsing depth, time and redirects; validate resolved destinations at every redirect. Replies and AI cannot expand scope, fetch arbitrary resources or execute commands. |

These method semantics feed BE-0050–BE-0065 and final BE-0102–BE-0103. The exact named
UDP set, privilege policy and capability matrix are still BE-0003/BE-0008 decisions.

### Architecture, map and file-lifecycle details

The [architecture recommendation](backend-architecture.md#retain-the-current-application-boundary)
keeps one authoritative document in today's UI-owned Rust state. Slint projects it; bounded
workers carry owned messages and immutable save/evidence snapshots. Keep parsing, full import
validation, serialisation and inference off the event loop. Slint's `spawn_local` is not a
Tokio runtime, and a cross-thread `Send` closure cannot capture `Rc<RefCell<AppState>>`.
A bounded UI-side receiver drain can extend the current timer; preserve `view::sync` rather
than posting one closure per reply. Measure reconciliation/snapshot/projection cost before
moving domain ownership or adding an actor/store framework (BE-0055–BE-0056, BE-0094).

Identity reconciliation first uses explicit confirmed associations, then compatible stable
identifiers and scoped MAC evidence checked against simultaneous conflicts/history. Names,
OUI and AI are supporting hints. DHCP changes may continue a card's history; IP reuse, MAC
randomisation and overlapping VPN prefixes require review rather than transferring notes.
Keep last success separate from latest failure and recheck current revisions at admission
(BE-0019–BE-0023, BE-0066–BE-0071).

[Map inspection](backend-map-persistence.md#what-the-code-already-does) found that a drop
currently calls `Network::connect`, changes Arrangement and relays out the map. `lay_out`
replaces positions; initialisation starts with empty arrangement. Serialising `positions`
alone would therefore neither separate display/network meaning nor preserve restored layout.
During drag, even those positions include preview movement; spring velocities and generated
SVG paths are also transient. BE-0024–BE-0033 must establish the following shared contract:

- Save display parentage, explicit root order, ordered tiers/left-right siblings and committed
  icon-centre coordinates. Use absolute map units initially; coordinates restore the picture,
  and tree/order intent drives a later explicit layout action.
- Traverse the display forest for snapping, reparenting and whole-branch translation. An
  access point can stay network-connected to a switch while displayed under another icon.
  Retain network evidence even when an edge is not representable by the displayed tree.
- Commit parentage/order/coordinates together on drop; Cancel restores the previous revision.
  Save during drag or settling animation captures committed state without waiting for springs.
- Restore motion at rest and the map-space view centre/zoom after viewport creation. Initial
  fit cannot override a saved manual view. Do not save renderer paths, hover or pointer capture.
- Validate finite bounded coordinates, unique placements, ownership and display cycles,
  matching roots/tiers and existing references. Network evidence can legitimately have loops;
  the display forest cannot. Preserve placement through identity correction explicitly.

Keep Slint and the existing layout/snap/motion implementation. The reviewed project enables
Skia; Slint's generic software renderer has scaling limitations and is distinct from Skia
software rendering. Neither a universal software fallback nor free-position obstacle routing
was demonstrated. petgraph, rstar, ELK, egui/snarl and Iced were comparisons, not selected
dependencies or reasons for a UI rewrite. Simple free connectors and the outcome's clear-area
new-card placement remain the contract; mode-switch/styling examples remain BE-0004.

For [complete saves](backend-results.md#saving-and-opening-projects), the schema includes
inventory/ownership, contexts/addresses/services, observations/manual values, roles/groups,
embedded icons, network/display relationships, layout/view/panels/selection/filters, scan
settings/results/history/logs and AI settings/suggestions/accepted provenance/model identity.
Runtime handles and credentials are excluded by their explicit contracts. AI weights ship
with the app; save the model identity rather than making projects depend on a model download.

Open parses and validates a separate candidate, checks supported schema/migrations and only
then replaces the current project after dirty handling. Saved runs reopen as historical.
Save writes a complete sibling temporary file and acknowledges after qualified replacement;
Save As changes the path only on success. Serialise writes per destination and identify every
receipt by document generation, revision and path: an old save cannot clear a newer edit or
change a newly opened project's filename. Cancellation does not undo replacement already
committed. External changes and competing writers must be reported before replacement.
These are BE-0034–BE-0045 obligations, without the older recovery-store proposal.

### Supporting-library and AI qualification results

The [crate research](backend-crates.md) is a shortlist, not an approved dependency set.
Reuse std, pinned Slint and existing ipnet first; avoid direct dependencies where the qualified
engine already exposes the needed functionality. Versions below identify the inspected
snapshot and must be rechecked before selection.

| Candidate / route | Finding and required decision | Tasks |
| --- | --- | --- |
| Tokio 1.54.0 / tokio-util 0.7.20 | Candidate runtime, bounded channels, supervision and cooperative cancellation; select minimal compatible features. Concurrency and starts/second need distinct limits, not a task for every host/port. | BE-0007, BE-0009, BE-0055–BE-0060 |
| netdev 0.46.3, mdns-sd 0.21.6, hickory-resolver 0.26.3 | Candidate interface/multicast/DNS support only if upstream needs supplementation. Per-destination routing, split DNS, readiness, platform permissions and shutdown remain unqualified. No default public DNS/DoH dependency. | BE-0008, BE-0050, BE-0063 |
| Serde/serde_json | Recommended typed JSON route; serialisation does not provide domain validation, safe replacement or access policy. | BE-0010, BE-0034–BE-0039 |
| atomic-write-file 0.3.1 or tempfile 3.27.0 | Choose one vetted save route. Research records ACL/ownership preservation limits and symlink replacement semantics; tempfile persist does not itself sync file/directory. Do not promise universal power-loss safety. | BE-0006, BE-0010, BE-0041–BE-0043 |
| rfd 0.17.2 | Windows/macOS native picker candidate. Default Linux portal/Zenity route conflicts with the boundary; investigate in-process GTK3 with defaults disabled and qualify native library/Slint packaging. | BE-0010; BLK-004 |
| directories 6.0.0 | Per-user preferences-path candidate, not project storage or recovery. Maintenance after repository migration and exact compatibility were not verified. | BE-0010, BE-0012 |
| Safe neighbour/ICMP and protocol/OUI libraries | No safe neighbour wrapper qualified. surge-ping, SSDP, SNMP, ONVIF and OUI candidates have individual permission/interoperability/data-licence questions; do not add the whole list. Authenticated managed integrations stay deferred. | BE-0008, BE-0098 |
| SQLite, UUIDs and graph/spatial indexes | Conditional alternatives only. One JSON document needs neither a second database nor independent-file merge keys; measured bottlenecks or separately approved scope must justify additions. | BE-0034, BE-0094 |

[AI research](backend-opportunities.md#requested-existing-offline-cpu-models) proposes
**Qwen3-0.6B Q8_0 GGUF, approximately 639 MB, with Candle 0.11.0's quantized Qwen3 adapter**
as the first feasibility candidate. The publisher artifact and released adapter were found;
their exact artifact/tokenizer/chat-template/runtime compatibility, Rust 1.94 fit, CPU
latency/RAM and network-identification quality were **not demonstrated**. Apache-2.0 model
licensing and a weight size within the allowance do not establish those results.

SmolLM2-360M-Instruct Q8_0 (about 386 MB) and a DeBERTa xsmall quantized ONNX model (about
87.2 MB) were smaller alternatives, with usefulness unverified. MiniLM embeddings support
similarity/search, not a probability that a device runs Windows. They are alternatives or
deferred ideas, never extra bundled models under this outcome. Candle is the first runtime
candidate; embedded llama.cpp bindings had a material safety disclaimer, and ONNX routes
require model/operator/tokenizer qualification plus deliberate native packaging and telemetry
controls. An external runner is excluded under every route (BE-0011; BLK-005).

For BE-0082–BE-0091, pin model/runtime/tokenizer/template identities, hashes and notices;
bound load admission, CPU threads, context, output and shutdown. Supply evidence IDs, sources
and age, allow only named fields/categories plus Unknown, and reject invented evidence,
malformed/oversized output and instructions embedded in banners. Check document generation,
evidence revision, model and policy again before applying a suggestion. A pre-Add typed value
is already manual; an imported policy is not fresh consent.

The user's example is `DESKTOP-13123` plus TCP 3389/RDP evidence → **likely Windows PC**.
A confirmed RDP response is stronger than a port number, but Linux xrdp also supplies RDP:
neither clue verifies OS version, hardware model or manufacturer. Keep identity confidence,
suggestion score and relationship certainty distinct; any unevaluated model number must be
labelled uncalibrated. Compare held-out cases with an explainable evidence baseline and
report per-field false positives and abstention, including Linux RDP, renamed/generic hosts,
no-MAC routed hosts, spoofed banners and conflicting evidence. No accuracy, peak RAM, cold
load or latency results were produced by the review. BE-0005 defines the bar; BE-0085 measures
it. Automation remains blocked if stricter thresholds fail, without disabling manual review.

## IDs, status and maintenance

| Item | Permanent format | Current allocation | Next unused ID |
| --- | --- | --- | --- |
| Milestone | `M-001` | `M-001`–`M-020` | `M-021` |
| Task | `BE-0001` | `BE-0001`–`BE-0106` | `BE-0107` |
| Blocker | `BLK-001` | `BLK-001`–`BLK-009` | `BLK-010` |

IDs identify work, not its execution position. Allocate the next unused ID in this roadmap
before adding an item in either file; then advance this table. Never renumber or reuse an ID.
Insert a later-numbered milestone wherever its dependencies require. A task may move between
milestones while retaining its ID. Split work by retaining the original ID for its remaining
scope and allocating new IDs for the other pieces, with explicit links.

All task rows initially have status **Planned**. Use `Planned`, `Ready`, `Active`, `Blocked`,
`Done` or `Cancelled`; add a Status column when execution starts. A task becomes Ready only
when all listed dependencies are Done, applicable blockers are closed and its scope is
authorised. Record blocking IDs/reasons rather than marking a dependency complete to proceed.
Cancelled work does not satisfy dependencies; explicitly revise dependent tasks. A milestone
is Done only when every required task and its exit check pass.

`Depends on` lists task prerequisites; all are required. `—` means no task prerequisite, not
permission to bypass project rules. Milestone entry dependencies are additional prerequisites
for every task in that milestone. Rows are in suggested execution order; work may proceed
independently only when those dependencies allow it.

The roadmap owns milestone scope, task IDs, dependencies and completion checks. The backlog
holds optional detailed work cards, newly captured work and execution evidence using the same
IDs. Do not copy the entire roadmap into it. A backlog card for an existing task keeps its ID;
new scope receives a new ID and roadmap placement before it becomes Ready.

No dates, effort estimates or percentage-complete claims are invented. Add estimates after
qualification. Reorder affected work when a blocker changes, and retain cancelled IDs with
their reason so references remain meaningful.

## Blockers and decisions

All blockers are **open** at this baseline. Closing one requires recorded evidence or the
specified user decision. An upstream limitation is not permission to weaken a requirement.

| ID | Blocker / unresolved choice | Resolution tasks | Work held until resolved |
| --- | --- | --- | --- |
| BLK-001 | Protected product rules still prohibit real networking/persistence/AI. | BE-0001: explicit approval of the concrete scope amendment, then approved rule update. | All backend implementation, including feasibility code; this planning work may proceed. |
| BLK-002 | Scanner release/toolchain fit is unproven. Research records netscli-core 0.3.4 requiring Rust 1.96 against protected 1.94; older 0.2.6 is not a qualified fallback. | BE-0007: qualify a compliant stock release or obtain a concrete approved toolchain decision. | Adding/integrating the engine. No silent upgrade, fork or old-version pin. |
| BLK-003 | Cross-platform scanner call paths, safe neighbour/ICMP access and pause/cancellation are unqualified. Research found OS-command paths. | BE-0008, BE-0009: qualify supported paths and disclose limitations; seek explicit approval for an unavoidable platform exception. | Promising coverage, production collector integration and acceptance. No shell-command or capture-driver workaround. |
| BLK-004 | Native dialogs and safe replacement must satisfy in-process packaging on all OSes, including Linux; metadata/ACL/link behaviour is unresolved. | BE-0006, BE-0010: choose and qualify one file-dialog/save path. | M-008 native file integration and portable Open/Save claims. |
| BLK-005 | Model, tokenizer, template and safe CPU runtime are not qualified together. Model size alone proves little. | BE-0005, BE-0011: one compliant, useful model/runtime with recorded licences and limits. | M-016 AI integration. No second-model, download or external-runner fallback. |
| BLK-006 | Workload, target syntax/exclusions, method list, resource limits and finite shutdown expectations need concrete acceptance values. | BE-0002, BE-0003: agreed limits and test scenarios, including services/card counts. | Scan configuration/runtime acceptance and performance claims. |
| BLK-007 | Remaining map presentation rules need examples: mode switching and organisational/network link styling. | BE-0004: agree examples while retaining simple free connectors and non-disruptive new-node placement. | Final map interaction acceptance; settled relationship semantics stay fixed. |
| BLK-008 | AI field/category quality targets, baseline hardware, resource/latency budgets and automation thresholds/policy scope are unspecified. | BE-0005: agreed measurable criteria; BE-0085 and BE-0091 supply evidence. | Calling suggestions useful or enabling automatic policies. Default remains off. |
| BLK-009 | Windows/Linux/macOS machines, permitted test networks/targets and execution authorisation are not provided by these documents. | BE-0002 identifies environments. Obtain and record access/approval before each live or GUI session, including qualification; BE-0102 confirms the final acceptance session. | Live network, packaged GUI, credential and cross-platform acceptance claims. Deterministic source checks may proceed within approved scope. |

If a required capability cannot meet the contract, record the failed check and alternatives,
keep the affected milestone Blocked and request the specific decision. Do not silently reduce
the first working backend to one OS, omit AI, build a custom scanner or add recovery storage.

## Delivery sequence

The outcome's order is preserved: complete project foundation before real scanning, reliable
updates before AI integration, then integrated acceptance. Early feasibility checks expose
scanner/file/model blockers before substantial implementation depends on them.

| Order / milestone | Outcome | Entry dependencies |
| --- | --- | --- |
| [M-001](#m-001-contract-and-acceptance-decisions) | Authorised contract and measurable acceptance decisions | None; decision work first |
| [M-002](#m-002-qualify-critical-dependencies) | Qualified scanner, file lifecycle and model/runtime choices | M-001 |
| [M-003](#m-003-durable-inventory-and-network-contexts) | Owned metadata, durable identity and scoped endpoints | M-001 |
| [M-004](#m-004-observations-and-manual-authority) | Provenance, manual precedence and truthful status | M-003 |
| [M-005](#m-005-separate-map-relationships) | Ownership, connectivity and display forest separated | M-003, BE-0004 |
| [M-006](#m-006-committed-placement-and-view-state) | Automatic/free placement and restorable view state | M-005 |
| [M-007](#m-007-complete-versioned-project-format) | Complete validated portable JSON document | M-004, M-006 |
| [M-008](#m-008-native-open-save-and-save-as) | Reliable native file lifecycle and dirty handling | M-007, BE-0010 |
| [M-009](#m-009-project-foundation-checkpoint) | Useful manual project workflow before live scanning | M-008 |
| [M-010](#m-010-explicit-scan-scope-and-capabilities) | Validated scope/settings and honest capability preview | M-009, BE-0007–BE-0009 |
| [M-011](#m-011-background-run-lifecycle) | Responsive bounded run control | M-010 |
| [M-012](#m-012-connect-real-scanner-observations) | Real engine results replace simulation | M-011 |
| [M-013](#m-013-identity-continuity-and-admission) | Repeat runs reconcile safely with inventory | M-012 |
| [M-014](#m-014-real-discovery-review-workflow) | Existing review/edit/Add flow works with real data | M-013 |
| [M-015](#m-015-reliable-updates-history-and-conflicts) | Saved, traceable updates preserve user work | M-014 |
| [M-016](#m-016-offline-ai-suggestions) | One model provides bounded reviewable suggestions | M-015, BE-0011 |
| [M-017](#m-017-explicit-ai-automation-controls) | Independent opt-in field filling and auto-add | M-016 |
| [M-018](#m-018-integrated-correctness-and-responsiveness) | Combined workflows withstand races and load | M-017 |
| [M-019](#m-019-self-contained-builds-on-three-platforms) | Runnable app bundles include every required asset | M-018, M-002; session access/approval before live checks |
| [M-020](#m-020-first-working-backend-acceptance) | Evidence-backed first working backend handover | M-019; BE-0102 confirms final live-session access/approval |

M-002, M-003 and independent model/map work can be investigated separately after M-001.
Each qualification task records its own approved selection and closes its own blockers.
BE-0012 consolidates those results; it does not force file or scanner work to wait for AI
qualification. Dependency independence does not permit multiple agents to edit the same working tree; follow
[collaboration rules](agents/collaboration.md). A failed model feasibility check need not
stop authorised inventory work, but it still blocks the AI milestone and overall completion.

## M-001: Contract and acceptance decisions

Goal: resolve implementation authority and the remaining choices without reopening settled scope.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0001 | Present the exact backend/product-guide amendment for approval, obtain explicit authorisation and apply only the approved rule changes. Preserve Rust/Slint, pure model and protected safety constraints. | — |
| BE-0002 | Agree acceptance environments and workload: roughly 250 devices, interfaces per device, guests, services, accumulated history, viewport sizes, CPU/RAM/UI latency and cancellation/close/save time limits. Identify all three OS test environments. | — |
| BE-0003 | Specify address/CIDR/TCP-range/exclusion syntax, source-interface selection, named UDP methods, identification depths, pacing/timeouts/retries and optional permission degradation. Record supported versus requested capabilities. | BE-0002 |
| BE-0004 | Agree automatic/free mode switching and relationship styling examples. Preserve whole-branch movement, simple free connectors, explicit-only rearrangement and clear-area insertion of new cards. | — |
| BE-0005 | Agree baseline CPU hardware, model fields/categories, resource/latency limits, Unknown handling, per-field quality targets and stricter automation thresholds. Decide policy scope and reversibility; manual replacement stays prohibited. | BE-0002 |
| BE-0006 | Specify file/input safety limits, supported-schema policy, writer-conflict behaviour, external-change checks and access/ACL/link expectations. Explain large-history handling without automatic deletion and document the absence of crash recovery. | BE-0002 |

Exit: each decision has an answer and reviewable acceptance examples; BLK-001 is closed before
any implementation starts. Pending answers keep their dependent work blocked.

## M-002: Qualify critical dependencies

Goal: find hard integration problems before committing to a scanner, save path or model runtime.
Use maintained tools and minimal real integration checks, not disposable scripts or speculative
frameworks. Feasibility work must be authorised; live checks need selected approved targets.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0007 | Qualify a stock netscli-core release, features, full dependency graph, licences/data licences and protected toolchain fit on Windows/Linux/macOS. Record the selected version/features and approved MSRV decision before any protected change. | BE-0001, BE-0003 |
| BE-0008 | Trace selected scanner, interface, DNS, neighbour and ICMP paths for child processes, capture/native libraries, safe API access and privileges. Produce a three-platform method matrix; qualify only necessary supplementary libraries and obtain approval of any unavoidable exception before integration. | BE-0007 |
| BE-0009 | Exercise incremental results, pause/no-new-probe behaviour, stop/close task accounting, timeouts and queue pressure against the selected upstream API. Do not substitute a custom scanner/retry engine for missing upstream control. | BE-0008 |
| BE-0010 | Qualify native dialogs and one safe save/replacement approach on all three OSes, including Linux integration, ACL/link/metadata behaviour and failed replacement. Record the approved selection. No helper executable or separately installed runtime. | BE-0001, BE-0006 |
| BE-0011 | Qualify one pretrained artifact with its tokenizer/template and CPU runtime: safety/toolchain, offline loading, licences, weight allowance and preliminary usefulness/resource measurements. Record the approved selection; Qwen/Candle is a research candidate, not a predetermined winner. | BE-0001, BE-0005 |
| BE-0012 | Consolidate qualified versions/features/assets, approvals and evidence from each branch. Audit that every feasibility blocker is closed before the integrated app-bundle checkpoint; record any still-blocked requirement honestly. | BE-0007, BE-0008, BE-0009, BE-0010, BE-0011 |

Exit: every selected dependency has a compliant integration route. A successful Cargo build
alone does not qualify OS permissions, file integrity, scanner cancellation or AI usefulness.

## M-003: Durable inventory and network contexts

Goal: extend the current pure Rust model rather than create a second inventory.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0013 | Replace static-only metadata with owned runtime values while preserving existing device/card/edit behaviour. | BE-0001 |
| BE-0014 | Establish non-reused device/card/run/result keys and persisted allocation state; keep bounded Slint handles separate from durable IDs. Check exhaustion and save/reopen continuity. | BE-0013 |
| BE-0015 | Add named network contexts and scoped address associations. Equal private IPs in different contexts remain distinct; missing remote MAC/prefix is valid. | BE-0014 |
| BE-0016 | Preserve device → child-interface and VM/container ownership with validated references and cycles rejected. Do not infer guest ownership from vendor/IP hints. | BE-0015 |
| BE-0017 | Represent service endpoints by context/address/transport/port with explicit collection outcomes, including not tested, denied and no response. TCP/UDP sharing a port remain distinct. | BE-0015 |
| BE-0018 | Route manual mutations through existing model validation; cover duplicate keys, invalid references, address edits and ownership changes with meaningful deterministic cases. | BE-0016, BE-0017 |

Exit: runtime data fits the existing model, remains free of Slint, and cannot bypass invariants
through an unchecked editor or imported record.

## M-004: Observations and manual authority

Goal: make evidence and user intent distinguishable before receiving network results.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0019 | Store field/source/context/time evidence separately from manual values and inferred suggestions; preserve meaningful unavailable/unknown states. | BE-0018 |
| BE-0020 | Apply manual → supported observation → labelled inference → unknown precedence through shared edit/reconciliation logic. Clearing an override has explicit fallback semantics. | BE-0019 |
| BE-0021 | Record last success separately from latest method outcome; display last-observed time without equating a timeout or old evidence to Offline. | BE-0019 |
| BE-0022 | Represent conflicting manual/detected fields with both values, source/time and acknowledgement keyed to the actual differing value. | BE-0020 |
| BE-0023 | Exercise manual edits during delayed observations, conflict acknowledgement/reappearance, missing data and transport-specific outcomes without any live traffic. | BE-0021, BE-0022 |

Exit: neither collection nor AI can silently overwrite manual fields; failed methods cannot
erase supported facts or manufacture fresh reachability.

## M-005: Separate map relationships

Goal: reuse tree/arrangement behaviour while removing its coupling to network claims.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0024 | Separate evidence-backed network relationships from ownership and a saved display forest referencing existing card IDs. Retain unrepresented network edges. | BE-0018, BE-0004 |
| BE-0025 | Extend current traversal/Arrangement/snapping to use display parentage, explicit root order and ordered tiers/siblings. | BE-0024 |
| BE-0026 | Make display reparenting one validated presentation edit; reject display cycles/multiple parents without claiming the network has no loops. | BE-0025 |
| BE-0027 | Distinguish observed, inferred, manually recorded and organisational links in existing canvas presentation and its key. | BE-0026 |
| BE-0028 | Check a display branch whose parent differs from its network uplink, multi-card devices and guest ownership; changes to one relationship leave the others intact. | BE-0027 |

Exit: dragging an icon under another icon cannot create a detected connection or change
device/interface ownership.

## M-006: Committed placement and view state

Goal: preserve the user's map across edits, scans and future file operations.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0029 | Store committed map-space icon centres, arrangement, placement mode and layout parameters in Rust; keep pointer previews and spring state transient. | BE-0028 |
| BE-0030 | Implement free positioning with simple derived connectors and one committed translation for the displayed parent plus descendants; cancel restores the prior committed branch. | BE-0029 |
| BE-0031 | Preserve existing automatic tree/snap/order/animation where applicable; implement the agreed mode switch and explicit layout action without introducing Reset. | BE-0030, BE-0004 |
| BE-0032 | Admit new cards into a clear area without moving established manual positions. Refresh and ordinary edits preserve placement and display parentage. | BE-0031 |
| BE-0033 | Capture restorable viewport, zoom, panels, filters, selection and display options into the document. Restore after viewport creation; initial fit cannot override a saved manual view. | BE-0032 |

Exit: whole-branch offsets survive movement and a snapshot during drag/animation contains the
last committed placement. Position restoration has defined behaviour across viewport sizes.

## M-007: Complete versioned project format

Goal: establish the complete save contract before wiring native file controls.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0034 | Define the `.rhopper.json` schema/version and complete-state checklist, including IDs, contexts, inventory, relationships, layout/view, manual data, scan settings/results/history/logs and AI state/model identity. No competing database authority. | BE-0023, BE-0033, BE-0006 |
| BE-0035 | Embed custom SVG/icon assets with stable identifiers, validated size/content and built-in fallbacks; remove dependence on absolute development-machine paths. | BE-0034 |
| BE-0036 | Serialise immutable revisioned snapshots off the UI thread; preserve every retained history entry while bounding active buffer/parser work. Define a clear failure if an approved file limit is exceeded. | BE-0035 |
| BE-0037 | Parse Open into a separate candidate and validate schema, counts/strings, references, ownership/display cycles, ordered placement and finite coordinates/zoom before installation. | BE-0036 |
| BE-0038 | Define supported migrations and reject unknown newer versions or failed migrations without touching the current project. Do not invent a migration for an unreleased historical format. | BE-0037 |
| BE-0039 | Round-trip complete state and reject corrupt/hostile files, embedded remote references and secret-bearing fields. Never persist credentials or executable/runtime objects. | BE-0038 |

Exit: all complete-save categories have explicit schema coverage and round-trip evidence;
invalid input is handled before the active document can be replaced.

## M-008: Native Open, Save and Save As

Goal: reliable explicit saves with a previous file protected against failure.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0040 | Connect qualified native Open/Save/Save As dialogs to the existing application. Picker cancellation changes neither document nor active path. | BE-0039, BE-0010 |
| BE-0041 | Write a complete sibling temporary snapshot, flush/commit through the qualified replacement path and preserve the old destination on failure; report errors visibly. | BE-0040 |
| BE-0042 | Enforce one writer per file and detect conflicting external changes before replacement, using BE-0006's policy. Check races and access/metadata failures. | BE-0041 |
| BE-0043 | Serialise writes per destination; receipts include document generation/revision/path. Older saves cannot overwrite newer snapshots or clear newer edits; Save As changes the active path only after success. | BE-0042 |
| BE-0044 | Implement dirty indication and Save/Discard/Cancel before close/project replacement. Invalid Open leaves the active project intact; a document switch invalidates old callbacks. No autosave/recovery files. | BE-0043 |
| BE-0045 | Restore full project/view state with motion at rest and old observations labelled last-known. A reopened active-run record is historical; no probes or AI auto-add start because a file requests them. | BE-0044 |

Exit: save failure, picker cancellation, external edit and stale-save receipt cases preserve
the correct file and working project. A full round trip restores committed placement.

## M-009: Project foundation checkpoint

Goal: a useful persistent manual project exists before real scanning is connected.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0046 | Establish an empty real project that supports manual devices, child cards, contexts, roles/groups and icons without requiring sample-network data. | BE-0045 |
| BE-0047 | Exercise existing list/map selection, search, filters, inline/pre-admission editors, guest/card edits and custom roles against the one document; preserve drafts and selection. | BE-0046 |
| BE-0048 | Demonstrate full-state save/reopen, free-branch movement and automatic arrangement with multiple interfaces/contexts. Record source checks separately from authorised visual evidence. | BE-0047 |
| BE-0049 | Review the foundation against the outcome and remove superseded simulation-only persistence assumptions. Advance to scanning only after the foundation exit check passes. | BE-0048 |

Exit: the project foundation is usable and complete. No real scanner has been implied by
successful persistence or mock interaction checks.

## M-010: Explicit scan scope and capabilities

Goal: build truthful run settings before issuing network work.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0050 | Enumerate interfaces/addresses and relevant routes through qualified APIs, enabling empty-inventory bootstrap. Distinguish VPN/virtual adapters and user network contexts. | BE-0049, BE-0008 |
| BE-0051 | Validate explicit IPv4 addresses/CIDRs/exclusions lazily, including /31 and /32 behaviour. Show the selected source/interface; mapped prefixes never grant scan permission. | BE-0050, BE-0003 |
| BE-0052 | Expose selected TCP lists/ranges, named UDP protocols and supported identification depth. Permit TCP/service checks without successful ping; no blanket UDP sweep. | BE-0051 |
| BE-0053 | Show available/unavailable methods and reasons, estimated work and approved limits before Start. Reject invalid/oversized work without sending probes. | BE-0052, BE-0009 |
| BE-0054 | Freeze context, targets, exclusions, ports, methods and limits into an immutable run configuration; retain supported backend options without empty speculative settings. | BE-0053 |

Exit: every probe belongs to an explicit approved user-started scope; routed/VLAN scanning
uses existing reachability and does not configure routes or devices.

## M-011: Background run lifecycle

Goal: real lifecycle semantics without blocking the event loop.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0055 | Own one active scan and its runtime/tasks off the UI thread; workers emit owned data into the existing Rust document boundary, never a second mutable inventory. | BE-0054 |
| BE-0056 | Add bounded nonblocking command/result flow, coalesced progress and UI drain budgets. Keep Stop control independent of a full result queue; report coverage loss instead of silently dropping findings. | BE-0055 |
| BE-0057 | Implement preparing/running/pausing/paused/cancelling and terminal outcomes; use upstream control for pause/resume and finite in-flight work, not a new probing scheduler. | BE-0056, BE-0009 |
| BE-0058 | Reject stale run/document-generation events; apply retained observations before the terminal acknowledgement. Cancellation retains valid partial results. | BE-0057 |
| BE-0059 | Keep runs alive across navigation and cancel/account for owned work on close or confirmed project replacement. Cancelled replacement leaves the current project; close Cancel does not silently restart a stopped scan. | BE-0058, BE-0044 |
| BE-0060 | Verify monotonic elapsed time, stage work counters, indeterminate work, partial method failures, full queues and bounded shutdown. Scheduling 100% is not completed collection. | BE-0059 |

Exit: Start/Pause/Resume/Stop have observable, truthful effects; no collection survives the
run or app close, and no new traffic starts on navigation or Open.

## M-012: Connect real scanner observations

Goal: replace fabricated answer production using the qualified library's existing probes.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0061 | Integrate the qualified upstream scanner on the owned runtime with capture disabled and only approved features. Remove fake status refresh/timer answer generation from the real Start path. | BE-0060, BE-0007 |
| BE-0062 | Convert incremental TCP and identification results into scoped endpoint observations with typed outcomes, provenance and timestamps. Do not copy whole fixture devices into production results. | BE-0061 |
| BE-0063 | Integrate qualified named UDP/discovery methods and necessary supplementary libraries only where the engine lacks required coverage. Stop subscriptions at run end and record overload/disconnect/permission failures. | BE-0062, BE-0008 |
| BE-0064 | Bound untrusted response sizes, parser depth, protocol timeouts and any description URL/redirect scope. Redact operation credentials; returned data cannot execute commands or expand targets. | BE-0063 |
| BE-0065 | Exercise local and routed/no-MAC observation cases, optional-method denial and partial failure with deterministic fixtures; perform a live smoke run only with explicit targets/authorisation. | BE-0064 |

Exit: findings originate from the real selected collector, with no fabricated fallback.
Unsupported methods remain explicit, and a failed method never erases prior inventory.

## M-013: Identity continuity and admission

Goal: repeated scans update the right interface/device without sacrificing manual control.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0066 | Reconcile scoped evidence using confirmed associations and compatible identifiers/MACs. IP/name/OUI/AI alone cannot establish identity; ambiguous candidates stay in review. | BE-0065, BE-0023 |
| BE-0067 | Preserve identity through supported DHCP/address changes; isolate IP reuse, MAC randomisation, overlapping contexts and conflicting simultaneous evidence. | BE-0066 |
| BE-0068 | Upsert repeated findings/announcements and services under stable keys, including multiple interfaces per parent. Missing MAC/prefix stays unknown instead of generating invented facts. | BE-0067 |
| BE-0069 | Revalidate review/Add against the latest document revision and selected endpoint/context. Admit only supported cards and reject stale/conflicting candidates after intervening edits. | BE-0068 |
| BE-0070 | Update existing mapped devices through shared reconciliation without overwriting manual fields, guest ownership or established display placement; keep identity corrections explicit. | BE-0069 |
| BE-0071 | Verify repeat-run deduplication, changed/reused IPs, routed hosts, multi-card ownership and delayed events with deterministic cases, including the fixture-copy and newcomer-duplication risks from the review. | BE-0070 |

Exit: neither repeat results nor one changed address creates duplicate inventory or transfers
another device's notes, identity or saved map position.

## M-014: Real discovery review workflow

Goal: reuse existing pages and forms for real results rather than add a parallel editor.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0072 | Drive results, counts/filters, stable result selection and details from real evidence. Label method, age, uncertainty and possible duplicates honestly. | BE-0071 |
| BE-0073 | Preserve pre-admission name/role/icon/notes editing and Add/Ignore/leave-unadded actions; typed pre-admission edits become protected manual values. | BE-0072 |
| BE-0074 | Reuse mapped-device editing and Show on map; discovery and inventory reference the same underlying device, preserving drafts during result batches. | BE-0073 |
| BE-0075 | Present real progress, pause/in-flight behaviour, terminal/partial states and per-method issues using current discovery controls/logs. Remove misleading simulated-scan wording from real mode. | BE-0074 |
| BE-0076 | Confirm start/stop/navigation and review/edit/Add workflows, keyboard focus and translated text without a new dashboard. Record any authorised GUI evidence separately from source checks. | BE-0075 |

Exit: users can identify, review, edit and admit real findings through the existing workflow,
including when AI is unavailable or disabled.

## M-015: Reliable updates, history and conflicts

Goal: make repeated scan/update/save cycles trustworthy before AI writes suggestions.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0077 | Wire persistent field comparisons into existing device/card details with Keep my value / Use detected value. Deduplicate repeat differences and alert again for a changed detected value. | BE-0076, BE-0022 |
| BE-0078 | Retain run history, address/evidence changes, issues and logs without automatic deletion. Use bounded visible windows/working buffers without pruning the saved project. | BE-0077 |
| BE-0079 | Save all real run settings/results/history and compare observations using absolute times. Reopen completed/cancelled/failed/partial runs as historical, never resumed work. | BE-0078, BE-0045 |
| BE-0080 | Check editing/moving cards during scans, save while results arrive, Save As/Open with pending work and repeated manual refresh. Preserve newer revisions and committed placement. | BE-0079 |
| BE-0081 | Demonstrate a reliable manual scan → review → Add/update → conflict handling → save/reopen loop with no AI dependency or automatic background probes. | BE-0080 |

Exit: detected conflicts remain reviewable, history survives a round trip, and manual fields
and map placement survive all supported scan/save lifecycle transitions.

## M-016: Offline AI suggestions

Goal: make one qualified pretrained model useful inside current discovery/details workflows.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0082 | Bundle/load the chosen model, tokenizer/template and runtime assets locally with pinned identity, hashes and notices; bound threads, admission, context/output and shutdown. | BE-0081, BE-0011 |
| BE-0083 | Build a bounded structured evidence snapshot with IDs/source/age; validate allowed output fields/categories, evidence references, reasons and Unknown. Treat banners and model output as untrusted data. | BE-0082 |
| BE-0084 | Present first-time type/OS-family/descriptive suggestions with supporting observations, reasons and clearly labelled confidence. Allow explicit accept/edit/reject; suggestions never become verified observations. | BE-0083 |
| BE-0085 | Evaluate held-out representative cases against an explainable evidence baseline. Measure per-field false positives/abstention, peak RAM, CPU, cold load and latency on agreed machines; label uncalibrated scores honestly. | BE-0084, BE-0005 |
| BE-0086 | Reject stale generation/evidence/model/policy results and protect edits made during inference. Missing/corrupt/incompatible assets leave manual workflows usable; no downloads, telemetry or silent model substitution. | BE-0085 |

Exit: one model runs fully offline within approved budgets and adds demonstrable review value.
RDP/hostname hints may suggest a Windows PC but cannot verify an OS version or manufacturer.

## M-017: Explicit AI automation controls

Goal: offer the agreed opt-ins without increasing AI's authority over user data.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0087 | Add independent Fill missing inferred fields and Automatically add new devices controls, both off by default, with agreed policy scope and clear explanations. No automatic manual-field replacement control. | BE-0086, BE-0005 |
| BE-0088 | Apply approved missing-field suggestions only after current revision/policy/manual-value checks, preserving source/model identity and an explicit reversal path. | BE-0087 |
| BE-0089 | Gate auto-add on valid scoped evidence, resolved identity and agreed quality thresholds. Ambiguous/possible-duplicate findings remain for manual review regardless of score. | BE-0088 |
| BE-0090 | Persist policy/settings/suggestions/accepted provenance/model identity. Open restores state without executing collection/inference/admission or trusting imported automation consent. | BE-0089, BE-0079 |
| BE-0091 | Verify default-off/on combinations, policy changes mid-inference, manual edits before Add, duplicate ambiguity, reversal and quality thresholds. If thresholds fail, keep automation blocked and report the unmet requirement. | BE-0090, BE-0085 |

Exit: separate opt-ins work to the agreed measured bar, manual values always win and manual
review remains fully functional without inference.

## M-018: Integrated correctness and responsiveness

Goal: verify combined behaviour under realistic workload rather than rely on isolated demos.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0092 | Run deterministic cross-feature scenarios for hostile files/replies, ownership/display cycles, wrong-context identity, stale results and save receipts. Repair defects through existing boundaries. | BE-0091 |
| BE-0093 | Stress bounded queues and approved work limits; verify Stop responsiveness, late results and terminal delivery while AI/file work is busy. No unaccounted background collection after close. | BE-0092 |
| BE-0094 | Measure roughly 250-device workloads by actual card/service/history counts, including animation, view projection, save snapshots and AI memory. Optimise only demonstrated bottlenecks before adding indexes/frameworks. | BE-0093, BE-0002 |
| BE-0095 | Confirm keyboard/focus/error feedback, readable provenance/conflicts, no dead controls and complete translations in integrated map/discovery/file/AI workflows. | BE-0094 |
| BE-0096 | Remove obsolete production simulation paths and obsolete Forget/Reset controls within the approved scope; update README behaviour/layout documentation and run the full required gate. Any retained demo mode requires a separate explicit decision. | BE-0095, BE-0001 |

Exit: all agreed source/interaction/performance criteria pass with evidence; no speculative
cache, storage layer or framework is introduced just to claim future scalability.

## M-019: Self-contained builds on three platforms

Goal: make the first working backend runnable without a development environment or extra installs.
These are local app bundles for acceptance, not a public release or an installer project.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0097 | Produce locally runnable Windows, Linux and macOS app bundles with required native libraries, runtime icons, translations and the single model/tokenizer assets. Document supported OS/CPU baselines. | BE-0096, BE-0012 |
| BE-0098 | Audit actual bundled dependency/load paths, model/signature/data licences and third-party notices. No runtime assets may rely on repository paths or an external scanner/driver/service. | BE-0097 |
| BE-0099 | Launch each bundle normally from its desktop environment on an agreed clean baseline; verify native dialogs, writable locations and network permission denial/grant, especially macOS privacy and Linux UI/render dependencies. | BE-0098 |
| BE-0100 | Verify offline inference from bundled assets with internet unavailable, zero model/runtime download and no telemetry. Local-network scans remain a separately authorised check. | BE-0099 |
| BE-0101 | Reopen a complete project across all three OSes, including embedded custom icons, ownership, committed coordinates/view, history and AI state; disclose platform limits without silently dropping state. | BE-0100 |

Exit: users can run the bundled app on each required OS; a terminal-only run or successful
cross-compile does not establish this. Public signing/distribution and installer polish wait.

## M-020: First working backend acceptance

Goal: close the roadmap with demonstrated behaviour and an honest handover.

| Task ID | Task / completion check | Depends on |
| --- | --- | --- |
| BE-0102 | Confirm exact machines, networks, local/routed/VLAN targets, selected ports/methods and live-test authorisation. Do not start probes or credential operations based on this document alone. | BE-0101, BE-0002 |
| BE-0103 | On each OS, demonstrate real local/routed IPv4 runs, chosen TCP ranges/named UDP/depth, pause/resume/stop, navigation, failure/permission degradation and bounded close. Preserve valid partial results. | BE-0102 |
| BE-0104 | Demonstrate repeat-run identity, conflicts, manual precedence and map preservation; exercise full-state Open/Save/Save As, failed-save/external-change handling and cross-platform reopening with retained history. | BE-0103 |
| BE-0105 | Demonstrate offline AI suggestions plus both opt-ins against the agreed quality/resource criteria, including ambiguity, spoofed/conflicting evidence and unavailable-model fallback. | BE-0104, BE-0091 |
| BE-0106 | Run final required gates, link source/GUI/live/file/AI/platform evidence separately, reconcile every task/blocker and document known limits. Handover as a first working backend only when every required exit check passes. | BE-0105 |

Exit: every mandatory requirement is evidenced on all three OSes. If access, quality or a
required method remains unresolved, record the blocker and remaining task IDs; do not mark
the roadmap Done on the strength of unit tests alone.

## Verification and evidence rules

Every implementation task follows [AGENTS.md](../AGENTS.md), including the ordered full gate:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build
```

Use the maintained `cargo tool checks gate --scope app` command for that sequence; include the
tools gate when tooling changes. Do not weaken lints, toolchain rules or tests. Necessary
tooling lives in maintained task areas under `tools/`, with README/help and tests, never in
product `src` or a disposable task script.

Attach evidence to the matching task/card: revision, platform/environment, scenario/input,
command or user action, expected/actual result and limitations. Gates establish source
health; deterministic fixtures establish the exercised logic; GUI checks establish the
observed interaction; live runs establish only their authorised target/method coverage.
Keep local qualification evidence free of credentials and do not commit private network data.

## Outside this roadmap

Do not allocate implementation tasks for these until separately requested. Capture a future
idea in the blank backlog using a new ID and an explicit later milestone when it is accepted.

- Production-release hardening beyond the working-backend criteria: public installers,
  release signing/notarisation/distribution, automated release pipelines and publication.
- IPv6 probing and zone-aware discovery, managed physical topology, authenticated vendor
  integrations, exports/document generation, semantic search and historical comparison UI.
- Multiple models, training, model-download updates, cloud sync, background monitoring,
  scheduled scans, equipment configuration, remote control or vulnerability testing.
- SQLite migration, independently edited file merging, extra named map views or a UI rewrite.

These deferrals do not relax first-backend safety, complete saving, bundled dependencies,
offline AI, truthful behaviour or the three-platform requirement.
