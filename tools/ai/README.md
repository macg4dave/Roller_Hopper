# Device-identification model evaluation

Run a local Qwen3 GGUF model over held-out device cases, then score its
answers. A model that will not load reports the engine's own reason, which the app's AI
settings window does not show:

```sh
cargo tool ai --help
cargo tool ai score --help
cargo run --release --target-dir target --features cuda --manifest-path tools/Cargo.toml -- ai run --help

cargo run --release --target-dir target --features cuda --manifest-path tools/Cargo.toml -- ai run \
  --model tools/a.i.testmodels/Qwen3/Qwen3-4B-Q4_K_M.gguf --output answers.jsonl
cargo tool ai score --answers answers.jsonl
```

Add `--think` to use Qwen3's thinking prompt and recommended thinking sampling profile.

`ai run` needs the `cuda` feature, which adds the in-process Candle engine with CUDA. Like the
app, it runs the model only on an NVIDIA GPU with 8 GB or more, and refuses to start on any
other; there is no processor inference. The feature is off by default because the command hook
builds this crate before every shell command. `--target-dir target` shares the app's compiled
CUDA dependencies.

`ai run` loads the model from local files only (the GGUF file's embedded tokenizer, as the app
does), sends no network
traffic, and writes one JSON line per case: `id`, `model`, `answer`, `raw`, cold-load and case
timings, and prompt/output token counts. Each request uses the system prompt in `prompt.md`,
the case's evidence as JSON with `schema.json` in the question, and Qwen3's recommended
sampling for the selected thinking mode; the answer ends at its first complete JSON object.
The schema's evidence IDs are narrowed to the case's own; `ai score` flags any other ID cited.

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
