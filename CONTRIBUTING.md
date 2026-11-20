# Contributing to Rugi

Thanks for taking the time to contribute. Bug reports, fixes, documentation
and new widgets are all welcome. For a large change (a new subsystem, an API
redesign), open an issue first so the approach can be agreed before you
write the code.

## Building

You need Rust 1.90 or later. The C and C++ parts also need CMake 3.20+ and a
C++17 compiler. On Linux, install `libxkbcommon-dev`, `libwayland-dev` and
`libfontconfig1-dev` (Debian and Ubuntu names).

```bash
cargo run -p rugi --example hello
cargo run --release -p rugi-demo
```

`cargo run --release` rebuilds in about a second; `--profile dist` is the
fully optimized build used for benchmarks. See
[docs/building.md](docs/building.md).

## Checks

CI runs these on macOS, Windows and Linux. Run them before opening a pull
request:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
cmake -S . -B build && cmake --build build && ctest --test-dir build --output-on-failure
```

The test suite includes a fuzzer for the UI, keyboard reachability of every
demo page, WCAG contrast of every theme, and a hostile C caller that runs the
whole C ABI under AddressSanitizer:

```bash
RUGI_FUZZ_FRAMES=100000 cargo test --release -p rugi-ui --test fuzz
cmake -S . -B build-asan -DRUGI_SANITIZE=ON -DRUGI_CARGO_PROFILE=dev
cmake --build build-asan && ctest --test-dir build-asan --output-on-failure
```

## Guidelines

- **Fix the cause.** A bug fix comes with a test that fails before it and
  passes after.
- **No allocations per frame.** `crates/rugi-ui/tests/alloc_probe.rs` enforces
  it; reuse buffers instead.
- **Measure performance changes.** For anything on the frame path, compare
  `cargo run --profile dist -p rugi-benchmarks` before and after, on an idle
  machine, and put the numbers in the pull request.
- **Keyboard and contrast.** New interactive widgets must be reachable with
  Tab and usable from the keyboard (`tests/keyboard.rs`). Theme colors must
  pass `crates/rugi-theme/tests/contrast.rs`.
- **C ABI.** Changes are additive within an ABI version: never rename, remove
  or reorder an exported function, struct field or constant. Regenerate the
  header with `RUGI_BLESS_HEADER=1 cargo test -p rugi-ffi header`.
- **Comments and docs.** Comments explain why, not what. Doc comments start
  with one sentence that says what the item does. Keep the docs plain and
  factual; numbers in them must come from a measurement.
- **Changelog.** Add a line under `Unreleased` in
  [CHANGELOG.md](CHANGELOG.md) for anything a user would notice.

## License

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in Rugi by you, as defined in the Apache-2.0 license,
shall be dual licensed as MIT OR Apache-2.0, without any additional terms or
conditions.
