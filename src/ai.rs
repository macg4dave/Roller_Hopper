//! The AI models the user can install, where they go, and running the loaded one. Each offered
//! model is one GGUF file or a safetensors folder's files, each pinned by revision, size and
//! SHA-256, so an install is either exactly those files or not installed. GGUF files and Hugging Face safetensors folders the user puts in the
//! models folder themselves can be loaded too; every model supplies its own tokenizer in its GGUF
//! metadata or beside its safetensors.

mod chat;
mod engine;
mod generation;
mod install;
mod response;
mod run;
mod samples;
mod weights;

pub(crate) use generation::{Generation, MAX_CONTEXT_TOKENS};
pub(crate) use install::{InstallError, install};
pub(crate) use run::{
    Answered, End, Job, ModelFormat, Precision, RunError, TaskProfile, Worker, WorkerEvent,
};

use std::fs;
use std::path::{Path, PathBuf};

/// A model file, downloaded from its model's pinned revision.
#[derive(Debug)]
pub(crate) struct ModelFile {
    name: &'static str,
    size: u64,
    /// Lower-case hex.
    sha256: &'static str,
}

/// A model the user can install, in a folder of its own: one GGUF file, or the files of a
/// Hugging Face safetensors folder.
#[derive(Debug)]
pub(crate) struct Model {
    /// The model's own name, shown as given.
    pub(crate) name: &'static str,
    folder: &'static str,
    /// The pinned revision's download address, ending in `/`; each file's name follows it.
    source: &'static str,
    files: &'static [ModelFile],
}

/// A model ready to load: an offered one that is installed, or a GGUF file or safetensors
/// folder the user put in the models folder.
#[derive(Debug, Clone)]
pub(crate) enum Choice {
    Offered(&'static Model),
    Own(PathBuf),
}

impl Choice {
    /// The name the model list shows: an offered model's own name, or the file's or folder's
    /// name, so the two never clash.
    pub(crate) fn name(&self) -> String {
        match self {
            Self::Offered(model) => model.name.to_owned(),
            Self::Own(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    /// The GGUF file or safetensors folder to load, for a models folder `dir`.
    pub(crate) fn file(&self, dir: &Path) -> PathBuf {
        match self {
            Self::Offered(model) => model.load_path(dir),
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

pub(crate) use samples::evidence_sample;

/// The models offered for install, recommended first.
pub(crate) const MODELS: &[Model] = &[
    // 4-bit Q4_K_M keeps 4B parameters to 2.5 GB.
    Model {
        name: "Qwen3-4B",
        folder: "qwen3-4b-q4_k_m",
        source: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/bc640142c66e1fdd12af0bd68f40445458f3869b/",
        files: &[ModelFile {
            name: "Qwen3-4B-Q4_K_M.gguf",
            size: 2_497_280_256,
            sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
        }],
    },
    // Smaller and faster, but weaker at identification (roadmap Findings).
    Model {
        name: "Qwen3-0.6B",
        folder: "qwen3-0.6b-q8_0",
        source: "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/23749fefcc72300e3a2ad315e1317431b06b590a/",
        files: &[ModelFile {
            name: "Qwen3-0.6B-Q8_0.gguf",
            size: 639_446_688,
            sha256: "9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031",
        }],
    },
];

impl Model {
    /// The download size of all its files, in bytes.
    pub(crate) fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Whether all its files are in `dir` at full size. The hashes were checked when they were
    /// downloaded; re-reading gigabytes each time the window opens would be too slow.
    pub(crate) fn is_installed(&self, dir: &Path) -> bool {
        let folder = self.path(dir);
        self.files.iter().all(|f| f.is_complete(&folder))
    }

    fn path(&self, dir: &Path) -> PathBuf {
        dir.join(self.folder)
    }

    /// Its GGUF file, or else its folder of safetensors.
    fn load_path(&self, dir: &Path) -> PathBuf {
        let folder = self.path(dir);
        match self
            .files
            .iter()
            .find(|f| has_extension(Path::new(f.name), "gguf"))
        {
            Some(gguf) => folder.join(gguf.name),
            None => folder,
        }
    }

    fn url(&self, file: &ModelFile) -> String {
        format!("{}{}", self.source, file.name)
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
/// the user's GGUF files and safetensors folders, by path: those at the top of the folder and
/// those one folder down, apart from the offered models' own folders. Whether one is a model
/// the app can run is only known once it loads.
pub(crate) fn installed(dir: &Path) -> Vec<Choice> {
    let offered = MODELS.iter().filter(|m| m.is_installed(dir));
    let offered_folders: Vec<PathBuf> = MODELS.iter().map(|m| m.path(dir)).collect();
    let mut own: Vec<PathBuf> = entries(dir)
        .filter(|path| !offered_folders.contains(path))
        .flat_map(|path| {
            if path.is_dir() {
                // The folder itself may be a safetensors model.
                let mut inside: Vec<PathBuf> = entries(&path).collect();
                inside.push(path);
                inside
            } else {
                vec![path]
            }
        })
        .filter(|path| {
            if path.is_dir() {
                is_safetensors_folder(path)
            } else {
                has_extension(path, "gguf")
            }
        })
        .collect();
    own.sort();
    offered
        .map(Choice::Offered)
        .chain(own.into_iter().map(Choice::Own))
        .collect()
}

/// Whether `folder` is laid out as a Hugging Face model: its `config.json` beside its
/// safetensors weights. The tokenizer and chat template it also needs are checked on load.
fn is_safetensors_folder(folder: &Path) -> bool {
    folder.join("config.json").is_file()
        && entries(folder).any(|p| p.is_file() && has_extension(&p, "safetensors"))
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
}

/// What `dir` holds; a missing folder or an unreadable entry holds nothing that could load.
fn entries(dir: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gguf_files_at_the_top_and_one_folder_down_are_listed_after_offered_models() {
        let dir = std::env::temp_dir().join(format!("network-mapper-own-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        let offered = MODELS.first().unwrap().path(&dir);
        for folder in [dir.join("nested").join("deeper"), offered.clone()] {
            fs::create_dir_all(folder).unwrap();
        }
        for name in [
            "b.gguf",
            "A.GGUF",
            "notes.txt",
            "nested/c.gguf",
            "nested/deeper/d.gguf",
        ] {
            fs::write(dir.join(name), "").unwrap();
        }
        // An offered model's folder holding a partial download is not the user's model.
        fs::write(offered.join("Qwen3-4B-Q4_K_M.gguf"), "").unwrap();
        fs::create_dir_all(dir.join("folder.gguf")).unwrap();
        let listed = installed(&dir);
        let names: Vec<String> = listed.iter().map(Choice::name).collect();
        assert_eq!(names, ["A.GGUF", "b.gguf", "c.gguf"]);
        assert_eq!(
            listed.first().map(|c| c.file(&dir)),
            Some(dir.join("A.GGUF"))
        );
        fs::remove_dir_all(&dir).ok();
        assert!(installed(&dir).is_empty());
    }

    #[test]
    fn safetensors_folders_at_the_top_and_one_folder_down_are_listed_by_name() {
        let dir = std::env::temp_dir().join(format!("network-mapper-hf-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        for folder in ["Tiny", "hf/Small", "hf/no-weights", "hf/no-config"] {
            fs::create_dir_all(dir.join(folder)).unwrap();
        }
        for name in [
            "Tiny/config.json",
            "Tiny/model.safetensors",
            "hf/Small/config.json",
            "hf/Small/model-00001-of-00002.SAFETENSORS",
            "hf/no-weights/config.json",
            "hf/no-config/model.safetensors",
            "z.gguf",
        ] {
            fs::write(dir.join(name), "").unwrap();
        }
        let listed = installed(&dir);
        let names: Vec<String> = listed.iter().map(Choice::name).collect();
        // By path, so the top folder's "Tiny" comes before "hf/Small".
        assert_eq!(names, ["Tiny", "Small", "z.gguf"]);
        assert_eq!(listed.first().map(|c| c.file(&dir)), Some(dir.join("Tiny")));
        fs::remove_dir_all(&dir).ok();
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

    #[test]
    fn an_offered_models_folder_is_not_listed_as_the_users_own_safetensors() {
        let dir =
            std::env::temp_dir().join(format!("network-mapper-offered-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        let folder = MODELS.first().unwrap().path(&dir);
        fs::create_dir_all(&folder).unwrap();
        // Laid out like a safetensors model, but it is the offered model's own folder.
        fs::write(folder.join("config.json"), "{}").unwrap();
        fs::write(folder.join("model.safetensors"), "").unwrap();
        assert!(installed(&dir).is_empty());
        fs::remove_dir_all(&dir).ok();
    }
}
