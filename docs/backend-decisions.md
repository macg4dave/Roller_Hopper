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
| Q10 | User chooses whether AI may update/replace fields; show confidence for review | Overwrite/admission controls need explicit settings and provenance; manual-wins is only the proposed initial default |
| Q11 | One or two existing small models, bundled, offline, CPU, using Rust tools; no custom AI system | Reuse pretrained weights and an inference runtime. No custom training pipeline, cloud dependency, GPU requirement or AI service |
| Q21 | Everything inside the app; no outside servers or other programs | All backend/inference runs in process. Exclude child executables, scanner/model programs, localhost servers and separately installed helper services; native dependencies ship as app libraries. |
| Q22 | Produce save files and open files | Native Open/Save/Save As and portable user-owned map documents belong in the first release; a private cache/database alone is insufficient. Format and save details remain below. |
| Q4a | Selected TCP ranges plus named UDP discovery protocols | Broad coverage is user-selected TCP; no blanket UDP port sweep. Arbitrary routed-range syntax, exclusions and resource limits remain Q4/Q7. |
| Q6a | Pause launches no new work; in-flight probes may finish and display results | Pause is a scheduling boundary. Navigation, close/restart and shutdown/save policy remain Q6. |
| Q22b | Versioned JSON with explicit Open/Save/Save As | Native format direction is agreed; no initial SQLite inventory. Extension, icon portability, recovery, conflicts and saved-view/history content remain Q13/Q22a. |
| Q7a | Home/small-office networks, up to roughly 250 devices | First-release acceptance targets this scale; card/service counts, port workload, latency, rates and memory limits still need Q7. This is not proof of throughput or a hard device cap. |
| Q16c | Up to roughly 700 MB of bundled AI weights if useful on a normal CPU | Qwen's approximately 639 MB artifact fits the weight allowance; runtime/installer size, peak RAM, CPU latency and usefulness remain unverified. No model is selected by this answer. |
| Q14 | Keep the inferred card-based tree; managed topology later | No first-release physical switch-port/loop requirement or graph rewrite. Authenticated topology collectors remain future scope. |

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
| Q5 | Should known devices refresh only on request, periodically while open, or continuously? Is unsolicited discovery acceptable on a new network? Affects traffic, privacy and battery. | Manual broad scans; protocol TTL/cache updates and opt-in directed refresh while open; no automatic scan of a newly joined network. |
| Q6 | With pause semantics agreed in Q6a, should navigating away continue a run? What should close/restart do? Defines lifetime and persistence. | Navigation keeps the run; close cancels, retains partial results and asks about unsaved edits; restart never resumes traffic silently. |
| Q7 | For the agreed roughly 250-device target, how many cards/services, which subnets/port ranges and what latency/CPU/RAM limits should be accepted? Probe work depends on more than device count. | Agree a representative small-office workload; bound every run and measure large work estimates before launch. No invented throughput promise. |
| Q17 | Is OS identification limited to passive/selected service evidence, or should additional fingerprint probes be available? Changes traffic and error expectations. | Explainable inference from already permitted evidence; no active OS fingerprinting, vulnerability checks or extra probes merely because AI requests them. |
| Q18 | May optional native discovery require elevation, and is any non-topology authenticated device enrichment essential? Physical managed topology is deferred by Q14. | Ordinary scans need no mandatory admin path. External capture drivers/helper programs are excluded by Q21. Any read-only enrichment needs named equipment and explicit credentials. |

### Data ownership, topology and storage

| ID | Question and impact | Recommended default |
| --- | --- | --- |
| Q9 | What should editable IP/MAC/gateway/port values mean once observed values arrive: notes/intended configuration, manual inventory, or equipment changes? Determines conflicts and UI wording. | Local documentation/manual inventory only; never configure equipment. Preserve original observations separately from assertions. |
| Q15 | Does Forget remove from map, suppress rediscovery, or delete history? Should forgetting a host remove its guests? Should Reset layout also reset manual connectivity? Changes data-loss semantics. | Separate map removal, suppression and permanent deletion; no automatic history purge. Make cascading effects explicit. Keep current reset behaviour pending an explicit decision. |
| Q12 | How long should address history, raw evidence and run logs survive? Must local data/backups be encrypted? Who may access them on shared machines? Sets storage/security requirements. | Per-user local data, no raw packets, bounded diagnostic evidence; choose retention before schema approval. Encryption is not provided merely by SQLite. |
| Q13 | Alongside required Save/Save As, should there be autosave or a separate recovery copy? How should unsaved edits, failed saves and two writers behave? Determines file acknowledgement and recovery. | Explicit saves with a dirty indicator and Save/Discard/Cancel on replacement/close; one writer per file; optional labelled recovery, never silently overwriting the saved document. |
| Q19 | Should one save file hold several named network contexts? Is merging independently edited files needed, or just opening/saving each? Affects identity scope and whether UUIDs are justified. | Each document can hold distinct network contexts; Open replaces the active document after handling unsaved edits. File merging is separate unapproved scope. |
| Q22a | With versioned JSON agreed in Q22b, what extension, views/settings/history and custom-icon packaging should survive? Determines portability and validation. | Snapshot inventory, current evidence, user edits, roles/groups and map arrangement; portable icon identifiers/resources. Keep credentials and model weights outside documents. Exact extension/resource packaging pending. |

### Embedded AI and acceptance

| ID | Question and impact | Recommended default |
| --- | --- | --- |
| Q10a | Should auto-fill, updating existing inferred values, overwriting manual fields and auto-add be separate switches? Per field, per network or global? Determines authority and reversibility. | Separate controls; suggestions available, auto-fill missing inferred fields opt-in, manual replacement and auto-add initially off. Record applied changes and allow undo. |
| Q16 | With roughly 700 MB of weights allowed by Q16c, what baseline CPU, peak RAM, latency and useful fields/categories must inference meet? Determines feasibility beyond download size. | One CPU model first, bundled with its licence/hash; measure actual RAM/latency on baseline PCs. Add a second only for a demonstrated improvement. |
| Q16a | What false-positive rate is acceptable for review suggestions versus auto-add/overwrite? How should confidence be displayed before calibration? Defines evidence needed for autonomous changes. | Allow Unknown/abstention; display an explicitly uncalibrated model score until evaluated. Keep automatic thresholds disabled until validated. |
| Q16b | Should models ever be updated from a download or learn locally from corrections? Is any off-device telemetry permitted? These are different from refreshing guesses using new observations. | Fully offline packaged inference, no telemetry or self-training. New observations may refresh suggestions; weights change only with an explicitly installed app/model update. |
| Q20 | After decisions, may the protected product guide be updated to the agreed backend/AI scope and actual inline-edit UI? Its current frontend-only/no-OS-guessing rules still apply. | Approve a specific scope update before implementation; this documentation task leaves all rules untouched. |

## Research decisions still open

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
