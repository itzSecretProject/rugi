//! The three pages, each a column of glass cards.

use crate::{BackdropSettings, Effects, PanelSettings, GLASS, SPEEDS, THEMES};
use rugi::{lucide, BackdropKind, ButtonKind, Color, GlassStyle, Ui};

const LICENSES: [(&str, &str); 4] = [
    ("Rugi", "MIT OR Apache-2.0"),
    ("Inter", "SIL Open Font License 1.1"),
    ("JetBrains Mono", "SIL Open Font License 1.1"),
    ("Lucide icons", "ISC"),
];

/// Cards inside the panel: a second, lighter layer of glass.
fn card_style() -> GlassStyle {
    let mut g = GlassStyle::frosted().with_radius(12.0).with_blur(0.35).with_tint(Color::rgba(255, 255, 255, 8));
    g.highlight = 0.12;
    g.grain = 0.0;
    g.border = Color::rgba(255, 255, 255, 16);
    g
}

impl Effects {
    pub(crate) fn backdrop_page(&mut self, ui: &mut Ui) {
        let card = card_style();
        let b = &mut self.backdrop;
        ui.glass_card(&card, |ui| {
            ui.separator_text("Backdrop");
            ui.segmented("backdrop", &mut b.kind, &BackdropKind::ALL.map(BackdropKind::name));
            ui.labeled("Variation", |ui| ui.spin_box_int("Variation", &mut b.variation, 0, 99, 1));
            ui.slider("Size", &mut b.size, 0.5, 2.0);
            ui.slider("Brightness", &mut b.brightness, 0.2, 1.5);
            ui.checkbox("Use the accent color", &mut b.use_accent);
        });
        ui.glass_card(&card, |ui| {
            ui.separator_text("Motion");
            ui.segmented("speed", &mut b.speed, &SPEEDS.map(|(name, _)| name));
            ui.slider("Follow the pointer", &mut b.pointer_response, 0.0, 1.0);
        });
        if ui.button_kind("Restore defaults", ButtonKind::Secondary).clicked {
            *b = BackdropSettings::default();
        }
    }

    pub(crate) fn panel_page(&mut self, ui: &mut Ui) {
        let card = card_style();
        let s = &mut self.panel;
        ui.glass_card(&card, |ui| {
            ui.separator_text("Theme");
            ui.segmented("theme", &mut s.theme, &THEMES.map(|(name, _)| name));
            ui.checkbox("Custom accent color", &mut s.custom_accent);
            if s.custom_accent {
                ui.color_edit("Accent", &mut s.accent);
            }
            ui.slider_int("Text size (%)", &mut s.text_size, 80, 130);
        });
        ui.glass_card(&card, |ui| {
            ui.separator_text("Glass");
            let mut glass = s.glass;
            if ui.segmented("glass", &mut glass, &GLASS.map(|(name, ..)| name)).changed {
                s.select_glass(glass);
            }
            ui.slider("Blur", &mut s.blur, 0.0, 1.0);
            let min = PanelSettings::min_tint(ui.theme());
            s.tint = s.tint.max(min);
            ui.slider("Tint", &mut s.tint, min, 1.0);
            if min > 0.0 {
                ui.caption(&format!("Light themes start at {min:.2} so dark text stays readable."));
            }
        });
        if ui.button_kind("Restore defaults", ButtonKind::Secondary).clicked {
            *s = PanelSettings::default();
        }
    }

    pub(crate) fn about_page(&mut self, ui: &mut Ui) {
        let card = card_style();
        let screen = ui.screen_rect();
        let details = [
            ("Version", format!("Rugi {}", env!("CARGO_PKG_VERSION"))),
            ("GPU", self.gpu.clone()),
            ("Window", format!("{:.0} × {:.0} at {}× scale", screen.width(), screen.height(), ui.scale())),
        ];
        ui.glass_card(&card, |ui| {
            ui.heading("Effects");
            ui.paragraph("An example of Rugi's glass panels, procedural backdrops and custom window chrome.");
            for (key, value) in &details {
                ui.labeled(key, |ui| ui.label(value));
            }
            if ui.button_ex("Copy details", ButtonKind::Secondary, Some(lucide::COPY.into())).clicked {
                let text: String = details.iter().map(|(k, v)| format!("{k}: {v}\n")).collect();
                ui.set_clipboard(&text);
            }
        });
        ui.glass_card(&card, |ui| {
            ui.separator_text("Licenses");
            for (name, license) in LICENSES {
                ui.labeled(name, |ui| ui.label(license));
            }
        });
    }
}
