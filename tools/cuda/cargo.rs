//! Runs Cargo against the repository's CUDA toolkit (`cuda-toolkit/`): the build settings that
//! mistral.rs and Candle read, the toolkit first on `PATH` for this run only (mistral.rs runs
//! bare `nvcc`), and the toolkit's runtime DLLs copied beside the build first, so `run` and
//! `test` find them. Nothing outside the repository and the target folder changes.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

/// Where the toolkit is unpacked, relative to the repository root; ignored by git.
const TOOLKIT: &str = "cuda-toolkit";

#[derive(Debug, clap::Args)]
#[command(
    after_help = "Examples:\n  cargo tool cuda cargo build --release --features cuda\n  cargo tool cuda cargo run --release --features cuda\n  cargo tool cuda cargo test --release --features cuda -- --ignored llama sampling_settings\n  cargo tool cuda cargo clippy --all-targets --all-features -- -D warnings\n  cargo tool cuda cargo --dry-run build --release --features cuda\n\nSets CUDA_PATH, CUDA_HOME, NVCC, CUDARC_CUDA_VERSION (from nvcc), CUDA_COMPUTE_CAP\n(from nvidia-smi unless already set), CUDAFORGE_THREADS (every thread, unless already\nset, so nvcc kernel builds use all cores) and, on Windows, NVCC_CCBIN (MSVC's cl.exe folder)\nand NVCC_APPEND_FLAGS, and puts the toolkit's bin folders first on PATH. Before running\nCargo it copies the toolkit's runtime DLLs into the profile folder and its deps folder of\nthe target directory, skipping files already there at the same size."
)]
pub(crate) struct Args {
    /// Print the settings and command without running Cargo or copying files.
    #[arg(long)]
    dry_run: bool,
    /// Cargo's arguments, run in the repository root.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    cargo_args: Vec<OsString>,
}

pub(super) fn run(args: &Args, root: &Path) -> io::Result<()> {
    let toolkit = root.join(TOOLKIT);
    let nvcc = toolkit.join("bin").join(executable("nvcc"));
    if !nvcc.is_file() {
        return Err(io::Error::other(format!(
            "no CUDA toolkit: {} is missing (see README \"AI on the GPU\")",
            nvcc.display()
        )));
    }
    let settings = settings(&toolkit, &nvcc)?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut output = io::stdout().lock();
    for (name, value) in settings.iter().filter(|(name, _)| *name != "PATH") {
        writeln!(output, "{name}={}", value.to_string_lossy())?;
    }
    let bin = toolkit.join("bin");
    writeln!(output, "PATH starts with {} and its x64", bin.display())?;
    let profile = profile(&args.cargo_args);
    let dlls = dlls(&toolkit)?;
    if !dlls.is_empty() {
        let folder = target_dir(&cargo, root)?.join(&profile);
        writeln!(output, "runtime DLLs -> {}", folder.display())?;
        if !args.dry_run {
            copy_dlls(&dlls, &folder)?;
        }
    }
    writeln!(output, "cargo {}", join(&args.cargo_args))?;
    output.flush()?;
    if args.dry_run {
        return Ok(());
    }
    let mut command = Command::new(&cargo);
    command
        .current_dir(root)
        .args(&args.cargo_args)
        .envs(settings);
    let status = command
        .status()
        .map_err(|error| io::Error::new(error.kind(), format!("could not start cargo: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("cargo failed: {status}")))
    }
}

fn executable(name: &str) -> String {
    format!("{name}{}", env::consts::EXE_SUFFIX)
}

/// The variables Cargo and the build scripts get.
fn settings(toolkit: &Path, nvcc: &Path) -> io::Result<Vec<(&'static str, OsString)>> {
    let version = output(Command::new(nvcc).arg("--version"))?;
    let version = cudarc_version(&version).ok_or_else(|| {
        io::Error::other(format!(
            "cannot read the CUDA release from {}",
            nvcc.display()
        ))
    })?;
    let compute_cap = match env::var_os("CUDA_COMPUTE_CAP") {
        Some(set) => set,
        None => compute_cap()?.into(),
    };
    // cudaforge gives each kernel-compiling build script half the threads by default, which
    // leaves the rest idle once that script is all that remains to build.
    let threads = match env::var_os("CUDAFORGE_THREADS") {
        Some(set) => set,
        None => thread::available_parallelism()
            .map_or(1, NonZero::get)
            .to_string()
            .into(),
    };
    let bin = toolkit.join("bin");
    let path = env::var_os("PATH").unwrap_or_default();
    let path = env::join_paths(
        [bin.clone(), bin.join("x64")]
            .into_iter()
            .chain(env::split_paths(&path)),
    )
    .map_err(io::Error::other)?;
    let mut settings = vec![
        ("CUDA_PATH", toolkit.as_os_str().to_owned()),
        ("CUDA_HOME", toolkit.as_os_str().to_owned()),
        ("NVCC", nvcc.as_os_str().to_owned()),
        ("CUDARC_CUDA_VERSION", version.into()),
        ("CUDA_COMPUTE_CAP", compute_cap),
        ("CUDAFORGE_THREADS", threads),
        ("PATH", path),
    ];
    settings.extend(msvc_settings()?);
    Ok(settings)
}

/// nvcc needs MSVC's compiler, its conforming preprocessor for CUDA 13's headers, and `/MD`
/// to match Skia's C runtime. `<string>` is included in every kernel because mistral.rs's
/// Marlin kernels use `std::to_string` without it, which MSVC's headers do not bring in.
#[cfg(windows)]
fn msvc_settings() -> io::Result<Vec<(&'static str, OsString)>> {
    let cl = find_msvc_tools::find_tool("x86_64", "cl.exe")
        .ok_or_else(|| io::Error::other("cannot find MSVC's cl.exe (Visual Studio C++ tools)"))?;
    let folder = cl
        .path()
        .parent()
        .ok_or_else(|| io::Error::other("cl.exe has no folder"))?;
    Ok(vec![
        ("NVCC_CCBIN", folder.as_os_str().to_owned()),
        (
            "NVCC_APPEND_FLAGS",
            "-Xcompiler /Zc:preprocessor -Xcompiler /MD -include string".into(),
        ),
    ])
}

#[cfg(not(windows))]
fn msvc_settings() -> io::Result<Vec<(&'static str, OsString)>> {
    Ok(Vec::new())
}

fn compute_cap() -> io::Result<String> {
    let answer = output(
        Command::new("nvidia-smi").args(["--query-gpu=compute_cap", "--format=csv,noheader"]),
    )?;
    compute_cap_from(&answer).ok_or_else(|| {
        io::Error::other(format!(
            "cannot read a compute capability from nvidia-smi ({answer:?}); set CUDA_COMPUTE_CAP"
        ))
    })
}

/// `CUDARC_CUDA_VERSION` from `nvcc --version`: "release 13.2," gives 13020.
fn cudarc_version(nvcc: &str) -> Option<String> {
    let release = nvcc.split("release ").nth(1)?.split(',').next()?;
    let (major, minor) = release.trim().split_once('.')?;
    let (major, minor): (u32, u32) = (major.parse().ok()?, minor.parse().ok()?);
    Some(format!("{}", major * 1000 + minor * 10))
}

/// `CUDA_COMPUTE_CAP` from nvidia-smi's first GPU: "12.0" gives 120.
fn compute_cap_from(answer: &str) -> Option<String> {
    let first = answer.lines().next()?.trim();
    let (major, minor) = first.split_once('.')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits(major) && digits(minor)).then(|| format!("{major}{minor}"))
}

/// The target directory's folder for the profile the arguments pick: `--release`, or
/// `--profile <name>` (where `dev` builds into `debug`), else `debug`.
fn profile(args: &[OsString]) -> String {
    // Arguments after `--` belong to the program, not Cargo.
    let words: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .take_while(|w| w != "--")
        .collect();
    // The last one given wins, as in Cargo.
    let named = words.iter().enumerate().rev().find_map(|(i, word)| {
        if word == "--release" {
            Some("release")
        } else if word == "--profile" {
            words.get(i + 1).map(String::as_str)
        } else {
            word.strip_prefix("--profile=")
        }
    });
    match named {
        None | Some("dev") => "debug".to_owned(),
        Some(name) => name.to_owned(),
    }
}

fn output(command: &mut Command) -> io::Result<String> {
    let result = command.output().map_err(|error| {
        io::Error::new(error.kind(), format!("could not run {command:?}: {error}"))
    })?;
    if !result.status.success() {
        return Err(io::Error::other(format!(
            "{command:?} failed: {}",
            result.status
        )));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

fn target_dir(cargo: &OsStr, root: &Path) -> io::Result<PathBuf> {
    let metadata = output(Command::new(cargo).current_dir(root).args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
    ]))?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata).map_err(io::Error::other)?;
    metadata
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("cargo metadata gave no target_directory"))
}

/// The toolkit's runtime DLLs (Windows keeps them in `bin\x64`); none elsewhere.
fn dlls(toolkit: &Path) -> io::Result<Vec<PathBuf>> {
    let folder = toolkit.join("bin").join("x64");
    if !folder.is_dir() {
        return Ok(Vec::new());
    }
    let mut dlls = Vec::new();
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dll"))
        {
            dlls.push(path);
        }
    }
    dlls.sort();
    Ok(dlls)
}

/// Copies `dlls` into `folder` and its `deps`, where test binaries run.
fn copy_dlls(dlls: &[PathBuf], folder: &Path) -> io::Result<()> {
    for destination in [folder.to_owned(), folder.join("deps")] {
        fs::create_dir_all(&destination)?;
        for dll in dlls {
            let Some(name) = dll.file_name() else {
                continue;
            };
            let target = destination.join(name);
            let size = |path: &Path| fs::metadata(path).map(|m| m.len()).ok();
            if size(&target) != size(dll) {
                fs::copy(dll, &target)?;
            }
        }
    }
    Ok(())
}

fn join(args: &[OsString]) -> String {
    args.iter()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_cudarc_version_comes_from_nvccs_release() {
        let nvcc = "nvcc: NVIDIA (R) Cuda compiler driver\nCuda compilation tools, release 13.2, V13.2.86\n";
        assert_eq!(cudarc_version(nvcc).as_deref(), Some("13020"));
        assert_eq!(
            cudarc_version("release 12.8, V12.8.93").as_deref(),
            Some("12080")
        );
        assert_eq!(cudarc_version("no release here"), None);
    }

    #[test]
    fn the_compute_capability_is_the_first_gpus_without_its_dot() {
        assert_eq!(compute_cap_from("12.0\n8.9\n").as_deref(), Some("120"));
        assert_eq!(compute_cap_from("8.6").as_deref(), Some("86"));
        assert_eq!(compute_cap_from("[N/A]"), None);
        assert_eq!(compute_cap_from(""), None);
    }

    #[test]
    fn the_profile_folder_follows_cargos_profile_arguments() {
        assert_eq!(profile(&args(&["build", "--features", "cuda"])), "debug");
        assert_eq!(
            profile(&args(&["test", "--release", "--", "--ignored"])),
            "release"
        );
        assert_eq!(profile(&args(&["build", "--profile", "dev"])), "debug");
        assert_eq!(profile(&args(&["build", "--profile=bench"])), "bench");
        // Arguments after `--` belong to the program, not Cargo.
        assert_eq!(profile(&args(&["run", "--", "--release"])), "debug");
    }

    #[test]
    fn dlls_are_copied_beside_the_build_and_its_tests_once() {
        let dir = env::temp_dir().join(format!("tools-cuda-dlls-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        let toolkit = dir.join("toolkit");
        fs::create_dir_all(toolkit.join("bin").join("x64")).unwrap();
        fs::write(
            toolkit.join("bin").join("x64").join("cudart64_13.dll"),
            "dll",
        )
        .unwrap();
        fs::write(
            toolkit.join("bin").join("x64").join("readme.txt"),
            "not a dll",
        )
        .unwrap();
        let found = dlls(&toolkit).unwrap();
        assert_eq!(found.len(), 1);
        let release = dir.join("target").join("release");
        copy_dlls(&found, &release).unwrap();
        for folder in [release.clone(), release.join("deps")] {
            assert_eq!(
                fs::read_to_string(folder.join("cudart64_13.dll")).unwrap(),
                "dll"
            );
        }
        assert_eq!(dlls(&dir.join("absent")).unwrap(), Vec::<PathBuf>::new());
        fs::remove_dir_all(&dir).ok();
    }
}
