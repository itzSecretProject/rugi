//! Centered rows, integer value widgets, the search button and text that is
//! shortened to fit.

use rugi_core::{InputEvent, Key, Modifiers, PointerButton};
use rugi_ui::{FrameInput, Rect, Repaint, TextRole, Ui, UiOptions, Vec2};

const INPUT: FrameInput = FrameInput { size: Vec2 { x: 800.0, y: 600.0 }, scale: 1.0, dt: 1.0 / 60.0 };

fn ui() -> Ui {
    Ui::new(UiOptions { inspection: true, ..Default::default() })
}

/// Runs one frame and returns what `f` returns and the repaint request.
fn frame<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> (R, Repaint) {
    ui.begin_frame(INPUT).unwrap();
    let r = ui.central_panel(f);
    let repaint = ui.end_frame().repaint.unwrap_or(Repaint::Idle);
    assert_eq!(ui.take_error(), None);
    (r, repaint)
}

fn press(ui: &mut Ui, key: Key) {
    ui.push_event(InputEvent::Key { key, pressed: true, modifiers: Modifiers::default() });
    ui.push_event(InputEvent::Key { key, pressed: false, modifiers: Modifiers::default() });
}

fn click(ui: &mut Ui, p: Vec2) {
    ui.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
}

fn center_y(r: Rect) -> f32 {
    (r.min.y + r.max.y) * 0.5
}

#[test]
fn centered_row_aligns_a_label_with_an_input_from_the_first_frame() {
    let mut ui = ui();
    let mut text = String::from("160 px");
    let ((label, input), repaint) =
        frame(&mut ui, |ui| ui.horizontal_centered(|ui| (ui.label("Width"), ui.text_input("##w", &mut text, "").rect)));
    assert!((center_y(label) - center_y(input)).abs() <= 1.0, "label {label:?} input {input:?}");
    assert!(label.height() < input.height());
    assert_eq!(repaint, Repaint::Idle, "a row of control height must not need a second frame");
}

#[test]
fn tall_row_settles_after_one_frame_and_then_stays_idle() {
    let mut ui = ui();
    let row = |ui: &mut Ui| {
        ui.horizontal_centered(|ui| {
            let tall = ui.allocate(Vec2::new(40.0, 80.0), false);
            let button = ui.button("Next").rect;
            let group = ui.vertical(|ui| {
                ui.label("Title");
                ui.caption("Subtitle");
                ui.last_item_rect()
            });
            (tall, button, group)
        })
    };
    let (_, first) = frame(&mut ui, row);
    assert_eq!(first, Repaint::Immediately, "a row taller than its guess asks for another frame");
    let ((tall, button, caption), second) = frame(&mut ui, row);
    assert!((center_y(tall) - center_y(button)).abs() <= 1.0, "button not centered: {button:?} in {tall:?}");
    assert!(caption.max.y < tall.max.y && caption.min.y > tall.min.y + 10.0, "group not centered: {caption:?}");
    assert_ne!(second, Repaint::Immediately, "a settled row must not keep requesting frames");
    let (_, third) = frame(&mut ui, row);
    assert_ne!(third, Repaint::Immediately);
}

#[test]
fn right_aligned_centered_row_keeps_cards_with_their_content() {
    let mut ui = ui();
    let row = |ui: &mut Ui| {
        ui.begin_horizontal_centered(rugi_ui::Align::End);
        let button = ui.button("Close").rect;
        ui.card(|ui| {
            ui.label("A");
            ui.label("B");
        });
        let card = ui.last_item_rect();
        ui.end();
        (button, card)
    };
    frame(&mut ui, row);
    let ((button, card), _) = frame(&mut ui, row);
    assert!((center_y(button) - center_y(card)).abs() <= 1.0, "{button:?} vs {card:?}");
    assert!(card.max.x <= button.min.x);
}

#[test]
fn slider_int_steps_by_whole_values() {
    let mut ui = ui();
    let mut v = 5;
    let run = |ui: &mut Ui, v: &mut i32| frame(ui, |ui| ui.slider_int("Count", v, 0, 10)).0;
    let rect = run(&mut ui, &mut v).rect;
    // Clicking the track focuses it and moves the value to the nearest integer.
    click(&mut ui, Vec2::new(rect.max.x - 1.0, center_y(rect)));
    run(&mut ui, &mut v);
    assert_eq!(v, 10);
    press(&mut ui, Key::ArrowLeft);
    press(&mut ui, Key::ArrowLeft);
    assert!(run(&mut ui, &mut v).changed);
    assert_eq!(v, 8);
    press(&mut ui, Key::Home);
    run(&mut ui, &mut v);
    assert_eq!(v, 0);
    press(&mut ui, Key::ArrowDown);
    run(&mut ui, &mut v);
    assert_eq!(v, 0, "the value is clamped to the range");
}

#[test]
fn slider_int_is_exact_for_large_ranges() {
    let mut ui = ui();
    let mut v = i32::MAX - 1;
    let rect = frame(&mut ui, |ui| ui.slider_int("Big", &mut v, i32::MIN, i32::MAX)).0.rect;
    ui.push_event(InputEvent::PointerMoved { x: rect.min.x - 50.0, y: -100.0 });
    frame(&mut ui, |ui| ui.slider_int("Big", &mut v, i32::MIN, i32::MAX));
    // Focus it by keyboard so the pointer does not move the value.
    press(&mut ui, Key::Tab);
    frame(&mut ui, |ui| ui.slider_int("Big", &mut v, i32::MIN, i32::MAX));
    press(&mut ui, Key::ArrowRight);
    frame(&mut ui, |ui| ui.slider_int("Big", &mut v, i32::MIN, i32::MAX));
    assert_eq!(v, i32::MAX);
}

#[test]
fn spin_box_int_steps_clamps_and_drags() {
    let mut ui = ui();
    let mut v = 3;
    let run = |ui: &mut Ui, v: &mut i32| frame(ui, |ui| ui.spin_box_int("Threads", v, 1, 8, 2)).0;
    let r = run(&mut ui, &mut v);
    // The + stepper sits at the right end.
    click(&mut ui, Vec2::new(r.rect.max.x - 10.0, center_y(r.rect)));
    run(&mut ui, &mut v);
    assert_eq!(v, 5);
    press(&mut ui, Key::Tab);
    run(&mut ui, &mut v);
    press(&mut ui, Key::ArrowUp);
    press(&mut ui, Key::ArrowUp);
    run(&mut ui, &mut v);
    assert_eq!(v, 8, "steps stop at the maximum");
    press(&mut ui, Key::Home);
    run(&mut ui, &mut v);
    assert_eq!(v, 1);
    // Dragging moves one step per 4 px, in whole steps.
    let c = r.rect.center();
    ui.push_event(InputEvent::PointerMoved { x: c.x, y: c.y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    run(&mut ui, &mut v);
    for i in 1..=6 {
        ui.push_event(InputEvent::PointerMoved { x: c.x + i as f32 * 2.0, y: c.y });
        run(&mut ui, &mut v);
    }
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
    run(&mut ui, &mut v);
    assert!(v >= 5 && (v - 1) % 2 == 0, "dragging moves in whole steps of 2, got {v}");
}

#[test]
fn search_button_is_a_keyboard_button() {
    let mut ui = ui();
    let mut clicks = 0;
    let mut run = |ui: &mut Ui| {
        frame(ui, |ui| {
            if ui.search_button("Search commands", "Ctrl K").clicked {
                clicks += 1;
            }
        });
        clicks
    };
    run(&mut ui);
    let w = ui.output().widgets.iter().find(|w| w.kind == "search_button").expect("recorded").clone();
    assert_eq!(w.label, "Search commands");
    press(&mut ui, Key::Tab);
    run(&mut ui);
    assert_eq!(ui.focused_id(), Some(w.id));
    press(&mut ui, Key::Enter);
    assert_eq!(run(&mut ui), 1);
    press(&mut ui, Key::Space);
    assert_eq!(run(&mut ui), 2);
    click(&mut ui, w.rect.center());
    assert_eq!(run(&mut ui), 3);
}

#[test]
fn clipped_text_is_shortened_to_fit() {
    let mut ui = ui();
    let long = "/home/user/projects/rugi/crates/rugi-ui/src/widgets/advanced.rs";
    let (r, _) = frame(&mut ui, |ui| ui.label_truncated(long, TextRole::Body, Some(150.0)));
    assert!(r.width() <= 150.5 && r.width() > 100.0, "{r:?}");
    let (r, _) = frame(&mut ui, |ui| ui.label_truncated("short", TextRole::Body, Some(150.0)));
    assert!(r.width() < 60.0);
    let style = ui.theme().typography.body;
    for max in [0.0, 5.0, 40.0, 333.0] {
        let l = ui.layout_text_fit(long, &style, max);
        assert!(l.size().x <= max.max(l.size().x.min(20.0)) + 0.5, "{max}: {}", l.size().x);
    }
}
