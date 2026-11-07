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
show what works. All stages below are proposed, not a record of completed implementation.

Start with look and feel. Sample data and manually arranged examples are enough to test an
interaction before connecting it to live discovery or other background systems. A clunky but
usable first version is acceptable. Keep the concepts consistent so richer graphics and motion
can be added later without changing how users understand the map.

## 1. Give devices room to move

Make arranging the map feel closer to moving desktop icons while retaining the useful tree
and branch structure.

- Let users position devices freely without constant automatic snapping or rearrangement.
- Make snapping and automatic layout deliberate choices.
- Keep connections attached and understandable as devices move.
- Preserve chosen positions when returning to the map or reopening a project.

**First milestone:** arrange a small example network by hand and keep that arrangement while
exploring it.

## 2. Introduce bubble groups

Give related machines a visible shared space so the overview communicates structure at a glance.

- Create, name, move, and resize groups around related devices.
- Move a device into or out of a group through an understandable interaction.
- Keep connections between groups visible and readable.
- Retain individual device icons and branches inside each group.

**First milestone:** arrange two groups of machines and follow a connection between them.

## 3. Explore inside a machine

Treat a machine as somewhere to explore, with its own contents and relationships.

- Reveal its VMs, network interfaces, internal connections, and notes.
- Make physical containment, group membership, and network connections distinguishable.
- Keep the containing machine and its wider network context recognisable.
- Provide an obvious way to return to the previous level.

**First milestone:** enter an example host, inspect its VMs and interfaces, and return to the
overview without losing orientation.

## 4. Follow connections across levels

Show how internal resources connect to the surrounding infrastructure.

- Give group and machine boundaries visible connection points for interfaces or network links.
- Show where an internal connection meets an external connection.
- Let users follow a VM through a host interface to a switch port or VLAN.
- Reveal detail as needed so the overview stays understandable.

**First milestone:** trace VM1 through its host's Ethernet 2 interface to the external network,
with the same relationship understandable from both inside and outside the host.

## 5. Make navigation continuous

Bring the views together into a fluid journey through the infrastructure.

- Move from overview to group to machine contents through progressive zoom or scrolling.
- Introduce and remove detail gradually as the user moves between levels.
- Keep enough surrounding context to show where the user is.
- Make entering, leaving, and returning to a place predictable with clear navigation controls.

**First milestone:** travel from the network overview into a VM's host and back through one
continuous map experience, without opening another window.

## 6. Turn the map into useful understanding

Use the relationships and notes to help explain the infrastructure.

- Make notes easy to find alongside the devices and relationships they describe.
- Help users answer questions such as what connects here and what depends on this machine.
- Highlight incomplete information and relationships that need investigation.
- Distinguish observed facts, user-entered information, and suggested interpretations.

**First milestone:** select a machine and understand its contents, connections, dependencies,
and relevant notes from the map. Begin with recorded relationships; richer analysis can follow.

## 7. Refine the visual experience

Polish the interactions proven by earlier stages.

- Refine bubble boundaries, connection routing, icons, and visual hierarchy.
- Smooth transitions while keeping navigation responsive and text readable.
- Keep busy maps understandable as the number of devices and relationships grows.
- Use effects to clarify structure and movement.

**First milestone:** explore a representative multi-group map comfortably, with clear
connections and smooth transitions between levels.

## Review points

After each milestone, record what feels useful, what feels confusing, and what should change
before moving on. In particular, revisit:

- Whether free placement and tree structure work comfortably together.
- Whether bubble groups and machine interiors are easy to tell apart.
- Whether connections remain traceable across boundaries.
- Whether deeper navigation preserves orientation.
- Which interactions need real data next, and which still benefit from a visual prototype.

Keep implementation details and individual coding tasks outside this roadmap. Update the
milestones and their order as the product direction becomes clearer.
