//! Connects the AI settings window to [`crate::ai`]: shows what is installed and runs Install AI
//! in the background, so the window stays responsive during a download of gigabytes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use slint::{CloseRequestResponse, ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::ai::{self, InstallError, MODELS};
use crate::ui::{AiSettingsWindow, AppWindow, InstallOutcome};

/// Opens the window from Settings, bringing back the same window each time.
pub(crate) fn connect(ui: &AppWindow) -> Result<(), slint::PlatformError> {
    let window = AiSettingsWindow::new()?;
    if let Some(model) = MODELS.first() {
        window.set_offered_name(model.name.into());
        window.set_offered_gb(gigabytes(model.size()));
    }
    // An empty folder disables Install AI.
    let folder = ai::models_dir();
    window.set_folder(
        folder
            .as_deref()
            .map(|d| d.display().to_string())
            .unwrap_or_default()
            .into(),
    );

    let cancel = Arc::new(AtomicBool::new(false));
    let (weak, flag) = (window.as_weak(), Arc::clone(&cancel));
    window.on_install(move || start_install(&weak, &flag));
    window.on_cancel_install(move || cancel.store(true, Ordering::Relaxed));

    // The app runs until its last window closes, so the main window takes this one with it.
    // A download in progress stops with the app; its partial file is never taken as installed.
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

/// Re-reads the models folder, which may have changed outside the app.
fn show_installed(window: &AiSettingsWindow) {
    let installed = ai::models_dir()
        .map(|d| ai::installed(&d))
        .unwrap_or_default();
    let names: Vec<SharedString> = installed.iter().map(|m| m.name.into()).collect();
    window.set_offered_installed(
        MODELS
            .first()
            .is_some_and(|m| installed.iter().any(|i| std::ptr::eq(*i, m))),
    );
    window.set_installed(ModelRc::new(VecModel::from(names)));
}

fn start_install(weak: &Weak<AiSettingsWindow>, cancel: &Arc<AtomicBool>) {
    let (Some(window), Some(model)) = (weak.upgrade(), MODELS.first()) else {
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
