//! Buttons, toggles, sliders, numbers, progress and text.

use super::two_columns;
use crate::{lucide, ButtonKind, FontFamily, FontWeight, Rect, TextRole, Ui, Vec2};

pub(super) const SOURCE: &str = include_str!("basics.rs");

const BUTTON_KINDS: [(&str, ButtonKind); 4] = [
    ("Primary", ButtonKind::Primary),
    ("Secondary", ButtonKind::Secondary),
    ("Ghost", ButtonKind::Ghost),
    ("Danger", ButtonKind::Danger),
];

/// Seconds the progress bar takes to fill.
const FILL_SECONDS: f32 = 4.0;

pub(super) struct State {
    last_button: &'static str,
    bold: bool,
    mono: bool,
    muted: bool,
    role: u8,
    opacity: f32,
    segments: i32,
    dots: i32,
    radius: f32,
    progress: f32,
    running: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_button: "none",
            bold: false,
            mono: false,
            muted: false,
            role: 1,
            opacity: 0.8,
            segments: 6,
            dots: 8,
            radius: 5.0,
            progress: 0.0,
            running: false,
        }
    }
}

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    two_columns(ui, app, left, right);
}

fn left(ui: &mut Ui, app: &mut State) {
    demo_section!(ui, "Buttons", "A button returns a Response; .clicked is true on the frame it is activated.", |ui| {
        ui.horizontal(|ui| {
            for (label, kind) in BUTTON_KINDS {
                if ui.button_kind(label, kind).clicked {
                    app.last_button = label;
                }
            }
            let upload = Some(lucide::UPLOAD.into());
            if ui.button_ex("Upload", ButtonKind::Secondary, upload).clicked {
                app.last_button = "Upload";
            }
            // Text after "##" is only part of the id: no visible label.
            let star = Some(lucide::STAR.into());
            if ui.button_ex("##star", ButtonKind::Ghost, star).clicked {
                app.last_button = "Star";
            }
            ui.tooltip("Star");
        });
        ui.label_fmt(format_args!("Last clicked: {}", app.last_button));
    });
    demo_section!(ui, "Checkboxes, switches, radio buttons", "They style the sample text below.", |ui| {
        ui.horizontal(|ui| {
            ui.checkbox("Bold", &mut app.bold);
            ui.checkbox("Monospace", &mut app.mono);
            ui.toggle("Muted color", &mut app.muted);
        });
        ui.horizontal(|ui| {
            ui.radio("Caption", &mut app.role, 0);
            ui.radio("Body", &mut app.role, 1);
            ui.radio("Heading", &mut app.role, 2);
        });
        let t = &ui.theme().typography;
        let mut style = [t.caption, t.body, t.heading][app.role as usize];
        if app.bold {
            style = style.weight(FontWeight(700));
        }
        if app.mono {
            style.family = FontFamily::Mono;
        }
        let p = &ui.theme().palette;
        let color = if app.muted { p.text_muted } else { p.text };
        ui.text_styled("The quick brown fox jumps over the lazy dog", style, color, false);
    });
    demo_section!(
        ui,
        "Text",
        "Text roles use the theme's typography; badges take any color; label_truncated shortens text to a width.",
        |ui| {
            ui.title("Title");
            ui.heading("Heading");
            ui.label("Body");
            ui.muted("Muted");
            ui.caption("Caption");
            ui.bullet_text("Bullet");
            ui.horizontal(|ui| {
                let p = ui.theme().palette.clone();
                ui.badge("Saved", p.success);
                ui.badge("Pending", p.warning);
                ui.badge("Failed", p.danger);
            });
            ui.paragraph("Paragraphs wrap to the available width. Text shaping handles Ünïcödé and “quotes”.");
            let path = "crates/rugi-ui/src/widgets/basic.rs";
            ui.label_truncated(path, TextRole::Muted, Some(160.0));
        }
    );
}

fn right(ui: &mut Ui, app: &mut State) {
    demo_section!(ui, "Sliders", "Drag, click the track, or use the arrow keys (Shift: larger steps).", |ui| {
        ui.slider("Opacity", &mut app.opacity, 0.0, 1.0);
        ui.slider_int("Segments", &mut app.segments, 1, 12);
        // Custom painting into an allocated rectangle.
        let r = ui.allocate(Vec2::new(0.0, 24.0), true);
        let (n, gap) = (app.segments as f32, 4.0);
        let w = (r.width() - gap * (n - 1.0)) / n;
        let color = ui.theme().palette.accent.fade(app.opacity);
        for i in 0..app.segments {
            let x = r.min.x + i as f32 * (w + gap);
            let cell = Rect::from_xywh(x, r.min.y, w, r.height());
            ui.painter().rect(cell, 3.0, color, None);
        }
    });
    demo_section!(ui, "Spin boxes", "Click − and +, drag sideways, or use the arrow keys.", |ui| {
        ui.labeled("Dots", |ui| ui.spin_box_int("##dots", &mut app.dots, 1, 20, 1));
        ui.labeled("Radius (px)", |ui| ui.spin_box("##radius", &mut app.radius, 2.0, 10.0, 0.5));
        let r = ui.allocate(Vec2::new(0.0, 24.0), true);
        let color = ui.theme().palette.accent;
        for i in 0..app.dots {
            let x = r.min.x + app.radius + i as f32 * (app.radius * 2.0 + 6.0);
            let center = Vec2::new(x, r.center().y);
            ui.painter().circle(center, app.radius, color, None);
        }
    });
    demo_section!(ui, "Progress", "The bar fills in 4 seconds. Animations request frames only while they run.", |ui| {
        if app.running {
            app.progress = (app.progress + ui.dt() / FILL_SECONDS).min(1.0);
            app.running = app.progress < 1.0;
            ui.request_repaint();
        }
        ui.progress_bar(app.progress);
        ui.horizontal_centered(|ui| {
            // "###run" keeps one id, and so the focus, across labels.
            let label = match (app.running, app.progress >= 1.0) {
                (true, _) => "Pause###run",
                (false, true) => "Restart###run",
                (false, false) => "Start###run",
            };
            if ui.button_kind(label, ButtonKind::Secondary).clicked {
                if app.progress >= 1.0 {
                    app.progress = 0.0;
                }
                app.running = !app.running;
            }
            ui.label_fmt(format_args!("{:.0}%", app.progress * 100.0));
        });
    });
}
