# Development tools

This standalone Rust crate does not import or modify the product. Run commands from the
repository root:

```sh
cargo tool --help
cargo tool repo gate
cargo tool repo gate --scope tools
cargo tool repo gate --scope app --dry-run
cargo tool icons audit
cargo tool icons audit --directory path/to/icons
```

`repo gate` runs the four required checks in order (fmt check, strict clippy, tests, build),
first for tools and then for the app. It stops at the first failure and inherits Cargo's output.
It does not format or repair source files. `--dry-run` prints the plan; it performs no checks.
Cargo itself may update lock files and build artefacts during a real gate.

`icons audit` reads the library, reports SVG files below the picker's supported depth and
case-insensitive conflicts in extension-free icon IDs, then fails if it found issues. Icons
with the same name in different groups are valid. Non-SVG files are ignored; SVG content and
rendering are not checked. Symbolic links are not followed. An explicit relative directory is
relative to your current directory; the default always points to this repository's `icons/`.

Dependencies and build output are separate from the application. `clap` supplies argument
parsing and help; `walkdir` supplies directory traversal. No product dependencies are added.
