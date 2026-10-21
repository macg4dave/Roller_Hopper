# Building Network Mapper

What a machine needs to build, test and run Network Mapper from source. Only Windows 11 with an
NVIDIA GPU has been checked; the Linux and macOS notes come from upstream documentation and are
unconfirmed (backlog BLK-009). What a machine needs only to *run* a built copy is in
[installing.md](installing.md).

## Every build

| Need | Version | Why |
| --- | --- | --- |
| Rust, through [rustup](https://rustup.rs) | 1.99.1, with `rustfmt` and `clippy` | Pinned in `rust-toolchain.toml`; rustup installs it on the first `cargo` command. |
| Internet access on the first build | — | Cargo downloads crates and Git dependencies, and `skia-bindings` downloads prebuilt Skia binaries with `curl` (part of Windows 10 and 11). |
| Windows: Visual Studio or its Build Tools, "Desktop development with C++" | 2022 or later (checked with Visual Studio 18) | MSVC and the Windows SDK, which Rust's `x86_64-pc-windows-msvc` target uses to link, and which `nvcc` uses as its host compiler. |

Without prebuilt Skia binaries for the platform, `skia-bindings` builds Skia itself and then
also needs LLVM (clang), Python 3 and Ninja.

## The default CUDA build

CUDA is a default feature. Plain `cargo build`, `cargo test` and the quality gate produce and
validate the GPU-capable app. Use the maintained command when you only need a CUDA build:

```sh
cargo tool cuda build
```

Candle's CUDA dependencies are cached in Cargo's normal target directory. The former
`cuda-cache/` belongs to the removed engine and remains untouched until the user chooses to remove
it. The first Candle CUDA build after the engine switch has not been authorised or qualified yet.

| Need | Version | Notes |
| --- | --- | --- |
| NVIDIA CUDA Toolkit, normal system install | 13.2 in use | `winget install --id Nvidia.CUDA --version 13.2` sets `CUDA_PATH` and puts its `bin` and `bin\x64` folders on `PATH`. |
| `NVCC_CCBIN` user environment variable | — | The folder holding MSVC's `cl.exe`, for example `C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\<version>\bin\Hostx64\x64`. Update it when Visual Studio updates MSVC to a new version folder. |
| NVIDIA GPU and driver | driver 580 or later | `nvidia-smi` tells the build which GPU to compile for. Without a GPU, set `CUDA_COMPUTE_CAP` (for example `120`). |

Restart the terminal or editor after installing so it sees the new environment variables.

- **CUDA qualification is pending.** The Candle source path and CPU model tests are verified; do
  not treat that as evidence that the default CUDA build or live GPU inference works.

## The CPU-only build

```sh
cargo build --release --no-default-features
```

For the complete portable source gate on a machine without CUDA:

```sh
cargo tool checks gate --scope app --cpu
```

This portable CPU-only gate does not replace the required default CUDA gate. Add `--offline` after
dependencies have been fetched to prove the build uses only the lockfile and local Cargo cache.

It needs nothing from the CUDA section. On the CPU the model is far faster with AVX2:

```sh
RUSTFLAGS="-C target-cpu=x86-64-v3" cargo run --release --no-default-features
```

A build made with that flag runs only on processors with AVX2.

## Development tools

The `tools/` crate (`cargo tool …`) uses the same toolchain and needs nothing more. Its `ai`
feature, used by `cargo tool ai run`, builds Candle on the CPU.

## Linux and macOS (unconfirmed)

- **Linux:** Slint's build guide lists fontconfig, xkbcommon and xcb development packages
  (Debian: `libfontconfig-dev`, `libxkbcommon-dev`, `libxcb-shape0-dev`,
  `libxcb-xfixes0-dev`). A CUDA build would need the Linux CUDA Toolkit and cuDNN, without
  `NVCC_CCBIN` or the MSVC flags.
- **macOS:** CUDA is not available, so build with `--no-default-features`.
