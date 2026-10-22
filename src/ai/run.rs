//! A background worker that owns one Candle model and reports load and generation events.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use candle_core::DType;

use super::engine::{self, Model, Settings};
use super::generation::Generation;

/// How a safetensors model's weights are held once loaded. A GGUF file keeps its own
/// quantisation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Precision {
    #[default]
    F32,
    /// Half the memory of F32.
    F16,
    /// Half the memory of F32, with F32's range; the safetensors' usual format.
    Bf16,
}

impl Precision {
    fn dtype(self) -> DType {
        match self {
            Self::F32 => DType::F32,
            Self::F16 => DType::F16,
            Self::Bf16 => DType::BF16,
        }
    }
}

/// The on-disk model format, which has its own answer settings in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelFormat {
    Gguf,
    Safetensors,
}

/// A narrow instruction over the same strict result schema. Focused runs reduce how many
/// judgements a small local model must make at once without changing what callers can trust.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum TaskProfile {
    #[default]
    Identify,
    DeviceType,
    OsFamily,
    Description,
    Conflicts,
}

impl TaskProfile {
    pub(crate) fn from_index(index: i32) -> Self {
        match index {
            1 => Self::DeviceType,
            2 => Self::OsFamily,
            3 => Self::Description,
            4 => Self::Conflicts,
            _ => Self::Identify,
        }
    }

    fn instruction(self) -> &'static str {
        match self {
            Self::Identify => "Complete the whole identification.",
            Self::DeviceType => "Focus on device type; use Unknown for unsupported OS claims.",
            Self::OsFamily => "Focus on OS family; use Unknown for unsupported type claims.",
            Self::Description => {
                "Focus on a concise evidence-cited description; keep uncertain fields Unknown."
            }
            Self::Conflicts => {
                "Focus on conflicting evidence; lower confidence and prefer Unknown when it is unresolved."
            }
        }
    }
}

impl ModelFormat {
    pub(crate) fn from_path(path: &Path) -> Self {
        if path.is_dir() {
            Self::Safetensors
        } else {
            Self::Gguf
        }
    }
}

#[derive(Debug)]
pub(crate) enum Job {
    Load {
        model: PathBuf,
        gpu: bool,
        /// Used only by a safetensors model.
        precision: Precision,
    },
    Unload,
    Run {
        system: String,
        user: String,
        settings: Generation,
        profile: TaskProfile,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerEvent {
    Unloaded,
    Loading,
    Ready(ModelFormat),
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
    NoTokenizer,
    Broken,
    NoGpu,
    InsufficientGpuMemory,
    InvalidInput,
    InvalidOutput,
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
            Job::Load {
                model: path,
                gpu,
                precision,
            } => {
                loaded = None;
                report(WorkerEvent::Loading);
                let format = ModelFormat::from_path(&path);
                match Model::load(&path, gpu, precision.dtype()).map_err(RunError::from) {
                    Ok(model) => {
                        loaded = Some(model);
                        report(WorkerEvent::Ready(format));
                    }
                    Err(error) => report(WorkerEvent::LoadFailed(error)),
                }
            }
            Job::Run {
                system,
                user,
                settings,
                profile,
            } => {
                let result = loaded.as_mut().map_or(Err(RunError::NoModel), |model| {
                    report(WorkerEvent::Running);
                    let system = format!("{system}\n\nTask focus: {}", profile.instruction());
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
    let evidence_ids = super::response::evidence_ids(texts.1).ok_or(RunError::InvalidInput)?;
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
        evidence_ids,
        enforce_schema: true,
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
            engine::Error::NoTokenizer => Self::NoTokenizer,
            engine::Error::Broken => Self::Broken,
            engine::Error::NoGpu => Self::NoGpu,
            #[cfg(feature = "cuda")]
            engine::Error::InsufficientGpuMemory => Self::InsufficientGpuMemory,
            engine::Error::InvalidOutput => Self::InvalidOutput,
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
            profile: TaskProfile::Identify,
        }
    }
    fn load(model: PathBuf) -> Job {
        Job::Load {
            model,
            gpu: false,
            precision: Precision::F32,
        }
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

    #[test]
    fn a_file_is_gguf_and_a_model_folder_is_safetensors() {
        let dir =
            std::env::temp_dir().join(format!("network-mapper-format-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            ModelFormat::from_path(&dir.join("model.gguf")),
            ModelFormat::Gguf
        );
        assert_eq!(ModelFormat::from_path(&dir), ModelFormat::Safetensors);
        std::fs::remove_dir_all(&dir).ok();
    }

    // AI tests run only on the GPU (docs/agents/rust.md), so these exist only in CUDA builds.

    #[cfg(feature = "cuda")]
    #[test]
    #[ignore = "local CUDA 491 MB GGUF smoke test; run in release"]
    fn local_cuda_gguf_model_loads_and_answers() {
        smoke_local_model(
            test_model("Qwen2.5-0.5B-Instruct-GGUF/qwen2.5-0.5b-instruct-q4_k_m.gguf"),
            Precision::F32,
        );
    }

    #[cfg(feature = "cuda")]
    #[test]
    #[ignore = "local CUDA 988 MB safetensors smoke test, in each precision; run in release"]
    fn local_cuda_safetensors_model_loads_and_answers_in_each_precision() {
        for precision in [Precision::F32, Precision::F16, Precision::Bf16] {
            smoke_local_model(test_model("Qwen2.5-0.5B-Instruct"), precision);
        }
    }

    #[cfg(feature = "cuda")]
    #[test]
    #[ignore = "local CUDA Llama 3, Gemma 3, Phi-3 and GLM-4 smoke tests (about 1.7 GB); run in release"]
    fn local_cuda_models_of_every_kind_load_and_answer() {
        // Llama 3.2 1B and Gemma 3 270M are real models; the Phi-3 and GLM-4 ones are tiny
        // with random weights, so they only prove that the kind loads and generates. The
        // Gemma 3 GGUF file reads the tokenizer.json beside it. No Phi-3 or GLM-4 GGUF file
        // small enough exists: tiny ones are too narrow to quantise and real ones exceed 1 GB.
        for model in [
            "Llama-3.2-1B-Instruct-GGUF/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
            "gemma-3-270m-it",
            "gemma-3-270m-it-GGUF/gemma-3-270m-it-Q8_0.gguf",
            "tiny-random-phi3",
            "tiny-random-glm4",
        ] {
            smoke_local_model(test_model(model), Precision::F32);
        }
    }

    /// Loads `model` on the GPU and asks it the same question twice: greedy answers are the
    /// same each time only if the second starts from an empty cache.
    #[cfg(feature = "cuda")]
    fn smoke_local_model(model: PathBuf, precision: Precision) {
        let name = format!("{} ({precision:?})", model.display());
        let format = ModelFormat::from_path(&model);
        let jobs = vec![
            Job::Load {
                model,
                gpu: true,
                precision,
            },
            run(),
            run(),
        ];
        let mut finished = 0;
        let seen = events(jobs, |event| {
            if matches!(event, WorkerEvent::Finished(_)) {
                finished += 1;
            }
            finished == 2 || matches!(event, WorkerEvent::LoadFailed(_))
        });
        assert_eq!(
            seen.get(..3),
            Some(
                &[
                    WorkerEvent::Loading,
                    WorkerEvent::Ready(format),
                    WorkerEvent::Running
                ][..]
            ),
            "{name} did not load: {seen:?}"
        );
        let answers = answers(&seen);
        assert_eq!(answers.len(), 2, "{name} did not finish: {seen:?}");
        assert!(
            answers.first().is_some_and(|text| !text.trim().is_empty()),
            "{name} gave no answer"
        );
        assert_eq!(
            answers.first(),
            answers.last(),
            "{name} answered differently"
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

    #[cfg(feature = "cuda")]
    fn test_model(relative: &str) -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tools/a.i.testmodels")
            .join(relative)
    }
}
