//! Shared file scope, preflight and preview behaviour for text edits.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, clap::Args)]
pub(super) struct Options {
    /// Repository root (defaults to this tool's repository); file paths are relative to it.
    #[arg(long)]
    root: Option<PathBuf>,
    /// Write changes. Without this flag, only print a preview.
    #[arg(long)]
    apply: bool,
}

impl Options {
    pub(super) fn root(&self, default: &Path) -> io::Result<PathBuf> {
        self.root.as_deref().unwrap_or(default).canonicalize()
    }
}

pub(super) struct Change {
    pub(super) path: PathBuf,
    pub(super) before: Vec<u8>,
    pub(super) after: Vec<u8>,
}

/// Resolve, deduplicate and validate every target before planning any edits.
pub(super) fn targets(root: &Path, paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    paths
        .iter()
        .map(|path| {
            let joined = root.join(path);
            let resolved = joined.canonicalize()?;
            if !resolved.starts_with(root)
                || !resolved.is_file()
                || fs::symlink_metadata(&joined)?.file_type().is_symlink()
            {
                return Err(io::Error::other(format!(
                    "target must be a regular file inside {}: {}",
                    root.display(),
                    joined.display()
                )));
            }
            Ok(resolved)
        })
        .collect::<io::Result<BTreeSet<_>>>()
        .map(|paths| paths.into_iter().collect())
}

pub(super) fn read(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}

pub(super) fn text(bytes: &[u8]) -> Option<&str> {
    if bytes.contains(&0) {
        return None;
    }
    // Invalid UTF-8 is a non-text result: normalisation skips it, replacement reports an error.
    std::str::from_utf8(bytes).ok()
}

pub(super) fn read_text(path: &Path) -> io::Result<String> {
    let bytes = read(path)?;
    text(&bytes).map(str::to_owned).ok_or_else(|| {
        io::Error::other(format!(
            "expected UTF-8 text without NUL bytes: {}",
            path.display()
        ))
    })
}

pub(super) fn finish(changes: &[Change], options: &Options) -> io::Result<()> {
    let mut output = io::stdout().lock();
    for change in changes {
        writeln!(
            output,
            "{}: {} -> {} bytes",
            change.path.display(),
            change.before.len(),
            change.after.len()
        )?;
    }
    writeln!(
        output,
        "{} file(s) {};{}",
        changes.len(),
        if options.apply {
            "to update"
        } else {
            "would change"
        },
        if options.apply {
            ""
        } else {
            " use --apply to write"
        }
    )?;
    output.flush()?;
    if options.apply {
        // Check the entire plan again before the first write, including concurrent edits.
        for change in changes {
            if read(&change.path)? != change.before {
                return Err(io::Error::other(format!(
                    "file changed during preflight: {}",
                    change.path.display()
                )));
            }
        }
        for change in changes {
            fs::write(&change.path, &change.after)?;
        }
        writeln!(output, "Updated {} file(s).", changes.len())?;
    }
    Ok(())
}
