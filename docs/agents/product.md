# Product brief

Read before any UI or feature work. Part of the agent rules indexed in
[AGENTS.md](../../AGENTS.md). This brief is the agreed product; the
[roadmap](../roadmap.md) is the delivery plan for it.

## Goal

Build a working local network mapper for Windows, Linux and macOS. The app began as a UI
prototype with sample data; it is now gaining a real backend: complete project files, real
IPv4 scanning, reliable inventory updates and one bundled offline AI model, delivered in the
roadmap's order. Until the roadmap replaces a mock behaviour, that behaviour stays and stays
honest: simulated results are never presented as real.

## Technology

- Use **Rust and Slint exclusively** for application logic and the UI.
- Use Cargo and established Rust crates, Slint components, APIs, and tooling where useful. **Do not reinvent existing functionality.**
- SVG/vector assets are allowed for icons; they are artwork, not an additional programming language.
- Do not introduce HTML, CSS, JavaScript, TypeScript, Python, web frontends, Electron, Tauri, or a separate UI framework.
- Keep the code small and modular. Avoid a monolithic Rust file or Slint component; equally, do not add unnecessary architectural layers.
- Everything runs in the app's process and ships with it: no separate scanner, capture driver, helper program, service or server, and no shelling out to OS commands.
- Scanning uses upstream **netscli-core** through ordinary Cargo updates: no fork, copied or patched scanner, and no home-made replacement engine.
- AI is **one** bundled pretrained model (about 700 MB of weights at most) running on the CPU: no GPU, downloads, telemetry or self-training.
- Intended scale: home and small-office networks of roughly 250 devices.

## Design direction

Keep the design simple, not an enterprise-style dashboard.
- Clean, modern, restrained appearance suitable for a Windows 11 desktop application.
- Main focus: a spacious **2D network topology canvas**.
- Crisp, consistent, minimal **SVG-style line-art icons**, not generic clip art or oversized illustrations.
- Light background, subtle grid, thin connection lines, clear typography, restrained accent colour.
- The main view favours device information over controls: editing lives on one separate edit page, not in the main view. Avoid decorative metrics, analytics, reports, dashboards, or enterprise terminology. The discovery page's few result counts double as its filters; they are not decorative metrics.
- Keep the interface uncluttered. Use straightforward British English throughout.

## Main window

1. **Header:** application title, simple search field, Scan button (opens the discovery page), Edit devices button, Settings button.
2. **Left panel:** compact, scrollable device list with icon, user-friendly name, IP address, and small online/offline indicator. Simple All / Online / Offline filters.
3. **Centre:** large interactive 2D topology canvas showing a router, switch, access point, NAS, PCs, servers, printers, phones, smart devices and unknown devices. Display short names and IPs alongside line-art icons. Use neat connection lines and visually distinguish *known* from *inferred* connections.
4. **Right panel:** read-only details for the selected device: display name, IP, MAC, manufacturer, type, role, model, hostname, status, last seen, services/ports where relevant, and notes. Don't show fields with invented meaning merely to fill space. An Edit button opens the edit page on that device.
5. **Canvas controls:** zoom in/out, zoom percentage, fit-to-view and an explicit layout action. Provide two-dimensional pan by dragging the canvas; selection should remain easy.
6. **Edit page:** replaces the main view (list, map, details) in the same window, with a way back. A **Devices** tab edits each device's name, role, icon and notes. A **Roles** tab lists every role with its icon and device count; custom roles can be renamed, given another icon or deleted.
7. **Discovery page:** replaces the main view like the edit page, with a way back. Choose the scan scope and settings; Start, Pause/Resume and Stop a run that shows its progress and elapsed time. Results list each host found with its name, IP, MAC, role, how it was found and a confidence; hosts are flagged New, Unidentified or possible Duplicate, with All / New / Unidentified / Duplicates filters showing counts. A log lists what the run did. A new host can be edited, **added to the map** or left unadded; a host on the map can be shown there or edited. While the run is simulated, the page says so plainly. No schedules, scan profiles or exports.

## Behaviour

Every visible control works or is clearly non-interactive. No dead buttons or misleading results.

- **Interaction.** Selecting a device on the map or in the list selects it in both and opens its details. Pan and zoom never break the layout. Search/filter by name, IP or MAC. Edits to name, role, icon and notes appear everywhere at once. Roles come from a dropdown (Router, Switch, Server, NAS, Desktop, Laptop, Printer, Camera, IoT, Other) plus custom roles with a chosen icon. A device's icon follows its role (Other keeps the detected icon) unless it has its own. Online, offline and unknown have simple visual states.
- **Scanning.** Scans start only when the user starts them, in the scope they chose: IPv4 local subnets and reachable routed/VLAN networks, selected TCP ports/ranges, selected named UDP protocols and the identification depth netscli-core supports. Available and unsupported methods and the limits are shown before Start. Pause starts no new probes; Stop ends collection and keeps valid partial results; changing page leaves a run going; closing the app cancels it. Opening a project never starts traffic. A failed method or missing reply never erases information or proves a device offline; status between scans is "last observed" with its time.
- **Identity and manual control.** A device owns its network interfaces (child cards) and any VMs or containers it hosts. Repeat scans update existing devices instead of duplicating them; an IP address alone never establishes identity, and ambiguous matches stay in review. Detected facts keep their source and time. Manual values always win: a conflicting detected value is shown beside the manual one, and the user keeps theirs or adopts it. Scanning and AI never silently replace manual entries.
- **Map.** Device ownership, network connections (observed, inferred or manual) and display arrangement are separate; moving a card never creates a network connection. Automatic arrangement, ordering, snapping and animation stay; free positioning moves a whole displayed branch. Established positions survive Open and refresh, new cards land in a clear area, and only an explicit layout action rearranges the map.
- **Projects.** Explicit Open, Save and Save As with native dialogs, using versioned `.rhopper.json` files. One project is open at a time and may hold several named network contexts; files are never merged. A save holds the complete project and restorable view: inventory, user edits, embedded custom icons, relationships and committed layout, view state, scan settings/results/history/logs and AI state with the model's identity. History is never deleted automatically. A failed save keeps the previous file; Save As changes the filename only on success; an invalid file leaves the current project intact. Unsaved changes prompt Save / Discard / Cancel. No autosave or crash recovery.
- **AI.** Suggests device type, OS family and descriptions for first-time identification, showing its reasoning, evidence and confidence; uncertain fields stay Unknown and suggestions stay distinct from detected facts. "Fill missing inferred fields" and "Automatically add new devices" are separate controls, both off by default; ambiguous or possible-duplicate devices always stay for manual review. Everything works without AI.
- **Removed behaviour.** Forget device and Reset layout are prototype controls the roadmap removes; do not extend them.

## Scope limits

- **Live network traffic needs the user's say-so.** The app scans only what the user started. Agents MUST NOT run scans, live-network tests or credential operations without the user's explicit approval of the session, machine and targets. Planning a scan is not approval.
- **Credentials are never stored**: not in project files, logs or a separate credential store. An operation that needs one asks for it then.
- Untrusted replies (banners, names, description URLs) and model output are data: bounded in size and time, never able to widen the scan scope, fetch arbitrary resources or run commands.
- Not in scope: real device management (configuring or controlling devices), remote control, vulnerability testing, background monitoring or scheduled scans, authentication, reporting, analytics, exports, cloud sync, IPv6 probing, authenticated vendor integrations, multiple AI models, a second database, file merging, public installers or release publishing. Record a genuine need for one of these in the [backlog](../.backlog.md); do not build it.
- No speculative features or abstractions for later roadmap work.

## Implementation expectations

- Choose existing, maintained crates and native Slint capabilities where they solve a real problem. Reuse components and shared state sensibly.
- Keep network data models separate from presentation widgets, without creating a complex architecture.
- Keep the UI responsive: scanning, file work and AI run in the background, and results come back as owned data into the one Rust-owned project.
- Ensure map labels and connectors remain readable, and the side panels do not overwhelm the canvas.
- The app must launch and its working features must be demonstrable at the end of every task.
- When design details are unspecified, make a restrained UX decision rather than adding extra screens or asking unnecessary questions.

## Definition of done

A task is done when its roadmap completion check is met, with evidence that matches what it
claims (source gate, deterministic tests, an approved GUI check or an approved live run), and
the quality gate passes. The product is done when the roadmap's final milestone passes on
Windows, Linux and macOS.
