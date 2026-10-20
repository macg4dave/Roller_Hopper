//! Development commands, isolated from the application and its dependencies.

#[path = "../ai/cli.rs"]
mod ai;
#[path = "../checks/cli.rs"]
mod checks;
#[path = "../icons/cli.rs"]
mod icons;
#[path = "../text/cli.rs"]
mod text;

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
    /// Run development quality checks.
    Checks {
        #[command(subcommand)]
        command: checks::Command,
    },
    /// Inspect and edit text files with explicit write control.
    Text {
        #[command(subcommand)]
        command: text::Command,
    },
    /// Inspect the runtime icon library without changing it.
    Icons {
        #[command(subcommand)]
        command: icons::Command,
    },
    /// Evaluate the device-identification model against held-out cases.
    Ai {
        #[command(subcommand)]
        command: ai::Command,
    },
}

fn main() -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| io::Error::other("tooling crate has no repository parent"))?;
    match Cli::parse().area {
        Area::Checks { command } => checks::run(command, root),
        Area::Text { command } => text::run(command, root),
        Area::Icons { command } => icons::run(command, root),
        Area::Ai { command } => ai::run(command, root),
    }
}
