# Slint standards

Read before touching `ui/`, `translations/` or `src/ui_bridge.rs`. Part of the agent rules
indexed in [AGENTS.md](../../AGENTS.md).

- UI lives in `.slint` files under `ui/`, compiled by `build.rs` with `slint-build` (not the
  `slint!` macro) so the UI is type-checked at build time. Style is `fluent` (Windows 11).
- Use `std-widgets` (`LineEdit`, `ComboBox`, `Button`, `ScrollView`, `TabWidget`, …) instead of
  rebuilding widgets.
- Design tokens (colours, spacing, radii, font sizes) live in one `global Theme`. MUST NOT
  scatter hard-coded colours or sizes across components.
- **UI text lives only in `translations/<lang>/LC_MESSAGES/network-mapper.po`.** MUST NOT
  write wording in `.slint` files or build it in Rust. Every visible or accessible string is
  `@tr("area.name")` with a lower-case dotted key (counts:
  `@tr("area.name" | "area.name.plural" % n)`). Wording derived from data (type, status, role,
  "last seen") goes through the `Labels` global; Rust sends enums, numbers and names.
- When adding, renaming or removing UI text, update the `msgid`/`msgstr` entries in
  `translations/en/…/network-mapper.po` (and any other language files) in the same change.
  `cargo test` fails on a key with no English text, an unused English entry, or wording passed
  to `@tr()`. Full how-to: [translations/README.md](../../translations/README.md).
- Small reusable components (device icon, status dot, device row, detail field) go in
  `ui/components/`. One component per concern; reuse before creating a variant.
- Keep each component focused (guideline: under ~200 lines). Past that, split by responsibility.
- **Rust owns the data; Slint presents it.** Rust exposes state through Slint structs and
  `ModelRc`/`VecModel`; Slint raises callbacks; Rust updates state and pushes the change back.
  Filtering, searching and editing logic live in Rust. Purely visual state (hover, pan offset,
  zoom level) may live in Slint.
- In callbacks, capture `ui.as_weak()` and upgrade it — never capture a strong component handle
  in its own callback (it creates a reference cycle).
- Icons are SVG files in `ui/icons/`: one consistent line-art style, same stroke width and
  viewBox, tinted with `colorize` rather than duplicated per colour.
- For UI changes, launch the app (`cargo run`) when possible and check the change visually
  against the design direction in [product.md](product.md).
