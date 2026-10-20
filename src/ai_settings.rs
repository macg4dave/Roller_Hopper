//! Connects the AI settings window to [`crate::ai`]: installs models in the background, loads
//! the one picked, and runs it on the prompt and evidence, so the window stays responsive
//! through downloads of gigabytes and answers that take minutes.

mod generation;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use slint::{
    CloseRequestResponse, ComponentHandle, Model as _, ModelRc, SharedString, VecModel, Weak,
};

use crate::ai::{
    self, Answered, End, Generation, InstallError, Job, MODELS, Model, RunError, Worker,
    WorkerEvent,
};
use crate::ui::{AiSettingsWindow, AnswerEnd, AppWindow, InstallOutcome, ModelState};

/// Opens the window from Settings, bringing back the same window each time.
pub(crate) fn connect(ui: &AppWindow) -> Result<(), slint::PlatformError> {
    let window = AiSettingsWindow::new()?;
    show_offers(&window);
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

    let cancel = Arc::new(AtomicBool::new(false));
    let (weak, flag) = (window.as_weak(), Arc::clone(&cancel));
    window.on_install(move |index| start_install(&weak, &flag, index));
    window.on_cancel_install(move || cancel.store(true, Ordering::Relaxed));
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
            let model = installed_model(&open, index);
            open.set_can_think(model.is_some_and(Model::thinks));
            let gpu = open.get_use_gpu();
            send(&open, &loader, Job::Load { model, gpu });
        }
    });
    // The picked model, if any, reloads on the other device.
    let (moving, mover) = (window.as_weak(), Rc::clone(&worker));
    window.on_set_gpu(move |gpu| {
        if let Some(open) = moving.upgrade()
            && let Some(model) = installed_model(&open, open.get_loaded_index())
        {
            let model = Some(model);
            send(&open, &mover, Job::Load { model, gpu });
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

/// The model the window lists at `index` among the installed ones.
fn installed_model(window: &AiSettingsWindow, index: i32) -> Option<&'static Model> {
    let name = window
        .get_installed()
        .row_data(usize::try_from(index).ok()?)?;
    MODELS.iter().find(|m| m.name == name.as_str())
}

fn show_event(window: &AiSettingsWindow, event: WorkerEvent) {
    match event {
        WorkerEvent::Unloaded => window.set_state(ModelState::None),
        WorkerEvent::Loading => window.set_state(ModelState::Loading),
        WorkerEvent::Ready => window.set_state(ModelState::Ready),
        WorkerEvent::Finished(Ok(answered)) => {
            show_answered(window, answered);
            window.set_state(ModelState::Ready);
        }
        // Without a GPU the pick stays, so switching CUDA off loads it on the processor.
        WorkerEvent::LoadFailed(RunError::NoGpu) => window.set_state(ModelState::NoGpu),
        WorkerEvent::LoadFailed(e) => {
            window.set_loaded_index(-1);
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
        RunError::NoGpu => ModelState::NoGpu,
        RunError::TooLong => ModelState::TooLong,
        RunError::Failed => ModelState::Failed,
    }
}

fn show_offers(window: &AiSettingsWindow) {
    let names: Vec<SharedString> = MODELS.iter().map(|m| m.name.into()).collect();
    let sizes: Vec<f32> = MODELS.iter().map(|m| gigabytes(m.size())).collect();
    window.set_offered_names(ModelRc::new(VecModel::from(names)));
    window.set_offered_gb(ModelRc::new(VecModel::from(sizes)));
    // An empty folder disables Install AI.
    let folder = ai::models_dir().map(|d| d.display().to_string());
    window.set_folder(folder.unwrap_or_default().into());
    show_installed(window);
}

/// Re-reads the models folder, which may have changed outside the app, keeping the loaded
/// model picked in the list.
fn show_installed(window: &AiSettingsWindow) {
    let loaded = installed_model(window, window.get_loaded_index());
    let installed = ai::models_dir()
        .map(|d| ai::installed(&d))
        .unwrap_or_default();
    let flags: Vec<bool> = MODELS
        .iter()
        .map(|m| installed.iter().any(|i| std::ptr::eq(*i, m)))
        .collect();
    let names: Vec<SharedString> = installed.iter().map(|m| m.name.into()).collect();
    let index = loaded
        .and_then(|l| installed.iter().position(|i| std::ptr::eq(*i, l)))
        .and_then(|i| i32::try_from(i).ok())
        .unwrap_or(-1);
    window.set_offered_installed(ModelRc::new(VecModel::from(flags)));
    window.set_installed(ModelRc::new(VecModel::from(names)));
    window.set_loaded_index(index);
}

fn start_install(weak: &Weak<AiSettingsWindow>, cancel: &Arc<AtomicBool>, index: i32) {
    let model = usize::try_from(index).ok().and_then(|i| MODELS.get(i));
    let (Some(window), Some(model)) = (weak.upgrade(), model) else {
        return;
    };
    let Some(dir) = ai::models_dir() else {
        window.set_outcome(InstallOutcome::NoFolder);
        return;
    };
    cancel.store(false, Ordering::Relaxed);
    window.set_installing(true);
    window.set_progress(0.0);
    window.set_outcome(InstallOutcome::None);

    let (weak, cancel) = (weak.clone(), Arc::clone(cancel));
    // Detached: the app's close ends it, and nothing waits for it.
    thread::spawn(move || {
        let total = model.size().max(1);
        let mut shown = 0;
        let result = ai::install(model, &dir, &cancel, &mut |done| {
            // Thousandths, so the window is not flooded with one update per read.
            let step = done.saturating_mul(1000) / total;
            if step != shown {
                shown = step;
                let fraction = thousandths(step);
                // An error only means the app is closing, so there is no window to update.
                weak.upgrade_in_event_loop(move |w| w.set_progress(fraction))
                    .ok();
            }
        });
        // As above: if the app is closing, nobody needs the outcome.
        weak.upgrade_in_event_loop(move |w| {
            w.set_installing(false);
            w.set_outcome(outcome(result));
            show_installed(&w);
        })
        .ok();
    });
}

fn outcome(result: Result<(), InstallError>) -> InstallOutcome {
    match result {
        Ok(()) => InstallOutcome::Done,
        Err(InstallError::Network) => InstallOutcome::Network,
        Err(InstallError::Disk) => InstallOutcome::Disk,
        Err(InstallError::Corrupt) => InstallOutcome::Corrupt,
        Err(InstallError::Cancelled) => InstallOutcome::Cancelled,
    }
}

/// Exact for anything up to a thousand, so for every progress step.
fn thousandths(step: u64) -> f32 {
    f32::from(u16::try_from(step.min(1000)).unwrap_or(1000)) / 1000.0
}

/// Decimal gigabytes, as download sizes are shown; a display value, so rounding is fine.
fn gigabytes(bytes: u64) -> f32 {
    f32::from(u16::try_from(bytes / 1_000_000).unwrap_or(u16::MAX)) / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_and_sizes_are_shown_as_fractions_and_gigabytes() {
        assert!((thousandths(250) - 0.25).abs() < f32::EPSILON);
        assert!((thousandths(5000) - 1.0).abs() < f32::EPSILON);
        assert!((gigabytes(2_508_702_910) - 2.508).abs() < 1e-6);
    }
}
