# Slint standards

Read before UI work, UI text or translation work, or changes to the code that connects the model
to the UI. Part of the agent rules indexed in [AGENTS.md](../../AGENTS.md).

- UI lives in `.slint` files compiled by the build script with `slint-build` (not the `slint!`
  macro), so the UI is type-checked at build time. Style is `fluent` (Windows 11).
- Use `std-widgets` (`LineEdit`, `ComboBox`, `Button`, `ScrollView`, `TabWidget`, …) instead of
  rebuilding widgets.
- Design tokens (colours, spacing, radii, font sizes) live in one `global Theme`. MUST NOT
  scatter hard-coded colours or sizes across components.
- **UI text lives only in the gettext translation files.** MUST NOT write wording in `.slint`
  files or build it in Rust. Every visible or accessible string is `@tr("area.name")` with a
  lower-case dotted key (counts: `@tr("area.name" | "area.name.plural" % n)`). Wording derived
  from data (type, status, role, "last seen") goes through one global of label functions; Rust
  sends enums, numbers and names.
- When adding, renaming or removing UI text, update the English entries (and any other
  language's) in the same change. The tests fail on a key with no English text, an unused
  English entry, or wording passed to `@tr()`.
- Text that is the user's own data (names, notes, custom roles, file names the user chose) is
  shown as given, not translated.
- Small reusable components (an icon, a status dot, a list row, a detail field) each get their
  own file among the shared components. One component per concern; reuse before creating a
  variant.
- Keep each component focused (guideline: under ~200 lines). Past that, split by responsibility.
- **Rust owns the data; Slint presents it.** Rust exposes state through Slint structs and
  `ModelRc`/`VecModel`; Slint raises callbacks; Rust updates state and pushes the change back.
  Filtering, searching and editing logic live in Rust. Purely visual state (hover, pan offset,
  zoom level, panel widths) may live in Slint.
- In callbacks, capture `ui.as_weak()` and upgrade it — never capture a strong component handle
  in its own callback (it creates a reference cycle).
- Icons are SVG line art in one consistent style, same stroke width and viewBox, tinted with
  `colorize` rather than duplicated per colour. Icons that are part of the UI are compiled in;
  icons the user may add are read at run time, with a compiled-in fallback.
- For UI changes, launch the app (`cargo run`) when possible and check the change visually
  against the design direction in [product.md](product.md).
