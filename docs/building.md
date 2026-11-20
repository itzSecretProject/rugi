# Building and running

## Requirements

- Rust 1.90 or later (developed on 1.95). The minimum comes from the dependency tree (`ordered-float`, through cosmic-text).
- For the C and C++ targets: CMake 3.20+ and a C++17 compiler.

## Build profiles

| Profile | Incremental rebuild | First build | Use |
|---|---|---|---|
| `cargo run` (dev) | 1–3 s | 93 s | Debug engine, optimized dependencies |
| `--release` | < 1 s | 56 s | Day-to-day work; about 8% slower than `dist` |
| `--profile dist` | ~55 s | – | Shipping and benchmarks: fat LTO, one codegen unit |

Times are from an Apple M1. Per-crate rebuild times and the reasoning behind
each profile are in [benchmarks.md](benchmarks.md#build-times-apple-m1-8-cores).

## Examples

| Package | What it shows |
|---|---|
| `rugi-demo` | The built-in demo, `ui.show_demo()` |
| `disk-usage` | A disk-usage explorer: scans a folder on a background thread and lists what takes the space |
| `effects` | Glass panels and animated backdrops in a borderless window |

Run any of them with `cargo run --release -p <package>`. `disk-usage` takes
the folder to scan as its first argument (default: the current folder).

## Headless screenshots

Every example can render offscreen, without a window, and replay scripted
input through the normal input path:

```bash
cargo run --release -p disk-usage -- crates --screenshot shot.png --sleep 500 --click rugi-ui --inspect
```

`--click`, `--rclick`, `--hover`, `--type` and `--key` script input;
`--inspect` prints the per-widget data (kind, label, rect, state) recorded for
the frame; `--sleep <ms>` keeps rendering in real time, for work that runs on
another thread. All flags are listed in `examples/common/src/lib.rs`.

## C and C++

CMake drives cargo to build the Rust library, then builds the C++ examples and
tests:

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
./build/rugi_cpp_demo                # C++ dashboard
./build/rugi_cpp_demo --showcase     # the built-in demo through the C ABI
./build/rugi_cpp_glass               # the glass settings window written in C++
```

On Windows, CMake copies `rugi_ffi.dll` next to the executables. This path is
written but untested.

## Tests

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
ctest --test-dir build --output-on-failure
```

The Rust suite covers core, text, layout, widgets, popups, effects, the FFI
layer, C header sync, GPU pixel output (including glass blur) and the
zero-allocation guarantee. It also includes:

- `crates/rugi-ui/tests/fuzz.rs`: seeded random input (NaN and infinite
  coordinates, extreme sizes and scales, hostile Unicode, random themes) over
  every demo page, checking that every frame is well formed.
- `crates/rugi-ui/tests/keyboard.rs`: Tab and Shift+Tab reach every focusable
  widget on every demo page, and Escape closes popups and dialogs.
  `keyboard_widgets.rs` covers table selection, context menus from the
  keyboard, arrow keys in menus and the color picker.
- `crates/rugi-theme/tests/contrast.rs`: every built-in theme meets WCAG 2.2
  AA (4.5:1 for text, 3:1 for outlines and indicators). Run it with
  `-- --nocapture` to print every ratio.

`ctest` runs a C++ interop test, a C11 smoke test, a hostile caller in C and
C++ that calls all exported functions with invalid handles, NULL and oversized
arguments, and checks that `rugi.h` and `rugi.hpp` compile warning-free as
C99, C11, C++17 and C++20.

Longer fuzz campaign, and the C/C++ suite under AddressSanitizer and UBSan:

```bash
RUGI_FUZZ_FRAMES=200000 RUGI_FUZZ_SEEDS=8 cargo test --release -p rugi-ui --test fuzz
cmake -S . -B build-asan -DRUGI_SANITIZE=ON -DRUGI_CARGO_PROFILE=dev
cmake --build build-asan && ctest --test-dir build-asan --output-on-failure
```

The Rust code itself is not instrumented by the sanitizers (that needs a
nightly toolchain); they check every allocation and memory access the C and
C++ callers make, including those into the library.

## Frame pacing log

```bash
RUGI_PACING_LOG=1 cargo run --release -p effects
```

Prints the display refresh rate, mean and p99 frame interval and frame build
time every 2 s while animating. It also reports the chosen surface format and
alpha mode.
