//! Import with AI on the discovery page: loads the model with the prompt and answer settings
//! saved from the AI settings window ([`crate::ai::Saved`]), asks it about each new host in
//! turn, puts its answer in place of what the scan made of the host
//! ([`crate::ai::Identity::fill`]), and adds the host to the map.
//! The worker is the one the AI settings window started; the model unloads itself once idle
//! ([`crate::ai::IDLE_UNLOAD`]). The queue of hosts is the run's
//! ([`crate::model::Discovery::begin_ai_import`]).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode, Weak};

use super::{AppState, update};
use crate::ai::{self, Asker, End, Identity, Job, RunError, Saved, Worker, WorkerEvent};
use crate::model::AiImportPhase;
use crate::ui::{AppWindow, Bridge};

/// How often the worker's events are taken in during an import.
const POLL: Duration = Duration::from_millis(200);

struct Importing {
    worker: Rc<Worker>,
    /// The worker's loading events and the events of the import's own runs.
    events: Receiver<WorkerEvent>,
    timer: Timer,
    /// The answer so far to the run in flight, if one is.
    answer: RefCell<String>,
    asking: Cell<bool>,
    /// The AI settings saved when the import began.
    settings: RefCell<Saved>,
    /// Whether this GPU can run the AI, once the worker has found out.
    gpu: Cell<Option<bool>>,
}

pub(super) fn connect(
    ui: &AppWindow,
    state: &Rc<RefCell<AppState>>,
    worker: Rc<Worker>,
    events: Receiver<WorkerEvent>,
) {
    let importing = Rc::new(Importing {
        worker,
        events,
        timer: Timer::default(),
        answer: RefCell::default(),
        asking: Cell::new(false),
        settings: RefCell::default(),
        gpu: Cell::new(None),
    });
    let bridge = ui.global::<Bridge<'_>>();
    let (begin_ui, begin_state, begin) = (ui.as_weak(), Rc::clone(state), Rc::clone(&importing));
    bridge.on_import_found_with_ai(move || begin.begin(&begin_ui, &begin_state));
    let (stop_ui, stop_state) = (ui.as_weak(), Rc::clone(state));
    bridge.on_stop_ai_import(move || {
        update(&stop_ui, &stop_state, |st| {
            st.discovery.end_ai_import(AiImportPhase::Stopped);
        });
        importing.end();
    });
}

impl Importing {
    /// Begins importing the run's new hosts with the saved AI settings, loading their model
    /// first; the model already loaded stays. On a GPU that cannot run the AI, says so instead.
    /// With no model installed, asks whether to download AI support; only a system with no
    /// folder to put it in says there is no model.
    fn begin(self: &Rc<Self>, ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>) {
        self.catch_up();
        let Some(window) = ui.upgrade() else { return };
        let bridge = window.global::<Bridge<'_>>();
        if self.gpu.get() == Some(false) {
            bridge.set_gpu_unsupported(true);
            return;
        }
        let settings = Saved::in_data_dir();
        let model = ai::import_model(settings.model.as_deref());
        if model.is_none() && ai::models_dir().is_some() {
            bridge.set_ai_download_question(true);
            return;
        }
        let mut begun = false;
        update(ui, state, |st| {
            begun = st.discovery.begin_ai_import();
            if begun && model.is_none() {
                st.discovery.end_ai_import(AiImportPhase::NoModel);
            }
        });
        let Some(model) = model.filter(|_| begun) else {
            return;
        };
        self.settings.replace(settings);
        if !self.worker.send(Job::Load(model)) {
            update(ui, state, |st| {
                st.discovery.end_ai_import(AiImportPhase::Failed);
            });
            return;
        }
        let (ui, state, this) = (ui.clone(), Rc::clone(state), Rc::downgrade(self));
        self.timer.start(TimerMode::Repeated, POLL, move || {
            if let Some(importing) = this.upgrade() {
                importing.take(&ui, &state);
            }
        });
    }

    /// Takes in the events since the last import. Loading and unloading meanwhile say nothing
    /// about the next one; only whether the GPU can run the AI is kept.
    fn catch_up(&self) {
        for event in self.events.try_iter() {
            if let WorkerEvent::GpuChecked(supported) = event {
                self.gpu.set(Some(supported));
            }
        }
    }

    /// Takes in the worker's events, ending once the import has ended, however it ended.
    fn take(&self, ui: &Weak<AppWindow>, state: &RefCell<AppState>) {
        let events: Vec<WorkerEvent> = self.events.try_iter().collect();
        let mut going = true;
        update(ui, state, |st| {
            for event in events {
                self.event(st, event);
            }
            going = st.discovery.ai_import_busy();
        });
        if !going {
            self.end();
        }
    }

    fn event(&self, st: &mut AppState, event: WorkerEvent) {
        let phase = st.discovery.ai_import().map(|i| i.phase);
        match event {
            WorkerEvent::GpuChecked(supported) => self.gpu.set(Some(supported)),
            WorkerEvent::Ready(_) if phase == Some(AiImportPhase::Loading) => self.ask(st),
            WorkerEvent::LoadFailed(_) if phase == Some(AiImportPhase::Loading) => {
                st.discovery.end_ai_import(AiImportPhase::Failed);
            }
            WorkerEvent::Output(Asker::Import, text) => *self.answer.borrow_mut() = text,
            WorkerEvent::Finished(Asker::Import, result) if self.asking.replace(false) => {
                self.answered(st, result);
            }
            // Another model loading meanwhile, the trial's runs and late answers.
            WorkerEvent::Unloaded
            | WorkerEvent::Loading
            | WorkerEvent::Ready(_)
            | WorkerEvent::LoadFailed(_)
            | WorkerEvent::Running(_)
            | WorkerEvent::Output(Asker::Trial, _)
            | WorkerEvent::Finished(..) => {}
        }
    }

    /// Takes the answer for the host with the model and asks about the next. An answer the
    /// prompt's fields cannot be read from, or a host with too much evidence to send, fills
    /// nothing: the host is added as the scan found it.
    fn answered(&self, st: &mut AppState, result: Result<ai::Answered, RunError>) {
        let identity = match result {
            Ok(answered) if answered.end == End::Stopped => {
                st.discovery.end_ai_import(AiImportPhase::Stopped);
                return;
            }
            Ok(_) => Identity::read(&self.answer.take()).unwrap_or_default(),
            Err(RunError::TooLong { .. }) => Identity::default(),
            Err(_) => {
                st.discovery.end_ai_import(AiImportPhase::Failed);
                return;
            }
        };
        let shown = (st.discovery.selected, st.discovery.selected_card);
        let filled = st
            .discovery
            .identified(|device| identity.fill(device), &mut st.network);
        if filled.is_some() {
            // The inferred topology was rebuilt, so an older Auto Map undo snapshot no longer
            // describes the current network.
            st.auto_map_undo = None;
            st.lay_out_at_rest();
            // The host shown in the details is edited on the map once added.
            st.select_found(shown.0, shown.1);
        }
        self.ask(st);
    }

    /// Sends the next host's evidence to the model; with none left, the import has finished.
    fn ask(&self, st: &mut AppState) {
        let Some(index) = st.discovery.next_for_ai() else {
            return;
        };
        let found = st.discovery.found().get(index);
        let user = found.map(|f| ai::evidence(&f.device)).unwrap_or_default();
        self.answer.take();
        let settings = self.settings.borrow();
        let job = Job::Run {
            asker: Asker::Import,
            system: settings.prompt().to_owned(),
            user,
            settings: settings.generation,
        };
        if self.worker.send(job) {
            self.asking.set(true);
        } else {
            st.discovery.end_ai_import(AiImportPhase::Failed);
        }
    }

    /// Stops taking in events, and stops the run in flight, if any; its late answer is ignored.
    fn end(&self) {
        self.timer.stop();
        if self.asking.replace(false) {
            self.worker.stop();
        }
    }
}
