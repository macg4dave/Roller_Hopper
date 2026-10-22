//! Direct Candle loading and autoregressive generation for a Qwen3 GGUF file.

use std::collections::HashSet;
use std::fs::File;
use std::path::Path;

use candle_core::quantized::{gguf_file, tokenizer::TokenizerFromGguf};
use candle_core::{DType, Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen3::ModelWeights;
use tokenizers::Tokenizer;

use super::chat;
use super::response;

/// The most prompt tokens one forward step takes. On the GPU, Candle's Qwen3 attention holds a
/// score for every pair of positions in a step, so a whole prompt at once needs more than a
/// gigabyte beside Qwen3-4B on an 8 GiB card, and running out there can kill the process inside
/// the driver. The answer is the same, since each chunk attends to everything cached before it.
const PROMPT_STEP: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Missing,
    /// Not a Qwen3 model.
    Unsupported,
    Broken,
    NoGpu,
    #[cfg(feature = "cuda")]
    InsufficientGpuMemory,
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
    /// Ends the answer once it holds one complete JSON object, for callers that want only that.
    pub(crate) stop_at_json: bool,
}

pub(crate) struct Answer {
    pub(crate) text: String,
    pub(crate) tokens: usize,
    pub(crate) prompt_tokens: usize,
    pub(crate) stopped: bool,
}

pub(crate) struct Model {
    weights: ModelWeights,
    tokenizer: Tokenizer,
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

impl Model {
    /// Loads a Qwen3 GGUF file. Its architecture and tokenizer are checked first, so another
    /// model is refused before gigabytes are read.
    pub(crate) fn load(path: &Path, gpu: bool) -> Result<Self, Error> {
        if !path.is_file() {
            return Err(Error::Missing);
        }
        let mut file = File::open(path).map_err(|_| Error::Missing)?;
        #[cfg(feature = "cuda")]
        let file_bytes = file.metadata().map_err(|_| Error::Missing)?.len();
        let content = gguf_file::Content::read(&mut file).map_err(|_| Error::Broken)?;
        if metadata_string(&content, "general.architecture")? != "qwen3" {
            return Err(Error::Unsupported);
        }
        let tokenizer = Tokenizer::from_gguf(&content).map_err(|_| Error::Broken)?;
        if !chat::is_qwen3(&tokenizer) {
            return Err(Error::Unsupported);
        }
        let context_limit = metadata_usize(&content, "qwen3.context_length").unwrap_or(32_768);
        let device = device(gpu)?;
        #[cfg(feature = "cuda")]
        require_gpu_memory(&device, file_bytes)?;
        let weights =
            ModelWeights::from_gguf(content, &mut file, &device).map_err(|_| Error::Broken)?;
        Ok(Self {
            weights,
            stops: chat::stop_tokens(&tokenizer),
            specials: chat::special_tokens(&tokenizer),
            tokenizer,
            device,
            context_limit,
            prefix: None,
        })
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
        self.weights.clear_kv_cache();
        for step in 0..maximum {
            if stop() {
                stopped = true;
                break;
            }
            let logits = if step == 0 {
                self.next_logits(&prompt, 0)?
            } else {
                let last = generated.last().ok_or(Error::Failed)?;
                self.next_logits(std::slice::from_ref(last), prompt_tokens + step - 1)?
            };
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
            if settings.stop_at_json && response::complete_json(&text).is_some() {
                break;
            }
        }
        Ok(Answer {
            text,
            tokens: generated.len(),
            prompt_tokens,
            stopped,
        })
    }

    /// The logits for the token after `tokens`, which follow `offset` tokens already in the
    /// cache, fed [`PROMPT_STEP`] tokens at a time.
    fn next_logits(&mut self, tokens: &[u32], offset: usize) -> Result<Tensor, Error> {
        let mut logits = None;
        for (index, chunk) in tokens.chunks(PROMPT_STEP).enumerate() {
            let input = Tensor::new(chunk, &self.device)
                .and_then(|tensor| tensor.unsqueeze(0))
                .and_then(|input| self.weights.forward(&input, offset + index * PROMPT_STEP))
                .map_err(|_| Error::Failed)?;
            logits = Some(input);
        }
        logits
            .ok_or(Error::Failed)?
            .squeeze(0)
            .and_then(|value| value.squeeze(0))
            .map_err(|_| Error::Failed)
    }

    /// The prompt's tokens in Qwen3's chat format, and how many answer tokens fit after them
    /// within the context limit.
    fn encode(
        &mut self,
        texts: (&str, &str),
        settings: &Settings,
    ) -> Result<(Vec<u32>, usize), Error> {
        let system = chat::plain(texts.0, &self.specials);
        let user = chat::plain(texts.1, &self.specials);
        let (prefix, turn) = chat::prompt_parts(&system, &user, settings.think);
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

/// Leaves room beyond the on-disk quantised weights for CUDA kernels, temporary tensors and the
/// attention cache. Loading without that room can terminate the process inside the NVIDIA
/// driver before Candle can return an allocation error.
#[cfg(feature = "cuda")]
fn require_gpu_memory(device: &Device, weight_bytes: u64) -> Result<(), Error> {
    let Device::Cuda(device) = device else {
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
/// vector on the host, so keeping these together avoids a second device round-trip. Both look
/// only at the last 64 tokens, as llama.cpp does for Qwen3's recommended presence penalty: over
/// the whole answer it would push down the JSON punctuation and evidence IDs that a valid
/// answer has to repeat, and almost everything after a long reasoning section.
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
    for token in recent {
        let Some(value) = usize::try_from(token)
            .ok()
            .and_then(|index| values.get_mut(index))
        else {
            continue;
        };
        if settings.presence_penalty > 0.0 {
            *value -= settings.presence_penalty;
        }
        if settings.repeat_penalty > 1.0 {
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
    use super::*;

    /// With thinking off, the prompt closes Qwen3's reasoning section before the answer; that
    /// only works if each marker reaches the model as its own token rather than as text.
    #[test]
    #[ignore = "reads the local Qwen3-4B fixture's tokenizer (2.5 GB file, metadata only)"]
    fn local_qwen3_thinking_markers_reach_the_model_as_single_tokens() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tools/a.i.testmodels/Qwen3/Qwen3-4B-Q4_K_M.gguf");
        let mut file = File::open(path).unwrap();
        let content = gguf_file::Content::read(&mut file).unwrap();
        let tokenizer = Tokenizer::from_gguf(&content).unwrap();
        assert!(chat::is_qwen3(&tokenizer));
        let (_, turn) = chat::prompt_parts("", "Hi", false);
        let ids = tokenizer.encode(turn, false).unwrap().get_ids().to_vec();
        for marker in ["<think>", "</think>"] {
            let id = tokenizer.token_to_id(marker).unwrap();
            assert!(ids.contains(&id), "{marker} was split into text: {ids:?}");
        }
    }

    #[test]
    fn presence_penalty_reaches_only_the_last_64_tokens() {
        let settings = Settings {
            sampling: Sampling::ArgMax,
            min_p: 0.0,
            repeat_penalty: 1.0,
            presence_penalty: 1.5,
            max_tokens: 1,
            context_tokens: 1,
            think: false,
            stop_at_json: false,
        };
        let logits = Tensor::new(&[2.0_f32, 2.0, 2.0], &Device::Cpu).unwrap();
        // Token 0 opened the answer; token 1 fills the 64 tokens since.
        let mut generated = vec![0];
        generated.extend([1; 64]);
        let penalised = penalise(&logits, &generated, &settings)
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        assert_eq!(penalised, [2.0, 0.5, 2.0]);
    }

    #[test]
    fn gpu_admission_keeps_runtime_headroom_beyond_quantised_weights() {
        const GIB_U64: u64 = 1024 * 1024 * 1024;
        const GIB_USIZE: usize = 1024 * 1024 * 1024;
        assert!(!enough_gpu_memory(3 * GIB_U64, 4 * GIB_USIZE));
        assert!(enough_gpu_memory(3 * GIB_U64, 5 * GIB_USIZE));
    }
}
