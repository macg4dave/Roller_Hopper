# Development checks

Run the repository's required checks through one ordered gate:

```sh
cargo tool checks --help
cargo tool checks gate --help
cargo tool checks gate
cargo tool checks gate --scope tools
cargo tool checks gate --scope app --dry-run
```

Each crate runs fmt check, strict clippy, tests and build, in that order. The default scope
checks tools first and then the app. Any failure stops the gate and produces a non-zero exit
code; Cargo output is inherited. `--dry-run` prints the plan without running checks.

The gate does not format or repair source files. Real checks can update Cargo lock files and
build artefacts. `cli.rs` dispatches commands; `gate.rs` owns the shared check plan.

## Claude command hook

```sh
cargo tool checks agent-command --help
cargo tool checks agent-command < hook-event.json
```

In PowerShell: `Get-Content -Raw hook-event.json | cargo tool checks agent-command`.
The input is data, for example:

```json
{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"cargo test --all-targets"}}
```

The command only inspects input; it never executes the supplied command. A permitted call
exits zero with empty stdout, leaving Claude's normal permissions intact. A prohibited call
exits zero with a `hookSpecificOutput` JSON denial and actionable tooling guidance. Malformed
JSON, an unexpected event/tool, or a missing/empty command exits non-zero with an error on stderr.

The project settings register both `Bash` and `PowerShell` with `onFailure: block` and a
120-second timeout. Cargo runs the explicit project tooling manifest in exec form; no shell
launcher script or hard-coded build-output path is needed. Cargo diagnostics go to stderr.
An unavailable Cargo executable, build failure or timeout must block the original call.
The first invocation may compile the tooling crate. Restart Claude after installing the
settings and inspect `/hooks` to confirm registration. This requires a Claude version supporting
exec-form command hooks and `onFailure`; see the
[official hook reference](https://code.claude.com/docs/en/hooks).

The [shared policy](../../docs/agents/tooling.md#command-enforcement) defines approved entry
points and the required alternative when blocked. Tests cover common command forms, quoted
paths and compound commands. The detector recognises command positions and quoted arguments;
it does not evaluate shell syntax, variable expansion, aliases, encoded programs, executable
contents or all obfuscations. Nested shells and dynamic shell programs are conservatively
rejected. It is an agent workflow guard, not a security boundary. A live Claude session is
needed to verify registration and actual tool-call denial; CLI tests alone do not prove that.
