//! Connects the AI settings window to [`crate::ai`]: installs models in the background, loads
//! and unloads the one picked, and runs it on the prompt and evidence, so the window stays
//! responsive through downloads of gigabytes and answers that take minutes.

mod generation;
mod install;

use std::rc::Rc;

use slint::{CloseRequestResponse, ComponentHandle, Model as _, ModelRc, SharedString, VecModel};

use crate::ai::{
    self, Answered, Choice, End, Generation, Job, MODELS, RunError, Worker, WorkerEvent,
};
use crate::ui::{AiSettingsWindow, AnswerEnd, AppWindow, ModelState};

/// Opens the window from Settings, bringing back the same window each time.
pub(crate) fn connect(ui: &AppWindow) -> Result<(), slint::PlatformError> {
    let window = AiSettingsWindow::new()?;
    install::connect(&window);
    show_installed(&window);
    restore_texts(&window);
    restore_generation(&window);
    window.set_max_answer_tokens(i32::try_from(ai::MAX_ANSWER_TOKENS).unwrap_or(i32::MAX));
    // A CUDA build uses the GPU unless the user switches it off.
    window.set_gpu_built(ai::gpu_built());
    window.set_use_gpu(ai::gpu_built());
    let restoring = window.as_weak();
    window.on_restore_texts(move || {
        if let Some(open) = restoring.upgrade() {
            restore_texts(&open);
        }
    });
    let resetting = window.as_weak();
    window.on_restore_generation(move || {
        if let Some(open) = resetting.upgrade() {
            restore_generation(&open);
        }
    });
    let refreshing = window.as_weak();
    window.on_refresh(move || {
        if let Some(open) = refreshing.upgrade() {
            show_installed(&open);
        }
    });
    connect_worker(&window);

    // The app runs until its last window closes, so the main window takes this one with it.
    // A download or answer in progress stops with the app; a partial download is never taken
    // as installed.
    let closing = window.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(open) = closing.upgrade() {
            // A failure leaves the window open with nothing left to report it to; the user
            // can still close it.
            open.hide().ok();
        }
        CloseRequestResponse::HideWindow
    });
    ui.on_open_ai_settings(move || {
        show_installed(&window);
        // A failure only means the window did not appear; the main window carries on and
        // Settings can open it again.
        window.show().ok();
    });
    Ok(())
}

/// Loading and running go to one worker, which keeps the loaded model between runs.
fn connect_worker(window: &AiSettingsWindow) {
    let reporting = window.as_weak();
    // Without a data folder nothing can be installed, so every load reports Missing.
    let dir = ai::models_dir().unwrap_or_default();
    let worker = Rc::new(Worker::start(dir, move |event| {
        // An error only means the app is closing, so there is no window to update.
        reporting
            .upgrade_in_event_loop(move |w| show_event(&w, event))
            .ok();
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
            let gpu = open.get_use_gpu();
            send(&open, &unloader, Job::Load { model: None, gpu });
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
    let running = window.as_weak();
    window.on_run(move |prompt, input| {
        if let Some(open) = running.upgrade() {
            let (system, user) = (prompt.into(), input.into());
            let settings = generation::from_ui(&open.get_generation());
            let job = Job::Run {
                system,
                user,
                settings,
            };
            send(&open, &worker, job);
        }
    });
}

/// Asks the worker to load the model listed as `name`, on the GPU or the CPU.
fn load(window: &AiSettingsWindow, worker: &Worker, name: &str, gpu: bool) {
    window.set_can_think(false);
    let Some(model) = choice_named(name) else {
        // The file left the folder after the list was read.
        window.set_loaded_name(SharedString::new());
        window.set_state(ModelState::Missing);
        show_installed(window);
        return;
    };
    window.set_loaded_name(name.into());
    send(
        window,
        worker,
        Job::Load {
            model: Some(model),
            gpu,
        },
    );
}

fn restore_texts(window: &AiSettingsWindow) {
    window.set_prompt(ai::PROMPT.into());
    window.set_input(ai::SAMPLE_EVIDENCE.into());
}

fn restore_generation(window: &AiSettingsWindow) {
    window.set_generation(generation::to_ui(&Generation::default()));
}

fn show_answered(window: &AiSettingsWindow, answered: Answered) {
    window.set_answer_tokens(i32::try_from(answered.tokens).unwrap_or(i32::MAX));
    window.set_answer_seconds(answered.elapsed.as_secs_f32());
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

/// The model the folder now holds under `name`, re-read so a file added or removed since the
/// list was shown is found or reported missing.
fn choice_named(name: &str) -> Option<Choice> {
    let dir = ai::models_dir()?;
    ai::installed(&dir).into_iter().find(|c| c.name() == name)
}

fn show_event(window: &AiSettingsWindow, event: WorkerEvent) {
    match event {
        WorkerEvent::Unloaded => {
            window.set_loaded_name(SharedString::new());
            window.set_can_think(false);
            window.set_state(ModelState::None);
        }
        WorkerEvent::Loading => window.set_state(ModelState::Loading),
        WorkerEvent::Ready { thinks } => {
            window.set_can_think(thinks);
            window.set_state(ModelState::Ready);
        }
        WorkerEvent::Finished(Ok(answered)) => {
            show_answered(window, answered);
            window.set_state(ModelState::Ready);
        }
        // Without a GPU the model stays chosen, so switching CUDA off loads it on the
        // processor.
        WorkerEvent::LoadFailed(RunError::NoGpu) => window.set_state(ModelState::NoGpu),
        WorkerEvent::LoadFailed(e) => {
            window.set_loaded_name(SharedString::new());
            window.set_state(state(e));
        }
        WorkerEvent::Running => {
            window.set_output(SharedString::new());
            window.set_answer_end(AnswerEnd::None);
            window.set_state(ModelState::Running);
        }
        WorkerEvent::Output(text) => window.set_output(text.into()),
        WorkerEvent::Finished(Err(e)) => window.set_state(state(e)),
    }
}

fn state(error: RunError) -> ModelState {
    match error {
        RunError::NoModel => ModelState::NoModel,
        RunError::Missing => ModelState::Missing,
        RunError::Broken => ModelState::Broken,
        RunError::Unsupported => ModelState::Unsupported,
        RunError::NoGpu => ModelState::NoGpu,
        RunError::TooLong => ModelState::TooLong,
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
