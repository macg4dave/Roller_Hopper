//! The AI models the user can install, where they go, and running the loaded one. Each offered
//! model is one GGUF file pinned by revision, size and SHA-256, so an install is either exactly
//! that file or nothing. GGUF files the user puts in the models folder themselves can be loaded
//! too; every model carries its own tokenizer and chat template.

mod generation;
mod install;
mod run;

pub(crate) use generation::{Generation, MAX_ANSWER_TOKENS};
pub(crate) use install::{InstallError, install};
pub(crate) use run::{Answered, End, Job, RunError, Worker, WorkerEvent};

use std::fs;
use std::path::{Path, PathBuf};

/// A model file, downloaded from a pinned revision.
#[derive(Debug)]
pub(crate) struct ModelFile {
    name: &'static str,
    url: &'static str,
    size: u64,
    /// Lower-case hex.
    sha256: &'static str,
}

/// A model the user can install: one GGUF file in a folder of its own.
#[derive(Debug)]
pub(crate) struct Model {
    /// The model's own name, shown as given.
    pub(crate) name: &'static str,
    folder: &'static str,
    file: ModelFile,
}

/// A model ready to load: an offered one that is installed, or a GGUF file the user put in the
/// models folder.
#[derive(Debug, Clone)]
pub(crate) enum Choice {
    Offered(&'static Model),
    Own(PathBuf),
}

impl Choice {
    /// The name the model list shows: an offered model's own name, or the file's name, so the
    /// two never clash.
    pub(crate) fn name(&self) -> String {
        match self {
            Self::Offered(model) => model.name.to_owned(),
            Self::Own(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    /// The GGUF file to load, for a models folder `dir`.
    pub(crate) fn file(&self, dir: &Path) -> PathBuf {
        match self {
            Self::Offered(model) => model.path(dir).join(model.file.name),
            Self::Own(path) => path.clone(),
        }
    }
}

/// The identification prompt, shared with the evaluation tool so both ask the same thing. The
/// tool holds its answers to the schema while generating; the AI settings window runs whatever
/// prompt the user edits, so it does not, and the schema is part of the prompt. It is the
/// model's instruction, not UI text, so it is not translated.
pub(crate) const PROMPT: &str = concat!(
    include_str!("../tools/ai/prompt.md"),
    "\nAnswer with JSON matching this schema:\n",
    include_str!("../tools/ai/schema.json"),
);

/// A sample device to try the prompt on, in the form the evaluation tool sends.
pub(crate) const SAMPLE_EVIDENCE: &str = include_str!("ai/sample-evidence.txt");

/// The models offered for install, recommended first.
pub(crate) const MODELS: &[Model] = &[
    // 4-bit Q4_K_M keeps 4B parameters to 2.5 GB.
    Model {
        name: "Qwen3-4B",
        folder: "qwen3-4b-q4_k_m",
        file: ModelFile {
            name: "Qwen3-4B-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/bc640142c66e1fdd12af0bd68f40445458f3869b/Qwen3-4B-Q4_K_M.gguf",
            size: 2_497_280_256,
            sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
        },
    },
    // Smaller and faster, but weaker at identification (roadmap Findings).
    Model {
        name: "Qwen3-0.6B",
        folder: "qwen3-0.6b-q8_0",
        file: ModelFile {
            name: "Qwen3-0.6B-Q8_0.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/23749fefcc72300e3a2ad315e1317431b06b590a/Qwen3-0.6B-Q8_0.gguf",
            size: 639_446_688,
            sha256: "9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031",
        },
    },
    // A tiny download for slow lines or small PCs; weaker again than Qwen3-0.6B.
    Model {
        name: "SmolLM2-135M",
        folder: "smollm2-135m-instruct-q8_0",
        file: ModelFile {
            name: "SmolLM2-135M-Instruct-Q8_0.gguf",
            url: "https://huggingface.co/bartowski/SmolLM2-135M-Instruct-GGUF/resolve/09816acd5d99df7be770d85ea30822623dab342c/SmolLM2-135M-Instruct-Q8_0.gguf",
            size: 144_811_360,
            sha256: "5a1395716f7913741cc51d98581b9b1228d80987a9f7d3664106742eb06bba83",
        },
    },
];

impl Model {
    /// The download size, in bytes.
    pub(crate) fn size(&self) -> u64 {
        self.file.size
    }

    /// Whether its file is in `dir` at full size. The hash was checked when it was downloaded;
    /// re-reading gigabytes each time the window opens would be too slow.
    pub(crate) fn is_installed(&self, dir: &Path) -> bool {
        self.file.is_complete(&self.path(dir))
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
    cfg!(feature = "cuda")
}

/// The models ready to load from `dir`: the offered ones fully installed, in offer order, then
/// the GGUF files at the top of the folder, by name. Whether a file is a model the app can run
/// is only known once it loads.
pub(crate) fn installed(dir: &Path) -> Vec<Choice> {
    let offered = MODELS.iter().filter(|m| m.is_installed(dir));
    // A missing folder or an unreadable entry holds nothing that could load.
    let mut own: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("gguf"))
        })
        .collect();
    own.sort();
    offered
        .map(Choice::Offered)
        .chain(own.into_iter().map(Choice::Own))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gguf_files_at_the_top_of_the_folder_are_listed_after_offered_models() {
        let dir = std::env::temp_dir().join(format!("network-mapper-own-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(dir.join("nested")).unwrap();
        for name in ["b.gguf", "A.GGUF", "notes.txt", "nested/c.gguf"] {
            fs::write(dir.join(name), "").unwrap();
        }
        fs::create_dir_all(dir.join("folder.gguf")).unwrap();
        let listed = installed(&dir);
        let names: Vec<String> = listed.iter().map(Choice::name).collect();
        assert_eq!(names, ["A.GGUF", "b.gguf"]);
        assert_eq!(
            listed.first().map(|c| c.file(&dir)),
            Some(dir.join("A.GGUF"))
        );
        fs::remove_dir_all(&dir).ok();
        assert!(installed(&dir).is_empty());
    }

    #[test]
    fn an_offered_model_loads_from_its_own_folder() {
        let dir = Path::new("models");
        let model = MODELS.first().unwrap();
        assert_eq!(
            Choice::Offered(model).file(dir),
            dir.join("qwen3-4b-q4_k_m").join("Qwen3-4B-Q4_K_M.gguf")
        );
    }
}
