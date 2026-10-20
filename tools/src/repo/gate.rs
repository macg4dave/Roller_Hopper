//! One shared plan for the application's and standalone tools' required gates.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// Select which crate to check (all checks tools first, then the app).
    #[arg(long, value_enum, default_value_t = Scope::All)]
    scope: Scope,
    /// Print the commands without running Cargo or changing files.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Scope {
    All,
    App,
    Tools,
}

const STEPS: &[&[&str]] = &[
    &["fmt", "--all", "--check"],
    &[
        "clippy",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ],
    &["test", "--all-targets"],
    &["build"],
];

fn directories(scope: Scope, root: &Path) -> Vec<PathBuf> {
    match scope {
        Scope::All => vec![root.join("tools"), root.to_owned()],
        Scope::App => vec![root.to_owned()],
        Scope::Tools => vec![root.join("tools")],
    }
}

pub(super) fn run(args: &Args, root: &Path) -> io::Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut output = io::stdout().lock();
    for directory in directories(args.scope, root) {
        for step in STEPS {
            writeln!(output, "[{}] cargo {}", directory.display(), step.join(" "))?;
            output.flush()?;
            if !args.dry_run {
                execute(Command::new(&cargo).current_dir(&directory).args(*step))?;
            }
        }
    }
    Ok(())
}

fn execute(command: &mut Command) -> io::Result<()> {
    let status = command.status().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("could not start {command:?}: {error}"),
        )
    })?;
    if !status.success() {
        return Err(io::Error::other(format!("{command:?} failed: {status}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_executable_is_a_contextual_error() {
        let error =
            execute(&mut Command::new("network-mapper-nonexistent-executable")).unwrap_err();
        assert!(error.to_string().contains("could not start"));
    }

    #[test]
    fn unsuccessful_child_is_an_error() -> io::Result<()> {
        let error = execute(
            Command::new(std::env::current_exe()?)
                .arg("--network-mapper-invalid-test-option")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null()),
        )
        .unwrap_err();
        assert!(error.to_string().contains("failed:"));
        Ok(())
    }
}
