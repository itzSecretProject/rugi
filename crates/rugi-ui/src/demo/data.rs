//! Line and bar plots, and a virtualized table.

use super::two_columns;
use crate::{lucide, TextRole, Ui};
use std::f32::consts::PI;
use std::fmt;

pub(super) const SOURCE: &str = include_str!("data.rs");

/// Rows of the table: one per code point from U+0000.
const ROWS: usize = 100_000;
/// The table starts at the space character, skipping the C0 control codes.
const FIRST: usize = 0x20;
/// Samples in the line plot.
const SAMPLES: usize = 120;
const FUNCTION_NAMES: [&str; 4] = ["sin(x)", "cos(x)", "sin(x) · sin(4x)", "sign(sin(x))"];
const FUNCTIONS: [fn(f32) -> f32; 4] = [f32::sin, f32::cos, sin_sin4, sign_sin];

fn sin_sin4(x: f32) -> f32 {
    x.sin() * (4.0 * x).sin()
}

fn sign_sin(x: f32) -> f32 {
    x.sin().signum()
}

pub(super) struct State {
    function: usize,
    animate: bool,
    phase: f32,
    samples: Vec<f32>,
    /// Lucide icons whose name starts with each letter, a to z.
    letters: [f32; 26],
    jump: String,
    row: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        let mut letters = [0.0; 26];
        for (name, _) in lucide::ALL {
            if let Some(i) = name.bytes().next().filter(u8::is_ascii_lowercase).map(|b| usize::from(b - b'a')) {
                letters[i] += 1.0;
            }
        }
        Self {
            function: 0,
            animate: false,
            phase: 0.0,
            samples: Vec::with_capacity(SAMPLES),
            letters,
            jump: String::new(),
            row: None,
        }
    }
}

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    two_columns(ui, app, line_plot, bar_chart);
    table(ui, app);
}

fn line_plot(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Line plot",
        "120 samples for x from 0 to 4π, with the y axis fixed to -1.2..1.2. Hover to read a sample.",
        |ui| {
            ui.labeled("Function", |ui| ui.combo_box("##function", &mut app.function, &FUNCTION_NAMES));
            if app.animate {
                app.phase += ui.dt();
                ui.request_repaint();
            }
            let f = FUNCTIONS[app.function];
            app.samples.clear();
            for i in 0..SAMPLES {
                let x = i as f32 / (SAMPLES - 1) as f32 * 4.0 * PI + app.phase;
                app.samples.push(f(x));
            }
            ui.plot_lines("##plot", &app.samples, Some((-1.2, 1.2)), 150.0);
            ui.toggle("Shift x by the elapsed time", &mut app.animate);
        }
    );
}

fn bar_chart(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Bar chart",
        "Lucide icons by the first letter of their name, a to z. Without a range the axis runs from 0 to a round number above the data.",
        |ui| {
            ui.plot_histogram("##letters", &app.letters, None, 150.0);
            let i = (0..26).max_by(|&a, &b| app.letters[a].total_cmp(&app.letters[b])).unwrap_or(0);
            let (letter, n) = (char::from(b'a' + i as u8), app.letters[i]);
            ui.label_fmt(format_args!("Most common: {letter}, {n} of {} icons", lucide::ALL.len()));
        }
    );
}

fn table(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Table",
        "100,000 Unicode code points from U+0020, classified with std's char methods. Only the visible rows call the cell closure; arrow keys move the selection.",
        |ui| {
            ui.labeled("Go to", |ui| {
                ui.set_next_item_width(200.0);
                let r = ui.text_input("##jump", &mut app.jump, "Code point in hex, then Enter");
                let digits = app.jump.trim().trim_start_matches(['U', 'u', '+']);
                match usize::from_str_radix(digits, 16) {
                    Ok(n) if !(FIRST..FIRST + ROWS).contains(&n) => {
                        ui.muted("Outside the table");
                    }
                    Ok(n) if r.submitted => app.row = Some(n - FIRST),
                    _ => {}
                }
            });
            let headers = ["Code point", "Character", "UTF-8", "Class"];
            let widths = [1.0, 0.8, 1.2, 1.6];
            let cell = |ui: &mut Ui, row: usize, col: usize| {
                let cp = FIRST + row;
                let c = char::from_u32(cp as u32);
                match (col, c) {
                    (0, _) => ui.label_fmt(format_args!("U+{cp:04X}")),
                    (1, Some(c)) if !c.is_control() => ui.label_fmt(format_args!("{c}")),
                    (2, Some(c)) => ui.label_fmt(format_args!("{}", Utf8Hex(c))),
                    (3, c) => ui.text(class(c), TextRole::Muted),
                    _ => ui.text("", TextRole::Muted),
                };
            };
            ui.table("##codepoints", &headers, &widths, ROWS, &mut app.row, 300.0, cell);
            match app.row {
                Some(r) => {
                    let cp = FIRST + r;
                    let c = char::from_u32(cp as u32);
                    ui.label_fmt(format_args!("Selected U+{cp:04X}, {}", class(c)))
                }
                None => ui.label("No row selected"),
            };
        }
    );
}

/// The classification of a code point by `char`'s `is_*` methods; `None`
/// is a surrogate, which is not a `char`.
fn class(c: Option<char>) -> &'static str {
    match c {
        None => "surrogate",
        Some(c) if c.is_control() => "control",
        Some(c) if c.is_whitespace() => "whitespace",
        Some(c) if c.is_numeric() => "numeric",
        Some(c) if c.is_uppercase() => "alphabetic, uppercase",
        Some(c) if c.is_lowercase() => "alphabetic, lowercase",
        Some(c) if c.is_alphabetic() => "alphabetic",
        Some(c) if c.is_ascii_punctuation() => "ASCII punctuation",
        Some(_) => "other",
    }
}

/// A character's UTF-8 bytes in hex: `é` → `C3 A9`.
struct Utf8Hex(char);

impl fmt::Display for Utf8Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0; 4];
        for (i, b) in self.0.encode_utf8(&mut buf).bytes().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{b:02X}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_point_columns() {
        assert_eq!(Utf8Hex('A').to_string(), "41");
        assert_eq!(Utf8Hex('é').to_string(), "C3 A9");
        assert_eq!(Utf8Hex('€').to_string(), "E2 82 AC");
        assert_eq!(class(char::from_u32(0xD800)), "surrogate");
        assert_eq!(class(Some('\n')), "control");
        assert_eq!(class(Some('7')), "numeric");
        assert_eq!(class(Some('Q')), "alphabetic, uppercase");
    }

    #[test]
    fn letter_counts_cover_every_icon() {
        let total: f32 = State::default().letters.iter().sum();
        assert_eq!(total as usize, lucide::ALL.len());
    }
}
