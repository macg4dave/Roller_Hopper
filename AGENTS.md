# Network Mapper — AGENTS.md

This is an **AI-led project**. This file and the guides it links are the single source of truth
for every coding agent (Claude, Codex, Copilot, Cursor, …) and for humans.

- **MUST / MUST NOT** rules are non-negotiable. Breaking one is a failed task, even if the code works.
- Rules are enforced mechanically where possible (`Cargo.toml` `[lints]`, `clippy.toml`,
  `rustfmt.toml`, `rust-toolchain.toml`). Agents MUST NOT weaken that configuration, or edit the
  agent rules, unless the user explicitly asks.

## Guides — read before working

Each guide is short. Read the ones your task touches **before** editing; when unsure, read them all.

| Guide | Read when |
| --- | --- |
| [docs/agents/workflow.md](docs/agents/workflow.md) | Always — delete → combine → add, and agent conduct |
| [docs/agents/collaboration.md](docs/agents/collaboration.md) | Always — working alongside other agents (Claude, Codex) |
| [docs/agents/product.md](docs/agents/product.md) | Any feature or UI work — product brief, scope limits, definition of done |
| [docs/agents/rust.md](docs/agents/rust.md) | Touching `src/`, `build.rs` or `Cargo.toml` |
| [docs/agents/slint.md](docs/agents/slint.md) | Touching `ui/`, `translations/` or `src/ui_bridge.rs` |

## Project layout

Adjust only through delete → combine.

```text
src/main.rs        wiring only: build state, create window, connect callbacks, run
src/ui.rs          Slint-generated types only (lint-exempt; never add code here)
src/model/         pure data and logic (devices, connections, roles, filtering) — no Slint imports
src/mock_data.rs   the sample network
src/ui_bridge.rs   model ↔ Slint struct conversion and callback handlers
ui/                .slint files: app window, panels, canvas, components/, icons/, theme
translations/      all UI text: one gettext .po file per language (English in en/)
docs/agents/       the agent guides linked above
```

The model layer MUST NOT depend on Slint. That separation is what lets a real backend replace
`mock_data.rs` later — no extra abstraction is needed beyond it.

## Protected files

Change only on explicit user request: `AGENTS.md`, `docs/agents/*.md`, `CLAUDE.md`,
`Cargo.toml` `[lints]`, `clippy.toml`, `rustfmt.toml`, `rust-toolchain.toml`, `src/ui.rs`,
`.claude/settings.json`.

## Quality gate

Run all four, in this order, before reporting any task as done:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build
```

- MUST NOT claim success if any step fails. Report the failure and its output honestly.
- MUST NOT make the gate pass by silencing lints, loosening config, or removing tests.

## Task report

End every task with this report (omit empty lines rather than writing "none" everywhere):

```text
Deleted:   what was removed and why
Combined:  what was merged or extended instead of duplicated
Added:     each new file / type / dependency, with a one-line justification
Gate:      fmt ✓/✗ · clippy ✓/✗ · test ✓/✗ (n passed) · build ✓/✗
Notes:     decisions made, limitations, follow-ups noticed but not done
Next:      only if unfinished — what remains, and where
```
