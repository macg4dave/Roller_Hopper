//! Import with AI on the discovery page: loads the main application AI, asks it about each new
//! host in turn, puts what it is confident about in place of what the scan made of the host
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
use crate::ai::{self, Asker, End, Generation, Identity, Job, RunError, Worker, WorkerEvent};
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
    /// Begins importing the run's new hosts, loading the model first; the model already
    /// loaded on the same device stays.
    fn begin(self: &Rc<Self>, ui: &Weak<AppWindow>, state: &Rc<RefCell<AppState>>) {
        let mut model = None;
        update(ui, state, |st| {
            if !st.discovery.begin_ai_import() {
                return;
            }
            model = ai::import_model();
            if model.is_none() {
                st.discovery.end_ai_import(AiImportPhase::NoModel);
            }
        });
        let Some(model) = model else { return };
        // Loading and unloading before now say nothing about this import.
        while self.events.try_recv().is_ok() {}
        let gpu = ai::gpu_built();
        if !self.worker.send(Job::Load { model, gpu }) {
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
        let user = found
            .map(|f| ai::evidence(&f.device, &f.methods))
            .unwrap_or_default();
        self.answer.take();
        let job = Job::Run {
            asker: Asker::Import,
            system: ai::PROMPT.to_owned(),
            user,
            settings: Generation::default(),
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
