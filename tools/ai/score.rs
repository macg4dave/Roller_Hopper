//! Score recorded answers: right, abstained or wrong per field, plus the failures that matter
//! more than a wrong guess (invalid output, invented evidence, misplaced confidence).

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::Value;

use super::cases::{self, Case};

#[derive(Debug, clap::Args)]
#[command(
    after_help = "Examples:\n  cargo tool ai score --answers answers.jsonl\n  cargo tool ai score --answers answers.jsonl --cases path/to/cases.json\n\nRead-only. Prints one line per case and a summary. Unknown counts as right when the case\nlists it, and as an abstention otherwise. Cases without an answer line are reported as missing."
)]
pub(crate) struct Args {
    /// Answers written by `ai run` (one JSON object per line).
    #[arg(long)]
    answers: PathBuf,
    /// Case file (defaults to tools/ai/cases.json).
    #[arg(long)]
    cases: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Right,
    Abstained,
    Wrong,
}

/// How one answer did.
#[derive(Debug, PartialEq)]
struct Marks {
    /// `None` when the answer is missing or not valid JSON of the expected shape.
    fields: Option<(Verdict, Verdict)>,
    invented: Vec<String>,
    overconfident: bool,
    long_description: bool,
}

pub(super) fn run(args: &Args, root: &Path) -> io::Result<()> {
    let default_cases = root.join("tools/ai/cases.json");
    let cases = cases::load(args.cases.as_deref().unwrap_or(&default_cases))?;
    let answers = load_answers(&args.answers)?;
    let citation = Regex::new(r"\bE\d+\b").map_err(io::Error::other)?;
    let mut output = io::stdout().lock();
    let mut summary = Summary::default();
    for case in &cases {
        let record = answers.get(&case.id);
        let marks = mark(case, record.and_then(|r| r.get("answer")), &citation);
        let seconds = record
            .and_then(|r| r.get("seconds"))
            .and_then(Value::as_f64);
        writeln!(output, "{}", line(case, record, &marks))?;
        summary.add(&marks, seconds);
    }
    summary.write(&mut output)
}

fn load_answers(path: &Path) -> io::Result<HashMap<String, Value>> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| io::Error::other(format!("cannot read {}: {error}", path.display())))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let record: Value = serde_json::from_str(line).map_err(io::Error::other)?;
            let id = record
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| io::Error::other(format!("answer line has no id: {line}")))?;
            Ok((id.to_owned(), record))
        })
        .collect()
}

fn mark(case: &Case, answer: Option<&Value>, citation: &Regex) -> Marks {
    let field = |key: &str| answer.and_then(|a| a.get(key)).and_then(Value::as_str);
    let fields = field("type")
        .zip(field("os_family"))
        .map(|(kind, os)| (verdict(kind, &case.types), verdict(os, &case.os_families)));
    let description = field("description").unwrap_or_default();
    let reviewed = answer
        .and_then(|a| a.get("evidence_review"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str));
    let mut invented: Vec<String> = reviewed
        .chain(citation.find_iter(description).map(|m| m.as_str()))
        .filter(|id| !case.evidence_ids.iter().any(|known| known == id))
        .map(str::to_owned)
        .collect();
    invented.dedup();
    let overconfident = [
        ("type_confidence", fields.map(|f| f.0)),
        ("os_confidence", fields.map(|f| f.1)),
    ]
    .into_iter()
    .any(|(key, verdict)| {
        let level = rank(field(key).unwrap_or("low"));
        let wrong_and_sure = verdict == Some(Verdict::Wrong) && level == rank("high");
        let above_cap = case
            .max_confidence
            .as_deref()
            .is_some_and(|cap| level > rank(cap));
        wrong_and_sure || above_cap
    });
    Marks {
        fields,
        invented,
        overconfident,
        long_description: description.chars().count() > 120,
    }
}

fn verdict(answer: &str, accepted: &[String]) -> Verdict {
    if accepted.iter().any(|a| a == answer) {
        Verdict::Right
    } else if answer == "Unknown" {
        Verdict::Abstained
    } else {
        Verdict::Wrong
    }
}

fn rank(confidence: &str) -> u8 {
    match confidence {
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

fn line(case: &Case, record: Option<&Value>, marks: &Marks) -> String {
    let answer = record.and_then(|r| r.get("answer"));
    let field = |key: &str| {
        answer
            .and_then(|a| a.get(key))
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_owned()
    };
    let Some((kind, os)) = marks.fields else {
        let reason = if record.is_some() {
            "INVALID"
        } else {
            "MISSING"
        };
        return format!("{:<28} {reason}", case.id);
    };
    let mut flags = Vec::new();
    if !marks.invented.is_empty() {
        flags.push(format!("invented {}", marks.invented.join(",")));
    }
    if marks.overconfident {
        flags.push("overconfident".to_owned());
    }
    if marks.long_description {
        flags.push("description over 120 chars".to_owned());
    }
    format!(
        "{:<28} type {:<11} {:<6} ({:<6}) os {:<9} {:<6} ({:<6}) {}",
        case.id,
        field("type"),
        label(kind),
        field("type_confidence"),
        field("os_family"),
        label(os),
        field("os_confidence"),
        flags.join("; ")
    )
}

fn label(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Right => "right",
        Verdict::Abstained => "unsure",
        Verdict::Wrong => "WRONG",
    }
}

#[derive(Debug, Default)]
struct Summary {
    cases: usize,
    invalid: usize,
    kind: [usize; 3],
    os: [usize; 3],
    invented: usize,
    overconfident: usize,
    seconds: Vec<f64>,
}

impl Summary {
    fn add(&mut self, marks: &Marks, seconds: Option<f64>) {
        self.cases += 1;
        match marks.fields {
            Some((kind, os)) => {
                count(&mut self.kind, kind);
                count(&mut self.os, os);
            }
            None => self.invalid += 1,
        }
        self.invented += usize::from(!marks.invented.is_empty());
        self.overconfident += usize::from(marks.overconfident);
        self.seconds.extend(seconds);
    }

    fn write(&self, output: &mut impl Write) -> io::Result<()> {
        writeln!(
            output,
            "\n{} cases, {} invalid or missing",
            self.cases, self.invalid
        )?;
        for (field, [right, unsure, wrong]) in [("type:     ", self.kind), ("os family:", self.os)]
        {
            writeln!(
                output,
                "{field} {right} right, {unsure} unsure, {wrong} wrong"
            )?;
        }
        writeln!(
            output,
            "invented evidence in {}; overconfident in {}",
            self.invented, self.overconfident
        )?;
        let total: f64 = self.seconds.iter().sum();
        let slowest = self.seconds.iter().copied().fold(0.0, f64::max);
        if let Some(mean) = mean(total, self.seconds.len()) {
            writeln!(
                output,
                "seconds per case: mean {mean:.1}, slowest {slowest:.1}"
            )?;
        }
        Ok(())
    }
}

fn count(tally: &mut [usize; 3], verdict: Verdict) {
    let slot = match verdict {
        Verdict::Right => &mut tally[0],
        Verdict::Abstained => &mut tally[1],
        Verdict::Wrong => &mut tally[2],
    };
    *slot += 1;
}

fn mean(total: f64, count: usize) -> Option<f64> {
    let count = u32::try_from(count).ok().filter(|&c| c > 0)?;
    Some(total / f64::from(count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn printer() -> Case {
        Case {
            id: "printer".into(),
            evidence: json!([]),
            evidence_ids: vec!["E1".into(), "E2".into()],
            types: vec!["Printer".into()],
            os_families: vec!["Embedded".into(), "Unknown".into()],
            max_confidence: None,
        }
    }

    fn marks(case: &Case, answer: &Value) -> Marks {
        mark(case, Some(answer), &Regex::new(r"\bE\d+\b").unwrap())
    }

    #[test]
    fn unknown_is_right_when_listed_and_an_abstention_otherwise() {
        let answer = json!({"type": "Unknown", "os_family": "Unknown", "description": ""});
        assert_eq!(
            marks(&printer(), &answer).fields,
            Some((Verdict::Abstained, Verdict::Right))
        );
    }

    #[test]
    fn citing_evidence_the_case_lacks_is_invented() {
        let answer = json!({
            "type": "Printer", "os_family": "Embedded",
            "description": "Prints over IPP (E2) and RTSP (E7)",
            "evidence_review": [{"id": "E1", "indicates": "raw printing"}, {"id": "E9", "indicates": "?"}]
        });
        assert_eq!(marks(&printer(), &answer).invented, ["E9", "E7"]);
    }

    #[test]
    fn a_sure_wrong_answer_or_one_above_the_cap_is_overconfident() {
        let sure_and_wrong =
            json!({"type": "Router", "type_confidence": "high", "os_family": "Embedded"});
        assert!(marks(&printer(), &sure_and_wrong).overconfident);
        let unsure_and_wrong =
            json!({"type": "Router", "type_confidence": "low", "os_family": "Embedded"});
        assert!(!marks(&printer(), &unsure_and_wrong).overconfident);
        let mut capped = printer();
        capped.max_confidence = Some("medium".into());
        let sure_and_right =
            json!({"type": "Printer", "type_confidence": "high", "os_family": "Embedded"});
        assert!(marks(&capped, &sure_and_right).overconfident);
    }

    #[test]
    fn an_answer_without_the_required_fields_is_invalid() {
        assert_eq!(marks(&printer(), &json!({"type": "Printer"})).fields, None);
        let missing = mark(&printer(), None, &Regex::new(r"\bE\d+\b").unwrap());
        assert_eq!(missing.fields, None);
    }
}
