# Device-identification model evaluation

Run a local GGUF model or safetensors folder over held-out device cases, then score its
answers. A model that will not load reports the engine's own reason, which the app's AI
settings window does not show:

```sh
cargo tool ai --help
cargo tool ai score --help
cargo run --release --features ai --manifest-path tools/Cargo.toml -- ai run --help

RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=target-ai \
  cargo run --release --features ai --manifest-path tools/Cargo.toml -- \
  ai run --model path/to/Qwen3-8B-Q4_K_M.gguf --output answers.jsonl
cargo tool ai score --answers answers.jsonl
```

`ai run` needs the `ai` feature, which adds the in-process Candle engine. It is off by
default because the command hook builds this crate before every shell command. Use a separate
`CARGO_TARGET_DIR` for AI builds so their `RUSTFLAGS` do not force the hook's build to
recompile. Without `target-cpu=native` (or `x86-64-v3`), CPU inference runs without AVX and is
several times slower.

`ai run` loads the model from local files only (a GGUF file's embedded tokenizer and chat
template, or a Hugging Face safetensors folder's own, as the app does), sends no network
traffic, and writes one JSON line per case: `id`, `model`, `answer` (`null`
if the output did not parse), `raw` and `seconds`. Each request uses the system prompt in
`prompt.md`, the case's evidence as JSON, greedy decoding with thinking off, and output held
to `schema.json`. The schema's evidence IDs are narrowed to the case's own, so the model
cannot cite evidence that does not exist.

`ai score` needs no model. Per field, an answer is right (listed in the case's `expect`),
unsure (`Unknown` when not listed) or wrong. It also flags invented evidence IDs in the review
or description, overconfidence (wrong with `high`, or above the case's `max_confidence`) and
descriptions over 120 characters.

Files:

- `prompt.md`: the system prompt; the app's identification prompt starts from it.
- `schema.json`: the answer shape, in the order the model must fill it (review first).
- `cases.json`: the held-out cases. They were written alongside the prompt, so a high score
  is weaker evidence than one on cases collected from real networks.
- `cli.rs` dispatches; `cases.rs` loads cases; `run.rs` asks the model; `score.rs` scores.
