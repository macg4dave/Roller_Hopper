# Conditional implementation roadmap

This is proposed sequencing for discussion, **not an approved development plan or task list**.
The review has considered the questions and recorded answers in
[the decision register](backend-decisions.md). Remaining design decisions must be resolved
before this sequence is converted into implementation work. No dates or effort estimates are
credible until workload, protocol and model requirements are settled.

The first release is cross-platform, IPv4, local and routed/VLAN capable, includes selected
broad port coverage and the agreed offline CPU AI behaviour. Phases below are integration
boundaries, not permission to quietly drop a confirmed requirement from that release.
It also includes user-owned Open/Save/Save As documents. Every runtime component is in process;
no external scanner/model executables, servers or separately installed helper services.
The [map persistence contract](backend-map-persistence.md) also preserves display trees,
ordered arrangement and committed positions independently of network connections. Future
free positioning moves displayed branches together; placement/routing choices remain open.
The follow-up confirmed selected TCP ranges plus named UDP protocols, versioned JSON with
explicit saves, and pause allowing bounded in-flight probes to finish. Remaining questions
still gate architecture approval; those answers are not implementation authorisation.
First-release acceptance targets home/small-office networks of roughly 250 devices. Up to
roughly 700 MB of AI weights is acceptable if useful on a normal CPU; RAM, latency and accuracy
still need agreed limits and evidence. Keep the inferred tree; managed topology comes later.
New answers establish manual precedence over AI, manual scans/refresh only, scans continuing
across pages and cancelling on close, and separate crash recovery. Save everything includes
the parent-device/child-interface hierarchy and all restorable application/project state;
asset and sensitive-data representation still needs an explicit design. There are no Forget
or Reset workflows. Identification depth is user-selected within the adopted tools' support.
The proposed mismatch warning still needs review; these
answers do not approve implementation or settle the remaining resource/accuracy questions.
The scanner approach is upstream netscli-core through ordinary Cargo upgrades, with no fork or
custom engine. Phase 0 must resolve [stock-release packaging and lifecycle gaps](backend-scanner-engine.md),
including release/toolchain requirements and OS-command call paths; Phase 2 integrates the qualified library rather
than implementing its probes. The user's latest instruction limits this session to documents.

| Phase | Intended outcome after approval | Entry dependency | Evidence required before proceeding |
| --- | --- | --- | --- |
| 0 — agree the contract | Approved scope, data authority, native document/save policy, identity, run semantics, limits, AI policy and model shortlist | Priority questions answered; concrete product-guide amendment approved | Written decisions, chosen acceptance workloads and in-process packaged-model feasibility; explicit unresolved gaps cannot be called supported |
| 1 — establish truthful data/lifecycle | Owned metadata, scoped addresses, durable keys, manual precedence and generation-aware outcomes; retain one UI-owned Rust document | Phase 0 | Existing map/list/edit/role/group/discovery interactions, including pre-admission and shared mapped-device editing, preserved; stale events, conflicts and bounded UI batch time checked without live traffic |
| 2 — first real vertical slice | OS target discovery and bounded ordinary-socket collection on Windows/Linux/macOS; usable manual Add/review and truthful partial outcomes | Phase 1; safe OS/library path selected | Authorised local and routed IPv4 runs on every OS; target/interface accuracy, pause/stop/close, permission degradation and no UI blocking |
| 3 — durable useful inventory | Complete-state Open/Save/Save As and separate recovery, user overlays, identity continuity, user-started service discovery, bounded broad TCP coverage and selected identification depth | Phase 2; document representation/retention/save/port-scope decisions | Save/reopen all restorable state including multiple child interfaces, view/settings/history; ordered writes and old-save receipts after Open/Save As; dirty/recovery handling; DHCP/IP reuse, conflict warnings, repeat-run deduplication and measured broad-scan limits; no automatic network refresh |
| 4 — embedded inference and policy | One bundled pretrained CPU model primarily assists first-time device review; missing-field automation and auto-add follow agreed settings; manual values remain protected | Model/runtime feasible in Phase 0; stable evidence flow; AI policy agreed | Offline operation on all OSes; review/undo; manual edits during inference; unknown/conflicting/spoofed input; installed-size, CPU/RAM and quality evidence against a simple evidence baseline |
| 5 — first-release acceptance | All confirmed requirements demonstrated in distributable packages | Earlier boundaries complete, any required named integrations included | Cross-platform packages/assets/notices; accessible and visual workflows; bounded large runs, cancellation, storage and offline AI evidence; remaining limitations documented |

Model feasibility belongs in Phase 0 so a small-model limitation is found before promising
an AI capability. A bespoke training project is not the fallback: reconsider the pretrained
model/expected fields with the user if available models cannot meet the agreed bar.

## Preserve the GUI during integration

Keep the current pages/forms and model conversion path. Replace simulated answer production
with real observations under the same user workflow once lifecycle and identity are coherent.
The old fake refresh cannot remain on a real Start path. Production must not quietly fall back
to fabricated devices; any retained sample mode needs an explicit product decision and clear
labelling, not a parallel production inventory architecture.

Reconcile asynchronous results by stable keys while preserving selection, text drafts,
dragging, pan/zoom, role/group styling and accepted manual values. Add only the UI states needed
to express real capability, failure, inference and user policy truthfully. Avoid a replacement
dashboard or a separate AI control centre.

## Future verification boundaries

When implementation is authorised, run the complete required fmt → clippy → test → build gate
in order, and the separate tools gate if tooling changes. Gates are necessary source evidence;
they do not establish real-network or packaged-product acceptance.

Use deterministic fixtures for identity, expiry, malformed replies, cancellation, queue
pressure, migration and policy logic. Live scans, multicast, credentials, privileged capture
and Slint MCP interaction require separately authorised sessions and selected targets. None
was run for this review.

Acceptance should include: validated imports including rootless cycles and cross-record
conflicts; nonblocking UI commands and Stop under a full queue; empty-inventory bootstrap;
local and routed devices without MACs; changed/reused IPs and overlapping network contexts;
optional methods denied; slow/absent
responses; pause/stop/restart and close during collection; edits during enrichment; no loss of
manual choices; repeat discovery without duplicate devices; save failure and recovery; and
offline inference with automatic policy off/on. Use the agreed scale on each OS, not just the
mock fixture. Do not treat a timer reaching 100% as a terminal success.
Check macOS permissions in a normally launched package, mDNS monitor/disconnect outcomes, and
save-file access metadata; source gates and Terminal runs do not establish those behaviours.

Deferred IPv6, physical-topology depth beyond named first-release needs, history/documentation
exports and other enhancements remain in [opportunities](backend-opportunities.md). No work
items have been opened for them.
