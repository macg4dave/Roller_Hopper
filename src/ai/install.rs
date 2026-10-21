//! Downloading a model's files. Each is written beside its final name as `.part`, hashed as it
//! arrives, and renamed into place only when its size and SHA-256 match, so a failed, corrupt
//! or cancelled download never looks installed. A model is installed only once all its files
//! are.

use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::{Model, ModelFile};

/// Why an install stopped. Details are not kept: the user's next step is the same for any
/// failure of one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallError {
    /// No connection, a refused request, or the download broke off.
    Network,
    /// The folder or file could not be written, for example a full disk.
    Disk,
    /// What arrived is not the pinned file.
    Corrupt,
    Cancelled,
}

/// Downloads `model`'s files into its folder in `dir`, skipping those already complete.
/// `progress` gets the bytes done so far across all of them; `cancel` stops it between reads.
pub(crate) fn install(
    model: &Model,
    dir: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), InstallError> {
    let folder = model.path(dir);
    let agent = agent();
    let mut before: u64 = 0;
    for file in model.files {
        if !file.is_complete(&folder) {
            fs::create_dir_all(&folder).map_err(|_| InstallError::Disk)?;
            // The status alone tells the user what to do: try again later.
            let mut response = agent
                .get(&model.url(file))
                .call()
                .map_err(|_| InstallError::Network)?;
            // Not ureq's own body limit: it fails the read after the last byte even when the
            // body is exactly the limit, so every complete download looked broken. save bounds
            // the read itself.
            let body = response.body_mut().as_reader();
            save(body, file, &folder, cancel, &mut |done| progress(before + done))?;
        }
        before += file.size;
        progress(before);
    }
    Ok(())
}

/// HTTPS only, and a stalled connection or server fails rather than waits for ever. The body
/// has no overall time limit: a large model on a slow line may take hours.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .build()
        .into()
}

/// Writes `body` as `file` in `folder`, checking it against the pinned size and hash.
fn save(
    body: impl Read,
    file: &ModelFile,
    folder: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), InstallError> {
    let part = folder.join(format!("{}.part", file.name));
    let saved = copy_checked(body, file, &part, cancel, progress)
        .and_then(|()| fs::rename(&part, folder.join(file.name)).map_err(|_| InstallError::Disk));
    if saved.is_err() {
        // Best effort: a leftover .part is never taken as installed, and the next try
        // overwrites it.
        fs::remove_file(&part).ok();
    }
    saved
}

fn copy_checked(
    body: impl Read,
    file: &ModelFile,
    part: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), InstallError> {
    // One byte past the pinned size is enough to tell a longer file from the pinned one,
    // without reading or storing however much more a server sends.
    let mut body = body.take(file.size.saturating_add(1));
    let mut out = File::create(part).map_err(|_| InstallError::Disk)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    let mut written: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(InstallError::Cancelled);
        }
        let read = match body.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err(InstallError::Network),
        };
        let chunk = buffer.get(..read).ok_or(InstallError::Network)?;
        out.write_all(chunk).map_err(|_| InstallError::Disk)?;
        hasher.update(chunk);
        written += u64::try_from(read).map_err(|_| InstallError::Network)?;
        progress(written);
    }
    out.sync_all().map_err(|_| InstallError::Disk)?;
    if written == file.size && hex(&hasher.finalize()) == file.sha256 {
        Ok(())
    } else {
        Err(InstallError::Corrupt)
    }
}

fn hex(bytes: &[u8]) -> String {
    // Every nibble is below 16, so from_digit never returns None.
    bytes
        .iter()
        .flat_map(|b| [b >> 4, b & 0x0f])
        .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor};
    use std::path::PathBuf;

    use super::super::MODELS;
    use super::*;

    const HELLO: ModelFile = ModelFile {
        name: "hello.bin",
        size: 5,
        sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
    };

    /// An empty folder of the test's own.
    fn folder(test: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("network-mapper-{}-{test}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const NO_FILES: [String; 0] = [];

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_matching_download_is_installed_under_its_name() {
        let dir = folder("matching");
        let mut seen = Vec::new();
        let saved = save(
            Cursor::new("hello"),
            &HELLO,
            &dir,
            &AtomicBool::new(false),
            &mut |n| {
                seen.push(n);
            },
        );
        assert_eq!(saved, Ok(()));
        assert_eq!(fs::read_to_string(dir.join("hello.bin")).unwrap(), "hello");
        assert_eq!(files_in(&dir), ["hello.bin"]);
        assert_eq!(seen.last(), Some(&5));
        assert!(HELLO.is_complete(&dir));
    }

    #[test]
    fn a_download_that_differs_from_the_pin_leaves_nothing_behind() {
        let dir = folder("corrupt");
        let wrong = save(
            Cursor::new("jello"),
            &HELLO,
            &dir,
            &AtomicBool::new(false),
            &mut |_| {},
        );
        let short = save(
            Cursor::new("hell"),
            &HELLO,
            &dir,
            &AtomicBool::new(false),
            &mut |_| {},
        );
        assert_eq!(wrong, Err(InstallError::Corrupt));
        assert_eq!(short, Err(InstallError::Corrupt));
        assert_eq!(files_in(&dir), NO_FILES);
        assert!(!HELLO.is_complete(&dir));
    }

    #[test]
    fn a_body_longer_than_the_pin_is_cut_off_and_refused() {
        let dir = folder("endless");
        let mut last = 0;
        let saved = save(
            io::repeat(b'x'),
            &HELLO,
            &dir,
            &AtomicBool::new(false),
            &mut |n| {
                last = n;
            },
        );
        assert_eq!(saved, Err(InstallError::Corrupt));
        assert_eq!(last, HELLO.size + 1);
        assert_eq!(files_in(&dir), NO_FILES);
    }

    #[test]
    fn a_cancelled_download_leaves_nothing_behind() {
        let dir = folder("cancelled");
        let saved = save(
            Cursor::new("hello"),
            &HELLO,
            &dir,
            &AtomicBool::new(true),
            &mut |_| {},
        );
        assert_eq!(saved, Err(InstallError::Cancelled));
        assert_eq!(files_in(&dir), NO_FILES);
    }

    #[test]
    fn a_model_counts_as_installed_only_with_all_its_files_at_full_size() {
        const FILES: &[ModelFile] = &[
            HELLO,
            ModelFile {
                name: "config.json",
                ..HELLO
            },
        ];
        let dir = folder("installed");
        let model = Model {
            name: "Test",
            folder: "test",
            source: "https://example.invalid/",
            files: FILES,
        };
        assert_eq!(model.size(), 10);
        assert!(!model.is_installed(&dir));
        let folder = model.path(&dir);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("hello.bin"), "hell").unwrap();
        assert!(!model.is_installed(&dir));
        fs::write(folder.join("hello.bin"), "hello").unwrap();
        // One file is still missing.
        assert!(!model.is_installed(&dir));
        fs::write(folder.join("config.json"), "hello").unwrap();
        assert!(model.is_installed(&dir));
        // Only the offered models count, and none of them is in this folder.
        assert_eq!(super::super::installed(&dir).len(), 0);
    }

    #[test]
    fn offered_models_are_pinned_to_a_revision_over_https() {
        for model in MODELS {
            assert!(model.source.starts_with("https://huggingface.co/"));
            // A full 40-character commit, not a branch that can move.
            let revision = model.source.split("/resolve/").nth(1).unwrap();
            let revision = revision.strip_suffix('/').unwrap();
            assert_eq!(revision.len(), 40);
            assert!(!model.files.is_empty());
        }
        for file in MODELS.iter().flat_map(|m| m.files) {
            assert_eq!(file.sha256.len(), 64);
            assert!(
                file.sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
        }
    }

    /// The real download path end to end, on a small pinned file (Qwen3's tokenizer). Run on
    /// request with `cargo test -- --ignored`.
    #[test]
    #[ignore = "downloads 11 MB from Hugging Face"]
    fn a_pinned_file_downloads_and_installs() {
        let dir = folder("live");
        let model = Model {
            name: "Tokenizer only",
            folder: "live",
            source: "https://huggingface.co/Qwen/Qwen3-4B/resolve/1cfa9a7208912126459214e8b04321603b3df60c/",
            files: &[ModelFile {
                name: "tokenizer.json",
                size: 11_422_654,
                sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
            }],
        };
        // Once to download, then again with nothing left to fetch.
        for _ in 0..2 {
            let installed = install(&model, &dir, &AtomicBool::new(false), &mut |_| {});
            assert_eq!(installed, Ok(()));
            assert!(model.is_installed(&dir));
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hashes_are_written_as_lower_case_hex() {
        assert_eq!(hex(&[0x00, 0x0f, 0xab]), "000fab");
    }
}
