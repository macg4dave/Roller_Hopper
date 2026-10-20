# Decisions and questions

20 October 2026. This is the decision register for [the review](backend-review.md), not an
implementation backlog. Recommendations below are proposed defaults, not silent approvals.
The questions already asked in the conversation have been incorporated here.

## Answers received

| ID | User direction | Architectural effect |
| --- | --- | --- |
| Q1 | Windows, Linux and macOS from the start | Shared model and protocols; explicit platform capability, packaging and acceptance matrix in every delivery phase |
| Q2 | All three proposed network categories: selected local subnets, routed/VLAN networks and broad port scanning | Target/route context and bounded work from the outset; neither local ARP nor ICMP alone can cover this scope |
| Q3 | Review and manual add; user chooses whether AI can auto-add | Preserve the existing discovery review workflow; model policy separately from finding/identity confidence |
| Q8 | IPv4 first; IPv6 later | Do not block first release on IPv6 scanning; keep scoped multi-address design open to a later dual-stack extension |
| Q10 | Earlier direction allowed user-selected AI field updates; show confidence for review | Refined by Q10a below: manual fields are protected, not candidates for an automatic-overwrite setting. Admission policy and confidence still need explicit provenance. |
| Q11 | One or two existing small models, bundled, offline, CPU, using Rust tools; no custom AI system | Reuse pretrained weights and an inference runtime. No custom training pipeline, cloud dependency, GPU requirement or AI service |
| Q21 | Everything inside the app; no outside servers or other programs | All backend/inference runs in process. Exclude child executables, scanner/model programs, localhost servers and separately installed helper services; native dependencies ship as app libraries. |
| Q22 | Produce save files and open files | Native Open/Save/Save As and portable user-owned map documents belong in the first release; a private cache/database alone is insufficient. Format and save details remain below. |
| Q4a | Selected TCP ranges plus named UDP discovery protocols | Broad coverage is user-selected TCP; no blanket UDP port sweep. Arbitrary routed-range syntax, exclusions and resource limits remain Q4/Q7. |
| Q6a | Pause launches no new work; in-flight probes may finish and display results | Pause is a scheduling boundary. Navigation, close/restart and shutdown/save policy remain Q6. |
| Q22b | Versioned JSON with explicit Open/Save/Save As | Native format direction is agreed; no initial SQLite inventory. Recovery and custom-icon scope are refined by Q13a/Q22c; extension, conflicts and saved-view/history remain Q13/Q22a. |
| Q7a | Home/small-office networks, up to roughly 250 devices | First-release acceptance targets this scale; card/service counts, port workload, latency, rates and memory limits still need Q7. This is not proof of throughput or a hard device cap. |
| Q16c | Up to roughly 700 MB of bundled AI weights if useful on a normal CPU | Qwen's approximately 639 MB artifact fits the weight allowance; runtime/installer size, peak RAM, CPU latency and usefulness remain unverified. No model is selected by this answer. |
| Q14 | Keep the inferred card-based tree; managed topology later | No first-release physical switch-port/loop requirement or graph rewrite. Authenticated topology collectors remain future scope. |
| Q5 | Manual scans; optional lightweight refresh | Broad scans remain explicit. Lightweight refresh is opt-in; cadence, probe budget and stale-status thresholds remain Q5a. |
| Q10a | “Manual is king”; AI is most useful for first-time devices | Manual fields always win. Prioritise suggestions for new devices; no automatic replacement of manual fields. Auto-add and filling missing inferred fields remain separate policy decisions. |
| Q6b | Continue scans across pages; cancel on app close | Run lifetime belongs to the app, not the discovery page. Retain partial findings, handle unsaved changes, and never resume traffic automatically after restart. |
| Q13a | Keep a separate crash-recovery copy | Explicit Save still controls the user's file; recovery is labelled unsaved, separate and subject to validation. Cadence/retention remain open. |
| Q22c | Do not worry about custom-icon saving | Defer embedded icons and companion-folder packaging. Save safe icon identifiers and use a built-in fallback when unavailable; missing artwork must not prevent Open. |
| Q23 | Use upstream nrev in the normal Cargo dependency workflow; no maintained fork | Reuse the embedded library and ordinary dependency upgrades; no copied scanner, local patches, submodule or custom scanning engine. Keep supported backend options broader than the initial GUI. See [integration and qualification](backend-scanner-engine.md). |
| Q23a | Users only run the app; no external software, services or servers | Preserve Q21, including no extra capture-driver setup. Published nrev 0.7.0 has a Windows Packet-link dependency obstacle; selection of the library does not establish standalone compatibility. |
| Q24 | This work is documents only; read code to understand direction, do not edit it | Record the chosen approach and compatibility gaps. Do not add dependencies, modify the discovery workflow or create implementation tasks in this session. |

For Q9, the user is unsure of conflict resolution and asked for a way to alert them. The
proposed answer is an inline, persistent comparison warning with source/time and explicit
Keep my value / Use detected value actions. The alert design is a recommendation; no conflict
resolution was approved silently. See [field precedence](backend-architecture.md#field-precedence-and-topology).

Concrete desired inference: hostname `DESKTOP-13123` with RDP ports open can be labelled a
likely Windows PC and have missing details suggested. This is a requested **guess**, not
evidence that every RDP listener is Windows. It intentionally adds OS-family inference beyond
the prototype's current product brief. A confidence display must distinguish a model's score
from measured accuracy and from identity confidence.

## Priority questions before architecture approval

These affect schema, authority, resource use or first-release acceptance. They can be answered
in groups; implementation should not start by guessing the answers.

### Scanning and operations

| ID | Question and impact | Recommended default |
| --- | --- | --- |
| Q4 | With transport scope agreed in Q4a, which routed-range forms and exclusions are required? Changes target validation and work bounds. | Explicit IPv4 CIDRs/addresses and exclusions; no internet-wide expansion. |
| Q5a | For the optional lightweight refresh, what cadence/traffic is acceptable, and when should stale become Offline? Defines background load and status meaning. | Refresh known endpoints only while enabled; bound the budget; no broad scan on a newly joined network. Show last-seen/stale before asserting Offline. |
| Q6 | With navigation/close agreed in Q6b, how long may close wait for cancellation or a save, and how should an unresponsive collector be presented? Defines shutdown acceptance. | Finite, visible shutdown; never report cancellation before owned work is accounted for. A timeout cannot prove a blocking native task stopped. |
| Q7 | For the agreed roughly 250-device target, how many cards/services, which subnets/port ranges and what latency/CPU/RAM limits should be accepted? Probe work depends on more than device count. | Agree a representative small-office workload; bound every run and measure large work estimates before launch. No invented throughput promise. |
| Q17 | Is OS identification limited to passive/selected service evidence, or should additional fingerprint probes be available? Changes traffic and error expectations. | Explainable inference from already permitted evidence; no active OS fingerprinting, vulnerability checks or extra probes merely because AI requests them. |
| Q18 | May optional native discovery require elevation, and is any non-topology authenticated device enrichment essential? Physical managed topology is deferred by Q14. | Ordinary scans need no mandatory admin path. External capture drivers/helper programs are excluded by Q21. Any read-only enrichment needs named equipment and explicit credentials. |

### Data ownership, topology and storage

| ID | Question and impact | Recommended default |
| --- | --- | --- |
| Q9 | Does the proposed field warning make conflicting manual/detected values clear enough, and which differences merit a renewed alert? Determines acknowledgement and noise. | Preserve manual assertions and observations separately; show both with source/time. Keep my value acknowledges that difference; Use detected value is an explicit edit. Never configure equipment. |
| Q15 | Does Forget remove from map, suppress rediscovery, or delete history? Should forgetting a host remove its guests? Should Reset layout also reset manual connectivity? Changes data-loss semantics. | Separate map removal, suppression and permanent deletion; no automatic history purge. Make cascading effects explicit. Keep current reset behaviour pending an explicit decision. |
| Q12 | How long should address history, raw evidence and run logs survive? Must local data/backups be encrypted? Who may access them on shared machines? Sets storage/security requirements. | Per-user local data, no raw packets, bounded diagnostic evidence; choose retention before schema approval. Encryption is not provided merely by SQLite. |
| Q13 | With separate recovery agreed in Q13a, how often should it write, how many copies survive and how should two writers/failed saves behave? Determines recovery and conflict handling. | Dirty indicator and Save/Discard/Cancel on replacement/close; one writer per file; bounded, labelled recovery that never silently overwrites the saved document. |
| Q19 | Should one save file hold several named network contexts? Is merging independently edited files needed, or just opening/saving each? Affects identity scope and whether UUIDs are justified. | Each document can hold distinct network contexts; Open replaces the active document after handling unsaved edits. File merging is separate unapproved scope. |
| Q22a | With versioned JSON agreed and custom-icon saving deferred, what extension and views/settings/history should survive? Determines document scope and validation. | Snapshot inventory, current evidence, user edits, roles/groups and map arrangement; safe icon identifiers with fallback. Keep credentials and model weights outside documents. |

### Embedded AI and acceptance

| ID | Question and impact | Recommended default |
| --- | --- | --- |
| Q10b | With manual fields protected by Q10a, may AI fill missing inferred fields or auto-add a first-time device, and should this be global or per network? Determines limited automation and reversibility. | Suggestions first; separate opt-in for filling missing inferred fields and auto-add. Manual/observed values stay protected; identity ambiguity always requires review. |
| Q16 | With roughly 700 MB of weights allowed by Q16c, what baseline CPU, peak RAM, latency and useful fields/categories must inference meet? Determines feasibility beyond download size. | One CPU model first, bundled with its licence/hash; measure actual RAM/latency on baseline PCs. Add a second only for a demonstrated improvement. |
| Q16a | What false-positive rate is acceptable for review suggestions versus filling missing inferred fields/auto-add? How should confidence appear before calibration? Defines evidence needed for limited automation. | Allow Unknown/abstention; display an explicitly uncalibrated model score until evaluated. Keep automatic thresholds disabled until validated; manual replacement is excluded. |
| Q16b | Should models ever be updated from a download or learn locally from corrections? Is any off-device telemetry permitted? These are different from refreshing guesses using new observations. | Fully offline packaged inference, no telemetry or self-training. New observations may refresh suggestions; weights change only with an explicitly installed app/model update. |
| Q20 | After decisions, may the protected product guide be updated to the agreed backend/AI scope and actual inline-edit UI? Its current frontend-only/no-OS-guessing rules still apply. | Approve a specific scope update before implementation; this documentation task leaves all rules untouched. |

## Research decisions still open

- Stock upstream nrev must satisfy standalone packaging and the required scan lifecycle before
  integration can be approved; the confirmed Windows Packet-link obstacle is recorded in
  [the engine note](backend-scanner-engine.md#confirmed-windows-dependency-issue). No fork or
  external-driver workaround is authorised.
- A maintained safe neighbour-table/ICMP path must be qualified on each OS under the existing
  no-unsafe rule; current candidates are not a verified drop-in solution.
- Embedded model and Rust runtime must be selected together. Off-the-shelf does not mean
  calibrated for network-device facts; model packaging and evidence evaluation are necessary.
- Native documents use the agreed versioned JSON direction with one in-memory authority.
  SQLite is not selected; future history/scale needs would require a separate decision, with
  migration compatibility unresolved.
  No protected toolchain upgrade is assumed.
- No live platform, broad-scan performance, storage recovery or model-quality evidence was
  produced by this documentation-only review.

After answers, revise the architecture and conditional roadmap together, then seek approval
of that concrete plan. No issue cards, source stubs or implementation tasks have been created.
