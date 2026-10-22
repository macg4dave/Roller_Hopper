//! The AI model the user can install, where it goes, and running the loaded one. The app runs
//! Qwen3 GGUF files only. The offered model is pinned by revision, size and SHA-256, so an
//! install is either exactly that file or not installed. Qwen3 GGUF files the user puts in the
//! models folder themselves can be loaded too; each carries its own tokenizer in its metadata.

mod chat;
mod engine;
mod generation;
mod identify;
mod install;
mod response;
mod run;
mod samples;
pub(crate) mod saved_prompt;

pub(crate) use generation::{Generation, MAX_CONTEXT_TOKENS};
pub(crate) use identify::{Identity, evidence};
pub(crate) use install::{InstallError, install};
pub(crate) use run::{Answered, Asker, End, IDLE_UNLOAD, Job, RunError, Worker, WorkerEvent};

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

/// A model the user can install: its GGUF file, in a folder of its own.
#[derive(Debug)]
pub(crate) struct Model {
    /// The model's own name, shown as given.
    pub(crate) name: &'static str,
    folder: &'static str,
    /// The pinned revision's download address, ending in `/`; each file's name follows it.
    source: &'static str,
    files: &'static [ModelFile],
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
            Self::Offered(model) => model.load_path(dir),
            Self::Own(path) => path.clone(),
        }
    }
}

/// The default identification prompt, which the AI settings window starts from until the user
/// changes it ([`saved_prompt`]). It names its own answer fields, so no schema is added. The
/// evaluation tool reads its own `tools/ai/prompt.md` and schema. It is the model's
/// instruction, not UI text, so it is not translated.
pub(crate) const PROMPT: &str = include_str!("ai/default-prompt.txt");

pub(crate) use samples::evidence_sample;

/// The models offered for install: only the main application AI.
pub(crate) const MODELS: &[Model] = &[
    // 4-bit Q4_K_M keeps 4B parameters to 2.5 GB. The same file as
    // `tools/a.i.testmodels/Qwen3`, in a folder of the same name, so a copy placed there by
    // hand counts as installed.
    Model {
        name: "Qwen3-4B-Q4_K_M",
        folder: "Qwen3",
        source: "https://huggingface.co/unsloth/Qwen3-4B-GGUF/resolve/22c9fc8a8c7700b76a1789366280a6a5a1ad1120/",
        files: &[ModelFile {
            name: "Qwen3-4B-Q4_K_M.gguf",
            size: 2_497_281_312,
            sha256: "f6f851777709861056efcdad3af01da38b31223a3ba26e61a4f8bf3a2195813a",
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

    /// Its GGUF file; a model without one has nothing to load, so its folder is reported
    /// missing.
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

/// The app's folder in the user's application data folder (`%APPDATA%` on Windows), or `None`
/// when the system has none.
pub(crate) fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Network Mapper"))
}

/// Where models are installed, in the app's data folder.
pub(crate) fn models_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("models"))
}

/// The file Import with AI loads: the offered main application AI (Qwen3-4B), if installed.
pub(crate) fn import_model() -> Option<PathBuf> {
    let dir = models_dir()?;
    let model = MODELS.first().filter(|m| m.is_installed(&dir))?;
    Some(model.load_path(&dir))
}

/// Whether this build can run models on an NVIDIA GPU (built with the `cuda` feature).
pub(crate) fn gpu_built() -> bool {
    cfg!(feature = "cuda")
}

/// The models ready to load from `dir`: the offered ones fully installed, in offer order, then
/// the user's GGUF files, by path: those at the top of the folder and those one folder down,
/// apart from the offered models' own folders. Whether one is a Qwen3 model the app can run is
/// only known once it loads.
pub(crate) fn installed(dir: &Path) -> Vec<Choice> {
    let offered = MODELS.iter().filter(|m| m.is_installed(dir));
    let offered_folders: Vec<PathBuf> = MODELS.iter().map(|m| m.path(dir)).collect();
    let mut own: Vec<PathBuf> = entries(dir)
        .filter(|path| !offered_folders.contains(path))
        .flat_map(|path| {
            if path.is_dir() {
                entries(&path).collect()
            } else {
                vec![path]
            }
        })
        .filter(|path| path.is_file() && has_extension(path, "gguf"))
        .collect();
    own.sort();
    offered
        .map(Choice::Offered)
        .chain(own.into_iter().map(Choice::Own))
        .collect()
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
    fn an_offered_model_loads_from_its_own_folder() {
        let dir = Path::new("models");
        let model = MODELS.first().unwrap();
        assert_eq!(
            Choice::Offered(model).file(dir),
            dir.join("Qwen3").join("Qwen3-4B-Q4_K_M.gguf")
        );
    }
}
