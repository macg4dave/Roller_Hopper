# Dependencies

What Network Mapper depends on: the software needed to build it, what a built copy needs to run,
and the Rust crates it uses. How to build is in [building.md](building.md).

## Building

### Every build

| Need | Version | Why |
| --- | --- | --- |
| Rust, through [rustup](https://rustup.rs) | 1.99.1, with `rustfmt` and `clippy` | Pinned in `rust-toolchain.toml`; rustup installs it on the first `cargo` command. |
| Windows: Visual Studio or its Build Tools, "Desktop development with C++" | 2022 or later (checked with Visual Studio 18) | MSVC and the Windows SDK link the program, and `nvcc` uses MSVC as its host compiler. |
| Internet access on the first build | — | Cargo downloads crates; `skia-bindings` downloads prebuilt Skia binaries with `curl` (part of Windows 10 and 11). |

Without prebuilt Skia binaries for the platform, `skia-bindings` builds Skia itself and then
also needs LLVM (clang), Python 3 and Ninja.

### The CUDA build (the default)

With the default `cuda` feature, Candle's `candle-kernels` crate compiles its GPU code with
NVIDIA's `nvcc` during the build, so the build machine needs the CUDA Toolkit. A built copy
needs only the few files listed under [running a built copy](#running-a-built-copy).

| Need | Version | Notes |
| --- | --- | --- |
| NVIDIA CUDA Toolkit | 13.2 | `winget install --id Nvidia.CUDA --version 13.2` sets `CUDA_PATH` and puts its `bin` and `bin\x64` folders on `PATH`. |
| `NVCC_CCBIN` user environment variable | — | The folder holding MSVC's `cl.exe`, for example `C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\<version>\bin\Hostx64\x64`. Update it when Visual Studio moves MSVC to a new version folder. |
| NVIDIA graphics card and driver | driver 580 or later | `nvidia-smi` tells the build which GPU to compile for. Without a card, set `CUDA_COMPUTE_CAP` (for example `120`). |

cuDNN is not needed: the project does not turn on Candle's `cudnn` feature. Restart the
terminal or editor after installing so it sees the new environment variables.

### The CPU-only build

Needs nothing beyond [every build](#every-build).

### Linux and macOS (unconfirmed)

- **Linux:** Slint's build guide lists fontconfig, xkbcommon and xcb development packages
  (Debian: `libfontconfig-dev`, `libxkbcommon-dev`, `libxcb-shape0-dev`,
  `libxcb-xfixes0-dev`). The file dialogs need the system GTK3 library. A CUDA build needs the
  Linux CUDA Toolkit, without `NVCC_CCBIN`.
- **macOS:** CUDA is not available, so build with `--no-default-features`.

## Running a built copy

The end-user requirements are in [installing.md](installing.md). Technically, a built copy
needs:

- **Every build:** the `icons` folder beside `network-mapper.exe` (device icons are read from
  there at run time) and the Visual C++ runtime (`msvcp140.dll`, `vcruntime140.dll`), installed
  or beside the program. An AVX2 build runs only on processors with AVX2.
- **The CUDA build:** an NVIDIA driver 580 or later (it supplies `nvcuda.dll`) and three CUDA
  13 files, from a CUDA Toolkit installation or beside the program: `cublas64_13.dll`,
  `cublasLt64_13.dll` and `curand64_10.dll`. Without them Windows will not start the program,
  even if you only use the processor. [building.md](building.md#making-a-portable-folder)
  shows how to package them.
- **The AI model (optional):** Install AI downloads Qwen3-4B-Q4_K_M (2.5 GB) from Hugging Face,
  pinned to a revision and checked by size and SHA-256, into
  `%APPDATA%\Network Mapper\models\`. Other Qwen3 GGUF files in that folder (or one folder down)
  are listed too. The model is never bundled.

## Rust crates

Direct dependencies of the app, as pinned in `Cargo.toml`. Default features are turned off
wherever they aren't needed. `Cargo.lock` fixes the full tree.

| Crate | Version | Used for |
| --- | --- | --- |
| [slint](https://crates.io/crates/slint) | 1.18.1 | The UI, drawn with the Skia renderer; `raw-window-handle-06` parents the file dialogs to the window. |
| [slint-build](https://crates.io/crates/slint-build) | 1.18.1 | Build time: compiles the `.slint` files and bundles the translations. |
| [netscli-core](https://crates.io/crates/netscli-core) | 0.3.4 | The scanner: ping sweep, neighbour table, mDNS names, TCP connect probes. Only the `mdns` feature; never `pcap` (needs a capture driver) or `db`. |
| [tokio](https://crates.io/crates/tokio) | 1.54.0 | The runtime a scan runs on, off the UI thread. |
| [candle-core, candle-nn, candle-transformers](https://github.com/huggingface/candle) | 0.11.0 | Runs the AI model, on the CPU or with CUDA (the `cuda` feature). |
| [tokenizers](https://crates.io/crates/tokenizers) | 0.22.2 | Turns prompt text into the model's tokens. |
| [ureq](https://crates.io/crates/ureq) | 3.4.2 | Downloads the AI model over HTTPS (rustls). |
| [sha2](https://crates.io/crates/sha2) | 0.11.1 | Checks the downloaded model files. |
| [serde, serde_json](https://serde.rs) | 1.0.229, 1.0.154 | The project file format and saved settings. |
| [atomic-write-file](https://crates.io/crates/atomic-write-file) | 0.3.1 | Saves through a temporary file beside the target, so a failed save keeps the old file. |
| [rfd](https://crates.io/crates/rfd) | 0.17.2 | The native Open and Save dialogs; GTK3 on Linux, never a helper program. |
| [ipnet](https://crates.io/crates/ipnet) | 2.12.2 | IPv4 addresses with their subnet. |
| [dirs](https://crates.io/crates/dirs) | 7.0.0 | Finds the user's application data folder. |

The tools crate (`tools/Cargo.toml`) adds `clap` (command line), `walkdir` (icon folder
traversal) and `regex`, and reuses Candle and tokenizers behind its optional `ai` feature.

### Licences

The app is MIT-licensed. When last audited, every runtime package (all targets, with `mdns`)
was MIT, Apache-2.0, ISC, BSD-3-Clause or Unicode-3.0; LGPL appeared only as one choice of an
"OR". netscli-core embeds the IEEE OUI registry (about 400 KB) for card vendors. The Qwen3
model is Apache-2.0.

### Adding a dependency

Follow [docs/agents/rust.md](agents/rust.md#dependencies): a crate must solve a real problem
that std or Slint doesn't, be maintained and widely used, and come with only the features it
needs. Record it in the table above in the same change.
