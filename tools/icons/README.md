# Runtime icon library

Inspect the paths and identifiers used by the runtime icon picker:

```sh
cargo tool icons --help
cargo tool icons audit --help
cargo tool icons audit
cargo tool icons audit --directory path/to/icons
```

The audit reads files and reports SVGs below the picker's supported depth and case-insensitive
conflicts in extension-free icon IDs. It returns a non-zero exit code if it finds issues.
Matching names in different groups are valid. Non-SVG files are ignored and symbolic links
are not followed. SVG content and rendering are not validated.

The default directory is this repository's `icons/`. An explicit relative `--directory` is
relative to the current directory. No files are changed. `cli.rs` dispatches commands and
`audit.rs` implements the checks.
