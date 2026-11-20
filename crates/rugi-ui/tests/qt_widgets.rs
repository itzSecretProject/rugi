//! Headless tests for popups, menus, dialogs and composite widgets.

use rugi_core::{InputEvent, Key, Modifiers, PointerButton};
use rugi_ui::{FrameInput, Rect, Ui, UiOptions, Vec2};

const INPUT: FrameInput = FrameInput { size: Vec2 { x: 800.0, y: 600.0 }, scale: 1.0, dt: 1.0 / 60.0 };

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

fn click(ui: &mut Ui, p: Vec2) {
    ui.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: true });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Primary, pressed: false });
}

fn rect_of(ui: &Ui, label: &str) -> Rect {
    ui.output().widgets.iter().find(|w| w.label == label).unwrap_or_else(|| panic!("no widget {label}")).rect
}

fn click_on(ui: &mut Ui, label: &str) {
    let c = rect_of(ui, label).center();
    click(ui, c);
}

fn has(ui: &Ui, label: &str) -> bool {
    ui.output().widgets.iter().any(|w| w.label == label)
}

#[test]
fn combo_box_opens_selects_and_closes() {
    let mut ui = ui();
    let items = ["Low", "Medium", "High"];
    let mut sel = 0usize;
    let mut below = 0;
    let mut build = |ui: &mut Ui, sel: &mut usize| {
        let r = ui.combo_box("quality", sel, &items);
        if ui.button("Below").clicked {
            below += 1;
        }
        r
    };
    let r = frame(&mut ui, |ui| build(ui, &mut sel)).rect;
    click(&mut ui, r.center());
    frame(&mut ui, |ui| build(ui, &mut sel));
    assert!(has(&ui, "High"), "popup items are built once open");
    // The popup covers the "Below" button: clicking there picks the item, not the button.
    let high = rect_of(&ui, "High");
    click(&mut ui, high.center());
    let changed = frame(&mut ui, |ui| build(ui, &mut sel)).changed;
    assert!(changed);
    assert_eq!(sel, 2);
    frame(&mut ui, |ui| build(ui, &mut sel));
    assert!(!has(&ui, "Medium"), "popup closed after choosing");
    // Keyboard: focused combo, ArrowUp.
    ui.push_event(InputEvent::Key { key: Key::ArrowUp, pressed: true, modifiers: Modifiers::default() });
    frame(&mut ui, |ui| build(ui, &mut sel));
    assert_eq!(sel, 1);
    assert_eq!(below, 0);
}

#[test]
fn popup_blocks_and_closes_on_outside_click_and_escape() {
    let mut ui = ui();
    let mut clicks = 0;
    let mut build = |ui: &mut Ui| {
        let open = ui.menu("File", |ui| {
            ui.menu_item("New", "⌘N");
            ui.menu_item("Open…", "⌘O");
        });
        if ui.button("Behind").clicked {
            clicks += 1;
        }
        open.is_some()
    };
    frame(&mut ui, &mut build);
    let file = rect_of(&ui, "File");
    click(&mut ui, file.center());
    assert!(frame(&mut ui, &mut build), "menu opened");
    // Click the "Behind" button while the menu is open: it is covered
    // by the popup's area? If not covered, the outside click closes the menu.
    let behind = rect_of(&ui, "Behind");
    click(&mut ui, Vec2::new(700.0, 500.0));
    frame(&mut ui, &mut build);
    assert!(!frame(&mut ui, &mut build), "outside click closes");
    click(&mut ui, file.center());
    assert!(frame(&mut ui, &mut build));
    ui.push_event(InputEvent::Key { key: Key::Escape, pressed: true, modifiers: Modifiers::default() });
    frame(&mut ui, &mut build);
    assert!(!frame(&mut ui, &mut build), "escape closes");
    let _ = behind;
    assert_eq!(clicks, 0);
}

#[test]
fn menu_item_click_returns_and_closes() {
    let mut ui = ui();
    let mut chosen = None;
    let mut build = |ui: &mut Ui| {
        ui.menu("Edit", |ui| {
            if ui.menu_item("Undo", "⌘Z").clicked {
                chosen = Some("undo");
            }
        });
    };
    frame(&mut ui, &mut build);
    click_on(&mut ui, "Edit");
    frame(&mut ui, &mut build);
    click_on(&mut ui, "Undo");
    frame(&mut ui, &mut build);
    frame(&mut ui, &mut build);
    assert!(!has(&ui, "Undo"));
    assert_eq!(chosen, Some("undo"));
}

#[test]
fn context_menu_opens_on_right_click() {
    let mut ui = ui();
    let build = |ui: &mut Ui| {
        ui.button("Row");
        ui.context_menu("ctx", |ui| {
            ui.menu_item("Delete", "");
        })
        .is_some()
    };
    frame(&mut ui, build);
    let row = rect_of(&ui, "Row");
    ui.push_event(InputEvent::PointerMoved { x: row.center().x, y: row.center().y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Secondary, pressed: true });
    assert!(frame(&mut ui, build));
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Secondary, pressed: false });
    assert!(frame(&mut ui, build));
    assert!(has(&ui, "Delete"));
}

#[test]
fn modal_blocks_everything_outside() {
    let mut ui = ui();
    let mut open = false;
    let mut outside = 0;
    let mut inside = 0;
    let mut build = |ui: &mut Ui, open: &mut bool| {
        if ui.button("Outside").clicked {
            outside += 1;
        }
        ui.modal("dlg", "Confirm", open, 400.0, |ui| {
            if ui.button("OK").clicked {
                inside += 1;
            }
        });
    };
    frame(&mut ui, |ui| build(ui, &mut open));
    let out_r = rect_of(&ui, "Outside");
    open = true;
    frame(&mut ui, |ui| build(ui, &mut open));
    frame(&mut ui, |ui| build(ui, &mut open));
    click(&mut ui, out_r.center());
    frame(&mut ui, |ui| build(ui, &mut open));
    click_on(&mut ui, "OK");
    frame(&mut ui, |ui| build(ui, &mut open));
    ui.push_event(InputEvent::Key { key: Key::Escape, pressed: true, modifiers: Modifiers::default() });
    frame(&mut ui, |ui| build(ui, &mut open));
    assert_eq!((outside, inside), (0, 1));
    assert!(!open, "escape closes the dialog");
}

#[test]
fn tabs_radio_collapsing_spin() {
    let mut ui = ui();
    let mut tab = 0usize;
    let mut mode = 1u8;
    let mut n = 5.0f32;
    let build = |ui: &mut Ui, tab: &mut usize, mode: &mut u8, n: &mut f32| -> bool {
        ui.tabs("tabs", tab, &["General", "Advanced"]);
        ui.radio("A", mode, 1);
        ui.radio("B", mode, 2);
        ui.set_next_item_width(160.0);
        ui.spin_box("count", n, 0.0, 10.0, 1.0);
        ui.collapsing("Details", false, |ui| ui.label("Hidden body")).is_some()
    };
    assert!(!frame(&mut ui, |ui| build(ui, &mut tab, &mut mode, &mut n)));
    click_on(&mut ui, "Advanced");
    frame(&mut ui, |ui| build(ui, &mut tab, &mut mode, &mut n));
    assert_eq!(tab, 1);
    click_on(&mut ui, "B");
    frame(&mut ui, |ui| build(ui, &mut tab, &mut mode, &mut n));
    assert_eq!(mode, 2);
    let spin = rect_of(&ui, "count");
    click(&mut ui, Vec2::new(spin.max.x - 10.0, spin.center().y));
    frame(&mut ui, |ui| build(ui, &mut tab, &mut mode, &mut n));
    assert_eq!(n, 6.0);
    click_on(&mut ui, "Details");
    assert!(frame(&mut ui, |ui| build(ui, &mut tab, &mut mode, &mut n)));
}

#[test]
fn table_is_virtualized_and_selects() {
    let mut ui = ui();
    let mut sel = None;
    let mut calls = 0usize;
    let build = |ui: &mut Ui, sel: &mut Option<usize>, calls: &mut usize| {
        ui.table("t", &["Name", "Size"], &[2.0, 1.0], 100_000, sel, 300.0, |ui, row, col| {
            *calls += 1;
            if col == 0 && row < 3 {
                ui.label(if row == 1 { "second" } else { "row" });
            }
        });
    };
    frame(&mut ui, |ui| build(ui, &mut sel, &mut calls));
    assert!(calls > 0 && calls < 40, "only visible rows build cells, got {calls}");
    click_on(&mut ui, "second");
    frame(&mut ui, |ui| build(ui, &mut sel, &mut calls));
    assert_eq!(sel, Some(1));
    // Right-click inside the table body opens its context menu.
    let ctx = |ui: &mut Ui, sel: &mut Option<usize>, calls: &mut usize| {
        build(ui, sel, calls);
        ui.context_menu("rows", |ui| {
            ui.menu_item("Delete row", "");
        });
    };
    let p = rect_of(&ui, "second").center();
    ui.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Secondary, pressed: true });
    ui.push_event(InputEvent::PointerButton { button: PointerButton::Secondary, pressed: false });
    frame(&mut ui, |ui| ctx(ui, &mut sel, &mut calls));
    frame(&mut ui, |ui| ctx(ui, &mut sel, &mut calls));
    assert!(has(&ui, "Delete row"));
}

#[test]
fn copy_and_paste_in_text_input() {
    let mut ui = ui();
    let mut s = String::from("hello world");
    let cmd = if cfg!(target_os = "macos") {
        Modifiers { logo: true, ..Default::default() }
    } else {
        Modifiers { ctrl: true, ..Default::default() }
    };
    let r = frame(&mut ui, |ui| ui.text_input("t", &mut s, "").rect);
    click(&mut ui, r.center());
    frame(&mut ui, |ui| ui.text_input("t", &mut s, ""));
    ui.push_event(InputEvent::Key { key: Key::A, pressed: true, modifiers: cmd });
    ui.push_event(InputEvent::Key { key: Key::C, pressed: true, modifiers: cmd });
    frame(&mut ui, |ui| ui.text_input("t", &mut s, ""));
    assert_eq!(ui.output().clipboard.as_deref(), Some("hello world"));
    ui.push_event(InputEvent::Key { key: Key::End, pressed: true, modifiers: Modifiers::default() });
    ui.push_event(InputEvent::Paste("!\nx".into()));
    frame(&mut ui, |ui| ui.text_input("t", &mut s, ""));
    assert_eq!(s, "hello world! x");
}

#[test]
fn scoped_styles_restore() {
    let mut ui = ui();
    let base = ui.theme().metrics.control_height;
    let h = frame(&mut ui, |ui| {
        let r = ui.with_style(|t| t.metrics.control_height = 48.0, |ui| ui.button("Big").rect);
        let s = ui.button("Normal").rect;
        (r.height(), s.height())
    });
    assert_eq!(h, (48.0, base));
    // Unbalanced push is restored at end_frame and reported.
    ui.begin_frame(INPUT).unwrap();
    ui.push_style(|t| t.metrics.control_height = 99.0);
    ui.end_frame();
    assert!(ui.take_error().is_some());
    assert_eq!(ui.theme().metrics.control_height, base);
}

#[test]
fn tooltip_appears_after_delay() {
    let mut ui = ui();
    let build = |ui: &mut Ui| {
        ui.button("Help");
        ui.tooltip("Explains things");
    };
    frame(&mut ui, build);
    let r = rect_of(&ui, "Help");
    ui.push_event(InputEvent::PointerMoved { x: r.center().x, y: r.center().y });
    frame(&mut ui, build);
    let before = ui.output().draw_list.instances().len();
    assert!(matches!(ui.output().repaint(), rugi_ui::Repaint::After(_) | rugi_ui::Repaint::Immediately));
    for _ in 0..40 {
        frame(&mut ui, build);
    }
    assert!(ui.output().draw_list.instances().len() > before + 5, "tooltip drawn");
}
