//! A background worker that owns one Candle model and reports load and generation events.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use super::engine::{self, Model, Settings};
use super::generation::Generation;

#[derive(Debug)]
pub(crate) enum Job {
    Load {
        model: PathBuf,
        gpu: bool,
    },
    Unload,
    /// Sends the texts exactly as given and reports the answer as written: a trial, with no
    /// rules beyond what the texts and settings say.
    Run {
        system: String,
        user: String,
        settings: Generation,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerEvent {
    Unloaded,
    Loading,
    Ready,
    LoadFailed(RunError),
    Running,
    Output(String),
    Finished(Result<Answered, RunError>),
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
}

impl Worker {
    pub(crate) fn start(report: impl Fn(WorkerEvent) + Send + 'static) -> Self {
        let (jobs, inbox) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        thread::spawn(move || work(&inbox, &stopping, &report));
        Self { jobs, stop }
    }

    pub(crate) fn send(&self, job: Job) -> bool {
        if matches!(job, Job::Run { .. }) {
            self.stop.store(false, Ordering::Relaxed);
        }
        self.jobs.send(job).is_ok()
    }

    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn work(inbox: &Receiver<Job>, stop: &AtomicBool, report: &dyn Fn(WorkerEvent)) {
    let mut loaded = None;
    for job in inbox {
        match job {
            Job::Unload => {
                loaded = None;
                report(WorkerEvent::Unloaded);
            }
            Job::Load { model: path, gpu } => {
                loaded = None;
                report(WorkerEvent::Loading);
                match Model::load(&path, gpu).map_err(RunError::from) {
                    Ok(model) => {
                        loaded = Some(model);
                        report(WorkerEvent::Ready);
                    }
                    Err(error) => report(WorkerEvent::LoadFailed(error)),
                }
            }
            Job::Run {
                system,
                user,
                settings,
            } => {
                let result = loaded.as_mut().map_or(Err(RunError::NoModel), |model| {
                    report(WorkerEvent::Running);
                    answer(model, (&system, &user), settings.bounded(), stop, report)
                });
                report(WorkerEvent::Finished(result));
            }
        }
    }
}

fn answer(
    model: &mut Model,
    texts: (&str, &str),
    generation: Generation,
    stop: &AtomicBool,
    report: &dyn Fn(WorkerEvent),
) -> Result<Answered, RunError> {
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
        // The answer is not cut short at a JSON object.
        stop_at_json: false,
    };
    let answer = model.answer(
        texts,
        &settings,
        &|| stop.load(Ordering::Relaxed),
        &mut |text| {
            first_token.get_or_insert_with(|| started.elapsed());
            report(WorkerEvent::Output(text.to_owned()));
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
    use std::path::PathBuf;
    use std::sync::mpsc::RecvTimeoutError;

    fn events(jobs: Vec<Job>, mut last: impl FnMut(&WorkerEvent) -> bool) -> Vec<WorkerEvent> {
        let (tx, rx) = mpsc::channel();
        let worker = Worker::start(move |event| tx.send(event).unwrap());
        for job in jobs {
            assert!(worker.send(job));
        }
        let mut seen = Vec::new();
        loop {
            match rx.recv_timeout(Duration::from_secs(600)) {
                Ok(event) => {
                    let done = last(&event);
                    seen.push(event);
                    if done {
                        return seen;
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return seen,
            }
        }
    }

    fn run() -> Job {
        Job::Run {
            system: String::new(),
            user: "Reply briefly.".into(),
            settings: Generation {
                max_tokens: 16,
                ..Generation::default()
            },
        }
    }
    fn load(model: PathBuf) -> Job {
        Job::Load { model, gpu: false }
    }

    #[test]
    fn missing_and_broken_models_are_reported_without_stopping_the_worker() {
        let dir = std::env::temp_dir().join(format!("network-mapper-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = dir.join("text.gguf");
        std::fs::write(&text, "not a model").unwrap();
        let seen = events(
            vec![
                run(),
                load(dir.join("absent.gguf")),
                load(text),
                Job::Unload,
            ],
            |event| *event == WorkerEvent::Unloaded,
        );
        assert_eq!(
            seen,
            [
                WorkerEvent::Finished(Err(RunError::NoModel)),
                WorkerEvent::Loading,
                WorkerEvent::LoadFailed(RunError::Missing),
                WorkerEvent::Loading,
                WorkerEvent::LoadFailed(RunError::Broken),
                WorkerEvent::Unloaded
            ]
        );
        std::fs::remove_dir_all(&dir).ok();
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
        let evidence = super::super::evidence_sample(0).unwrap();
        let identify = || Job::Run {
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
        let jobs = std::iter::once(Job::Load { model, gpu: true })
            .chain(std::iter::repeat_with(identify).take(RUNS))
            .collect();
        let mut finished = 0;
        let seen = events(jobs, |event| {
            if matches!(event, WorkerEvent::Finished(_)) {
                finished += 1;
            }
            finished == RUNS || matches!(event, WorkerEvent::LoadFailed(_))
        });
        assert_eq!(
            seen.get(..2),
            Some(&[WorkerEvent::Loading, WorkerEvent::Ready][..]),
            "{relative} did not load: {seen:?}"
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
            if let WorkerEvent::Output(so_far) = event {
                text.clone_from(so_far);
            } else if matches!(event, WorkerEvent::Finished(Ok(_))) {
                answers.push(std::mem::take(&mut text));
            }
        }
        answers
    }
}
