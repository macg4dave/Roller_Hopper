//! The held-out cases: each device's evidence and the answers that count as right.

use std::io;
use std::path::Path;

use serde_json::Value;

/// One device to identify.
#[derive(Debug)]
pub(crate) struct Case {
    pub(crate) id: String,
    /// The evidence exactly as the model sees it: objects with `id`, `source`, `age` and `fact`.
    #[cfg_attr(
        not(feature = "ai"),
        expect(
            dead_code,
            reason = "only `ai run`, behind the ai feature, sends it to a model"
        )
    )]
    pub(crate) evidence: Value,
    pub(crate) evidence_ids: Vec<String>,
    /// Device types that count as right; `Unknown` is listed when abstaining is right too.
    pub(crate) types: Vec<String>,
    pub(crate) os_families: Vec<String>,
    /// The most confidence the evidence justifies, for conflicting or thin evidence.
    pub(crate) max_confidence: Option<String>,
}

pub(crate) fn load(path: &Path) -> io::Result<Vec<Case>> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| io::Error::other(format!("cannot read {}: {error}", path.display())))?;
    let document: Value = serde_json::from_str(&text)
        .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
    let cases = document
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("the file has no \"cases\" array"))?;
    cases.iter().map(parse).collect()
}

fn parse(case: &Value) -> io::Result<Case> {
    let id = string(case, "id")?;
    let evidence = case
        .get("evidence")
        .filter(|evidence| evidence.is_array())
        .ok_or_else(|| invalid(&format!("case {id}: no evidence array")))?;
    let evidence_ids = evidence
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| string(item, "id"))
        .collect::<io::Result<_>>()?;
    let expect = case
        .get("expect")
        .ok_or_else(|| invalid(&format!("case {id}: no expect object")))?;
    Ok(Case {
        types: strings(expect, "type", &id)?,
        os_families: strings(expect, "os_family", &id)?,
        max_confidence: expect
            .get("max_confidence")
            .and_then(Value::as_str)
            .map(str::to_owned),
        evidence: evidence.clone(),
        evidence_ids,
        id,
    })
}

fn string(value: &Value, key: &str) -> io::Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(&format!("missing string \"{key}\" in {value}")))
}

fn strings(value: &Value, key: &str, case: &str) -> io::Result<Vec<String>> {
    let list: Vec<String> = value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if list.is_empty() {
        return Err(invalid(&format!(
            "case {case}: expect.{key} lists no answers"
        )));
    }
    Ok(list)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_case_keeps_its_evidence_ids_and_accepted_answers() {
        let case = parse(&serde_json::json!({
            "id": "printer",
            "evidence": [{"id": "E1", "source": "port scan", "age": "1 min", "fact": "TCP 9100 open"}],
            "expect": {"type": ["Printer"], "os_family": ["Embedded", "Unknown"], "max_confidence": "medium"}
        }))
        .unwrap();
        assert_eq!(case.evidence_ids, ["E1"]);
        assert_eq!(case.types, ["Printer"]);
        assert_eq!(case.os_families, ["Embedded", "Unknown"]);
        assert_eq!(case.max_confidence.as_deref(), Some("medium"));
    }

    #[test]
    fn a_case_without_accepted_answers_is_rejected() {
        let error = parse(&serde_json::json!({
            "id": "empty",
            "evidence": [],
            "expect": {"type": [], "os_family": ["Unknown"]}
        }))
        .unwrap_err();
        assert!(error.to_string().contains("expect.type"));
    }
}
