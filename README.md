# Network Mapper

A Rust + Slint desktop **UI prototype** of a local network mapper. It uses in-memory mock data
only — there is no real network discovery or scanning. Settings → AI settings installs an AI
model (Qwen3-4B, 2.5 GB; Qwen3-0.6B, 0.6 GB; or SmolLM2-135M, 0.15 GB) into the user's
application data folder,
loads one with [mistral.rs](https://github.com/EricLBuehler/mistral.rs), and runs it on an
editable prompt and sample device file. A GGUF file of your own (Llama, Mistral, Phi,
Starcoder2 or Qwen; mistral.rs 0.8.1 cannot run others such as LFM2 or Gemma) placed in that
`models` folder — `%APPDATA%\Network Mapper\models\` on Windows — or in a folder of its own
there is listed beside them; its own tokenizer and chat template are used.

## Run

```sh
cargo run
```

The AI model is far faster in a release build with AVX2 (on the CPU, about 25 s rather than
150 s for one answer from Qwen3-0.6B):

```sh
RUSTFLAGS="-C target-cpu=x86-64-v3" cargo run --release
```

### AI on the GPU (NVIDIA, Windows)

The `cuda` feature runs the model on an NVIDIA graphics card. It builds against the CUDA
13.2 toolkit unpacked in the repository's `cuda-toolkit/` folder (ignored by git; how to fill
it is in [tools/cuda](tools/cuda/README.md)), with Visual Studio's C++ tools and an NVIDIA
driver. Run Cargo through the repository command, which sets the build settings, puts the
toolkit on `PATH` for that run and copies the CUDA DLLs beside the app and its tests:

```sh
cargo tool cuda cargo run --release --features cuda
cargo tool cuda cargo test --release --features cuda -- --ignored llama sampling_settings
cargo tool cuda cargo clippy --all-targets --all-features -- -D warnings
```

The first build compiles mistral.rs's CUDA kernels and takes about 12 minutes. Those kernel
builds do not notice a changed setting, so after changing one run
`cargo clean --release -p candle-kernels -p mistralrs-quant`. A plain `cargo clippy
--all-features`, as the gate runs it, fails without these settings (backlog BLK-011).

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
