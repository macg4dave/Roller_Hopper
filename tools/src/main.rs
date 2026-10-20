//! Development commands, isolated from the application and its dependencies.

mod icons;
mod repo;

use std::io;
use std::path::Path;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(about = "Development tools for Network Mapper")]
struct Cli {
    #[command(subcommand)]
    area: Area,
}

#[derive(Debug, Subcommand)]
enum Area {
    /// Run the repository's required development checks.
    Repo {
        #[command(subcommand)]
        command: repo::Command,
    },
    /// Inspect the runtime icon library without changing it.
    Icons {
        #[command(subcommand)]
        command: icons::Command,
    },
}

fn main() -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| io::Error::other("tooling crate has no repository parent"))?;
    match Cli::parse().area {
        Area::Repo { command } => repo::run(command, root),
        Area::Icons { command } => icons::run(command, root),
    }
}
