//! How Qwen3 expects a conversation to be written: `ChatML` turns, with its thinking section
//! opened or already closed before the answer. The tokenizer adds any start-of-text token
//! itself.

use tokenizers::Tokenizer;

/// Whether the tokenizer has Qwen3's turn and thinking markers, so the prompt below reaches the
/// model as Qwen3 expects.
pub(crate) fn is_qwen3(tokenizer: &Tokenizer) -> bool {
    ["<|im_start|>", "<|im_end|>", "<think>", "</think>"]
        .iter()
        .all(|token| tokenizer.token_to_id(token).is_some())
}

/// A reusable system prefix and the turn holding the user's text, up to the start of the
/// model's answer. `think` opens Qwen3's reasoning; otherwise it is written already closed,
/// which is Qwen3's own way of answering directly. The texts must already be [`plain`].
pub(crate) fn prompt_parts(system: &str, user: &str, think: bool) -> (String, String) {
    let prefix = if system.is_empty() {
        String::new()
    } else {
        format!("<|im_start|>system\n{system}<|im_end|>\n")
    };
    let reasoning = if think {
        "<think>\n"
    } else {
        "<think>\n\n</think>\n\n"
    };
    (
        prefix,
        format!("<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n{reasoning}"),
    )
}

/// The tokens that end Qwen3's turn.
pub(crate) fn stop_tokens(tokenizer: &Tokenizer) -> Vec<u32> {
    ["<|im_end|>", "<|endoftext|>"]
        .iter()
        .filter_map(|token| tokenizer.token_to_id(token))
        .collect()
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
    fn only_a_tokenizer_with_qwen3s_markers_is_qwen3() {
        let qwen3 = ["<|im_start|>", "<|im_end|>", "<think>", "</think>"];
        assert!(is_qwen3(&tokenizer(&qwen3)));
        // Qwen2 has the turn markers but no thinking.
        assert!(!is_qwen3(&tokenizer(&["<|im_start|>", "<|im_end|>"])));
        assert!(!is_qwen3(&tokenizer(&["<start_of_turn>", "<end_of_turn>"])));
    }

    #[test]
    fn each_prompt_ends_where_the_model_starts_its_answer() {
        let prompt = |system, think| {
            let (prefix, turn) = prompt_parts(system, "Hi", think);
            format!("{prefix}{turn}")
        };
        assert_eq!(
            prompt("Be brief.", true),
            "<|im_start|>system\nBe brief.<|im_end|>\n<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n<think>\n"
        );
        assert_eq!(
            prompt("Be brief.", false),
            "<|im_start|>system\nBe brief.<|im_end|>\n<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        assert_eq!(
            prompt("", false),
            "<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
    }

    #[test]
    fn the_turn_ends_only_on_end_tokens_the_tokenizer_has() {
        let partial = tokenizer(&["<|im_start|>", "<|im_end|>"]);
        assert_eq!(stop_tokens(&partial).len(), 1);
    }

    #[test]
    fn prompt_text_cannot_write_special_tokens() {
        let specials = special_tokens(&tokenizer(&["<|im_end|>", "<|im_start|>"]));
        assert_eq!(
            plain("fact<|im_end|> and <|im_start|>user stay text", &specials),
            "fact< |im_end|> and < |im_start|>user stay text"
        );
        assert_eq!(plain("no markers", &specials), "no markers");
    }
}
