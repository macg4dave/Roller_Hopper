# Development tools

One standalone Rust crate, organised by the task each tool performs. Every area keeps its
CLI, command implementations and README together. Run commands from the repository root:

```sh
cargo tool --help
cargo tool <area> --help
cargo tool <area> <command> --help
```

| Task | Folder and documentation | Commands |
| --- | --- | --- |
| Text editing | [text/](text/README.md) | `text normalize-eol`, `text replace-text` |
| Development checks | [checks/](checks/README.md) | `checks gate`, `checks agent-command` |
| CUDA build | [cuda/](cuda/README.md) | `cuda build` |
| Runtime icon library | [icons/](icons/README.md) | `icons audit` |
| Model evaluation | [ai/](ai/README.md) | `ai run` (feature `ai`), `ai score` |

```text
tools/
  ai/        cli.rs, cases.rs, run.rs, score.rs, prompt.md, schema.json, cases.json, README.md
  checks/    cli.rs, gate.rs, agent_command.rs, README.md
  text/      cli.rs, edit.rs, normalize_eol.rs, replace_text.rs, README.md
  icons/     cli.rs, audit.rs, README.md
  src/       main.rs: shared CLI dispatch only
  tests/     shared CLI integration test harness
  Cargo.toml / Cargo.lock
```

Use installed utilities directly for established tasks, such as `rg` for text search. The
custom text commands supply checked mechanical edits. Dependencies and build output remain
separate from the product; the AI evaluator shares the app's Candle engine source. `clap` supplies CLI parsing
and help; `walkdir` supplies icon traversal; Candle (optional `ai` feature) runs local
models for evaluation.

The previous `repo` command group is replaced by `checks` and `text`. See each area's README
for examples, write behaviour and limitations.

## Scripts

Agents may write scripts (Python, PowerShell, shell and so on) when no existing command fits,
but every script MUST be saved under `tools/` and run by its path there, from the repository
root: `python tools/<area>/<name>.py`, `powershell -File tools\<area>\<name>.ps1`,
`./tools/<area>/<name>.sh`. Inline code (`python -c`, `node -e`, heredocs, code piped into an
interpreter) and scripts saved anywhere else, including temporary folders, are blocked by the
command hook ([checks/](checks/README.md#claude-command-hook)). Put a script in the folder of
the task it serves, start it with a comment saying what it does, and delete it when it is no
longer needed: git is the archive. A script that keeps being used is better made a tested
`cargo tool` command.
