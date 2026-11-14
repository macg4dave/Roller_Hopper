# GitHub projects for node workspaces and infrastructure management

Reviewed 14 November 2026. Candidate review guide; no roadmap phases or adoption decisions assigned.

## What was imported

Imported **18 repositories as 125 selected, unmodified source/document/licence files** (2,788,500 bytes). These are local reference bundles, not complete runnable clones, app dependencies or incorporated application code.

The [import manifest](../site/research/github-review/manifest.json) records full commit IDs, upstream paths, URLs, byte counts and SHA-256 hashes. Each repository directory also contains IMPORT.json and the original applicable licence/notice files. Sources are under `site/research/github-review/sources/<owner>--<repository>/`, which Git ignores. Local import scripts, selection and inventories are beside the manifest so the review can be reproduced. No downloaded program, demo, server or build script was executed.

This guide is suitable for Git: it contains public research and generic product proposals. Real infrastructure remains in the private workspace. The roadmap was left unchanged for this review.

## Best places to start reviewing

- **Node interaction:** N1 egui-snarl for a focused editor; N4 Graphite for nested graphs, inspectors and edit commands.
- **Notes and infrastructure knowledge:** M1 NetBox, M2 Nautobot and M10 BookStack for inventory, journals, linked notes and revisions.
- **Machine management:** M5/M6 Cockpit, M7 MeshCentral and M8 Rundeck for access, service/VM actions, execution records and recovery UX.
- **Rust operations:** M9 Komodo for procedure and result modelling, with GPL restrictions respected.
- **Optional algorithms:** L1 Petgraph and L2/L3 Sugiyama for specific graph-analysis/layout problems; no dependency needed just to draw nodes.

These are review priorities, not implementation order. No single project supplies our whole device workspace.

## Rust, Slint and reuse boundaries

The application currently uses Slint. egui and Iced node editors cannot be imported as native Slint widgets. Porting useful interaction logic, evaluating a separate prototype, or choosing a different toolkit are distinct decisions. The recommendation for this review is to retain Slint and study the editor patterns first; no toolkit switch or embedding layer has been implemented.

Keep machine/entity IDs separate from canvas/node IDs. Preserve one infrastructure model with typed relationships and per-device view state. An infrastructure dependency graph may contain cycles; an executable workflow needs its own validated control flow. Drawing a wire must not run a command. Auto-layout must preserve domain relationships and allow undo.

## Candidate catalogue

Last push is repository metadata observed during review, not a release date, security assessment or guarantee of maintenance. All imported repositories reported unarchived. Source links below are pinned to the imported commit, even if their default branch changes later.

| ID | Project | Area | Verified licence scope | Last push | Imported commit |
| --- | --- | --- | --- | --- | --- |
| N1 | [zakarumych/egui-snarl](https://github.com/zakarumych/egui-snarl) | Node editor | [MIT OR Apache-2.0](https://github.com/zakarumych/egui-snarl/blob/23e7489b51b56e9f51e756bae8efdabc49f12fec/LICENSE-MIT) | 2026-08-26 | `23e7489b51b5` |
| N2 | [trevyn/egui_node_graph2](https://github.com/trevyn/egui_node_graph2) | Node editor | [MIT](https://github.com/trevyn/egui_node_graph2/blob/0842bc2708af74e0f100f2f4dfcdc54c39a39e52/LICENSE) | 2024-12-18 | `0842bc2708af` |
| N3 | [mkmarek/iced-node-editor](https://github.com/mkmarek/iced-node-editor) | Node editor | [MIT](https://github.com/mkmarek/iced-node-editor/blob/572a04191408674697108fa33819e90cb193d3e5/LICENSE) | 2026-03-26 | `572a04191408` |
| N4 | [GraphiteEditor/Graphite](https://github.com/GraphiteEditor/Graphite) | Rust node-based application | [MIT OR Apache-2.0, unless locally noted](https://github.com/GraphiteEditor/Graphite/blob/86ebb7d92c42bf75c5b0382ca0b72ac4ba0be271/LICENSE-MIT) | 2026-11-14 | `86ebb7d92c42` |
| L1 | [petgraph/petgraph](https://github.com/petgraph/petgraph) | Rust graph algorithms | [MIT OR Apache-2.0](https://github.com/petgraph/petgraph/blob/ae9c2be876891dee24880a156955e94b2e03cf23/LICENSE-MIT) | 2026-11-14 | `ae9c2be87689` |
| L2 | [paddison/rust-sugiyama](https://github.com/paddison/rust-sugiyama) | Rust layered layout | [MIT](https://github.com/paddison/rust-sugiyama/blob/75f31ef9ab7082950ef2f709d3aa05150c0515df/LICENSE) | 2025-09-21 | `75f31ef9ab70` |
| L3 | [austinorr/fast-sugiyama](https://github.com/austinorr/fast-sugiyama) | Rust layout fork and Python wrapper | [MIT; retain both upstream and fork notices](https://github.com/austinorr/fast-sugiyama/blob/07273fb38731d0cf4ef1c5ec78fe93e570c51c50/LICENSE) | 2026-03-13 | `07273fb38731` |
| F1 | [dora-rs/dora](https://github.com/dora-rs/dora) | Rust dataflow runtime | [Apache-2.0](https://github.com/dora-rs/dora/blob/5eb91188e887ac7e43bf790d6a77de9387d4736f/LICENSE) | 2026-11-14 | `5eb91188e887` |
| M1 | [netbox-community/netbox](https://github.com/netbox-community/netbox) | Infrastructure source of truth | [Apache-2.0](https://github.com/netbox-community/netbox/blob/2266fb273ff08812be268d0f949722b78401f294/LICENSE.txt) | 2026-11-14 | `2266fb273ff0` |
| M2 | [nautobot/nautobot](https://github.com/nautobot/nautobot) | Infrastructure automation and source of truth | [Apache-2.0; NOTICE also identifies inherited material](https://github.com/nautobot/nautobot/blob/f29a3bb7488883c8471bdd87459bf944875c8d77/LICENSE.txt) | 2026-11-14 | `f29a3bb74888` |
| M3 | [netdisco/netdisco](https://github.com/netdisco/netdisco) | Network discovery and topology | [BSD-3-Clause](https://github.com/netdisco/netdisco/blob/a8296e59d10a9b576a479372df2e117c538aa8bc/LICENSE.md) | 2026-11-12 | `a8296e59d10a` |
| M4 | [librenms/librenms](https://github.com/librenms/librenms) | Network monitoring | [GPL-3.0-or-later with stated third-party exceptions](https://github.com/librenms/librenms/blob/534df051769b8237b9cfcf8aab0dcebbe0ab19a1/LICENSE.txt) | 2026-11-14 | `534df051769b` |
| M5 | [cockpit-project/cockpit](https://github.com/cockpit-project/cockpit) | Linux server management | [Mixed repository; imported code is LGPL-2.1-or-later](https://github.com/cockpit-project/cockpit/blob/81517745c50d4be3e30403c9bcc2f6ce9d226b14/LICENSES/LGPL-2.1.txt) | 2026-11-13 | `81517745c50d` |
| M6 | [cockpit-project/cockpit-machines](https://github.com/cockpit-project/cockpit-machines) | Virtual machine management | [Imported files: LGPL-2.1-or-later](https://github.com/cockpit-project/cockpit-machines/blob/523c437489f685ff611d178b96b30e4f2fe17567/LICENSE) | 2026-11-13 | `523c437489f6` |
| M7 | [Ylianst/MeshCentral](https://github.com/Ylianst/MeshCentral) | Remote machine management | [Apache-2.0](https://github.com/Ylianst/MeshCentral/blob/b8cac8f2cb701bdfabf32424ddec6f58cde52142/LICENSE) | 2026-11-10 | `b8cac8f2cb70` |
| M8 | [rundeck/rundeck](https://github.com/rundeck/rundeck) | Runbook and job execution | [Apache-2.0](https://github.com/rundeck/rundeck/blob/21f4101fcf98225d9e404722a09e1c6feedfcfcc/LICENSE) | 2026-11-14 | `21f4101fcf98` |
| M9 | [moghtech/komodo](https://github.com/moghtech/komodo) | Rust server/deployment management | [GPL-3.0-or-later](https://github.com/moghtech/komodo/blob/780ac68b992094a9fccd5fffb760e0c84fd3c3d1/LICENSE) | 2026-10-23 | `780ac68b9920` |
| M10 | [BookStackApp/BookStack](https://github.com/BookStackApp/BookStack) | Operational documentation | [MIT](https://github.com/BookStackApp/BookStack/blob/0b6b667e0d84ebb3406de6de9fadbf4230c29c0b/LICENSE) | 2026-11-14 | `0b6b667e0d84` |

## What to reuse and what to avoid

### N1 — zakarumych/egui-snarl

**Assessment: Study first; selective MIT code reuse possible.**

Data-only nodes plus a viewer that controls pins and connection callbacks separate application semantics from rendering. Inspect its collapsed nodes, wire interaction and stored view state. This is the strongest small node-editor reference in this set for our device canvases.

Use the same separation between entity references, sockets, layout and Slint presentation. This is an egui widget, not a Slint component; direct adoption needs an isolated UI integration prototype. Preserve our entity IDs and validation rather than adopting widget-local IDs as machine identity.

Read [src/ui/viewer.rs](https://github.com/zakarumych/egui-snarl/blob/23e7489b51b56e9f51e756bae8efdabc49f12fec/src/ui/viewer.rs) and [upstream licence](https://github.com/zakarumych/egui-snarl/blob/23e7489b51b56e9f51e756bae8efdabc49f12fec/LICENSE-MIT). [Open local bundle](../site/research/github-review/sources/zakarumych--egui-snarl/IMPORT.json).

### N2 — trevyn/egui_node_graph2

**Assessment: Secondary editor reference; check maintenance before dependency use.**

Node templates, categories, typed input/output parameters and a node finder provide concrete examples for an add-node palette and socket rules. The README identifies this as a fork of the archived original; that does not establish its current maintenance health.

Borrow interaction and schema ideas for hardware, workload, note and task nodes. Its inspected last push is in 2024, and it uses egui. Compare its template API with Snarl before copying abstractions; a small purpose-built Slint adaptation may fit better.

Read [egui_node_graph2/src/traits.rs](https://github.com/trevyn/egui_node_graph2/blob/0842bc2708af74e0f100f2f4dfcdc54c39a39e52/egui_node_graph2/src/traits.rs) and [upstream licence](https://github.com/trevyn/egui_node_graph2/blob/0842bc2708af74e0f100f2f4dfcdc54c39a39e52/LICENSE). [Open local bundle](../site/research/github-review/sources/trevyn--egui_node_graph2/IMPORT.json).

### N3 — mkmarek/iced-node-editor

**Assessment: Small interaction-code reference.**

A compact implementation of draggable nodes, connections, panning and zooming. Graph-container translation/scale callbacks and the connection renderer offer a manageable code-reading starting point.

Study coordinate transforms and event ownership against our existing canvas. This is Iced-specific, the project is small, and its API/build compatibility has not been tested. It is not a reason to introduce Iced alongside Slint.

Read [iced_node_editor/src/graph_container.rs](https://github.com/mkmarek/iced-node-editor/blob/572a04191408674697108fa33819e90cb193d3e5/iced_node_editor/src/graph_container.rs) and [upstream licence](https://github.com/mkmarek/iced-node-editor/blob/572a04191408674697108fa33819e90cb193d3e5/LICENSE). [Open local bundle](../site/research/github-review/sources/mkmarek--iced-node-editor/IMPORT.json).

### N4 — GraphiteEditor/Graphite

**Assessment: Study first for a mature editing experience.**

Nested network paths, graph-edit messages, selection/history controls and a document-node model show how a full application grows around a node editor. It is particularly useful for navigating from a device to a workload graph and back.

Adapt the document/view boundary, breadcrumbs, inspectors and edit commands. Graphite is a large graphics application with a Rust core and a different frontend; its graphics execution engine and document stack would be an expensive transplant. Check local exceptions before reusing an individual file.

Read [editor/src/messages/portfolio/document/node_graph/node_graph_message.rs](https://github.com/GraphiteEditor/Graphite/blob/86ebb7d92c42bf75c5b0382ca0b72ac4ba0be271/editor/src/messages/portfolio/document/node_graph/node_graph_message.rs) and [upstream licence](https://github.com/GraphiteEditor/Graphite/blob/86ebb7d92c42bf75c5b0382ca0b72ac4ba0be271/LICENSE-MIT). [Open local bundle](../site/research/github-review/sources/GraphiteEditor--Graphite/IMPORT.json).

### L1 — petgraph/petgraph

**Assessment: Potential library dependency if algorithms justify it.**

StableGraph and the algorithm module are relevant to dependency traversal and graph consistency. The imported snapshot is from the 0.8 branch, whose Cargo manifest reports 0.8.3. The default branch explicitly warns of a new architecture under development.

Keep business IDs independent of graph indices. Use a library for a concrete need such as cycle analysis or impact traversal, not to replace our current model speculatively. No version has been added to Cargo.toml; review the chosen release and dependency licence closure before adoption.

Read [src/graph_impl/stable_graph/mod.rs](https://github.com/petgraph/petgraph/blob/ae9c2be876891dee24880a156955e94b2e03cf23/src/graph_impl/stable_graph/mod.rs) and [upstream licence](https://github.com/petgraph/petgraph/blob/ae9c2be876891dee24880a156955e94b2e03cf23/LICENSE-MIT). [Open local bundle](../site/research/github-review/sources/petgraph--petgraph/IMPORT.json).

### L2 — paddison/rust-sugiyama

**Assessment: Layout experiment candidate.**

The original Rust Sugiyama implementation is a focused reference for layered directed layouts, with configurable layout behaviour. Its source is much closer to an optional arrange-nodes operation than a complete GUI toolkit.

Evaluate on variable-size device nodes, typed socket positions, external-resource references and disconnected groups. Treat layout cycle removal as an internal calculation: it must not delete or reverse stored infrastructure dependencies. Preserve manual placement and make arrangement undoable.

Read [src/algorithm/mod.rs](https://github.com/paddison/rust-sugiyama/blob/75f31ef9ab7082950ef2f709d3aa05150c0515df/src/algorithm/mod.rs) and [upstream licence](https://github.com/paddison/rust-sugiyama/blob/75f31ef9ab7082950ef2f709d3aa05150c0515df/LICENSE). [Open local bundle](../site/research/github-review/sources/paddison--rust-sugiyama/IMPORT.json).

### L3 — austinorr/fast-sugiyama

**Assessment: Compare with L2; do not adopt both.**

A fork with a Rust core and Python bindings that can be compared against the original for layout behaviour and performance. The imported core manifest names log and petgraph dependencies; the repository description's no-dependencies claim should not be used as a dependency audit.

Inspect the Rust crate only for a Rust app; the Python layer is not required. Benchmark representative graphs and crossing/overlap quality before choosing either implementation. The smaller contributor footprint and derivative history are adoption risks, not proof that it is faster for our workload.

Read [crates/rust-sugiyama/src/lib.rs](https://github.com/austinorr/fast-sugiyama/blob/07273fb38731d0cf4ef1c5ec78fe93e570c51c50/crates/rust-sugiyama/src/lib.rs) and [upstream licence](https://github.com/austinorr/fast-sugiyama/blob/07273fb38731d0cf4ef1c5ec78fe93e570c51c50/LICENSE). [Open local bundle](../site/research/github-review/sources/austinorr--fast-sugiyama/IMPORT.json).

### F1 — dora-rs/dora

**Assessment: Descriptor and validation reference.**

Dataflow descriptors, input/output validation and event models are useful examples for describing a runnable flow separately from a visual graph. The imported error-propagation example is relevant to showing failed or interrupted steps.

Borrow declarative validation and lifecycle ideas for action nodes. Dora is robotics/distributed middleware with a much larger runtime surface than our current need. A graph that documents relationships must remain distinct from executable operations; installing a robotics runtime is not proposed.

Read [libraries/core/src/descriptor/validate.rs](https://github.com/dora-rs/dora/blob/5eb91188e887ac7e43bf790d6a77de9387d4736f/libraries/core/src/descriptor/validate.rs) and [upstream licence](https://github.com/dora-rs/dora/blob/5eb91188e887ac7e43bf790d6a77de9387d4736f/LICENSE). [Open local bundle](../site/research/github-review/sources/dora-rs--dora/IMPORT.json).

### M1 — netbox-community/netbox

**Assessment: Study first for inventory and object journals.**

Device, cable and VM models demonstrate inventory objects independent of scan results. The imported journaling guide distinguishes human context from automatic change logs, including author/time and entry kind.

Use these distinctions for machine profiles, notes, evidence and relationships. This is a Python/Django server, not a Rust library. Adapt the model concepts or later use a configured API; do not import the whole backend or imply that a source-of-truth record is a current live observation. Preserve NOTICE for any code reuse.

Read [netbox/dcim/models/devices.py](https://github.com/netbox-community/netbox/blob/2266fb273ff08812be268d0f949722b78401f294/netbox/dcim/models/devices.py) and [upstream licence](https://github.com/netbox-community/netbox/blob/2266fb273ff08812be268d0f949722b78401f294/LICENSE.txt). [Open local bundle](../site/research/github-review/sources/netbox-community--netbox/IMPORT.json).

### M2 — nautobot/nautobot

**Assessment: Study first for jobs, changes and linked notes.**

Job definitions distinguish available code and its source version from actual job results. Change logging records object changes, and object-note views provide a reference for keeping operational annotations attached to entities.

Use the job-definition/job-run distinction for management nodes and record import provenance. Its Django/Celery-oriented backend is not a drop-in. If extracting code, retain NOTICE and follow the listed inherited licences, rather than treating the top-level Apache label as the whole provenance story.

Read [nautobot/extras/models/jobs.py](https://github.com/nautobot/nautobot/blob/f29a3bb7488883c8471bdd87459bf944875c8d77/nautobot/extras/models/jobs.py) and [upstream licence](https://github.com/nautobot/nautobot/blob/f29a3bb7488883c8471bdd87459bf944875c8d77/LICENSE.txt). [Open local bundle](../site/research/github-review/sources/nautobot--nautobot/IMPORT.json).

### M3 — netdisco/netdisco

**Assessment: Topology/evidence model reference.**

Neighbour discovery, topology records, per-port VLAN data and node first/last-seen fields give useful examples of evidence that changes over time. This is a stronger discovery-history reference than a static picture of links.

Adapt the separation of discovered neighbours, manual topology and observations. This is a Perl/SNMP system, not Rust code to paste into our scanner. Retain provenance and non-endorsement notices if copying material; do not enable SNMP or collect credentials merely to test it.

Read [lib/App/Netdisco/Worker/Plugin/Discover/Neighbors.pm](https://github.com/netdisco/netdisco/blob/a8296e59d10a9b576a479372df2e117c538aa8bc/lib/App/Netdisco/Worker/Plugin/Discover/Neighbors.pm) and [upstream licence](https://github.com/netdisco/netdisco/blob/a8296e59d10a9b576a479372df2e117c538aa8bc/LICENSE.md). [Open local bundle](../site/research/github-review/sources/netdisco--netdisco/IMPORT.json).

### M4 — librenms/librenms

**Assessment: Behavioural reference or future API integration.**

Neighbour discovery and the imported alert/device/history models are relevant to separating topology, observations, fault state and alert history. Read the README's exceptions as well as LICENSE.txt.

Use it to evaluate monitoring and diagnostic UX, with notifications optional. Do not copy or translate GPL implementation into the existing MIT-labelled app while representing the result as MIT-only. An independently implemented design or separately reviewed API integration is the proposed route.

Read [LibreNMS/Discovery/Neighbors/NeighborFinder.php](https://github.com/librenms/librenms/blob/534df051769b8237b9cfcf8aab0dcebbe0ab19a1/LibreNMS/Discovery/Neighbors/NeighborFinder.php) and [upstream licence](https://github.com/librenms/librenms/blob/534df051769b8237b9cfcf8aab0dcebbe0ab19a1/LICENSE.txt). [Open local bundle](../site/research/github-review/sources/librenms--librenms/IMPORT.json).

### M5 — cockpit-project/cockpit

**Assessment: Study first for a machine workspace and jobs.**

Service details, the service helper, machine connection state and storage-job presentation show how operational state, permissions, pending work and errors can coexist in a machine interface.

Adapt the user journeys into device nodes and an inspector, or link to an existing management UI later. The inspected files have explicit LGPL identifiers; the repository also contains GPL, BSD, MIT and CC-BY-SA material. Do not port these components into Slint as if they were MIT.

Read [pkg/systemd/service-details.jsx](https://github.com/cockpit-project/cockpit/blob/81517745c50d4be3e30403c9bcc2f6ce9d226b14/pkg/systemd/service-details.jsx) and [upstream licence](https://github.com/cockpit-project/cockpit/blob/81517745c50d4be3e30403c9bcc2f6ce9d226b14/LICENSES/LGPL-2.1.txt). [Open local bundle](../site/research/github-review/sources/cockpit-project--cockpit/IMPORT.json).

### M6 — cockpit-project/cockpit-machines

**Assessment: VM operations and console UX reference.**

VM confirmation dialogs, console choices, disk presentation and the libvirt provider illustrate operations with concrete guest/host context. Read source headers rather than relying on GitHub's simplified licence badge.

Use the workflow ideas for guest power actions, resource inspectors and recovery console selection. This is a Cockpit/TypeScript module with external dependencies. Prefer an independent Rust/Slint implementation or a reviewed external-management link; copying or translating source retains its licence obligations.

Read [src/libvirtApi/domain.ts](https://github.com/cockpit-project/cockpit-machines/blob/523c437489f685ff611d178b96b30e4f2fe17567/src/libvirtApi/domain.ts) and [upstream licence](https://github.com/cockpit-project/cockpit-machines/blob/523c437489f685ff611d178b96b30e4f2fe17567/LICENSE). [Open local bundle](../site/research/github-review/sources/cockpit-project--cockpit-machines/IMPORT.json).

### M7 — Ylianst/MeshCentral

**Assessment: Access/authorisation patterns; possible separate integration.**

Server-side user/agent handlers demonstrate how device groups, rights, sessions and remote access participate in a full management system. The README describes remote desktop, terminal and file management rather than just host discovery.

Study access-profile and operation boundaries, including disconnected agents and authorised sessions. MeshCentral is an agent/server platform in JavaScript. Do not install agents or replace our local architecture for this review; source modules have substantial internal dependencies and need more than a top-level licence check.

Read [meshuser.js](https://github.com/Ylianst/MeshCentral/blob/b8cac8f2cb701bdfabf32424ddec6f58cde52142/meshuser.js) and [upstream licence](https://github.com/Ylianst/MeshCentral/blob/b8cac8f2cb701bdfabf32424ddec6f58cde52142/LICENSE). [Open local bundle](../site/research/github-review/sources/Ylianst--MeshCentral/IMPORT.json).

### M8 — rundeck/rundeck

**Assessment: Study first for execution semantics.**

Execution/workflow records and an authorising node-execution service are concrete references for separating a saved runbook, the target set, authorisation and an actual run's result.

Translate the conceptual separation into action/runbook/result nodes, not the Java/Groovy service stack. Review error handling, target checks and audit fields before designing the executor. Community source does not imply rights to separately licensed enterprise plugins or assets.

Read [rundeckapp/src/main/groovy/rundeck/services/AuthorizingNodeExecutionService.groovy](https://github.com/rundeck/rundeck/blob/21f4101fcf98225d9e404722a09e1c6feedfcfcc/rundeckapp/src/main/groovy/rundeck/services/AuthorizingNodeExecutionService.groovy) and [upstream licence](https://github.com/rundeck/rundeck/blob/21f4101fcf98225d9e404722a09e1c6feedfcfcc/LICENSE). [Open local bundle](../site/research/github-review/sources/rundeck--rundeck/IMPORT.json).

### M9 — moghtech/komodo

**Assessment: Strong Rust operational reference; no direct MIT-only port.**

Procedure stages execute in sequence with parallel executions inside a stage. Server, update and log models are directly relevant Rust examples for machines, deployment jobs and their recorded outcomes.

Study resource/action/run separation, stage semantics and error/result reporting. This is the clearest Rust infrastructure-management candidate here, but its GPL licence is a real boundary. Keep imported source for study; use independently written behaviour or consider a separately reviewed API integration rather than copying into an MIT-only derivative.

Read [client/core/rs/src/entities/procedure.rs](https://github.com/moghtech/komodo/blob/780ac68b992094a9fccd5fffb760e0c84fd3c3d1/client/core/rs/src/entities/procedure.rs) and [upstream licence](https://github.com/moghtech/komodo/blob/780ac68b992094a9fccd5fffb760e0c84fd3c3d1/LICENSE). [Open local bundle](../site/research/github-review/sources/moghtech--komodo/IMPORT.json).

### M10 — BookStackApp/BookStack

**Assessment: Study first for notes, revisions and activity.**

Page and revision records preserve content, author, time, revision number and change summary. Activity records provide a useful distinction between editing a document and recording an operational event.

Use the knowledge model for canonical runbooks, device-linked notes and revisions rather than another plain text field. This is PHP/Laravel, so the useful reuse is design or selective algorithms, not a runtime dependency. The GitHub README now points to Codeberg as the main home; future updates must follow that move.

Read [app/Entities/Repos/RevisionRepo.php](https://github.com/BookStackApp/BookStack/blob/0b6b667e0d84ebb3406de6de9fadbf4230c29c0b/app/Entities/Repos/RevisionRepo.php) and [upstream licence](https://github.com/BookStackApp/BookStack/blob/0b6b667e0d84ebb3406de6de9fadbf4230c29c0b/LICENSE). [Open local bundle](../site/research/github-review/sources/BookStackApp--BookStack/IMPORT.json).

## Licence handling for the imported material

The imported files retain their upstream content, copyright headers, directory paths and applicable licence notices. A licence badge was used only as a search hint; the decisions above use licence files, Cargo declarations, README notices and individual source headers where present. No third-party source was relicensed as this application's MIT code.

| Licence family | Conditions to carry into any actual adoption | Current treatment |
| --- | --- | --- |
| MIT | Keep copyright and permission notices with copies or substantial portions | Reference imports retain original notices; dual MIT/Apache candidates may be evaluated under MIT for a selected file where that choice applies |
| Apache-2.0 | Carry the licence, applicable attribution/NOTICE content and notices of modifications; retain the upstream licence on reused material | Root NOTICE files imported for NetBox, Nautobot and Dora; no source modifications or incorporation yet |
| BSD-3-Clause | Retain copyright, conditions and disclaimer; do not imply endorsement | Netdisco licence retained |
| LGPL-2.1-or-later | Linking, modification and redistribution conditions require a concrete integration assessment; translating source does not erase its licence | Cockpit code imported for reference only; no direct component port selected |
| GPL-3.0-or-later | Copied/adapted implementation cannot be represented as an MIT-only derivative; distributing a combined derivative may require corresponding source and GPL terms | LibreNMS and Komodo isolated as reference imports; independent implementation or external integration is the proposed route |
| Mixed licences / third-party dependencies | A repository-level label does not clear every file, asset, font, plugin or dependency | Only selected files and relevant notices imported; no icons, screenshots, fonts, release binaries or transitive libraries were copied |

Before bringing any source into the application, record the exact upstream commit/path, chosen licence, retained notice path and local modifications in a third-party attribution record. Review that selected component's dependencies and assets, test its compatibility, and only then add a narrowly scoped dependency or port. That full distribution audit has not been performed here; reference import is not an approval to ship a library or service.

A separately deployed service/API is an architectural option, not a blanket exemption from licence conditions. Its deployment, client code, branding, plugins and any modifications still need review. Public availability alone is not permission to copy unlicensed code.

## Review exercises before adoption

| Exercise | What to inspect | Decision it should resolve |
| --- | --- | --- |
| Editor interaction | N1/N2/N3: create node, validate sockets, reconnect wire, select/group, reopen layout; N4: nested graph navigation and edit history | Which behaviours to adapt into Slint; whether any isolated widget prototype is worth testing |
| Layout quality | L2 versus L3 on variable-size nodes, cycles, disconnected components, pinned positions and external references | Whether a layout library materially improves the existing canvas; select one implementation only |
| Notes and journals | M1/M2/M10: canonical document, imported source, manual journal, edit revision and audit event | Separate record types and conflict/revision handling without copying a whole wiki |
| Operations | M5–M9 and F1: saved definition, target context, execution prerequisites, interruption, failure and result history | The minimum action/runbook/result model; no remote actions are run during this source review |
| Discovery evidence | M3/M4: neighbour observations, manual links, first/last seen and alert history | How to preserve uncertainty and stale evidence instead of flattening everything into one status |

Use synthetic data in committed experiments; use the private real projects for local acceptance once an experiment exists. No candidates were built, benchmarked, visually exercised or security-audited in this review. In particular, screenshots and star counts have not been used as proof that a library will fit the application.

## Candidates deliberately not imported

- [dagrs-dev/dagrs](https://github.com/dagrs-dev/dagrs): repository metadata reported archived. Dora and the operational projects are better starting points for active-runtime comparisons; no archived runtime dependency is proposed.
- The original editor lineage named in N2's pinned README: that README describes the original as archived, and the named original URL returned 404 during this review. N2 provides the comparison without importing another copy or assuming an unavailable upstream is maintained.
- The searches did not establish a maintained Slint-native node editor suitable for direct use. This is a search result limitation, not a claim that none exists.

## Decisions for your review

1. Choose the editor behaviours worth prototyping from N1/N4, with N2/N3 as alternatives.
2. Choose the knowledge model ideas from NetBox/Nautobot/BookStack and the operation model ideas from Cockpit/Rundeck/Komodo.
3. Decide whether to build native capabilities, integrate an existing service, or simply expose a management link for each selected area.
4. Validate the chosen component's licence and technical fit before importing implementation into app code.
5. Only after that review, decide where the selected work belongs on the roadmap. No phase assignment or feature implementation was made by this research task.
