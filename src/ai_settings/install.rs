//! Install AI: lists the offered models and downloads the one picked in the background, so the
//! windows stay responsive through downloads of gigabytes. The download asked for when Import
//! with AI found no model is the same one: there is only ever one, shown in the AI settings
//! window and on the main window's status bar.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::ai::{self, InstallError, MODELS, Model};
use crate::ui::{AiSettingsWindow, AppWindow, Bridge, InstallOutcome};

pub(super) fn connect(window: &AiSettingsWindow, ui: &AppWindow) {
    let names: Vec<SharedString> = MODELS.iter().map(|m| m.name.into()).collect();
    let sizes: Vec<f32> = MODELS.iter().map(|m| gigabytes(m.size())).collect();
    let bridge = ui.global::<Bridge<'_>>();
    // Import with AI offers the main application AI, the first offered.
    bridge.set_ai_download_gb(sizes.first().copied().unwrap_or_default());
    window.set_offered_names(ModelRc::new(VecModel::from(names)));
    window.set_offered_gb(ModelRc::new(VecModel::from(sizes)));
    // An empty folder disables Install AI.
    let folder = ai::models_dir().map(|d| d.display().to_string());
    window.set_folder(folder.unwrap_or_default().into());

    let installer = Installer {
        window: window.as_weak(),
        ui: ui.as_weak(),
        cancel: Arc::new(AtomicBool::new(false)),
    };
    // Either window cancels the one download.
    let cancel_in_settings = Arc::clone(&installer.cancel);
    window.on_cancel_install(move || cancel_in_settings.store(true, Ordering::Relaxed));
    let cancel_in_status = Arc::clone(&installer.cancel);
    bridge.on_cancel_ai_download(move || cancel_in_status.store(true, Ordering::Relaxed));
    let from_settings = installer.clone();
    window.on_install(move |index| {
        let model = usize::try_from(index).ok().and_then(|i| MODELS.get(i));
        from_settings.start(model, false);
    });
    bridge.on_download_ai(move || installer.start(MODELS.first(), true));
}

/// Starts downloads, reporting to both windows.
#[derive(Clone)]
struct Installer {
    window: Weak<AiSettingsWindow>,
    ui: Weak<AppWindow>,
    cancel: Arc<AtomicBool>,
}

impl Installer {
    /// Downloads `model` unless a download is already running; `announce` tells the main
    /// window how it ended, unless the user cancelled it.
    fn start(&self, model: Option<&'static Model>, announce: bool) {
        let (Some(window), Some(model)) = (self.window.upgrade(), model) else {
            return;
        };
        // Both windows ask for the same files, so a second download would write over the first.
        if window.get_installing() {
            return;
        }
        let Some(dir) = ai::models_dir() else {
            window.set_outcome(InstallOutcome::NoFolder);
            if announce {
                self.announce(InstallOutcome::NoFolder);
            }
            return;
        };
        self.cancel.store(false, Ordering::Relaxed);
        window.set_installing(true);
        window.set_progress(0.0);
        window.set_outcome(InstallOutcome::None);
        self.show_progress(true, 0.0);

        let installer = self.clone();
        // Detached: the app's close ends it, and nothing waits for it.
        thread::spawn(move || {
            let total = model.size().max(1);
            let mut shown = 0;
            let result = ai::install(model, &dir, &installer.cancel, &mut |done| {
                // Thousandths, so the windows are not flooded with one update per read.
                let step = done.saturating_mul(1000) / total;
                if step != shown {
                    shown = step;
                    installer.show_progress(true, thousandths(step));
                }
            });
            installer.finish(outcome(result), announce);
        });
    }

    /// Shows how far the download has got in both windows. From any thread; an error only
    /// means the app is closing, so there is no window to update.
    fn show_progress(&self, downloading: bool, fraction: f32) {
        self.window
            .upgrade_in_event_loop(move |w| w.set_progress(fraction))
            .ok();
        self.ui
            .upgrade_in_event_loop(move |ui| {
                let bridge = ui.global::<Bridge<'_>>();
                bridge.set_ai_downloading(downloading);
                bridge.set_ai_download_progress(fraction);
            })
            .ok();
    }

    fn finish(&self, outcome: InstallOutcome, announce: bool) {
        self.show_progress(false, 0.0);
        // As above: if the app is closing, nobody needs the outcome.
        self.window
            .upgrade_in_event_loop(move |w| {
                w.set_installing(false);
                w.set_outcome(outcome);
                super::show_installed(&w);
            })
            .ok();
        if announce && outcome != InstallOutcome::Cancelled {
            self.announce(outcome);
        }
    }

    /// Tells the main window how the download asked for from Import with AI ended.
    fn announce(&self, outcome: InstallOutcome) {
        // As above.
        self.ui
            .upgrade_in_event_loop(move |ui| {
                ui.global::<Bridge<'_>>().set_ai_download_outcome(outcome);
            })
            .ok();
    }
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
