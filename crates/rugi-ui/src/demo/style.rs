//! A live editor for the active theme.

use super::two_columns;
use crate::{ButtonKind, Color, FontFamily, Theme, Ui};

pub(super) const SOURCE: &str = include_str!("style.rs");

pub(super) struct State {
    text_scale: i32,
}

impl Default for State {
    fn default() -> Self {
        Self { text_scale: 100 }
    }
}

/// The theme being edited this frame; applied when `changed`.
struct Edit {
    theme: Theme,
    changed: bool,
    text_scale: i32,
}

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    let mut edit = Edit { theme: ui.theme().clone(), changed: false, text_scale: app.text_scale };
    two_columns(ui, &mut edit, left, right);
    app.text_scale = edit.text_scale;
    if edit.changed {
        ui.set_theme(edit.theme);
    }
}

fn left(ui: &mut Ui, edit: &mut Edit) {
    demo_section!(ui, "Colors", "Semantic colors of the active theme. Click a swatch to edit it.", |ui| {
        let mut accent = edit.theme.palette.accent;
        if ui.color_edit("Accent", &mut accent).changed {
            // Also derives the accent's hover and text variants.
            edit.theme.set_accent(accent);
            edit.changed = true;
        }
        let p = &mut edit.theme.palette;
        let roles: [(&str, &mut Color); 10] = [
            ("Background", &mut p.background),
            ("Panel", &mut p.panel),
            ("Surface", &mut p.surface),
            ("Sunken", &mut p.sunken),
            ("Border", &mut p.border),
            ("Text", &mut p.text),
            ("Muted text", &mut p.text_muted),
            ("Success", &mut p.success),
            ("Warning", &mut p.warning),
            ("Danger", &mut p.danger),
        ];
        for (name, color) in roles {
            edit.changed |= ui.color_edit(name, color).changed;
        }
    });
    demo_section!(ui, "Export", "Every token edited on this page, as Rust code.", |ui| {
        ui.code_block(&theme_code(&edit.theme, edit.text_scale));
        if ui.button_kind("Reset theme", ButtonKind::Secondary).clicked {
            edit.theme = if edit.theme.dark { Theme::dark() } else { Theme::light() };
            edit.text_scale = 100;
            edit.changed = true;
        }
    });
}

fn right(ui: &mut Ui, edit: &mut Edit) {
    demo_section!(ui, "Shape and spacing", "Metrics are in logical pixels.", |ui| {
        let m = &mut edit.theme.metrics;
        let metrics: [(&str, &mut f32, i32, i32); 6] = [
            ("Control radius (px)", &mut m.radius_md, 0, 18),
            ("Card radius (px)", &mut m.radius_lg, 0, 28),
            ("Control height (px)", &mut m.control_height, 20, 46),
            ("Spacing (px)", &mut m.spacing, 2, 20),
            ("Card padding (px)", &mut m.card_padding, 8, 32),
            ("Label column (px)", &mut m.label_width, 60, 240),
        ];
        for (label, value, min, max) in metrics {
            let mut px = value.round() as i32;
            if ui.slider_int(label, &mut px, min, max).changed {
                *value = px as f32;
                edit.changed = true;
            }
        }
    });
    demo_section!(ui, "Text and motion", "", |ui| {
        let t = &mut edit.theme;
        let scale = &mut edit.text_scale;
        if ui.slider_int("Text size (%)", scale, 80, 140).changed {
            let base = if t.dark { Theme::dark() } else { Theme::light() };
            t.typography = base.typography;
            t.scale_text(*scale as f32 / 100.0);
            edit.changed = true;
        }
        let mut ms = (t.motion.hover * 1000.0).round() as i32;
        if ui.slider_int("Hover fade (ms)", &mut ms, 0, 300).changed {
            t.motion.hover = ms as f32 / 1000.0;
            edit.changed = true;
        }
        let mut mono = t.typography.body.family == FontFamily::Mono;
        if ui.toggle("Monospace everywhere", &mut mono).changed {
            use FontFamily::{Mono, Sans};
            t.set_font_family(if mono { Mono } else { Sans });
            edit.changed = true;
        }
    });
}

/// Rust code that rebuilds the main tokens of `theme`.
/// Rust code that rebuilds every token this page edits.
fn theme_code(theme: &Theme, text_scale: i32) -> String {
    use std::fmt::Write as _;
    let (p, m) = (&theme.palette, &theme.metrics);
    let hex = |c: Color| format!("0x{:02X}{:02X}{:02X}", c.r, c.g, c.b);
    let base = if theme.dark { "dark" } else { "light" };
    let mut code = format!("let mut t = Theme::{base}();\nt.set_accent(Color::hex({}));\n", hex(p.accent));
    let colors = [
        ("background", p.background),
        ("panel", p.panel),
        ("surface", p.surface),
        ("sunken", p.sunken),
        ("border", p.border),
        ("text", p.text),
        ("text_muted", p.text_muted),
        ("success", p.success),
        ("warning", p.warning),
        ("danger", p.danger),
    ];
    for (name, c) in colors {
        let _ = writeln!(code, "t.palette.{name} = Color::hex({});", hex(c));
    }
    let metrics = [
        ("radius_md", m.radius_md),
        ("radius_lg", m.radius_lg),
        ("control_height", m.control_height),
        ("spacing", m.spacing),
        ("card_padding", m.card_padding),
        ("label_width", m.label_width),
    ];
    for (name, v) in metrics {
        let _ = writeln!(code, "t.metrics.{name} = {v:.0}.0;");
    }
    if text_scale != 100 {
        let _ = writeln!(code, "t.scale_text({:.2});", text_scale as f32 / 100.0);
    }
    let _ = writeln!(code, "t.motion.hover = {:.3};", theme.motion.hover);
    code.push_str("ui.set_theme(t);");
    code
}
