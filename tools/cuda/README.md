# CUDA build

The app uses Candle's CUDA backend. Cargo keeps compiled dependencies in its normal target
directory.

```sh
cargo tool cuda build
```

The command runs the default CUDA build with every logical CPU reported by the operating system,
unless `CARGO_BUILD_JOBS` already sets an explicit limit.
