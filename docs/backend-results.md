# Backend implementation overview

This document defines the agreed backend, its behaviour and the order of delivery.

## Application structure

The backend runs inside the existing desktop application. Rust handles the network data,
scanning, saving and AI processing; Slint presents the map, editors and discovery results.
Scanning and AI run in the background while the interface remains responsive.

| Area | Direction |
| --- | --- |
| Platforms | Windows, Linux and macOS from the first backend release. |
| Intended scale | Home and small-office networks of roughly 250 devices. |
| Scanner | Upstream netscli-core, maintained through normal library updates. |
| Project storage | Versioned JSON files with the extension `.rhopper.json`. |
| AI | One bundled pretrained model running offline on the CPU. |
| Installation | One packaged application, including its libraries, icons and AI assets. No separate scanner, driver, helper application, service or server installation. |

The application maintains one working project containing the inventory, network contexts,
observations, manual edits, map arrangement and settings. The map, device list and discovery
page all use that same information.

## Scanning

All scans and refreshes are started manually. The user selects the network, targets, ports
and identification depth before starting. Opening a project, changing pages or changing
networks does not start a scan.

The initial backend covers IPv4 on selected local subnets and reachable routed/VLAN networks.
It scans selected TCP ports and ranges and selected named UDP protocols. Identification uses
the methods supported by netscli-core, up to the depth selected by the user. Available methods
and scanning limits are shown before starting; unsupported methods are identified clearly.

| Control | Behaviour |
| --- | --- |
| Start | Begins collection within the selected scope and settings. |
| Pause | Starts no new probes; work already in progress may finish and display results. |
| Resume | Continues the remaining work. |
| Stop | Ends collection and retains valid partial results. |
| Change page | Leaves the current scan running. |
| Close application | Cancels collection and handles unsaved changes. |

Progress shows the work completed, elapsed time and findings. Completed, cancelled, failed
and partial results are distinguished. A failed method or missing response does not erase
existing information or prove that a device is offline. Between scans, status is shown as
last observed, with its observation time. Collection stops when the run ends.

## Results, identity and manual control

Results enter the discovery page for review and addition to the map. Users can edit a new
finding before adding it, leave it unadded, or open an existing mapped device. Repeated scans
update the existing inventory rather than create duplicate entries.

Each device is a parent containing its network interfaces as child cards. Each interface
retains its own addresses, services and network relationships. Virtual-machine and container
ownership remain separate from network connections.

Devices and interfaces retain their identity when an address changes. Matching combines
network context and available identifying evidence; an IP address alone does not establish
identity. Identical private addresses in different network contexts remain distinct.
Ambiguous matches stay in review, protecting existing names, notes and relationships.

Detected facts retain their source and observation time. Manual values always take priority.
When a detected value conflicts with a manual value, the affected field shows both values
and the source/time of detection. The user keeps their value or explicitly adopts the
detected value. Scanning and AI never silently replace manual entries.

## Map structure and placement

The map keeps three separate relationships:

- **Device ownership:** which device owns an interface or hosts a guest.
- **Network connectivity:** observed, inferred or manually recorded network connections.
- **Display organisation:** how cards are arranged into displayed branches.

Rearranging a display branch does not change network connectivity. Observed, inferred and
organisational relationships are visually distinguishable.

The map retains automatic tree arrangement, ordering, snapping and animation. Free positioning
uses simple connectors. Moving a parent in free positioning moves its whole displayed branch
while preserving the relative positions of its descendants.

Opening a project or refreshing results preserves established positions. New cards appear
in a clear area without rearranging existing manual placement. Automatic rearrangement occurs
only through an explicit user action. Saving preserves committed placement rather than an
unfinished drag or an intermediate animation frame.

## Saving and opening projects

Open, Save and Save As use native file dialogs. One project is active at a time and can contain
multiple named network contexts. Projects are opened separately; files are not merged.

Each save contains the complete project and restorable application state:

| Saved information | Contents |
| --- | --- |
| Inventory | Devices, child interfaces, guest ownership, addresses, services and network contexts. |
| User information | Names, roles, groups, notes, edits and icon selections. Custom icons are embedded in the file. |
| Relationships and layout | Network connections, display parentage, root/tier/sibling order and committed card positions. |
| View | Zoom, view position, placement settings, panels, selection, filters and display options. |
| Scanning | Scan settings, results, observation times, retained history and logs. |
| AI | Settings, suggestions, accepted information and the identity of the bundled model used. |

History and logs are retained without automatic deletion. Credentials are never saved,
including in project files, logs or a separate credential store. Any operation requiring
credentials obtains them for that use only.

Saving protects the previous file if the save fails. Save As changes the active filename only
after success. Only one writer edits a file at a time, and conflicting external changes are
reported before replacement. Invalid or unreadable files leave the current project intact.

Unsaved changes prompt Save, Discard or Cancel before closing or replacing the project.
There is no automatic saving or crash recovery. Opening a saved project restores its data
and view without restarting scans or treating old observations as fresh results.

## Embedded AI

One small pretrained model ships with the application and runs locally on the CPU. The model
weight allowance is approximately 700 MB. It requires no GPU, download, external runner or
internet connection. There is no telemetry or self-training.

AI assists mainly with first-time device identification. It uses collected names, services
and other observations to suggest device type, operating-system family and descriptive
information. Each suggestion shows its reasoning, supporting observations and confidence.
Uncertain information remains unknown; suggestions remain distinguishable from detected facts.

Reviewable suggestions are the starting behaviour. Filling missing inferred fields and adding
new devices automatically are separate user controls, both off by default. Automatic addition
follows the selected policy and leaves ambiguous or possible duplicate devices for manual
review. Manual values remain protected under every setting. Manual review, editing and
addition remain available without AI.

## Delivery order

1. **Project foundation:** establish the complete inventory, manual ownership, map arrangement
   and Open/Save/Save As behaviour.
2. **Real scanning:** connect netscli-core to the existing discovery workflow, including scope,
   progress, pause, resume, stop and partial results.
3. **Reliable updates:** combine observations, preserve identity across scans, display conflicts
   and retain history without losing manual edits or map placement.
4. **Embedded AI:** add offline identification suggestions and the separate automation controls.
5. **Complete application:** deliver the agreed behaviour in packaged Windows, Linux and macOS
   applications, with complete project restoration and a responsive map and discovery workflow.
