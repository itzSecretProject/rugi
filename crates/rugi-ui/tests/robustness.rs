//! Regression tests for hostile host input found by the `fuzz` test.

use rugi_core::{InputEvent, PointerButton};
use rugi_ui::{AmbientRate, FrameInput, Repaint, Theme, Ui, UiError, UiOptions, Vec2};

const INPUT: FrameInput = FrameInput { size: Vec2 { x: 800.0, y: 600.0 }, scale: 1.0, dt: 1.0 / 60.0 };

fn ui() -> Ui {
    Ui::new(UiOptions { inspection: true, ..Default::default() })
}

fn assert_finite(ui: &Ui) {
    for inst in ui.output().draw_list.instances() {
        let floats = inst.rect.iter().chain(&inst.radii).chain(&inst.uv).chain(&inst.params);
        assert!(floats.into_iter().all(|f| f.is_finite()), "non-finite instance {inst:?}");
    }
}

#[test]
fn tiny_ambient_rate_does_not_overflow_repaint_delay() {
    let mut ui = ui();
    ui.set_ambient_rate(AmbientRate { focused: Some(1.0e-30), unfocused: Some(1.0e-30) });
    ui.begin_frame(INPUT).unwrap();
    ui.request_ambient_repaint();
    let out = ui.end_frame();
    assert!(matches!(out.repaint(), Repaint::After(_)));
}

#[test]
fn infinite_caret_blink_does_not_overflow_repaint_delay() {
    let mut ui = ui();
    let mut t = Theme::dark();
    t.motion.caret_blink = f32::INFINITY;
    ui.set_theme(t);
    let mut s = String::from("text");
    let mut frame = |ui: &mut Ui| {
        ui.begin_frame(INPUT).unwrap();
        let r = ui.central_panel(|ui| ui.text_input("Name", &mut s, ""));
        ui.end_frame();
        assert_finite(ui);
        r
    };
    let r = frame(&mut ui);
    ui.push_event(InputEvent::PointerMoved { x: r.rect.center().x, y: r.rect.center().y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
    assert!(frame(&mut ui).focused);
    frame(&mut ui);
}

#[test]
fn degenerate_motion_keeps_animations_finite() {
    for v in [0.0, -1.0, 1.0e-30, 1.0e30, f32::INFINITY, f32::NAN] {
        let mut ui = ui();
        let mut t = Theme::dark();
        t.motion.hover = v;
        t.motion.press = v;
        t.motion.value = v;
        ui.set_theme(t);
        let mut on = false;
        for i in 0..4 {
            ui.push_event(InputEvent::PointerMoved { x: 30.0 + i as f32, y: 20.0 });
            ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: i % 2 == 0 });
            ui.begin_frame(INPUT).unwrap();
            ui.central_panel(|ui| {
                ui.button("Button");
                ui.checkbox("Check", &mut on);
                ui.toggle("Toggle", &mut on);
            });
            ui.end_frame();
            assert_finite(&ui);
        }
    }
}

fn scroll_frame(ui: &mut Ui) {
    ui.begin_frame(INPUT).unwrap();
    ui.central_panel(|ui| {
        ui.scroll_area("list", None, |ui| {
            for i in 0..100 {
                ui.push_id_u64(i);
                ui.button("Row");
                ui.pop_id();
            }
        });
    });
    ui.end_frame();
    assert_finite(ui);
}

#[test]
fn non_finite_scroll_is_ignored() {
    let mut ui = ui();
    scroll_frame(&mut ui);
    ui.push_event(InputEvent::PointerMoved { x: 100.0, y: 100.0 });
    for (dx, dy) in [(f32::NAN, f32::NAN), (-1.0e30, f32::NAN), (f32::INFINITY, f32::NEG_INFINITY)] {
        ui.push_event(InputEvent::Scroll { dx, dy });
        scroll_frame(&mut ui);
    }
    // Finite deltas that overflow when summed saturate instead.
    ui.push_event(InputEvent::Scroll { dx: 0.0, dy: -f32::MAX });
    ui.push_event(InputEvent::Scroll { dx: 0.0, dy: -f32::MAX });
    scroll_frame(&mut ui);
    ui.push_event(InputEvent::Scroll { dx: 0.0, dy: f32::MAX });
    scroll_frame(&mut ui);
}

#[test]
fn non_finite_pointer_is_ignored_or_clamped() {
    let mut ui = ui();
    let mut v = 0.5f32;
    let frame = |ui: &mut Ui, v: &mut f32| {
        ui.begin_frame(INPUT).unwrap();
        let r = ui.central_panel(|ui| ui.slider("Value", v, 0.0, 1.0));
        ui.end_frame();
        assert_finite(ui);
        r
    };
    let r = frame(&mut ui, &mut v);
    ui.push_event(InputEvent::PointerMoved { x: r.rect.center().x, y: r.rect.center().y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    frame(&mut ui, &mut v);
    for (x, y) in [(f32::NAN, 0.0), (f32::INFINITY, r.rect.center().y), (f32::NEG_INFINITY, f32::NAN), (f32::MAX, 0.0)]
    {
        ui.push_event(InputEvent::PointerMoved { x, y });
        frame(&mut ui, &mut v);
        assert!(v.is_finite(), "pointer ({x}, {y}) made the slider value {v}");
    }
    // Dragging to an infinitely distant point saturates the slider.
    ui.push_event(InputEvent::PointerMoved { x: f32::INFINITY, y: r.rect.center().y });
    frame(&mut ui, &mut v);
    assert_eq!(v, 1.0);
}

#[test]
fn non_finite_window_size_is_rejected() {
    let mut ui = ui();
    for size in [Vec2::new(f32::INFINITY, 600.0), Vec2::new(800.0, f32::NAN), Vec2::new(f32::NEG_INFINITY, 0.0)] {
        let r = ui.begin_frame(FrameInput { size, ..INPUT });
        assert_eq!(r, Err(UiError::InvalidArgument("size must be finite")));
    }
    // Zero, negative and huge finite sizes render (the background at most).
    for size in [Vec2::ZERO, Vec2::new(-5.0, -5.0), Vec2::new(1.0, 1.0), Vec2::new(1.0e30, f32::MAX)] {
        ui.begin_frame(FrameInput { size, ..INPUT }).unwrap();
        ui.show_demo();
        ui.end_frame();
        assert_eq!(ui.take_error(), None);
        assert_finite(&ui);
    }
}

#[test]
fn extreme_scale_is_clamped() {
    let mut ui = ui();
    for scale in [1.0e-30, f32::MIN_POSITIVE, 1.0e30, f32::MAX] {
        ui.begin_frame(FrameInput { scale, ..INPUT }).unwrap();
        ui.show_demo();
        ui.end_frame();
        assert_finite(&ui);
        assert!((1.0 / 16.0..=16.0).contains(&ui.scale()));
    }
}

#[test]
fn short_scroll_area_with_overflowing_content() {
    for h in [0.0, 10.0, 20.0, 31.0] {
        let mut ui = ui();
        ui.begin_frame(INPUT).unwrap();
        ui.central_panel(|ui| {
            ui.scroll_area("short", Some(h), |ui| {
                for i in 0..10 {
                    ui.push_id_u64(i);
                    ui.button("Row");
                    ui.pop_id();
                }
            });
        });
        ui.end_frame();
        assert_finite(&ui);
    }
}

fn panel_frame(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    ui.begin_frame(INPUT).unwrap();
    ui.central_panel(f);
    ui.end_frame();
    assert_finite(ui);
    for w in &ui.output().widgets {
        let r = w.rect;
        assert!([r.min.x, r.min.y, r.max.x, r.max.y].iter().all(|v| !v.is_nan()), "NaN layout {w:?}");
    }
}

#[test]
fn value_widgets_accept_nan_and_infinite_bounds() {
    let mut ui = ui();
    let bounds =
        [(f32::NAN, 1.0), (0.0, f32::NAN), (f32::NAN, f32::NAN), (f32::NEG_INFINITY, f32::INFINITY), (5.0, -5.0)];
    for (lo, hi) in bounds {
        let (mut a, mut b) = (0.5f32, 0.5f32);
        for _ in 0..2 {
            panel_frame(&mut ui, |ui| {
                ui.slider("s", &mut a, lo, hi);
                ui.spin_box("b", &mut b, lo, hi, f32::NAN);
            });
        }
        assert!(a.is_finite() && b.is_finite(), "bounds ({lo}, {hi}) gave {a}, {b}");
    }
}

#[test]
fn plots_skip_non_finite_samples_and_ranges() {
    let mut ui = ui();
    let values = [1.0, f32::NAN, f32::INFINITY, -f32::INFINITY, 3.0];
    for range in [None, Some((f32::NAN, 1.0)), Some((1.0, 1.0)), Some((0.0, f32::INFINITY))] {
        panel_frame(&mut ui, |ui| {
            ui.plot_lines("l", &values, range, 80.0);
            ui.plot_histogram("h", &values, range, 80.0);
        });
    }
}

#[test]
fn huge_or_non_finite_container_sizes() {
    let mut ui = ui();
    let mut sel = None;
    let mut i = 0;
    let mut open = true;
    for v in [f32::NAN, f32::INFINITY, f32::MAX, -1.0e30] {
        panel_frame(&mut ui, |ui| {
            ui.space(v);
            ui.set_next_item_width(v);
            ui.button("w");
            ui.allocate(rugi_ui::Vec2::new(v, v), false);
            ui.scroll_area("a", Some(v), |ui| {
                ui.button("in");
            });
            ui.list_box("lb", &mut i, &["a", "b"], v);
            ui.table("t", &["a", "b"], &[v, 1.0], 10_000, &mut sel, v, |ui, _, _| {
                ui.label("x");
            });
            ui.begin_vertical_width(v);
            ui.button("v");
            ui.end();
            ui.modal("m", "Modal", &mut open, v, |ui| ui.button("ok"));
        });
        assert_eq!(ui.take_error(), None);
    }
}

#[test]
fn text_painted_at_non_finite_positions_is_dropped() {
    let mut ui = ui();
    for p in [
        Vec2::new(f32::NAN, 0.0),
        Vec2::new(f32::INFINITY, 10.0),
        Vec2::new(10.0, f32::NEG_INFINITY),
        Vec2::new(f32::MAX, f32::MAX),
    ] {
        ui.begin_frame(INPUT).unwrap();
        let l = ui.layout_text("Hello", &Theme::dark().typography.body, None);
        ui.paint_text(&l, p, rugi_ui::Color::WHITE);
        ui.end_frame();
        assert_finite(&ui);
    }
}

#[test]
fn page_dots_with_a_huge_count_lays_out_only_visible_dots() {
    let mut ui = ui();
    let mut index = usize::MAX / 2;
    let start = std::time::Instant::now();
    ui.begin_frame(INPUT).unwrap();
    ui.central_panel(|ui| ui.page_dots("dots", usize::MAX, &mut index));
    ui.end_frame();
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    assert!(ui.output().draw_list.instances().len() < 200);
}
