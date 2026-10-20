@AGENTS.md
@docs/agents/workflow.md
@docs/agents/collaboration.md
@docs/agents/product.md
@docs/agents/rust.md
@docs/agents/slint.md
@docs/agents/tooling.md

## Claude Code specifics

- `AGENTS.md` and `docs/agents/` (imported above) are the source of truth, shared with Codex and
  other agents. Do not duplicate their rules here; add only Claude Code-specific notes.
- For work that touches more than three files, outline the delete → combine → add plan before
  editing.
- Run the quality gate through the shell exactly as written in `AGENTS.md`.
- `cargo add` and edits to protected files trigger a permission prompt by design (Claude Code
  settings). Explain why the change is needed when asking.
- `.claude/settings.json` runs `checks agent-command` before Bash and PowerShell calls using
  Cargo's explicit tooling manifest path (independent of the shell's working directory).
  Hook startup, build, input and timeout failures block the call. See
  [the checks README](tools/checks/README.md#claude-command-hook) for verification and mechanics;
  [the shared tooling guide](docs/agents/tooling.md#command-enforcement) owns the policy.
