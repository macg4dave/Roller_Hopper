# Slint standards

Read before touching `ui/` or `src/ui_bridge.rs`. Part of the agent rules indexed in
[AGENTS.md](../../AGENTS.md).

- UI lives in `.slint` files under `ui/`, compiled by `build.rs` with `slint-build` (not the
  `slint!` macro) so the UI is type-checked at build time. Style is `fluent` (Windows 11).
- Use `std-widgets` (`LineEdit`, `ComboBox`, `Button`, `ScrollView`, `TabWidget`, …) instead of
  rebuilding widgets.
- Design tokens (colours, spacing, radii, font sizes) live in one `global Theme`. MUST NOT
  scatter hard-coded colours or sizes across components.
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
  against the [reference mockup](../design/reference-mockup.png).
