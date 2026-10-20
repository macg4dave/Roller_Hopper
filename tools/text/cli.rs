//! Text-file inspection and editing.

#[path = "edit.rs"]
mod edit;
#[path = "normalize_eol.rs"]
mod normalize_eol;
#[path = "replace_text.rs"]
mod replace_text;

use std::{io, path::Path};

#[derive(Debug, clap::Subcommand)]
pub(super) enum Command {
    /// Preview or apply CRLF-to-LF conversion in changed files or explicit paths.
    NormalizeEol(normalize_eol::Args),
    /// Preview or apply an exact UTF-8 text replacement with a required match count.
    ReplaceText(replace_text::Args),
}

pub(super) fn run(command: Command, root: &Path) -> io::Result<()> {
    match command {
        Command::NormalizeEol(args) => normalize_eol::run(&args, root),
        Command::ReplaceText(args) => replace_text::run(&args, root),
    }
}
