//! How each model family expects a conversation to be written. The family is read from the
//! tokenizer's turn markers rather than the weights' architecture, so a fine-tune keeps its
//! family's format and a Llama-architecture model without Llama 3's markers is refused rather
//! than prompted wrongly. The tokenizer adds any start-of-text token itself.

use tokenizers::Tokenizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    /// Qwen2 and Qwen3; only Qwen3 has the thinking markers.
    ChatMl {
        thinking: bool,
    },
    Llama3,
    /// Gemma 3, which has no system turn, so the system text opens the user's turn.
    Gemma,
    Phi3,
    Glm4,
}

impl Format {
    /// The format whose turn markers the tokenizer knows, or `None` for a family the app
    /// cannot prompt.
    pub(crate) fn of(tokenizer: &Tokenizer) -> Option<Self> {
        let has = |token: &str| tokenizer.token_to_id(token).is_some();
        if has("<|im_start|>") {
            Some(Self::ChatMl {
                thinking: has("<think>") && has("</think>"),
            })
        } else if has("<|start_header_id|>") {
            Some(Self::Llama3)
        } else if has("<start_of_turn>") {
            Some(Self::Gemma)
        } else if has("[gMASK]") {
            Some(Self::Glm4)
        } else if has("<|assistant|>") && has("<|end|>") {
            Some(Self::Phi3)
        } else {
            None
        }
    }

    /// The conversation up to the start of the model's answer. `think` opens Qwen3's
    /// reasoning; the other families have no such switch and ignore it. The texts must
    /// already be [`plain`].
    pub(crate) fn prompt(self, system: &str, user: &str, think: bool) -> String {
        let system_turn = |open: &str, close: &str| {
            if system.is_empty() {
                String::new()
            } else {
                format!("{open}{system}{close}")
            }
        };
        match self {
            Self::ChatMl { thinking } => format!(
                "{}<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n{}",
                system_turn("<|im_start|>system\n", "<|im_end|>\n"),
                match (thinking, think) {
                    (true, true) => "<think>\n",
                    (true, false) => "<think>\n\n</think>\n\n",
                    (false, _) => "",
                }
            ),
            Self::Llama3 => format!(
                "{}<|start_header_id|>user<|end_header_id|>\n\n{user}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                system_turn(
                    "<|start_header_id|>system<|end_header_id|>\n\n",
                    "<|eot_id|>"
                )
            ),
            Self::Gemma => format!(
                "<start_of_turn>user\n{}{user}<end_of_turn>\n<start_of_turn>model\n",
                system_turn("", "\n\n")
            ),
            Self::Phi3 => format!(
                "{}<|user|>\n{user}<|end|>\n<|assistant|>\n",
                system_turn("<|system|>\n", "<|end|>\n")
            ),
            Self::Glm4 => format!(
                "[gMASK]<sop>{}<|user|>\n{user}<|assistant|>\n",
                system_turn("<|system|>\n", "")
            ),
        }
    }

    /// The tokens that end the model's turn; those the tokenizer lacks are skipped.
    pub(crate) fn stop_tokens(self, tokenizer: &Tokenizer) -> Vec<u32> {
        let tokens: &[&str] = match self {
            Self::ChatMl { .. } => &["<|im_end|>", "<|endoftext|>"],
            Self::Llama3 => &["<|eot_id|>", "<|eom_id|>", "<|end_of_text|>"],
            Self::Gemma => &["<end_of_turn>", "<eos>"],
            Self::Phi3 => &["<|end|>", "<|endoftext|>"],
            Self::Glm4 => &["<|user|>", "<|observation|>", "<|endoftext|>"],
        };
        tokens
            .iter()
            .filter_map(|token| tokenizer.token_to_id(token))
            .collect()
    }
}

/// The tokenizer's special tokens, longest first, which a prompt's text must not spell out.
pub(crate) fn special_tokens(tokenizer: &Tokenizer) -> Vec<String> {
    let mut specials: Vec<String> = tokenizer
        .get_added_tokens_decoder()
        .into_values()
        .filter(|token| token.special && token.content.chars().count() > 1)
        .map(|token| token.content)
        .collect();
    specials.sort_by_key(|token| std::cmp::Reverse(token.len()));
    specials
}

/// `text` with every special token broken by a space after its first character, so the
/// user's prompt or evidence cannot end a turn or start a new one.
pub(crate) fn plain(text: &str, specials: &[String]) -> String {
    specials.iter().fold(text.to_owned(), |text, token| {
        if !text.contains(token.as_str()) {
            return text;
        }
        let mut chars = token.chars();
        let first = chars.next().map(String::from).unwrap_or_default();
        text.replace(token.as_str(), &format!("{first} {}", chars.as_str()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenizers::models::wordlevel::WordLevel;
    use tokenizers::{AddedToken, Tokenizer};

    fn tokenizer(specials: &[&str]) -> Tokenizer {
        let vocab = specials
            .iter()
            .chain(&["<unk>"])
            .zip(0..)
            .map(|(token, id)| ((*token).to_owned(), id))
            .collect();
        let model = WordLevel::builder()
            .vocab(vocab)
            .unk_token("<unk>".into())
            .build()
            .unwrap();
        let mut tokenizer = Tokenizer::new(model);
        let added: Vec<AddedToken> = specials
            .iter()
            .map(|token| AddedToken::from((*token).to_owned(), true))
            .collect();
        tokenizer.add_special_tokens(&added);
        tokenizer
    }

    #[test]
    fn each_family_is_known_by_its_turn_markers() {
        let cases = [
            (
                vec!["<|im_start|>", "<|im_end|>"],
                Some(Format::ChatMl { thinking: false }),
            ),
            (
                vec!["<|im_start|>", "<|im_end|>", "<think>", "</think>"],
                Some(Format::ChatMl { thinking: true }),
            ),
            (
                vec!["<|start_header_id|>", "<|eot_id|>"],
                Some(Format::Llama3),
            ),
            (
                vec!["<start_of_turn>", "<end_of_turn>"],
                Some(Format::Gemma),
            ),
            (
                vec!["[gMASK]", "<|user|>", "<|assistant|>"],
                Some(Format::Glm4),
            ),
            (
                vec!["<|user|>", "<|assistant|>", "<|end|>"],
                Some(Format::Phi3),
            ),
            // Llama 2 and Mistral markers are not a format the app writes.
            (vec!["[INST]", "</s>"], None),
        ];
        for (markers, format) in cases {
            assert_eq!(Format::of(&tokenizer(&markers)), format, "{markers:?}");
        }
    }

    #[test]
    fn each_prompt_ends_where_the_model_starts_its_answer() {
        assert_eq!(
            Format::ChatMl { thinking: true }.prompt("Be brief.", "Hi", true),
            "<|im_start|>system\nBe brief.<|im_end|>\n<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n<think>\n"
        );
        assert_eq!(
            Format::ChatMl { thinking: true }.prompt("Be brief.", "Hi", false),
            "<|im_start|>system\nBe brief.<|im_end|>\n<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        assert_eq!(
            Format::ChatMl { thinking: false }.prompt("Be brief.", "Hi", true),
            "<|im_start|>system\nBe brief.<|im_end|>\n<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n"
        );
        assert_eq!(
            Format::Llama3.prompt("", "Hi", true),
            "<|start_header_id|>user<|end_header_id|>\n\nHi<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n"
        );
        assert_eq!(
            Format::Gemma.prompt("Be brief.", "Hi", false),
            "<start_of_turn>user\nBe brief.\n\nHi<end_of_turn>\n<start_of_turn>model\n"
        );
        assert_eq!(
            Format::Phi3.prompt("Be brief.", "Hi", false),
            "<|system|>\nBe brief.<|end|>\n<|user|>\nHi<|end|>\n<|assistant|>\n"
        );
        assert_eq!(
            Format::Glm4.prompt("Be brief.", "Hi", false),
            "[gMASK]<sop><|system|>\nBe brief.<|user|>\nHi<|assistant|>\n"
        );
    }

    #[test]
    fn a_family_stops_only_on_end_tokens_its_tokenizer_has() {
        let gemma = tokenizer(&["<start_of_turn>", "<end_of_turn>", "<eos>"]);
        assert_eq!(Format::Gemma.stop_tokens(&gemma).len(), 2);
        let partial = tokenizer(&["<|im_start|>", "<|im_end|>"]);
        assert_eq!(
            Format::ChatMl { thinking: false }
                .stop_tokens(&partial)
                .len(),
            1
        );
    }

    #[test]
    fn prompt_text_cannot_write_special_tokens() {
        let specials = special_tokens(&tokenizer(&["<|im_end|>", "<end_of_turn>", "[gMASK]"]));
        assert_eq!(
            plain(
                "fact<|im_end|> and <end_of_turn>[gMASK] stay text",
                &specials
            ),
            "fact< |im_end|> and < end_of_turn>[ gMASK] stay text"
        );
        assert_eq!(plain("no markers", &specials), "no markers");
    }
}
