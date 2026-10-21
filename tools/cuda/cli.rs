//! Explicit Candle CUDA builds.

use std::io;
use std::path::Path;
use std::process::Command as Process;

#[derive(Clone, Copy, Debug, clap::Subcommand)]
pub(super) enum Command {
    /// Build the app with Candle's CUDA backend.
    Build,
}

pub(super) fn run(_command: Command, root: &Path) -> io::Result<()> {
    let mut cargo = Process::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cargo
        .current_dir(root)
        .args(["build", "--locked", "--features", "cuda"]);
    if std::env::var_os("CARGO_BUILD_JOBS").is_none() {
        cargo.env(
            "CARGO_BUILD_JOBS",
            std::thread::available_parallelism()?.to_string(),
        );
    }
    let status = cargo.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("CUDA build failed with {status}")))
    }
}
