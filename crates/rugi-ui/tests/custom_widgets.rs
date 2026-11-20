//! Headless tests for the custom-menu widgets and effects: key bindings,
//! multi-select, segmented control, rail, carousel, page dots, transitions,
//! glass containers and window chrome.

use rugi_core::{InputEvent, Key, Modifiers, PointerButton, PrimitiveKind, WindowCommand};
use rugi_ui::{
    lucide, Backdrop, BackdropKind, Bind, BindMode, FrameInput, GlassStyle, KeyBind, Rect, Slide, Ui, UiOptions, Vec2,
};

const INPUT: FrameInput = FrameInput { size: Vec2 { x: 900.0, y: 640.0 }, scale: 1.0, dt: 1.0 / 60.0 };

fn ui() -> Ui {
    Ui::new(UiOptions { inspection: true, ..Default::default() })
}

fn frame<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.begin_frame(INPUT).unwrap();
    let r = ui.central_panel(f);
    ui.end_frame();
    assert_eq!(ui.take_error(), None);
    r
}

fn press(ui: &mut Ui, button: PointerButton, p: Vec2) {
    ui.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
    ui.push_event(InputEvent::PointerButton { button, pressed: true });
    ui.push_event(InputEvent::PointerButton { button, pressed: false });
}

fn key(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.push_event(InputEvent::Key { key, pressed: true, modifiers });
    ui.push_event(InputEvent::Key { key, pressed: false, modifiers });
}

fn rect_of(ui: &Ui, label: &str) -> Rect {
    ui.output().widgets.iter().find(|w| w.label == label).unwrap_or_else(|| panic!("no widget {label}")).rect
}

#[test]
fn keybind_captures_keys_combos_mouse_and_clears() {
    let mut ui = ui();
    let mut bind = KeyBind::default();
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    let r = rect_of(&ui, "Record");
    press(&mut ui, PointerButton::Primary, r.center());
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    // Listening: the next key is captured with its modifiers.
    let ctrl = Modifiers { ctrl: true, ..Default::default() };
    key(&mut ui, Key::K, ctrl);
    let changed = frame(&mut ui, |ui| ui.keybind("Record", &mut bind)).changed;
    assert!(changed);
    assert_eq!(bind.bind, Bind::Key(Key::K));
    assert!(bind.mods.ctrl);
    let mut label = String::new();
    bind.write_label(&mut label);
    assert_eq!(label, "Ctrl+K");
    // Not listening any more: keys do nothing.
    key(&mut ui, Key::F9, Modifiers::default());
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    assert_eq!(bind.bind, Bind::Key(Key::K));
    // Rebind to the middle mouse button.
    press(&mut ui, PointerButton::Primary, r.center());
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    press(&mut ui, PointerButton::Middle, r.center());
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    assert_eq!(bind.bind, Bind::Mouse(PointerButton::Middle));
    // Backspace clears; Escape cancels without change.
    press(&mut ui, PointerButton::Primary, r.center());
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    key(&mut ui, Key::Backspace, Modifiers::default());
    frame(&mut ui, |ui| ui.keybind("Record", &mut bind));
    assert_eq!(bind.bind, Bind::None);
}

#[test]
fn bind_active_hold_and_toggle() {
    let mut ui = ui();
    let mut hold = KeyBind::key(Key::F9);
    let mut toggle = KeyBind::key(Key::Insert).with_mode(BindMode::Toggle);
    let mut always = KeyBind::default().with_mode(BindMode::Always);
    ui.push_event(InputEvent::Key { key: Key::F9, pressed: true, modifiers: Modifiers::default() });
    key(&mut ui, Key::Insert, Modifiers::default());
    let (h, t, a) =
        frame(&mut ui, |ui| (ui.bind_active(&mut hold), ui.bind_active(&mut toggle), ui.bind_active(&mut always)));
    assert!(h && t && a);
    // F9 still held next frame; toggle stays on without a new press.
    let (h, t) = frame(&mut ui, |ui| (ui.bind_active(&mut hold), ui.bind_active(&mut toggle)));
    assert!(h && t);
    ui.push_event(InputEvent::Key { key: Key::F9, pressed: false, modifiers: Modifiers::default() });
    key(&mut ui, Key::Insert, Modifiers::default());
    let (h, t) = frame(&mut ui, |ui| (ui.bind_active(&mut hold), ui.bind_active(&mut toggle)));
    assert!(!h && !t);
}

#[test]
fn multi_select_toggles_items_and_summarises() {
    let mut ui = ui();
    let items = ["Head", "Chest", "Legs"];
    let mut flags = [false, true, false];
    frame(&mut ui, |ui| ui.multi_select("Parts", &mut flags, &items));
    let r = rect_of(&ui, "Parts");
    press(&mut ui, PointerButton::Primary, r.center());
    frame(&mut ui, |ui| ui.multi_select("Parts", &mut flags, &items));
    let head = rect_of(&ui, "Head");
    press(&mut ui, PointerButton::Primary, head.center());
    let changed = frame(&mut ui, |ui| ui.multi_select("Parts", &mut flags, &items)).changed;
    assert!(changed);
    assert_eq!(flags, [true, true, false]);
    // The popup stays open for further toggles.
    assert!(ui.output().widgets.iter().any(|w| w.label == "Legs"));
}

#[test]
fn segmented_selects_by_click_and_arrows() {
    let mut ui = ui();
    let mut sel = 0usize;
    let labels = ["720p", "1080p", "1440p"];
    frame(&mut ui, |ui| ui.segmented("res", &mut sel, &labels));
    let r = rect_of(&ui, "res");
    press(&mut ui, PointerButton::Primary, Vec2::new(r.max.x - 10.0, r.center().y));
    assert!(frame(&mut ui, |ui| ui.segmented("res", &mut sel, &labels)).changed);
    assert_eq!(sel, 2);
    key(&mut ui, Key::ArrowLeft, Modifiers::default());
    frame(&mut ui, |ui| ui.segmented("res", &mut sel, &labels));
    assert_eq!(sel, 1);
}

#[test]
fn rail_items_select_and_carousel_moves() {
    let mut ui = ui();
    let mut page = 0usize;
    let mut index = 0usize;
    let mut painted = Vec::new();
    let build = |ui: &mut Ui, page: &mut usize, index: &mut usize, painted: &mut Vec<usize>| {
        for (i, icon) in [lucide::HOUSE, lucide::SETTINGS].iter().enumerate() {
            if ui.rail_item(&format!("rail{i}"), (*icon).into(), *page == i, &format!("Page {i}")).clicked {
                *page = i;
            }
        }
        painted.clear();
        let r = ui.carousel("cards", 5, index, Vec2::new(200.0, 260.0), |_, i, _, _| painted.push(i));
        ui.page_dots("dots", 5, index);
        r
    };
    frame(&mut ui, |ui| build(ui, &mut page, &mut index, &mut painted));
    assert!(painted.contains(&0) && painted.contains(&1) && !painted.contains(&4), "only nearby cards paint");
    assert_eq!(*painted.last().unwrap(), 0, "centered card paints last (on top)");
    let rail1 = rect_of(&ui, "Page 1");
    press(&mut ui, PointerButton::Primary, rail1.center());
    frame(&mut ui, |ui| build(ui, &mut page, &mut index, &mut painted));
    assert_eq!(page, 1);
    // Click the card to the right of center: it becomes the selection.
    let area = rect_of(&ui, "cards");
    press(&mut ui, PointerButton::Primary, Vec2::new(area.center().x + 200.0, area.center().y));
    let r = frame(&mut ui, |ui| build(ui, &mut page, &mut index, &mut painted));
    assert!(r.changed);
    assert_eq!(index, 1);
    // Clicking the centered card reports a click without changing the index.
    for _ in 0..60 {
        frame(&mut ui, |ui| build(ui, &mut page, &mut index, &mut painted));
    }
    press(&mut ui, PointerButton::Primary, area.center());
    let r = frame(&mut ui, |ui| build(ui, &mut page, &mut index, &mut painted));
    assert!(r.clicked && !r.changed);
}

#[test]
fn glass_backdrop_and_transition_emit_primitives() {
    let mut ui = ui();
    let mut key = 0u64;
    let build = |ui: &mut Ui, key: u64| {
        ui.backdrop(&Backdrop::new(BackdropKind::Smoke));
        ui.glass_card(&GlassStyle::liquid(), |ui| {
            ui.begin_transition("page", key, Slide::Forward);
            ui.label("Content");
            ui.end();
        });
        ui.begin_glass_panel(Rect::from_xywh(500.0, 300.0, 200.0, 120.0), &GlassStyle::frosted());
        ui.label("Panel");
        ui.end();
    };
    frame(&mut ui, |ui| build(ui, key));
    let out = ui.output();
    let kinds: Vec<u32> = out.draw_list.instances().iter().map(|i| i.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == PrimitiveKind::Backdrop as u32).count(), 1);
    assert_eq!(out.draw_list.glass_count(), 2);
    assert_eq!(out.repaint(), rugi_core::Repaint::Immediately, "animated backdrop keeps frames coming");
    let content_x = rect_of(&ui, "Content").min.x;
    // Changing the key restarts the transition: content starts offset and faded.
    key = 1;
    frame(&mut ui, |ui| build(ui, key));
    assert!(rect_of(&ui, "Content").min.x > content_x, "slides in from the right");
}

#[test]
fn title_bar_emits_window_commands() {
    let mut ui = ui();
    frame(&mut ui, |ui| ui.title_bar("App", 32.0));
    let bar = rect_of(&ui, "App");
    // Press on the empty bar: window drag.
    ui.push_event(InputEvent::PointerMoved { x: bar.min.x + 40.0, y: bar.center().y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    frame(&mut ui, |ui| ui.title_bar("App", 32.0));
    assert_eq!(ui.output().window_commands, vec![WindowCommand::StartDrag]);
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
    frame(&mut ui, |ui| ui.title_bar("App", 32.0));
    // The recorded rect is the drag area; minimize (28 px) and close follow it.
    press(&mut ui, PointerButton::Primary, Vec2::new(bar.max.x + 6.0 + 28.0 + 6.0 + 14.0, bar.center().y));
    frame(&mut ui, |ui| ui.title_bar("App", 32.0));
    assert_eq!(ui.output().window_commands, vec![WindowCommand::Close]);
}

#[test]
fn password_input_masks_text() {
    let mut ui = ui();
    let mut pw = String::new();
    let opts = rugi_ui::TextInputOptions { password: true, icon: Some(lucide::LOCK.into()) };
    frame(&mut ui, |ui| ui.text_input_ex("##pw", &mut pw, "Password", opts));
    let r = rect_of(&ui, "Password");
    press(&mut ui, PointerButton::Primary, r.center());
    frame(&mut ui, |ui| ui.text_input_ex("##pw", &mut pw, "Password", opts));
    ui.push_event(InputEvent::Text("hunter2".into()));
    frame(&mut ui, |ui| ui.text_input_ex("##pw", &mut pw, "Password", opts));
    assert_eq!(pw, "hunter2");
    // Copy is disabled for password fields.
    key(&mut ui, Key::A, Modifiers { logo: true, ctrl: true, ..Default::default() });
    key(&mut ui, Key::C, Modifiers { logo: true, ctrl: true, ..Default::default() });
    frame(&mut ui, |ui| ui.text_input_ex("##pw", &mut pw, "Password", opts));
    assert_eq!(ui.output().clipboard, None);
}

#[test]
fn ambient_backdrop_respects_frame_budget() {
    let mut ui = ui();
    let b = Backdrop::new(BackdropKind::Aurora);
    frame(&mut ui, |ui| ui.backdrop(&b));
    assert_eq!(ui.output().repaint(), rugi_core::Repaint::Immediately, "focused: display refresh");
    // Unfocused: 30 fps by default.
    ui.push_event(InputEvent::WindowFocused(false));
    frame(&mut ui, |ui| ui.backdrop(&b));
    assert_eq!(ui.output().repaint(), rugi_core::Repaint::After(std::time::Duration::from_secs_f32(1.0 / 30.0)));
    // A widget animation still gets full-rate frames.
    let mut on = false;
    frame(&mut ui, |ui| ui.toggle("t", &mut on));
    let r = rect_of(&ui, "t");
    press(&mut ui, PointerButton::Primary, r.center());
    frame(&mut ui, |ui| {
        ui.backdrop(&b);
        ui.toggle("t", &mut on)
    });
    assert_eq!(ui.output().repaint(), rugi_core::Repaint::Immediately);
    // Custom budget while focused; frozen backdrops go idle.
    ui.push_event(InputEvent::WindowFocused(true));
    ui.set_ambient_rate(rugi_ui::AmbientRate { focused: Some(60.0), unfocused: Some(10.0) });
    for _ in 0..30 {
        frame(&mut ui, |ui| ui.backdrop(&b));
    }
    assert_eq!(ui.output().repaint(), rugi_core::Repaint::After(std::time::Duration::from_secs_f32(1.0 / 60.0)));
    let frozen = Backdrop { speed: 0.0, ..b };
    frame(&mut ui, |ui| ui.backdrop(&frozen));
    assert_eq!(ui.output().repaint(), rugi_core::Repaint::Idle);
}
