//! Seeded input fuzzer for `Ui::show_demo`.
//!
//! Every frame feeds a random batch of hostile input (non-finite pointer
//! positions, unbalanced button releases, every key with random modifiers,
//! awkward Unicode, focus loss) under random frame parameters (zero, tiny and
//! huge windows, odd scale factors, garbage `dt`), then checks that the frame
//! output is well formed. `RUGI_FUZZ_FRAMES` and `RUGI_FUZZ_SEED` select a
//! longer campaign; `RUGI_FUZZ_SEEDS` runs that many consecutive seeds.

use rugi_core::{InputEvent, Key, Modifiers, PointerButton};
use rugi_ui::{AmbientRate, DemoPage, FrameInput, Theme, Ui, UiOptions, Vec2};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

struct LiveBytes;
static LIVE: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator; only tracks the live byte count.
unsafe impl GlobalAlloc for LiveBytes {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size(), Ordering::Relaxed);
        // SAFETY: caller's contract.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        // SAFETY: caller's contract.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        LIVE.fetch_add(n, Ordering::Relaxed);
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        // SAFETY: caller's contract.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: LiveBytes = LiveBytes;

const DEFAULT_FRAMES: u64 = 3000;
const DEFAULT_SEED: u64 = 0x6b61_6972_6f00_0001;
/// Upper bound on instances per frame; the demo draws a few thousand.
const MAX_INSTANCES: usize = 200_000;
/// Upper bound on live heap bytes at any sampled frame.
const MAX_LIVE_BYTES: usize = 512 << 20;

const PAGES: [DemoPage; 9] = [
    DemoPage::Welcome,
    DemoPage::Basics,
    DemoPage::Inputs,
    DemoPage::Layout,
    DemoPage::Popups,
    DemoPage::Data,
    DemoPage::Style,
    DemoPage::Fonts,
    DemoPage::Metrics,
];

const TEXTS: &[&str] = &[
    "a",
    " ",
    "Hello, world",
    "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
    "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}",
    "e\u{0301}\u{0302}\u{0303}\u{0304}\u{0305}\u{0306}\u{0307}",
    "\u{0301}",
    "\u{05E9}\u{05DC}\u{05D5}\u{05DD} \u{0627}\u{0644}\u{0633}\u{0644}\u{0627}\u{0645}",
    "\u{202E}abc\u{202C}",
    "\u{6F22}\u{5B57}\u{304B}\u{306A}\u{D55C}\u{AE00}",
    "\0",
    "a\0b",
    "\r\n",
    "line1\nline2\rline3\tend",
    "\u{FEFF}\u{200B}\u{200C}\u{200D}",
    "\u{FFFD}\u{10FFFF}\u{E000}",
    "\u{0BF5}\u{A9C1}\u{1D11E}\u{1F600}",
    "\u{0E01}\u{0E34}\u{0E4D}\u{0E48}",
    "\u{092E}\u{0948}\u{0902}",
    "%s%n{}{{}}\\",
    "1e309",
    "-0",
    "NaN",
];

/// SplitMix64: small, fast, deterministic.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, p: f64) -> bool {
        ((self.next() >> 11) as f64 / (1u64 << 53) as f64) < p
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// A coordinate: mostly in range, sometimes hostile.
fn coord(rng: &mut Rng, extent: f32) -> f32 {
    match rng.below(20) {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => f32::NEG_INFINITY,
        3 => -rng.range(0.0, 1.0e6),
        4 => rng.range(1.0e6, 1.0e30),
        5 => f32::MAX,
        6 => -0.0,
        _ => rng.range(-10.0, extent.clamp(1.0, 1.0e5) + 10.0),
    }
}

fn hostile(rng: &mut Rng) -> f32 {
    *rng.pick(&[0.0, -0.0, 1.0, -1.0, 1.0e-30, 1.0e30, -1.0e30, f32::MAX, f32::NAN, f32::INFINITY, f32::NEG_INFINITY])
}

fn modifiers(rng: &mut Rng) -> Modifiers {
    let bits = if rng.chance(0.6) { 0 } else { rng.below(16) };
    Modifiers { shift: bits & 1 != 0, ctrl: bits & 2 != 0, alt: bits & 4 != 0, logo: bits & 8 != 0 }
}

fn nasty_text(rng: &mut Rng, out: &mut String) {
    out.clear();
    for _ in 0..1 + rng.below(3) {
        let t = TEXTS[rng.below(TEXTS.len())];
        out.push_str(t);
    }
    if rng.chance(0.02) {
        let s = out.clone();
        for _ in 0..200 {
            out.push_str(&s);
        }
    }
}

fn frame_input(rng: &mut Rng) -> FrameInput {
    let size = match rng.below(24) {
        0 => Vec2::new(0.0, 0.0),
        1 => Vec2::new(1.0, 1.0),
        2 => Vec2::new(16384.0, 16384.0),
        3 => Vec2::new(hostile(rng), hostile(rng)),
        4 => Vec2::new(rng.range(0.0, 50.0), rng.range(0.0, 50.0)),
        5 => Vec2::new(rng.range(1.0, 4000.0), rng.range(1.0, 4000.0)),
        _ => Vec2::new(1280.0, 800.0),
    };
    let scale = match rng.below(24) {
        0 => 0.0,
        1 => f32::NAN,
        2 => rng.range(0.5, 4.0),
        3 => *rng.pick(&[0.5, 1.0, 1.25, 1.5, 3.0, 4.0]),
        4 => hostile(rng),
        _ => 2.0,
    };
    let dt = match rng.below(16) {
        0 => 0.0,
        1 => -rng.range(0.0, 10.0),
        2 => rng.range(1.0, 1.0e9),
        3 => hostile(rng),
        _ => 1.0 / 60.0,
    };
    FrameInput { size, scale, dt }
}

/// A theme with randomized metrics, text sizes and motion. Sizes stay in the
/// range a style editor offers; durations include zero and infinity.
fn random_theme(rng: &mut Rng) -> Theme {
    let mut t = if rng.chance(0.5) { Theme::dark() } else { Theme::light() };
    let m = &mut t.metrics;
    for v in [
        &mut m.radius_sm,
        &mut m.radius_md,
        &mut m.radius_lg,
        &mut m.spacing,
        &mut m.spacing_sm,
        &mut m.spacing_lg,
        &mut m.panel_padding,
        &mut m.card_padding,
        &mut m.control_height,
        &mut m.control_padding_x,
        &mut m.border_width,
        &mut m.focus_ring_width,
        &mut m.checkbox_size,
        &mut m.slider_thumb,
        &mut m.slider_track,
        &mut m.scrollbar_width,
        &mut m.label_width,
    ] {
        if rng.chance(0.3) {
            *v = rng.range(0.0, 64.0);
        }
    }
    for s in t.typography.styles_mut() {
        if rng.chance(0.3) {
            s.size = rng.range(1.0, 96.0);
            s.line_height = rng.range(0.5, 3.0);
            s.letter_spacing = rng.range(-2.0, 8.0);
        }
    }
    let mo = &mut t.motion;
    for v in [&mut mo.hover, &mut mo.press, &mut mo.value, &mut mo.caret_blink] {
        if rng.chance(0.3) {
            *v = *rng.pick(&[0.0, 1.0e-30, 0.05, 1.0, 1.0e30, f32::INFINITY]);
        }
    }
    t
}

/// Widget centers from the previous frame, used to aim pointer events.
fn targets(ui: &Ui, out: &mut Vec<Vec2>) {
    out.clear();
    for w in &ui.output().widgets {
        let p = w.rect.intersect(&w.clip).center();
        if p.x.is_finite() && p.y.is_finite() {
            out.push(p);
        }
    }
}

struct EventState {
    /// Widget centers from the previous frame.
    aims: Vec<Vec2>,
    /// Remaining frames of the current drag gesture.
    drag: u32,
    text: String,
}

fn events(rng: &mut Rng, ui: &mut Ui, size: Vec2, es: &mut EventState, log: &mut String) {
    let EventState { aims, drag, text } = es;
    if *drag > 0 {
        *drag -= 1;
        let (x, y) = (coord(rng, size.x), coord(rng, size.y));
        let _ = writeln!(log, "  PointerMoved {{ x: {x:?}, y: {y:?} }} (drag)");
        ui.push_event(InputEvent::PointerMoved { x, y });
        if rng.chance(0.5) {
            return;
        }
    }
    let n = if rng.chance(0.1) { 0 } else { 1 + rng.below(8) };
    for _ in 0..n {
        let e = match rng.below(100) {
            0..=29 => {
                let (x, y) = if !aims.is_empty() && rng.chance(0.6) {
                    let p = *rng.pick(aims);
                    (p.x + rng.range(-3.0, 3.0), p.y + rng.range(-3.0, 3.0))
                } else {
                    (coord(rng, size.x), coord(rng, size.y))
                };
                InputEvent::PointerMoved { x, y }
            }
            30..=31 => InputEvent::PointerLeft,
            32..=49 => {
                let button = *rng.pick(&[
                    PointerButton::Primary,
                    PointerButton::Primary,
                    PointerButton::Secondary,
                    PointerButton::Middle,
                ]);
                InputEvent::PointerButton { button, pressed: rng.chance(0.5) }
            }
            50..=56 => {
                let (dx, dy) = if rng.chance(0.2) {
                    (hostile(rng), hostile(rng))
                } else {
                    (rng.range(-200.0, 200.0), rng.range(-200.0, 200.0))
                };
                InputEvent::Scroll { dx, dy }
            }
            57..=81 => {
                let key = *rng.pick(&Key::ALL);
                InputEvent::Key { key, pressed: rng.chance(0.8), modifiers: modifiers(rng) }
            }
            82..=84 => InputEvent::ModifiersChanged(modifiers(rng)),
            85..=92 => {
                nasty_text(rng, text);
                InputEvent::Text(text.clone())
            }
            93..=95 => {
                nasty_text(rng, text);
                InputEvent::Paste(text.clone())
            }
            96..=97 => InputEvent::WindowFocused(rng.chance(0.5)),
            _ => {
                // Drag gesture: press on a widget now; the following frames
                // move the held pointer, possibly to hostile positions.
                let p = aims.get(rng.below(aims.len().max(1))).copied().unwrap_or(Vec2::new(10.0, 10.0));
                ui.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
                *drag = 1 + rng.below(6) as u32;
                InputEvent::PointerButton { button: PointerButton::Primary, pressed: true }
            }
        };
        if log.len() < 1 << 16 {
            let _ = writeln!(log, "  {e:?}");
        }
        ui.push_event(e);
    }
}

fn check_output(ui: &Ui) {
    let out = ui.output();
    let dl = &out.draw_list;
    let insts = dl.instances();
    assert!(insts.len() <= MAX_INSTANCES, "{} instances", insts.len());
    for (i, inst) in insts.iter().enumerate() {
        let floats = inst.rect.iter().chain(&inst.radii).chain(&inst.uv).chain(&inst.params);
        for f in floats {
            assert!(f.is_finite(), "instance {i} has a non-finite field: {inst:?}");
        }
    }
    let mut next = 0;
    for c in dl.commands() {
        assert_eq!(c.start, next, "draw commands are not contiguous");
        assert!(c.start <= c.end && c.end as usize <= insts.len(), "bad command range {c:?}");
        assert!([c.clip.min.x, c.clip.min.y, c.clip.max.x, c.clip.max.y].iter().all(|v| !v.is_nan()), "NaN clip {c:?}");
        next = c.end;
    }
    assert_eq!(next as usize, insts.len(), "instances not covered by commands");
    for w in &out.widgets {
        assert!(!w.rect.min.x.is_nan() && !w.rect.min.y.is_nan(), "NaN widget rect {w:?}");
    }
}

/// Labels only drawn on the metrics page, which plots measured CPU time.
const METRICS_LABELS: &[&str] = &["Frame build time (ms)", "Last frame"];

/// Hash of the draw list, for determinism checks. Frames that show measured
/// timings (inspector, metrics page) hash to zero, and glyph atlas
/// coordinates are skipped because those timings change which glyphs were
/// rasterized and so the atlas packing.
fn digest(ui: &Ui) -> u64 {
    let out = ui.output();
    if ui.inspector_enabled() || out.widgets.iter().any(|w| METRICS_LABELS.contains(&w.label.as_str())) {
        return 0;
    }
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    let glyph = rugi_core::draw::PrimitiveKind::Glyph as u32;
    for inst in out.draw_list.instances() {
        let uv: &[f32] = if inst.kind == glyph { &[] } else { &inst.uv };
        for f in inst.rect.iter().chain(&inst.radii).chain(uv).chain(&inst.params) {
            mix(f.to_bits() as u64);
        }
        mix(inst.color0 as u64 ^ (inst.color1 as u64) << 32);
        mix(inst.border_color as u64 ^ (inst.kind as u64) << 32);
    }
    for c in out.draw_list.commands() {
        mix(c.start as u64 ^ (c.end as u64) << 32);
    }
    h
}

/// Prints the failing frame's input when a check panics.
struct Report<'a> {
    seed: u64,
    frame: u64,
    log: &'a RefCell<String>,
}

impl Drop for Report<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("fuzz failure: seed {:#x}, frame {}\n{}", self.seed, self.frame, self.log.borrow());
        }
    }
}

struct Options {
    /// Records a digest per frame.
    deterministic: bool,
}

/// Runs `frames` fuzzed frames and returns a digest per frame.
fn run(seed: u64, frames: u64, opts: &Options) -> Vec<u64> {
    let mut rng = Rng(seed);
    let mut ui = Ui::new(UiOptions { inspection: true, ..Default::default() });
    let mut es = EventState { aims: Vec::new(), drag: 0, text: String::new() };
    let cell = RefCell::new(String::new());
    let mut digests = Vec::new();
    let mut size = Vec2::new(1280.0, 800.0);
    for frame in 0..frames {
        let mut log = cell.borrow_mut();
        log.clear();
        if rng.chance(0.03) {
            let p = *rng.pick(&PAGES);
            let _ = writeln!(log, "  set_demo_page({p:?})");
            ui.set_demo_page(p);
        }
        if rng.chance(0.01) {
            let dark = rng.chance(0.5);
            let _ = writeln!(log, "  set_theme(dark: {dark})");
            ui.set_theme(if dark { Theme::dark() } else { Theme::light() });
        }
        if rng.chance(0.005) {
            let t = random_theme(&mut rng);
            let _ = writeln!(log, "  set_theme({:?} {:?} {:?})", t.metrics, t.typography, t.motion);
            ui.set_theme(t);
        }
        if rng.chance(0.005) {
            let mut fps = || match rng.below(4) {
                0 => None,
                1 => Some(30.0),
                _ => Some(hostile(&mut rng)),
            };
            let rate = AmbientRate { focused: fps(), unfocused: fps() };
            let _ = writeln!(log, "  set_ambient_rate({rate:?})");
            ui.set_ambient_rate(rate);
        }
        if rng.chance(0.005) {
            let on = rng.chance(0.5);
            let _ = writeln!(log, "  set_inspector({on})");
            ui.set_inspector(on);
        }
        if rng.chance(0.005) {
            let on = rng.chance(0.5);
            let _ = writeln!(log, "  set_command_palette_open({on})");
            ui.set_command_palette_open(on);
        }
        let input = if rng.chance(0.15) { frame_input(&mut rng) } else { FrameInput { size, ..frame_input(&mut rng) } };
        if rng.chance(0.02) {
            size = input.size;
        }
        events(&mut rng, &mut ui, input.size, &mut es, &mut log);
        let _ = writeln!(log, "  {input:?}");
        drop(log);
        let _report = Report { seed, frame, log: &cell };
        let valid =
            input.scale.is_finite() && input.scale > 0.0 && input.size.x.is_finite() && input.size.y.is_finite();
        match ui.begin_frame(input) {
            Ok(()) => assert!(valid, "begin_frame accepted {input:?}"),
            Err(_) => {
                assert!(!valid, "begin_frame rejected {input:?}");
                continue;
            }
        }
        ui.show_demo();
        ui.end_frame();
        assert_eq!(ui.take_error(), None, "demo reported API misuse");
        check_output(&ui);
        if opts.deterministic {
            digests.push(digest(&ui));
        }
        if frame % 256 == 0 {
            let live = LIVE.load(Ordering::Relaxed);
            assert!(live < MAX_LIVE_BYTES, "{live} live heap bytes");
        }
        targets(&ui, &mut es.aims);
    }
    eprintln!("seed {seed:#x}: {frames} frames, {} live heap bytes", LIVE.load(Ordering::Relaxed));
    digests
}

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| {
            let v = v.trim();
            v.strip_prefix("0x").map_or_else(|| v.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
        })
        .unwrap_or(default)
}

#[test]
fn fuzz_demo() {
    let frames = env("RUGI_FUZZ_FRAMES", DEFAULT_FRAMES);
    let seed = env("RUGI_FUZZ_SEED", DEFAULT_SEED);
    let seeds = env("RUGI_FUZZ_SEEDS", 1);
    for s in seed..seed + seeds {
        run(s, frames, &Options { deterministic: false });
    }
}

#[test]
fn fuzz_is_deterministic() {
    let opts = Options { deterministic: true };
    let a = run(DEFAULT_SEED ^ 1, 300, &opts);
    let b = run(DEFAULT_SEED ^ 1, 300, &opts);
    let first = a.iter().zip(&b).position(|(x, y)| x != y);
    assert_eq!(first, None, "same seed produced different frames");
}

/// Widget calls with hostile numeric arguments, as a C host could make them.
fn hostile_widgets(rng: &mut Rng, ui: &mut Ui, st: &mut ApiState, log: &RefCell<String>) {
    let mut h = || hostile(rng);
    let vals = [h(), h(), h(), h()];
    for _ in 0..12 {
        let call = rng.below(22);
        // The arguments of each call are the next few `hostile` draws.
        let mut peek = Rng(rng.0);
        let _ = writeln!(
            log.borrow_mut(),
            "  call {call} with {:?}",
            [hostile(&mut peek), hostile(&mut peek), hostile(&mut peek)]
        );
        match call {
            0 => {
                let (lo, hi) = (hostile(rng), hostile(rng));
                ui.slider("s", &mut st.f, lo, hi);
            }
            1 => {
                let (lo, hi, step) = (hostile(rng), hostile(rng), hostile(rng));
                ui.spin_box("sb", &mut st.f, lo, hi, step);
            }
            2 => ui.space(hostile(rng)),
            3 => {
                ui.set_next_item_width(hostile(rng));
                ui.button("w");
            }
            4 => {
                ui.allocate(Vec2::new(hostile(rng), hostile(rng)), rng.chance(0.5));
            }
            5 => {
                ui.progress_bar(hostile(rng));
            }
            6 => {
                let pair = (hostile(rng), hostile(rng));
                let r = rng.chance(0.5).then_some(pair);
                ui.plot_lines("pl", &vals, r, hostile(rng));
            }
            7 => {
                let pair = (hostile(rng), hostile(rng));
                let r = rng.chance(0.5).then_some(pair);
                ui.plot_histogram("ph", &vals, r, hostile(rng));
            }
            8 => ui.columns(rng.below(4) as u32, |ui| {
                ui.button("c");
            }),
            9 => {
                let h = hostile(rng);
                ui.scroll_area("sa", Some(h), |ui| {
                    ui.button("in");
                });
            }
            10 => {
                ui.label_truncated("truncated label text", rugi_ui::TextRole::Body, Some(hostile(rng)));
            }
            11 => {
                let n = rng.below(4);
                ui.combo_box("cb", &mut st.index, &["a", "b", "c"][..n]);
            }
            12 => {
                let n = rng.below(4);
                ui.tabs("tb", &mut st.index, &["a", "b", "c"][..n]);
            }
            13 => {
                let n = rng.below(4);
                ui.segmented("sg", &mut st.index, &["a", "b", "c"][..n]);
            }
            14 => {
                let (count, h) = (rng.below(4), hostile(rng));
                ui.page_dots("pd", count, &mut st.index);
                ui.list_box("lb", &mut st.index, &["a", "b"][..count.min(2)], h);
            }
            15 => {
                let size = Vec2::new(hostile(rng), hostile(rng));
                ui.carousel("cr", rng.below(4), &mut st.index, size, |_, _, _, _| {});
            }
            16 => {
                let (rows, h) = (rng.below(50), hostile(rng));
                let widths = [hostile(rng), 80.0];
                ui.table("t", &["a", "b"], &widths, rows, &mut st.row, h, |ui, _, _| {
                    ui.label("x");
                });
            }
            17 => {
                let r = rugi_ui::Rect::from_xywh(hostile(rng), hostile(rng), hostile(rng), hostile(rng));
                ui.paint_scrim(r, hostile(rng), hostile(rng));
                ui.paint_focus_ring(r, hostile(rng), hostile(rng));
            }
            18 => {
                let id = ui.make_id("anim");
                let (target, tc) = (hostile(rng), hostile(rng));
                ui.animate_value(id, target, tc);
                ui.spring_value(id.with("s"), target, tc);
            }
            19 => {
                ui.icon_button("ib", rugi_ui::lucide::STAR.into(), hostile(rng), "tip");
            }
            20 => {
                ui.begin_vertical_width(hostile(rng));
                ui.button("v");
                ui.end();
            }
            _ => {
                let w = hostile(rng);
                ui.modal("m", "Modal", &mut st.modal, w, |ui| ui.button("ok"));
                st.modal = rng.chance(0.5);
            }
        }
    }
}

#[derive(Default)]
struct ApiState {
    f: f32,
    index: usize,
    row: Option<usize>,
    modal: bool,
}

#[test]
fn fuzz_widget_arguments() {
    let frames = env("RUGI_FUZZ_FRAMES", DEFAULT_FRAMES);
    let seed = env("RUGI_FUZZ_SEED", DEFAULT_SEED);
    let seeds = env("RUGI_FUZZ_SEEDS", 1);
    for s in seed..seed + seeds {
        run_widgets(s, frames);
    }
}

fn run_widgets(seed: u64, frames: u64) {
    let mut rng = Rng(seed ^ 0xA11);
    let mut ui = Ui::new(UiOptions { inspection: true, ..Default::default() });
    let mut st = ApiState::default();
    let mut es = EventState { aims: Vec::new(), drag: 0, text: String::new() };
    let log = RefCell::new(String::new());
    for frame in 0..frames {
        log.borrow_mut().clear();
        let size = Vec2::new(800.0, 600.0);
        events(&mut rng, &mut ui, size, &mut es, &mut log.borrow_mut());
        let _report = Report { seed, frame, log: &log };
        ui.begin_frame(FrameInput { size, scale: 1.0, dt: 1.0 / 60.0 }).unwrap();
        let mut sub = Rng(rng.next());
        ui.central_panel(|ui| hostile_widgets(&mut sub, ui, &mut st, &log));
        ui.end_frame();
        let _ = ui.take_error();
        check_output(&ui);
        targets(&ui, &mut es.aims);
    }
}
