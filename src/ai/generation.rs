//! How the model picks each token of an answer: the settings the user can change in the AI
//! settings window, kept within limits that keep a run bounded.

use candle_transformers::generation::Sampling;

/// The most tokens a prompt and its answer may hold together, whatever the user sets (Qwen3's
/// own context). The limit bounds the attention cache as well as the run: the engine cannot cap
/// its plain (non-paged) cache itself, so the worker refuses longer prompts and stops the answer
/// at the limit.
pub(crate) const MAX_CONTEXT_TOKENS: usize = 32_768;

/// The smallest context limit, which still fits the identification prompt.
const MIN_CONTEXT_TOKENS: usize = 512;

/// The default is the evaluation tool's: greedy, so the same prompt always gives the same
/// answer, with thinking off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Generation {
    /// 0 picks the likeliest token every time; higher values let less likely ones through.
    pub(crate) temperature: f32,
    /// Samples only among the likeliest tokens that together reach this probability; 1 for
    /// all of them.
    pub(crate) top_p: f32,
    /// Samples only among this many likeliest tokens; 0 for no limit.
    pub(crate) top_k: usize,
    /// Samples only among tokens at least this share as likely as the likeliest; 0 for all.
    pub(crate) min_p: f32,
    /// Above 1, makes tokens already in the text less likely; 1 for none.
    pub(crate) repeat_penalty: f32,
    /// Stops a model that never ends its turn; never more than the context limit.
    pub(crate) max_tokens: usize,
    /// The most tokens the prompt and answer may hold together; the model's own limit, if
    /// lower, still applies.
    pub(crate) context_tokens: usize,
    /// Lets a model whose chat template can (such as Qwen3) reason before it answers.
    pub(crate) think: bool,
}

impl Default for Generation {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            top_p: 1.0,
            top_k: 0,
            min_p: 0.0,
            repeat_penalty: 1.0,
            // Enough for the identification answer.
            max_tokens: 700,
            // Enough for the identification prompt and answer, with a modest cache.
            context_tokens: 8192,
            think: false,
        }
    }
}

impl Generation {
    /// The same settings within their limits; a value that is not a number takes the default.
    pub(crate) fn bounded(self) -> Self {
        let default = Self::default();
        let within = |value: f32, low: f32, high: f32, fallback: f32| {
            if value.is_nan() {
                fallback
            } else {
                value.clamp(low, high)
            }
        };
        let context_tokens = self
            .context_tokens
            .clamp(MIN_CONTEXT_TOKENS, MAX_CONTEXT_TOKENS);
        Self {
            temperature: within(self.temperature, 0.0, 2.0, default.temperature),
            top_p: within(self.top_p, 0.01, 1.0, default.top_p),
            top_k: self.top_k.min(1000),
            min_p: within(self.min_p, 0.0, 1.0, default.min_p),
            repeat_penalty: within(self.repeat_penalty, 1.0, 2.0, default.repeat_penalty),
            max_tokens: self.max_tokens.clamp(1, context_tokens),
            context_tokens,
            ..self
        }
    }

    /// The engine's sampling for these settings: greedy at temperature 0, otherwise narrowed
    /// by top-k, top-p and min-p when they are set.
    pub(crate) fn sampling(&self) -> Sampling {
        if self.temperature <= 0.0 {
            return Sampling::ArgMax;
        }
        let temperature = f64::from(self.temperature);
        match (self.top_k, self.top_p < 1.0) {
            (0, false) => Sampling::All { temperature },
            (k, false) => Sampling::TopK { k, temperature },
            (0, true) => Sampling::TopP {
                p: f64::from(self.top_p),
                temperature,
            },
            (k, true) => Sampling::TopKThenTopP {
                k,
                p: f64::from(self.top_p),
                temperature,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sampling settings that decide how a token is picked.
    type Picking = (f64, Option<usize>, Option<f64>);

    fn picking(settings: &Generation) -> Picking {
        match settings.sampling() {
            Sampling::ArgMax => (0.0, Some(1), None),
            Sampling::All { temperature } | Sampling::GumbelSoftmax { temperature } => {
                (temperature, None, None)
            }
            Sampling::TopK { k, temperature } => (temperature, Some(k), None),
            Sampling::TopP { p, temperature } => (temperature, None, Some(p)),
            Sampling::TopKThenTopP { k, p, temperature } => (temperature, Some(k), Some(p)),
        }
    }

    #[test]
    fn the_default_is_greedy_and_fits_an_identification_answer() {
        let default = Generation::default();
        assert_eq!(picking(&default), (0.0, Some(1), None));
        assert_eq!(default.max_tokens, 700);
        assert!(!default.think);
        assert_eq!(default.bounded(), default);
    }

    #[test]
    fn sampling_follows_temperature_top_k_top_p_min_p_and_the_repeat_penalty() {
        // Values f32 holds exactly, so they compare equal after widening.
        let warm = Generation {
            temperature: 0.75,
            ..Generation::default()
        };
        assert_eq!(picking(&warm), (0.75, None, None));
        let narrowed = Generation {
            top_k: 40,
            top_p: 0.5,
            min_p: 0.125,
            repeat_penalty: 1.25,
            ..warm
        };
        assert_eq!(picking(&narrowed), (0.75, Some(40), Some(0.5)));
        // Top-k, top-p and min-p only narrow sampling; at temperature 0 the answer stays
        // greedy.
        let cold = Generation {
            temperature: 0.0,
            ..narrowed
        };
        assert_eq!(picking(&cold), (0.0, Some(1), None));
    }

    #[test]
    fn settings_out_of_range_are_brought_within_limits() {
        let wild = Generation {
            temperature: f32::NAN,
            top_p: 0.0,
            top_k: 1_000_000,
            min_p: f32::NAN,
            repeat_penalty: 9.0,
            max_tokens: 0,
            context_tokens: 0,
            think: true,
        }
        .bounded();
        assert!(wild.temperature.abs() < f32::EPSILON);
        assert!((wild.top_p - 0.01).abs() < f32::EPSILON);
        assert_eq!(wild.top_k, 1000);
        assert!(wild.min_p.abs() < f32::EPSILON);
        assert!((wild.repeat_penalty - 2.0).abs() < f32::EPSILON);
        assert_eq!(wild.max_tokens, 1);
        assert_eq!(wild.context_tokens, MIN_CONTEXT_TOKENS);
        assert!(wild.think);
        let long = Generation {
            max_tokens: usize::MAX,
            context_tokens: usize::MAX,
            ..Generation::default()
        };
        let long = long.bounded();
        assert_eq!(long.context_tokens, MAX_CONTEXT_TOKENS);
        assert_eq!(long.max_tokens, MAX_CONTEXT_TOKENS);
    }

    #[test]
    fn the_answer_limit_stays_within_the_context_limit() {
        let tight = Generation {
            max_tokens: 4000,
            context_tokens: 1024,
            ..Generation::default()
        }
        .bounded();
        assert_eq!((tight.max_tokens, tight.context_tokens), (1024, 1024));
    }
}
