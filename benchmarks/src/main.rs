//! Rugi benchmarks (run with `--profile dist`).
//!
//! ```text
//! cargo run --profile dist -p rugi-benchmarks                 # Rugi CPU + GPU
//! cargo run --profile dist -p rugi-benchmarks --features egui # + egui comparison
//! ```
//!
//! Environment variables:
//! - `RUGI_BENCH_SCALE=<f>`: DPI scale of the GPU benchmarks (default 2).
//! - `RUGI_BENCH_EFFECTS=1`: only the backdrop and glass benchmarks.
//! - `RUGI_BENCH_PROFILE=<rows>`: a 6 s loop for external profilers;
//!   `RUGI_BENCH_STATIC=1` uses static labels in it.
//!
//! Workload `list(N)`: a 1280×800 window whose central area holds a scroll
//! area with N rows; each row has a label with a formatted number, a button,
//! a checkbox and a slider (4 widgets/row). Steady state: no input, so the
//! measurement is the per-frame cost of rebuilding the UI and producing
//! GPU-ready output (Rugi: instances; egui: tessellated meshes).
//! `benchmarks/imgui` runs the same workload with Dear ImGui.

use rugi_core::Vec2;
use rugi_ui::{Backdrop, BackdropKind, ButtonKind, FrameInput, GlassStyle, Rect, Ui, UiOptions};
use rugi_winit::HeadlessRunner;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const WARMUP: usize = 100;
const FRAMES: usize = 1000;
/// Frames per GPU measurement.
const GPU_FRAMES: u32 = 300;
const WINDOW: Vec2 = Vec2::new(1280.0, 800.0);
const DT: f32 = 1.0 / 60.0;
const CPU_INPUT: FrameInput = FrameInput { size: WINDOW, scale: 2.0, dt: DT };

// ---------------------------------------------------------------------------
// Allocation counting

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

/// Forwards to the system allocator and counts allocations and bytes.
struct CountingAllocator;

// SAFETY: forwards to the system allocator unchanged; only counts calls.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// `(allocations, bytes)` since the program started.
fn alloc_counts() -> (u64, u64) {
    (ALLOCS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed))
}

// ---------------------------------------------------------------------------
// Statistics

struct Summary {
    mean: f64,
    p50: f64,
    p99: f64,
}

fn summarize(mut us: Vec<f64>) -> Summary {
    us.sort_by(f64::total_cmp);
    let mean = us.iter().sum::<f64>() / us.len() as f64;
    let p = |q: f64| us[((us.len() as f64 - 1.0) * q).round() as usize];
    Summary { mean, p50: p(0.5), p99: p(0.99) }
}

// ---------------------------------------------------------------------------
// The list(N) workload

/// How each row builds its label.
#[derive(Clone, Copy)]
enum Labels {
    /// Pre-formatted strings.
    Static,
    /// Formatted into a reused buffer every frame, like `ImGui::Text("Row %d: %.1f")`.
    Formatted,
    /// Rugi's lazy `label_fmt`.
    Lazy,
}

impl Labels {
    fn tag(self) -> &'static str {
        match self {
            Labels::Static => "static",
            Labels::Formatted => "fmt   ",
            Labels::Lazy => "lazy  ",
        }
    }
}

struct Rows {
    checks: Vec<bool>,
    values: Vec<f32>,
    labels: Vec<String>,
    buf: String,
    mode: Labels,
}

impl Rows {
    fn new(n: usize, mode: Labels) -> Self {
        Self {
            checks: vec![false; n],
            values: (0..n).map(|i| (i % 100) as f32).collect(),
            labels: (0..n).map(|i| format!("Row {i}: {:.1}", (i % 100) as f32)).collect(),
            buf: String::with_capacity(64),
            mode,
        }
    }
}

fn rugi_list(ui: &mut Ui, s: &mut Rows) {
    ui.central_panel(|ui| {
        ui.scroll_area("list", None, |ui| {
            for i in 0..s.checks.len() {
                ui.push_id_u64(i as u64);
                ui.horizontal(|ui| {
                    match s.mode {
                        Labels::Lazy => {
                            ui.label_fmt(format_args!("Row {i}: {:.1}", s.values[i]));
                        }
                        Labels::Formatted => {
                            use std::fmt::Write as _;
                            s.buf.clear();
                            let _ = write!(s.buf, "Row {i}: {:.1}", s.values[i]);
                            ui.label(&s.buf);
                        }
                        Labels::Static => {
                            ui.label(&s.labels[i]);
                        }
                    }
                    ui.button_kind("Action", ButtonKind::Secondary);
                    ui.checkbox("On", &mut s.checks[i]);
                    ui.set_next_item_width(200.0);
                    ui.slider("Value", &mut s.values[i], 0.0, 100.0);
                });
                ui.pop_id();
            }
        });
    });
}

// ---------------------------------------------------------------------------
// CPU benchmarks

/// Builds `list(N)` repeatedly for about a second, unmeasured, so the first
/// measured workload does not pay for CPU clock ramp-up. The Dear ImGui
/// benchmark does the same.
fn warm_up_cpu() {
    let mut ui = Ui::new(UiOptions::default());
    let mut s = Rows::new(200, Labels::Formatted);
    let t = Instant::now();
    while t.elapsed().as_secs_f32() < 1.0 {
        ui.begin_frame(CPU_INPUT).unwrap();
        rugi_list(&mut ui, &mut s);
        ui.end_frame();
    }
}

fn bench_rugi_cpu(n: usize, mode: Labels) {
    let mut ui = Ui::new(UiOptions::default());
    let mut s = Rows::new(n, mode);
    let mut times = Vec::with_capacity(FRAMES);
    let (mut allocs, mut bytes) = (0u64, 0u64);
    let mut last = Default::default();
    for f in 0..WARMUP + FRAMES {
        let (a0, b0) = alloc_counts();
        let t = Instant::now();
        ui.begin_frame(CPU_INPUT).unwrap();
        rugi_list(&mut ui, &mut s);
        let out = ui.end_frame();
        let el = t.elapsed().as_secs_f64() * 1e6;
        if f >= WARMUP {
            times.push(el);
            let (a1, b1) = alloc_counts();
            allocs += a1 - a0;
            bytes += b1 - b0;
            last = out.stats;
        }
    }
    let s = summarize(times);
    println!(
        "rugi  {}({n:>4}) CPU build: mean {:>7.1} us  p50 {:>7.1}  p99 {:>7.1} | {:>6} instances {:>3} batches {:>5} culled | allocs/frame {:>6.1} ({:.1} KiB)",
        mode.tag(),
        s.mean, s.p50, s.p99, last.instances, last.batches, last.culled,
        allocs as f64 / FRAMES as f64, bytes as f64 / FRAMES as f64 / 1024.0
    );
}

#[cfg(feature = "egui")]
fn bench_egui(n: usize) {
    let ctx = egui::Context::default();
    let mut s = Rows::new(n, Labels::Formatted);
    let raw = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(WINDOW.x, WINDOW.y))),
        ..Default::default()
    };
    let mut times = Vec::with_capacity(FRAMES);
    let (mut allocs, mut bytes) = (0u64, 0u64);
    let mut meshes = 0;
    for f in 0..WARMUP + FRAMES {
        let (a0, b0) = alloc_counts();
        let t = Instant::now();
        let out = ctx.run_ui(raw(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in 0..s.checks.len() {
                        ui.push_id(i, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("Row {i}: {:.1}", s.values[i]));
                                let _ = ui.button("Action");
                                ui.checkbox(&mut s.checks[i], "On");
                                ui.add(egui::Slider::new(&mut s.values[i], 0.0..=100.0).text("Value"));
                            });
                        });
                    }
                });
            });
        });
        let clipped = ctx.tessellate(out.shapes, 2.0);
        let el = t.elapsed().as_secs_f64() * 1e6;
        if f >= WARMUP {
            times.push(el);
            let (a1, b1) = alloc_counts();
            allocs += a1 - a0;
            bytes += b1 - b0;
            meshes = clipped.len();
        }
    }
    let s = summarize(times);
    println!(
        "egui   list({n:>4}) CPU run+tessellate: mean {:>7.1} us  p50 {:>7.1}  p99 {:>7.1} | {:>6} clipped meshes | allocs/frame {:>6.1} ({:.1} KiB)",
        s.mean, s.p50, s.p99, meshes, allocs as f64 / FRAMES as f64, bytes as f64 / FRAMES as f64 / 1024.0
    );
}

/// Shaping cost of new strings versus the layout cache.
fn bench_text_layout() {
    const STRINGS: usize = 2000;
    let mut ui = Ui::new(UiOptions::default());
    let style = ui.theme().typography.body;
    let layout_all = |ui: &mut Ui| {
        let t = Instant::now();
        for i in 0..STRINGS {
            ui.layout_text(&format!("Unique label number {i} with some words"), &style, None);
        }
        t.elapsed().as_secs_f64() * 1e6 / STRINGS as f64
    };
    let cold = layout_all(&mut ui);
    let warm = layout_all(&mut ui);
    println!("rugi  text layout (36 chars): cold shape {cold:.2} us/string, cached {warm:.2} us/string");
}

/// Builds `list(N)` in a loop for 6 s, for external profilers (`sample`, `perf`).
fn profile_loop(n: usize, mode: Labels) {
    let mut ui = Ui::new(UiOptions::default());
    let mut s = Rows::new(n, mode);
    let t = Instant::now();
    let mut frames = 0u64;
    while t.elapsed().as_secs_f32() < 6.0 {
        ui.begin_frame(CPU_INPUT).unwrap();
        rugi_list(&mut ui, &mut s);
        ui.end_frame();
        frames += 1;
    }
    println!("profile loop: {:.1} us/frame", t.elapsed().as_secs_f64() * 1e6 / frames as f64);
}

// ---------------------------------------------------------------------------
// GPU benchmarks

fn gpu_scale() -> f32 {
    std::env::var("RUGI_BENCH_SCALE").ok().and_then(|s| s.parse().ok()).unwrap_or(2.0)
}

/// `"WxH"` in physical pixels.
fn pixel_size(scale: f32) -> String {
    format!("{}x{}", (WINDOW.x * scale) as u32, (WINDOW.y * scale) as u32)
}

fn headless_runner(benchmark: &str, scale: f32) -> Option<HeadlessRunner> {
    HeadlessRunner::new(UiOptions::default(), WINDOW, scale).map_err(|e| println!("{benchmark} skipped: {e}")).ok()
}

fn bench_rugi_gpu(n: usize) {
    let scale = gpu_scale();
    let Some(mut runner) = headless_runner("rugi GPU benchmark", scale) else {
        return;
    };
    let mut s = Rows::new(n, Labels::Formatted);
    let mut build = Vec::new();
    let mut wall = Vec::new();
    let mut gpu = Vec::new();
    for f in 0..WARMUP + GPU_FRAMES as usize {
        let t = Instant::now();
        runner.frame(DT, |ui| rugi_list(ui, &mut s));
        let b = t.elapsed().as_secs_f64() * 1e6;
        if f == WARMUP {
            println!("  (instances in frame: {})", runner.ui.output().stats.instances);
        }
        let (w, g) = runner.render_offscreen_timed().unwrap();
        if f >= WARMUP {
            build.push(b);
            wall.push(w as f64);
            if let Some(g) = g {
                gpu.push(g);
            }
        }
    }
    let throughput = runner.render_offscreen_throughput(GPU_FRAMES).unwrap();
    let b = summarize(build);
    let w = summarize(wall);
    let info = runner.adapter_info();
    let gpu_txt = if gpu.is_empty() {
        "GPU pass time: n/a (no timestamp queries)".to_string()
    } else {
        let g = summarize(gpu);
        format!("GPU pass time mean {:.1} us p99 {:.1} us", g.mean, g.p99)
    };
    println!(
        "rugi  list({n:>4}) {} on {} ({:?}): build mean {:.1} us | prepare+submit+wait mean {:.1} us | {gpu_txt} | sustained {throughput:.1} us/frame ({GPU_FRAMES} back-to-back) | draw calls {}",
        pixel_size(scale), info.name, info.backend, b.mean, w.mean, runner.render_stats().draw_calls
    );
}

/// GPU cost of effects: sustained frame time for a plain frame, a
/// full-window animated backdrop, and backdrop plus glass panels (each glass
/// panel needs a snapshot and a blur chain).
fn bench_effects_gpu() {
    let scale = gpu_scale();
    let Some(mut runner) = headless_runner("effects benchmark", scale) else {
        return;
    };
    let mut s = Rows::new(50, Labels::Formatted);
    let cases: [(&str, Option<BackdropKind>, u32); 6] = [
        ("50 rows, no effects", None, 0),
        ("backdrop aurora", Some(BackdropKind::Aurora), 0),
        ("backdrop smoke (domain-warped fbm)", Some(BackdropKind::Smoke), 0),
        ("backdrop smoke + 1 glass panel", Some(BackdropKind::Smoke), 1),
        ("backdrop smoke + 3 glass panels", Some(BackdropKind::Smoke), 3),
        ("backdrop smoke + 6 glass (nested cards)", Some(BackdropKind::Smoke), 6),
    ];
    for (name, backdrop, glass_panels) in cases {
        let mut frame = |ui: &mut Ui| {
            if let Some(kind) = backdrop {
                ui.backdrop(&Backdrop::new(kind));
            }
            for i in 0..glass_panels {
                let r = Rect::from_xywh(40.0 + i as f32 * 60.0, 40.0 + i as f32 * 40.0, 700.0, 500.0);
                ui.glass(r, &GlassStyle::frosted());
            }
            rugi_list(ui, &mut s);
        };
        for _ in 0..WARMUP {
            runner.frame(DT, &mut frame);
        }
        let us = runner.render_offscreen_throughput(GPU_FRAMES).unwrap();
        println!(
            "effects  {name:<40} {}: sustained {us:>7.1} us/frame | draw calls {}",
            pixel_size(scale),
            runner.render_stats().draw_calls
        );
    }
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("warning: run benchmarks with --profile dist");
    }
    if std::env::var_os("RUGI_BENCH_EFFECTS").is_some() {
        bench_effects_gpu();
        return;
    }
    if let Ok(n) = std::env::var("RUGI_BENCH_PROFILE") {
        let mode = if std::env::var_os("RUGI_BENCH_STATIC").is_some() { Labels::Static } else { Labels::Formatted };
        profile_loop(n.parse().unwrap_or(1000), mode);
        return;
    }

    println!("CPU: {}", std::env::consts::ARCH);
    warm_up_cpu();
    for n in [20, 50, 200, 1000] {
        bench_rugi_cpu(n, Labels::Static);
    }
    for n in [50, 200, 1000] {
        bench_rugi_cpu(n, Labels::Lazy);
    }
    for n in [50, 200, 1000] {
        bench_rugi_cpu(n, Labels::Formatted);
        #[cfg(feature = "egui")]
        bench_egui(n);
    }
    bench_text_layout();
    // n = 0 measures the fixed cost: full-screen background, submit, wait.
    for n in [0, 200, 1000] {
        bench_rugi_gpu(n);
    }
    bench_effects_gpu();
}
