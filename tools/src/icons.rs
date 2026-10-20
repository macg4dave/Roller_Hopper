//! Runtime icon-library maintenance.

mod audit;

use std::{io, path::Path};

#[derive(Debug, clap::Subcommand)]
pub(super) enum Command {
    /// Find SVGs hidden by folder depth or conflicting icon IDs; does not validate SVG content.
    Audit(audit::Args),
}

pub(super) fn run(command: Command, root: &Path) -> io::Result<()> {
    match command {
        Command::Audit(args) => audit::run(&args, root),
    }
}
