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
| Q4 | Does broad scanning mean all TCP ports, or UDP too? Must arbitrary routed ranges be supported, and what exclusions are required? Changes probe semantics and work bounds. | Full user-selected TCP ranges; optional named UDP protocol discovery, not a blanket UDP sweep. Explicit target/exclusion lists; no internet-wide expansion. |
| Q5 | Should known devices refresh only on request, periodically while open, or continuously? Is unsolicited discovery acceptable on a new network? Affects traffic, privacy and battery. | Manual broad scans; protocol TTL/cache updates and opt-in directed refresh while open; no automatic scan of a newly joined network. |
| Q6 | May paused runs finish in-flight probes and show their results? Should navigating away continue a run? What should close/restart do? Defines cancellation and persistence. | Pause launches no new work; bounded in-flight work completes. Navigation keeps the run; close cancels and saves partial results; restart never resumes traffic silently. |
| Q7 | What are typical and maximum devices/cards, subnet sizes, ports, latency and CPU/RAM budgets? Scale cannot be inferred from a home-sized mock. | Agree measurable small-office and routed-network workloads; bound every run and test large work estimates before launch. No invented throughput promise. |
| Q17 | Is OS identification limited to passive/selected service evidence, or should additional fingerprint probes be available? Changes traffic and error expectations. | Explainable inference from already permitted evidence; no active OS fingerprinting, vulnerability checks or extra probes merely because AI requests them. |
| Q18 | Which managed switches/routers/cameras need authenticated SNMP/API support in release one? Are read-only credentials available, and may native OS elevation be required? Determines achievable physical topology and parity. | Ordinary scans need no mandatory admin path. External capture drivers/helper programs are excluded by Q21. Optional read-only integrations only for named equipment and explicit credentials. |

### Data ownership, topology and storage

| ID | Question and impact | Recommended default |
| --- | --- | --- |
| Q9 | What should editable IP/MAC/gateway/port values mean once observed values arrive: notes/intended configuration, manual inventory, or equipment changes? Determines conflicts and UI wording. | Local documentation/manual inventory only; never configure equipment. Preserve original observations separately from assertions. |
| Q14 | Is the card-based inferred tree the long-term view, or must the first release show physical ports, redundant links and topology loops? This determines whether a projection is enough. | Retain card nodes and inferred tree; preserve extra evidence without inventing physical links. Physical topology only where a supported source supplies it. |
| Q15 | Does Forget remove from map, suppress rediscovery, or delete history? Should forgetting a host remove its guests? Should Reset layout also reset manual connectivity? Changes data-loss semantics. | Separate map removal, suppression and permanent deletion; no automatic history purge. Make cascading effects explicit. Keep current reset behaviour pending an explicit decision. |
| Q12 | How long should address history, raw evidence and run logs survive? Must local data/backups be encrypted? Who may access them on shared machines? Sets storage/security requirements. | Per-user local data, no raw packets, bounded diagnostic evidence; choose retention before schema approval. Encryption is not provided merely by SQLite. |
| Q13 | Alongside required Save/Save As, should there be autosave or a separate recovery copy? How should unsaved edits, failed saves and two writers behave? Determines file acknowledgement and recovery. | Explicit saves with a dirty indicator and Save/Discard/Cancel on replacement/close; one writer per file; optional labelled recovery, never silently overwriting the saved document. |
| Q19 | Should one save file hold several named network contexts? Is merging independently edited files needed, or just opening/saving each? Affects identity scope and whether UUIDs are justified. | Each document can hold distinct network contexts; Open replaces the active document after handling unsaved edits. File merging is separate unapproved scope. |
| Q22a | What native file format/extension is preferred, which views/settings/history must survive, and should custom icons travel inside the file or beside it? Determines portability and validation. | Versioned JSON snapshots of inventory, current evidence, user edits, roles/groups and map arrangement; portable icon identifiers/resources. Keep credentials and model weights outside documents. Exact extension/resource packaging pending. |

### Embedded AI and acceptance

| ID | Question and impact | Recommended default |
| --- | --- | --- |
| Q10a | Should auto-fill, updating existing inferred values, overwriting manual fields and auto-add be separate switches? Per field, per network or global? Determines authority and reversibility. | Separate controls; suggestions available, auto-fill missing inferred fields opt-in, manual replacement and auto-add initially off. Record applied changes and allow undo. |
| Q16 | What installed-size/RAM/latency ceiling defines small, and which fields/categories need useful predictions? Determines pretrained-model feasibility and package limits. | One CPU model first, bundled with its licence/hash; measure actual RAM/latency on baseline PCs. Add a second only for a demonstrated improvement. |
| Q16a | What false-positive rate is acceptable for review suggestions versus auto-add/overwrite? How should confidence be displayed before calibration? Defines evidence needed for autonomous changes. | Allow Unknown/abstention; display an explicitly uncalibrated model score until evaluated. Keep automatic thresholds disabled until validated. |
| Q16b | Should models ever be updated from a download or learn locally from corrections? Is any off-device telemetry permitted? These are different from refreshing guesses using new observations. | Fully offline packaged inference, no telemetry or self-training. New observations may refresh suggestions; weights change only with an explicitly installed app/model update. |
| Q20 | After decisions, may the protected product guide be updated to the agreed backend/AI scope and actual inline-edit UI? Its current frontend-only/no-OS-guessing rules still apply. | Approve a specific scope update before implementation; this documentation task leaves all rules untouched. |

## Research decisions still open

- A maintained safe neighbour-table/ICMP path must be qualified on each OS under the existing
  no-unsafe rule; current candidates are not a verified drop-in solution.
- Embedded model and Rust runtime must be selected together. Off-the-shelf does not mean
  calibrated for network-device facts; model packaging and evidence evaluation are necessary.
- Native documents recommend versioned JSON with one in-memory authority. SQLite remains a
  conditional alternative for approved history/scale, with migration compatibility unresolved.
  No protected toolchain upgrade is assumed.
- No live platform, broad-scan performance, storage recovery or model-quality evidence was
  produced by this documentation-only review.

After answers, revise the architecture and conditional roadmap together, then seek approval
of that concrete plan. No issue cards, source stubs or implementation tasks have been created.
