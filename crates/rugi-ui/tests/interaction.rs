//! Headless interaction tests: synthetic input in, widget results out.

use rugi_core::{InputEvent, Key, Modifiers, PointerButton};
use rugi_ui::{FrameInput, Rect, Repaint, Ui, UiError, UiOptions, Vec2};

fn ui() -> Ui {
    Ui::new(UiOptions { inspection: true, ..Default::default() })
}

const INPUT: FrameInput = FrameInput { size: Vec2 { x: 800.0, y: 600.0 }, scale: 1.0, dt: 1.0 / 60.0 };

/// Runs one frame and returns whatever `f` returns.
fn frame<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.begin_frame(INPUT).unwrap();
    let r = ui.central_panel(f);
    ui.end_frame();
    r
}

fn click_at(ui: &mut Ui, p: Vec2) {
    ui.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
}

fn key(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.push_event(InputEvent::Key { key, pressed: true, modifiers });
}

#[test]
fn button_click_within_one_frame() {
    let mut ui = ui();
    let rect = frame(&mut ui, |ui| ui.button("Save").rect);
    click_at(&mut ui, rect.center());
    let clicked = frame(&mut ui, |ui| ui.button("Save").clicked);
    assert!(clicked, "press+release inside one frame must click");
    // Next frame: no click.
    assert!(!frame(&mut ui, |ui| ui.button("Save").clicked));
}

#[test]
fn click_requires_release_inside() {
    let mut ui = ui();
    let rect = frame(&mut ui, |ui| ui.button("Go").rect);
    ui.push_event(InputEvent::PointerMoved { x: rect.center().x, y: rect.center().y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    assert!(frame(&mut ui, |ui| ui.button("Go").pressed));
    ui.push_event(InputEvent::PointerMoved { x: 790.0, y: 590.0 });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
    assert!(!frame(&mut ui, |ui| ui.button("Go").clicked));
}

#[test]
fn checkbox_toggles() {
    let mut ui = ui();
    let mut v = false;
    let rect = frame(&mut ui, |ui| ui.checkbox("Enable", &mut v).rect);
    click_at(&mut ui, rect.center());
    let changed = frame(&mut ui, |ui| ui.checkbox("Enable", &mut v).changed);
    assert!(changed && v);
}

#[test]
fn slider_drag_and_keyboard() {
    let mut ui = ui();
    let mut v = 0.0f32;
    let rect = frame(&mut ui, |ui| ui.slider("Volume", &mut v, 0.0, 100.0).rect);
    // Drag to the far right.
    ui.push_event(InputEvent::PointerMoved { x: rect.min.x + 20.0, y: rect.center().y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    frame(&mut ui, |ui| ui.slider("Volume", &mut v, 0.0, 100.0));
    ui.push_event(InputEvent::PointerMoved { x: rect.max.x + 50.0, y: rect.center().y });
    let r = frame(&mut ui, |ui| ui.slider("Volume", &mut v, 0.0, 100.0));
    assert!(r.changed);
    assert_eq!(v, 100.0);
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
    frame(&mut ui, |ui| ui.slider("Volume", &mut v, 0.0, 100.0));
    // Slider kept focus from the click: arrow keys step by 1%.
    key(&mut ui, Key::ArrowLeft, Modifiers::default());
    key(&mut ui, Key::ArrowLeft, Modifiers::default());
    frame(&mut ui, |ui| ui.slider("Volume", &mut v, 0.0, 100.0));
    assert!((v - 98.0).abs() < 1e-3, "v = {v}");
    key(&mut ui, Key::Home, Modifiers::default());
    frame(&mut ui, |ui| ui.slider("Volume", &mut v, 0.0, 100.0));
    assert_eq!(v, 0.0);
}

#[test]
fn text_input_editing() {
    let mut ui = ui();
    let mut s = String::from("Rugi");
    let rect = frame(&mut ui, |ui| ui.text_input("name", &mut s, "").rect);
    // Click far right of the text -> caret at end.
    click_at(&mut ui, Vec2::new(rect.max.x - 5.0, rect.center().y));
    frame(&mut ui, |ui| ui.text_input("name", &mut s, ""));
    ui.push_event(InputEvent::Text(" UI".into()));
    let r = frame(&mut ui, |ui| ui.text_input("name", &mut s, ""));
    assert!(r.changed && r.focused);
    assert_eq!(s, "Rugi UI");
    key(&mut ui, Key::Backspace, Modifiers::default());
    frame(&mut ui, |ui| ui.text_input("name", &mut s, ""));
    assert_eq!(s, "Rugi U");
    // Select all + type replaces.
    let cmd = if cfg!(target_os = "macos") {
        Modifiers { logo: true, ..Default::default() }
    } else {
        Modifiers { ctrl: true, ..Default::default() }
    };
    key(&mut ui, Key::A, cmd);
    frame(&mut ui, |ui| ui.text_input("name", &mut s, ""));
    ui.push_event(InputEvent::Text("ñandú".into()));
    frame(&mut ui, |ui| ui.text_input("name", &mut s, ""));
    assert_eq!(s, "ñandú");
    // Shift+Left selects one char, Backspace deletes it (UTF-8 safe).
    key(&mut ui, Key::ArrowLeft, Modifiers { shift: true, ..Default::default() });
    key(&mut ui, Key::Backspace, Modifiers::default());
    frame(&mut ui, |ui| ui.text_input("name", &mut s, ""));
    assert_eq!(s, "ñand");
    key(&mut ui, Key::Enter, Modifiers::default());
    assert!(frame(&mut ui, |ui| ui.text_input("name", &mut s, "")).submitted);
}

#[test]
fn unfocused_input_ignores_typing() {
    let mut ui = ui();
    let mut s = String::new();
    ui.push_event(InputEvent::Text("x".into()));
    frame(&mut ui, |ui| ui.text_input("t", &mut s, "hint"));
    assert!(s.is_empty());
}

#[test]
fn tab_cycles_focus_and_enter_activates() {
    let mut ui = ui();
    let build = |ui: &mut Ui| (ui.button("A").clicked, ui.button("B").clicked);
    frame(&mut ui, build);
    key(&mut ui, Key::Tab, Modifiers::default());
    frame(&mut ui, build);
    let a = ui.output().widgets.iter().find(|w| w.label == "A").unwrap().id;
    assert_eq!(ui.focused_id(), Some(a));
    key(&mut ui, Key::Tab, Modifiers::default());
    frame(&mut ui, build);
    key(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(frame(&mut ui, build), (false, true));
    // Shift+Tab goes back.
    key(&mut ui, Key::Tab, Modifiers { shift: true, ..Default::default() });
    frame(&mut ui, build);
    assert_eq!(ui.focused_id(), Some(a));
    // Escape clears focus.
    key(&mut ui, Key::Escape, Modifiers::default());
    frame(&mut ui, build);
    assert_eq!(ui.focused_id(), None);
}

#[test]
fn scroll_area_scrolls_and_clamps() {
    let mut ui = ui();
    let content = |ui: &mut Ui| {
        ui.scroll_area("list", Some(100.0), |ui| {
            for i in 0..50 {
                ui.label(&format!("Row {i}"));
            }
        });
        ui.output().draw_list.instances().len()
    };
    frame(&mut ui, content);
    let area = ui.last_item_rect();
    let first_row_y = |ui: &Ui| ui.output().widgets.iter().find(|w| w.label == "Row 0").unwrap().rect.min.y;
    let y0 = first_row_y(&ui);
    ui.push_event(InputEvent::PointerMoved { x: area.center().x, y: area.center().y });
    ui.push_event(InputEvent::Scroll { dx: 0.0, dy: -40.0 });
    frame(&mut ui, content);
    assert_eq!(first_row_y(&ui), y0 - 40.0);
    // Huge scroll clamps to content end.
    ui.push_event(InputEvent::Scroll { dx: 0.0, dy: -100000.0 });
    frame(&mut ui, content);
    let y_end = first_row_y(&ui);
    assert!(y_end < y0 - 500.0 && y_end > y0 - 2000.0);
    // Rows far outside the viewport are culled on the CPU.
    assert!(ui.output().stats.culled > 0);
}

#[test]
fn nested_scroll_routes_wheel_to_innermost() {
    let mut ui = ui();
    let build = |ui: &mut Ui| {
        ui.scroll_area("outer", Some(300.0), |ui| {
            ui.label("Outer 0");
            ui.scroll_area("inner", Some(100.0), |ui| {
                for i in 0..40 {
                    ui.label(&format!("Inner {i}"));
                }
            });
            for i in 1..40 {
                ui.label(&format!("Outer {i}"));
            }
        });
    };
    let y = |ui: &Ui, l: &str| ui.output().widgets.iter().find(|w| w.label == l).unwrap().rect.min.y;
    frame(&mut ui, build);
    let (o0, i0) = (y(&ui, "Outer 0"), y(&ui, "Inner 0"));
    ui.push_event(InputEvent::PointerMoved { x: 100.0, y: i0 + 20.0 });
    frame(&mut ui, build);
    ui.push_event(InputEvent::Scroll { dx: 0.0, dy: -30.0 });
    frame(&mut ui, build);
    assert_eq!(y(&ui, "Outer 0"), o0, "outer must not move");
    assert_eq!(y(&ui, "Inner 0"), i0 - 30.0, "inner scrolls");
}

#[test]
fn ids_isolate_state_per_scope() {
    let mut ui = ui();
    let mut a = false;
    let mut b = false;
    let build = |ui: &mut Ui, a: &mut bool, b: &mut bool| {
        ui.scope("left", |ui| ui.checkbox("Same", a).rect);
        ui.scope("right", |ui| ui.checkbox("Same", b).rect)
    };
    let rb = frame(&mut ui, |ui| build(ui, &mut a, &mut b));
    click_at(&mut ui, rb.center());
    frame(&mut ui, |ui| build(ui, &mut a, &mut b));
    assert!(!a && b);
}

#[test]
fn becomes_idle_after_animations_settle() {
    let mut ui = ui();
    let rect = frame(&mut ui, |ui| ui.button("Hover").rect);
    ui.push_event(InputEvent::PointerMoved { x: rect.center().x, y: rect.center().y });
    frame(&mut ui, |ui| ui.button("Hover"));
    assert_eq!(ui.output().repaint(), Repaint::Immediately);
    for _ in 0..120 {
        frame(&mut ui, |ui| ui.button("Hover"));
    }
    assert_eq!(ui.output().repaint(), Repaint::Idle);
}

#[test]
fn misuse_is_reported_not_panicking() {
    let mut ui = ui();
    // Widget outside a frame.
    assert!(!ui.button("x").clicked);
    assert_eq!(ui.take_error(), Some(UiError::NoFrame));
    // Unbalanced containers are auto-closed.
    ui.begin_frame(INPUT).unwrap();
    ui.begin_card();
    ui.begin_horizontal(rugi_ui::Align::Start);
    ui.end_frame();
    assert_eq!(ui.take_error(), Some(UiError::UnclosedContainers(2)));
    // Double begin.
    ui.begin_frame(INPUT).unwrap();
    assert_eq!(ui.begin_frame(INPUT), Err(UiError::FrameInProgress));
    ui.end();
    assert_eq!(ui.take_error(), Some(UiError::NothingToEnd));
    ui.end_frame();
}

#[test]
fn vertical_group_in_row_shrinks_to_content() {
    let mut ui = ui();
    let (group, after) = frame(&mut ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("Short");
            });
            let g = ui.last_item_rect();
            let b = ui.button("Next").rect;
            (g, b)
        })
    });
    assert!(group.width() < 100.0, "group width {}", group.width());
    assert!(after.min.x < group.max.x + 20.0, "next item follows the group");
}

#[test]
fn layout_is_dpi_independent() {
    let mut a = ui();
    let mut b = ui();
    let ra = frame(&mut a, |ui| ui.button("Same size").rect);
    b.begin_frame(FrameInput { scale: 2.0, ..INPUT }).unwrap();
    let rb = b.central_panel(|ui| ui.button("Same size").rect);
    b.end_frame();
    assert_eq!(ra, rb);
    let _ = Rect::ZERO;
}
