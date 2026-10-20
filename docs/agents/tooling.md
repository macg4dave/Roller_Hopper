# Agent tooling

Read before running any shell command other than `cargo`, `git` or `rustfmt`, and before working
on agent tooling. Part of the agent rules indexed in [AGENTS.md](../../AGENTS.md).

Agents often reach for shell utilities — `jq`, `grep`, `sed`, `awk`, `find`, Python or PowerShell
one-liners. Those differ between machines and shells (this repo is worked on from PowerShell, Git
Bash and Unix shells) and are not reviewed like project code. In this repository, every such
utility is a **Rust subcommand of the tooling crate**, so every agent runs the same, tested tool
everywhere.

## What you may run

- `cargo` (including the quality gate and `cargo run`), `git`, `gh` and `rustfmt` — directly.
- Your agent's built-in file tools (read, search, edit, write) — directly. They are part of the
  agent, not shell utilities.
- **Everything else goes through `cargo tool <area> <command>`.** MUST NOT call `jq`, `grep`,
  `rg`, `sed`, `awk`, `find`, `xargs`, `python`, `node`, PowerShell cmdlet pipelines or similar
  for project work, and MUST NOT write throwaway scripts in another language.
- If no command does what you need, add or extend one (below) as part of your current task. You
  do not need to ask first. MUST NOT fall back to a shell tool "just this once".

## Building tooling you will use

Agents are encouraged to build tools that make their own work on this repository easier — not
only replacements for shell utilities. If you find yourself wanting a script (install /
uninstall the app, set up prerequisites, check the translation file, list unused icons, …), make
it a Rust tool instead.

Adding tooling code is normal work, not a detour:

- When you need a command that does not exist, build it in the same task, run the tools gate
  (below), then use it. Report it under `Added:` in the task report.
- New tool dependencies go in the tooling crate's manifest, never in the app's.
- Built-in file tools stay available while you build, so a missing command never blocks you.

Before adding anything, in this order:

1. **Look.** Run `cargo tool --help` and read the tooling source to see what already exists.
2. **Improve.** If an existing command nearly fits, extend it (a flag, an option, better output)
   and fix anything weak you notice in it. Do not add a near-duplicate beside it.
3. **Add** a new command only when the need is real — you will use it in this task, or it
   replaces a step agents repeat — and say why in the task report. No tools "for later".

Keep tooling **distinct by area**. Each area is one clap subcommand group and one module, with
one clear concern:

| Area | Concern | Example commands |
| --- | --- | --- |
| `search` | finding things in the repo | `search grep`, `search files` |
| `data` | querying and converting structured data | `data jq`, `data toml`, `data csv` |
| `edit` | mechanical changes to files | `edit replace` |
| `install` | installing / uninstalling the app and its prerequisites | `install app`, `install remove` |

- A new command goes in the area it belongs to. Add a new area only for a concern no existing area
  covers, and give it a one-word name.
- MUST NOT mix concerns: a search command never edits files; an install command never searches.
  If a command needs another area's work, it calls that area's functions, it does not copy them.
- Tools that touch the system outside the repository (such as `install`) MUST say exactly what they
  will change, support a dry run (`--dry-run`), and change nothing until the user asks for it.
  They wrap existing mechanisms (`cargo install`, `cargo uninstall`, the OS's own paths via the
  `directories` crate); they do not reimplement an installer.

## Do not reinvent the wheel

`tools/` wraps established crates; it does not reimplement them. A command that replaces a
general utility is thin glue: parse arguments, call the crate, write the result.

Commands specific to this repository (check the translation file, list unused icons, …) may hold
their own logic, because no crate knows our files. They still use crates for the generic parts
(walking files, parsing, regex) and the existing areas' functions rather than copies of them.

| Need | Command | Wraps |
| --- | --- | --- |
| Query/transform JSON (`jq`) | `data jq` | `jaq-core`, `jaq-std`, `jaq-json` |
| Search file contents (`grep`/`rg`) | `search grep` | `grep` (ripgrep's library), `ignore` |
| Find files (`find`) | `search files` | `ignore`, `globset` |
| Regex replace (`sed`) | `edit replace` | `regex`, `ignore` |
| Compare text (`diff`) | `search diff` | `similar` |
| TOML / CSV to JSON | `data toml`, `data csv` | `toml`, `csv`, `serde_json` |

The table lists the intended mapping; add each command when it is first needed, not ahead of
time (see [workflow.md](workflow.md) — no speculative code).

- MUST NOT write your own JSON parser, regex engine, glob matcher, directory walker or diff
  algorithm. If a crate does it, use the crate.
- Pick crates that are maintained and widely used; check docs.rs for the version you add. Use
  `clap` (derive) for argument parsing.
- Keep the familiar interface of the wrapped tool where it is cheap (e.g. `jq` takes a jq filter;
  `grep` takes a pattern and paths and respects `.gitignore`). Do not invent new syntax.

## Combining tools

Commands read stdin and write stdout, so they chain through an ordinary pipe:

```sh
cargo tool search grep --json "Role::Custom" src | cargo tool data jq '.data.path.text'
cargo tool data toml Cargo.toml | cargo tool data jq '.dependencies | keys'
```

- Structured output is JSON. `search grep --json` uses `grep-printer`'s JSON format (the same as
  `rg --json`), so it pipes straight into `data jq`.
- A pipe between two `cargo tool` calls is fine. Piping into a non-Rust utility is not.
- If you chain the same steps repeatedly, add a command that does them in one call instead.

## Structure

- Tooling is a standalone crate with its own workspace and lock file, so tool dependencies never
  reach the app. A Cargo alias makes it `cargo tool`.
- The entry point is a clap enum with one variant per area and does nothing but dispatch. Each
  area is one module holding its subcommand enum; each command is one module of its own.
- If the tooling crate does not exist yet, the first agent that needs a tool sets up exactly
  this — with only the area and command it needs — and keeps its build output out of git.

## Standards for tool code

Tool code is project code. [rust.md](rust.md) applies in full, with these additions:

- The tooling crate carries the same `[lints]` table as the app, and it is protected in the
  same way. Keep the two identical.
- Write output with `writeln!` on a locked `std::io::stdout()` and return errors from `main`
  (`print_stdout`/`print_stderr` are linted). Exit non-zero on failure.
- Tools work on local files only: no network access, and no modification of files unless the
  command exists to do that (`edit replace`, `install`) and was asked to.
- Every command has `--help` text that says what it does, so the next agent can find it.
- Test each command's glue (arguments in, output out) — not the wrapped crate's behaviour.
- When working on tooling, run the quality gate for it as well (`tools/` is the crate's folder):

  ```sh
  cargo fmt --all --check --manifest-path tools/Cargo.toml
  cargo clippy --manifest-path tools/Cargo.toml --all-targets --all-features -- -D warnings
  cargo test --manifest-path tools/Cargo.toml --all-targets
  cargo build --manifest-path tools/Cargo.toml
  ```

- Every new command, area and dependency is an "add" step: justify it in the task report.
