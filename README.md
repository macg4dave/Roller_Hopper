# Network Mapper

A Rust + Slint desktop **UI prototype** of a local network mapper. It uses in-memory mock data
only — there is no real network discovery or scanning.

## Run

```sh
cargo run
```

## Contributing (humans and AI agents)

Read [AGENTS.md](AGENTS.md) first. It indexes the guides in [docs/agents/](docs/agents/) — the
product brief, the Rust/Slint standards, the delete → combine → add workflow and how agents share
the repo — and holds the quality gate every change must pass:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build
```

Design reference: [docs/design/reference-mockup.png](docs/design/reference-mockup.png).
