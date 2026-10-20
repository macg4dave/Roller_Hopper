//! Exercise preview, write preflight and Git selection in temporary repositories.

use std::fs;
use std::io;
use std::process::Command;

use super::{Fixture, tool};

fn edit(fixture: &Fixture, command: &str) -> Command {
    let mut process = tool();
    process.args(["text", command, "--root"]).arg(&fixture.0);
    process
}

fn git(fixture: &Fixture, arguments: &[&str]) -> io::Result<()> {
    let output = Command::new("git")
        .current_dir(&fixture.0)
        .args(["-c", "core.autocrlf=false"])
        .args(arguments)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn normalize_previews_and_applies_git_changes_without_touching_binary_or_ignored_files()
-> io::Result<()> {
    let fixture = Fixture::new()?;
    git(&fixture, &["init", "--quiet"])?;
    fs::write(fixture.0.join("tracked file.txt"), b"before\n")?;
    fs::write(fixture.0.join("deleted.txt"), b"gone\n")?;
    git(&fixture, &["add", "."])?;
    fs::remove_file(fixture.0.join("deleted.txt"))?;
    fs::write(fixture.0.join("tracked file.txt"), b"a\r\nb\rc")?;
    fs::write(fixture.0.join("new file.txt"), b"new\r\n")?;
    fs::write(fixture.0.join("binary.bin"), b"\0\r\n")?;
    fs::write(fixture.0.join(".gitignore"), b"ignored.txt\n")?;
    fs::write(fixture.0.join("ignored.txt"), b"ignored\r\n")?;
    let preview = edit(&fixture, "normalize-eol").output()?;
    assert!(preview.status.success(), "{preview:?}");
    assert_eq!(fs::read(fixture.0.join("tracked file.txt"))?, b"a\r\nb\rc");
    assert!(String::from_utf8_lossy(&preview.stdout).contains("2 file(s) would change"));
    let applied = edit(&fixture, "normalize-eol").arg("--apply").output()?;
    assert!(applied.status.success(), "{applied:?}");
    assert_eq!(fs::read(fixture.0.join("tracked file.txt"))?, b"a\nb\rc");
    assert_eq!(fs::read(fixture.0.join("new file.txt"))?, b"new\n");
    assert_eq!(fs::read(fixture.0.join("binary.bin"))?, b"\0\r\n");
    assert_eq!(fs::read(fixture.0.join("ignored.txt"))?, b"ignored\r\n");
    Ok(())
}

#[test]
fn explicit_normalize_deduplicates_paths_and_is_idempotent() -> io::Result<()> {
    let fixture = Fixture::new()?;
    fs::write(fixture.0.join("one.txt"), b"a\r\n")?;
    let applied = edit(&fixture, "normalize-eol")
        .args(["--apply", "one.txt", "one.txt"])
        .output()?;
    assert!(applied.status.success());
    assert!(String::from_utf8_lossy(&applied.stdout).contains("Updated 1 file(s)"));
    let preview = edit(&fixture, "normalize-eol").arg("one.txt").output()?;
    assert!(preview.status.success());
    assert!(String::from_utf8_lossy(&preview.stdout).contains("0 file(s) would change"));
    Ok(())
}

fn replacement(fixture: &Fixture) -> io::Result<Command> {
    fs::write(fixture.0.join("old.txt"), "old\nblock")?;
    fs::write(fixture.0.join("new.txt"), "new\ntext")?;
    let mut command = edit(fixture, "replace-text");
    command.args(["--old-file", "old.txt", "--new-file", "new.txt"]);
    Ok(command)
}

#[test]
fn replacement_supports_multiline_unicode_preview_and_explicit_match_count() -> io::Result<()> {
    let fixture = Fixture::new()?;
    let original = "★old\nblock\r\nold\nblock!";
    fs::write(fixture.0.join("target.txt"), original)?;
    let output = replacement(&fixture)?
        .args(["--expect", "2", "target.txt"])
        .output()?;
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(fixture.0.join("target.txt"))?, original);
    let applied = replacement(&fixture)?
        .args(["--expect", "2", "--apply", "target.txt"])
        .output()?;
    assert!(applied.status.success(), "{applied:?}");
    assert_eq!(
        fs::read_to_string(fixture.0.join("target.txt"))?,
        "★new\ntext\r\nnew\ntext!"
    );
    Ok(())
}

#[test]
fn replacement_preflights_all_targets_before_writing() -> io::Result<()> {
    let fixture = Fixture::new()?;
    fs::write(fixture.0.join("a.txt"), "old\nblock")?;
    fs::write(fixture.0.join("z.txt"), "different")?;
    let output = replacement(&fixture)?
        .args(["--apply", "a.txt", "z.txt"])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expected 1 matches, found 0"));
    assert_eq!(fs::read_to_string(fixture.0.join("a.txt"))?, "old\nblock");
    Ok(())
}

#[test]
fn replacement_rejects_empty_search_and_supports_deletion() -> io::Result<()> {
    let fixture = Fixture::new()?;
    fs::write(fixture.0.join("target.txt"), "old\nblock!")?;
    let mut empty_search = replacement(&fixture)?;
    fs::write(fixture.0.join("old.txt"), "")?;
    let rejected = empty_search.args(["--apply", "target.txt"]).output()?;
    assert!(!rejected.status.success());
    let mut deletion = replacement(&fixture)?;
    fs::write(fixture.0.join("new.txt"), "")?;
    assert!(
        deletion
            .args(["--apply", "target.txt"])
            .output()?
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(fixture.0.join("target.txt"))?, "!");
    Ok(())
}

#[test]
fn edits_reject_targets_outside_the_selected_root() -> io::Result<()> {
    let fixture = Fixture::new()?;
    let outside = Fixture::new()?;
    outside.file("outside.txt")?;
    let output = edit(&fixture, "normalize-eol")
        .arg("--apply")
        .arg(outside.0.join("outside.txt"))
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("inside"));
    Ok(())
}

#[test]
fn edit_help_includes_examples_and_invalid_match_counts_fail() -> io::Result<()> {
    for command in ["normalize-eol", "replace-text"] {
        let output = tool().args(["text", command, "--help"]).output()?;
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Examples:"));
    }
    let fixture = Fixture::new()?;
    let output = replacement(&fixture)?
        .args(["--expect", "0", "target.txt"])
        .output()?;
    assert!(!output.status.success());
    Ok(())
}
