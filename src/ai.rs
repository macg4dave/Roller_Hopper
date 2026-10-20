//! The AI models the user can install, where they go, and running the loaded one. Each model's
//! files are pinned by revision, size and SHA-256, so an install is either exactly those files
//! or nothing.

mod generation;
mod install;
mod run;

pub(crate) use generation::{Generation, MAX_ANSWER_TOKENS};
pub(crate) use install::{InstallError, install};
pub(crate) use run::{Answered, End, Job, RunError, Worker, WorkerEvent};

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
    arch: Arch,
    weights: ModelFile,
    tokenizer: ModelFile,
}

/// How a model's weights are laid out, which decides how they load and how a chat is framed.
/// Both mark turns with `<|im_start|>` and `<|im_end|>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arch {
    /// Qwen3, answering with its thinking switched off.
    Qwen3,
    /// Llama-style, such as `SmolLM2`.
    Llama,
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
        arch: Arch::Qwen3,
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
        arch: Arch::Qwen3,
        weights: ModelFile {
            name: "Qwen3-0.6B-Q8_0.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/23749fefcc72300e3a2ad315e1317431b06b590a/Qwen3-0.6B-Q8_0.gguf",
            size: 639_446_688,
            sha256: "9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031",
        },
        tokenizer: QWEN3_TOKENIZER,
    },
    // A tiny download for slow lines or small PCs; weaker again than Qwen3-0.6B.
    Model {
        name: "SmolLM2-135M",
        folder: "smollm2-135m-instruct-q8_0",
        arch: Arch::Llama,
        weights: ModelFile {
            name: "SmolLM2-135M-Instruct-Q8_0.gguf",
            url: "https://huggingface.co/bartowski/SmolLM2-135M-Instruct-GGUF/resolve/09816acd5d99df7be770d85ea30822623dab342c/SmolLM2-135M-Instruct-Q8_0.gguf",
            size: 144_811_360,
            sha256: "5a1395716f7913741cc51d98581b9b1228d80987a9f7d3664106742eb06bba83",
        },
        tokenizer: ModelFile {
            name: "tokenizer.json",
            url: "https://huggingface.co/HuggingFaceTB/SmolLM2-135M-Instruct/resolve/12fd25f77366fa6b3b4b768ec3050bf629380bac/tokenizer.json",
            size: 2_104_556,
            sha256: "9ca9acddb6525a194ec8ac7a87f24fbba7232a9a15ffa1af0c1224fcd888e47c",
        },
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

    /// Whether it can reason before answering (see [`Generation::think`]).
    pub(crate) fn thinks(&self) -> bool {
        match self.arch {
            Arch::Qwen3 => true,
            Arch::Llama => false,
        }
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
