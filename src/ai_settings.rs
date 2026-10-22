//! Connects the AI settings window to [`crate::ai`]: installs models in the background, loads
//! and unloads the one picked, and runs it on the prompt and a host's evidence, which its
//! sidebar ([`scan`]) scans for, so the window stays responsive through downloads of gigabytes
//! and answers that take minutes.
//!
//! The window starts the app's one AI worker, which Import with AI shares: the model's loading
//! and unloading show here whoever asked for them, and each run's events go to whoever asked.

mod generation;
mod install;
mod scan;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::Sender;

use slint::{ComponentHandle, Model as _, ModelRc, SharedString, VecModel};

use crate::ai::{
    self, Answered, Asker, End, Generation, Job, MODELS, RunError, Saved, Worker, WorkerEvent,
};
use crate::ui::{AiSettingsWindow, AnswerEnd, AppWindow, ModelState, SaveOutcome};

/// Opens the window from Settings, bringing back the same window each time. Returns the shared
/// worker; the events of Import with AI's runs, and of loading and unloading, go to `import`.
pub(crate) fn connect(
    ui: &AppWindow,
    import: Sender<WorkerEvent>,
) -> Result<Rc<Worker>, slint::PlatformError> {
    let window = AiSettingsWindow::new()?;
    install::connect(&window);
    show_installed(&window);
    scan::connect(&window);
    window.set_max_context_tokens(i32::try_from(ai::MAX_CONTEXT_TOKENS).unwrap_or(i32::MAX));
    window.set_gpu_built(ai::gpu_built());
    show_saved(&window);
    let restoring = window.as_weak();
    window.on_restore_prompt(move || {
        if let Some(open) = restoring.upgrade() {
            open.set_prompt(ai::PROMPT.into());
        }
    });
    let saving = window.as_weak();
    window.on_save(move || {
        if let Some(open) = saving.upgrade() {
            save(&open);
        }
    });
    let resetting = window.as_weak();
    window.on_restore_generation(move || {
        if let Some(open) = resetting.upgrade() {
            restore_generation(&open);
        }
    });
    let thinking = window.as_weak();
    window.on_select_thinking(move |enabled| {
        if let Some(open) = thinking.upgrade() {
            open.set_generation(generation::to_ui(&Generation::qwen3(enabled)));
        }
    });
    let refreshing = window.as_weak();
    window.on_refresh(move || {
        if let Some(open) = refreshing.upgrade() {
            show_installed(&open);
        }
    });
    let worker = connect_worker(&window, import);
    // Closing the main window ends the app with this window (`ui_bridge::project`). A download
    // or answer in progress stops with the app; a partial download is never taken as installed.
    ui.on_open_ai_settings(move || {
        show_installed(&window);
        // A failure only means the window did not appear; the main window carries on and
        // Settings can open it again.
        window.show().ok();
    });
    Ok(worker)
}

/// The run an event belongs to, if it belongs to one.
fn asker(event: &WorkerEvent) -> Option<Asker> {
    match event {
        WorkerEvent::Running(asker)
        | WorkerEvent::Output(asker, _)
        | WorkerEvent::Finished(asker, _) => Some(*asker),
        WorkerEvent::Unloaded
        | WorkerEvent::Loading
        | WorkerEvent::Ready(_)
        | WorkerEvent::LoadFailed(_) => None,
    }
}

/// Loading and running go to one worker, which keeps the loaded model between runs until it
/// has been idle for [`ai::IDLE_UNLOAD`].
fn connect_worker(window: &AiSettingsWindow, import: Sender<WorkerEvent>) -> Rc<Worker> {
    let reporting = window.as_weak();
    let worker = Rc::new(Worker::start(ai::IDLE_UNLOAD, move |event| {
        let asker = asker(&event);
        if asker != Some(Asker::Trial) {
            // An error only means the app is closing, with no import left to tell.
            import.send(event.clone()).ok();
        }
        if asker != Some(Asker::Import) {
            // An error only means the app is closing, so there is no window to update.
            reporting
                .upgrade_in_event_loop(move |w| show_event(&w, event))
                .ok();
        }
    }));

    let (loading, loader) = (window.as_weak(), Rc::clone(&worker));
    window.on_load(move |index| {
        if let Some(open) = loading.upgrade() {
            let name = usize::try_from(index)
                .ok()
                .and_then(|i| open.get_models().row_data(i))
                .unwrap_or_default();
            load(&open, &loader, &name, open.get_use_gpu());
        }
    });
    let (unloading, unloader) = (window.as_weak(), Rc::clone(&worker));
    window.on_unload(move || {
        if let Some(open) = unloading.upgrade() {
            send(&open, &unloader, Job::Unload);
        }
    });
    // The loaded model, if any, reloads on the other device.
    let (moving, mover) = (window.as_weak(), Rc::clone(&worker));
    window.on_set_gpu(move |gpu| {
        if let Some(open) = moving.upgrade() {
            let name = open.get_loaded_name();
            if !name.is_empty() {
                load(&open, &mover, &name, gpu);
            }
        }
    });
    let stopper = Rc::clone(&worker);
    window.on_stop(move || stopper.stop());
    let (running, runner) = (window.as_weak(), Rc::clone(&worker));
    window.on_run(move |prompt, input| {
        if let Some(open) = running.upgrade() {
            let (system, user) = (prompt.into(), input.into());
            let settings = generation::from_ui(&open.get_generation());
            let job = Job::Run {
                asker: Asker::Trial,
                system,
                user,
                settings,
            };
            send(&open, &runner, job);
        }
    });
    worker
}

/// Asks the worker to load the model listed as `name`, on the GPU or the CPU.
fn load(window: &AiSettingsWindow, worker: &Worker, name: &str, gpu: bool) {
    let Some(model) = file_named(name) else {
        // The file left the folder after the list was read.
        window.set_loaded_name(SharedString::new());
        window.set_state(ModelState::Missing);
        show_installed(window);
        return;
    };
    window.set_loaded_name(name.into());
    send(window, worker, Job::Load { model, gpu });
}

/// Shows the saved AI settings: the prompt, answer settings, GPU choice and, picked in the
/// model list, the model last loaded. Nothing loads until the user asks.
fn show_saved(window: &AiSettingsWindow) {
    let saved = Saved::in_data_dir();
    window.set_prompt(saved.prompt().into());
    window.set_generation(generation::to_ui(&saved.generation));
    window.set_use_gpu(saved.gpu);
    let models = window.get_models();
    let at = saved
        .model
        .and_then(|name| models.iter().position(|m| m == name.as_str()));
    if let Some(at) = at.and_then(|i| i32::try_from(i).ok()) {
        window.set_model_index(at);
    }
    window.set_save_outcome(SaveOutcome::None);
}

/// Saves the window's AI settings for next time and for Import with AI, with the model loaded
/// (or else the one picked in the list), saying whether that worked.
fn save(window: &AiSettingsWindow) {
    let loaded = window.get_loaded_name();
    let picked = usize::try_from(window.get_model_index())
        .ok()
        .and_then(|i| window.get_models().row_data(i));
    let model = Some(loaded)
        .filter(|name| !name.is_empty())
        .or(picked)
        .map(String::from);
    let settings = generation::from_ui(&window.get_generation());
    let saved = Saved::new(&window.get_prompt(), settings, window.get_use_gpu(), model);
    let done = ai::data_dir().is_some_and(|dir| saved.save(&dir).is_ok());
    window.set_save_outcome(if done {
        SaveOutcome::Saved
    } else {
        SaveOutcome::Failed
    });
}

fn restore_generation(window: &AiSettingsWindow) {
    window.set_generation(generation::to_ui(&Generation::default()));
}

fn show_answered(window: &AiSettingsWindow, answered: Answered) {
    window.set_answer_tokens(i32::try_from(answered.tokens).unwrap_or(i32::MAX));
    window.set_answer_seconds(answered.elapsed.as_secs_f32());
    window.set_prompt_tokens(i32::try_from(answered.prompt_tokens).unwrap_or(i32::MAX));
    // No answer token came, so the window shows no time to it.
    window.set_first_token_seconds(answered.first_token.map_or(0.0, |t| t.as_secs_f32()));
    window.set_answer_end(match answered.end {
        End::Complete => AnswerEnd::Complete,
        End::Limit => AnswerEnd::Limit,
        End::Stopped => AnswerEnd::Stopped,
    });
}

fn send(window: &AiSettingsWindow, worker: &Worker, job: Job) {
    if !worker.send(job) {
        window.set_state(ModelState::Failed);
    }
}

/// The file of the model the folder now holds under `name`, re-read so a file added or
/// removed since the list was shown is found or reported missing.
fn file_named(name: &str) -> Option<PathBuf> {
    let dir = ai::models_dir()?;
    let model = ai::installed(&dir).into_iter().find(|c| c.name() == name)?;
    Some(model.file(&dir))
}

/// The name the model list shows for the model file at `path`, which Import with AI may have
/// loaded rather than this window.
fn name_of(path: &Path) -> Option<String> {
    let dir = ai::models_dir()?;
    let model = ai::installed(&dir)
        .into_iter()
        .find(|c| c.file(&dir) == path)?;
    Some(model.name())
}

fn show_event(window: &AiSettingsWindow, event: WorkerEvent) {
    match event {
        WorkerEvent::Unloaded => {
            window.set_loaded_name(SharedString::new());
            window.set_state(ModelState::None);
        }
        WorkerEvent::Loading => window.set_state(ModelState::Loading),
        WorkerEvent::Ready(path) => {
            if let Some(name) = name_of(&path) {
                window.set_loaded_name(name.into());
            }
            window.set_state(ModelState::Ready);
        }
        WorkerEvent::Finished(_, Ok(answered)) => {
            show_answered(window, answered);
            window.set_state(ModelState::Ready);
        }
        // When CUDA is unavailable or lacks room, the model stays chosen so switching it off
        // loads the model on the processor.
        WorkerEvent::LoadFailed(error @ (RunError::NoGpu | RunError::InsufficientGpuMemory)) => {
            window.set_state(state(error));
        }
        WorkerEvent::LoadFailed(e) => {
            window.set_loaded_name(SharedString::new());
            window.set_state(state(e));
        }
        WorkerEvent::Running(_) => {
            window.set_output(SharedString::new());
            window.set_answer_end(AnswerEnd::None);
            window.set_state(ModelState::Running);
        }
        WorkerEvent::Output(_, text) => window.set_output(text.into()),
        WorkerEvent::Finished(_, Err(e)) => window.set_state(state(e)),
    }
}

fn state(error: RunError) -> ModelState {
    match error {
        RunError::NoModel => ModelState::NoModel,
        RunError::Missing => ModelState::Missing,
        RunError::Unsupported => ModelState::Unsupported,
        RunError::Broken => ModelState::Broken,
        RunError::NoGpu => ModelState::NoGpu,
        RunError::InsufficientGpuMemory => ModelState::InsufficientGpuMemory,
        RunError::TooLong { .. } => ModelState::TooLong,
        RunError::Failed => ModelState::Failed,
    }
}

/// Re-reads the models folder, which may have changed outside the app, keeping the model
/// picked in the list, or else the loaded one.
fn show_installed(window: &AiSettingsWindow) {
    let dir = ai::models_dir();
    let models = dir.as_deref().map(ai::installed).unwrap_or_default();
    let names: Vec<SharedString> = models.iter().map(|m| m.name().into()).collect();
    let picked = usize::try_from(window.get_model_index())
        .ok()
        .and_then(|i| window.get_models().row_data(i));
    let loaded = window.get_loaded_name();
    let index = picked
        .and_then(|p| names.iter().position(|n| *n == p))
        .or_else(|| names.iter().position(|n| *n == loaded))
        .or_else(|| (!names.is_empty()).then_some(0))
        .and_then(|i| i32::try_from(i).ok())
        .unwrap_or(-1);
    let flags: Vec<bool> = MODELS
        .iter()
        .map(|m| dir.as_deref().is_some_and(|d| m.is_installed(d)))
        .collect();
    window.set_offered_installed(ModelRc::new(VecModel::from(flags)));
    window.set_models(ModelRc::new(VecModel::from(names)));
    window.set_model_index(index);
}
