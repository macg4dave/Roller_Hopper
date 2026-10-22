# Network Mapper

A Rust + Slint desktop **UI prototype** of a local network mapper. It uses in-memory mock data
only — there is no real network discovery or scanning. Settings → AI settings installs an AI
model (Qwen3-4B, 2.5 GB; or Qwen3-0.6B, 0.6 GB) into the user's application data folder,
loads one with [Candle](https://github.com/huggingface/candle), and runs it on an editable
prompt and sample device file. A Qwen2, Qwen3, Llama 3, Gemma 3, Phi-3 or GLM-4 text model of
your own — a GGUF file, or a Hugging Face safetensors model folder (`config.json`,
`tokenizer.json` and `*.safetensors`) — placed in that `models` folder
(`%APPDATA%\Network Mapper\models\` on Windows) or one folder down there is listed beside
them. Its own tokenizer is used; a Gemma 3 or Phi-3 GGUF file needs the model's
`tokenizer.json` beside it.

## Run

```sh
cargo run
```

The default build includes CUDA for running the AI on an NVIDIA graphics card. What to install
first, and the CPU-only build, are in [docs/building.md](docs/building.md); what a computer
needs to run a built copy is in [docs/installing.md](docs/installing.md).

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
                   layout; the simulated discovery run) — no Slint imports
src/ai.rs          the AI models offered, pinned by revision and hash, where they are
                   installed, the user's own GGUF files, the prompt and sample; ai/ downloads
                   and checks them and runs the loaded one with mistral.rs (CPU or CUDA) — no
                   Slint imports
src/ai_settings.rs the AI settings window: install, load/unload, GPU switch and Run in the
                   background
src/mock_data.rs   the sample network, and in mock_data/ the hosts a discovery run finds
src/ui_bridge.rs   the app state and selection, wiring every Slint callback to it
src/ui_bridge/     edit/ (device, card, card move, port, role and group edits), map/ (layout, dragging,
                   lines, motion), discovery (the timed scan run), view/ (sending state to
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
