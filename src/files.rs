//! Durable writes shared by the project file and small application settings.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use atomic_write_file::AtomicWriteFile;

/// Replaces `path` only after every byte has been written. A symbolic link is followed so the
/// link itself is never silently replaced; a broken link is an error.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let target = through_link(path)?;
    let mut file = AtomicWriteFile::open(target)?;
    file.write_all(bytes)?;
    file.commit()
}

fn through_link(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::canonicalize(path),
        Ok(_) => Ok(path.to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_owned()),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("could not inspect {}: {error}", path.display()),
        )),
    }
}
