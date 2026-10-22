//! Converts the window's generation settings to [`Generation`] and back. Slint has only
//! 32-bit signed whole numbers, so counts are converted with their limits.

use crate::ai::{Generation, ModelFormat};
use crate::ui::GenerationSettings;

pub(super) fn to_ui(settings: &Generation) -> GenerationSettings {
    let count = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
    GenerationSettings {
        temperature: settings.temperature,
        top_p: settings.top_p,
        top_k: count(settings.top_k),
        min_p: settings.min_p,
        repeat_penalty: settings.repeat_penalty,
        max_tokens: count(settings.max_tokens),
        context_tokens: count(settings.context_tokens),
        think: settings.think,
    }
}

/// The window's settings within their limits; a negative number counts as 0.
pub(super) fn from_ui(settings: &GenerationSettings) -> Generation {
    let count = |n: i32| usize::try_from(n).unwrap_or(0);
    Generation {
        temperature: settings.temperature,
        top_p: settings.top_p,
        top_k: count(settings.top_k),
        min_p: settings.min_p,
        repeat_penalty: settings.repeat_penalty,
        max_tokens: count(settings.max_tokens),
        context_tokens: count(settings.context_tokens),
        think: settings.think,
    }
    .bounded()
}

/// The profile belonging to the loaded model format.
pub(super) fn for_format(
    format: ModelFormat,
    gguf: &GenerationSettings,
    safetensors: &GenerationSettings,
) -> Generation {
    match format {
        ModelFormat::Gguf => from_ui(gguf),
        ModelFormat::Safetensors => from_ui(safetensors),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_the_round_trip_through_the_window() {
        let settings = Generation {
            temperature: 0.75,
            top_p: 0.5,
            top_k: 40,
            min_p: 0.125,
            repeat_penalty: 1.25,
            max_tokens: 2000,
            context_tokens: 16_384,
            think: true,
        };
        assert_eq!(from_ui(&to_ui(&settings)), settings);
        let default = Generation::default();
        assert_eq!(from_ui(&to_ui(&default)), default);
    }

    #[test]
    fn negative_numbers_from_the_window_count_as_zero_within_limits() {
        let window = GenerationSettings {
            top_k: -5,
            max_tokens: -1,
            ..to_ui(&Generation::default())
        };
        let settings = from_ui(&window);
        assert_eq!(settings.top_k, 0);
        // The answer limit is at least one token.
        assert_eq!(settings.max_tokens, 1);
    }

    #[test]
    fn each_model_format_keeps_its_own_settings() {
        let mut gguf = to_ui(&Generation::default());
        gguf.temperature = 0.25;
        let mut safetensors = to_ui(&Generation::default());
        safetensors.temperature = 0.75;
        assert_eq!(
            for_format(ModelFormat::Gguf, &gguf, &safetensors).temperature,
            0.25
        );
        assert_eq!(
            for_format(ModelFormat::Safetensors, &gguf, &safetensors).temperature,
            0.75
        );
    }
}
