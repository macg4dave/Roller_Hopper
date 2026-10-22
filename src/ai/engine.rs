//! Direct Candle loading and autoregressive generation for local text models.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use candle_core::quantized::{gguf_file, tokenizer::TokenizerFromGguf};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::{LogitsProcessor, Sampling};
use tokenizers::Tokenizer;

use super::chat::{self, Format};
use super::response;
use super::weights::{Kind, Weights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Missing,
    /// An architecture or chat format the engine does not run.
    Unsupported,
    /// A GGUF file whose tokenizer Candle cannot read, with no `tokenizer.json` beside it.
    NoTokenizer,
    Broken,
    NoGpu,
    #[cfg(feature = "cuda")]
    InsufficientGpuMemory,
    InvalidOutput,
    TooLong {
        prompt_tokens: usize,
        limit: usize,
    },
    Failed,
}

pub(crate) struct Settings {
    pub(crate) sampling: Sampling,
    pub(crate) min_p: f32,
    pub(crate) repeat_penalty: f32,
    pub(crate) presence_penalty: f32,
    pub(crate) max_tokens: usize,
    pub(crate) context_tokens: usize,
    pub(crate) think: bool,
    /// Empty for free text; otherwise require the device-identification schema and only these
    /// evidence citations.
    pub(crate) evidence_ids: Vec<String>,
    /// Refuse a completed answer that does not satisfy the identification contract.
    pub(crate) enforce_schema: bool,
}

pub(crate) struct Answer {
    pub(crate) text: String,
    pub(crate) tokens: usize,
    pub(crate) prompt_tokens: usize,
    pub(crate) stopped: bool,
}

pub(crate) struct Model {
    weights: Weights,
    tokenizer: Tokenizer,
    format: Format,
    /// The tokens that end the model's turn.
    stops: Vec<u32>,
    /// The tokenizer's special tokens, which prompt text is kept from spelling out.
    specials: Vec<String>,
    device: Device,
    context_limit: usize,
    prefix: Option<PromptPrefix>,
}

struct PromptPrefix {
    text: String,
    tokens: Vec<u32>,
}

/// A model whose weights are not loaded yet: its tokenizer and format are checked first, so an
/// unsupported model is refused before gigabytes are read.
struct Parts {
    tokenizer: Tokenizer,
    format: Format,
    device: Device,
    context_limit: usize,
}

impl Model {
    /// Loads a GGUF file, or a safetensors folder with its weights held as `dtype`; a GGUF
    /// file keeps its own quantisation.
    pub(crate) fn load(path: &Path, gpu: bool, dtype: DType) -> Result<Self, Error> {
        if path.is_dir() {
            Self::load_safetensors(path, gpu, dtype)
        } else if path.is_file() {
            Self::load_gguf(path, gpu)
        } else {
            Err(Error::Missing)
        }
    }

    fn load_gguf(path: &Path, gpu: bool) -> Result<Self, Error> {
        let mut file = File::open(path).map_err(|_| Error::Missing)?;
        #[cfg(feature = "cuda")]
        let file_bytes = file.metadata().map_err(|_| Error::Missing)?.len();
        let content = gguf_file::Content::read(&mut file).map_err(|_| Error::Broken)?;
        let architecture = metadata_string(&content, "general.architecture")?;
        let kind = Kind::of_gguf(&architecture).ok_or(Error::Unsupported)?;
        // Candle reads only BPE tokenizers from GGUF metadata; SentencePiece ones (Gemma,
        // Phi-3) come from the model's own tokenizer.json beside the file.
        let tokenizer = Tokenizer::from_gguf(&content).or_else(|_| {
            Tokenizer::from_file(path.with_file_name("tokenizer.json"))
                .map_err(|_| Error::NoTokenizer)
        })?;
        let declared =
            metadata_usize(&content, &format!("{architecture}.context_length")).unwrap_or(32_768);
        let parts = Parts::new(tokenizer, gpu, declared.min(kind.gguf_position_limit()))?;
        #[cfg(feature = "cuda")]
        parts.require_gpu_memory(file_bytes)?;
        let weights = Weights::gguf(kind, content, &mut file, &parts.device)?;
        Ok(parts.with(weights))
    }

    fn load_safetensors(folder: &Path, gpu: bool, dtype: DType) -> Result<Self, Error> {
        let config_text = read(folder.join("config.json"))?;
        let config: serde_json::Value =
            serde_json::from_str(&config_text).map_err(|_| Error::Broken)?;
        let kind = config
            .get("model_type")
            .and_then(serde_json::Value::as_str)
            .ok_or(Error::Broken)
            .and_then(|name| Kind::of_safetensors(name).ok_or(Error::Unsupported))?;
        let context_limit = config
            .get("max_position_embeddings")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(32_768);
        let tokenizer =
            Tokenizer::from_file(folder.join("tokenizer.json")).map_err(|_| Error::Broken)?;
        let parts = Parts::new(tokenizer, gpu, context_limit)?;
        let tensors = load_tensors(folder, &parts.device)?;
        let vb = VarBuilder::from_tensors(tensors, dtype, &parts.device);
        let weights = Weights::safetensors(kind, &config_text, vb, &parts.device)?;
        Ok(parts.with(weights))
    }

    pub(crate) fn answer(
        &mut self,
        texts: (&str, &str),
        settings: &Settings,
        stop: &dyn Fn() -> bool,
        output: &mut dyn FnMut(&str),
    ) -> Result<Answer, Error> {
        let (prompt, maximum) = self.encode(texts, settings)?;
        let prompt_tokens = prompt.len();
        let mut generated = Vec::with_capacity(maximum);
        let mut picker = LogitsProcessor::from_sampling(299_792_458, settings.sampling.clone());
        let mut text = String::new();
        let mut stopped = false;
        self.weights.reset();
        for step in 0..maximum {
            if stop() {
                stopped = true;
                break;
            }
            let (tokens, offset) = if step == 0 {
                (prompt.as_slice(), 0)
            } else {
                (
                    std::slice::from_ref(generated.last().ok_or(Error::Failed)?),
                    prompt_tokens + step - 1,
                )
            };
            let input = Tensor::new(tokens, &self.device)
                .and_then(|tensor| tensor.unsqueeze(0))
                .map_err(|_| Error::Failed)?;
            let logits = self
                .weights
                .forward(&input, offset)?
                .squeeze(0)
                .and_then(|value| value.squeeze(0))
                .map_err(|_| Error::Failed)?;
            let token = pick_token(&mut picker, &logits, settings, &generated)?;
            if self.stops.contains(&token) {
                break;
            }
            generated.push(token);
            let raw = self
                .tokenizer
                .decode(&generated, true)
                .map_err(|_| Error::Failed)?;
            response::visible(&raw, settings.think).clone_into(&mut text);
            output(&text);
            if !settings.evidence_ids.is_empty() && response::complete_json(&text).is_some() {
                break;
            }
        }
        if !stopped && settings.enforce_schema {
            text = response::validate(&text, &settings.evidence_ids).ok_or(Error::InvalidOutput)?;
            output(&text);
        }
        Ok(Answer {
            text,
            tokens: generated.len(),
            prompt_tokens,
            stopped,
        })
    }

    /// The prompt's tokens in the model's chat format, and how many answer tokens fit after
    /// them within the context limit.
    fn encode(
        &mut self,
        texts: (&str, &str),
        settings: &Settings,
    ) -> Result<(Vec<u32>, usize), Error> {
        let system = chat::plain(texts.0, &self.specials);
        let user = chat::plain(texts.1, &self.specials);
        let (prefix, turn) = self.format.prompt_parts(&system, &user, settings.think);
        let tokens = if prefix.is_empty() {
            self.tokenizer
                .encode(turn, true)
                .map_err(|_| Error::Failed)?
                .get_ids()
                .to_vec()
        } else {
            let mut prefix_tokens = self.prefix_tokens(prefix)?;
            let turn_tokens = self
                .tokenizer
                .encode(turn, false)
                .map_err(|_| Error::Failed)?;
            prefix_tokens.extend_from_slice(turn_tokens.get_ids());
            prefix_tokens
        };
        let prompt_tokens = tokens.len();
        let limit = self.context_limit.min(settings.context_tokens);
        let room = limit
            .checked_sub(prompt_tokens)
            .filter(|room| *room > 0)
            .ok_or(Error::TooLong {
                prompt_tokens,
                limit,
            })?;
        Ok((tokens, settings.max_tokens.min(room)))
    }

    fn prefix_tokens(&mut self, text: String) -> Result<Vec<u32>, Error> {
        if let Some(cached) = &self.prefix
            && cached.text == text
        {
            return Ok(cached.tokens.clone());
        }
        let tokens = self
            .tokenizer
            .encode(text.clone(), true)
            .map_err(|_| Error::Failed)?
            .get_ids()
            .to_vec();
        self.prefix = Some(PromptPrefix {
            text,
            tokens: tokens.clone(),
        });
        Ok(tokens)
    }
}

impl Parts {
    fn new(tokenizer: Tokenizer, gpu: bool, context_limit: usize) -> Result<Self, Error> {
        let format = Format::of(&tokenizer).ok_or(Error::Unsupported)?;
        Ok(Self {
            tokenizer,
            format,
            device: device(gpu)?,
            context_limit,
        })
    }

    fn with(self, weights: Weights) -> Model {
        Model {
            weights,
            stops: self.format.stop_tokens(&self.tokenizer),
            specials: chat::special_tokens(&self.tokenizer),
            tokenizer: self.tokenizer,
            format: self.format,
            device: self.device,
            context_limit: self.context_limit,
            prefix: None,
        }
    }

    /// Leaves room beyond the on-disk quantised weights for CUDA kernels, temporary tensors
    /// and the attention cache. Loading without that room can terminate the process inside the
    /// NVIDIA driver before Candle can return an allocation error.
    #[cfg(feature = "cuda")]
    fn require_gpu_memory(&self, weight_bytes: u64) -> Result<(), Error> {
        let Device::Cuda(device) = &self.device else {
            return Ok(());
        };
        let (free, _) = device
            .cuda_stream()
            .context()
            .mem_get_info()
            .map_err(|_| Error::NoGpu)?;
        enough_gpu_memory(weight_bytes, free)
            .then_some(())
            .ok_or(Error::InsufficientGpuMemory)
    }
}

/// A quantised model needs temporary allocations and an attention cache in addition to its
/// weights, while the Slint renderer shares the GPU. Keep at least 2 GiB or half the file free,
/// whichever is larger. This admits Qwen3-4B Q4 on an 8 GiB GPU while retaining room for its
/// bounded 8K attention cache and the UI.
#[cfg(any(feature = "cuda", test))]
fn enough_gpu_memory(weight_bytes: u64, free_bytes: usize) -> bool {
    const GIB: u64 = 1024 * 1024 * 1024;
    let headroom = (2 * GIB).max(weight_bytes / 2);
    weight_bytes
        .checked_add(headroom)
        .is_some_and(|required| required <= free_bytes as u64)
}

fn pick_token(
    picker: &mut LogitsProcessor,
    logits: &Tensor,
    settings: &Settings,
    generated: &[u32],
) -> Result<u32, Error> {
    let logits = penalise(logits, generated, settings)?;
    let minimum = settings.min_p;
    picker
        .sample_f(&logits, |probabilities| {
            let highest = probabilities.iter().copied().fold(0.0_f32, f32::max);
            probabilities
                .iter_mut()
                .filter(|probability| **probability < highest * minimum)
                .for_each(|probability| *probability = 0.0);
        })
        .map_err(|_| Error::Failed)
}

/// Applies both controls in one host copy; Candle's sampler already reads the whole logits
/// vector on the host, so keeping these together avoids a second device round-trip.
fn penalise(logits: &Tensor, generated: &[u32], settings: &Settings) -> Result<Tensor, Error> {
    if settings.repeat_penalty <= 1.0 && settings.presence_penalty <= 0.0 {
        return Ok(logits.clone());
    }
    let device = logits.device();
    let mut values = logits
        .to_dtype(DType::F32)
        .and_then(|tensor| tensor.to_vec1::<f32>())
        .map_err(|_| Error::Failed)?;
    let recent: HashSet<u32> = generated.iter().rev().take(64).copied().collect();
    let present: HashSet<u32> = generated.iter().copied().collect();
    for token in present {
        let Some(value) = usize::try_from(token)
            .ok()
            .and_then(|index| values.get_mut(index))
        else {
            continue;
        };
        if settings.presence_penalty > 0.0 {
            *value -= settings.presence_penalty;
        }
        if settings.repeat_penalty > 1.0 && recent.contains(&token) {
            if *value >= 0.0 {
                *value /= settings.repeat_penalty;
            } else {
                *value *= settings.repeat_penalty;
            }
        }
    }
    let len = values.len();
    Tensor::from_vec(values, len, device).map_err(|_| Error::Failed)
}

fn device(gpu: bool) -> Result<Device, Error> {
    if !gpu {
        return Ok(Device::Cpu);
    }
    #[cfg(feature = "cuda")]
    {
        Device::new_cuda(0).map_err(|_| Error::NoGpu)
    }
    #[cfg(not(feature = "cuda"))]
    {
        Err(Error::NoGpu)
    }
}

fn load_tensors(folder: &Path, device: &Device) -> Result<HashMap<String, Tensor>, Error> {
    let mut paths = std::fs::read_dir(folder)
        .map_err(|_| Error::Missing)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("safetensors"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    if paths.is_empty() {
        return Err(Error::Missing);
    }
    let mut tensors = HashMap::new();
    for path in paths {
        tensors.extend(candle_core::safetensors::load(path, device).map_err(|_| Error::Broken)?);
    }
    Ok(tensors)
}

fn read(path: PathBuf) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|_| Error::Missing)
}

fn metadata_string(content: &gguf_file::Content, key: &str) -> Result<String, Error> {
    content
        .metadata
        .get(key)
        .ok_or(Error::Broken)?
        .to_string()
        .cloned()
        .map_err(|_| Error::Broken)
}

fn metadata_usize(content: &gguf_file::Content, key: &str) -> Option<usize> {
    content
        .metadata
        .get(key)
        .and_then(|value| value.to_u64().ok())
        .and_then(|n| usize::try_from(n).ok())
}

#[cfg(test)]
mod tests {
    use super::enough_gpu_memory;

    #[test]
    fn gpu_admission_keeps_runtime_headroom_beyond_quantised_weights() {
        const GIB_U64: u64 = 1024 * 1024 * 1024;
        const GIB_USIZE: usize = 1024 * 1024 * 1024;
        assert!(!enough_gpu_memory(3 * GIB_U64, 4 * GIB_USIZE));
        assert!(enough_gpu_memory(3 * GIB_U64, 5 * GIB_USIZE));
    }
}
