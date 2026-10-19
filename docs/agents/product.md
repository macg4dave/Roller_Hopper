# Product brief

Read before any UI or feature work. Part of the agent rules indexed in
[AGENTS.md](../../AGENTS.md).

## Goal

Build a polished, working **UI prototype** of a local network mapper, based on the reference
mockup at [docs/design/reference-mockup.png](../design/reference-mockup.png). This phase is
**frontend only**: use realistic sample data and simulated actions. Do not implement actual
network discovery, scanning, monitoring, or persistence.

## Technology

- Use **Rust and Slint exclusively** for application logic and the UI.
- Use Cargo and established Rust crates, Slint components, APIs, and tooling where useful. **Do not reinvent existing functionality.**
- SVG/vector assets are allowed for icons; they are artwork, not an additional programming language.
- Do not introduce HTML, CSS, JavaScript, TypeScript, Python, web frontends, Electron, Tauri, or a separate UI framework.
- Keep the code small and modular. Avoid a monolithic Rust file or Slint component; equally, do not add unnecessary architectural layers.

## Design direction

Match the reference mockup (the simpler design), not an enterprise-style dashboard.
- Clean, modern, restrained appearance suitable for a Windows 11 desktop application.
- Main focus: a spacious **2D network topology canvas**.
- Crisp, consistent, minimal **SVG-style line-art icons**, not generic clip art or oversized illustrations.
- Light background, subtle grid, thin connection lines, clear typography, restrained accent colour.
- Less management UI; more useful device information. Avoid decorative metrics, analytics, reports, dashboards, or enterprise terminology.
- Keep the interface uncluttered. Use straightforward British English throughout.

## Main window

1. **Header:** application title, simple search field, Scan button, Settings button.
2. **Left panel:** compact, scrollable device list with icon, user-friendly name, IP address, and small online/offline indicator. Simple All / Online / Offline filters.
3. **Centre:** large interactive 2D topology canvas showing a router, switch, access point, NAS, PCs, servers, printers, phones, smart devices and unknown devices. Display short names and IPs alongside line-art icons. Use neat connection lines and visually distinguish *known* from *inferred* connections.
4. **Right panel:** details for the selected device, prioritising information over controls: display name, IP, MAC, manufacturer, type, role, model, hostname, status, last seen, services/ports where relevant, and notes. Don't show fields with invented meaning merely to fill space.
5. **Canvas controls:** zoom in/out, zoom percentage, fit-to-view and reset layout. Provide two-dimensional pan by dragging the canvas; selection should remain easy.

## Prototype behaviour

All visible controls should behave plausibly **within the mock UI**:
- Select a device on the map or in the list; both reflect the selection and open its details.
- Pan horizontally and vertically and zoom the topology without breaking its layout.
- Search/filter devices by name or IP (and MAC if convenient).
- Edit a device's **display name**, **role**, and **notes** in the UI. Changes immediately appear wherever that data is displayed.
- Choose a role from a dropdown (e.g. Router, Switch, Server, NAS, Desktop, Laptop, Printer, Camera, IoT, Other); allow the user to **add a custom role** to the dropdown.
- Use a sensible set of mock devices with internally consistent names, IPs, MACs and relationships.
- Make Scan visibly simulate a refresh using local dummy data. Never imply a real scan happened.
- Use simple visual states for online, offline, and unknown devices.
- If an interaction is visible, wire it up to mock state or clearly make it non-interactive. No dead buttons or misleading results.

## Scope limits

- **No backend.** No real network access, socket probing, SNMP, mDNS, ARP collection, port scanning, elevated permissions, services, database, or external API calls.
- No authentication, reporting, analytics, device management, configuration deployment, or remote-control features.
- No speculative features beyond what is needed for a convincing, usable mapper prototype.
- Keep data in memory; restarting may reset it. Don't build storage abstractions yet.
- Do not over-engineer a future backend. Build a clear UI that can be connected to one later.

## Implementation expectations

- Choose existing, maintained crates and native Slint capabilities where they solve a real problem. Reuse components and shared state sensibly.
- Keep network data models separate from presentation widgets, without creating a complex architecture.
- Ensure map labels and connectors remain readable, and the side panels do not overwhelm the canvas.
- The prototype should launch and allow the key interactions above to be demonstrated.
- Run the appropriate Cargo build/check, resolve errors, and briefly report what works and any limitations.
- When design details are unspecified, make a restrained UX decision rather than adding extra screens or asking unnecessary questions.

## Definition of done

A runnable Rust/Slint desktop mockup resembling the reference mockup, with an interactive pannable/zoomable device map, device list, selection and details, editable labels and roles, search/filters, and simulated scan. **No real network functionality.**
