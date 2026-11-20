//! Glass panels and animated backdrops: a glass panel in a borderless window,
//! over one of Rugi's procedural backdrops.
//!
//! The Backdrop page changes the backdrop behind the panel, the Panel page
//! the theme and the glass of the panel itself, and About lists the build,
//! the GPU and the licenses. Every control applies at once. Drag the window
//! by the panel's title bar.
//!
//! ```text
//! cargo run --release -p effects [-- --page panel --theme light --backdrop smoke --glass tinted]
//! cargo run --release -p effects -- --screenshot out.png [--click <label>]...
//! ```
//!
//! `--page`, `--theme`, `--backdrop` and `--glass` take a name from the UI
//! (case-insensitive). The screenshot flags are listed in
//! `examples/common/src/lib.rs`.

mod pages;

use rugi::{lucide, AppOptions, Backdrop, BackdropKind, Color, GlassStyle, InitContext, Rect, Slide, Theme, Ui, Vec2};
use rugi_example_shot as shot;

const PAGES: [(&str, &str); 3] = [(lucide::WALLPAPER, "Backdrop"), (lucide::PALETTE, "Panel"), (lucide::INFO, "About")];

type ThemeFn = fn() -> Theme;
const THEMES: [(&str, ThemeFn); 6] = [
    ("Dark", Theme::dark),
    ("Light", Theme::light),
    ("Rose", Theme::rose),
    ("Mocha", Theme::mocha),
    ("Forest", Theme::forest),
    ("Graphite", Theme::graphite),
];

/// Glass presets with the tint each one starts with (0..1, over the theme background).
const GLASS: [(&str, GlassStyle, f32); 3] = [
    ("Light blur", GlassStyle::liquid(), 0.3),
    ("Strong blur", GlassStyle::frosted(), 0.45),
    ("Tinted", GlassStyle::smoked(), 0.75),
];

/// Backdrop speeds, as speed multipliers.
const SPEEDS: [(&str, f32); 4] = [("Still", 0.0), ("Slow", 0.4), ("Normal", 1.0), ("Fast", 2.0)];

/// Lowest tint a light theme gets: its dark text needs a light surface over
/// the dark backdrop.
const LIGHT_THEME_MIN_TINT: f32 = 0.75;

const PANEL_SIZE: Vec2 = Vec2::new(760.0, 580.0);
const PANEL_MARGIN: f32 = 28.0;
const SIDEBAR_WIDTH: f32 = 170.0;
const TITLE_BAR_HEIGHT: f32 = 30.0;

/// The backdrop behind the panel.
struct BackdropSettings {
    kind: usize,
    variation: i32,
    size: f32,
    speed: usize,
    pointer_response: f32,
    use_accent: bool,
    brightness: f32,
}

impl Default for BackdropSettings {
    fn default() -> Self {
        Self {
            kind: BackdropKind::Aurora as usize,
            variation: 0,
            size: 1.0,
            speed: 2,
            pointer_response: 0.35,
            use_accent: false,
            brightness: 1.0,
        }
    }
}

/// The panel: theme and glass.
struct PanelSettings {
    theme: usize,
    custom_accent: bool,
    accent: Color,
    text_size: i32,
    glass: usize,
    blur: f32,
    tint: f32,
}

impl Default for PanelSettings {
    fn default() -> Self {
        let (_, style, tint) = GLASS[1];
        Self {
            theme: 0,
            custom_accent: false,
            accent: Color::hex(0x0A84FF),
            text_size: 100,
            glass: 1,
            blur: style.blur,
            tint,
        }
    }
}

/// The settings a theme is built from: theme, custom accent and text size.
type ThemeKey = (usize, Option<Color>, i32);

impl PanelSettings {
    fn theme_key(&self) -> ThemeKey {
        (self.theme, self.custom_accent.then_some(self.accent), self.text_size)
    }

    fn theme(&self) -> Theme {
        let mut theme = THEMES[self.theme].1();
        if self.custom_accent {
            theme.set_accent(self.accent);
        }
        theme.scale_text(self.text_size as f32 / 100.0);
        theme
    }

    /// The tint the panel is drawn with: the slider value, raised to the
    /// minimum of light themes.
    /// Lowest tint `theme` allows: light themes need a light panel under
    /// their dark text.
    fn min_tint(theme: &Theme) -> f32 {
        if theme.dark {
            0.0
        } else {
            LIGHT_THEME_MIN_TINT
        }
    }

    fn glass(&self, theme: &Theme) -> GlassStyle {
        let (_, style, _) = GLASS[self.glass];
        let tint = theme.palette.background.with_alpha(self.tint.max(Self::min_tint(theme)));
        style.with_blur(self.blur).with_tint(tint)
    }

    /// Switches to glass preset `i` and resets blur and tint to its values.
    fn select_glass(&mut self, i: usize) {
        let (_, style, tint) = GLASS[i];
        self.glass = i;
        self.blur = style.blur;
        self.tint = tint;
    }
}

#[derive(Default)]
struct Effects {
    page: usize,
    backdrop: BackdropSettings,
    panel: PanelSettings,
    /// What the current theme was built from.
    applied: Option<ThemeKey>,
    /// GPU adapter and graphics API, from the runner.
    gpu: String,
}

impl rugi::App for Effects {
    fn init(&mut self, ctx: &mut InitContext<'_>) {
        let info = ctx.adapter_info();
        self.gpu = format!("{} ({:?})", info.name, info.backend);
    }

    fn frame(&mut self, ui: &mut Ui) {
        let key = self.panel.theme_key();
        if self.applied != Some(key) {
            ui.set_theme(self.panel.theme());
            self.applied = Some(key);
        }
        ui.backdrop(&self.backdrop(ui.theme()));

        let screen = ui.screen_rect();
        let size = PANEL_SIZE.min(screen.size() - Vec2::new(PANEL_MARGIN * 2.0, PANEL_MARGIN * 2.0));
        let panel = Rect::from_center_size(screen.center(), size);
        ui.begin_glass_panel(panel, &self.panel.glass(ui.theme()));
        ui.title_bar("Effects", TITLE_BAR_HEIGHT);
        let m = &ui.theme().metrics;
        let body_height = panel.height() - 2.0 * m.panel_padding - TITLE_BAR_HEIGHT - m.spacing;
        ui.horizontal(|ui| {
            ui.begin_vertical_width(SIDEBAR_WIDTH);
            for (i, (icon, name)) in PAGES.iter().enumerate() {
                if ui.nav_item(name, Some((*icon).into()), self.page == i).clicked {
                    self.page = i;
                }
            }
            ui.end();
            ui.begin_vertical_width(ui.available_width());
            ui.begin_transition("page", self.page as u64, Slide::Up);
            ui.scroll_area("page", Some(body_height), |ui| match self.page {
                0 => self.backdrop_page(ui),
                1 => self.panel_page(ui),
                _ => self.about_page(ui),
            });
            ui.end();
            ui.end();
        });
        ui.end();
    }
}

impl Effects {
    fn backdrop(&self, theme: &Theme) -> Backdrop {
        let s = &self.backdrop;
        let mut b = Backdrop::new(BackdropKind::ALL[s.kind]).with_seed(s.variation as f32);
        if s.use_accent {
            let p = &theme.palette;
            b = b.with_colors(b.base, p.accent, p.accent.lighten(0.45));
        }
        b.scale = s.size;
        b.speed = SPEEDS[s.speed].1;
        b.intensity = s.brightness;
        b.pointer_response = s.pointer_response;
        b
    }
}

/// Index of `name` in `names` (case-insensitive), or 0.
fn position<'a>(names: impl IntoIterator<Item = &'a str>, name: &str) -> usize {
    names.into_iter().position(|n| n.eq_ignore_ascii_case(name)).unwrap_or(0)
}

fn main() -> shot::Result {
    let args = shot::args();
    let mut app = Effects::default();
    let pick = |flag: &str, names: &[&str]| shot::arg(&args, flag).map(|name| position(names.iter().copied(), &name));
    app.page = pick("--page", &PAGES.map(|(_, n)| n)).unwrap_or(0);
    app.panel.theme = pick("--theme", &THEMES.map(|(n, _)| n)).unwrap_or(0);
    app.backdrop.kind = pick("--backdrop", &BackdropKind::ALL.map(BackdropKind::name)).unwrap_or(app.backdrop.kind);
    if let Some(glass) = pick("--glass", &GLASS.map(|(n, ..)| n)) {
        app.panel.select_glass(glass);
    }
    let options = AppOptions {
        title: "Effects".into(),
        size: [1000, 700],
        min_size: [720, 600],
        decorations: false,
        ..Default::default()
    };
    shot::run(options, app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rugi::{App, FrameInput, UiOptions};

    #[test]
    fn light_themes_raise_the_tint() {
        let panel = PanelSettings { tint: 0.2, ..Default::default() };
        assert_eq!(panel.glass(&Theme::dark()).tint.a, 51);
        assert_eq!(PanelSettings::min_tint(&Theme::light()), LIGHT_THEME_MIN_TINT);
        assert_eq!(panel.glass(&Theme::light()).tint.a, 191);
    }

    #[test]
    fn every_page_builds_without_errors() {
        let mut ui = Ui::new(UiOptions::default());
        let mut app = Effects::default();
        for (page, (_, name)) in PAGES.iter().enumerate() {
            app.page = page;
            let input = FrameInput { size: Vec2::new(1000.0, 700.0), scale: 1.0, dt: 1.0 / 60.0 };
            ui.begin_frame(input).unwrap();
            app.frame(&mut ui);
            ui.end_frame();
            assert!(ui.take_error().is_none(), "page {name}");
        }
    }
}
