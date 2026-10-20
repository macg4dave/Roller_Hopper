# Development checks

Run the repository's required checks through one ordered gate:

```sh
cargo tool checks --help
cargo tool checks gate --help
cargo tool checks gate
cargo tool checks gate --scope tools
cargo tool checks gate --scope app --dry-run
```

Each crate runs fmt check, strict clippy, tests and build, in that order. The default scope
checks tools first and then the app. Any failure stops the gate and produces a non-zero exit
code; Cargo output is inherited. `--dry-run` prints the plan without running checks.

The gate does not format or repair source files. Real checks can update Cargo lock files and
build artefacts. `cli.rs` dispatches commands; `gate.rs` owns the shared check plan.
