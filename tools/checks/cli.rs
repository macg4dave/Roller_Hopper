//! Development quality checks.

#[path = "agent_command.rs"]
mod agent_command;
#[path = "gate.rs"]
mod gate;

use std::{io, path::Path};

#[derive(Debug, clap::Subcommand)]
pub(super) enum Command {
    /// Run fmt, clippy, tests and build in order; stop on the first failure.
    Gate(gate::Args),
    /// Check a Claude `PreToolUse` event from stdin without executing its command.
    #[command(
        after_help = "Examples:\n  cargo tool checks agent-command < hook-event.json\n  Get-Content -Raw hook-event.json | cargo tool checks agent-command"
    )]
    AgentCommand,
}

pub(super) fn run(command: Command, root: &Path) -> io::Result<()> {
    match command {
        Command::Gate(args) => gate::run(&args, root),
        Command::AgentCommand => agent_command::run(),
    }
}
