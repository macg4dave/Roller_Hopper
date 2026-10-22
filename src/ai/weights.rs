//! The model architectures the engine runs, each through its own Candle implementation:
//! loading GGUF or safetensors weights, one forward step, and clearing the attention cache
//! between answers.

use std::fs::File;

use candle_core::quantized::gguf_file;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::{
    gemma3, glm4_new, llama, phi3, quantized_gemma3, quantized_glm4, quantized_llama,
    quantized_phi3, quantized_qwen2, quantized_qwen3, qwen2, qwen3,
};

use super::engine::Error;

/// An architecture the engine runs from either format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Qwen2,
    Qwen3,
    /// Llama 3, and models built on it.
    Llama,
    Gemma3,
    Phi3,
    /// GLM-4-0414 and later.
    Glm4,
}

impl Kind {
    /// The kind a GGUF file's `general.architecture` names.
    pub(super) fn of_gguf(architecture: &str) -> Option<Self> {
        match architecture {
            "qwen2" => Some(Self::Qwen2),
            "qwen3" => Some(Self::Qwen3),
            "llama" => Some(Self::Llama),
            "gemma3" => Some(Self::Gemma3),
            "phi3" => Some(Self::Phi3),
            "glm4" => Some(Self::Glm4),
            _ => None,
        }
    }

    /// The kind a safetensors folder's `config.json` names as its `model_type`. Of Gemma 3,
    /// only the text-only models load: the larger ones, built with images, are not.
    pub(super) fn of_safetensors(model_type: &str) -> Option<Self> {
        match model_type {
            "gemma3_text" => Some(Self::Gemma3),
            "gemma3" => None,
            other => Self::of_gguf(other),
        }
    }

    /// The most positions this kind's GGUF implementation holds, whatever the file claims.
    pub(super) fn gguf_position_limit(self) -> usize {
        match self {
            Self::Llama => quantized_llama::MAX_SEQ_LEN,
            Self::Qwen2 | Self::Qwen3 | Self::Gemma3 | Self::Phi3 | Self::Glm4 => usize::MAX,
        }
    }
}

pub(super) enum Weights {
    Qwen2(qwen2::ModelForCausalLM),
    Qwen3(qwen3::ModelForCausalLM),
    /// The cache in use, and a fresh one to restore before each answer.
    Llama(Box<(llama::Llama, llama::Cache, llama::Cache)>),
    Gemma3(gemma3::Model),
    Phi3(phi3::Model),
    Glm4(glm4_new::ModelForCausalLM),
    QuantizedQwen2(quantized_qwen2::ModelWeights),
    QuantizedQwen3(quantized_qwen3::ModelWeights),
    QuantizedLlama(quantized_llama::ModelWeights),
    QuantizedGemma3(quantized_gemma3::ModelWeights),
    QuantizedPhi3(quantized_phi3::ModelWeights),
    QuantizedGlm4(quantized_glm4::ModelWeights),
}

impl Weights {
    pub(super) fn gguf(
        kind: Kind,
        content: gguf_file::Content,
        file: &mut File,
        device: &Device,
    ) -> Result<Self, Error> {
        match kind {
            Kind::Qwen2 => quantized_qwen2::ModelWeights::from_gguf(content, file, device)
                .map(Self::QuantizedQwen2),
            Kind::Qwen3 => quantized_qwen3::ModelWeights::from_gguf(content, file, device)
                .map(Self::QuantizedQwen3),
            Kind::Llama => quantized_llama::ModelWeights::from_gguf(content, file, device)
                .map(Self::QuantizedLlama),
            Kind::Gemma3 => quantized_gemma3::ModelWeights::from_gguf(content, file, device)
                .map(Self::QuantizedGemma3),
            Kind::Phi3 => quantized_phi3::ModelWeights::from_gguf(false, content, file, device)
                .map(Self::QuantizedPhi3),
            // Its unquantised tensors are expanded to F32, like the other GGUF models'.
            Kind::Glm4 => {
                quantized_glm4::ModelWeights::from_gguf(content, file, device, DType::F32)
                    .map(Self::QuantizedGlm4)
            }
        }
        .map_err(|_| Error::Broken)
    }

    /// A safetensors folder's weights, from its `config.json` text.
    pub(super) fn safetensors(
        kind: Kind,
        config: &str,
        vb: VarBuilder<'_>,
        device: &Device,
    ) -> Result<Self, Error> {
        let json = |error: serde_json::Error| candle_core::Error::wrap(error);
        let build = || -> candle_core::Result<Self> {
            Ok(match kind {
                Kind::Qwen2 => Self::Qwen2(qwen2::ModelForCausalLM::new(
                    &serde_json::from_str(config).map_err(json)?,
                    vb,
                )?),
                Kind::Qwen3 => Self::Qwen3(qwen3::ModelForCausalLM::new(
                    &serde_json::from_str(config).map_err(json)?,
                    vb,
                )?),
                Kind::Llama => {
                    let cfg = serde_json::from_str::<llama::LlamaConfig>(config)
                        .map_err(json)?
                        .into_config(false);
                    let cache = llama::Cache::new(true, vb.dtype(), &cfg, device)?;
                    let model = llama::Llama::load(vb, &cfg)?;
                    Self::Llama(Box::new((model, cache.clone(), cache)))
                }
                Kind::Gemma3 => {
                    let mut cfg = serde_json::from_str(config).map_err(json)?;
                    add_sliding_window_pattern(&mut cfg);
                    Self::Gemma3(gemma3::Model::new(
                        false,
                        &serde_json::from_value(cfg).map_err(json)?,
                        vb,
                    )?)
                }
                Kind::Phi3 => Self::Phi3(phi3::Model::new(
                    &serde_json::from_str(config).map_err(json)?,
                    vb,
                )?),
                Kind::Glm4 => Self::Glm4(glm4_new::ModelForCausalLM::new(
                    &serde_json::from_str(config).map_err(json)?,
                    vb,
                )?),
            })
        };
        build().map_err(|_| Error::Broken)
    }

    /// The logits for the next token after `input`, which starts at position `offset`.
    pub(super) fn forward(&mut self, input: &Tensor, offset: usize) -> Result<Tensor, Error> {
        match self {
            Self::Qwen2(model) => model.forward(input, offset),
            Self::Qwen3(model) => model.forward(input, offset),
            Self::Llama(parts) => {
                let (model, cache, _) = &mut **parts;
                model.forward(input, offset, cache)
            }
            Self::Gemma3(model) => model.forward(input, offset),
            Self::Phi3(model) => model.forward(input, offset),
            Self::Glm4(model) => model.forward(input, offset),
            Self::QuantizedQwen2(model) => model.forward(input, offset),
            Self::QuantizedQwen3(model) => model.forward(input, offset),
            Self::QuantizedLlama(model) => model.forward(input, offset),
            Self::QuantizedGemma3(model) => model.forward(input, offset),
            Self::QuantizedPhi3(model) => model.forward(input, offset),
            Self::QuantizedGlm4(model) => model.forward(input, offset),
        }
        .map_err(|_| Error::Failed)
    }

    /// Forgets the last answer, so the next starts from an empty cache. Without this, some
    /// kinds would carry the last prompt and answer into the next.
    pub(super) fn reset(&mut self) {
        match self {
            Self::Qwen2(model) => model.clear_kv_cache(),
            Self::Qwen3(model) => model.clear_kv_cache(),
            Self::Llama(parts) => {
                let (_, cache, fresh) = &mut **parts;
                cache.clone_from(fresh);
            }
            Self::Gemma3(model) => model.clear_kv_cache(),
            Self::Phi3(model) => model.clear_kv_cache(),
            Self::Glm4(model) => model.clear_kv_cache(),
            Self::QuantizedQwen2(model) => model.clear_kv_cache(),
            Self::QuantizedQwen3(model) => model.clear_kv_cache(),
            Self::QuantizedLlama(model) => model.clear_kv_cache(),
            // These replace their cache whenever a prompt starts at position 0.
            Self::QuantizedGemma3(_) | Self::QuantizedPhi3(_) | Self::QuantizedGlm4(_) => {}
        }
    }
}

/// Newer Gemma 3 configurations name `sliding_window_pattern` `_sliding_window_pattern`, or
/// list each layer's attention in `layer_types` instead; Candle needs the old name. Every
/// `pattern`-th layer attends to the whole context, so the first full layer gives it.
fn add_sliding_window_pattern(config: &mut serde_json::Value) {
    let Some(fields) = config.as_object_mut() else {
        return;
    };
    if fields.contains_key("sliding_window_pattern") {
        return;
    }
    let from_layers = || {
        fields
            .get("layer_types")?
            .as_array()?
            .iter()
            .position(|layer| layer.as_str() == Some("full_attention"))
            .map(|index| serde_json::Value::from(index + 1))
    };
    if let Some(pattern) = fields
        .get("_sliding_window_pattern")
        .cloned()
        .or_else(from_layers)
    {
        fields.insert("sliding_window_pattern".into(), pattern);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_newer_gemma_3_configuration_gets_its_sliding_window_pattern() {
        let pattern = |mut config: serde_json::Value| {
            add_sliding_window_pattern(&mut config);
            config.get("sliding_window_pattern").cloned()
        };
        assert_eq!(
            pattern(json!({"_sliding_window_pattern": 6})),
            Some(json!(6))
        );
        let layers = ["sliding_attention", "sliding_attention", "full_attention"];
        assert_eq!(pattern(json!({"layer_types": layers})), Some(json!(3)));
        // An older configuration keeps its own; one with neither is left to fail to parse.
        let both = json!({"sliding_window_pattern": 4, "_sliding_window_pattern": 6});
        assert_eq!(pattern(both), Some(json!(4)));
        assert_eq!(pattern(json!({})), None);
    }

    #[test]
    fn both_formats_name_the_same_kinds_apart_from_gemma_3() {
        for name in ["qwen2", "qwen3", "llama", "phi3", "glm4"] {
            assert!(Kind::of_gguf(name).is_some(), "{name}");
            assert_eq!(Kind::of_gguf(name), Kind::of_safetensors(name), "{name}");
        }
        assert_eq!(Kind::of_gguf("gemma3"), Some(Kind::Gemma3));
        assert_eq!(Kind::of_safetensors("gemma3_text"), Some(Kind::Gemma3));
        // Gemma 3 with images, and kinds the engine has no implementation for.
        for name in ["gemma3", "qwen35", "lfm2", "chatglm", "mistral"] {
            assert_eq!(Kind::of_safetensors(name), None, "{name}");
        }
        assert_eq!(Kind::of_gguf("qwen35"), None);
    }
}
