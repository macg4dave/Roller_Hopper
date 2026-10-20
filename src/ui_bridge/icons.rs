//! The icon folder. Every SVG file in it, or in one of its subfolders, can be picked as a device
//! or role icon; a subfolder's name is shown as a heading over its icons. The folder is read when
//! the app starts and again whenever the icon picker opens, so icons dropped in while the app
//! runs show up without a restart.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use slint::{Image, ModelRc, SharedString, VecModel};

use crate::model::Icon;
use crate::ui;

const FOLDER: &str = "icons";

/// Where the icons are: an `icons` folder beside the program, or the one in the source tree
/// when there is none (as under `cargo run`).
pub(super) fn folder() -> PathBuf {
    let beside_program = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(FOLDER)));
    beside_program
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join(FOLDER))
}

/// The icons found in the folder, loaded and ready to show.
#[derive(Default)]
pub(super) struct Icons {
    /// Headings (empty for the folder itself) and their icons, in the order shown.
    groups: Vec<(String, Vec<Icon>)>,
    images: HashMap<Icon, Image>,
}

impl Icons {
    pub(super) fn load(root: &Path) -> Self {
        let mut images = HashMap::new();
        let groups = find(root)
            .into_iter()
            .map(|(heading, files)| {
                let icons = files.into_iter().filter_map(|(icon, path)| {
                    // A file that is not a usable SVG is left out rather than shown blank.
                    let image = Image::load_from_path(&path).ok()?;
                    images.insert(icon.clone(), image);
                    Some(icon)
                });
                (heading, icons.collect())
            })
            .collect();
        Self { groups, images }
    }

    /// The icon's picture; an empty image if its file is gone, which the UI shows as Unknown.
    pub(super) fn image(&self, icon: &Icon) -> Image {
        self.images.get(icon).cloned().unwrap_or_default()
    }

    pub(super) fn choice(&self, icon: &Icon) -> ui::IconChoice {
        ui::IconChoice {
            id: icon.0.as_str().into(),
            label: label(icon).into(),
            image: self.image(icon),
        }
    }

    /// Every icon for the picker, under its heading.
    pub(super) fn groups(&self) -> Vec<ui::IconGroup> {
        let groups = self.groups.iter().filter(|(_, icons)| !icons.is_empty());
        let groups = groups.map(|(heading, icons)| {
            let choices: Vec<_> = icons.iter().map(|icon| self.choice(icon)).collect();
            ui::IconGroup {
                heading: SharedString::from(heading.as_str()),
                icons: ModelRc::new(VecModel::from(choices)),
            }
        });
        groups.collect()
    }
}

/// An icon's name for people: its file name, with `-` and `_` read as spaces.
fn label(icon: &Icon) -> String {
    let name = icon.0.rsplit('/').next().unwrap_or_default();
    name.replace(['-', '_'], " ")
}

/// The SVG files directly in `dir`, by name, ignoring case.
fn svg_files(dir: &Path, prefix: &str) -> Vec<(Icon, PathBuf)> {
    let mut files: Vec<_> = entries(dir)
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        })
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?;
            Some((Icon(format!("{prefix}{stem}")), path))
        })
        .collect();
    files.sort_by_key(|(icon, _)| icon.0.to_lowercase());
    files
}

/// The folder's own icons under no heading, then each subfolder's under its name.
fn find(root: &Path) -> Vec<(String, Vec<(Icon, PathBuf)>)> {
    let mut folders: Vec<(String, PathBuf)> = entries(root)
        .filter(|path| path.is_dir())
        .filter_map(|path| Some((path.file_name()?.to_str()?.to_owned(), path)))
        .collect();
    folders.sort_by_key(|(name, _)| name.to_lowercase());
    let subfolders = folders.into_iter().map(|(name, path)| {
        let files = svg_files(&path, &format!("{name}/"));
        (name, files)
    });
    std::iter::once((String::new(), svg_files(root, "")))
        .chain(subfolders)
        .collect()
}

/// The paths in `dir`. A folder that cannot be read simply offers no icons.
fn entries(dir: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch icon folder, removed again when its test passes.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("network-mapper-{name}"));
            // Left behind by an earlier failed run.
            if dir.exists() {
                fs::remove_dir_all(&dir).unwrap();
            }
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self, path: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "<svg/>").unwrap();
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // A failed test keeps its folder to look at; `new` clears it on the next run.
            if !std::thread::panicking() {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
    }

    fn names(found: &[(String, Vec<(Icon, PathBuf)>)]) -> Vec<(&str, Vec<&str>)> {
        let names = found.iter().map(|(heading, files)| {
            let icons = files.iter().map(|(icon, _)| icon.0.as_str()).collect();
            (heading.as_str(), icons)
        });
        names.collect()
    }

    #[test]
    fn subfolders_become_headings_and_only_svg_files_count() {
        let scratch = Scratch::new("icons-find");
        scratch.file("Zebra.svg");
        scratch.file("aardvark.SVG");
        scratch.file("Network/Router.svg");
        scratch.file("Network/readme.txt");
        scratch.file("garden/Sprinkler.svg");
        scratch.file("garden/deeper/Hidden.svg");
        assert_eq!(
            names(&find(&scratch.0)),
            vec![
                ("", vec!["aardvark", "Zebra"]),
                ("garden", vec!["garden/Sprinkler"]),
                ("Network", vec!["Network/Router"]),
            ]
        );
    }

    #[test]
    fn the_shipped_icons_load_and_include_every_detected_one() {
        let icons = Icons::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join(FOLDER));
        let shipped: usize = icons.groups.iter().map(|(_, icons)| icons.len()).sum();
        assert_eq!(icons.images.len(), shipped, "every file is a usable SVG");
        let kind = crate::model::DeviceKind::AccessPoint;
        assert!(icons.image(&kind.icon()).size().width > 0);
        assert_eq!(icons.image(&Icon::new("Gone/Missing")).size().width, 0);
    }

    #[test]
    fn a_missing_folder_offers_no_icons() {
        let found = find(Path::new("no such folder"));
        assert!(found.iter().all(|(_, files)| files.is_empty()));
    }

    #[test]
    fn labels_come_from_file_names() {
        assert_eq!(label(&Icon::new("Network/Access point")), "Access point");
        assert_eq!(label(&Icon::new("smart-plug_mini")), "smart plug mini");
    }
}
