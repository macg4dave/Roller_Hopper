//! Normalise changed text files without treating arbitrary binary data as text.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::edit::{self, Change, Options};

#[derive(Debug, clap::Args)]
#[command(
    after_help = "Examples:\n  cargo tool text normalize-eol\n  cargo tool text normalize-eol --apply\n  cargo tool text normalize-eol --root path/to/repo -- src/main.rs README.md\n\nWith no FILE arguments, reads Git's modified tracked and untracked, non-ignored files.\nDeleted files are skipped. NUL-delimited Git paths preserve spaces and newlines.\nOnly CRLF pairs are changed; lone CR bytes and the final newline are preserved.\nNon-UTF-8 and NUL-containing files are skipped. No files are written without --apply."
)]
pub(crate) struct Args {
    #[command(flatten)]
    options: Options,
    /// Explicit files instead of Git's changed-file selection; directories are not accepted.
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

pub(super) fn run(args: &Args, default_root: &Path) -> io::Result<()> {
    let root = args.options.root(default_root)?;
    let paths = if args.files.is_empty() {
        changed_files(&root)?
    } else {
        args.files.clone()
    };
    let mut changes = Vec::new();
    for path in edit::targets(&root, &paths)? {
        let before = edit::read(&path)?;
        let Some(text) = edit::text(&before) else {
            writeln!(
                io::stdout().lock(),
                "Skipped non-text file: {}",
                path.display()
            )?;
            continue;
        };
        let after = text.replace("\r\n", "\n").into_bytes();
        if before != after {
            changes.push(Change {
                path,
                before,
                after,
            });
        }
    }
    edit::finish(&changes, &args.options)
}

fn changed_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "ls-files",
            "--modified",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| {
            let name = std::str::from_utf8(name).map_err(io::Error::other)?;
            Ok(PathBuf::from(name))
        })
        .collect::<io::Result<Vec<_>>>()
        .map(|paths| {
            paths
                .into_iter()
                .filter(|path| root.join(path).exists())
                .collect()
        })
}
