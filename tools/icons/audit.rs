//! Audit the picker's root/subfolder and extension-free identifier conventions.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Debug, clap::Args)]
#[command(
    after_help = "Examples:\n  cargo tool icons audit\n  cargo tool icons audit --directory path/to/icons\n\nRead-only: reports hidden SVG paths and case-insensitive icon ID conflicts.\nSVG content and rendering are not validated. Issues produce a non-zero exit code."
)]
pub(crate) struct Args {
    /// Icon folder (defaults to the repository's icons/; relative paths use the current directory).
    #[arg(long)]
    directory: Option<PathBuf>,
}

pub(super) fn run(args: &Args, root: &Path) -> io::Result<()> {
    let default_directory = root.join("icons");
    let directory = args.directory.as_deref().unwrap_or(&default_directory);
    if !directory.is_dir() {
        return Err(io::Error::other(format!(
            "icon directory does not exist or is not a directory: {}",
            directory.display()
        )));
    }
    let mut ids = BTreeMap::new();
    let mut visible = 0;
    let mut issues = 0;
    let mut output = io::stdout().lock();
    for entry in WalkDir::new(directory).sort_by_file_name() {
        let entry = entry.map_err(io::Error::other)?;
        let path = entry.path();
        if !entry.file_type().is_file()
            || !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        {
            continue;
        }
        if entry.depth() > 2 {
            writeln!(output, "Hidden below picker depth: {}", path.display())?;
            issues += 1;
            continue;
        }
        let relative = path.strip_prefix(directory).map_err(io::Error::other)?;
        let id = relative.with_extension("");
        let Some(id) = id.to_str() else {
            writeln!(output, "Non-UTF-8 icon ID: {}", path.display())?;
            issues += 1;
            continue;
        };
        // IDs retain their group: Router and Network/Router are distinct choices.
        if let Some(previous) = record_id(&mut ids, id, relative) {
            writeln!(
                output,
                "Conflicting IDs: {} and {}",
                previous.display(),
                relative.display()
            )?;
            issues += 1;
        }
        visible += 1;
    }
    writeln!(output, "{visible} picker SVG files; {issues} issues.")?;
    if issues > 0 {
        return Err(io::Error::other("icon audit failed; see reported paths"));
    }
    Ok(())
}

fn record_id(ids: &mut BTreeMap<String, PathBuf>, id: &str, path: &Path) -> Option<PathBuf> {
    ids.insert(id.to_lowercase(), path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_conflict_ignoring_case_but_keep_their_group() {
        // Case-only filename pairs cannot coexist on the default Windows filesystem.
        let mut ids = BTreeMap::new();
        assert_eq!(record_id(&mut ids, "Router", Path::new("Router.svg")), None);
        assert_eq!(
            record_id(&mut ids, "Network/Router", Path::new("Network/Router.svg")),
            None
        );
        assert_eq!(
            record_id(&mut ids, "router", Path::new("router.SVG")),
            Some(PathBuf::from("Router.svg"))
        );
    }
}
