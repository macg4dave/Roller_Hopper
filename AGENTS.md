# AGENTS.md

This is an **AI-led project**. This file and the guides it links are the single source of truth
for how every coding agent (Claude, Codex, Copilot, Cursor, …) and every human works here. They
describe *how to work*, not where things are: learn the code's layout by reading it, and see
[README.md](README.md) for an overview.

- **MUST / MUST NOT** rules are non-negotiable. Breaking one is a failed task, even if the code works.
- Rules are enforced mechanically where possible (lint, format and toolchain configuration).
  Agents MUST NOT weaken that configuration, or edit the agent rules, unless the user explicitly
  asks.

## Guides — read before working

Each guide is short. Read the ones your task touches **before** editing; when unsure, read them all.

| Guide | Read when |
| --- | --- |
| [docs/agents/workflow.md](docs/agents/workflow.md) | Always — delete → combine → add, and agent conduct |
| [docs/agents/collaboration.md](docs/agents/collaboration.md) | Always — working alongside other agents (Claude, Codex) |
| [docs/agents/product.md](docs/agents/product.md) | Any feature or UI work — product brief, scope limits, definition of done |
| [docs/agents/rust.md](docs/agents/rust.md) | Writing or reviewing Rust code, build scripts or dependencies |
| [docs/agents/slint.md](docs/agents/slint.md) | UI, UI text or translation work, or code that connects the model to the UI |
| [docs/agents/tooling.md](docs/agents/tooling.md) | Before any shell command other than `cargo`/`git`/`rustfmt`, wanting a script, or working on agent tooling |

## Architecture

- Keep the network model free of UI code: it MUST NOT depend on Slint. That separation is what
  lets a real backend replace the mock data later — no extra abstraction is needed beyond it.
- Rust owns the data and the logic; the UI presents it. One thin bridge converts between them.
- Change the layout only through delete → combine ([workflow.md](docs/agents/workflow.md)), and
  keep [README.md](README.md) in step when you do.

## Protected files

Change only on explicit user request:

- the agent rules: this file, the guides it links, and tool-specific rule files (such as
  `CLAUDE.md`);
- lint, format and toolchain configuration, including the `[lints]` table of every Cargo
  manifest;
- the module that only wraps Slint's generated code;
- agent tool settings (such as `.claude/settings.json`).

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
