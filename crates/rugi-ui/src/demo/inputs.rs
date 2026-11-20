//! Text fields, combo boxes, lists, tabs and other selection widgets.

use super::two_columns;
use crate::{lucide, FontFamily, FontWeight, Key, KeyBind, Rect, TextInputOptions, TextStyle, Ui, Vec2};

pub(super) const SOURCE: &str = include_str!("inputs.rs");

const COLORS: [&str; 4] = ["Accent", "Success", "Warning", "Danger"];
const SHAPES: [&str; 4] = ["Circle", "Square", "Rounded square", "Pill"];
const WEIGHTS: [&str; 3] = ["Regular", "Medium", "Semibold"];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
/// Days in each month of 2026.
const DAYS_2026: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const SAMPLE: &str = "Sphinx of black quartz, judge my vow";

pub(super) struct State {
    name: String,
    password: String,
    todo: String,
    todos: Vec<String>,
    color: usize,
    shapes: [bool; 4],
    month: usize,
    tab: usize,
    sample: String,
    size: i32,
    weight: usize,
    mono: bool,
    reset: KeyBind,
}

impl Default for State {
    fn default() -> Self {
        Self {
            name: String::new(),
            password: String::new(),
            todo: String::new(),
            todos: vec!["Write the changelog".into()],
            color: 0,
            shapes: [true, true, false, true],
            month: 0,
            tab: 0,
            sample: SAMPLE.into(),
            size: 18,
            weight: 0,
            mono: false,
            reset: KeyBind::key(Key::F5),
        }
    }
}

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    two_columns(ui, app, left, right);
}

fn left(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Text input",
        "Click or drag to select; Alt or Ctrl with the arrow keys jumps by word. Copy and paste use the system clipboard.",
        |ui| {
            ui.labeled("Name", |ui| ui.text_input("##name", &mut app.name, "Your name"));
            ui.labeled("Password", |ui| {
                let opts = TextInputOptions { password: true, icon: Some(lucide::LOCK.into()) };
                ui.text_input_ex("##password", &mut app.password, "At least 8 characters", opts)
            });
            let n = app.password.chars().count();
            let verdict = if n >= 8 { "long enough" } else { "too short" };
            ui.muted(&format!("Password: {n} characters, {verdict}"));
        }
    );
    demo_section!(ui, "Submit with Enter", "Check a task to remove it.", |ui| {
        let r = ui.text_input("##todo", &mut app.todo, "Add a task and press Enter");
        if r.submitted && !app.todo.trim().is_empty() {
            app.todos.push(std::mem::take(&mut app.todo));
        }
        let mut done = None;
        for (i, todo) in app.todos.iter().enumerate() {
            ui.push_id_u64(i as u64);
            if ui.checkbox(todo, &mut false).changed {
                done = Some(i);
            }
            ui.pop_id();
        }
        if let Some(i) = done {
            app.todos.remove(i);
        }
        if app.todos.is_empty() {
            ui.muted("No tasks.");
        }
    });
    demo_section!(ui, "Combo box and multi-select", "Up and Down change a focused combo box.", |ui| {
        ui.labeled("Color", |ui| ui.combo_box("##color", &mut app.color, &COLORS));
        ui.labeled("Shapes", |ui| ui.multi_select("##shapes", &mut app.shapes, &SHAPES));
        let p = &ui.theme().palette;
        let color = [p.accent, p.success, p.warning, p.danger][app.color];
        let r = ui.allocate(Vec2::new(0.0, 24.0), true);
        let mut x = r.min.x;
        for (shape, on) in app.shapes.iter().enumerate() {
            if !on {
                continue;
            }
            let (width, radius) = [(24.0, 12.0), (24.0, 0.0), (24.0, 6.0), (48.0, 12.0)][shape];
            ui.painter().rect(Rect::from_xywh(x, r.min.y, width, 24.0), radius, color, None);
            x += width + 8.0;
        }
    });
}

fn right(ui: &mut Ui, app: &mut State) {
    demo_section!(ui, "Tabs", "Left and Right switch tabs while the tab bar has focus.", |ui| {
        ui.tabs("##preview", &mut app.tab, &["Text", "Style", "Shortcut"]);
        match app.tab {
            0 => {
                ui.labeled("Sample", |ui| ui.text_input("##sample", &mut app.sample, "Sample text"));
                ui.slider_int("Size (px)", &mut app.size, 10, 32);
            }
            1 => {
                ui.labeled("Weight", |ui| ui.segmented("##weight", &mut app.weight, &WEIGHTS));
                ui.toggle("Monospace", &mut app.mono);
            }
            _ => {
                ui.keybind("Reset the preview", &mut app.reset);
                ui.muted("Click the binding, then press a key.");
            }
        }
        if ui.bind_active(&mut app.reset) {
            (app.sample, app.size, app.weight, app.mono) = (SAMPLE.into(), 18, 0, false);
        }
        let weight = FontWeight([400, 500, 600][app.weight]);
        let mut style = TextStyle::new(app.size as f32).weight(weight);
        if app.mono {
            style.family = FontFamily::Mono;
        }
        let color = ui.theme().palette.text;
        ui.text_styled(&app.sample, style, color, false);
    });
    demo_section!(ui, "List box", "A scrolling list of selectable rows.", |ui| {
        ui.list_box("##months", &mut app.month, &MONTHS, 150.0);
        ui.label_fmt(format_args!("{} 2026 has {} days", MONTHS[app.month], DAYS_2026[app.month]));
    });
}
