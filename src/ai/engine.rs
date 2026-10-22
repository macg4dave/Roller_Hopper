//! Direct Candle loading and autoregressive generation for local text models.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use candle_core::quantized::{gguf_file, tokenizer::TokenizerFromGguf};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::{LogitsProcessor, Sampling};
use tokenizers::Tokenizer;

use super::chat::{self, Format};
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
    pub(crate) max_tokens: usize,
    pub(crate) context_tokens: usize,
    pub(crate) think: bool,
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
        self.weights.reset();
        for step in 0..maximum {
            if stop() {
                return Ok(Answer {
                    text,
                    tokens: generated.len(),
                    prompt_tokens,
                    stopped: true,
                });
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
            text = self
                .tokenizer
                .decode(&generated, true)
                .map_err(|_| Error::Failed)?;
            output(&text);
        }
        Ok(Answer {
            text,
            tokens: generated.len(),
            prompt_tokens,
            stopped: false,
        })
    }

    /// The prompt's tokens in the model's chat format, and how many answer tokens fit after
    /// them within the context limit.
    fn encode(&self, texts: (&str, &str), settings: &Settings) -> Result<(Vec<u32>, usize), Error> {
        let prompt = self.format.prompt(
            &chat::plain(texts.0, &self.specials),
            &chat::plain(texts.1, &self.specials),
            settings.think,
        );
        let tokens = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|_| Error::Failed)?
            .get_ids()
            .to_vec();
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
        }
    }
}

fn pick_token(
    picker: &mut LogitsProcessor,
    logits: &Tensor,
    settings: &Settings,
    generated: &[u32],
) -> Result<u32, Error> {
    let logits = if settings.repeat_penalty > 1.0 {
        let recent = generated.iter().rev().take(64).copied().collect::<Vec<_>>();
        candle_transformers::utils::apply_repeat_penalty(logits, settings.repeat_penalty, &recent)
            .map_err(|_| Error::Failed)?
    } else {
        logits.clone()
    };
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
