# Project Review and Future Planning

You are working on a **documentation-only project. Do not write, modify, refactor, or test any code.**

Your task is to review the entire project, investigate the best approaches for its future development, and create planning documents in the `/docs` folder.

## Current Project Status

- **GUI:** Already implemented, using Rust and Slint.
- **Backend:** Currently mocked. No production backend exists.
- **Network Map:** Approximately 90% complete, with most functionality already represented in the GUI.
- **Network Scanner:** New interface with little or no underlying implementation.
- **Technology:** Rust and Slint. Prefer established Rust crates, libraries, and APIs over custom implementations.

The goal is to develop a clear, well-researched technical plan before connecting the GUI to a real backend.

## Selected Scanner and Cross-Platform Direction

- Use [netscli-core](https://github.com/fstubner/netscli/tree/main/crates/netscli-core) as the selected scanner library through normal upstream Cargo releases. This replaces the earlier nrev direction; do not maintain a fork, copy its engine into this project, or build a replacement scanner.
- Support **Windows, Linux and macOS** from the outset. Keep application logic, data models and scanner integration shared across operating systems wherever practical. Prefer established cross-platform APIs whose upstream maintainers handle platform differences.
- Retain the library's useful scanning options even when the initial GUI exposes only a subset. Distinguish supported capabilities from proposals and unverified platform behaviour.
- Users should only need to run the app, without separately installing scanner software, drivers, services or servers. Identify any dependency or packaging conflict during planning rather than assuming it is acceptable.
- If a genuine requirement cannot be met through the shared approach, **inform me and obtain my explicit approval before implementing a platform-specific exception**. Explain the need, why portable alternatives are insufficient, the affected operating systems, and the maintenance, permission and packaging implications. This includes any necessary OS-command invocation or compatibility workaround.
- Research and document such exceptions within this documentation-only task. Do not treat the choice of library as approval for code changes, toolchain changes or unresolved platform requirements.

## Your Responsibilities

### Confirmed Product Decisions

- Preserve the existing parent-and-children model: a device is the parent, and its multiple network interfaces are stored as child cards with their own addresses and services. Scan results must fit this structure rather than create a separate device for every interface. Interface ownership and physical network links are distinct relationships.
- **Manual scans and refresh only.** Do not plan scheduled scans, automatic refresh or background network probing.
- Identification may go **as deep as the user selects and the adopted tools support**. Expose the scope and limitations honestly; raise any necessary platform-specific exception for approval under the cross-platform direction above.
- **There is no Forget or Reset feature in the intended product.** Do not invent forget, reset, rediscovery-suppression or history-purge workflows.
- **Save everything:** preserve all application/project data and restorable state, including the complete device/interface hierarchy, relationships, edits, scan settings/results, retained history/logs, AI settings/suggestions and view/layout settings. Do not silently choose a reduced save subset. Raise any special handling needed for credentials or external assets explicitly during planning.
- **Save the map's structure and placement:** preserve stable icon/card identities, display parentage, root/sibling/tier ordering, committed map-space coordinates and restorable view settings. Custom display trees are separate from network connections; rearranging them must not assert connectivity. In future free positioning, moving a parent moves its whole displayed branch. Reuse the existing placement behaviour where suitable. See [map persistence and UI assessment](backend-map-persistence.md); keeping Slint is the current research recommendation, not permission to change code.

### 1. Review the Entire Project

Understand the existing architecture, GUI functionality, mock data, application flow, and intended user experience.

Identify what already works, what is simulated, what is missing, and which architectural decisions could cause problems later.

**Preserve the existing functionality and direction.** Do not propose unnecessary rewrites.

### 2. Research the Future Architecture

Investigate the best approaches for:

- Network discovery, scanning, identification, and device classification.
- Data collection, processing, validation, storage, and retrieval.
- Communication between the backend and Slint GUI.
- Background operations, concurrency, cancellation, and progress reporting.
- Keeping device information updated without unnecessary rescanning.
- Identifying devices reliably when IP addresses change.
- Combining information from multiple discovery methods.
- Persistent storage, configuration, and user-defined device information.
- Performance, memory efficiency, security, and scalability.
- Cross-platform considerations and operating-system limitations.
- Handling failed scans, missing information, and unreliable results.

Consider how these systems should work together rather than treating them as isolated features.

### 3. Research Existing Rust Solutions

Research suitable Rust crates, libraries, and external APIs.

For each significant recommendation, explain:

- What it does and why we need it.
- Why it is preferable to the alternatives.
- Its maturity, maintenance, compatibility, and limitations.
- Whether it integrates cleanly into our existing architecture.
- Whether it avoids unnecessary custom code.

Prefer proven solutions. **Do not reinvent functionality that a reliable library already provides.**

### 4. Identify Gaps and Opportunities

Think beyond the current implementation.

Identify missing capabilities, potential improvements, technical risks, and opportunities we may not have considered.

Include a separate **blue-sky section** exploring ambitious but realistic possibilities, such as intelligent device identification, automatic network documentation, historical changes, topology discovery, and optional embedded AI.

Clearly distinguish essential functionality from future enhancements and experimental ideas.

### 5. Ask Questions

This is an important part of the assignment.

**Ask plenty of thoughtful questions before committing to major architectural decisions.**

Focus on questions that genuinely affect the design, including user expectations, scanning behaviour, data ownership, accuracy, performance, supported networks, device identification, and future features.

Challenge assumptions where appropriate. Do not simply accept the current mock implementation as the correct architecture.

Group questions by subject, explain their impact, and provide a recommended default where possible.

Do not silently make important product decisions.

### 6. Create Planning Documents

Write your findings into separate, focused Markdown files in `/docs`.

Choose sensible filenames and organise the information so that another AI coding agent can easily understand and implement the agreed architecture.

Include:

- Current project assessment.
- Proposed architecture and component responsibilities.
- Data model and data flow.
- Network discovery and scanning strategy.
- Recommended Rust crates and supporting research.
- Gaps, risks, and unresolved technical decisions.
- Blue-sky ideas and future opportunities.
- Prioritised questions for me.
- A proposed phased implementation roadmap.

Avoid duplicated information, excessive documentation, and speculative complexity.

## Important Rules

1. **No coding.** Only create or update documentation inside `/docs`.
2. Do not change existing application behaviour or project configuration.
3. Review the actual repository before making recommendations.
4. Research current solutions rather than relying entirely on assumptions.
5. Challenge unnecessary complexity and favour simple, modular architecture.
6. Do not assume the mocked backend represents the ideal production design.
7. Clearly separate confirmed findings, recommendations, assumptions, and unanswered questions.
8. Do not invent requirements or treat blue-sky ideas as approved features.
9. Do not attempt implementation or create implementation tasks until the architectural questions have been considered.
10. Keep documentation practical, concise, and useful to future coding agents.

## Expected Outcome

By the end of this review, I want to understand:

- What we have already built.
- What the real backend should look like.
- Which technologies and Rust crates we should use.
- How information should move through the application.
- Where the important gaps and technical risks are.
- What opportunities exist to make the application significantly better.
- Which decisions I need to make before implementation begins.
- How we can introduce the backend incrementally without damaging the existing GUI.

**This is a research, architecture, and planning exercise, not an implementation exercise.**

Start by reviewing the repository and preparing your initial findings and questions. We will use those questions to refine the architecture before approving a development roadmap.
