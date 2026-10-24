//! Project files: one versioned `.rhopper.json` document holding the map. Open reads a file
//! into a separate project and checks it before it replaces anything; Save writes a complete
//! temporary file beside the old one and then swaps it in, so a failed save keeps the old
//! file. Nothing here knows about Slint.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::files;
use crate::model::{Arrangement, Device, Network};

/// Marks a file as a project, whatever it is called.
const FORMAT: &str = "rhopper";
/// The version this build writes. Older ones would be migrated here; newer ones are refused.
const VERSION: u32 = 1;
/// What every project file's name ends with.
pub(crate) const EXTENSION: &str = "rhopper.json";

/// The agreed limits on what Open reads.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DEVICES: usize = 10_000;
const MAX_CARDS: usize = 50_000;
const MAX_SERVICES: usize = 200_000;
const MAX_TEXT_BYTES: usize = 4 * 1024;
const MAX_NOTES_BYTES: usize = 64 * 1024;

/// What a project holds: the network and how the user arranged its map.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct Project {
    pub(crate) network: Network,
    pub(crate) arrangement: Arrangement,
}

/// The document as saved: the project, with what tells it apart from any other JSON file.
#[derive(Serialize, Deserialize)]
struct Document<P> {
    format: String,
    version: u32,
    project: P,
}

/// Why a file could not be opened, as data: the UI words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpenError {
    /// The system's own message.
    Unreadable(String),
    /// Over the agreed limits on size, counts or text.
    TooLarge,
    /// Not a project file at all.
    NotProject,
    /// Saved by a newer version of the app.
    Newer,
    /// A project file that does not hold together.
    Damaged,
}

/// Why a save failed: the system's own message. The file there before is left as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaveError {
    TooLarge,
    Io(String),
}

/// Reads and checks the project at `path`.
pub(crate) fn read(path: &Path) -> Result<Project, OpenError> {
    let unreadable = |error: std::io::Error| OpenError::Unreadable(error.to_string());
    let bytes = read_bounded(path, MAX_FILE_BYTES)
        .map_err(unreadable)?
        .ok_or(OpenError::TooLarge)?;
    parse(&bytes)
}

/// Reads at most one byte beyond `limit`, so a changing or sparse file cannot make the process
/// allocate without bound between a metadata check and the read.
fn read_bounded(path: &Path, limit: u64) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok((u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= limit).then_some(bytes))
}

fn parse(bytes: &[u8]) -> Result<Project, OpenError> {
    // serde_json limits nesting depth itself, so a hostile file cannot exhaust the stack.
    let header: Document<IgnoredAny> =
        serde_json::from_slice(bytes).map_err(|_| OpenError::NotProject)?;
    if header.format != FORMAT || header.version == 0 {
        return Err(OpenError::NotProject);
    }
    if header.version > VERSION {
        return Err(OpenError::Newer);
    }
    let document: Document<Project> =
        serde_json::from_slice(bytes).map_err(|_| OpenError::Damaged)?;
    let Project {
        network,
        mut arrangement,
    } = document.project;
    if !within_limits(&network) {
        return Err(OpenError::TooLarge);
    }
    // Which check failed is for the tests (see `Network::checked`); the user is told the file
    // is damaged either way.
    let network = network.checked().map_err(|_| OpenError::Damaged)?;
    arrangement.retain(&network);
    Ok(Project {
        network,
        arrangement,
    })
}

/// The large built-in infrastructure fixture, decoded and checked exactly like a project the
/// user opens from disk.
pub(crate) fn mock() -> Result<Project, OpenError> {
    parse(include_bytes!("../mock-data/infrastructure.rhopper.json"))
}

fn within_limits(network: &Network) -> bool {
    let devices = network.devices();
    let cards = devices.iter().flat_map(|d| &d.cards);
    let services = cards.clone().flat_map(|c| c.services.iter());
    let card_texts = cards.clone().flat_map(|c| {
        let texts = [Some(c.name.as_str()), c.vendor.as_deref(), Some(&c.icon.0)];
        let descriptions = c.services.iter().map(|s| Some(s.description.as_str()));
        texts.into_iter().chain(descriptions)
    });
    let role_names = network.roles().into_iter().map(|role| {
        let name = network.role_name(&role).map_or(0, str::len);
        let icon = network.role_icon(&role).map_or(0, |i| i.0.len());
        name.max(icon)
    });
    let groups = network.groups().iter();
    let group_names = groups.map(|(_, name)| name.typed().map_or(0, str::len));
    let short = |text: Option<&str>| text.is_none_or(|t| t.len() <= MAX_TEXT_BYTES);
    devices.len() <= MAX_DEVICES
        && cards.count() <= MAX_CARDS
        && services.count() <= MAX_SERVICES
        && devices.iter().all(|d| device_within_limits(d, short))
        && card_texts.into_iter().all(short)
        && role_names.chain(group_names).all(|n| n <= MAX_TEXT_BYTES)
}

fn device_within_limits(device: &Device, short: impl Fn(Option<&str>) -> bool) -> bool {
    let texts = [
        Some(device.name.as_str()),
        device.manufacturer.as_deref(),
        device.model.as_deref(),
        device.hostname.as_deref(),
        device.icon.as_ref().map(|i| i.0.as_str()),
        device.os.as_ref().and_then(|os| os.detail.as_deref()),
    ];
    let services = device.mdns_services.iter().map(|s| Some(s.as_str()));
    device.notes.len() <= MAX_NOTES_BYTES && texts.into_iter().chain(services).all(short)
}

/// Saves `project` at `path`, replacing what was there only once the new file is complete.
pub(crate) fn write(path: &Path, project: &Project) -> Result<(), SaveError> {
    if !within_limits(&project.network) {
        return Err(SaveError::TooLarge);
    }
    let document = Document {
        format: FORMAT.to_owned(),
        version: VERSION,
        project,
    };
    let bytes = serde_json::to_vec_pretty(&document).map_err(|e| SaveError::Io(e.to_string()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(SaveError::TooLarge);
    }
    files::atomic_write(path, &bytes).map_err(|e| SaveError::Io(e.to_string()))
}

/// `path` with the project file ending, unless it already ends in `.json`.
pub(crate) fn with_extension(path: PathBuf) -> PathBuf {
    let named = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if named {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".");
    name.push(EXTENSION);
    name.into()
}

/// The project's name: its file name without the project file ending.
pub(crate) fn name(path: &Path) -> String {
    let file = path.file_name().unwrap_or_default().to_string_lossy();
    let ending = [format!(".{EXTENSION}"), ".json".to_owned()];
    let mut stems = ending.iter().filter_map(|e| file.strip_suffix(e.as_str()));
    stems.next().unwrap_or(&file).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use std::fs;

    fn sample() -> Project {
        Project {
            network: mock_data::network(),
            arrangement: Arrangement::default(),
        }
    }

    fn saved(project: &Project) -> Vec<u8> {
        let document = Document {
            format: FORMAT.to_owned(),
            version: VERSION,
            project,
        };
        serde_json::to_vec(&document).unwrap()
    }

    #[test]
    fn a_project_comes_back_as_it_was_saved() {
        let folder = std::env::temp_dir().join(format!("rhopper-test-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("home.rhopper.json");
        let project = sample();
        write(&path, &project).unwrap();
        assert_eq!(read(&path), Ok(project));
        write(&path, &Project::default()).unwrap();
        assert_eq!(read(&path), Ok(Project::default()), "replaced");
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn other_files_newer_versions_and_damaged_projects_are_refused() {
        assert_eq!(parse(b"not json"), Err(OpenError::NotProject));
        assert_eq!(parse(b"{\"a\": 1}"), Err(OpenError::NotProject));
        let other = br#"{"format": "other", "version": 1, "project": {}}"#;
        assert_eq!(parse(other), Err(OpenError::NotProject));
        let newer = br#"{"format": "rhopper", "version": 2, "project": {}}"#;
        assert_eq!(parse(newer), Err(OpenError::Newer));
        let empty = br#"{"format": "rhopper", "version": 1, "project": {}}"#;
        assert_eq!(parse(empty), Err(OpenError::Damaged));

        let mut project = sample();
        let first = project.network.devices()[0].id;
        let second = project.network.devices()[1].id;
        project.network.device_mut(second).unwrap().id = first;
        assert_eq!(parse(&saved(&project)), Err(OpenError::Damaged));
    }

    #[test]
    fn text_past_the_limits_is_refused() {
        let mut project = sample();
        let id = project.network.devices()[0].id;
        project.network.device_mut(id).unwrap().notes = "n".repeat(MAX_NOTES_BYTES);
        assert!(parse(&saved(&project)).is_ok(), "notes may be long");
        project.network.device_mut(id).unwrap().name = "n".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(parse(&saved(&project)), Err(OpenError::TooLarge));
        assert_eq!(
            write(Path::new("unused"), &project),
            Err(SaveError::TooLarge)
        );
    }

    #[test]
    fn reading_never_takes_more_than_the_file_limit() {
        let folder = std::env::temp_dir().join(format!("rhopper-limit-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("large.rhopper.json");
        fs::write(&path, [0_u8; 17]).unwrap();
        assert_eq!(read_bounded(&path, 16).unwrap(), None);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn a_project_is_named_after_its_file() {
        assert_eq!(name(Path::new("C:/maps/home.rhopper.json")), "home");
        assert_eq!(name(Path::new("office.json")), "office");
        let path = with_extension(PathBuf::from("lab"));
        assert_eq!(path, PathBuf::from("lab.rhopper.json"));
        let kept = with_extension(PathBuf::from("lab.JSON"));
        assert_eq!(kept, PathBuf::from("lab.JSON"));
    }
}
