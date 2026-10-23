//! A background worker that owns one Candle model and reports load and generation events. The
//! AI settings window and Import with AI share it, so the model is in memory once; it unloads
//! itself once nothing has used it for [`IDLE_UNLOAD`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use super::engine::{self, Model, Settings};
use super::generation::Generation;

/// How long a loaded model may go unused before it is unloaded, freeing its memory.
pub(crate) const IDLE_UNLOAD: Duration = Duration::from_mins(5);

#[derive(Debug)]
pub(crate) enum Job {
    /// Finds out whether this GPU can run the AI ([`engine::gpu_supported`]).
    CheckGpu,
    /// Loads the model onto the GPU, unless that file is already loaded.
    Load(PathBuf),
    Unload,
    /// Sends the texts exactly as given and reports the answer as written, with no rules beyond
    /// what the texts and settings say.
    Run {
        asker: Asker,
        system: String,
        user: String,
        settings: Generation,
    },
}

/// Who a run is for, which its events name so each goes back to the one that asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asker {
    /// The AI settings window's trial run.
    Trial,
    /// Import with AI, which wants one JSON answer and nothing after it.
    Import,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerEvent {
    /// Whether this GPU can run the AI; with no CUDA, or too little memory, it cannot.
    GpuChecked(bool),
    Unloaded,
    Loading,
    /// The model file now loaded.
    Ready(PathBuf),
    LoadFailed(RunError),
    Running(Asker),
    /// The answer so far.
    Output(Asker, String),
    Finished(Asker, Result<Answered, RunError>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Answered {
    pub(crate) tokens: usize,
    pub(crate) prompt_tokens: usize,
    pub(crate) first_token: Option<Duration>,
    pub(crate) elapsed: Duration,
    pub(crate) end: End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum End {
    Complete,
    Limit,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunError {
    NoModel,
    Missing,
    Unsupported,
    Broken,
    NoGpu,
    InsufficientGpuMemory,
    TooLong { prompt_tokens: usize, limit: usize },
    Failed,
}

#[derive(Debug)]
pub(crate) struct Worker {
    jobs: Sender<Job>,
    stop: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
}

impl Worker {
    /// Starts the worker, which unloads its model once it has been `idle` that long.
    pub(crate) fn start(idle: Duration, report: impl Fn(WorkerEvent) + Send + 'static) -> Self {
        let (jobs, inbox) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let busy = Arc::new(AtomicBool::new(false));
        let working = Arc::clone(&busy);
        thread::spawn(move || work(&inbox, idle, &stopping, &working, &report));
        Self { jobs, stop, busy }
    }

    pub(crate) fn send(&self, job: Job) -> bool {
        let exclusive = !matches!(job, Job::CheckGpu);
        if exclusive
            && self
                .busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        if matches!(job, Job::Run { .. }) {
            self.stop.store(false, Ordering::Relaxed);
        }
        if self.jobs.send(job).is_ok() {
            true
        } else {
            if exclusive {
                self.busy.store(false, Ordering::Release);
            }
            false
        }
    }

    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// The model loaded, with the file it was loaded from.
struct Loaded {
    path: PathBuf,
    model: Model,
}

/// What the worker does next.
#[derive(Debug)]
enum Next {
    Do(Job),
    /// Nothing came for the idle time while a model was loaded.
    Idle,
    /// The app dropped the worker.
    Close,
}

/// Waits for the next job; with a model `loaded`, only for `idle`.
fn next(inbox: &Receiver<Job>, loaded: bool, idle: Duration) -> Next {
    let job = if loaded {
        inbox.recv_timeout(idle)
    } else {
        inbox.recv().map_err(|_| RecvTimeoutError::Disconnected)
    };
    match job {
        Ok(job) => Next::Do(job),
        Err(RecvTimeoutError::Timeout) => Next::Idle,
        Err(RecvTimeoutError::Disconnected) => Next::Close,
    }
}

fn work(
    inbox: &Receiver<Job>,
    idle: Duration,
    stop: &AtomicBool,
    busy: &AtomicBool,
    report: &dyn Fn(WorkerEvent),
) {
    let mut loaded: Option<Loaded> = None;
    loop {
        match next(inbox, loaded.is_some(), idle) {
            Next::Close => return,
            Next::Idle => {
                loaded = None;
                report(WorkerEvent::Unloaded);
            }
            Next::Do(Job::Unload) => {
                loaded = None;
                busy.store(false, Ordering::Release);
                report(WorkerEvent::Unloaded);
            }
            Next::Do(Job::CheckGpu) => report(WorkerEvent::GpuChecked(engine::gpu_supported())),
            Next::Do(Job::Load(path)) => {
                if let Some(same) = loaded.as_ref().filter(|l| l.path == path) {
                    busy.store(false, Ordering::Release);
                    report(WorkerEvent::Ready(same.path.clone()));
                    continue;
                }
                loaded = None;
                report(WorkerEvent::Loading);
                match Model::load(&path).map_err(RunError::from) {
                    Ok(model) => {
                        busy.store(false, Ordering::Release);
                        report(WorkerEvent::Ready(path.clone()));
                        loaded = Some(Loaded { path, model });
                    }
                    Err(error) => {
                        busy.store(false, Ordering::Release);
                        report(WorkerEvent::LoadFailed(error));
                    }
                }
            }
            Next::Do(Job::Run {
                asker,
                system,
                user,
                settings,
            }) => {
                let result = loaded.as_mut().map_or(Err(RunError::NoModel), |l| {
                    report(WorkerEvent::Running(asker));
                    let run = Run {
                        asker,
                        generation: settings.bounded(),
                        stop,
                        report,
                    };
                    answer(&mut l.model, (&system, &user), &run)
                });
                busy.store(false, Ordering::Release);
                report(WorkerEvent::Finished(asker, result));
            }
        }
    }
}

/// One run's settings and where it reports.
struct Run<'a> {
    asker: Asker,
    generation: Generation,
    stop: &'a AtomicBool,
    report: &'a dyn Fn(WorkerEvent),
}

fn answer(model: &mut Model, texts: (&str, &str), run: &Run<'_>) -> Result<Answered, RunError> {
    let Run {
        asker,
        generation,
        stop,
        report,
    } = *run;
    let started = Instant::now();
    let mut first_token = None;
    let settings = Settings {
        sampling: generation.sampling(),
        min_p: generation.min_p,
        repeat_penalty: generation.repeat_penalty,
        presence_penalty: generation.presence_penalty,
        max_tokens: generation.max_tokens,
        context_tokens: generation.context_tokens,
        think: generation.think,
        // A trial's answer is shown as written, not cut short at a JSON object.
        stop_at_json: asker == Asker::Import,
    };
    let answer = model.answer(
        texts,
        &settings,
        &|| stop.load(Ordering::Relaxed),
        &mut |text| {
            first_token.get_or_insert_with(|| started.elapsed());
            report(WorkerEvent::Output(asker, text.to_owned()));
        },
    )?;
    debug_assert!(answer.text.is_empty() || answer.tokens > 0);
    let end = if answer.stopped {
        End::Stopped
    } else if answer.tokens == generation.max_tokens {
        End::Limit
    } else {
        End::Complete
    };
    Ok(Answered {
        tokens: answer.tokens,
        prompt_tokens: answer.prompt_tokens,
        first_token,
        elapsed: started.elapsed(),
        end,
    })
}

impl From<engine::Error> for RunError {
    fn from(error: engine::Error) -> Self {
        match error {
            engine::Error::Missing => Self::Missing,
            engine::Error::Unsupported => Self::Unsupported,
            engine::Error::Broken => Self::Broken,
            engine::Error::NoGpu => Self::NoGpu,
            #[cfg(feature = "cuda")]
            engine::Error::InsufficientGpuMemory => Self::InsufficientGpuMemory,
            engine::Error::TooLong {
                prompt_tokens,
                limit,
            } => Self::TooLong {
                prompt_tokens,
                limit,
            },
            engine::Error::Failed => Self::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;

    fn events(jobs: Vec<Job>, mut last: impl FnMut(&WorkerEvent) -> bool) -> Vec<WorkerEvent> {
        #[derive(Clone, Copy)]
        enum Terminal {
            Gpu,
            Load,
            Unload,
            Run(Asker),
        }
        let (tx, rx) = mpsc::channel();
        let worker = Worker::start(IDLE_UNLOAD, move |event| tx.send(event).unwrap());
        let mut seen = Vec::new();
        for job in jobs {
            let terminal = match &job {
                Job::CheckGpu => Terminal::Gpu,
                Job::Load(_) => Terminal::Load,
                Job::Unload => Terminal::Unload,
                Job::Run { asker, .. } => Terminal::Run(*asker),
            };
            let is_terminal = |event: &WorkerEvent| match (terminal, event) {
                (Terminal::Gpu, WorkerEvent::GpuChecked(_))
                | (Terminal::Unload, WorkerEvent::Unloaded)
                | (Terminal::Load, WorkerEvent::Ready(_) | WorkerEvent::LoadFailed(_)) => true,
                (Terminal::Run(asker), WorkerEvent::Finished(finished, _)) => asker == *finished,
                _ => false,
            };
            assert!(worker.send(job));
            loop {
                match rx.recv_timeout(Duration::from_secs(600)) {
                    Ok(event) => {
                        let done = last(&event);
                        let job_done = is_terminal(&event);
                        seen.push(event);
                        if done {
                            return seen;
                        }
                        if job_done {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return seen,
                }
            }
        }
        seen
    }

    fn run() -> Job {
        Job::Run {
            asker: Asker::Trial,
            system: String::new(),
            user: "Reply briefly.".into(),
            settings: Generation {
                max_tokens: 16,
                ..Generation::default()
            },
        }
    }
    #[test]
    fn missing_and_broken_models_are_reported_without_stopping_the_worker() {
        let dir = std::env::temp_dir().join(format!("network-mapper-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = dir.join("text.gguf");
        std::fs::write(&text, "not a model").unwrap();
        // Both are refused before the GPU is touched.
        let seen = events(
            vec![
                run(),
                Job::Load(dir.join("absent.gguf")),
                Job::Load(text),
                Job::Unload,
            ],
            |event| *event == WorkerEvent::Unloaded,
        );
        assert_eq!(
            seen,
            [
                WorkerEvent::Finished(Asker::Trial, Err(RunError::NoModel)),
                WorkerEvent::Loading,
                WorkerEvent::LoadFailed(RunError::Missing),
                WorkerEvent::Loading,
                WorkerEvent::LoadFailed(RunError::Broken),
                WorkerEvent::Unloaded
            ]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_loaded_model_waits_only_the_idle_time_and_none_waits_for_the_next_job() {
        let (jobs, inbox) = mpsc::channel();
        let idle = Duration::from_millis(20);
        assert!(matches!(next(&inbox, true, idle), Next::Idle));
        jobs.send(Job::Unload).unwrap();
        assert!(matches!(next(&inbox, true, idle), Next::Do(Job::Unload)));
        jobs.send(Job::Unload).unwrap();
        assert!(matches!(next(&inbox, false, idle), Next::Do(Job::Unload)));
        drop(jobs);
        assert!(matches!(next(&inbox, false, idle), Next::Close));
        assert!(matches!(next(&inbox, true, idle), Next::Close));
    }

    #[test]
    fn an_exclusive_job_cannot_be_queued_behind_an_active_one() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = Worker::start(IDLE_UNLOAD, move |event| {
            if event == WorkerEvent::Loading {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }
        });
        assert!(worker.send(Job::Load(PathBuf::from("missing.gguf"))));
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(!worker.send(Job::Unload));
        release_tx.send(()).unwrap();
    }

    // AI tests run only on the GPU (docs/agents/rust.md), so these exist only in CUDA builds.

    /// The quick routine check: a small Qwen3 takes the same loading and prompt path as the
    /// application AI.
    #[cfg(feature = "cuda")]
    #[test]
    #[ignore = "local CUDA Qwen3-0.6B (639 MB) repeated-run smoke test; run in release"]
    fn local_cuda_small_qwen3_answers_the_same_over_repeated_runs() {
        repeated_runs("Qwen3_0.6B/Qwen3-0.6B-Q8_0.gguf");
    }

    /// The application AI itself, whose memory on an 8 GiB card is what repeated runs strain.
    #[cfg(feature = "cuda")]
    #[test]
    #[ignore = "local CUDA Qwen3-4B (2.5 GB) repeated-run smoke test; run in release"]
    fn local_cuda_qwen3_4b_answers_the_same_over_repeated_runs() {
        repeated_runs("Qwen3/Qwen3-4B-Q4_K_M.gguf");
    }

    /// Loads the fixture at `relative` on the GPU and runs the full identification prompt
    /// several times in a row, as a user trying prompts does: every run must finish, and with
    /// the same seed give the same answer, which it does only if each starts from an empty
    /// cache.
    #[cfg(feature = "cuda")]
    fn repeated_runs(relative: &str) {
        const RUNS: usize = 6;
        // The evidence Import with AI sends for the sample network's first found host.
        let network = crate::mock_data::network();
        let host = crate::mock_data::answers(&network).swap_remove(0);
        let evidence = super::super::evidence(&host.device);
        let identify = || Job::Run {
            asker: Asker::Trial,
            system: super::super::PROMPT.into(),
            user: evidence.clone(),
            settings: Generation {
                max_tokens: 256,
                ..Generation::default()
            },
        };
        let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tools/a.i.testmodels")
            .join(relative);
        let load = || Job::Load(model.clone());
        // Loading the loaded file again keeps it.
        let jobs = [load(), load()]
            .into_iter()
            .chain(std::iter::repeat_with(identify).take(RUNS))
            .collect();
        let mut finished = 0;
        let seen = events(jobs, |event| {
            if matches!(event, WorkerEvent::Finished(..)) {
                finished += 1;
            }
            finished == RUNS || matches!(event, WorkerEvent::LoadFailed(_))
        });
        let ready = WorkerEvent::Ready(model.clone());
        assert_eq!(
            seen.get(..3),
            Some(&[WorkerEvent::Loading, ready.clone(), ready][..]),
            "{relative} did not load once: {seen:?}"
        );
        let answers = answers(&seen);
        assert_eq!(
            answers.len(),
            RUNS,
            "{relative}: a run did not finish: {seen:?}"
        );
        assert!(
            answers.iter().all(|text| !text.trim().is_empty()),
            "{relative}"
        );
        assert!(
            answers.windows(2).all(|pair| pair.first() == pair.last()),
            "{relative} answered differently"
        );
    }

    /// The text of each answer that finished.
    #[cfg(feature = "cuda")]
    fn answers(seen: &[WorkerEvent]) -> Vec<String> {
        let mut answers = Vec::new();
        let mut text = String::new();
        for event in seen {
            if let WorkerEvent::Output(_, so_far) = event {
                text.clone_from(so_far);
            } else if matches!(event, WorkerEvent::Finished(_, Ok(_))) {
                answers.push(std::mem::take(&mut text));
            }
        }
        answers
    }
}
