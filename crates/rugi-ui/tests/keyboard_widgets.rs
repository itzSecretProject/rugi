//! Keyboard operation of widgets with their own key handling: table row
//! selection, context menus (Shift+F10 and the Menu key), arrow keys in
//! popups, and the color picker's square and hue bar.

use rugi_core::{Color, InputEvent, Key, Modifiers};
use rugi_ui::{FrameInput, Ui, UiOptions, Vec2};

const INPUT: FrameInput = FrameInput { size: Vec2 { x: 900.0, y: 700.0 }, scale: 1.0, dt: 1.0 / 60.0 };
const SHIFT: Modifiers = Modifiers { shift: true, ctrl: false, alt: false, logo: false };
const NONE: Modifiers = Modifiers { shift: false, ctrl: false, alt: false, logo: false };

fn ui() -> Ui {
    Ui::new(UiOptions { inspection: true, ..Default::default() })
}

fn frame(ui: &mut Ui, f: &mut impl FnMut(&mut Ui)) {
    ui.begin_frame(INPUT).unwrap();
    ui.central_panel(|ui| f(ui));
    ui.end_frame();
    assert_eq!(ui.take_error(), None);
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers, f: &mut impl FnMut(&mut Ui)) {
    ui.push_event(InputEvent::Key { key, pressed: true, modifiers });
    ui.push_event(InputEvent::Key { key, pressed: false, modifiers });
    frame(ui, f);
    // A second frame applies focus moves made at the end of the first.
    frame(ui, f);
}

fn focused_kind(ui: &Ui) -> Option<(&'static str, String)> {
    let id = ui.focused_id()?;
    ui.output().widgets.iter().find(|w| w.id == id).map(|w| (w.kind, w.label.clone()))
}

fn visible(ui: &Ui, label: &str) -> bool {
    ui.output().widgets.iter().any(|w| w.label == label && !w.rect.intersect(&w.clip).is_empty())
}

/// Presses Tab until a widget of `kind` has focus.
fn tab_to(ui: &mut Ui, kind: &str, f: &mut impl FnMut(&mut Ui)) {
    for _ in 0..20 {
        press(ui, Key::Tab, NONE, f);
        if focused_kind(ui).is_some_and(|(k, _)| k == kind) {
            return;
        }
    }
    panic!("Tab never reached a {kind}");
}

#[test]
fn arrow_keys_move_the_table_selection_and_scroll_it_into_view() {
    let mut ui = ui();
    let mut selected = None;
    let mut draw = |ui: &mut Ui| {
        ui.table("rows", &["Name"], &[1.0], 1000, &mut selected, 200.0, |ui, row, _| {
            ui.label(&format!("row {row}"));
        });
    };
    frame(&mut ui, &mut draw);
    tab_to(&mut ui, "table", &mut draw);

    let mut step = |ui: &mut Ui, key: Key| {
        let mut sel = None;
        let mut draw = |ui: &mut Ui| {
            ui.table("rows", &["Name"], &[1.0], 1000, &mut selected, 200.0, |ui, row, _| {
                ui.label(&format!("row {row}"));
            });
            sel = selected;
        };
        press(ui, key, NONE, &mut draw);
        sel
    };
    assert_eq!(step(&mut ui, Key::ArrowDown), Some(0));
    assert_eq!(step(&mut ui, Key::ArrowDown), Some(1));
    assert_eq!(step(&mut ui, Key::ArrowUp), Some(0));
    assert_eq!(step(&mut ui, Key::End), Some(999));
    assert!(visible(&ui, "row 999"), "End must scroll the last row into view");
    assert_eq!(step(&mut ui, Key::Home), Some(0));
    assert!(visible(&ui, "row 0"));
    let after_page = step(&mut ui, Key::PageDown).unwrap();
    assert!(after_page > 1, "PageDown moved only to {after_page}");
    assert!(visible(&ui, &format!("row {after_page}")));
}

/// A button with a three-item context menu; returns the clicked item.
fn menu_target(ui: &mut Ui, clicked: &mut Option<&'static str>) {
    ui.button("Target");
    ui.context_menu("ctx", |ui| {
        for item in ["Open", "Rename", "Delete"] {
            if ui.menu_item(item, "").clicked {
                *clicked = Some(item);
            }
        }
    });
}

#[test]
fn shift_f10_opens_the_context_menu_and_arrows_choose_an_item() {
    let mut ui = ui();
    let mut clicked = None;
    let mut draw = |ui: &mut Ui| menu_target(ui, &mut clicked);
    frame(&mut ui, &mut draw);
    tab_to(&mut ui, "button", &mut draw);

    press(&mut ui, Key::F10, SHIFT, &mut draw);
    assert_eq!(focused_kind(&ui), Some(("menu_item", "Open".into())), "first item takes focus");
    press(&mut ui, Key::ArrowDown, NONE, &mut draw);
    assert_eq!(focused_kind(&ui), Some(("menu_item", "Rename".into())));
    press(&mut ui, Key::ArrowUp, NONE, &mut draw);
    press(&mut ui, Key::ArrowUp, NONE, &mut draw);
    assert_eq!(focused_kind(&ui), Some(("menu_item", "Delete".into())), "Up wraps to the last item");
    press(&mut ui, Key::Enter, NONE, &mut draw);
    assert!(!visible(&ui, "Open"), "the menu closes after a choice");
    assert_eq!(focused_kind(&ui), Some(("button", "Target".into())), "focus returns to the opener");
    assert_eq!(clicked, Some("Delete"));
}

#[test]
fn the_menu_key_opens_context_menus_and_escape_returns_focus() {
    let mut ui = ui();
    let mut clicked = None;
    let mut draw = |ui: &mut Ui| menu_target(ui, &mut clicked);
    frame(&mut ui, &mut draw);
    tab_to(&mut ui, "button", &mut draw);
    press(&mut ui, Key::ContextMenu, NONE, &mut draw);
    assert!(visible(&ui, "Rename"));
    press(&mut ui, Key::Escape, NONE, &mut draw);
    assert!(!visible(&ui, "Rename"));
    assert_eq!(focused_kind(&ui), Some(("button", "Target".into())));
}

#[test]
fn a_focused_table_opens_its_context_menu_from_the_keyboard() {
    let mut ui = ui();
    let mut selected = Some(3);
    let mut draw = |ui: &mut Ui| {
        ui.table("rows", &["Name"], &[1.0], 50, &mut selected, 200.0, |ui, row, _| {
            ui.label(&format!("row {row}"));
        });
        ui.context_menu("row-menu", |ui| {
            ui.menu_item("Reveal", "");
        });
    };
    frame(&mut ui, &mut draw);
    tab_to(&mut ui, "table", &mut draw);
    press(&mut ui, Key::ContextMenu, NONE, &mut draw);
    assert_eq!(focused_kind(&ui), Some(("menu_item", "Reveal".into())));
}

#[test]
fn color_picker_square_and_hue_bar_respond_to_arrow_keys() {
    let mut ui = ui();
    let mut color = Color::hex(0x336699);
    let mut current = color;
    let mut draw = |ui: &mut Ui| {
        ui.color_edit("Tint", &mut color);
        current = color;
    };
    frame(&mut ui, &mut draw);
    tab_to(&mut ui, "color_edit", &mut draw);
    press(&mut ui, Key::Enter, NONE, &mut draw);

    tab_to(&mut ui, "color_square", &mut draw);
    let before = current_color(&mut ui, &mut color);
    press_on_picker(&mut ui, &mut color, Key::ArrowRight, SHIFT);
    let more_saturated = current_color(&mut ui, &mut color);
    assert_ne!(more_saturated, before, "Shift+Right must raise saturation");
    press_on_picker(&mut ui, &mut color, Key::ArrowDown, SHIFT);
    let darker = current_color(&mut ui, &mut color);
    assert!(darker.luma() < more_saturated.luma(), "Down must lower value");

    let mut draw = |ui: &mut Ui| {
        ui.color_edit("Tint", &mut color);
    };
    tab_to(&mut ui, "hue_bar", &mut draw);
    let before = current_color(&mut ui, &mut color);
    press_on_picker(&mut ui, &mut color, Key::ArrowRight, SHIFT);
    assert_ne!(current_color(&mut ui, &mut color), before, "Shift+Right must change the hue");
    let _ = current;
}

fn press_on_picker(ui: &mut Ui, color: &mut Color, key: Key, mods: Modifiers) {
    let mut draw = |ui: &mut Ui| {
        ui.color_edit("Tint", color);
    };
    press(ui, key, mods, &mut draw);
}

fn current_color(ui: &mut Ui, color: &mut Color) -> Color {
    let mut draw = |ui: &mut Ui| {
        ui.color_edit("Tint", color);
    };
    frame(ui, &mut draw);
    *color
}
