//! Ask a local GGUF model about each case, its output held to the answer schema.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use mistralrs::{Constraint, GgufModelBuilder, Model, RequestBuilder, TextMessageRole};
use serde_json::{Value, json};

use super::cases::{self, Case};

#[derive(Debug, clap::Args)]
#[command(
    after_help = "Examples:\n  cargo run --release --features ai --manifest-path tools/Cargo.toml -- ai run --model models/Qwen3-8B-Q4_K_M.gguf --output answers.jsonl\n  cargo tool ai score --answers answers.jsonl\n\nLoads the model from the local file only and sends no network traffic. Writes one JSON line\nper case: id, model file, answer (null if unparsable), raw text and seconds. Build with\nRUSTFLAGS=\"-C target-cpu=native\" (or x86-64-v3) or CPU inference runs without AVX."
)]
pub(crate) struct Args {
    /// GGUF model file; its embedded tokenizer and chat template are used.
    #[arg(long)]
    model: PathBuf,
    /// Answers file to write (one JSON object per line).
    #[arg(long)]
    output: PathBuf,
    /// Case file (defaults to tools/ai/cases.json).
    #[arg(long)]
    cases: Option<PathBuf>,
    /// System prompt (defaults to tools/ai/prompt.md).
    #[arg(long)]
    prompt: Option<PathBuf>,
    /// JSON schema the answer must match (defaults to tools/ai/schema.json).
    #[arg(long)]
    schema: Option<PathBuf>,
    /// Most tokens one answer may take.
    #[arg(long, default_value_t = 700)]
    max_tokens: usize,
}

pub(super) fn run(args: &Args, root: &Path) -> io::Result<()> {
    let folder = root.join("tools/ai");
    let read = |path: &Option<PathBuf>, name: &str| {
        let path = path.clone().unwrap_or_else(|| folder.join(name));
        std::fs::read_to_string(&path)
            .map_err(|error| io::Error::other(format!("cannot read {}: {error}", path.display())))
    };
    let prompt = read(&args.prompt, "prompt.md")?;
    let schema: Value = serde_json::from_str(&read(&args.schema, "schema.json")?)
        .map_err(|error| io::Error::other(format!("schema: {error}")))?;
    let cases = cases::load(
        &args
            .cases
            .clone()
            .unwrap_or_else(|| folder.join("cases.json")),
    )?;
    let mut output = BufWriter::new(File::create(&args.output)?);
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let model = load(&args.model).await?;
        let name = args
            .model
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        for case in &cases {
            let mut record = ask(&model, case, &prompt, &schema, args.max_tokens).await?;
            if let Some(fields) = record.as_object_mut() {
                fields.insert("model".into(), json!(name));
            }
            writeln!(output, "{record}")?;
            output.flush()?;
            let seconds = record
                .get("seconds")
                .and_then(Value::as_f64)
                .unwrap_or_default();
            let mut progress = io::stderr().lock();
            writeln!(progress, "{} done in {seconds:.1}s", case.id)?;
        }
        Ok(())
    })
}

async fn load(path: &Path) -> io::Result<Model> {
    let folder = path.parent().unwrap_or_else(|| Path::new("."));
    let file = path
        .file_name()
        .ok_or_else(|| io::Error::other("--model must name a .gguf file"))?;
    let started = Instant::now();
    let model = GgufModelBuilder::new(folder.to_string_lossy(), vec![file.to_string_lossy()])
        .build()
        .await
        .map_err(|error| io::Error::other(format!("cannot load {}: {error}", path.display())))?;
    let mut progress = io::stderr().lock();
    writeln!(
        progress,
        "model loaded in {:.1}s",
        started.elapsed().as_secs_f64()
    )?;
    Ok(model)
}

async fn ask(
    model: &Model,
    case: &Case,
    prompt: &str,
    schema: &Value,
    max_tokens: usize,
) -> io::Result<Value> {
    let request = RequestBuilder::default()
        .add_message(TextMessageRole::System, prompt)
        .add_message(TextMessageRole::User, question(case)?)
        .set_constraint(Constraint::JsonSchema(schema_for(
            schema,
            &case.evidence_ids,
        )?))
        .set_deterministic_sampler()
        .set_sampler_max_len(max_tokens)
        .enable_thinking(false);
    let started = Instant::now();
    let response = model
        .send_chat_request(request)
        .await
        .map_err(|error| io::Error::other(format!("case {}: {error}", case.id)))?;
    let seconds = started.elapsed().as_secs_f64();
    let raw = response
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.content)
        .unwrap_or_default();
    let answer = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);
    Ok(json!({"id": case.id, "answer": answer, "raw": raw, "seconds": seconds}))
}

/// The user turn: the evidence as JSON, so quoted replies stay visibly data.
fn question(case: &Case) -> io::Result<String> {
    let evidence = serde_json::to_string_pretty(&case.evidence).map_err(io::Error::other)?;
    Ok(format!("Evidence about one device:\n{evidence}"))
}

/// The answer schema with its evidence IDs limited to this case's, so none can be invented.
fn schema_for(schema: &Value, ids: &[String]) -> io::Result<Value> {
    let mut schema = schema.clone();
    let slot = schema
        .pointer_mut("/properties/evidence_review/items/properties/id")
        .ok_or_else(|| io::Error::other("schema lacks evidence_review item ids"))?;
    slot["enum"] = json!(ids);
    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_only_allows_the_cases_evidence_ids() {
        let schema = json!({"properties": {"evidence_review": {"items": {"properties": {"id": {"type": "string"}}}}}});
        let limited = schema_for(&schema, &["E1".into(), "E2".into()]).unwrap();
        assert_eq!(
            limited.pointer("/properties/evidence_review/items/properties/id/enum"),
            Some(&json!(["E1", "E2"]))
        );
        assert!(schema_for(&json!({}), &[]).is_err());
    }
}
