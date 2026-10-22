//! The user's own prompt, kept in the app's data folder only while it differs from the default
//! [`PROMPT`](super::PROMPT), so a better default reaches anyone who has not changed it.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

const FILE: &str = "prompt.txt";

/// The prompt saved in `dir`, or the default when none is saved or it cannot be read.
pub(crate) fn load(dir: &Path) -> String {
    fs::read_to_string(dir.join(FILE)).unwrap_or_else(|_| super::PROMPT.to_owned())
}

/// Keeps `text` as the user's prompt in `dir`; the default itself removes any saved one.
pub(crate) fn save(dir: &Path, text: &str) -> io::Result<()> {
    let file = dir.join(FILE);
    if text == super::PROMPT {
        return match fs::remove_file(file) {
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            removed => removed,
        };
    }
    fs::create_dir_all(dir)?;
    fs::write(file, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edited_prompt_is_kept_until_the_default_comes_back() {
        let dir = std::env::temp_dir()
            .join(format!("network-mapper-prompt-{}", std::process::id()))
            .join("Network Mapper");
        fs::remove_dir_all(&dir).ok();
        // Nothing saved yet, and no folder either.
        assert_eq!(load(&dir), super::super::PROMPT);
        save(&dir, "Name the device.").unwrap();
        assert_eq!(load(&dir), "Name the device.");
        // Back to the default: the saved file goes, so a newer default would show.
        save(&dir, super::super::PROMPT).unwrap();
        assert!(!dir.join(FILE).exists());
        assert_eq!(load(&dir), super::super::PROMPT);
        // Saving the default with nothing saved is not an error.
        save(&dir, super::super::PROMPT).unwrap();
        fs::remove_dir_all(&dir).ok();
    }
}
