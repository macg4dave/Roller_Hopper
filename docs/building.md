# Building Network Mapper

How to build, check and package Network Mapper from source. What each build needs installed,
and what a built copy needs to run, is in [dependencies.md](dependencies.md).

Only Windows 11 with an NVIDIA graphics card has been checked. Linux and macOS notes come from
upstream documentation and are unconfirmed.

## Builds at a glance

| Build | Command | Runs on |
| --- | --- | --- |
| Development (CUDA) | `cargo build` | A PC with an NVIDIA card and the files in [dependencies.md](dependencies.md#running-a-built-copy) |
| Release (CUDA) | `cargo build --release --locked` | The same |
| Without CUDA | `cargo build --release --locked --no-default-features` | Any 64-bit Windows 11 PC, with no AI |

The AI runs only on an NVIDIA GPU with 8 GB or more, never on the processor. A build without
CUDA has everything else, and says the GPU is not supported when the AI is asked for.

CUDA is the default feature, so a plain `cargo build`, `cargo run` or `cargo test` makes the
GPU-capable app. The executable lands in `target/debug/` or `target/release/` as
`network-mapper.exe`.

The first build needs internet access: Cargo downloads crates, and Slint's Skia renderer
downloads prebuilt Skia binaries with `curl`. Later builds work offline.

## Running from source

```sh
cargo run
```

Under `cargo run` the app reads device icons from the repository's `icons/` folder. UI text is
compiled into the program from `translations/`.

## Checking a change

Every change passes the quality gate, in this order:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build
```

`cargo tool checks gate` runs the same steps for the tools crate and then the app, stopping at
the first failure. On a machine without CUDA, `cargo tool checks gate --scope app --cpu` checks
the app built without CUDA; it is useful, but it does not replace the CUDA gate. Add `--offline` once
dependencies are fetched to prove the build uses only `Cargo.lock` and the local Cargo cache.

### AI smoke tests

The ignored AI tests load the models under `tools/a.i.testmodels/` on the GPU and check that
each answers the same way over repeated runs. They do not measure accuracy or speed. Close the
app (or unload its model) first so the card has room, then run them in release mode:

```sh
cargo test --release -- --ignored local_
```

`local_cuda_small` runs only the quick Qwen3-0.6B test. AI tests always run on the GPU, never
through `--no-default-features`.

## Making a portable folder

There is no installer. A portable copy is a folder you can move to another PC and run from
anywhere:

```text
network-mapper/
  network-mapper.exe     from target/release/
  icons/                 the repository's icons/ folder, copied whole
  msvcp140.dll           Visual C++ runtime
  vcruntime140.dll
  vcruntime140_1.dll
  cublas64_13.dll        CUDA build only
  cublasLt64_13.dll      CUDA build only
  curand64_10.dll        CUDA build only
```

1. Build the release: `cargo build --release --locked` (add `--no-default-features` for a
   folder without CUDA or AI).
2. Create the folder and copy in `target/release/network-mapper.exe` and the `icons/` folder.
3. Copy the Visual C++ runtime from Visual Studio's redistributable folder,
   `VC\Redist\MSVC\<version>\x64\Microsoft.VC145.CRT\`.
4. For the CUDA build only, copy the three CUDA files from `%CUDA_PATH%\bin\x64\`.

The CUDA folder is about 650 MB, mostly `cublasLt64_13.dll`. The folder without CUDA is small
and needs no NVIDIA files.

To confirm which libraries the program loads, list its imports with `dumpbin /dependents
network-mapper.exe` (from a Visual Studio developer prompt). The release build links only
`cublas64_13.dll` and `curand64_10.dll` from CUDA, and `cublas64_13.dll` loads
`cublasLt64_13.dll`. Don't copy cuDNN, `cudart` or NVRTC: the app never loads them.

Keep the folder outside `target/`, since `cargo clean` deletes everything there.

The program opens a console window beside its main window, as it is not yet marked as a
Windows GUI program.

## Development tools

The `tools/` crate holds the project's own commands, run with `cargo tool` (an alias in
`.cargo/config.toml`). `cargo tool --help` lists them; [tools/README.md](../tools/README.md)
describes each area. It uses the same toolchain and needs nothing more. Its `cuda` feature,
used by `cargo tool ai run`, builds Candle with CUDA; like the app, it runs models only on a
supported GPU.
