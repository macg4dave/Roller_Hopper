# Backend review

Reviewed 20 October 2026 against source at `f2a9eee` and the working tree. This is a
documentation-only assessment under [the review brief](chatgpt_document_promt.md).
No application code, configuration or agent rules were changed. No application, network scan,
test suite or quality gate was run; behaviour below is established by reading source, not by
runtime or visual acceptance. Existing changes to the prompt and deleted mockup image were
left untouched.

## Reading order and decision status

| Document | Purpose |
| --- | --- |
| This review | Existing functionality, evidence and integration gaps |
| [Decisions and questions](backend-decisions.md) | User answers, proposed defaults and approval blockers |
| [Architecture and data](backend-architecture.md) | Responsibilities, ownership, identity, storage and GUI flow |
| [Discovery strategy](backend-discovery.md) | Methods, network scope, permissions and failure handling |
| [Crate research](backend-crates.md) | Current primary sources, trade-offs and compatibility limits |
| [Conditional roadmap](backend-roadmap.md) | Phases and acceptance boundaries for discussion |
| [Opportunities](backend-opportunities.md) | AI direction and separate future/blue-sky possibilities |

**Confirmed user direction:** support Windows, Linux and macOS from the first real-backend
release; cover local subnets, routed/VLAN targets and broad port scanning; IPv4 first, IPv6
later; retain review and manual addition. Bundle one or two existing small CPU AI models for
offline guesses and field updates, with confidence for review and user control over automatic
addition and overwrites. This is not a request to train a bespoke AI system. Exact policies,
model selection and operational limits remain questions.

**Additional confirmed constraints:** everything runs inside the app; no outside backend/AI
servers, localhost services, external executables or separately installed helper programs.
Dependencies must be in-process libraries shipped with the app. Users must be able to produce
save files and open them again; native Open, Save and Save As are first-release requirements.

Everything described as a recommendation is provisional. The current
[product guide](agents/product.md) still authorises a frontend prototype only. These documents
do not amend that guide or approve implementation. The roadmap becomes implementable only
after the decisions and the necessary product-scope changes are approved.

## What exists

The network map is a substantial interactive prototype. A percentage such as “90% complete”
cannot be established from source alone. The scanner is more than an empty interface: its
control and review workflow exists, but every network result is fabricated.

| Area | Implemented source behaviour | Evidence |
| --- | --- | --- |
| Startup and ownership | Creates sample network and one UI-owned `Rc<RefCell<AppState>>`; connects callbacks and enters Slint | [main.rs](../src/main.rs), [ui_bridge.rs](../src/ui_bridge.rs) |
| Inventory | Devices with multiple cards, optional nested VM/container ownership, status, metadata, notes; search name/IP/MAC and status filters | [device.rs](../src/model/device.rs), [card.rs](../src/model/card.rs), [network.rs](../src/model/network.rs) |
| Map | One node per **card**, not per device; pan, zoom, fit, reset, dragging/reconnection, snapping and motion; known/inferred links | [tree.rs](../src/model/network/tree.rs), [map bridge](../src/ui_bridge/map.rs), [canvas](../ui/topology-canvas.slint) |
| Editing | Inline details editor and separate edit page reuse forms; add/forget devices, cards, VMs and containers; edit names, roles, icons, notes, IPv4 settings, MACs and ports | [edit bridge](../src/ui_bridge/edit.rs), [device form](../ui/device-form.slint), [card editor](../ui/card-editor.slint) |
| Roles and groups | Standard/custom role edits and restoration; role groups with colours and line styles | [roles.rs](../src/model/roles.rs), [groups.rs](../src/model/groups.rs), [edit page](../ui/edit-page.slint) |
| Discovery review | Target/options; Start/Pause/Resume/Stop; progress/time, counts/filters, method/confidence, logs, role choice, Add/Ignore and Show on map | [discovery model](../src/model/discovery.rs), [discovery bridge](../src/ui_bridge/discovery.rs), [discovery page](../ui/discovery-page.slint) |
| Presentation | Fluent, light-only design; retained map pan/zoom under other pages; resizable panels; runtime SVG picker; bundled gettext English | [window](../ui/app-window.slint), [theme](../ui/theme.slint), [icons](../src/ui_bridge/icons.rs), [build.rs](../build.rs), [translations](../translations/README.md) |

The Settings button currently explains where display options live; it is not a backend
configuration page. Role groups are user presentation categories, not VLANs. Card/port edits
and reconnection change local mock records, never equipment configuration.

## Current flow and simulation boundary

`mock_data::network → AppState → Bridge models → Slint`; callbacks return through
`handler → update → view::push`. The [model](../src/model.rs) has no Slint dependency.
The [view synchroniser](../src/ui_bridge/view.rs) already retains `VecModel`s, updates changed
rows and replaces their contents when length changes. Preserve this useful boundary.

On Start, [the bridge](../src/ui_bridge/discovery.rs) refreshes sample statuses and creates
answers from [mapped devices plus fixed newcomers](../src/mock_data/answers.rs). A 150 ms
Slint timer reveals this precomputed list over roughly forty steps. Methods and confidence
are assigned from sample device kinds. Elapsed time is steps multiplied by the timer interval.
The source contains no production socket probing, discovery adapters, durable inventory or AI.

## Gaps that matter before connecting real data

| Confirmed source fact | Consequence for planning |
| --- | --- |
| Metadata is `Option<&'static str>`; one optional IPv4/prefix and gateway per card | Runtime metadata needs ownership; real aliases, IPv6 and unknown remote prefix need explicit modelling |
| IP uniqueness is global; discovery matches MAC+IP, calls changed-IP/same-MAC a possible duplicate, and treats missing MAC as new | Introduce scoped evidence reconciliation before accepting real results; IP is not identity |
| Device/card IDs use current maximum plus one | Removing the highest ID permits reuse; persistence and delayed events need non-reused keys and a bridge mapping |
| Services are keyed only by `u16` port | TCP and UDP, probe outcomes and user descriptions need separate meaning |
| All inventory and edits reset at restart; there is no document/file workflow | Add user-owned native save/open documents; do not confuse a private app cache with a saved file |
| Status/relative last-seen are device-wide; empty values also mean not collected | Need endpoint evidence, absolute observation time, freshness and typed missing/error states |
| One upstream per card; cycles rejected; manual links can also become `Known`/“detected” | Retain the tree as a display projection; distinguish observed, inferred and user-asserted relationships |
| Target list comes from mapped-card prefixes | Empty inventory cannot bootstrap; known map subnets are neither local routing facts nor permission to scan |
| Stop becomes `Finished`; no failed/cancelled/partial terminal state; discovery result selection/actions use list indices | Need truthful run outcomes, stable result selection and rejection of stale events |
| Every update and motion tick reconstructs view vectors | Batch worker results; measure before adding indexes, caches or a new graph/layout engine |

Further static findings, recorded without source repairs:

- Starting any target refreshes **all** mock devices before results arrive. This is simulation,
  not a suitable production status policy.
- A newcomer added to the map can appear twice on the next simulated run: mapped answers and
  the fixed newcomer list overlap, while `Discovery::start` sorts without deduplication. This
  is a source-derived finding, not a reproduced GUI defect.
- `Network::new` trusts its inputs. Arbitrary persisted/imported observations need validation
  beyond the consistency of today's fixture.
- Reset layout also restores the saved connections; Forget recursively removes guests and
  promises their return on restart. Persistence changes the implications of both controls.
- The product guide's read-only details/separate edit description differs from current inline
  editing. Preserve actual functionality; resolve the documentation direction explicitly.

## Existing quality assets and remaining evidence

The manifest/lock resolve Slint and slint-build 1.18.1 and ipnet 2.12.2. Rust is pinned to
1.94.0, edition 2024. Protected lint/toolchain rules remain constraints on future selections.

Existing test definitions cover model editing/filtering, mock consistency, tree/layout/dragging,
discovery simulation, icon lookup and translation keys. They do not establish live discovery,
cross-platform permissions, persistence, real cancellation or AI accuracy. No pass count is
claimed in this review.

[Development tooling](../tools/README.md) is a separate Rust crate: ordered gates, preview-first
text edits, icon path/identifier audit and a Claude command-policy hook. The icon audit does
not render SVGs; the hook is a workflow guard, not a product security boundary. This review
used ordinary `rg`, Git and PowerShell reads, plus `cargo tool --help`; no custom scripts.

No CI workflow or release installer definition was found in the repository inventory. Delivery
of the runtime icon folder, licence notices, platform packages, keyboard/screen-reader behaviour
and performance still need future acceptance evidence. These are planning gaps, not requests
to expand this documentation task into implementation.
