//! Device-identification model evaluation: run a model over held-out cases, then score it.

#[path = "cases.rs"]
mod cases;
#[cfg(feature = "ai")]
#[path = "../../src/ai/chat.rs"]
mod chat;
#[cfg(feature = "ai")]
#[path = "../../src/ai/engine.rs"]
mod engine;
#[cfg(feature = "ai")]
#[path = "../../src/ai/response.rs"]
mod response;
#[cfg(feature = "ai")]
#[path = "run.rs"]
mod run;
#[path = "score.rs"]
mod score;
#[cfg(feature = "ai")]
#[path = "../../src/ai/weights.rs"]
mod weights;

use std::{io, path::Path};

#[derive(Debug, clap::Subcommand)]
pub(super) enum Command {
    /// Ask a local GGUF model about every case and record its answers (needs `--features ai`).
    #[cfg(feature = "ai")]
    Run(run::Args),
    /// Score recorded answers against the cases' expectations; needs no model.
    Score(score::Args),
}

pub(super) fn run(command: Command, root: &Path) -> io::Result<()> {
    match command {
        #[cfg(feature = "ai")]
        Command::Run(args) => run::run(&args, root),
        Command::Score(args) => score::run(&args, root),
    }
}
