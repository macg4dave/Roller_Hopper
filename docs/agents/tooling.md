# Agent tooling

Read before running any shell command other than `cargo`, `git` or `rustfmt`, and before working
on agent tooling. Part of the agent rules indexed in [AGENTS.md](../../AGENTS.md).

Use established tools to get the work done. Commands such as `jq`, `rg` and `grep` can run
directly. Build useful repository tooling when a task needs it, and use that tooling to complete
the task. A missing command is a problem to solve, not a reason to stop or skip required work.

## Use the tools available

- Run `cargo`, `git`, `gh`, `rustfmt` and your agent's built-in file tools directly.
- Run installed utilities directly: `jq`, `rg`, `grep`, `sed`, `awk`, `find`, `xargs`, `diff`,
  PowerShell cmdlets and other established tools are allowed. They do not need Rust wrappers.
- Use commands appropriate to the current shell and platform. Check availability and `--help`
  when needed; do not assume a Unix command has the same meaning in PowerShell.
- Ordinary pipes and short, readable shell commands are encouraged. Use an existing tool for
  the job it already does well: `jq` for JSON, `rg` or `grep` for search, and so on.
- MUST NOT write or run one-off Python scripts for repository work. This includes inline
  programs (`python -c`, heredocs), temporary scripts and scripts deleted after use. Python
  and Node may launch established tools, but MUST NOT contain ad hoc task logic. Do not move
  that logic into a Node program or a Bash/PowerShell script to bypass this rule.
- MUST use existing repository tooling when it covers the task. If it falls short, improve
  the closest existing command; if no suitable command exists, implement the smallest tested
  Rust command in `tools/` and use it in the same task, even if the immediate need is one-off.
  Direct use of established utilities and short pipelines as described above remains allowed.
- If a preferred utility is missing, first use a suitable installed equivalent, an existing
  repository command or a built-in file tool. Do not install or reimplement a utility merely
  because it is your usual choice.

For example, these need no custom repository command:

```sh
rg "Role::Custom" src
git diff --stat
cargo metadata --no-deps --format-version 1 | jq '.packages[].name'
```

## Build useful tooling when needed

Adding necessary tooling is part of the current task. Agents MUST build or extend a repository
tool when existing tools cannot adequately complete a concrete task step, or when substantial
custom logic needs a maintained home. A repeated workflow can also justify combining existing
steps into one useful command.

Follow [workflow.md](workflow.md)'s delete → combine → add order:

1. **Look.** Inspect the available tools and repository tooling with built-in file tools or
   permitted commands. If `tools/` and the Cargo alias exist, run `cargo tool --help` and read
   the relevant source. The example commands in this guide are not an inventory of implemented
   commands.
2. **Reuse or improve.** Use an existing command or a short pipeline where it meets the need.
   Extend a nearly suitable repository command instead of adding a duplicate. Fix weaknesses
   relevant to the task; leave unrelated work alone.
3. **Build the gap.** If custom tooling is needed, MUST implement the smallest working command
   in the same task. Set up the crate and alias below if missing. No placeholders, speculative
   commands or complete replacements for utilities that already work.
4. **Verify and use.** Test the command, run its quality gate, then use it on the actual task
   input and check the result. Continue the original task and run the app's quality gate.

- MUST NOT stop, defer the task, skip a required check or ask the user to supply tooling solely
  because a command, crate or alias does not exist. Try an adequate available tool, then build
  the necessary repository tooling if the gap remains. Report a genuine environment or
  permission blocker with the failed command and error.
- Necessary repository tooling changes are authorised as part of the task; no separate request
  is needed. Creating a tool does not authorise system changes outside the task's scope.
- Every new command, area and dependency is an "add" step: explain its actual use in the task
  report. No tools "for later".

## Reuse established implementations

Custom tooling is Rust code in `tools/`. Use std first, then maintained crates or existing
executables for generic work. A command can wrap an established tool and combine its output
with repository-specific logic. Keep that glue small and handle missing executables and failed
commands with clear errors.

- MUST NOT write your own JSON parser, regex engine, glob matcher, directory walker or diff
  algorithm. Use established implementations when those capabilities are needed.
- Do not add `cargo tool data jq` or `cargo tool search grep` merely to rename `jq` or `grep`.
  Add a wrapper only when it supplies a concrete repository need beyond the existing command.
- New Rust dependencies belong in the tooling crate's manifest, never in the app's. Pick
  maintained crates, check docs.rs for the version being added, and disable unused features.
- Use `clap` derive for custom command-line arguments, with discoverable `--help` text.
- Read stdin and write stdout where useful. Structured output is JSON; errors go to stderr and
  failures exit non-zero. Repository commands may pipe into standard tools and vice versa.

## Structure and bootstrap

Create the tooling crate only when custom tooling is needed. Use built-in file tools to write
it and available commands to inspect the repository; the crate cannot be a prerequisite for
its own creation.

- `tools/` is a standalone crate with its own `[workspace]` and `Cargo.lock`, keeping its
  dependencies separate from the app's. Keep its lock file in version control when committing.
- Copy the app's lint tables unchanged into a new tooling manifest. Preserve existing tooling
  and protected configuration. Ignore `tools/target/` build output.
- The entry point uses a clap enum to dispatch to areas. Each area has one clear concern and
  one module; each command has its own module. Add only the area and command needed now.
- Extend an existing area before adding another. Areas might cover repository checks,
  inspection, mechanical edits or installation. Reuse functions across commands rather than
  copying their logic.
- Add a missing alias in `.cargo/config.toml`, preserving any existing settings:

  ```toml
  [alias]
  tool = "run --manifest-path tools/Cargo.toml --"
  ```

If the crate exists but the alias is missing or broken, inspect or run it directly:

```sh
cargo run --manifest-path tools/Cargo.toml -- --help
cargo run --manifest-path tools/Cargo.toml -- <area> <command>
```

An alias failure does not mean the implementation is missing. After setup, confirm
`cargo tool <area> <command> --help` and the real task invocation both work.

## Standards for custom tools

Tool code is project code. [rust.md](rust.md) applies in full, with these additions:

- The tooling crate carries the same lint tables as the app, protected in the same way.
- Write output with `writeln!` on locked stdout/stderr and return errors from `main`;
  `print_stdout` and `print_stderr` are linted.
- Keep file-processing commands local. A search or inspection command does not edit files;
  a command that edits files must make that purpose clear and stay within the authorised task.
- Tools that change the system outside the repository, such as installation tools, MUST
  describe exactly what they change, support `--dry-run` and run only when the user has
  authorised those changes. Wrap existing installation mechanisms rather than reimplementing
  them; their network access is allowed when the installation needs it.
- Test the command's glue: arguments, input/output, failure handling and repository-specific
  behaviour. Do not retest a wrapped crate or utility's own implementation.
- When adding or changing Rust tooling, run its full gate as well as the app's gate:

  ```sh
  cargo fmt --all --check --manifest-path tools/Cargo.toml
  cargo clippy --manifest-path tools/Cargo.toml --all-targets --all-features -- -D warnings
  cargo test --manifest-path tools/Cargo.toml --all-targets
  cargo build --manifest-path tools/Cargo.toml
  ```
