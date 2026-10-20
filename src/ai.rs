//! The AI models the user can install, where they go, and running the loaded one. Each model's
//! files are pinned by revision, size and SHA-256, so an install is either exactly those files
//! or nothing.

mod install;
mod run;

pub(crate) use install::{InstallError, install};
pub(crate) use run::{Job, RunError, Worker, WorkerEvent};

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

/// A model the user can install, kept in its own folder: GGUF weights and the tokenizer that
/// turns text into their tokens.
#[derive(Debug)]
pub(crate) struct Model {
    /// The model's own name, shown as given.
    pub(crate) name: &'static str,
    folder: &'static str,
    weights: ModelFile,
    tokenizer: ModelFile,
}

/// The identification prompt, shared with the evaluation tool so both ask the same thing. The
/// tool holds its answers to the schema while generating; the app cannot, so the schema is
/// part of the prompt. It is the model's instruction, not UI text, so it is not translated.
pub(crate) const PROMPT: &str = concat!(
    include_str!("../tools/ai/prompt.md"),
    "\nAnswer with JSON matching this schema:\n",
    include_str!("../tools/ai/schema.json"),
);

/// A sample device to try the prompt on, in the form the evaluation tool sends.
pub(crate) const SAMPLE_EVIDENCE: &str = include_str!("ai/sample-evidence.txt");

/// The same tokenizer serves every Qwen3 size.
const QWEN3_TOKENIZER: ModelFile = ModelFile {
    name: "tokenizer.json",
    url: "https://huggingface.co/Qwen/Qwen3-4B/resolve/1cfa9a7208912126459214e8b04321603b3df60c/tokenizer.json",
    size: 11_422_654,
    sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
};

/// The models offered for install, recommended first.
pub(crate) const MODELS: &[Model] = &[
    // 4-bit Q4_K_M keeps 4B parameters to 2.5 GB.
    Model {
        name: "Qwen3-4B",
        folder: "qwen3-4b-q4_k_m",
        weights: ModelFile {
            name: "Qwen3-4B-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/bc640142c66e1fdd12af0bd68f40445458f3869b/Qwen3-4B-Q4_K_M.gguf",
            size: 2_497_280_256,
            sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
        },
        tokenizer: QWEN3_TOKENIZER,
    },
    // Smaller and faster, but weaker at identification (roadmap Findings).
    Model {
        name: "Qwen3-0.6B",
        folder: "qwen3-0.6b-q8_0",
        weights: ModelFile {
            name: "Qwen3-0.6B-Q8_0.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/23749fefcc72300e3a2ad315e1317431b06b590a/Qwen3-0.6B-Q8_0.gguf",
            size: 639_446_688,
            sha256: "9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031",
        },
        tokenizer: QWEN3_TOKENIZER,
    },
];

impl Model {
    fn files(&self) -> [&ModelFile; 2] {
        [&self.weights, &self.tokenizer]
    }

    /// The download size of all its files, in bytes.
    pub(crate) fn size(&self) -> u64 {
        self.files().iter().map(|f| f.size).sum()
    }

    /// Whether every file is in `dir` at its full size. The hashes were checked when they were
    /// downloaded; re-reading gigabytes each time the window opens would be too slow.
    pub(crate) fn is_installed(&self, dir: &Path) -> bool {
        self.files().iter().all(|f| f.is_complete(&self.path(dir)))
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

/// Whether this build can run models on an NVIDIA GPU (built with the `cuda` feature).
pub(crate) fn gpu_built() -> bool {
    candle_core::utils::cuda_is_available()
}

/// The models fully installed in `dir`, in offer order.
pub(crate) fn installed(dir: &Path) -> Vec<&'static Model> {
    MODELS.iter().filter(|m| m.is_installed(dir)).collect()
}
