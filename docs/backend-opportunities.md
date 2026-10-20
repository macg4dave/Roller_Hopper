# Embedded AI and future opportunities

The embedded inference direction below is **requested**, with model/runtime and detailed
authority still provisional. The later blue-sky section is separate, unapproved future scope.
See [the decision register](backend-decisions.md) for confirmed answers and remaining questions.

## Requested: existing offline CPU models

The goal is useful suggestions inside the existing map/discovery workflow: infer device type,
OS family or missing descriptive details from collected evidence, show confidence and let the
user review, enable automatic updates, permit selected overwrites or choose auto-add. Bundle
one or two existing models; do not create a training service, agent framework, cloud dependency,
GPU requirement or separate chatbot application.
Everything runs in the app process. No Ollama/other model runner, child executable, localhost
server or separately installed inference program is allowed, even if invoked invisibly.

**Proposed first candidate: one Qwen3-0.6B instruction model**, with a short evidence-to-fields
prompt. Its publisher's [Q8_0 GGUF artifact](https://huggingface.co/Qwen/Qwen3-0.6B-GGUF)
is Apache-2.0 and about **639 MB**. It has a non-thinking mode; use a bounded short context
and output, not the model's maximum reasoning settings. This is an available pretrained model,
not a proven network identifier. The size may exceed the user's eventual meaning of “small”.

For the Rust-tools preference, evaluate the existing **Candle 0.11.0 quantized Qwen3 adapter**
against that artifact first; this reuses a model implementation rather than building a model.
It still needs bundled `tokenizer.json`, tokenizer configuration/chat-template assets, sampling
and validated output glue. Rust 1.94/platform compatibility has not been demonstrated.
[Released adapter](https://github.com/huggingface/candle/blob/0.11.0/candle-transformers/src/models/quantized_qwen3.rs)

The GGUF runtime alternative is **llama.cpp only as an embedded in-process library**, controlled
by Rust. Its native CPU support is established, but the researched Rust binding has an explicit
safety disclaimer. Do not call that binding production-qualified or weaken the app's unsafe
policy. Executable/server integration is excluded by the user's hard requirement. Choose one
compliant runtime after feasibility review, not both.
[Runtime trade-offs and primary sources](backend-crates.md#embedded-inference-shortlist)

| Alternative, if the first candidate is unsuitable | Why consider it | Why not automatically bundle it too |
| --- | --- | --- |
| [SmolLM2-360M-Instruct, publisher GGUF](https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct-GGUF) | Apache-2.0; publisher [Q8_0 files](https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct-GGUF/tree/main) are about **386 MB**; smaller general instruction model | Less capacity may reduce useful classification/reasoning. Device-identification quality is unverified; use as a replacement only if it meets the agreed bar. |
| [DeBERTa xsmall zero-shot model](https://huggingface.co/MoritzLaurer/deberta-v3-xsmall-zeroshot-v1.1-all-33) | MIT; publisher's [quantized ONNX](https://huggingface.co/MoritzLaurer/deberta-v3-xsmall-zeroshot-v1.1-all-33/tree/main/onnx) is about **87.2 MB**, plus tokenization/runtime. Existing NLI weights can rank fixed type/OS hypotheses without training. | Requires an ONNX route and model-specific tokenization/scoring, not a fastembed drop-in. Publisher reports no zero-shot evaluation for this small model; no network accuracy proof. It cannot freely generate verified inventory facts. |
| [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) through fastembed | Existing compact semantic embeddings can help search and candidate ranking without training | Embedding cosine similarity is not confidence that a host runs Windows. A second similarity vote is not independent factual verification. Defer unless a search use case earns it. |

Weight-file sizes are not installer size or RAM: account for tokenizer/runtime, native libraries,
KV cache, compute buffers and the GUI. Select a CPU thread limit and load on demand. Measure
installed size, cold start, inference latency and peak memory on baseline Windows/Linux/macOS
machines before choosing a default. No such measurements were made during this review.

## A small integration contract, not an AI platform

1. Send a bounded structured snapshot of existing evidence: hostname, observed service/protocol
   clues, source/age, advertised fields and explicit missing/conflicting data. Do not scan again
   simply because a model wants more information.
2. Request only allowed device/OS categories and specific proposed fields, with evidence IDs,
   a short reason, score and an Unknown/insufficient-evidence result. Prefer constrained output
   where the selected runtime supports it, then validate all fields in Rust regardless.
3. Treat hostname/banner/TXT contents as untrusted data. Reject unsupported categories, invented
   evidence IDs, oversized/truncated output and malformed records. Model output never becomes
   an executable command, URL fetch instruction, scan target or credential operation.
4. Cache suggestions by evidence fingerprint and model/feature-schema version. New observations
   may refresh the guess; that is inference, not training, self-learning or a weight download.
5. Apply through ordinary reconciliation and the user's settings, with retained previous values
   and undo. Keep current manual Add, Ignore, review and manual editing available without AI.

For the user's example, record the actual facts independently: hostname `DESKTOP-13123` and
open TCP 3389; a confirmed RDP protocol response is stronger than the port number alone.
A possible suggestion is **“Likely Windows PC”**, with those observations as reasons. Do not
infer Windows version, hardware model or manufacturer from them. Linux
[xrdp also implements RDP on port 3389](https://github.com/neutrinolabs/xrdp), so the inference
must be reviewable and can legitimately remain Unknown when evidence conflicts.

## Confidence and user authority

Keep three distinct measures: whether observations match the same device, strength of a
type/OS suggestion, and confidence/provenance of a topology relationship. The existing mock
percentage cannot become the production scoring algorithm.

If a number is shown before validation, label it **AI confidence score — uncalibrated** and
retain its model origin; do not describe it as the measured chance of correctness. An LLM's
self-reported number is not calibration. NLI label-normalised scores and embedding similarity
also do not supply that probability. Model quality can be evaluated on a representative held-out
set without training a new model.

Proposed separate user controls:

| Control | Proposed initial setting, awaiting approval |
| --- | --- |
| Produce suggestions for review | Available on demand |
| Fill missing inferred fields | User opt-in |
| Refresh previously inferred values | User opt-in; keep observed/manual values distinct |
| Replace user-entered fields | Off until explicitly enabled for selected fields; retain previous values and undo |
| Add a new finding to the map automatically | Off until user enables policy and quality thresholds are supported |

Auto-add requires a valid scoped observation and resolved identity, not only an AI guess.
Ambiguous identity/duplicate findings still enter review. The model cannot choose its own
threshold, bypass a disabled policy, merge/delete devices or promote inferred topology to
observed adjacency. These are proposed application boundaries, while the user's permission
to choose field replacements is a confirmed requirement.

Suggested evaluation cases include Linux RDP servers, renamed Windows hosts, generic
hostnames, shared/vendor NICs, routed hosts without MACs, missing services, spoofed banners,
unknown product families and contradictory evidence. Record false positives and abstention
separately for each field; automatic changes need a stricter acceptance bar than suggestions.
No published model card establishes these application-specific results today.

Ship model/tokenizer/runtime files with pinned revisions, hashes and licences. Verify the
packaged app with networking unavailable: no first-run download, external inference, model hub
fallback, telemetry or extra service installation. Verify the actual runtime distribution,
especially ONNX alternatives. A missing/incompatible model should leave manual review usable
and report inference unavailable; never download a substitute silently.

## Blue-sky possibilities — not approved features

| Opportunity | Benefit | Prerequisite and reason to defer |
| --- | --- | --- |
| Explainable multi-source identification | Better suggestions than a hostname/port heuristic; show contradictory clues | Evaluate existing models and evidence quality first; no invented hardware/OS facts or bespoke AI platform |
| Automatic network documentation | A selected map and curated notes could become a useful local inventory document | Requires stable identities, user-owned labels and an agreed export format; avoid an enterprise reporting dashboard |
| Historical changes and comparison | Explain a changed IP, service, device disappearance or moved connection | Needs retention/privacy choices and reliable timestamps; record concise changes, not every packet |
| Managed physical topology | Show actual switch ports, VLAN context, redundant links and wireless associations | Requires named device APIs/SNMP/LLDP evidence and permissions; current tree cannot express every physical graph |
| Local router/DHCP/controller integrations | Improve address continuity and hidden-device discovery with less probing | Vendor-specific authentication/licensing, data trust and availability; never assume all home routers expose an API |
| Semantic inventory search | Find “the printer in the office” across curated labels/notes | Only add an embedding model if measured benefit justifies package/RAM cost; no second model by default |
| Persistent “what changed?” notices | Surface useful changes without repeating whole-network scans | Needs confidence/freshness and alert-noise policy; no continuous active scanning by default |
| IPv6 and multi-address views | Cover modern dual-stack networks and devices with multiple endpoints | Explicitly deferred by user; requires scoped zones, targeted discovery and UI/schema acceptance |

None of these authorises scheduling, cloud sync, remote control, equipment configuration,
vulnerability assessment, passive packet capture or a new UI framework. Future ideas should
extend the current mapper only after the real inventory and first-release requirements are
reliable.
