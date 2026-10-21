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
        model: Option<PathBuf>,
        gpu: bool,
    },
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
    Broken,
    NoGpu,
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
            Job::Load { model: None, .. } => {
                loaded = None;
                report(WorkerEvent::Unloaded);
            }
            Job::Load {
                model: Some(path),
                gpu,
            } => {
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
        max_tokens: generation.max_tokens,
        context_tokens: generation.context_tokens,
        think: generation.think,
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
            engine::Error::Unsupported | engine::Error::Broken => Self::Broken,
            engine::Error::NoGpu => Self::NoGpu,
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
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::RecvTimeoutError;

    fn events(jobs: Vec<Job>, last: fn(&WorkerEvent) -> bool) -> Vec<WorkerEvent> {
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
    fn load(model: Option<PathBuf>) -> Job {
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
                load(Some(dir.join("absent.gguf"))),
                load(Some(text)),
                load(None),
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

    #[test]
    #[ignore = "local 491 MB model smoke test; run in release"]
    fn local_gguf_model_loads_and_answers() {
        smoke_local_model(test_model(
            "Qwen2.5-0.5B-Instruct-GGUF/qwen2.5-0.5b-instruct-q4_k_m.gguf",
        ));
    }

    #[test]
    #[ignore = "local 988 MB model smoke test; run in release"]
    fn local_safetensors_model_loads_and_answers() {
        smoke_local_model(test_model("Qwen2.5-0.5B-Instruct"));
    }

    fn smoke_local_model(model: PathBuf) {
        let seen = events(vec![load(Some(model)), run()], |event| {
            matches!(event, WorkerEvent::Finished(_) | WorkerEvent::LoadFailed(_))
        });
        assert_eq!(
            seen.get(..3),
            Some(
                &[
                    WorkerEvent::Loading,
                    WorkerEvent::Ready,
                    WorkerEvent::Running
                ][..]
            )
        );
        assert!(
            matches!(seen.last(), Some(WorkerEvent::Finished(Ok(_)))),
            "model did not finish: {seen:?}"
        );
        assert!(
            seen.iter()
                .any(|event| matches!(event, WorkerEvent::Output(text) if !text.trim().is_empty()))
        );
    }

    fn test_model(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tools/a.i.testmodels")
            .join(relative)
    }
}
