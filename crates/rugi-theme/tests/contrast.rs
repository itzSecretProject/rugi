//! WCAG 2.2 contrast of every built-in theme.
//!
//! Text roles need 4.5:1 (success criterion 1.4.3, normal-size text) against
//! every surface they are drawn on; component boundaries and focus indicators
//! need 3:1 (1.4.11). Run with `--nocapture` to print the measured ratios.

use rugi_core::Color;
use rugi_theme::Theme;

const TEXT: f64 = 4.5;
const NON_TEXT: f64 = 3.0;

fn channel(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Color) -> f64 {
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// `fg` composited over the opaque `bg`.
fn over(fg: Color, bg: Color) -> Color {
    let a = fg.a as f64 / 255.0;
    let mix = |f: u8, b: u8| (f as f64 * a + b as f64 * (1.0 - a)).round() as u8;
    Color::rgb(mix(fg.r, bg.r), mix(fg.g, bg.g), mix(fg.b, bg.b))
}

fn ratio(fg: Color, bg: Color) -> f64 {
    let (a, b) = (luminance(over(fg, bg)) + 0.05, luminance(bg) + 0.05);
    a.max(b) / a.min(b)
}

/// Every (foreground, background, minimum) pair the widgets draw.
fn pairs(t: &Theme) -> Vec<(String, Color, Color, f64)> {
    let p = &t.palette;
    let mut v = Vec::new();
    let surfaces = [
        ("background", p.background),
        ("panel", p.panel),
        ("surface", p.surface),
        ("hover", p.surface_hover),
        ("sunken", p.sunken),
    ];
    for (sname, s) in surfaces {
        for (tname, c) in [
            ("text", p.text),
            ("text_muted", p.text_muted),
            ("text_faint", p.text_faint),
            ("accent_text", p.accent_text),
            ("success", p.success),
            ("warning", p.warning),
            ("danger", p.danger),
        ] {
            v.push((format!("{tname} on {sname}"), c, s, TEXT));
        }
        v.push((format!("accent indicator / focus ring on {sname}"), p.accent, s, NON_TEXT));
        v.push((format!("control_border on {sname}"), p.control_border, s, NON_TEXT));
    }
    for (sname, s) in [("surface", p.surface), ("panel", p.panel)] {
        let soft = over(p.accent_soft, s);
        v.push((format!("accent_text on accent_soft over {sname}"), p.accent_text, soft, TEXT));
        v.push((format!("text on accent_soft over {sname}"), p.text, soft, TEXT));
    }
    // Badges draw their text on a 14% tint of the same color.
    for (name, c) in
        [("accent_text", p.accent_text), ("success", p.success), ("warning", p.warning), ("danger", p.danger)]
    {
        v.push((format!("{name} badge"), c, over(c.with_alpha(0.14), p.surface), TEXT));
    }
    // Solid buttons: rest, hover (lighten 8%) and pressed (darken 12%).
    for (name, fill, label) in [("accent", p.accent, p.on_accent), ("danger", p.danger, p.on_danger)] {
        for (state, f) in [("rest", fill), ("hover", fill.lighten(0.08)), ("pressed", fill.darken(0.12))] {
            v.push((format!("{name} button label, {state}"), label, f, TEXT));
        }
    }
    v
}

#[test]
fn built_in_themes_meet_wcag_aa() {
    let mut failures = Vec::new();
    for t in Theme::presets() {
        println!("\n{}", t.name);
        for (name, fg, bg, min) in pairs(&t) {
            let r = ratio(fg, bg);
            let mark = if r + 1e-9 >= min { "  " } else { "!!" };
            println!("  {mark} {r:5.2}  (min {min})  {name}");
            if r + 1e-9 < min {
                failures.push(format!("{}: {name} = {r:.2} < {min}", t.name));
            }
        }
    }
    assert!(failures.is_empty(), "{} contrast failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn ratio_matches_reference_values() {
    assert!((ratio(Color::BLACK, Color::WHITE) - 21.0).abs() < 1e-9);
    assert!((ratio(Color::WHITE, Color::WHITE) - 1.0).abs() < 1e-9);
    // #767676 on white is the well-known 4.54:1 grey.
    assert!((ratio(Color::hex(0x767676), Color::WHITE) - 4.54).abs() < 0.01);
}
