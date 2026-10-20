//! Exercise the public CLI using isolated fixtures, never the live icon library.

#[path = "cli/edits.rs"]
mod edits;

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> io::Result<Self> {
        let directory = std::env::temp_dir().join(format!(
            "network-mapper-tools-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        // Never reuse or delete an existing directory belonging to another run.
        fs::create_dir(&directory)?;
        Ok(Self(directory))
    }

    fn file(&self, name: &str) -> io::Result<()> {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, "fixture: audit checks paths, not SVG contents")
    }

    fn audit(&self) -> io::Result<Output> {
        tool()
            .args(["icons", "audit", "--directory"])
            .arg(&self.0)
            .output()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            // Drop cannot propagate errors; make cleanup failures visible to the test runner.
            writeln!(
                io::stderr().lock(),
                "could not remove {}: {error}",
                self.0.display()
            )
            .unwrap_or_else(|_| {
                // Cleanup diagnostics are best effort if stderr itself is unavailable.
            });
        }
    }
}

fn tool() -> Command {
    Command::new(env!("CARGO_BIN_EXE_network-mapper-tools"))
}

#[test]
fn gate_plan_keeps_required_order_and_scope() -> io::Result<()> {
    for scope in ["all", "tools", "app"] {
        let output = tool()
            .args(["checks", "gate", "--scope", scope, "--dry-run"])
            .output()?;
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let commands: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.split_once("] cargo "))
            .map(|(_, command)| command)
            .collect();
        let expected = [
            "fmt --all --check",
            "clippy --all-targets --all-features -- -D warnings",
            "test --all-targets",
            "build",
        ];
        assert_eq!(
            commands,
            expected.repeat(if scope == "all" { 2 } else { 1 })
        );
        let tools_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let tools_label = format!("[{}]", tools_manifest.display());
        assert_eq!(stdout.contains(&tools_label), scope != "app");
    }
    Ok(())
}

#[test]
fn invalid_arguments_fail_with_help() -> io::Result<()> {
    let output = tool()
        .args(["checks", "gate", "--scope", "invalid"])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid"));
    for arguments in [
        vec!["--help"],
        vec!["checks", "--help"],
        vec!["checks", "gate", "--help"],
        vec!["text", "--help"],
        vec!["text", "normalize-eol", "--help"],
        vec!["text", "replace-text", "--help"],
        vec!["icons", "--help"],
        vec!["icons", "audit", "--help"],
    ] {
        let help = tool().args(&arguments).output()?;
        assert!(help.status.success());
        if arguments.len() == 3 {
            assert!(String::from_utf8_lossy(&help.stdout).contains("Examples:"));
        }
    }
    Ok(())
}

#[test]
fn audit_accepts_groups_and_mixed_case_extensions() -> io::Result<()> {
    let fixture = Fixture::new()?;
    for name in ["Router.svg", "Network/Router.SVG", "Network/readme.txt"] {
        fixture.file(name)?;
    }
    let output = fixture.audit()?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("2 picker SVG files; 0 issues"));
    Ok(())
}

#[test]
fn audit_reports_hidden_icons() -> io::Result<()> {
    let fixture = Fixture::new()?;
    for name in ["Router.svg", "Network/deeper/Hidden.svg"] {
        fixture.file(name)?;
    }
    let output = fixture.audit()?;
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hidden below picker depth:"));
    assert!(stdout.contains("1 issues"));
    Ok(())
}

#[test]
fn audit_rejects_a_missing_directory() -> io::Result<()> {
    let fixture = Fixture::new()?;
    let output = tool()
        .args(["icons", "audit", "--directory"])
        .arg(fixture.0.join("missing"))
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a directory"));
    Ok(())
}
