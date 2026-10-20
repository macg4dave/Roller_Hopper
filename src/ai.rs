//! The AI models the user can install, and where they go. Each model's files are pinned by
//! revision, size and SHA-256, so an install is either exactly those files or nothing. Running
//! a model is not built yet; this module only knows what is installed.

mod install;

pub(crate) use install::{InstallError, install};

use std::fs;
use std::path::{Path, PathBuf};

/// One file of a model, downloaded from a pinned revision.
#[derive(Debug)]
pub(crate) struct ModelFile {
    name: &'static str,
    url: &'static str,
    size: u64,
    /// Lower-case hex.
    sha256: &'static str,
}

/// A model the user can install, kept in its own folder.
#[derive(Debug)]
pub(crate) struct Model {
    /// The model's own name, shown as given.
    pub(crate) name: &'static str,
    folder: &'static str,
    files: &'static [ModelFile],
}

/// The models offered for install, recommended first.
pub(crate) const MODELS: &[Model] = &[Model {
    name: "Qwen3-4B",
    folder: "qwen3-4b-q4_k_m",
    files: &[
        // Apache-2.0. The 4-bit Q4_K_M quantisation keeps 4B parameters to 2.5 GB.
        ModelFile {
            name: "Qwen3-4B-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/bc640142c66e1fdd12af0bd68f40445458f3869b/Qwen3-4B-Q4_K_M.gguf",
            size: 2_497_280_256,
            sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
        },
        ModelFile {
            name: "tokenizer.json",
            url: "https://huggingface.co/Qwen/Qwen3-4B/resolve/1cfa9a7208912126459214e8b04321603b3df60c/tokenizer.json",
            size: 11_422_654,
            sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
        },
    ],
}];

impl Model {
    /// The download size of all its files, in bytes.
    pub(crate) fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Whether every file is in `dir` at its full size. The hashes were checked when they were
    /// downloaded; re-reading gigabytes each time the window opens would be too slow.
    pub(crate) fn is_installed(&self, dir: &Path) -> bool {
        self.files.iter().all(|f| f.is_complete(&self.path(dir)))
    }

    fn path(&self, dir: &Path) -> PathBuf {
        dir.join(self.folder)
    }
}

impl ModelFile {
    fn is_complete(&self, folder: &Path) -> bool {
        fs::metadata(folder.join(self.name)).is_ok_and(|m| m.is_file() && m.len() == self.size)
    }
}

/// Where models are installed: the user's application data folder (`%APPDATA%` on Windows), or
/// `None` when the system has none.
pub(crate) fn models_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Network Mapper").join("models"))
}

/// The models fully installed in `dir`, in offer order.
pub(crate) fn installed(dir: &Path) -> Vec<&'static Model> {
    MODELS.iter().filter(|m| m.is_installed(dir)).collect()
}
