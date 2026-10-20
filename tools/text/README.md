# Text editing

Preview and apply checked mechanical edits. Use an installed search utility such as
`rg "pattern" src` to find text; these commands handle the edits:

```sh
cargo tool text --help
cargo tool text normalize-eol --help
cargo tool text replace-text --help

# Preview CRLF-to-LF conversion in Git-modified and untracked, non-ignored files.
cargo tool text normalize-eol
# Explicit targets instead of Git selection.
cargo tool text normalize-eol --apply -- tools/README.md

# Put the exact old and new UTF-8 blocks in files, including their intended line endings.
cargo tool text replace-text --old-file before.txt --new-file after.txt -- src/model/layout.rs
cargo tool text replace-text --old-file before.txt --new-file after.txt --expect 2 --apply -- src/a.rs src/b.rs
```

Both commands preview by default; `--apply` writes. File paths are relative to the repository
root, overridden with `--root path/to/repo`. Targets must be files within that root; duplicate
paths are merged and direct symbolic-link targets are rejected.

Normalisation skips deleted Git paths, invalid UTF-8 and files containing NUL bytes. Explicit
targets must exist. Only CRLF pairs are changed; lone CR bytes and the final newline are
preserved. Staged-only changes are not selected automatically; pass those paths explicitly.

Replacement requires UTF-8 text and exactly `--expect` non-overlapping matches in each target
(default: one). Matching includes whitespace, line endings and final newlines. An empty old
block is rejected; an empty new block deletes the match. Multiple replacements are separate
invocations, each with its own old/new block files.

All targets and match counts are checked before the first write, then changed files are
rechecked against the preview input. Writes are not a transaction: an I/O error or a concurrent
edit during writing can leave a partially applied batch. These commands do not format, test,
stage or commit files. Run `cargo tool checks gate` afterwards.

`cli.rs` dispatches commands; `edit.rs` shares scope, preflight and preview handling.
`normalize_eol.rs` and `replace_text.rs` each implement one command.
