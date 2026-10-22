//! The bounded evidence and structured answer contract around local model generation. Network
//! text stays untrusted data, and only exact submitted evidence IDs can come back as citations.

use std::collections::HashSet;

use serde_json::Value;

const MAX_EVIDENCE_BYTES: usize = 64 * 1024;
const MAX_EVIDENCE_ITEMS: usize = 10;
const ANSWER_FIELDS: [&str; 6] = [
    "description",
    "evidence_review",
    "os_confidence",
    "os_family",
    "type",
    "type_confidence",
];
const TYPES: [&str; 14] = [
    "Router",
    "Switch",
    "AccessPoint",
    "Nas",
    "Desktop",
    "Laptop",
    "Server",
    "Printer",
    "Phone",
    "Tv",
    "Console",
    "Camera",
    "Smart",
    "Unknown",
];
const OS_FAMILIES: [&str; 7] = [
    "Windows", "macOS", "Linux", "iOS", "Android", "Embedded", "Unknown",
];
const CONFIDENCE: [&str; 3] = ["high", "medium", "low"];

/// Evidence IDs from one bounded JSON array, in submission order.
pub(crate) fn evidence_ids(text: &str) -> Option<Vec<String>> {
    if text.len() > MAX_EVIDENCE_BYTES {
        return None;
    }
    let items = serde_json::from_str::<Value>(text)
        .ok()?
        .as_array()?
        .clone();
    if items.is_empty() || items.len() > MAX_EVIDENCE_ITEMS {
        return None;
    }
    let mut seen = HashSet::new();
    items
        .iter()
        .map(|item| {
            let object = item.as_object()?;
            let id = bounded_string(object.get("id")?, 16)?;
            bounded_string(object.get("source")?, 64)?;
            bounded_string(object.get("age")?, 32)?;
            let fact = object.get("fact")?;
            (bounded_string(fact, 512).is_some() || fact.as_object().is_some_and(|map| !map.is_empty()))
                .then_some(())?;
            valid_id(id).then_some(())?;
            seen.insert(id).then(|| id.to_owned())
        })
        .collect()
}

fn bounded_string(value: &Value, maximum: usize) -> Option<&str> {
    value
        .as_str()
        .filter(|text| !text.is_empty() && text.chars().count() <= maximum)
}

fn valid_id(id: &str) -> bool {
    id.strip_prefix('E').is_some_and(|number| {
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    })
}

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

/// A canonical pretty-printed answer only when its shape, categories and citations satisfy the
/// product contract.
pub(crate) fn validate(text: &str, evidence_ids: &[String]) -> Option<String> {
    let complete = complete_json(text)?;
    let answer: Value = serde_json::from_str(complete).ok()?;
    let object = answer.as_object()?;
    let mut fields: Vec<&str> = object.keys().map(String::as_str).collect();
    fields.sort_unstable();
    (fields == ANSWER_FIELDS).then_some(())?;
    member(object.get("type")?, &TYPES)?;
    member(object.get("os_family")?, &OS_FAMILIES)?;
    member(object.get("type_confidence")?, &CONFIDENCE)?;
    member(object.get("os_confidence")?, &CONFIDENCE)?;
    let description = bounded_string(object.get("description")?, 120)?;
    description
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| valid_id(word))
        .all(|id| evidence_ids.iter().any(|known| known == id))
        .then_some(())?;
    let review = object.get("evidence_review")?.as_array()?;
    (!review.is_empty() && review.len() <= MAX_EVIDENCE_ITEMS).then_some(())?;
    let mut cited = HashSet::new();
    for item in review {
        let item = item.as_object()?;
        (item.len() == 2 && item.contains_key("id") && item.contains_key("indicates"))
            .then_some(())?;
        let id = bounded_string(item.get("id")?, 16)?;
        evidence_ids.iter().any(|known| known == id).then_some(())?;
        cited.insert(id).then_some(())?;
        bounded_string(item.get("indicates")?, 120)?;
    }
    serde_json::to_string_pretty(&answer).ok()
}

fn member(value: &Value, choices: &[&str]) -> Option<()> {
    choices.contains(&value.as_str()?).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn answer() -> Value {
        json!({
            "evidence_review": [{"id": "E1", "indicates": "SSH identifies Linux"}],
            "type": "Server",
            "type_confidence": "medium",
            "os_family": "Linux",
            "os_confidence": "high",
            "description": "Linux server offering SSH (E1)"
        })
    }

    #[test]
    fn evidence_is_bounded_and_has_unique_ids() {
        let valid = json!([{"id":"E1","source":"ssh","age":"1 min","fact":"TCP 22 open"}]);
        assert_eq!(evidence_ids(&valid.to_string()), Some(vec!["E1".into()]));
        let repeated = json!([
            {"id":"E1","source":"ssh","age":"1 min","fact":"TCP 22 open"},
            {"id":"E1","source":"dns","age":"2 min","fact":"host.local"}
        ]);
        assert!(evidence_ids(&repeated.to_string()).is_none());
        let empty_fact = json!([{"id":"E1","source":"ssh","age":"1 min","fact":""}]);
        assert!(evidence_ids(&empty_fact.to_string()).is_none());
    }

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

    #[test]
    fn only_the_exact_answer_contract_and_submitted_ids_are_accepted() {
        let text = answer().to_string();
        assert!(validate(&text, &["E1".into()]).is_some());
        assert!(validate(&text, &["E2".into()]).is_none());
        let mut extra = answer();
        extra["invented"] = json!(true);
        assert!(validate(&extra.to_string(), &["E1".into()]).is_none());
        let mut invented_description = answer();
        invented_description["description"] = json!("Linux server offering SSH (E9)");
        assert!(validate(&invented_description.to_string(), &["E1".into()]).is_none());
    }
}
