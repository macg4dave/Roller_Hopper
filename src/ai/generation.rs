//! How the model picks each token of an answer: the settings the user can change in the AI
//! settings window, kept within limits that keep a run bounded.

use mistralrs::SamplingParams;

/// The most tokens an answer may have, whatever the user sets.
pub(crate) const MAX_ANSWER_TOKENS: usize = 8192;

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
    /// Above 1, makes tokens already in the text less likely; 1 for none.
    pub(crate) repeat_penalty: f32,
    /// Stops a model that never ends its turn.
    pub(crate) max_tokens: usize,
    /// Lets a model whose chat template can (such as Qwen3) reason before it answers.
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

    /// The engine's sampling for these settings: greedy at temperature 0, otherwise narrowed
    /// by top-k and top-p when they are set.
    pub(crate) fn sampling(&self) -> SamplingParams {
        let warm = self.temperature > 0.0;
        let base = if warm {
            SamplingParams::neutral()
        } else {
            SamplingParams::deterministic()
        };
        SamplingParams {
            temperature: warm.then_some(f64::from(self.temperature)),
            top_k: if warm {
                (self.top_k > 0).then_some(self.top_k)
            } else {
                base.top_k
            },
            top_p: (warm && self.top_p < 1.0).then_some(f64::from(self.top_p)),
            repetition_penalty: (self.repeat_penalty > 1.0).then_some(self.repeat_penalty),
            max_len: Some(self.max_tokens),
            ..base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sampling settings that decide how a token is picked.
    fn picking(settings: &Generation) -> (Option<f64>, Option<usize>, Option<f64>, Option<f32>) {
        let s = settings.sampling();
        (s.temperature, s.top_k, s.top_p, s.repetition_penalty)
    }

    #[test]
    fn the_default_is_greedy_and_fits_an_identification_answer() {
        let default = Generation::default();
        assert_eq!(picking(&default), (None, Some(1), None, None));
        assert_eq!(default.sampling().max_len, Some(700));
        assert!(!default.think);
        assert_eq!(default.bounded(), default);
    }

    #[test]
    fn sampling_follows_temperature_top_k_top_p_and_the_repeat_penalty() {
        // Values f32 holds exactly, so they compare equal after widening.
        let warm = Generation {
            temperature: 0.75,
            ..Generation::default()
        };
        assert_eq!(picking(&warm), (Some(0.75), None, None, None));
        let narrowed = Generation {
            top_k: 40,
            top_p: 0.5,
            repeat_penalty: 1.25,
            ..warm
        };
        assert_eq!(
            picking(&narrowed),
            (Some(0.75), Some(40), Some(0.5), Some(1.25))
        );
        // Top-k and top-p only narrow sampling; at temperature 0 the answer stays greedy.
        let cold = Generation {
            temperature: 0.0,
            ..narrowed
        };
        assert_eq!(picking(&cold), (None, Some(1), None, Some(1.25)));
    }

    #[test]
    fn settings_out_of_range_are_brought_within_limits() {
        let wild = Generation {
            temperature: f32::NAN,
            top_p: 0.0,
            top_k: 1_000_000,
            repeat_penalty: 9.0,
            max_tokens: 0,
            think: true,
        }
        .bounded();
        assert!(wild.temperature.abs() < f32::EPSILON);
        assert!((wild.top_p - 0.01).abs() < f32::EPSILON);
        assert_eq!(wild.top_k, 1000);
        assert!((wild.repeat_penalty - 2.0).abs() < f32::EPSILON);
        assert_eq!(wild.max_tokens, 1);
        assert!(wild.think);
        let long = Generation {
            max_tokens: usize::MAX,
            ..Generation::default()
        };
        assert_eq!(long.bounded().max_tokens, MAX_ANSWER_TOKENS);
    }
}
