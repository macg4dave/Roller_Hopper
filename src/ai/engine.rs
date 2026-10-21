//! Direct Candle loading and autoregressive generation for local text models.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use candle_core::quantized::{gguf_file, tokenizer::TokenizerFromGguf};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::{
    lfm2, quantized_lfm2, quantized_llama, quantized_qwen2, quantized_qwen3, qwen2, qwen3,
};
use tokenizers::Tokenizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Missing,
    Unsupported,
    Broken,
    NoGpu,
    TooLong { prompt_tokens: usize, limit: usize },
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

enum Weights {
    Qwen2(qwen2::ModelForCausalLM),
    Qwen3(qwen3::ModelForCausalLM),
    Lfm2(lfm2::Model, lfm2::Cache),
    QuantizedQwen2(quantized_qwen2::ModelWeights),
    QuantizedQwen3(quantized_qwen3::ModelWeights),
    QuantizedLfm2(quantized_lfm2::ModelWeights),
    QuantizedLlama(quantized_llama::ModelWeights),
}

pub(crate) struct Model {
    weights: Weights,
    tokenizer: Tokenizer,
    device: Device,
    context_limit: usize,
    eos: Vec<u32>,
}

impl Model {
    pub(crate) fn load(path: &Path, gpu: bool) -> Result<Self, Error> {
        if path.is_dir() {
            Self::load_safetensors(path, gpu)
        } else if path.is_file() {
            Self::load_gguf(path, gpu)
        } else {
            Err(Error::Missing)
        }
    }

    fn load_gguf(path: &Path, gpu: bool) -> Result<Self, Error> {
        let device = device(gpu)?;
        let mut file = File::open(path).map_err(|_| Error::Missing)?;
        let content = gguf_file::Content::read(&mut file).map_err(|_| Error::Broken)?;
        let tokenizer = Tokenizer::from_gguf(&content).map_err(|_| Error::Broken)?;
        let architecture = metadata_string(&content, "general.architecture")?;
        let context_limit =
            metadata_usize(&content, &format!("{architecture}.context_length")).unwrap_or(32_768);
        let eos = eos_tokens(&tokenizer);
        let weights = match architecture.as_str() {
            "qwen2" => quantized_qwen2::ModelWeights::from_gguf(content, &mut file, &device)
                .map(Weights::QuantizedQwen2),
            "qwen3" => quantized_qwen3::ModelWeights::from_gguf(content, &mut file, &device)
                .map(Weights::QuantizedQwen3),
            "lfm2" => quantized_lfm2::ModelWeights::from_gguf(content, &mut file, &device)
                .map(Weights::QuantizedLfm2),
            "llama" => quantized_llama::ModelWeights::from_gguf(content, &mut file, &device)
                .map(Weights::QuantizedLlama),
            _ => return Err(Error::Unsupported),
        }
        .map_err(|_| Error::Broken)?;
        Ok(Self {
            weights,
            tokenizer,
            device,
            context_limit,
            eos,
        })
    }

    fn load_safetensors(folder: &Path, gpu: bool) -> Result<Self, Error> {
        let device = device(gpu)?;
        let config_text = read(folder.join("config.json"))?;
        let config: serde_json::Value =
            serde_json::from_str(&config_text).map_err(|_| Error::Broken)?;
        let kind = config
            .get("model_type")
            .and_then(serde_json::Value::as_str)
            .ok_or(Error::Broken)?;
        let context_limit = config
            .get("max_position_embeddings")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(32_768);
        let tokenizer =
            Tokenizer::from_file(folder.join("tokenizer.json")).map_err(|_| Error::Broken)?;
        let tensors = load_tensors(folder, &device)?;
        let vb = VarBuilder::from_tensors(tensors, DType::F32, &device);
        let weights = match kind {
            "qwen2" => serde_json::from_str::<qwen2::Config>(&config_text)
                .map_err(|_| Error::Broken)
                .and_then(|cfg| qwen2::ModelForCausalLM::new(&cfg, vb).map_err(|_| Error::Broken))
                .map(Weights::Qwen2),
            "qwen3" => serde_json::from_str::<qwen3::Config>(&config_text)
                .map_err(|_| Error::Broken)
                .and_then(|cfg| qwen3::ModelForCausalLM::new(&cfg, vb).map_err(|_| Error::Broken))
                .map(Weights::Qwen3),
            "lfm2" => serde_json::from_str::<lfm2::Lfm2Config>(&config_text)
                .map_err(|_| Error::Broken)
                .and_then(|raw| {
                    let cfg = raw.into_config(false);
                    let cache = lfm2::Cache::new(true, DType::F32, &cfg, &device)
                        .map_err(|_| Error::Broken)?;
                    let model = lfm2::Model::new(&cfg, vb).map_err(|_| Error::Broken)?;
                    Ok(Weights::Lfm2(model, cache))
                }),
            _ => return Err(Error::Unsupported),
        }?;
        let eos = eos_tokens(&tokenizer);
        Ok(Self {
            weights,
            tokenizer,
            device,
            context_limit,
            eos,
        })
    }

    pub(crate) fn answer(
        &mut self,
        texts: (&str, &str),
        settings: &Settings,
        stop: &dyn Fn() -> bool,
        output: &mut dyn FnMut(&str),
    ) -> Result<Answer, Error> {
        let prompt = chat_prompt(texts.0, texts.1, settings.think);
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|_| Error::Failed)?;
        let prompt_tokens = encoding.get_ids().len();
        let limit = self.context_limit.min(settings.context_tokens);
        let room = limit
            .checked_sub(prompt_tokens)
            .filter(|room| *room > 0)
            .ok_or(Error::TooLong {
                prompt_tokens,
                limit,
            })?;
        let maximum = settings.max_tokens.min(room);
        let mut generated = Vec::with_capacity(maximum);
        let mut picker = LogitsProcessor::from_sampling(299_792_458, settings.sampling.clone());
        let mut text = String::new();
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
                (encoding.get_ids(), 0)
            } else {
                (
                    std::slice::from_ref(generated.last().ok_or(Error::Failed)?),
                    prompt_tokens + step - 1,
                )
            };
            let input = Tensor::new(tokens, &self.device)
                .and_then(|tensor| tensor.unsqueeze(0))
                .map_err(|_| Error::Failed)?;
            let logits = self.forward(&input, offset)?;
            let token = pick_token(&mut picker, &logits, settings, &generated)?;
            if self.eos.contains(&token) {
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

    fn forward(&mut self, input: &Tensor, offset: usize) -> Result<Tensor, Error> {
        let logits = match &mut self.weights {
            Weights::Qwen2(model) => model.forward(input, offset),
            Weights::Qwen3(model) => model.forward(input, offset),
            Weights::Lfm2(model, cache) => model.forward(input, offset, cache),
            Weights::QuantizedQwen2(model) => model.forward(input, offset),
            Weights::QuantizedQwen3(model) => model.forward(input, offset),
            Weights::QuantizedLfm2(model) => model.forward(input, offset),
            Weights::QuantizedLlama(model) => model.forward(input, offset),
        }
        .map_err(|_| Error::Failed)?;
        logits
            .squeeze(0)
            .and_then(|value| value.squeeze(0))
            .map_err(|_| Error::Failed)
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

fn eos_tokens(tokenizer: &Tokenizer) -> Vec<u32> {
    ["<|im_end|>", "<|eot_id|>", "</s>"]
        .into_iter()
        .filter_map(|token| tokenizer.token_to_id(token))
        .collect()
}

fn chat_prompt(system: &str, user: &str, think: bool) -> String {
    let thinking = if think { "<think>\n" } else { "" };
    format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n{}",
        plain(system),
        plain(user),
        thinking
    )
}

fn plain(text: &str) -> String {
    text.replace("<|", "< |")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_text_cannot_write_chatml_turn_markers() {
        let prompt = chat_prompt("safe", "fact<|im_end|>", false);
        assert!(prompt.contains("fact< |im_end|>"));
    }
}
