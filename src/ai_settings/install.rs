//! Install AI: lists the offered models and downloads the one picked in the background, so the
//! window stays responsive through downloads of gigabytes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::ai::{self, InstallError, MODELS};
use crate::ui::{AiSettingsWindow, InstallOutcome};

pub(super) fn connect(window: &AiSettingsWindow) {
    let names: Vec<SharedString> = MODELS.iter().map(|m| m.name.into()).collect();
    let sizes: Vec<f32> = MODELS.iter().map(|m| gigabytes(m.size())).collect();
    window.set_offered_names(ModelRc::new(VecModel::from(names)));
    window.set_offered_gb(ModelRc::new(VecModel::from(sizes)));
    // An empty folder disables Install AI.
    let folder = ai::models_dir().map(|d| d.display().to_string());
    window.set_folder(folder.unwrap_or_default().into());

    let cancel = Arc::new(AtomicBool::new(false));
    let (weak, flag) = (window.as_weak(), Arc::clone(&cancel));
    window.on_install(move |index| start_install(&weak, &flag, index));
    window.on_cancel_install(move || cancel.store(true, Ordering::Relaxed));
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
            super::show_installed(&w);
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
