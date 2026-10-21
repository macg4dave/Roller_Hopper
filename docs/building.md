# Building Network Mapper

What a machine needs to build, test and run Network Mapper from source. Only Windows 11 with an
NVIDIA GPU has been checked; the Linux and macOS notes come from upstream documentation and are
unconfirmed (backlog BLK-009). What a machine needs only to *run* a built copy is in
[installing.md](installing.md).

## Every build

| Need | Version | Why |
| --- | --- | --- |
| Rust, through [rustup](https://rustup.rs) | 1.99.1, with `rustfmt` and `clippy` | Pinned in `rust-toolchain.toml`; rustup installs it on the first `cargo` command. |
| Git | any recent | mistral.rs is a Git dependency (v0.9.4 is not on crates.io). |
| Internet access on the first build | — | Cargo downloads crates and Git dependencies, and `skia-bindings` downloads prebuilt Skia binaries with `curl` (part of Windows 10 and 11). |
| Windows: Visual Studio or its Build Tools, "Desktop development with C++" | 2022 or later (checked with Visual Studio 18) | MSVC and the Windows SDK, which Rust's `x86_64-pc-windows-msvc` target uses to link, and which `nvcc` uses as its host compiler. |

Without prebuilt Skia binaries for the platform, `skia-bindings` builds Skia itself and then
also needs LLVM (clang), Python 3 and Ninja.

## The CUDA build (the default)

The `cuda` feature is on by default, so a plain `cargo build`, `cargo test` and the quality
gate all compile mistral.rs's CUDA kernels. That needs:

| Need | Version | Notes |
| --- | --- | --- |
| NVIDIA CUDA Toolkit, normal system install | 13.0–13.3 (13.2 in use) | mistral.rs 0.9.4 rejects 13.4. `winget install --id Nvidia.CUDA --version 13.2` sets `CUDA_PATH` and puts its `bin` and `bin\x64` folders on `PATH`. |
| NVIDIA cuDNN | 9 for CUDA 13 (9.27 in use) | Copy its headers into `%CUDA_PATH%\include`, `lib\x64\cudnn*.lib` into `%CUDA_PATH%\lib\x64` and `bin\x64\cudnn*.dll` into `%CUDA_PATH%\bin\x64`. Once `CUDA_PATH` is set, the build searches only there. Archives: <https://developer.download.nvidia.com/compute/cudnn/redist/cudnn/windows-x86_64/>. |
| `NVCC_CCBIN` user environment variable | — | The folder holding MSVC's `cl.exe`, for example `C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\<version>\bin\Hostx64\x64`. Update it when Visual Studio updates MSVC to a new version folder. |
| NVIDIA GPU and driver | driver 580 or later | `nvidia-smi` tells the build which GPU to compile for. Without a GPU, set `CUDA_COMPUTE_CAP` (for example `120`). |

`.cargo/config.toml` adds the nvcc flags the kernels need on Windows (`NVCC_APPEND_FLAGS`).
Restart the terminal or editor after installing so it sees the new environment variables.

- **The output runs only on the GPU generation it was built for.** The kernels are compiled
  for the build machine's compute capability (12.0, `sm_120a`, for an RTX 50-series card) and
  carry no code for other generations.
- **The first build takes about 12 minutes**, almost all of it CUDA kernels. Debug and release
  builds compile them separately, and so does rust-analyzer's own debug build.
- **Kernel builds do not notice every changed setting.** After changing the toolkit, cuDNN or
  MSVC, run `cargo clean --release -p candle-kernels -p mistralrs-quant`. On the main
  development PC, AGENTS.md protects the release kernel cache: ask before cleaning it.

## The CPU-only build

```sh
cargo build --release --no-default-features
```

It needs nothing from the CUDA section. On the CPU the model is far faster with AVX2:

```sh
RUSTFLAGS="-C target-cpu=x86-64-v3" cargo run --release --no-default-features
```

A build made with that flag runs only on processors with AVX2. The quality gate's
`cargo clippy --all-features` still builds the CUDA feature (backlog BLK-011).

## Development tools

The `tools/` crate (`cargo tool …`) uses the same toolchain and needs nothing more. Its `ai`
feature, used by `cargo tool ai run`, builds mistral.rs on the CPU.

## Linux and macOS (unconfirmed)

- **Linux:** Slint's build guide lists fontconfig, xkbcommon and xcb development packages
  (Debian: `libfontconfig-dev`, `libxkbcommon-dev`, `libxcb-shape0-dev`,
  `libxcb-xfixes0-dev`). A CUDA build would need the Linux CUDA Toolkit and cuDNN, without
  `NVCC_CCBIN` or the MSVC flags.
- **macOS:** CUDA is not available, so build with `--no-default-features`.
