//! Literal multiline replacements with full match-count preflight.

use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use super::edit::{self, Change, Options};

#[derive(Debug, clap::Args)]
#[command(
    after_help = "Examples:\n  cargo tool text replace-text --old-file before.txt --new-file after.txt -- src/model/layout.rs\n  cargo tool text replace-text --old-file before.txt --new-file after.txt --expect 2 --apply -- src/a.rs src/b.rs\n\nPut the exact old and new blocks in UTF-8 files; multiline text needs no shell escaping.\nMatching is literal and non-overlapping, including whitespace, line endings and final newlines.\nEach target must contain exactly --expect matches before any file is written.\nAn empty old block is rejected; an empty new block deletes the matched text.\nPreview is the default; use --apply to write. This does not run tests or format the result."
)]
pub(crate) struct Args {
    #[command(flatten)]
    options: Options,
    /// UTF-8 file containing the exact text to find (relative to --root, or absolute).
    #[arg(long)]
    old_file: PathBuf,
    /// UTF-8 file containing the replacement (relative to --root, or absolute).
    #[arg(long)]
    new_file: PathBuf,
    /// Required non-overlapping matches in EACH target file; zero is not allowed.
    #[arg(long, default_value = "1")]
    expect: NonZeroUsize,
    /// Target files, relative to --root; all are checked before any writes.
    #[arg(required = true, value_name = "FILE")]
    files: Vec<PathBuf>,
}

pub(super) fn run(args: &Args, default_root: &Path) -> io::Result<()> {
    let root = args.options.root(default_root)?;
    let old = edit::read_text(&root.join(&args.old_file))?;
    let new = edit::read_text(&root.join(&args.new_file))?;
    if old.is_empty() {
        return Err(io::Error::other("old text must not be empty"));
    }
    let mut changes = Vec::new();
    for path in edit::targets(&root, &args.files)? {
        let before = edit::read(&path)?;
        let text = edit::text(&before)
            .ok_or_else(|| io::Error::other(format!("not UTF-8 text: {}", path.display())))?;
        let matches = text.matches(&old).count();
        if matches != args.expect.get() {
            return Err(io::Error::other(format!(
                "{}: expected {} matches, found {matches}; no files written",
                path.display(),
                args.expect
            )));
        }
        let after = text.replace(&old, &new).into_bytes();
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
