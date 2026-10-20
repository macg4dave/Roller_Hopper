# Network Mapper

A Rust + Slint desktop **UI prototype** of a local network mapper. It uses in-memory mock data
only — there is no real network discovery or scanning.

## Run

```sh
cargo run
```

## Icons

Device and role icons come from the `icons/` folder. Every SVG file in it can be picked in the
app's icon picker, named after its file (`Fish tank.svg` shows as "Fish tank"). Each subfolder
becomes a heading over its icons; deeper folders are not read. Icons added while the app runs
appear the next time the picker opens.

Use single-colour line art: icons are tinted to show status and selection, so a coloured icon
shows as one solid colour. A built app looks for an `icons` folder beside its executable; under
`cargo run` it uses the one in this repository.

## Languages

All UI text is translatable. To add or change a language, see
[translations/README.md](translations/README.md).

## Project layout

```text
src/main.rs        wiring only: build state, create window, connect callbacks, run
src/ui.rs          Slint-generated types only (lint-exempt; never add code here)
src/model/         pure data and logic (devices with their network cards, virtual machines and
                   containers; the map's tree of network cards, roles, groups,
                   layout) — no Slint imports
src/mock_data.rs   the sample network
src/ui_bridge/     model ↔ Slint conversion, callback handlers, map motion, reading the icon folder
ui/                .slint files: app window, panels, canvas, components/, theme
ui/icons/          icons that are part of the UI (buttons, logo), compiled in
icons/             device icons for the picker, read at run time (see above)
translations/      all UI text: one gettext .po file per language (English in en/)
docs/agents/       the agent guides indexed by AGENTS.md
```

Development tooling is a separate crate in `tools/`, run with `cargo tool`. Use
`cargo tool checks gate` for the tools and app quality gates, `cargo tool text --help` for
text editing, or `cargo tool icons audit` to check the icon library's paths and identifiers.
Each task area has its own folder and README; see [tools/README.md](tools/README.md).

## Contributing (humans and AI agents)

Read [AGENTS.md](AGENTS.md) first. It indexes the guides in [docs/agents/](docs/agents/) — the
product brief, the Rust/Slint standards, the delete → combine → add workflow and how agents share
the repo — and holds the quality gate every change must pass:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build
```
