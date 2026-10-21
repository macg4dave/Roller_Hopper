//! Explicit CUDA builds backed by mistral.rs's shared kernel build root.

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command as Process;

const CACHE: &str = "cuda-cache";

#[derive(Clone, Copy, Debug, clap::Subcommand)]
pub(super) enum Command {
    /// Build with CUDA, reusing compiled kernels from cuda-cache/.
    Build,
    /// Delete cuda-cache/ and build every CUDA kernel again.
    Rebuild,
}

pub(super) fn run(command: Command, root: &Path) -> io::Result<()> {
    if matches!(command, Command::Rebuild) {
        remove_cache(root)?;
    }
    let mut cargo = Process::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cargo
        .current_dir(root)
        .args(["build", "--locked", "--features", "cuda"]);
    if std::env::var_os("CARGO_BUILD_JOBS").is_none() {
        cargo.env(
            "CARGO_BUILD_JOBS",
            std::thread::available_parallelism()?.to_string(),
        );
    }
    let status = cargo.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("CUDA build failed with {status}")))
    }
}

fn remove_cache(root: &Path) -> io::Result<()> {
    let cache = root.join(CACHE);
    if cache.parent() != Some(root)
        || cache.file_name().and_then(|name| name.to_str()) != Some(CACHE)
    {
        return Err(io::Error::other(
            "refusing to remove an unexpected CUDA cache path",
        ));
    }
    match fs::remove_dir_all(cache) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_the_cache_never_removes_its_parent() -> io::Result<()> {
        let fixture =
            std::env::temp_dir().join(format!("network-mapper-cuda-{}", std::process::id()));
        let cache = fixture.join(CACHE);
        fs::create_dir_all(&cache)?;
        fs::write(cache.join("kernel.o"), b"cached")?;
        remove_cache(&fixture)?;
        assert!(fixture.is_dir());
        assert!(!cache.exists());
        fs::remove_dir(fixture)
    }
}
