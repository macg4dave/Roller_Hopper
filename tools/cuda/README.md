# CUDA build cache

The app uses mistral.rs's shared CUDA build root at `cuda-cache/`. Compiled kernel objects remain
there across Cargo profiles and target-directory rebuilds. The directory is ignored by Git.

```sh
cargo tool cuda build
cargo tool cuda rebuild
```

`build` reuses the cache and builds the app with `--features cuda`. `rebuild` is the explicit,
destructive refresh: it removes only the repository's `cuda-cache/` directory, then builds it
again. Use `rebuild` after changing CUDA, cuDNN, MSVC, GPU compute capability or kernel flags.

For AI agents, `cuda-cache/` is protected. Ordinary builds, runs, tests and gates use it without
special permission. A cache miss, first population or invalidation requires explicit user
permission; `rebuild` additionally deletes the existing cache. If CUDA compilation starts
unexpectedly, stop and report the miss; do not retry it or bypass the cache with a different target
directory or CPU-only build.

The command uses every logical CPU reported by the operating system unless
`CARGO_BUILD_JOBS` is already set, in which case that explicit limit wins.
