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

## Your Responsibilities

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
