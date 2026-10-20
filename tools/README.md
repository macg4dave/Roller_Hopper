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
separate from the product; the tools do not import product code. `clap` supplies CLI parsing
and help; `walkdir` supplies icon traversal; `mistralrs` (optional `ai` feature) runs local
models for evaluation.

The previous `repo` command group is replaced by `checks` and `text`. See each area's README
for examples, write behaviour and limitations.
