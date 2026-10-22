//! Ask a local Candle model about each case and record its answer.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use candle_core::DType;
use candle_transformers::generation::Sampling;
use serde_json::{Value, json};

use super::cases::{self, Case};
use super::engine::{Model, Settings};

#[derive(Debug, clap::Args)]
#[command(
    after_help = "Examples:\n  cargo run --release --features ai --manifest-path tools/Cargo.toml -- ai run --model models/Qwen3-4B-Q4_K_M.gguf --output answers.jsonl\n  cargo tool ai score --answers answers.jsonl\n\nLoads the model through Candle from local files only and sends no network traffic."
)]
pub(crate) struct Args {
    #[arg(long)]
    model: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    cases: Option<PathBuf>,
    #[arg(long)]
    prompt: Option<PathBuf>,
    #[arg(long)]
    schema: Option<PathBuf>,
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
    let started = Instant::now();
    let mut model = Model::load(&args.model, false, DType::F32).map_err(engine_error)?;
    writeln!(
        io::stderr().lock(),
        "model loaded in {:.1}s",
        started.elapsed().as_secs_f64()
    )?;
    let mut output = BufWriter::new(File::create(&args.output)?);
    for case in &cases {
        let mut record = ask(&mut model, case, &prompt, &schema, args.max_tokens)?;
        if let Some(fields) = record.as_object_mut() {
            fields.insert(
                "model".into(),
                json!(args.model.file_name().map(|name| name.to_string_lossy())),
            );
        }
        writeln!(output, "{record}")?;
        output.flush()?;
        writeln!(
            io::stderr().lock(),
            "{} done in {:.1}s",
            case.id,
            record
                .get("seconds")
                .and_then(Value::as_f64)
                .unwrap_or_default()
        )?;
    }
    Ok(())
}

fn ask(
    model: &mut Model,
    case: &Case,
    prompt: &str,
    schema: &Value,
    max_tokens: usize,
) -> io::Result<Value> {
    let question = format!(
        "Evidence about one device:\n{}\nAnswer with JSON matching this schema:\n{}",
        serde_json::to_string_pretty(&case.evidence).map_err(io::Error::other)?,
        schema_for(schema, &case.evidence_ids)?
    );
    let settings = Settings {
        sampling: Sampling::ArgMax,
        min_p: 0.0,
        repeat_penalty: 1.0,
        max_tokens,
        context_tokens: 32_768,
        think: false,
    };
    let started = Instant::now();
    let answer = model
        .answer((prompt, &question), &settings, &|| false, &mut |_| {})
        .map_err(engine_error)?;
    debug_assert!(answer.tokens <= max_tokens);
    debug_assert!(answer.prompt_tokens <= settings.context_tokens);
    debug_assert!(!answer.stopped);
    let parsed = serde_json::from_str::<Value>(&answer.text).unwrap_or(Value::Null);
    Ok(
        json!({"id": case.id, "answer": parsed, "raw": answer.text, "seconds": started.elapsed().as_secs_f64()}),
    )
}

fn engine_error(error: super::engine::Error) -> io::Error {
    io::Error::other(format!("Candle model error: {error:?}"))
}

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
