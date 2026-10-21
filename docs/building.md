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

## The default CUDA build

CUDA is a default feature. Plain `cargo build`, `cargo test` and the quality gate produce and
validate the GPU-capable app. Use the maintained command when you only need a CUDA build:

```sh
cargo tool cuda build
cargo tool cuda rebuild
```

`cuda build` reuses `cuda-cache/`. `cuda rebuild` is the explicit command that deletes that cache
before rebuilding it. The cache is mistral.rs's supported shared CUDA build root, so it is reused
across Cargo output directories. The CUDA build needs:

Every ordinary build, run, test and gate keeps CUDA enabled and reuses this cache. No permission is
needed to consume it. Populating a missing cache or rebuilding invalidated objects requires the
user's explicit permission; agents must stop an unexpected `nvcc`, `cicc` or `ptxas` run rather
than silently regenerating kernels. See [AGENTS.md](../AGENTS.md#protected-cuda-cache).

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
- **The first cache build takes about 12 minutes**, almost all of it CUDA kernels. Later builds
  reuse the shared objects even across Cargo profiles.
- **Explicitly rebuild after changing build inputs.** After changing CUDA, cuDNN, MSVC, Mistral,
  the GPU compute capability, features or kernel flags, a human may run
  `cargo tool cuda rebuild`; an AI agent must first obtain permission for that exact cache
  invalidation. Source-only app changes continue to reuse the existing cache.

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
feature, used by `cargo tool ai run`, builds mistral.rs on the CPU.

## Linux and macOS (unconfirmed)

- **Linux:** Slint's build guide lists fontconfig, xkbcommon and xcb development packages
  (Debian: `libfontconfig-dev`, `libxkbcommon-dev`, `libxcb-shape0-dev`,
  `libxcb-xfixes0-dev`). A CUDA build would need the Linux CUDA Toolkit and cuDNN, without
  `NVCC_CCBIN` or the MSVC flags.
- **macOS:** CUDA is not available, so build with `--no-default-features`.
