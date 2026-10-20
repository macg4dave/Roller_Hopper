# CUDA builds

Build, run and test the app's `cuda` feature against the CUDA toolkit kept in the repository's
`cuda-toolkit/` folder (ignored by git):

```sh
cargo tool cuda --help
cargo tool cuda cargo --help
cargo tool cuda cargo build --release --features cuda
cargo tool cuda cargo run --release --features cuda
cargo tool cuda cargo test --release --features cuda -- --ignored llama sampling_settings
cargo tool cuda cargo clippy --all-targets --all-features -- -D warnings
cargo tool cuda cargo --dry-run build --release --features cuda
```

`cuda cargo` runs Cargo in the repository root with:

- `CUDA_PATH`, `CUDA_HOME` and `NVCC` pointing into `cuda-toolkit/`;
- `CUDARC_CUDA_VERSION` read from `nvcc --version` (13.2 gives `13020`);
- `CUDA_COMPUTE_CAP` from `nvidia-smi` (12.0 gives `120`), unless it is already set;
- on Windows, `NVCC_CCBIN` (MSVC's `cl.exe` folder, found as the `cc` crate finds it) and
  `NVCC_APPEND_FLAGS` (`/Zc:preprocessor` for CUDA 13's headers, `/MD` to match Skia);
- the toolkit's `bin` and `bin\x64` first on `PATH`, because mistral.rs runs bare `nvcc`.

These apply to that Cargo run only; they override any `[env]` in a user Cargo config. Before
running Cargo it copies the toolkit's runtime DLLs (`bin\x64\*.dll`) into the target
directory's profile folder (`--release`, `--profile`, else `debug`) and its `deps` folder,
where the app and its test binaries load them; files already there at the same size are
skipped. `--dry-run` prints the settings and command and copies nothing.

## The toolkit

mistral.rs 0.8.1 accepts CUDA 11.4 to 13.2. `cuda-toolkit/` holds NVIDIA's CUDA 13.2.2
Windows redistributables (`cuda_nvcc`, `cuda_crt`, `libnvvm`, `cuda_cccl`, `cuda_cudart`,
`cuda_nvrtc`, `libcublas`, `libcurand`) from
<https://developer.download.nvidia.com/compute/cuda/redist/redistrib_13.2.2.json>, each
archive checked against the manifest's SHA-256 and unpacked over one folder with Windows'
`tar.exe -xf <archive> -C cuda-toolkit --strip-components=1`. Visual Studio's C++ tools and
an NVIDIA driver are needed as well.

`cli.rs` dispatches; `cargo.rs` builds the settings, copies the DLLs and runs Cargo.
