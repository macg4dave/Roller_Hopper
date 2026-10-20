//! How the model picks each token of an answer: the settings the user can change in the AI
//! settings window, kept within limits that keep a run bounded.

use candle_transformers::generation::Sampling;

/// The most tokens an answer may have, whatever the user sets.
pub(crate) const MAX_ANSWER_TOKENS: usize = 8192;
/// How many of the answer's latest tokens the repeat penalty looks back over.
pub(crate) const REPEAT_WINDOW: usize = 64;

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
    /// Above 1, makes tokens already in the answer less likely; 1 for none.
    pub(crate) repeat_penalty: f32,
    /// Stops a model that never ends its turn.
    pub(crate) max_tokens: usize,
    /// Starts the random choices, so a sampled answer can be repeated.
    pub(crate) seed: u64,
    /// Lets a model that can (Qwen3) reason before it answers.
    pub(crate) think: bool,
}

impl Default for Generation {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            top_p: 1.0,
            top_k: 0,
            repeat_penalty: 1.0,
            // Enough for the identification answer.
            max_tokens: 700,
            seed: 0,
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
        Self {
            temperature: within(self.temperature, 0.0, 2.0, default.temperature),
            top_p: within(self.top_p, 0.01, 1.0, default.top_p),
            top_k: self.top_k.min(1000),
            repeat_penalty: within(self.repeat_penalty, 1.0, 2.0, default.repeat_penalty),
            max_tokens: self.max_tokens.clamp(1, MAX_ANSWER_TOKENS),
            ..self
        }
    }

    /// Candle's sampling for these settings: greedy at temperature 0, otherwise narrowed by
    /// top-k and top-p when they are set.
    pub(crate) fn sampling(&self) -> Sampling {
        if self.temperature <= 0.0 {
            return Sampling::ArgMax;
        }
        let (temperature, p) = (f64::from(self.temperature), f64::from(self.top_p));
        match (self.top_k, p < 1.0) {
            (0, false) => Sampling::All { temperature },
            (0, true) => Sampling::TopP { p, temperature },
            (k, false) => Sampling::TopK { k, temperature },
            (k, true) => Sampling::TopKThenTopP { k, p, temperature },
        }
    }

    /// Whether the repeat penalty changes anything.
    pub(crate) fn penalises_repeats(&self) -> bool {
        self.repeat_penalty > 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_greedy_and_fits_an_identification_answer() {
        let default = Generation::default();
        assert_eq!(default.sampling(), Sampling::ArgMax);
        assert!(!default.penalises_repeats());
        assert!(!default.think);
        assert_eq!(default.bounded(), default);
    }

    #[test]
    fn sampling_follows_temperature_top_k_and_top_p() {
        // Values f32 holds exactly, so they compare equal after widening.
        let warm = Generation {
            temperature: 0.75,
            ..Generation::default()
        };
        assert_eq!(warm.sampling(), Sampling::All { temperature: 0.75 });
        let k = Generation { top_k: 40, ..warm };
        assert_eq!(
            k.sampling(),
            Sampling::TopK {
                k: 40,
                temperature: 0.75
            }
        );
        let p = Generation { top_p: 0.5, ..warm };
        assert_eq!(
            p.sampling(),
            Sampling::TopP {
                p: 0.5,
                temperature: 0.75
            }
        );
        let both = Generation { top_p: 0.5, ..k };
        assert_eq!(
            both.sampling(),
            Sampling::TopKThenTopP {
                k: 40,
                p: 0.5,
                temperature: 0.75
            }
        );
        // Top-k and top-p only narrow sampling; at temperature 0 the answer stays greedy.
        let cold = Generation {
            temperature: 0.0,
            ..both
        };
        assert_eq!(cold.sampling(), Sampling::ArgMax);
    }

    #[test]
    fn settings_out_of_range_are_brought_within_limits() {
        let wild = Generation {
            temperature: f32::NAN,
            top_p: 0.0,
            top_k: 1_000_000,
            repeat_penalty: 9.0,
            max_tokens: 0,
            seed: 7,
            think: true,
        }
        .bounded();
        assert!(wild.temperature.abs() < f32::EPSILON);
        assert!((wild.top_p - 0.01).abs() < f32::EPSILON);
        assert_eq!(wild.top_k, 1000);
        assert!((wild.repeat_penalty - 2.0).abs() < f32::EPSILON);
        assert_eq!(wild.max_tokens, 1);
        assert_eq!((wild.seed, wild.think), (7, true));
        let long = Generation {
            max_tokens: usize::MAX,
            ..Generation::default()
        };
        assert_eq!(long.bounded().max_tokens, MAX_ANSWER_TOKENS);
    }
}
