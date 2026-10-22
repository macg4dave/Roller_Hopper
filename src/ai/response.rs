//! Reading generated text as it arrives: the answer apart from Qwen3's reasoning, and where a
//! JSON answer ends.

/// The final-answer part of generated text. Qwen's private reasoning is withheld until its
/// closing marker arrives and is never returned to callers or future prompts.
pub(crate) fn visible(text: &str, thinking: bool) -> &str {
    if !thinking {
        return text;
    }
    text.split_once("</think>")
        .map_or("", |(_, answer)| answer.trim_start())
}

/// One complete top-level JSON object from the first `{`, ignoring braces inside strings.
pub(crate) fn complete_json(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0_u32;
    let mut quoted = false;
    let mut escaped = false;
    for (offset, character) in text.get(start..)?.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            continue;
        }
        match character {
            '"' => quoted = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return text.get(start..start + offset + character.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_is_not_part_of_the_visible_answer() {
        assert_eq!(visible("private", true), "");
        assert_eq!(
            visible("private reasoning</think>\n\n{\"type\":\"Server\"}", true),
            "{\"type\":\"Server\"}"
        );
        assert_eq!(visible("direct", false), "direct");
    }

    #[test]
    fn a_complete_object_ignores_braces_in_strings_and_trailing_text() {
        assert_eq!(
            complete_json("note {\"description\":\"a } and \\\"{\\\"\"} trailing"),
            Some("{\"description\":\"a } and \\\"{\\\"\"}")
        );
        assert!(complete_json("{\"open\": true").is_none());
    }
}
