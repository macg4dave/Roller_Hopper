# Working alongside other agents

Claude Code, Codex and humans all work in this repository, sometimes at the same time. Read this
before starting any task. Part of the agent rules indexed in [AGENTS.md](../../AGENTS.md).

## One source of truth

- `AGENTS.md` and `docs/agents/` are the only rule files. Every agent reads the same rules.
- Tool-specific files hold **only** tool mechanics, never project rules: `CLAUDE.md` loads the
  shared guides into Claude; Codex reads `AGENTS.md` directly. MUST NOT copy rules into
  tool-specific files (`CLAUDE.md`, `.cursorrules`, `.github/copilot-instructions.md`, …) — link
  to `AGENTS.md` instead, so the agents cannot drift apart.

## Before you start

- Run `git status` and `git diff --stat`. Changes you did not make belong to the user or another
  agent.
- MUST NOT revert, reformat, rename or "clean up" changes you did not make. If they overlap the
  files your task needs, stop and ask the user rather than editing on top of them.
- If the last task report or commit says work was left unfinished, read it before picking up the
  same area.

## Sharing a working tree

- Only one agent edits a working tree at a time. For parallel work, give each agent its own
  branch in its own worktree:

  ```sh
  git worktree add ../<repository>-<agent>-<topic> -b <agent>/<topic>
  ```

  Branch names start with the agent (`claude/…`, `codex/…`) so ownership is obvious.
- `cargo fmt --all` rewrites every file. Run it only when the working tree holds nothing but your
  own changes; otherwise format just your files with `rustfmt <path>`. The gate's
  `cargo fmt --all --check` never writes, so it is always safe.
- Do not delete or regenerate shared outputs another agent may be using (build output, lock
  files) unless the task requires it.

## Handing over

- The task report ([AGENTS.md](../../AGENTS.md#task-report)) is the handover. If you stop before
  the task is done, add a `Next:` line saying exactly what remains and in which files.
- When you commit, add a trailer naming the agent, e.g. `Co-authored-by: Claude <…>` or
  `Co-authored-by: Codex <…>`, so history shows who made each change.
