# Benchmarks

Measured 2026-11-16 and 2026-11-17 on an Apple M1 (4 performance + 4
efficiency cores, 8 GB, integrated 8-core GPU), macOS 26.5.1, Metal backend
through wgpu 30. Rugi is built with `--profile dist` (fat LTO, rustc 1.95)
and Dear ImGui with `-O3 -flto` (Apple clang 21). Power source and thermal
state were not controlled. CPU-only numbers vary by roughly ±10% between runs,
GPU-interleaved runs by more (see the notes in each section).

To reproduce:

```bash
cargo run --profile dist -p rugi-benchmarks --features egui
RUGI_BENCH_SCALE=1 cargo run --profile dist -p rugi-benchmarks
git clone --depth 1 https://github.com/ocornut/imgui /tmp/imgui
benchmarks/imgui/run_imgui_bench.sh /tmp/imgui
```

## Workloads

A 1280×800 logical window with a scroll area filling the central region and N
rows. Each row has a label, a button, a checkbox and a 200 px slider (4
widgets). Steady state, no input, 1 s unmeasured warm-up, 100 warm-up frames,
1000 measured frames. Each library rebuilds the whole UI every frame and
produces renderer-ready output:

| Library | Measured span | Output |
|---|---|---|
| Rugi | `begin_frame` … `end_frame` | GPU instances |
| Dear ImGui 1.93.1 | `NewFrame` … `Render` | `ImDrawData` |
| egui 0.37 | `run_ui` + `tessellate` | meshes |

The workloads are implemented identically in `benchmarks/src/main.rs` and
`benchmarks/imgui/imgui_bench.cpp`:

- `static(N)`: precomputed labels (`ImGui::TextUnformatted`). Engine cost only.
- `fmt(N)`: every row formats `"Row %d: %.1f"` every frame into a reused
  buffer (`ImGui::Text` / Rust `write!` into a `String`). Engine plus
  formatting cost, no allocations on either side.
- `lazy(N)`: Rugi's `ui.label_fmt(format_args!(...))` formats only visible
  rows. Dear ImGui has no equivalent.

Both sides are built at full optimization: the Rust `dist` profile
(`lto = "fat"`, `codegen-units = 1`) and Dear ImGui with `-O3 -flto`.

## CPU frame build

Median of 7 alternating runs, in µs, measured 2026-11-18 against Dear ImGui
master (`1.93.1 WIP`, commit `77fe458`):

| Workload | Rugi | Dear ImGui 1.93.1 | Rugi speed-up | egui 0.37 |
|---|---:|---:|---:|---:|
| static(20): every row visible | 6.3 | 13.4 | 2.1× | – |
| static(50) | 7.5 | 24.5 | 3.3× | – |
| static(200) | 13.6 | 42.3 | 3.1× | – |
| static(1000) | 45.4 | 139.4 | 3.1× | – |
| fmt(50) | 12.4 | 31.5 | 2.5× | 371 |
| fmt(200) | 32.9 | 69.2 | 2.1× | 1,133 |
| fmt(1000) | 138.7 | 267.5 | 1.9× | 5,632 |
| lazy(50) / (200) / (1000), vs ImGui fmt | 9.0 / 15.0 / 47.0 | 31.5 / 69.2 / 267.5 | 3.5× / 4.6× / 5.7× | – |

These numbers include the robustness checks added on 2026-11-18 (every
instance is checked for non-finite values before it is queued); they cost
about 0.3 µs at static(20).

The egui column is from the 2026-11-16 run and was not re-measured. Against
Dear ImGui 1.91.5 the ratios are higher (up to 4.7×) because that release is
slower; 1.93.1 is the baseline used here.

Rugi makes 0 allocations per frame in every workload above; the
`alloc_probe` test enforces this for a 500-row UI. egui makes 36,871
allocations (9.1 MiB) per frame at 1000 rows.

How to read the results:

- `static` measures engine work: Rugi's frame build is 2.2–3.5× faster than
  Dear ImGui's on this workload.
- `fmt(1000)` is the only case under 2×. Most of its time goes to formatting
  1000 floats with Rust's `{:.1}`, which is slower than libc's `vsnprintf`
  and runs in user code. Rugi's own share is the `static(1000)` number.
- `lazy` shows deferred formatting, which Rust's `format_args!` makes
  possible. It is a Rugi feature, not a like-for-like comparison.

This is one synthetic workload on one Apple M1, and the two libraries produce
different output (80-byte SDF instances versus triangle vertices). Measure
your own UI before relying on these ratios.

Text: shaping a new 36-character string costs ~15 µs (once); a cached layout
lookup costs ~0.07 µs.

### Optimization history

Each step was profiled with macOS `sample`. Times in µs.

| Step | static(1000) | static(20) |
|---|---:|---:|
| v0.1 | ~900 (fmt) | – |
| Id-keyed maps without SipHash, `Rc` theme, word hashing, off-screen skip | ~337 (fmt) | – |
| Byte-scan label parsing, fast atlas hasher | 109 | 35 |
| Skip off-screen widgets before computing ids; no struct moves on `end()`; fast `with_u64` | 98 | 11 |
| Cached glyph runs per layout (pixel-identical, tested), `Rc` layouts, batched glyph append, fixed-point value labels, no `exp()` for settled animations | 90 | 8.1 |
| Whole-program optimization (LTO) for both libraries, inlining hints | 45 | 7.1 |
| One map lookup for hover/press/focus animation | 45.0 | 6.7 |
| Glass/backdrop counters on every pushed instance (regression, ≈ 6%) | – | 7.1 |
| Counting moved into the effect builders; `DrawList::push` force-inlined (≈ 10%) | 45.2 | 6.1 |
| Non-finite filter in `DrawList::push` as a serial sum of 16 floats (regression) | 55.7 | 11.7 |
| Filter rewritten as independent `x - x` lanes; branch-free size clamp; off-screen widgets join the Tab order only on Tab frames | 45.4 | 6.3 |
| Centered rows, integer widgets, ellipsis for clipped text, plot axis labels: +0.9% / +1.7% instructions per frame (`/usr/bin/time -l`, 790.4k vs 783.4k and 105.9k vs 104.1k), measured instead of time because the machine was under load | – | – |

## GPU (Rugi only)

Offscreen RGBA8 target, timestamp queries. GPU results did not change
materially with the CPU optimizations (sustained 1.58–1.79 ms/frame at
2560×1600 in the latest run).

| Target | Content | GPU pass time, isolated frame (mean) | Sustained, 300 frames back-to-back |
|---|---|---:|---:|
| 2560×1600 | empty (background only) | 2.4–3.2 ms | 1.34 ms/frame |
| 2560×1600 | list(200), 444 instances | 3.1–4.6 ms | 1.52 ms/frame |
| 1280×800 | list(200) | 1.1 ms | 0.50 ms/frame |

- GPU time scales with pixel count, not instance count: the cost is fragment
  shading of overlapping full-screen and panel quads through a large
  uber-shader. Isolated frames run ~2–3× slower than sustained ones, which is
  consistent with the GPU not raising its clock for sparse work (not proven).
- Remaining optimization targets: render the opaque background as the clear
  color, split the uber-shader into specialized rect and glyph pipelines to
  cut register pressure, and reduce interpolated varyings.
- Frame build time measured inside the GPU loop (0.5–1.9 ms) is higher and
  noisier than in the CPU-only table for the same UI. The thread blocks on the
  GPU between frames, so this is likely CPU frequency or core scheduling. Use
  the CPU-only table for comparisons.

## Effects: backdrops and glass

Run with `RUGI_BENCH_EFFECTS=1 cargo run --profile dist -p rugi-benchmarks`;
`RUGI_BENCH_SCALE=1` gives the 1280×800 column. The figures are sustained
time per frame: 300 frames through `Renderer::render_to_view` back to back,
the same path the window uses, including glass snapshots and the final copy.
Each scene shows 50 widget rows.

| Scene | 1280×800 px | 2560×1600 px |
|---|---:|---:|
| no effects | 0.49 ms | 1.57 ms |
| + Aurora backdrop | 0.93 ms | 3.35 ms |
| + Smoke backdrop (domain-warped fbm) | 0.95 ms | 3.45 ms |
| + Smoke + 1 glass panel | 1.63 ms | 6.61 ms |
| + Smoke + 3 glass panels | 1.82 ms | 6.89 ms |
| + Smoke + 6 nested glass panels | 2.01 ms | 7.67 ms |

A frame with glass renders offscreen, so the first glass panel costs a
full-screen store/load and a copy. Each additional panel adds a blur chain
(≈ 0.2–0.4 ms at 2560×1600). Runs vary by roughly ±30% with GPU temperature;
the table shows the coolest consecutive run.

A full-window Smoke backdrop at 2560×1600 went from 13.2 ms to 3.45 ms per
frame in three steps:

| Change | Time |
|---|---:|
| First implementation: four lattice hashes per noise octave | 13.2 ms |
| One bilinear tap into a 256×256 noise texture at smoothstep-warped coordinates | 8.0 ms |
| Separate fragment entry points for glass and backdrops, so the uber-shader's register pressure no longer lowers occupancy for every primitive | 7.5 ms |
| Soft backdrops (Aurora, Smoke, Plasma, Mesh) evaluated at half resolution, then composited with a full-resolution rounded mask and grain | 3.45 ms |

A backdrop that overlaps one already in the half-resolution layer, such as
carousel cards over the window background, is drawn at full resolution
instead. This prevents color bleed at the corners; a GPU test checks it.

## Build times (Apple M1, 8 cores)

Builds of the glass example (then named `glass-settings`, now `effects`),
measured on 2026-11-18 while other
applications were running (load average between 3 and 18 during the runs), so
they are upper bounds. Incremental rebuilds after touching one file, with a
warm cache:

| Profile | App file changed | `rugi-ui` changed | `rugi-core` changed |
|---|---:|---:|---:|
| `dev` (`cargo run`; dependencies at `-O3`) | 3.6 s | 2.3 s | 2.7 s |
| `release` (no LTO, engine crates 1 codegen unit) | 1.8 s | 1.2 s | 1.5 s |

`dist` (fat LTO, 1 codegen unit) re-optimizes the whole program, wgpu
included, on every change, which takes about a minute, so it is reserved for
shipping and for the benchmark tables above. `release` stays within about 8%
of its frame-build time by compiling the hot engine crates (`rugi-core`,
`rugi-text`, `rugi-ui`) as a single codegen unit each and by marking small
cross-crate helpers `#[inline]` (`DrawList::clip`, `rect`,
`TextStyle::line_px`, color helpers). Those annotations also speed up
applications that depend on Rugi with Cargo's default release settings.

Clean builds (`cargo clean` first; dependencies already downloaded):

| Profile | Clean build | `target/` size afterwards |
|---|---:|---:|
| `dev` | 135 s | ≈ 1.0 GB |
| `release` | 81 s | ≈ 0.77 GB |

Dependencies in `dev` are optimized but carry only line tables for debug
info. In an earlier measurement on another example, full debug info made the
clean dev build about 3.6 times slower, because optimized wgpu with complete
DWARF is expensive to produce.

Each profile keeps its own cache in `target/`, and incremental caches grow
over time. `cargo clean` resets it.

## Live app (glass example, 60 Hz panel)

Measured with `RUGI_PACING_LOG=1 cargo run --release -p effects` (then named
`glass-settings`). The
window has an animated backdrop behind a glass panel, so it renders and blurs
every frame.

| Metric | Value |
|---|---|
| Pacing | 60.0 fps mean, p99 18.2–19.5 ms |
| Frame build | 20–48 µs on the CPU |
| Process CPU (`ps`) | 32–41% of one core: GPU submission, the full-screen blur and the window server |
| Resident memory | ≈ 95 MB |

Animated backdrops request frames through `AmbientRate`: by default the full
refresh rate while focused and 30 fps while another window has focus.
Interaction and widget animations always run at the full rate.

## Frame pacing (windowed, release build, 60 Hz panel)

Measured earlier with a dashboard example that has since been replaced
(continuous animation; the benchmark suite was running at the same time on
the same CPU):

```
[rugi pacing] display Some(60.0) Hz | 240 samples | mean 16.67 ms (60.0 fps) | p99 17.51 ms | build 234 us
[rugi pacing] display Some(60.0) Hz | 240 samples | mean 16.67 ms (60.0 fps) | p99 17.87 ms | build 195 us
```

Refresh rate is read from the monitor, presentation follows it (AutoVsync),
and the measured interval is exposed to the app (`Ui::display_info`, the F12
HUD, `rugi_ui_display_info`). Frame build uses ~0.1–0.4 ms of the 16.7 ms
budget, leaving the same headroom for a 120 Hz (8.3 ms) or 144 Hz (6.9 ms)
display. This has not been measured on a 120/144 Hz panel, as none was
available.

Animations use time-based springs. `rugi_core::anim` tests show identical
settling time and no overshoot at 30, 60, 120, 144 and 240 Hz.

## Idle and interactive behavior (windowed, release build)

- Idle: the Rust demo used 0.54 s CPU over 6 min 03 s wall time (including
  start-up); the C++ demo used 0.34 s over 5 min 33 s. `ps` showed 0.0% CPU
  while idle. RSS ≈ 20–24 MB.
- Continuous 60 fps animation (`--live`): ~10% of one core (1.79 s CPU over
  17 s, including start-up), RSS ≈ 45 MB.
- Event-driven loop: a frame is rendered only on input, while an animation is
  in progress, or when a timer is due (caret blink at 0.53 s).
- Input latency was not measured; that needs photon/high-speed capture or OS
  input timestamps.
