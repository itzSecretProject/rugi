//! Typography and the bundled Lucide icon set.

use super::two_columns;
use crate::{lucide, ButtonKind, FontFamily, Ui};
use std::fmt;

pub(super) const SOURCE: &str = include_str!("fonts.rs");

#[derive(Default)]
pub(super) struct State {
    query: String,
    /// Indices into `lucide::ALL` that match `query`, and the query they match.
    matches: Vec<usize>,
    matched: Option<String>,
    selected: Option<usize>,
}

/// An icon name as its Rust constant: `arrow-up` → `ARROW_UP`.
struct ConstName<'a>(&'a str);

impl fmt::Display for ConstName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            let c = if c == '-' { '_' } else { c.to_ascii_uppercase() };
            fmt::Write::write_char(f, c)?;
        }
        Ok(())
    }
}

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    two_columns(ui, app, left, right);
}

fn left(ui: &mut Ui, _app: &mut State) {
    demo_section!(
        ui,
        "Type specimen",
        "The text styles of the active theme. Inter and JetBrains Mono are bundled; add fonts with ui.add_font(bytes) or ui.add_font_file(path).",
        |ui| {
            let t = ui.theme().typography.clone();
            let color = ui.theme().palette.text;
            let roles = [
                ("display", t.display),
                ("title", t.title),
                ("heading", t.heading),
                ("label", t.label),
                ("body", t.body),
                ("caption", t.caption),
            ];
            for (role, style) in roles {
                let text = format!("{role}: {} px, weight {}", style.size, style.weight.0);
                ui.text_styled(&text, style, color, false);
            }
            let mut mono = t.body;
            mono.family = FontFamily::Mono;
            ui.text_styled("body, monospace: let x = 0O1lI; // => != ==", mono, color, false);
        }
    );
    demo_section!(ui, "Icons in text", "Icon glyphs mix with text and take its size and color.", |ui| {
        ui.heading(&format!("{}  Heading", lucide::FOLDER));
        ui.label(&format!("{}  Body", lucide::CIRCLE_CHECK));
        ui.muted(&format!("{}  Muted", lucide::INFO));
        ui.caption(&format!("{}  Caption", lucide::CLOCK));
    });
}

fn right(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Icon browser",
        "Every bundled Lucide icon. Select a row, then press Copy to put its constant on the clipboard.",
        |ui| {
            ui.text_input("##query", &mut app.query, "Filter by name");
            let query = app.query.trim().to_lowercase();
            if app.matched.as_deref() != Some(query.as_str()) {
                let all = lucide::ALL.iter().enumerate();
                let hits = all.filter(|(_, (name, _))| name.contains(&query));
                app.matches = hits.map(|(i, _)| i).collect();
                app.matched = Some(query);
                app.selected = None;
            }
            let muted = ui.theme().palette.text_muted;
            let matches = &app.matches;
            let cell = |ui: &mut Ui, row: usize, col: usize| {
                let (name, glyph) = lucide::ALL[matches[row]];
                match col {
                    0 => ui.icon_ex(glyph.into(), 18.0, muted),
                    1 => ui.label(name),
                    _ => ui.label_fmt(format_args!("{}", ConstName(name))),
                };
            };
            let headers = ["Icon", "Name", "Constant"];
            let widths = [0.6, 2.0, 2.4];
            let rows = matches.len();
            ui.table("##icons", &headers, &widths, rows, &mut app.selected, 300.0, cell);
            let picked = app.selected.map(|i| lucide::ALL[app.matches[i]].0);
            ui.horizontal_centered(|ui| match picked {
                Some(name) => {
                    let path = format!("lucide::{}", ConstName(name));
                    if ui.button_kind("Copy", ButtonKind::Secondary).clicked {
                        ui.set_clipboard(&path);
                    }
                    ui.label(&path);
                }
                None => {
                    ui.muted(&format!("{rows} icons"));
                }
            });
        }
    );
}
