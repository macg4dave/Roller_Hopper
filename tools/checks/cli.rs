//! Development quality checks.

#[path = "gate.rs"]
mod gate;

use std::{io, path::Path};

#[derive(Debug, clap::Subcommand)]
pub(super) enum Command {
    /// Run fmt, clippy, tests and build in order; stop on the first failure.
    Gate(gate::Args),
}

pub(super) fn run(command: Command, root: &Path) -> io::Result<()> {
    match command {
        Command::Gate(args) => gate::run(&args, root),
    }
}
