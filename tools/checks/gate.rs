//! One shared plan for the application's and standalone tools' required gates.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, clap::Args)]
#[command(
    after_help = "Examples:\n  cargo tool checks gate\n  cargo tool checks gate --scope tools\n  cargo tool checks gate --scope app --cpu\n  cargo tool checks gate --offline --dry-run\n\nRuns fmt check, strict clippy, tests and build in that order.\nThe default checks tools first, then the CUDA-enabled app, stopping at the first failure.\nUse --cpu on a machine without CUDA. All dependency-resolving steps use Cargo.lock."
)]
pub(crate) struct Args {
    /// Select which crate to check (all checks tools first, then the app).
    #[arg(long, value_enum, default_value_t = Scope::All)]
    scope: Scope,
    /// Print the commands without running Cargo or changing files.
    #[arg(long)]
    dry_run: bool,
    /// Check the app without its default CUDA feature (not the full product gate).
    #[arg(long)]
    cpu: bool,
    /// Require all dependencies to be available in Cargo's local cache.
    #[arg(long)]
    offline: bool,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Scope {
    All,
    App,
    Tools,
}

const STEP_NAMES: &[&str] = &["fmt", "clippy", "test", "build"];

#[derive(Clone, Copy)]
enum Features {
    All,
    Cpu,
}

fn step_args(step: &str, features: Features, offline: bool) -> Vec<&'static str> {
    let mut args = match step {
        "fmt" => vec!["fmt", "--all", "--check"],
        "clippy" => vec!["clippy", "--locked", "--all-targets"],
        "test" => vec!["test", "--locked", "--all-targets"],
        "build" => vec!["build", "--locked"],
        _ => unreachable!("steps are a closed constant list"),
    };
    if step != "fmt" {
        match features {
            Features::All if step == "clippy" => args.push("--all-features"),
            Features::All => {}
            Features::Cpu => args.push("--no-default-features"),
        }
    }
    if offline && step != "fmt" {
        args.insert(1, "--offline");
    }
    if step == "clippy" {
        args.extend(["--", "-D", "warnings"]);
    }
    args
}

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
        let features = if directory == root && args.cpu {
            Features::Cpu
        } else {
            Features::All
        };
        for step in STEP_NAMES {
            let command_args = step_args(step, features, args.offline);
            writeln!(
                output,
                "[{}:{step}] cargo {}",
                directory.display(),
                command_args.join(" ")
            )?;
            output.flush()?;
            if !args.dry_run {
                let started = Instant::now();
                execute(
                    Command::new(&cargo)
                        .current_dir(&directory)
                        .args(&command_args),
                    &directory,
                    step,
                )?;
                writeln!(
                    output,
                    "[{}:{step}] passed in {:.1?}",
                    directory.display(),
                    started.elapsed()
                )?;
            }
        }
    }
    Ok(())
}

fn execute(command: &mut Command, directory: &Path, step: &str) -> io::Result<()> {
    let status = command.status().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("could not start {command:?}: {error}"),
        )
    })?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "{} {step} failed with {status}",
            directory.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_executable_is_a_contextual_error() {
        let error = execute(
            &mut Command::new("network-mapper-nonexistent-executable"),
            Path::new("test-crate"),
            "build",
        )
        .unwrap_err();
        assert!(error.to_string().contains("could not start"));
    }

    #[test]
    fn unsuccessful_child_is_an_error() -> io::Result<()> {
        let error = execute(
            Command::new(std::env::current_exe()?)
                .arg("--network-mapper-invalid-test-option")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null()),
            Path::new("test-crate"),
            "test",
        )
        .unwrap_err();
        assert!(error.to_string().contains("test-crate test failed"));
        Ok(())
    }

    #[test]
    fn dependency_steps_are_locked() {
        for step in ["clippy", "test", "build"] {
            assert!(step_args(step, Features::All, false).contains(&"--locked"));
        }
        assert!(!step_args("fmt", Features::All, false).contains(&"--locked"));
    }

    #[test]
    fn cpu_mode_omits_default_features_from_app_compilation() {
        for step in ["clippy", "test", "build"] {
            assert!(step_args(step, Features::Cpu, false).contains(&"--no-default-features"));
        }
    }

    #[test]
    fn offline_mode_only_changes_dependency_steps() {
        assert!(!step_args("fmt", Features::All, true).contains(&"--offline"));
        assert!(step_args("build", Features::All, true).contains(&"--offline"));
    }
}
