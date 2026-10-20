# Workflow and conduct

Applies to every task. Part of the agent rules indexed in [AGENTS.md](../../AGENTS.md).

## Change workflow: delete → combine → add

Every task, however small, follows these steps **in order**. Adding code is the last resort, not
the first instinct.

1. **Understand.** Read the code you will touch and the code that calls it, and the roadmap task
   or backlog entry the work belongs to. State the goal in one sentence before editing.
2. **Search.** Look for existing types, functions, Slint components, crate features or std APIs
   that already do — or nearly do — what is needed (code search, docs.rs, the Slint docs).
3. **Delete.** Remove code that the change makes obsolete, and any dead, duplicated or speculative
   code in the area you are touching. Do this *before* writing new code.
4. **Combine.** Meet the need by extending what exists: a new enum variant, a parameter, a reused
   Slint component, a merged pair of near-duplicate functions.
5. **Add — only what remains.** Prefer, in this order: std → native Slint feature → an established,
   maintained crate → new code. Every new file, module, public type or dependency needs a
   one-line justification in the task report.
6. **Verify.** Run the quality gate ([AGENTS.md](../../AGENTS.md#quality-gate)). All of it. Every
   time.
7. **Update the plan.** Bring the roadmap and backlog in line with what you did and found
   (see below).
8. **Report.** Use the report format ([AGENTS.md](../../AGENTS.md#task-report)).

Workflow rules:

- MUST NOT leave an old and a new version side by side. No `_v2`, `new_`, `old_`, `legacy_`,
  commented-out code, or "kept for reference" blocks. Git is the archive.
- MUST NOT write speculative code: no unused parameters, no trait with a single implementation
  "for later", no generics without a second caller, no feature flags, no `todo!()` stubs, no
  placeholder modules for roadmap work that has not started.
- MUST make the smallest diff that fully solves the task. Clean up dead/duplicated code *in the area
  you touch*; for problems elsewhere, list them in the report instead of fixing them unasked.
- If you are about to copy and paste a block, stop and combine instead. Only merge code that is
  genuinely the same concept — not code that merely looks alike.
- For refactors, a neutral or negative line count is the goal. Growth must be earned.
- When a change makes a doc comment, test or agent guide wrong, fix it in the same change.

## Planning: roadmap and backlog

Two living documents hold the plan. They describe what is still to do, never what was done:
git history is the archive. Agents MUST keep them current; they are not protected files.

- **[Roadmap](../roadmap.md): the next tasks.** Milestones in delivery order, each with tasks
  (`BE-` IDs), dependencies and a completion check. When the user asks for "the next task" or
  gives no specific task, take the first task whose `Depends on` is `—` and that no backlog
  blocker holds. A task that needs a user decision is done by preparing the options and asking.
- **[Backlog](../.backlog.md): bigger needs and blockers found.** Blockers (`BLK-` IDs) that
  hold roadmap tasks, and needs too big or too unclear for the current task. Record a blocker
  or need as soon as you find it rather than working round it silently.

Keep them free of rot:

- When a task is done, delete its row and remove its ID from every `Depends on` cell (leaving
  `—` when none remain). Delete a milestone, and its delivery-sequence row, once it has no tasks.
- Delete a blocker once resolved, recording the decision where it now applies (product brief,
  task text or code). Move a backlog need into the roadmap as tasks when it is planned, and
  delete it from the backlog then.
- Delete tasks and entries the code or a decision has made obsolete, and fix stale wording or
  links you notice. Never keep done, cancelled or superseded items, status logs or changelogs.
- Add only actionable items: a task states what changes and how to check it; a blocker states
  what it holds and what would resolve it. One or two lines each; rationale belongs in commits.
- Allocate IDs from the roadmap's register and never reuse one. No invented dates, estimates
  or percentages.
- Changing a product requirement needs the user's approval and goes into
  [product.md](product.md) first; the roadmap follows it.

## Agent conduct

- **Read before you write.** Never edit code you have not read in this session.
- **Do not guess APIs.** Check docs.rs / the Slint docs for the version pinned in `Cargo.lock`.
  If unsure an API exists, look it up rather than inventing it.
- **Stay in scope.** Do what was asked; propose — do not implement — anything extra.
- **Ask only when blocked** on a genuine product decision. Otherwise make a restrained choice
  in line with [product.md](product.md) and note it in the report.
- **Git.** Commit only when asked. Keep commits small and focused, using Conventional Commits
  (`feat:`, `fix:`, `refactor:`, `chore:`, `docs:`, `test:`). Never rewrite published history.
- **Honesty.** If something is skipped, partly done, or unverified, say so plainly.
