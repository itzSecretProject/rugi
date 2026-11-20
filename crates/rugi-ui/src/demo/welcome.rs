//! What a Rugi program looks like, how state and ids work, and the keyboard.

use super::shell::{INSPECTOR_SHORTCUT, PALETTE_SHORTCUT};
use super::two_columns;
use crate::{ButtonKind, Ui};

pub(super) const SOURCE: &str = include_str!("welcome.rs");

const SHORTCUTS: [(&str, &str); 8] = [
    ("Tab", "Focus the next control"),
    ("Shift Tab", "Focus the previous control"),
    ("Enter / Space", "Activate the focused control"),
    ("← → ↑ ↓", "Change sliders, tabs, lists and combo boxes"),
    ("Esc", "Close a popup or dialog"),
    ("Shift F10", "Open the context menu"),
    (PALETTE_SHORTCUT, "Command palette"),
    (INSPECTOR_SHORTCUT, "Inspector"),
];

pub(super) struct State {
    name: String,
    volume: f32,
    clicks: u32,
    rows: i32,
    show_details: bool,
    items: Vec<&'static str>,
}

impl Default for State {
    fn default() -> Self {
        Self { name: String::new(), volume: 0.5, clicks: 0, rows: 3, show_details: true, items: ITEMS.to_vec() }
    }
}

const ITEMS: [&str; 3] = ["Report.pdf", "Notes.txt", "Photo.png"];

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    two_columns(ui, app, left, right);
}

fn left(ui: &mut Ui, app: &mut State) {
    hello(ui, app);
    demo_section!(ui, "Your data drives the UI", "Change the data and the next frame shows it.", |ui| {
        ui.slider_int("Rows", &mut app.rows, 0, 6);
        ui.checkbox("Show details", &mut app.show_details);
        for i in 1..=app.rows {
            ui.horizontal(|ui| {
                ui.label_fmt(format_args!("Row {i}"));
                if app.show_details {
                    ui.muted("details");
                }
            });
        }
    });
}

/// The first section, with the same variables as `hello.rs` so that its code
/// reads exactly like the example.
fn hello(ui: &mut Ui, app: &mut State) {
    let mut name = std::mem::take(&mut app.name);
    let (mut volume, mut clicks) = (app.volume, app.clicks);
    let note = "The body of ui.central_panel in crates/rugi/examples/hello.rs. \
                Each section of this demo runs the code that its Show code button reveals.";
    demo_section!(ui, "A first program", note, code_open, |ui| {
        ui.title("Hello, Rugi");
        ui.text_input("name", &mut name, "Your name");
        ui.slider("Volume", &mut volume, 0.0, 1.0);
        if ui.button("Click me").clicked {
            clicks += 1;
        }
        ui.label(&format!("Clicked {clicks} times"));
    });
    app.name = name;
    (app.volume, app.clicks) = (volume, clicks);
}

fn right(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Ids",
        "Widgets keep their state by id, made from the label. push_id gives repeated labels their own scope.",
        |ui| {
            let mut removed = None;
            for (i, name) in app.items.iter().enumerate() {
                ui.push_id(name);
                ui.horizontal_centered(|ui| {
                    if ui.button_kind("Remove", ButtonKind::Secondary).clicked {
                        removed = Some(i);
                    }
                    ui.label(name);
                });
                ui.pop_id();
            }
            if let Some(i) = removed {
                app.items.remove(i);
            }
            if app.items.len() < ITEMS.len() && ui.button("Restore").clicked {
                app.items = ITEMS.to_vec();
            }
        }
    );
    demo_section!(ui, "Keyboard", "Every control can be reached and used without a mouse.", |ui| {
        for (keys, action) in SHORTCUTS {
            ui.horizontal_centered(|ui| {
                ui.begin_vertical_width(110.0);
                ui.kbd(keys);
                ui.end();
                ui.muted(action);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::super::block_source;
    use super::*;

    /// Line (1-based) of the `demo_section!` call in `hello`. The call is
    /// indented, so the text before it ends with a partial line.
    fn hello_section_line() -> u32 {
        let pos = SOURCE.find("demo_section!(ui, \"A first program\"").expect("the first section exists");
        SOURCE[..pos].lines().count() as u32
    }

    #[test]
    fn first_program_matches_hello_example() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("rugi/examples/hello.rs");
        let hello = std::fs::read_to_string(&path).expect("hello.rs is readable");
        let start = hello.find("ui.central_panel(|ui| {").expect("hello.rs calls ui.central_panel");
        // The call is indented, so this counts its own line too; `block_source`
        // takes the block after the first `| {` from there.
        let line = hello[..start].lines().count() as u32;
        let example = block_source(&hello, line);
        assert!(!example.is_empty());
        assert_eq!(block_source(SOURCE, hello_section_line()), example, "the demo's snippet differs from hello.rs");
    }
}
