//! The project file: New, Open, Save and Save As in the header, the project's name and whether
//! it has unsaved changes, and asking to save them before they would be lost (New, Open, or
//! closing the window). The file format is [`crate::project`]; the dialogs are the system's.

use std::cell::RefCell;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{CloseRequestResponse, ComponentHandle, Weak};

use super::{AppState, handler, update};
use crate::project::{self, OpenError, Project, SaveError};
use crate::ui::{self, AppWindow, Bridge};

/// What waits on the answer to "save changes first?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Then {
    New,
    Open,
    Close,
}

/// A file operation that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Problem {
    Open(OpenError),
    Save(SaveError),
}

impl AppState {
    fn project(&self) -> Project {
        Project {
            network: self.network.clone(),
            arrangement: self.arrangement.clone(),
        }
    }

    /// Whether the project has changed since it was opened or last saved.
    fn unsaved(&self) -> bool {
        self.network != self.saved.network || self.arrangement != self.saved.arrangement
    }

    /// Whether `then` can go ahead now. With unsaved changes it waits while the user is asked
    /// whether to save them first.
    fn ask_first(&mut self, then: Then) -> bool {
        if self.unsaved() {
            self.unsaved_question = Some(then);
        }
        self.unsaved_question.is_none()
    }

    /// Replaces the project, starting afresh. The icons, and how the discovery page scans, stay.
    fn replace_project(&mut self, project: Project, file: Option<PathBuf>) {
        let icons = std::mem::take(&mut self.icons);
        let target_text = std::mem::take(&mut self.discovery.target_text);
        let options = self.discovery.options;
        *self = Self::with_project(project, file);
        self.icons = icons;
        self.discovery.target_text = target_text;
        self.discovery.options = options;
    }

    /// Opens the project at `path`; one that cannot be read leaves the current project as it is.
    fn open_from(&mut self, path: PathBuf) {
        match project::read(&path) {
            Ok(opened) => self.replace_project(opened, Some(path)),
            Err(error) => self.file_problem = Some(Problem::Open(error)),
        }
    }

    /// Saves the project at `path`, where it is saved from then on. False if that failed.
    fn save_to(&mut self, path: PathBuf) -> bool {
        let project = self.project();
        match project::write(&path, &project) {
            Ok(()) => {
                self.saved = project;
                self.file = Some(path);
                true
            }
            Err(error) => {
                self.file_problem = Some(Problem::Save(error));
                false
            }
        }
    }
}

/// The header's file buttons, the unsaved-changes question and the window's close button.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    let wire = || (ui.as_weak(), Rc::clone(state));
    let (new_ui, new_state) = wire();
    bridge.on_new_project(move || request(&new_ui, &new_state, Then::New));
    let (open_ui, open_state) = wire();
    bridge.on_open_project(move || request(&open_ui, &open_state, Then::Open));
    let (save_ui, save_state) = wire();
    bridge.on_save_project(move || save(&save_ui, &save_state, false, None));
    let (as_ui, as_state) = wire();
    bridge.on_save_project_as(move || save(&as_ui, &as_state, true, None));
    let (answer_ui, answer_state) = wire();
    bridge.on_answer_unsaved(move |reply| answer(&answer_ui, &answer_state, reply));
    let dismiss = handler(ui, state, |st, ()| st.file_problem = None);
    bridge.on_dismiss_file_problem(move || dismiss(()));

    let (close_ui, close_state) = wire();
    ui.window().on_close_requested(move || {
        let mut close = true;
        update(&close_ui, &close_state, |st| {
            close = st.ask_first(Then::Close);
        });
        if close {
            CloseRequestResponse::HideWindow
        } else {
            CloseRequestResponse::KeepWindowShown
        }
    });
}

/// Starts `then`, or asks about unsaved changes first. A new project or another one waits
/// for a running scan to end, since its results belong to the project it started in.
fn request(ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>, then: Then) {
    let mut go = false;
    update(ui, state, |st| {
        go = !st.discovery.busy() && st.ask_first(then);
    });
    if go {
        proceed(ui, state, then);
    }
}

fn answer(ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>, reply: ui::UnsavedAnswer) {
    let mut then = None;
    update(ui, state, |st| then = st.unsaved_question.take());
    let Some(then) = then else { return };
    match reply {
        ui::UnsavedAnswer::Save => save(ui, state, false, Some(then)),
        ui::UnsavedAnswer::Discard => proceed(ui, state, then),
        ui::UnsavedAnswer::Cancel => {}
    }
}

fn proceed(ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>, then: Then) {
    match then {
        Then::New => update(ui, state, |st| st.replace_project(Project::default(), None)),
        Then::Open => open(ui, state),
        // Fails only once the event loop has stopped, when there is nothing left to close.
        Then::Close => slint::quit_event_loop().unwrap_or_default(),
    }
}

fn open(ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>) {
    let Some(window) = ui.upgrade() else { return };
    let dialog = dialog(&window);
    let (ui, state) = (ui.clone(), Rc::clone(state));
    spawn(async move {
        if let Some(file) = dialog.pick_file().await {
            update(&ui, &state, |st| st.open_from(file.path().to_owned()));
        }
    });
}

/// Saves where the project was opened from or last saved, or where the user picks (always,
/// when `choose`); then goes on with `then`, if the save worked.
fn save(ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>, choose: bool, then: Option<Then>) {
    let file = state.borrow().file.clone();
    if let Some(path) = file.clone().filter(|_| !choose) {
        save_then(ui, state, path, then);
        return;
    }
    let Some(window) = ui.upgrade() else { return };
    let mut dialog = dialog(&window);
    if let Some(name) = file.as_deref().and_then(Path::file_name) {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    let (ui, state) = (ui.clone(), Rc::clone(state));
    spawn(async move {
        if let Some(picked) = dialog.save_file().await {
            save_then(
                &ui,
                &state,
                project::with_extension(picked.path().to_owned()),
                then,
            );
        }
    });
}

fn save_then(
    ui: &Weak<AppWindow>,
    state: &Rc<RefCell<AppState>>,
    path: PathBuf,
    then: Option<Then>,
) {
    let mut saved = false;
    update(ui, state, |st| saved = st.save_to(path));
    if let Some(then) = then.filter(|_| saved) {
        proceed(ui, state, then);
    }
}

/// The system's file dialog over the window, showing project files.
fn dialog(window: &AppWindow) -> rfd::AsyncFileDialog {
    let kind = window.global::<Bridge<'_>>().get_project_file_type();
    rfd::AsyncFileDialog::new()
        .set_parent(&window.window().window_handle())
        .add_filter(kind.as_str(), &[project::EXTENSION, "json"])
}

/// Runs a dialog's task on the event loop.
fn spawn(task: impl Future<Output = ()> + 'static) {
    // Fails only when no event loop runs, and then no dialog could be shown anyway.
    slint::spawn_local(task).map(drop).unwrap_or_default();
}

/// The project's name, whether it has unsaved changes, and any question or problem to show.
pub(super) fn push(bridge: &Bridge<'_>, state: &AppState) {
    let name = state.file.as_deref().map(project::name).unwrap_or_default();
    bridge.set_project_name(name.into());
    bridge.set_unsaved(state.unsaved());
    bridge.set_unsaved_question(state.unsaved_question.is_some());
    bridge.set_file_problem(problem_view(state.file_problem.as_ref()));
}

fn problem_view(problem: Option<&Problem>) -> ui::FileProblem {
    let (kind, detail) = match problem {
        None => (ui::FileProblemKind::None, String::new()),
        Some(Problem::Open(OpenError::Unreadable(reason))) => {
            (ui::FileProblemKind::Unreadable, reason.clone())
        }
        Some(Problem::Open(OpenError::TooLarge)) => (ui::FileProblemKind::TooLarge, String::new()),
        Some(Problem::Open(OpenError::NotProject)) => {
            (ui::FileProblemKind::NotProject, String::new())
        }
        Some(Problem::Open(OpenError::Newer)) => (ui::FileProblemKind::Newer, String::new()),
        Some(Problem::Open(OpenError::Damaged)) => (ui::FileProblemKind::Damaged, String::new()),
        Some(Problem::Save(SaveError(reason))) => (ui::FileProblemKind::SaveFailed, reason.clone()),
    };
    ui::FileProblem {
        kind,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::Network;

    fn temp(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("rhopper-bridge-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        folder.join(name)
    }

    #[test]
    fn a_new_project_starts_saved_and_any_change_waits_on_the_question() {
        let mut state = AppState::new(Network::default());
        assert!(!state.unsaved());
        assert!(state.ask_first(Then::New), "nothing to lose");
        state.network = mock_data::network();
        assert!(state.unsaved());
        assert!(!state.ask_first(Then::Open));
        assert_eq!(state.unsaved_question, Some(Then::Open));
    }

    #[test]
    fn saving_clears_the_unsaved_mark_and_opening_brings_the_project_back() {
        let path = temp("saved.rhopper.json");
        let mut state = AppState::new(mock_data::network());
        state.network = Network::default();
        assert!(state.save_to(path.clone()));
        assert!(!state.unsaved());
        assert_eq!(state.file.as_deref(), Some(path.as_path()));

        let mut other = AppState::new(mock_data::network());
        other.discovery.target_text = "10.0.0.0/24".into();
        other.open_from(path.clone());
        assert_eq!(other.network, Network::default());
        assert_eq!(other.file, Some(path.clone()));
        assert!(!other.unsaved());
        assert_eq!(other.discovery.target_text, "10.0.0.0/24", "kept");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_file_that_cannot_be_opened_leaves_the_project_and_says_why() {
        let path = temp("not-a-project.json");
        std::fs::write(&path, "[1, 2, 3]").unwrap();
        let mut state = AppState::new(mock_data::network());
        state.open_from(path.clone());
        assert_eq!(state.network, mock_data::network());
        assert_eq!(
            state.file_problem,
            Some(Problem::Open(OpenError::NotProject))
        );
        std::fs::remove_file(path).unwrap();
    }
}
