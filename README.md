# Network Mapper

A Rust + Slint desktop local network mapper. The map starts empty. The discovery page scans the
IPv4 subnet you type for real with
[netscli-core](https://crates.io/crates/netscli-core): a ping sweep with the neighbour table and
mDNS names, then TCP connects to common ports and reverse DNS, with mDNS services, device type,
OS family and card vendor. Found hosts can be added to the map one by one or imported together,
and devices can be added by hand. New, Open, Save and Save as keep the map in a
`.rhopper.json` project file. Settings → AI settings installs an AI
model (Qwen3-4B-Q4_K_M, 2.5 GB) into the user's application data folder,
loads one with [Candle](https://github.com/huggingface/candle), and runs it on an editable
prompt and sample device file. The AI runs only on an NVIDIA GPU with 8 GB or more, never on
the processor; elsewhere the app says the GPU is not supported. The app runs Qwen3 GGUF files
only; another one placed in that
`models` folder (`%APPDATA%\Network Mapper\models\` on Windows) or one folder down there is
listed beside it.

## Run

```sh
cargo run
```

To start directly with the large checked-in stress-test project, use a release build:

```sh
cargo run --release -- --mock-data
```

This loads `mock-data/infrastructure.rhopper.json`. Use release mode when judging interaction
performance on this fixture; debug builds are intentionally unoptimised and can make its large
topology appear slow.

The default build includes CUDA for running the AI on an NVIDIA graphics card. How to build,
including a build without CUDA (and so without AI) and a portable folder, is in
[docs/building.md](docs/building.md);
what to install first and the crates used are in
[docs/dependencies.md](docs/dependencies.md). Installing and running a built copy, for end
users, is in [docs/installing.md](docs/installing.md).

The models under `tools/a.i.testmodels/` are development assets for the GPU smoke tests:
`Qwen3/` is a copy of the application AI and `Qwen3_0.6B/` a small Qwen3 for quick checks.

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
src/model/         pure data and logic (devices with their network cards; the map's tree of
                   network cards, roles, groups,
                   layout; the discovery run's record) — no Slint imports
src/scan.rs        the real scan on its own Tokio threads, and in scan/ this computer's
                   subnet, the port lists, mDNS, the type and OS rules, and turning what a
                   host answers into model data — no Slint imports
src/ai.rs          the AI models offered, pinned by revision and hash, where they are
                   installed, the user's own GGUF files, the prompt and sample; ai/ downloads
                   and checks them and runs the loaded one with Candle, on the GPU only (CUDA) — no
                   Slint imports
src/ai_settings.rs the AI settings window: install, load/unload, GPU switch and Run in the
                   background
src/project.rs     the project file: its versioned format, the checks Open makes, safe saving
src/mock_data.rs   the tests' sample network, and in mock_data/ the hosts a test run finds
src/ui_bridge.rs   the app state and selection, wiring every Slint callback to it
src/ui_bridge/     edit/ (device, card, card move, port, role and group edits), map/ (layout, dragging,
                   lines, motion), discovery (running a scan), project (New, Open, Save and
                   the unsaved-changes question), view/ (sending state to
                   the UI), model ↔ Slint conversion, reading the icon folder, choosing the
                   language
ui/                .slint files: app window, panels, canvas, edit and discovery pages,
                   AI settings window, components/, theme
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
