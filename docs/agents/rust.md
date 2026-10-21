# Rust standards

Read before writing or reviewing Rust code, build scripts or dependencies. Part of the agent
rules indexed in [AGENTS.md](../../AGENTS.md).

## Lints are law

- The manifest's lint set (`clippy::all` + `clippy::pedantic` + selected restriction lints) is
  the baseline. The gate runs clippy with `-D warnings`, so every warning is a failure.
- MUST NOT use `#[allow(...)]` — it is denied. If a lint is genuinely wrong for one item, use
  `#[expect(clippy::lint_name, reason = "why this case is correct")]` on the **narrowest** item.
  Never at module or crate level. Never to make the gate pass faster.
- **The single exception** is the module that only wraps `slint::include_modules!()` and exempts
  that generated code. MUST NOT write any other code in it, and MUST NOT copy its exemption
  anywhere else.
- `unsafe` is denied (`deny`, not `forbid`, only because Slint's generated code needs a local
  allow). There is no reason for it in our code.

## Errors and panics

- MUST NOT use `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` or `dbg!` outside
  tests. Propagate with `?`, or handle the case explicitly.
- Prefer `.get()` over indexing (`v[i]`) so an out-of-range lookup is handled, not a crash.
- `main` returns `Result<(), slint::PlatformError>` (or a small error enum if a second error
  source appears). Do not add `anyhow`/`thiserror` until there is a real error type to model.
- Never silently swallow an error (`let _ = …`, `.ok()` with no reason). If ignoring is correct,
  say why in a comment.

## Types and data modelling

- Make invalid states unrepresentable. Use enums, not strings or bools, for closed sets:
  device kind, status (online / offline / unknown), connection kind (known / inferred).
- Custom roles are the one open set: model it as `enum Role { Router, …, Custom(String) }`, not as
  a bare `String`.
- Use std types for well-known values (`std::net::Ipv4Addr`). Use a newtype for identifiers
  (`struct DeviceId(u32)`) so IDs cannot be confused with indices or counts.
- Derive the standard traits where they make sense: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`,
  `Default`. Implement `Display` only for language-neutral formats (a MAC address, a port list).
  User-visible wording is never built in Rust: send data and let the UI word it (see
  [slint.md](slint.md)).
- Borrow by default: take `&str`/`&[T]`/`&T` in parameters. A `.clone()` added only to satisfy
  the borrow checker is a design smell — rethink ownership first. Use `Rc::clone(&x)` explicitly.
- No global mutable state: no `static mut`, no `lazy_static`/`OnceLock` holding app state. State
  lives in one struct owned by `main` and shared with callbacks via `Rc<RefCell<…>>`.

## Functions, modules and style

- Functions do one thing. Clippy caps length at 60 lines and arguments at 5 — if you hit a cap,
  split or introduce a struct; do not `#[expect]` your way past it.
- Iterators and combinators over manual index loops; `match` over `if` chains on enums. Avoid
  wildcard `_` arms on our own enums so adding a variant forces every match to be revisited.
- Binary crate: items are private by default, `pub(crate)` at most. Use the `foo.rs` + `foo/`
  module layout, not `mod.rs`.
- Keep files focused (guideline: under ~300 lines). A file that grows past that is a signal to
  split by responsibility, not to add a layer.
- Names are descriptive and follow Rust API guidelines (RFC 430). User-facing text is British
  English and lives in the translation files; identifiers follow the APIs they touch (e.g.
  Slint's `color`).
- Comments explain *why*, not *what*. Add a `//!` module doc to each module and `///` docs to
  any item whose purpose is not obvious from its name and signature.

## Dependencies

- Each dependency must solve a real problem that std/Slint does not. Check it is maintained and
  widely used. Disable default features you do not need.
- Adding a dependency is an "add" step: justify it in the report, and remove any code it
  replaces in the same change.
- Never add a crate for something a few lines of clear std code can do.

## Tests

Write tests generously, because they make changes safe. But they serve the app; the app is not
built around them.

- Unit-test model logic in a `#[cfg(test)] mod tests` next to the code: filtering, search,
  editing name/role/notes, adding a custom role, and sample data consistency (unique IDs, IPs and
  MACs; every connection references existing devices). Add or extend tests whenever you add or
  change testable logic.
- Test behaviour, not implementation details. No tests that only restate a constant.
- A bug fix comes with a test that fails without the fix, where the logic is testable.
- **AI checks use the small model.** To check that the AI loads and answers, or that CUDA
  works, run only the live tests on SmolLM2-135M:
  `cargo test --release -- --ignored llama sampling_settings` (CUDA is a default feature; for
  the CPU only, add `--no-default-features`). Run the larger models' tests (Qwen3-0.6B, Qwen3-4B) only when the task is measuring
  usefulness or speed, or the user asks.

### Tests are not the source of truth

- The source of truth is the product brief ([product.md](product.md)) and what the user asked
  for. A test is one piece of evidence about that, not the definition of correct.
- When a test fails, step back and look at the whole feature before changing anything: what the
  user sees, and how the model, bridge and UI fit together. Then decide which is wrong, the code
  or the test.
- If the test is wrong, update or delete it in the same change and give the reason in the task
  report. A test is wrong if it is out of date after an intended change, tied to implementation
  details, or encodes behaviour the brief does not want.
- MUST NOT bend the app's design, add special cases, or keep poor code just to satisfy a test.
- MUST NOT delete, weaken or `#[ignore]` a test *only* to get the gate green. Every removed or
  loosened test needs a stated reason that stands without the gate.
