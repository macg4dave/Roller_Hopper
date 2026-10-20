//! Building and testing the app's `cuda` feature with the repository's CUDA toolkit.

#[path = "cargo.rs"]
mod cargo;

use std::{io, path::Path};

#[derive(Debug, clap::Subcommand)]
pub(super) enum Command {
    /// Run Cargo with the CUDA build settings and the toolkit's runtime DLLs beside the build.
    Cargo(cargo::Args),
}

pub(super) fn run(command: Command, root: &Path) -> io::Result<()> {
    match command {
        Command::Cargo(args) => cargo::run(&args, root),
    }
}
