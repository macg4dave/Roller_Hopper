//! The AI settings the user saved from the AI settings window: the prompt, the answer settings,
//! whether to use the GPU and the model last loaded. Import with AI runs with them. The prompt
//! is kept only while it differs from the default [`PROMPT`](super::PROMPT), so a better
//! default reaches anyone who has not changed it.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::Generation;

const FILE: &str = "ai-settings.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Saved {
    /// `None` while it is the default.
    prompt: Option<String>,
    pub(crate) generation: Generation,
    pub(crate) gpu: bool,
    /// The model list's name for the model last loaded.
    pub(crate) model: Option<String>,
}

impl Default for Saved {
    /// Nothing saved: the default prompt and answer settings, on the GPU in a build that can.
    fn default() -> Self {
        Self {
            prompt: None,
            generation: Generation::default(),
            gpu: super::gpu_built(),
            model: None,
        }
    }
}

impl Saved {
    pub(crate) fn new(
        prompt: &str,
        generation: Generation,
        gpu: bool,
        model: Option<String>,
    ) -> Self {
        Self {
            prompt: (prompt != super::PROMPT).then(|| prompt.to_owned()),
            generation,
            gpu,
            model,
        }
    }

    /// What is saved in `dir`, or nothing saved when there is no file or it cannot be read
    /// (from an older app, or edited by hand). Answer settings are brought within their limits.
    pub(crate) fn load(dir: &Path) -> Self {
        let text = fs::read_to_string(dir.join(FILE)).unwrap_or_default();
        let saved: Self = serde_json::from_str(&text).unwrap_or_default();
        Self {
            generation: saved.generation.bounded(),
            gpu: saved.gpu && super::gpu_built(),
            ..saved
        }
    }

    /// What is saved in the app's data folder, or nothing saved when the system has none.
    pub(crate) fn in_data_dir() -> Self {
        super::data_dir().map_or_else(Self::default, |dir| Self::load(&dir))
    }

    pub(crate) fn save(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        fs::write(dir.join(FILE), serde_json::to_string_pretty(self)?)
    }

    /// The prompt to run: the user's, or the default.
    pub(crate) fn prompt(&self) -> &str {
        self.prompt.as_deref().unwrap_or(super::PROMPT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_settings_come_back_and_the_default_prompt_is_not_kept() {
        let dir = std::env::temp_dir()
            .join(format!("network-mapper-ai-settings-{}", std::process::id()))
            .join("Network Mapper");
        fs::remove_dir_all(&dir).ok();
        assert_eq!(Saved::load(&dir), Saved::default(), "nothing saved yet");

        let thinking = Generation::qwen3(true);
        let saved = Saved::new("Name the device.", thinking, false, Some("Qwen3-4B".into()));
        saved.save(&dir).unwrap();
        let loaded = Saved::load(&dir);
        assert_eq!(loaded, saved);
        assert_eq!(loaded.prompt(), "Name the device.");

        let default = Saved::new(super::super::PROMPT, thinking, false, None);
        assert_eq!(default.prompt, None, "a newer default would show");
        assert_eq!(default.prompt(), super::super::PROMPT);

        fs::write(dir.join(FILE), "not settings").unwrap();
        assert_eq!(Saved::load(&dir), Saved::default());
        fs::remove_dir_all(&dir).ok();
    }
}
