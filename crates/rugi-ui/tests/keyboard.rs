//! Keyboard reachability of the demo: Tab and Shift+Tab visit every focusable
//! widget on each page in a terminating cycle, Enter/Space activate the focused
//! control, and Escape closes popups and dialogs.

use rugi_core::{Id, InputEvent, Key, Modifiers, WidgetInfo};
use rugi_ui::{DemoPage, FrameInput, Ui, UiOptions, Vec2};
use std::collections::HashSet;

/// Tall enough that every page fits without scrolling.
const TALL: FrameInput = FrameInput { size: Vec2 { x: 1280.0, y: 12000.0 }, scale: 1.0, dt: 1.0 / 60.0 };
const NORMAL: FrameInput = FrameInput { size: Vec2 { x: 1280.0, y: 800.0 }, scale: 1.0, dt: 1.0 / 60.0 };

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

/// Widget kinds that take keyboard focus.
const FOCUSABLE: &[&str] = &[
    "button",
    "icon_button",
    "checkbox",
    "toggle",
    "slider",
    "nav_item",
    "combo_box",
    "tab",
    "radio",
    "tree_node",
    "collapsing",
    "spin_box",
    "text_input",
    "keybind",
    "multi_select",
    "rail_item",
    "carousel",
    "color_edit",
    "menu",
    "menu_item",
    "selectable",
    "custom",
    "code_block",
    "table",
    "color_square",
    "hue_bar",
    "search_button",
];

fn frame(ui: &mut Ui, input: FrameInput) {
    ui.begin_frame(input).unwrap();
    ui.show_demo();
    ui.end_frame();
    assert_eq!(ui.take_error(), None);
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.push_event(InputEvent::Key { key, pressed: true, modifiers });
    ui.push_event(InputEvent::Key { key, pressed: false, modifiers });
}

const SHIFT: Modifiers = Modifiers { shift: true, ctrl: false, alt: false, logo: false };

fn open_page(page: DemoPage, input: FrameInput) -> Ui {
    let mut ui = Ui::new(UiOptions { inspection: true, ..Default::default() });
    ui.set_demo_page(page);
    for _ in 0..3 {
        frame(&mut ui, input);
    }
    ui
}

fn focusable(ui: &Ui) -> Vec<WidgetInfo> {
    ui.output().widgets.iter().filter(|w| FOCUSABLE.contains(&w.kind)).cloned().collect()
}

/// Presses `key` until focus returns to the first focused widget; returns the
/// visited ids in order. Panics if the cycle does not close.
fn cycle(ui: &mut Ui, input: FrameInput, mods: Modifiers, limit: usize) -> Vec<Id> {
    let mut seen = Vec::new();
    for _ in 0..limit {
        press(ui, Key::Tab, mods);
        frame(ui, input);
        let f = ui.focused_id().expect("Tab left nothing focused");
        if seen.first() == Some(&f) {
            return seen;
        }
        if seen.contains(&f) {
            let names: Vec<String> = seen
                .iter()
                .chain([&f])
                .map(|id| {
                    ui.output()
                        .widgets
                        .iter()
                        .find(|w| w.id == *id)
                        .map_or("?".to_owned(), |w| format!("{}:{}", w.kind, w.label))
                })
                .collect();
            panic!("Tab revisited a widget before closing the cycle (trap): {names:?}");
        }
        seen.push(f);
    }
    panic!("Tab cycle did not close within {limit} presses");
}

fn name(ui: &Ui, id: Id) -> String {
    ui.output().widgets.iter().find(|w| w.id == id).map_or("?".to_owned(), |w| format!("{} {:?}", w.kind, w.label))
}

/// Checks the forward and backward Tab cycles of `page` in a window of the
/// given size and returns the number of widgets visited.
fn check_page(page: DemoPage, input: FrameInput) -> usize {
    let all = focusable(&open_page(page, TALL));
    let mut ids = HashSet::new();
    for w in &all {
        assert!(ids.insert(w.id), "{page:?}: {} {:?} shares its id with another widget", w.kind, w.label);
    }
    let limit = all.len() * 2 + 8;

    let mut ui = open_page(page, input);
    let forward = cycle(&mut ui, input, Modifiers::default(), limit);
    let visited: HashSet<Id> = forward.iter().copied().collect();
    let missing: Vec<_> = all.iter().filter(|w| !visited.contains(&w.id)).map(|w| (w.kind, &w.label)).collect();
    assert!(missing.is_empty(), "{page:?} at {:?}: Tab never reaches {missing:?}", input.size);

    // Shift+Tab walks the same cycle backwards.
    let mut ui = open_page(page, input);
    let mut backward = cycle(&mut ui, input, SHIFT, limit);
    backward.reverse();
    let shift = backward.iter().position(|id| *id == forward[0]).expect("Shift+Tab cycle differs");
    backward.rotate_left(shift);
    assert_eq!(backward, forward, "{page:?}: Shift+Tab is not the reverse of Tab");
    forward.len()
}

#[test]
fn tab_cycles_visit_every_widget_on_every_page() {
    for page in PAGES {
        let n = check_page(page, TALL);
        eprintln!("{page:?}: {n} Tab stops");
    }
}

#[test]
fn tab_reaches_widgets_below_the_fold() {
    for page in PAGES {
        check_page(page, NORMAL);
    }
}

#[test]
fn tab_scrolls_the_focused_widget_into_view() {
    for page in [DemoPage::Welcome, DemoPage::Layout, DemoPage::Style] {
        let mut ui = open_page(page, NORMAL);
        let n = focusable(&open_page(page, TALL)).len();
        for _ in 0..n {
            press(&mut ui, Key::Tab, Modifiers::default());
            for _ in 0..3 {
                frame(&mut ui, NORMAL);
            }
            let f = ui.focused_id().unwrap();
            let Some(w) = ui.output().widgets.iter().find(|w| w.id == f) else { continue };
            let visible = w.rect.intersect(&w.clip).intersect(&ui.screen_rect());
            assert!(
                visible.height() >= w.rect.height().min(1.0) && !visible.is_empty(),
                "{page:?}: focused {} is off-screen at {:?}",
                name(&ui, f),
                w.rect
            );
        }
    }
}

/// Presses Tab until a widget matching `pred` is focused.
fn focus(ui: &mut Ui, input: FrameInput, pred: impl Fn(&WidgetInfo) -> bool) {
    for _ in 0..200 {
        let f = ui.focused_id();
        if ui.output().widgets.iter().any(|w| Some(w.id) == f && pred(w)) {
            return;
        }
        press(ui, Key::Tab, Modifiers::default());
        // A widget focused while off-screen is scrolled into view, and
        // recorded, in the next frame.
        frame(ui, input);
        frame(ui, input);
    }
    panic!("no widget matching the predicate is reachable with Tab");
}

fn has_label(ui: &Ui, label: &str) -> bool {
    ui.output().widgets.iter().any(|w| w.label == label)
}

fn activate(ui: &mut Ui, key: Key) {
    press(ui, key, Modifiers::default());
    for _ in 0..3 {
        frame(ui, NORMAL);
    }
}

#[test]
fn enter_and_space_activate_focused_controls() {
    let mut ui = open_page(DemoPage::Welcome, NORMAL);
    focus(&mut ui, NORMAL, |w| w.kind == "toggle" && w.label == "Dark theme");
    let dark = ui.theme().dark;
    activate(&mut ui, Key::Space);
    assert_ne!(ui.theme().dark, dark, "Space did not switch the theme toggle");
    activate(&mut ui, Key::Enter);
    assert_eq!(ui.theme().dark, dark, "Enter did not switch the theme toggle back");

    focus(&mut ui, NORMAL, |w| w.kind == "toggle" && w.label == "Inspector");
    activate(&mut ui, Key::Enter);
    assert!(ui.inspector_enabled(), "Enter did not switch the inspector toggle");
    activate(&mut ui, Key::Space);
    assert!(!ui.inspector_enabled());

    // The copy button of the first section's code, shown by default.
    focus(&mut ui, NORMAL, |w| w.kind == "code_block");
    press(&mut ui, Key::Enter, Modifiers::default());
    frame(&mut ui, NORMAL);
    let copied = ui.output().clipboard.as_deref().unwrap_or("");
    assert!(
        copied.starts_with("ui.title(\"Hello, Rugi\");"),
        "Enter on the copy button put {copied:?} on the clipboard"
    );
    // A section's code toggle.
    let shown = |ui: &Ui| ui.output().widgets.iter().filter(|w| w.kind == "code_block").count();
    let before = shown(&ui);
    focus(&mut ui, NORMAL, |w| w.kind == "button" && w.label == "Show code");
    activate(&mut ui, Key::Enter);
    assert_eq!(shown(&ui), before + 1, "Enter on Show code did not show the code");

    // A nav item switches the page.
    focus(&mut ui, NORMAL, |w| w.kind == "nav_item" && w.label == "Popups & overlays");
    activate(&mut ui, Key::Enter);
    assert!(has_label(&ui, "File"), "Enter on a nav item did not open its page");

    // The sidebar button opens the command palette.
    focus(&mut ui, NORMAL, |w| w.kind == "button" && w.label == "Commands");
    activate(&mut ui, Key::Space);
    assert!(ui.command_palette_open(), "Space on the palette button did not activate it");
    activate(&mut ui, Key::Escape);
    assert!(!ui.command_palette_open(), "Escape did not close the command palette");
}

#[test]
fn checkbox_toggle_radio_and_button_respond_to_keys() {
    let mut ui = Ui::new(UiOptions { inspection: true, ..Default::default() });
    let (mut check, mut on, mut radio, mut clicks) = (false, false, 0u8, 0);
    let mut run = |ui: &mut Ui| {
        ui.begin_frame(NORMAL).unwrap();
        ui.central_panel(|ui| {
            clicks += ui.button("Button").clicked as u32;
            ui.checkbox("Check", &mut check);
            ui.toggle("Toggle", &mut on);
            ui.radio("A", &mut radio, 0);
            ui.radio("B", &mut radio, 1);
        });
        ui.end_frame();
        (clicks, check, on, radio)
    };
    run(&mut ui);
    let mut state = Vec::new();
    for key in [Key::Enter, Key::Space] {
        for _ in 0..5 {
            press(&mut ui, Key::Tab, Modifiers::default());
            run(&mut ui);
            press(&mut ui, key, Modifiers::default());
            state.push(run(&mut ui));
        }
    }
    assert_eq!(
        state,
        [
            (1, false, false, 0),
            (1, true, false, 0),
            (1, true, true, 0),
            (1, true, true, 0),
            (1, true, true, 1),
            (2, true, true, 1),
            (2, false, true, 1),
            (2, false, false, 1),
            (2, false, false, 0),
            (2, false, false, 1),
        ]
    );
}

#[test]
fn escape_closes_popups_and_dialogs() {
    // Modal dialog.
    let mut ui = open_page(DemoPage::Popups, NORMAL);
    focus(&mut ui, NORMAL, |w| w.kind == "button" && w.label == "Open dialog");
    activate(&mut ui, Key::Enter);
    assert!(has_label(&ui, "Keep a backup"), "Enter did not open the dialog");
    // While the dialog is open, Tab stays inside it.
    let inside = cycle(&mut ui, NORMAL, Modifiers::default(), 64);
    let names: Vec<String> = inside.iter().map(|id| name(&ui, *id)).collect();
    assert!(names.iter().all(|n| !n.starts_with("nav_item")), "Tab left the modal dialog: {names:?}");
    activate(&mut ui, Key::Escape);
    assert!(!has_label(&ui, "Keep a backup"), "Escape did not close the dialog");

    // Menu bar menu.
    focus(&mut ui, NORMAL, |w| w.kind == "menu" && w.label == "File");
    activate(&mut ui, Key::Enter);
    assert!(has_label(&ui, "New document"), "Enter did not open the menu");
    activate(&mut ui, Key::Escape);
    assert!(!has_label(&ui, "New document"), "Escape did not close the menu");

    // Combo box popup.
    let mut ui = open_page(DemoPage::Inputs, NORMAL);
    focus(&mut ui, NORMAL, |w| w.kind == "combo_box");
    let list_open = |ui: &Ui| ui.output().widgets.iter().any(|w| w.kind == "selectable" && w.label == "Success");
    assert!(!list_open(&ui));
    activate(&mut ui, Key::Enter);
    assert!(list_open(&ui), "Enter did not open the combo box");
    activate(&mut ui, Key::Escape);
    assert!(!list_open(&ui), "Escape did not close the combo box");
}
