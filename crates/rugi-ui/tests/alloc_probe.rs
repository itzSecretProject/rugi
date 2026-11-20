//! Steady-state allocation check (debug builds count the same allocations).
use rugi_core::Vec2;
use rugi_ui::{FrameInput, TextRole, Ui, UiOptions};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static TRACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: caller's contract.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: caller's contract.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: caller's contract.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The counter is global, so the probes run one at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn steady_state_frames_do_not_allocate() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::new(UiOptions::default());
    let input = FrameInput { size: Vec2::new(1280.0, 800.0), scale: 2.0, dt: 1.0 / 60.0 };
    let mut checks = vec![false; 500];
    let mut values = vec![50.0f32; 500];
    let mut run = |ui: &mut Ui| {
        ui.begin_frame(input).unwrap();
        ui.central_panel(|ui| {
            ui.scroll_area("list", None, |ui| {
                for i in 0..500 {
                    ui.push_id_u64(i as u64);
                    ui.horizontal(|ui| {
                        ui.label("Static row label");
                        ui.button("Action");
                        ui.checkbox("On", &mut checks[i]);
                        ui.set_next_item_width(200.0);
                        ui.slider("Value", &mut values[i], 0.0, 100.0);
                    });
                    ui.pop_id();
                }
            });
        });
        ui.end_frame();
    };
    for _ in 0..10 {
        run(&mut ui);
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    for _ in 0..10 {
        run(&mut ui);
    }
    TRACK.store(false, Ordering::Relaxed);
    let per_frame = (ALLOCS.load(Ordering::Relaxed) - before) as f64 / 10.0;
    assert_eq!(per_frame, 0.0, "steady-state frames allocated {per_frame} times per frame");
}

#[test]
fn centered_rows_integer_widgets_and_plots_do_not_allocate() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut ui = Ui::new(UiOptions::default());
    let input = FrameInput { size: Vec2::new(1280.0, 800.0), scale: 2.0, dt: 1.0 / 60.0 };
    let mut ints = [3i32; 50];
    let mut text = String::from("value");
    let samples: Vec<f32> = (0..64).map(|i| (i as f32 * 0.3).sin()).collect();
    let mut run = |ui: &mut Ui| {
        ui.begin_frame(input).unwrap();
        ui.central_panel(|ui| {
            ui.search_button("Search", "Ctrl K");
            ui.plot_lines("lines", &samples, None, 80.0);
            ui.plot_histogram("bars", &samples[..12], Some((-1.0, 1.0)), 80.0);
            ui.scroll_area("rows", None, |ui| {
                for (i, v) in ints.iter_mut().enumerate() {
                    ui.push_id_u64(i as u64);
                    ui.horizontal_centered(|ui| {
                        ui.label("A label next to an input");
                        ui.set_next_item_width(120.0);
                        ui.text_input("##t", &mut text, "");
                        ui.spin_box_int("##n", v, 0, 10, 1);
                        ui.vertical(|ui| ui.caption("Group"));
                        ui.label_truncated(
                            "a/long/path/that/is/shortened/to/fit/the/width.rs",
                            TextRole::Muted,
                            Some(90.0),
                        );
                    });
                    ui.slider_int("Count", v, 0, 10);
                    ui.pop_id();
                }
            });
        });
        ui.end_frame();
    };
    for _ in 0..10 {
        run(&mut ui);
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    for _ in 0..10 {
        run(&mut ui);
    }
    TRACK.store(false, Ordering::Relaxed);
    let per_frame = (ALLOCS.load(Ordering::Relaxed) - before) as f64 / 10.0;
    assert_eq!(per_frame, 0.0, "steady-state frames allocated {per_frame} times per frame");
}
