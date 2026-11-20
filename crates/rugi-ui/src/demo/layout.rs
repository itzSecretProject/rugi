//! Rows, columns, cards, scrolling, collapsing sections and trees.

use super::two_columns;
use crate::{ButtonKind, Ui, Vec2};

pub(super) const SOURCE: &str = include_str!("layout.rs");

/// Crates of the workspace, as shown in the tree.
const CRATES: [&str; 9] =
    ["rugi", "rugi-core", "rugi-ffi", "rugi-icons", "rugi-render", "rugi-text", "rugi-theme", "rugi-ui", "rugi-winit"];
/// Most items in the row of the "Rows" section.
const MAX_ITEMS: u32 = 6;

pub(super) struct State {
    items: u32,
    width: i32,
    text: String,
    checked: bool,
    columns: i32,
    lines: i32,
    box_w: i32,
    box_h: i32,
    radius: i32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            items: 3,
            width: 200,
            text: String::new(),
            checked: true,
            columns: 3,
            lines: 40,
            box_w: 160,
            box_h: 40,
            radius: 6,
        }
    }
}

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    two_columns(ui, app, left, right);
}

fn left(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Rows",
        "horizontal flows left to right; horizontal_right flows right to left from the far edge.",
        |ui| {
            ui.horizontal(|ui| {
                for i in 1..=app.items {
                    ui.label_fmt(format_args!("Item {i}"));
                }
            });
            ui.horizontal_right(|ui| {
                if ui.button_kind("Add", ButtonKind::Secondary).clicked && app.items < MAX_ITEMS {
                    app.items += 1;
                }
                if ui.button_kind("Remove", ButtonKind::Secondary).clicked {
                    app.items = app.items.saturating_sub(1);
                }
            });
        }
    );
    demo_section!(
        ui,
        "Form rows",
        "labeled puts the label in a column of theme().metrics.label_width and the widget to its right.",
        |ui| {
            ui.labeled("Field width (px)", |ui| ui.spin_box_int("##width", &mut app.width, 60, 400, 20));
            ui.labeled("Text", |ui| {
                ui.set_next_item_width(app.width as f32);
                ui.text_input("##text", &mut app.text, "Resized by the spin box")
            });
        }
    );
    demo_section!(ui, "Centered rows", "horizontal_centered centers items of different heights.", |ui| {
        ui.horizontal_centered(|ui| {
            ui.heading("Heading");
            ui.caption("caption");
            ui.kbd("Esc");
            ui.checkbox("Checkbox", &mut app.checked);
        });
    });
    demo_section!(ui, "Columns", "Equal-width columns; items wrap to the next row.", |ui| {
        ui.slider_int("Columns", &mut app.columns, 1, 6);
        let n = app.columns as usize;
        ui.columns(app.columns as u32, |ui| {
            for i in 0..6 {
                ui.card(|ui| {
                    ui.label_fmt(format_args!("Cell {}", i + 1));
                    ui.muted(&format!("row {}, column {}", i / n + 1, i % n + 1));
                });
            }
        });
    });
}

fn right(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Scrolling",
        "The wheel scrolls the innermost area under the pointer. Tab scrolls focused widgets into view.",
        |ui| {
            ui.slider_int("Lines", &mut app.lines, 0, 200);
            ui.scroll_area("##squares", Some(160.0), |ui| {
                for i in 1..=app.lines {
                    ui.label_fmt(format_args!("{i}² = {}", i * i));
                }
            });
        }
    );
    demo_section!(ui, "Collapsing sections", "The height animates; the open state is kept per id.", |ui| {
        ui.collapsing("Size", true, |ui| {
            ui.slider_int("Width (px)", &mut app.box_w, 20, 240);
            ui.slider_int("Height (px)", &mut app.box_h, 10, 80);
        });
        ui.collapsing("Corners", false, |ui| {
            ui.slider_int("Radius (px)", &mut app.radius, 0, 40);
        });
        let r = ui.allocate(Vec2::new(app.box_w as f32, app.box_h as f32), false);
        let color = ui.theme().palette.accent;
        ui.painter().rect(r, app.radius as f32, color, None);
    });
    demo_section!(ui, "Trees", "Compact nodes with indented children.", |ui| {
        ui.tree_node("crates", true, |ui| {
            for name in CRATES {
                ui.tree_node(name, false, |ui| {
                    ui.bullet_text("Cargo.toml");
                    ui.bullet_text("src/lib.rs");
                });
            }
        });
    });
}
