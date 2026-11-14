# Roller Hopper roadmap

## Product direction

Roller Hopper is an infrastructure mapper. Network discovery is one way to populate the map;
its purpose is to help people explore infrastructure, understand relationships, and capture
knowledge about what they find.

The experience starts with a top-down network view: connected devices arranged into groups
that feel like bubbles. Moving deeper reveals a machine's VMs, network interfaces, internal
connections, and notes while keeping its relationship to the wider network visible. Exploration
should feel like moving through one continuous map, with a clear way back to the overview.

## How to use this roadmap

Work from the easier interaction changes toward the more demanding navigation and insight
features. The order is a starting point, not a fixed delivery schedule; revise it as prototypes
show what works. Stage 1 has an implementation status below; stages 2–8 remain proposed.

Start with look and feel. Sample data and manually arranged examples are enough to test an
interaction before connecting it to live discovery or other background systems. A clunky but
usable first version is acceptable. Keep the concepts consistent so richer graphics and motion
can be added later without changing how users understand the map.

The [GitHub reuse review](github-reuse-review.md) supplies the candidate IDs below.
The [licensing decision](licensing.md) selects GPL-3.0-or-later so compatible GPL code
can also be considered. These assignments select work to investigate, not libraries approved
for shipping. Retain Slint and extend the existing model, canvas and project format.

## 1. Give devices room to move

**Implemented, 14 November 2026:** free placement is the default; connections remain attached,
other cards stay put, and positions survive page changes and project save/reopen. Reconnection
snapping is an explicit mode. Automatic arrangement is a separate action. Both movement and
arrangement support guarded one-step undo. Saves use format version 4; versions 1–3 still open.
The interaction is implemented in Slint without a new library or copied upstream source.
Synthetic regression tests cover movement without rewiring, cancellation, mode changes,
arrangement undo, older-format loading and position persistence. Running-UI checks confirmed
that only the dragged card moves, connections follow it, Arrange/Undo restores placement,
and switching edit mode preserves it. This completes the first milestone; bubble groups remain next.

Make arranging the map feel closer to moving desktop icons while retaining the useful tree
and branch structure.

- Let users position devices freely without constant automatic snapping or rearrangement.
- Make snapping and automatic layout deliberate choices.
- Keep connections attached and understandable as devices move.
- Preserve chosen positions when returning to the map or reopening a project.

**First milestone:** arrange a small example network by hand and keep that arrangement while
exploring it.

**Reuse focus (N1, N3, N4):** study Snarl's separation of node data and presentation, Iced's
coordinate transforms, and Graphite's edit commands. Start by checking the existing drag,
snap and saved-position behaviour; add only missing behaviour. A move and an explicit arrange
operation should be undoable, and reopening must preserve the result. Do not embed another UI toolkit.

## 2. Introduce bubble groups

Give related machines a visible shared space so the overview communicates structure at a glance.

- Create, name, move, and resize groups around related devices.
- Move a device into or out of a group through an understandable interaction.
- Keep connections between groups visible and readable.
- Retain individual device icons and branches inside each group.

**First milestone:** arrange two groups of machines and follow a connection between them.

**Reuse focus (N1, N2, N4):** adapt selection, grouping and collapsed-node interactions.
Keep group membership distinct from machine containment; moving a group must preserve member
positions relative to it and keep external links attached. Reuse existing group and arrangement data.

## 3. Explore inside a machine

Treat a machine as somewhere to explore, with its own contents and relationships.

- Reveal its VMs, network interfaces, internal connections, and notes.
- Make physical containment, group membership, and network connections distinguishable.
- Keep the containing machine and its wider network context recognisable.
- Provide an obvious way to return to the previous level.

**First milestone:** enter an example host, inspect its VMs and interfaces, and return to the
overview without losing orientation.

**Reuse focus (N4, M1, M2):** use Graphite's nested navigation and NetBox/Nautobot's inventory
boundaries to distinguish a canonical machine from its appearances in different views. Start with
a manually described host and guest; preserve entity IDs and each view's position and zoom on return.

## 4. Follow connections across levels

Show how internal resources connect to the surrounding infrastructure.

- Give group and machine boundaries visible connection points for interfaces or network links.
- Show where an internal connection meets an external connection.
- Let users follow a VM through a host interface to a switch port or VLAN.
- Reveal detail as needed so the overview stays understandable.

**First milestone:** trace VM1 through its host's Ethernet 2 interface to the external network,
with the same relationship understandable from both inside and outside the host.

**Reuse focus (N1, N2, M3):** introduce typed connection points only where the existing interface,
segment and relationship model needs them. Reject incompatible endpoints without changing the map;
keep discovered neighbour evidence separate from user-confirmed links. Drawing a wire never runs an action.

## 5. Make navigation continuous

Bring the views together into a fluid journey through the infrastructure.

- Move from overview to group to machine contents through progressive zoom or scrolling.
- Introduce and remove detail gradually as the user moves between levels.
- Keep enough surrounding context to show where the user is.
- Make entering, leaving, and returning to a place predictable with clear navigation controls.

**First milestone:** travel from the network overview into a VM's host and back through one
continuous map experience, without opening another window.

**Reuse focus (N4):** prototype breadcrumbs and navigation history before animated transitions.
Returning to a host or overview must restore selection and view state without duplicating entities.

## 6. Turn the map into useful understanding

Use the relationships and notes to help explain the infrastructure.

- Make notes easy to find alongside the devices and relationships they describe.
- Help users answer questions such as what connects here and what depends on this machine.
- Highlight incomplete information and relationships that need investigation.
- Distinguish observed facts, user-entered information, and suggested interpretations.

**First milestone:** select a machine and understand its contents, connections, dependencies,
and relevant notes from the map. Begin with recorded relationships; richer analysis can follow.

**Knowledge milestone (M1, M2, M10):** extend existing notes with entity links, canonical runbook
documents, revisions and separate operational journal entries. Demonstrate editing a linked note,
reopening it and inspecting its previous revision without losing its device link.

**Evidence milestone (M3, M4):** retain observation source, first/last seen and stale or conflicting
evidence. Repeated discovery must not silently overwrite a manually confirmed relationship.
Keep alerts and alert history separate from the device's current reachability status.

**Analysis milestone (L1):** begin with recorded dependency traversal and an explanation of each
highlighted path. Evaluate Petgraph only if a measured cycle-analysis or impact-traversal need
exceeds the existing model. Infrastructure cycles are valid; graph indices never become entity IDs.

## 7. Refine the visual experience

Polish the interactions proven by earlier stages.

- Refine bubble boundaries, connection routing, icons, and visual hierarchy.
- Smooth transitions while keeping navigation responsive and text readable.
- Keep busy maps understandable as the number of devices and relationships grows.
- Use effects to clarify structure and movement.

**First milestone:** explore a representative multi-group map comfortably, with clear
connections and smooth transitions between levels.

**Reuse focus (L2, L3):** compare the two Sugiyama implementations on the same variable-size,
cyclic and disconnected examples. Choose at most one after measuring fit. Explicit arrangement
must respect pinned positions, preserve stored relationship direction and support undo; retain
the existing layout if neither candidate improves it.

## 8. Add deliberate machine operations

Start after machine identity, evidence and linked knowledge are reliable. Remote operations
are a separate capability from map editing and discovery.

- Use Cockpit and cockpit-machines (M5/M6) to shape service and VM inspectors, pending states,
  confirmation and recovery paths. Begin with read-only details or an explicit management link.
- Use MeshCentral (M7) to investigate access profiles, target rights and disconnected sessions;
  choose native support, an external link or a reviewed API integration per capability.
- Use Rundeck and Komodo (M8/M9) to separate a saved action/runbook, selected targets and each
  execution record. Include prerequisites, authorisation, progress, cancellation and recorded errors.
- Keep credentials outside project exports and tracked fixtures. Executable control flow needs
  validation separate from the infrastructure graph, including a deliberate policy for cycles.

**First milestone:** preview one action against a synthetic target, explicitly start a controlled
test operation, and inspect success, failure and interruption records. Reopening a project or
drawing a connection must never execute it. Validate a real operation only within an authorised
private environment after the synthetic checks pass.

**Deferred (F1):** Dora is a runtime comparison if actual streaming/dataflow requirements emerge.
Do not introduce a dataflow engine for map rendering or the first single-target action.

## Implementation starting points

Begin with stage 1's saved movement and undo behaviour, then stage 2 grouping. Existing features
may already satisfy parts of these milestones; audit them before adding code. Later rows depend
on the identity and persistence boundaries established by the earlier work.

| Order | Extend first | First reviewable change and acceptance |
| --- | --- | --- |
| 1–2 | `src/model/arrangement.rs`, `src/model/snap.rs`, `src/model/groups.rs`, `src/ui_bridge/map/` | Fill gaps in manual movement and group placement; move, undo and reopen a synthetic two-group map with stable links. |
| 3–5 | `src/model/device.rs`, `src/model/network/`, existing UI bridge and canvas | One host/guest view with typed boundary connections and a return path; reject an invalid wire and restore the previous view. |
| 6: knowledge | `src/model/notes.rs`, `src/project.rs`, existing note UI | Persist an entity-linked note and revision through a versioned project migration; old projects still open. |
| 6: evidence | `src/model/discovery.rs`, `src/model/network/relations.rs`, existing scan conversion | Preserve conflicting observations and manual decisions through rescan and save/reopen. |
| 6–7: algorithms | `src/model/network/`, `src/model/layout.rs` | Compare traversal/layout against current behaviour before adding a dependency; prove stable IDs, preserved cycles and undoable placement. |
| 8 | Existing device inspector, linked notes and project persistence | Read-only operation context first, then one explicitly initiated operation with durable results and tested cancellation/failure. |

Before each source adoption, complete the attribution and dependency checks in the licensing
decision. Selected source may be ported only after its actual file licence and technical fit
are verified. Add no unused executor, abstraction or dependency in anticipation of later stages.

## Review points

After each milestone, record what feels useful, what feels confusing, and what should change
before moving on. In particular, revisit:

- Whether free placement and tree structure work comfortably together.
- Whether bubble groups and machine interiors are easy to tell apart.
- Whether connections remain traceable across boundaries.
- Whether deeper navigation preserves orientation.
- Which interactions need real data next, and which still benefit from a visual prototype.

Use synthetic committed examples for regression checks and the relevant private projects for
local acceptance once a prototype exists. Record useful real-world evidence only under ignored
`site/` or `sites/`. Run the repository quality gate for every implementation change and check UI
changes visually under the project's workflow. Update the milestones as prototypes reveal what works.
