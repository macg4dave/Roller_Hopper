@AGENTS.md
@docs/agents/workflow.md
@docs/agents/collaboration.md
@docs/agents/product.md
@docs/agents/rust.md
@docs/agents/slint.md

## Claude Code specifics

- `AGENTS.md` and `docs/agents/` (imported above) are the source of truth, shared with Codex and
  other agents. Do not duplicate their rules here; add only Claude Code-specific notes.
- For work that touches more than three files, outline the delete → combine → add plan before
  editing.
- Run the quality gate through the shell exactly as written in `AGENTS.md`.
- `cargo add` and edits to protected files trigger a permission prompt by design
  (`.claude/settings.json`). Explain why the change is needed when asking.
