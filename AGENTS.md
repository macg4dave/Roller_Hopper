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
| [docs/agents/workflow.md](docs/agents/workflow.md) | Always — delete → combine → add, planning from the roadmap and backlog, agent conduct |
| [docs/agents/collaboration.md](docs/agents/collaboration.md) | Always — working alongside other agents (Claude, Codex) |
| [docs/agents/product.md](docs/agents/product.md) | Any feature or UI work — product brief, scope limits, definition of done |
| [docs/agents/rust.md](docs/agents/rust.md) | Writing or reviewing Rust code, build scripts or dependencies |
| [docs/agents/slint.md](docs/agents/slint.md) | UI, UI text or translation work, or code that connects the model to the UI |

## Planning

The [roadmap](docs/roadmap.md) says what to do next; the [backlog](docs/.backlog.md) holds
blockers and bigger needs found along the way. Both are living documents that every task keeps
current ([workflow.md](docs/agents/workflow.md#planning-roadmap-and-backlog)).

## Protected files

Change only on explicit user request:

- the agent rules: this file, the guides it links, and tool-specific rule files (such as
  `CLAUDE.md`);
- lint, format and toolchain configuration, including the `[lints]` table of every Cargo
  manifest;
- the module that only wraps Slint's generated code;
- agent tool settings (such as `.claude/settings.json`).

## Protected CUDA cache

`cuda-cache/` is expensive, shared build state. Agents MUST preserve and reuse it.

- Ordinary `cargo build`, `cargo run`, tests and the quality gate MUST keep the default CUDA feature
  enabled. `.cargo/config.toml` directs every one of those commands to `cuda-cache/`; use that cache
  normally without asking for permission.
- MUST obtain the user's explicit permission before populating a missing cache, running
  `cargo tool cuda rebuild`, or building after CUDA, cuDNN, MSVC, compute-capability, Mistral,
  feature or kernel-flag changes would invalidate its objects.
- MUST NOT delete, clean, move, truncate or bypass `cuda-cache/`, or run `cargo clean` against CUDA
  crates, without the user's explicit permission for that exact invalidation.
- `cargo tool cuda build` is the maintained explicit build command; it uses the same default CUDA
  feature and cache as plain Cargo. Use `cargo tool cuda rebuild` only when the user explicitly
  authorises discarding and recreating the cache.
- If `nvcc`, `cicc` or `ptxas` starts unexpectedly, stop the build, preserve completed cache files
  and report what caused the miss. Do not silently wait for or retry the recompilation.
- CPU-only checks are only for a machine that cannot build CUDA and do not qualify the normal app.

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
- The gate uses the default CUDA feature and protected cache. Stop and report an unexpected cache
  miss instead of falling back to a CPU-only gate.

## Task report

End every task with this report (omit empty lines rather than writing "none" everywhere):

```text
Deleted:   what was removed and why
Combined:  what was merged or extended instead of duplicated
Added:     each new file / type / dependency, with a one-line justification
Gate:      fmt ✓/✗ · clippy ✓/✗ · test ✓/✗ (n passed) · build ✓/✗
Plan:      roadmap/backlog changes: tasks removed or added, blockers found or resolved
Notes:     decisions made, limitations, follow-ups noticed but not done
Next:      only if unfinished — what remains, and where
```
