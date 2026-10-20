# Rust crates and supporting research

Research snapshot: 20 October 2026. Versions below are researched candidates, **not dependencies
added to the project or a tested lockfile**. Primary documentation/manifests were checked; no
candidate was compiled, benchmarked or exercised against equipment. Release activity and API
coverage do not establish a security audit or production interoperability.

The app currently resolves Slint/slint-build 1.18.1 and ipnet 2.12.2, with protected Rust 1.94.0.
Prefer current std/Slint facilities before new dependencies; enable only required features.
MSRV means declared minimum Rust version, not proof all selected transitive dependencies build.
All chosen runtime dependencies must execute in process and ship with the app. External
programs, services, scanner/model runners and localhost backends are excluded. Native Open,
Save and Save As are required; JSON snapshots are the revised proposed default rather than a
private database-only inventory.

## Recommended foundation

| Candidate | Purpose and fit over alternatives | Maintenance, compatibility and limits |
| --- | --- | --- |
| Existing **Slint 1.18.1** | Reuse `Bridge`, weak event-loop scheduling and retained `VecModel` updates; avoids a second UI or custom observer framework | Pinned APIs: [Weak](https://docs.rs/slint/1.18.1/slint/struct.Weak.html), [VecModel](https://docs.rs/slint/1.18.1/slint/struct.VecModel.html). Models are UI-thread objects, not worker payloads. No Slint upgrade is needed for the proposed boundary. |
| Existing **std + ipnet 2.12.2** | Address types, prefix arithmetic, clocks, paths and collections; extend existing use instead of adding a second CIDR parser | [ipnet API](https://docs.rs/ipnet/2.12.2/ipnet/) already offers IPv4/IPv6 types and lazy host ranges. Limits/authorisation must be checked before enumeration. |
| **Tokio 1.54.0** | TCP/UDP, timers, bounded channels, task supervision and semaphores; simpler than building a socket/thread scheduler or adding an actor framework | Established runtime; release 19 October 2026, MIT, MSRV 1.85: [manifest](https://raw.githubusercontent.com/tokio-rs/tokio/tokio-1.54.0/tokio/Cargo.toml), [releases](https://raw.githubusercontent.com/tokio-rs/tokio/tokio-1.54.0/tokio/CHANGELOG.md). One dedicated current-thread runtime is an initial I/O candidate; do not enable `full` by default. |
| **tokio-util 0.7.20** | `CancellationToken`/child tokens instead of custom cancellation flags | MIT, MSRV 1.85: [manifest](https://raw.githubusercontent.com/tokio-rs/tokio/tokio-util-0.7.20/tokio-util/Cargo.toml), [API](https://docs.rs/tokio-util/0.7.20/tokio_util/sync/struct.CancellationToken.html). Cooperative cancellation still needs I/O timeouts, owned-task cleanup and a terminal acknowledgement. |
| **netdev 0.46.3** | Cross-platform interface/address/prefix and gateway metadata; richer fit than `if-addrs` alone or deprecated `default-net`; avoids hand-written OS enumeration | Recent releases, MIT; [release record](https://docs.rs/crate/netdev/latest), [manifest](https://raw.githubusercontent.com/shellrow/netdev/main/Cargo.toml). No MSRV declared in inspected manifest. Advertises Windows/Linux/macOS; choose minimal gateway features. Not a remote scanner, general neighbour table or guarantee of per-destination route selection. |
| **mdns-sd 0.21.6** | Cross-platform DNS-SD/mDNS with cache, interface selection and an in-process background thread; avoids implementing DNS wire/TTL behaviour | Released 18 October 2026; MIT/Apache-2.0, MSRV 1.71: [manifest](https://raw.githubusercontent.com/keepsimple1/mdns-sd/v0.21.6/Cargo.toml), [changes](https://raw.githubusercontent.com/keepsimple1/mdns-sd/v0.21.6/CHANGELOG.md). Browses advertised services, not all hosts. Subscription disconnect can indicate overload. [zeroconf](https://docs.rs/zeroconf/latest/zeroconf/) is excluded where it requires separate Bonjour/Avahi services. |
| **hickory-resolver 0.26.3** | Async reverse/forward DNS, TTL/cache and resolver policy; std name resolution alone does not supply the intended reverse-lookup/cache controls | Established DNS project; MIT/Apache-2.0, MSRV 1.88: [tagged manifest](https://raw.githubusercontent.com/hickory-dns/hickory-dns/v0.26.3/Cargo.toml), [API](https://docs.rs/hickory-resolver/latest/hickory_resolver/). Retain system/selected network resolver behaviour; do not add public DNS/DoH by default. Verify split-DNS/VPN behaviour and exact feature set. |
| **serde 1.0.229 + serde_json 1.0.150** | Versioned typed native map snapshots; avoids a custom file parser and keeps one in-memory working document | MIT/Apache-2.0, MSRVs 1.56 and 1.71: [Serde manifest](https://raw.githubusercontent.com/serde-rs/serde/v1.0.229/serde/Cargo.toml), [JSON manifest](https://raw.githubusercontent.com/serde-rs/json/v1.0.150/Cargo.toml). Mature libraries; retain depth limits and add file/record bounds and domain validation. No database or schema service required. |
| **atomic-write-file 0.3.1** | Candidate for temporary-file plus commit/replacement on Save, rather than truncating an existing document | BSD-3-Clause, MSRV 1.85: [manifest](https://raw.githubusercontent.com/andreacorbellini/rust-atomic-write-file/v0.3.1/Cargo.toml), [limits](https://docs.rs/atomic-write-file/latest/atomic_write_file/). Supports Unix/macOS and Windows; symlink/ACL/metadata and durability semantics need explicit handling. Do not claim universal power-loss safety. Alternative tempfile is discussed below; choose one. |
| **rfd 0.17.2** | Native file-picker candidate; reuse OS dialogs rather than launch another program | MIT, no declared MSRV in [manifest](https://raw.githubusercontent.com/PolyMeilex/rfd/0.17.2/Cargo.toml). Windows/macOS native paths; **default Linux portal/Zenity fallback is disallowed**. Investigate [in-process GTK3 backend](https://raw.githubusercontent.com/PolyMeilex/rfd/0.17.2/src/backend/gtk3/file_dialog.rs) with defaults disabled; native dependencies, Slint integration and packaging are not qualified yet. |
| **directories 6.0.0** | Platform-correct per-user data/config/cache paths instead of custom environment-variable rules | MIT/Apache-2.0: [ProjectDirs](https://docs.rs/directories/6.0.0/directories/struct.ProjectDirs.html). Use local-data directory deliberately on Windows, not accidental roaming inventory. [GitHub archive notice](https://github.com/dirs-dev/directories-rs) reflects a move to Codeberg; current Codeberg maintenance and exact MSRV were not verified. Candidate pending that check. |

Tokio's [bounded mpsc](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html) is suitable for
commands/results and [watch](https://docs.rs/tokio/latest/tokio/sync/watch/index.html) for latest
progress; [JoinSet](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html) helps supervise
owned tasks. These replace custom queues/primitives, not the application's run policy.
[Started blocking tasks cannot be aborted](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html);
bound admission and call duration rather than assuming task cancellation kills native I/O.

## Document format and conditional SQLite alternative

The proposed initial native format is versioned JSON, not simultaneous SQLite plus JSON
authorities. File limits, supported-version migration, portable icon resources and revisioned
save acknowledgement belong to [the document contract](backend-architecture.md#native-documents-open-save-and-save-as).
Keep format handling separate from app preferences and derivative recovery snapshots.

**tempfile 3.27.0** is an established alternative if broader temporary-file facilities are
needed: MIT/Apache-2.0, MSRV 1.63 in its
[manifest](https://raw.githubusercontent.com/Stebalien/tempfile/v3.27.0/Cargo.toml).
[Persist](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist)
does not sync contents/directory automatically. Use one vetted save route with deliberate
flush/replacement/error handling, not both save crates.

**rusqlite 0.40.2** becomes conditional on approved historical queries/scale or a deliberate
SQLite-native file format. One worker-owned connection and direct SQL suffice without an ORM
or pool. MIT, release MSRV 1.88: [release](https://github.com/rusqlite/rusqlite/releases/tag/v0.40.2),
[manifest](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/Cargo.toml).
[Bundled SQLite](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/README.md) is an
in-process library, not an external server or program, but adds C build/package requirements.
Keep blocking work off Slint; automatic database commits do not by themselves implement
the requested Save/dirty-state UX.

**SQLx 0.9.0** is a credible alternative if async query/pooling/migration facilities justify
the additional surface. MIT/Apache-2.0 and MSRV 1.94.0 match the project exactly:
[manifest](https://raw.githubusercontent.com/transact-rs/sqlx/v0.9.0/Cargo.toml).
Its SQLite connection already uses a [worker thread](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnection.html).
Its [query macros](https://docs.rs/sqlx/latest/sqlx/macro.query.html) require a schema at build time or maintained offline metadata; plain queries
do not. Prefer the narrower bundled SQLite feature where appropriate. Choose **one** SQLite
stack, not SQLx plus rusqlite.

**Do not combine rusqlite 0.40.2 with an arbitrarily selected migration helper.**
[rusqlite_migration 2.6.0](https://raw.githubusercontent.com/cljoly/rusqlite_migration/v2.6.0/Cargo.toml)
requires Rust 1.95. The older
[2.5 helper depends on rusqlite 0.39](https://raw.githubusercontent.com/cljoly/rusqlite_migration/v2.5.0/rusqlite_migration/Cargo.toml),
so it is not a drop-in partner for 0.40.2. Resolve a compatible maintained pair or use a small
ordered transactional SQL migration sequence; do not change the protected toolchain to avoid
the decision.

WAL is optional, not an initial requirement. It still permits one writer and needs care with
checkpoints/backups; it is unsuitable for a database accessed across a network filesystem.
Verify the resolved SQLite release and current fixes when selecting features.
[SQLite WAL documentation](https://www.sqlite.org/wal.html)

**uuid 1.28.0** is conditional, not needed for IP/MAC matching. It offers portable catalogue
keys under MIT/Apache-2.0, MSRV 1.89:
[manifest](https://raw.githubusercontent.com/uuid-rs/uuid/v1.28.0/Cargo.toml).
For one local database, durable integer keys suffice. Add UUIDs only if portable/merged
catalogues are approved; generated IDs do not discover device identity.

## Conditional collectors and unresolved platform work

| Candidate | Recommendation and comparison | Limits requiring qualification |
| --- | --- | --- |
| **surge-ping 0.9.x** | Evaluate for async ICMP rather than constructing packets; MIT and Tokio-based | [Published docs](https://docs.rs/crate/surge-ping/latest) describe Linux datagram then raw fallback. Search/version caches disagreed between 0.9.0 and 0.9.1 during review; re-resolve exact release/MSRV before selection. Raw permission behaviour is not uniform across OSes. Native Windows ICMP is a separate alternative, not automatic safe Rust glue. |
| **Safe neighbour-table wrapper** | Selection remains open. OS neighbour caches are useful before choosing raw ARP/capture | [crossnet 0.1.1](https://docs.rs/crate/crossnet/latest) explicitly describes itself as early-stage; [net-lattice](https://github.com/F000NKKK/net-lattice) has broad inspection/mutation scope and evolving APIs. Neither was qualified here as a small proven cross-platform dependency. Older `netneighbours`/`ll-neighbors` search results are not maintenance proof. Do not invent a safe wrapper or bypass `unsafe_code = deny`. |
| **pnet 0.35.0 / pcap** | Not the baseline; external capture-driver paths are excluded | [pnet](https://docs.rs/pnet/0.35.0/pnet/) is low-level; [Windows requirements](https://github.com/libpnet/libpnet) introduce WinPcap/Npcap installation, which conflicts with the hard boundary. Ordinary TCP/UDP discovery does not need capture. Only a compliant in-process/native OS path could be considered later. |
| **snmp2 0.5.2** | Conditional read-only SNMP inventory/topology; Tokio and SNMPv3 options avoid implementing BER/USM | MIT/Apache-2.0, MSRV 1.90 in [docs](https://docs.rs/snmp2/latest/snmp2/). Disable MIB tooling requiring Net-SNMP programs/services; any native library must be packaged and in process. Its SHA2 wording conflicts with [RFC 7860](https://datatracker.ietf.org/doc/html/rfc7860); verify algorithms/interoperability and no silent weak fallback. |
| **async-snmp 0.18.1** | Async-first alternative to snmp2; choose only one after named-device evaluation | MIT/Apache-2.0, MSRV 1.88: [manifest](https://raw.githubusercontent.com/lukeod/async-snmp/v0.18.1/Cargo.toml), [policy docs](https://docs.rs/async-snmp/latest/async_snmp/). Review strict source correlation, v3 trust and retry behaviour. Documented features are not security qualification. |
| **ssdp-client 2.1.0** | Existing Tokio SSDP discovery is preferable to hand-building search/response parsing when UPnP enrichment is selected | MIT/Apache-2.0; published April 2025: [package/API](https://docs.rs/crate/ssdp-client/2.1.0). Small project; MSRV and per-interface cancellation/port sharing unverified. Description HTTP/XML collection still needs bounded scope and URL validation. |
| **oxvif 0.17.0** | Candidate for camera-specific read-only enrichment, rather than custom SOAP/WS-Discovery | [Published package](https://docs.rs/crate/oxvif/latest) shows September 2026 activity and broad camera-control surface. Not qualified here for licensing/MSRV/platform/interoperability; do not enable controls merely because the client offers them. |
| **mac_oui 0.4.11 / oui-data 0.2.1** | Shortlist offline manufacturer lookup; choose only after dataset freshness, licence and prefix coverage review | [mac_oui](https://docs.rs/mac_oui/latest/mac_oui/), [oui-data](https://docs.rs/crate/oui-data/0.2.1). Research candidates, not ready selections; exact MSRV/data redistribution conditions unverified. Do not depend on an online lookup or copy Nmap data casually. |

**Nmap and other external scanner executables are excluded** by the confirmed in-app rule,
even if bundled. Their detection databases/licences are not permission to copy them into a
new scanner. The app should reuse compliant protocol libraries and OS APIs, not invoke a
second program or run NSE scripts.

## Embedded inference shortlist

The user wants existing bundled models, not custom training. Select the pretrained artifact
and runtime together; model choices, scores and application policy live in
[the AI plan](backend-opportunities.md). These are alternatives, not a dependency shopping list.

| Rust/runtime route | Fit | Compatibility and limits |
| --- | --- | --- |
| **Candle 0.11.0** | Rust model implementations, including existing Qwen3 and quantized Qwen3 adapters; candidate for CPU inference inside the existing process | MIT/Apache-2.0; [released Qwen3 adapter](https://github.com/huggingface/candle/blob/0.11.0/candle-transformers/src/models/qwen3.rs), [quantized adapter](https://github.com/huggingface/candle/blob/0.11.0/candle-transformers/src/models/quantized_qwen3.rs), [core features](https://github.com/huggingface/candle/blob/0.11.0/candle-core/Cargo.toml). No MSRV declared in inspected manifests, so Rust 1.94 compatibility remains unproven. CPU defaults avoid mandatory GPU features; tokenizer/native build requirements remain. Not a ready-made device-identification pipeline. |
| **llama.cpp as an in-process library only** | Mature CPU GGUF execution and grammar-constrained generation; conditional alternative if Candle's chosen artifact/template path is unsuitable | MIT, native C/C++ build: [build docs](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md). `llama-cpp-2` 0.1.160 has a material [unsafety warning](https://github.com/utilityai/llama-cpp-rs/blob/0.1.160/llama-cpp-2/README.md#disclaimer) and unstable API; no binding is qualified here. Executable/server integration is excluded. Do not weaken the application's unsafe rule. |
| **ort 2.0.0-rc.15** | ONNX Runtime CPU alternative for an existing compact zero-shot classification artifact | MIT/Apache-2.0 wrapper, MSRV 1.92: [manifest](https://github.com/pykeio/ort/blob/v2.0.0-rc.15/Cargo.toml). Release-candidate API; native runtime 1.31 packaging, operator/tokenizer support and telemetry must be deliberate. Default binary downloads are a build/reproducibility concern, not offline application behaviour. |
| **tract 0.23.9** | Smaller-runtime candidate for a supported ONNX graph, if that model works without conversion/custom architecture | MIT/Apache-2.0, MSRV 1.91: [manifest](https://github.com/sonos/tract/blob/v0.23.9/Cargo.toml), [facade features](https://github.com/sonos/tract/blob/v0.23.9/api/rs/Cargo.toml). Disable broad defaults; architecture-specific native kernels remain. Operator coverage is model-specific; not every ONNX export is supported. No compatible device model was established here. |
| **fastembed 7.1.2 + MiniLM** | Existing small embedding models are useful for semantic ranking/search; not the preferred Windows/type inference engine | Apache-2.0, declared MSRV 1.88 but its ort dependency needs 1.92: [manifest](https://raw.githubusercontent.com/Anush008/fastembed-rs/v7.1.2/Cargo.toml). Defaults fetch models/runtime and use broad CPU resources. [Local-file loading](https://docs.rs/fastembed/latest/fastembed/struct.TextEmbedding.html) avoids runtime downloads. Similarity is not calibrated device confidence; a second model is not justified merely as a second vote. |

For ONNX routes, local inference alone does not prove zero outbound traffic.
[ONNX Runtime 1.31 privacy documentation](https://github.com/microsoft/onnxruntime/blob/v1.31.0/docs/Privacy.md)
states official native builds can enable telemetry by default and API suppression may follow
initialisation. Select a telemetry-free build or verify appropriate pre-initialisation controls
for the actual packaged runtime. Do not assume the behaviour of Microsoft and Pyke binary
distributions is identical.

## Selection gate for future implementation

Before adding any candidate: recheck its release/tag, declared licence and MSRV, transitive
features, advisories, platform/architecture support and package size. Run the protected gate
on the actual chosen set when coding is authorised. Keep application `unsafe` denied. Pin
model/runtime artifacts and verify licences/checksums; no automatic model downloads or backend
substitutions. The unresolved platform-wrapper and model-runtime choices are real decisions,
not permission to add speculative abstractions.
