//! Rugi design tokens.
//!
//! Widgets read colors and sizes from a [`Theme`]. A theme is plain data
//! (palette, metrics, typography, motion), so it can be built in code and
//! edited at runtime.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use rugi_core::{Color, FontFamily, FontWeight, TextStyle};
use std::borrow::Cow;

/// Semantic color roles.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    /// Window background.
    pub background: Color,
    /// Background gradient end (bottom) for the main canvas.
    pub background_alt: Color,
    /// Side panels / chrome.
    pub panel: Color,
    /// Cards and raised surfaces.
    pub surface: Color,
    /// Top of the subtle card gradient.
    pub surface_highlight: Color,
    /// Hovered neutral surface.
    pub surface_hover: Color,
    /// Pressed neutral surface.
    pub surface_active: Color,
    /// Recessed controls (text inputs, slider tracks).
    pub sunken: Color,
    /// Hairline borders.
    pub border: Color,
    /// Borders of hovered / emphasized elements.
    pub border_strong: Color,
    /// Outline of checkboxes, radio buttons and text inputs. Built-in themes
    /// keep it at 3:1 against surfaces (WCAG 1.4.11).
    pub control_border: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text.
    pub text_muted: Color,
    /// Tertiary text, placeholders.
    pub text_faint: Color,
    /// Accent fill (primary buttons, checked controls, indicators). Built-in
    /// themes keep [`Palette::on_accent`] at 4.5:1 on it in every state.
    pub accent: Color,
    /// Accent when hovered.
    pub accent_hover: Color,
    /// Accent when pressed.
    pub accent_active: Color,
    /// Text drawn on top of the accent.
    pub on_accent: Color,
    /// Translucent accent for selections and soft highlights.
    pub accent_soft: Color,
    /// Accent for text and icons on surfaces (links, selected items, badges);
    /// 4.5:1 against every surface, including `accent_soft` selections.
    pub accent_text: Color,
    /// Positive state.
    pub success: Color,
    /// Warning state.
    pub warning: Color,
    /// Destructive / error state.
    pub danger: Color,
    /// Text drawn on top of `danger` (danger buttons).
    pub on_danger: Color,
    /// Drop shadow color.
    pub shadow: Color,
}

/// Sizes and spacing in logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    /// Small radius (checkboxes, chips).
    pub radius_sm: f32,
    /// Default control radius.
    pub radius_md: f32,
    /// Card / panel radius.
    pub radius_lg: f32,
    /// Default gap between stacked items.
    pub spacing: f32,
    /// Small gap.
    pub spacing_sm: f32,
    /// Large gap (between sections).
    pub spacing_lg: f32,
    /// Inner padding of panels.
    pub panel_padding: f32,
    /// Inner padding of cards.
    pub card_padding: f32,
    /// Height of buttons, inputs, nav items.
    pub control_height: f32,
    /// Horizontal padding inside buttons and inputs.
    pub control_padding_x: f32,
    /// Hairline border width.
    pub border_width: f32,
    /// Focus ring width.
    pub focus_ring_width: f32,
    /// Checkbox edge length.
    pub checkbox_size: f32,
    /// Slider thumb diameter.
    pub slider_thumb: f32,
    /// Slider track thickness.
    pub slider_track: f32,
    /// Scrollbar thickness.
    pub scrollbar_width: f32,
    /// Width of the label column in form rows (`Ui::labeled` in rugi-ui).
    pub label_width: f32,
}

/// Text styles by role.
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    /// Large page title.
    pub title: TextStyle,
    /// Section heading.
    pub heading: TextStyle,
    /// Body text.
    pub body: TextStyle,
    /// Control labels (buttons, nav items).
    pub label: TextStyle,
    /// Small captions and metadata.
    pub caption: TextStyle,
    /// Large numeric display (stat cards).
    pub display: TextStyle,
}

impl Typography {
    /// All text styles, for bulk edits.
    pub fn styles_mut(&mut self) -> [&mut TextStyle; 6] {
        [&mut self.title, &mut self.heading, &mut self.body, &mut self.label, &mut self.caption, &mut self.display]
    }
}

/// Animation timing.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    /// Time constant for hover transitions (seconds).
    pub hover: f32,
    /// Time constant for press transitions (seconds).
    pub press: f32,
    /// Time constant for value transitions such as checkbox ticks (seconds).
    pub value: f32,
    /// Caret blink half-period (seconds). 0 disables blinking.
    pub caret_blink: f32,
}

/// A complete theme.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Human readable name.
    pub name: Cow<'static, str>,
    /// True for dark themes.
    pub dark: bool,
    /// Colors.
    pub palette: Palette,
    /// Sizes.
    pub metrics: Metrics,
    /// Text styles.
    pub typography: Typography,
    /// Motion.
    pub motion: Motion,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}

/// The default accent of the dark theme (system blue, dark appearance).
pub const DEFAULT_ACCENT: Color = Color::hex(0x0A84FF);

/// The default accent of the light theme (system blue, light appearance).
const LIGHT_ACCENT: Color = Color::hex(0x007AFF);

/// WCAG 2.2 AA minimum for normal-size text (1.4.3).
const TEXT_CONTRAST: f32 = 4.5;
/// WCAG 2.2 AA minimum for component boundaries and indicators (1.4.11).
const NON_TEXT_CONTRAST: f32 = 3.0;
/// Label used on light fills.
const DARK_LABEL: Color = Color::hex(0x101114);
/// Below this white-on-fill ratio a dark label is used instead of darkening
/// the fill a long way.
const LABEL_SWITCH: f32 = 3.0;
/// Hover and pressed fills of solid buttons, as drawn by `rugi-ui`.
const HOVER_LIGHTEN: f32 = 0.08;
const PRESS_DARKEN: f32 = 0.12;
/// Opacity of the tint behind badge text.
const BADGE_TINT: f32 = 0.14;

/// WCAG 2.2 contrast ratio between two colors (1 to 21). Alpha is ignored.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    fn channel(c: u8) -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let lum = |c: Color| 0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b) + 0.05;
    let (la, lb) = (lum(a), lum(b));
    la.max(lb) / la.min(lb)
}

/// `fg` composited over an opaque `bg`.
fn over(fg: Color, bg: Color) -> Color {
    Color { a: 255, ..bg.lerp(Color { a: 255, ..fg }, fg.a as f32 / 255.0) }
}

/// The first color on the way from `from` to `toward`, in 2% steps, that
/// satisfies `ok`; `toward` if none does.
fn adjust(from: Color, toward: Color, ok: impl Fn(Color) -> bool) -> Color {
    (0..=50).map(|i| from.lerp(toward, i as f32 / 50.0)).find(|&c| ok(c)).unwrap_or(toward)
}

/// White or near-black, whichever stays readable on `fill` while hovered and
/// pressed.
fn label_for(fill: Color) -> Color {
    let ok = |label: Color| {
        [fill, fill.lighten(HOVER_LIGHTEN), fill.darken(PRESS_DARKEN)]
            .iter()
            .all(|&f| contrast_ratio(label, f) >= TEXT_CONTRAST)
    };
    if ok(Color::WHITE) || !ok(DARK_LABEL) {
        Color::WHITE
    } else {
        DARK_LABEL
    }
}

impl Palette {
    /// Opaque backgrounds that text is drawn on.
    fn text_surfaces(&self) -> [Color; 5] {
        [self.background, self.panel, self.surface, self.surface_hover, self.sunken]
    }
}

fn metrics() -> Metrics {
    Metrics {
        radius_sm: 5.0,
        radius_md: 8.0,
        radius_lg: 12.0,
        spacing: 10.0,
        spacing_sm: 6.0,
        spacing_lg: 20.0,
        panel_padding: 16.0,
        card_padding: 18.0,
        control_height: 34.0,
        control_padding_x: 14.0,
        border_width: 1.0,
        focus_ring_width: 2.0,
        checkbox_size: 18.0,
        slider_thumb: 16.0,
        slider_track: 4.0,
        scrollbar_width: 6.0,
        label_width: 140.0,
    }
}

fn typography() -> Typography {
    Typography {
        title: TextStyle::new(22.0).weight(FontWeight::SEMIBOLD).letter_spacing(-0.01),
        heading: TextStyle::new(15.0).weight(FontWeight::SEMIBOLD),
        body: TextStyle::new(13.5),
        label: TextStyle::new(13.5).weight(FontWeight::MEDIUM),
        caption: TextStyle::new(12.0),
        display: TextStyle::new(26.0).weight(FontWeight::SEMIBOLD).letter_spacing(-0.015),
    }
}

fn motion() -> Motion {
    Motion { hover: 0.07, press: 0.04, value: 0.09, caret_blink: 0.53 }
}

impl Theme {
    /// The default dark theme.
    pub fn dark() -> Theme {
        let mut t = Theme {
            name: Cow::Borrowed("Rugi Dark"),
            dark: true,
            palette: Palette {
                background: Color::hex(0x0D0E12),
                background_alt: Color::hex(0x0A0B0E),
                panel: Color::hex(0x111217),
                surface: Color::hex(0x16171D),
                surface_highlight: Color::hex(0x1A1C23),
                surface_hover: Color::hex(0x1F2129),
                surface_active: Color::hex(0x262832),
                sunken: Color::hex(0x0E0F13),
                border: Color::hex(0x23252E),
                border_strong: Color::hex(0x343743),
                control_border: Color::hex(0x343743),
                text: Color::hex(0xECEDF1),
                text_muted: Color::hex(0x9A9DAA),
                text_faint: Color::hex(0x5F6371),
                accent: DEFAULT_ACCENT,
                accent_hover: DEFAULT_ACCENT,
                accent_active: DEFAULT_ACCENT,
                on_accent: Color::WHITE,
                accent_soft: DEFAULT_ACCENT,
                accent_text: DEFAULT_ACCENT,
                success: Color::hex(0x3DD68C),
                warning: Color::hex(0xF5B547),
                danger: Color::hex(0xF7646E),
                on_danger: Color::WHITE,
                shadow: Color::rgba(0, 0, 0, 140),
            },
            metrics: metrics(),
            typography: typography(),
            motion: motion(),
        };
        t.meet_contrast();
        t.set_accent(DEFAULT_ACCENT);
        t
    }

    /// A light theme.
    pub fn light() -> Theme {
        let mut t = Theme {
            name: Cow::Borrowed("Rugi Light"),
            dark: false,
            palette: Palette {
                background: Color::hex(0xF5F6F8),
                background_alt: Color::hex(0xEEF0F3),
                panel: Color::hex(0xFBFBFC),
                surface: Color::hex(0xFFFFFF),
                surface_highlight: Color::hex(0xFFFFFF),
                surface_hover: Color::hex(0xF0F1F4),
                surface_active: Color::hex(0xE6E8EC),
                sunken: Color::hex(0xF3F4F6),
                border: Color::hex(0xE2E4E9),
                border_strong: Color::hex(0xC9CCD4),
                control_border: Color::hex(0xC9CCD4),
                text: Color::hex(0x16181D),
                text_muted: Color::hex(0x5D6270),
                text_faint: Color::hex(0x9A9EAA),
                accent: LIGHT_ACCENT,
                accent_hover: LIGHT_ACCENT,
                accent_active: LIGHT_ACCENT,
                on_accent: Color::WHITE,
                accent_soft: LIGHT_ACCENT,
                accent_text: LIGHT_ACCENT,
                success: Color::hex(0x16A869),
                warning: Color::hex(0xD98E04),
                danger: Color::hex(0xE0414D),
                on_danger: Color::WHITE,
                shadow: Color::rgba(20, 24, 40, 40),
            },
            metrics: metrics(),
            typography: typography(),
            motion: motion(),
        };
        t.meet_contrast();
        t.set_accent(LIGHT_ACCENT);
        t
    }

    /// Sets the accent from a brand color and derives the colors that depend
    /// on it.
    ///
    /// The fill is darkened or lightened only as far as needed for its label
    /// (`on_accent`, white or near-black) to reach 4.5:1 in the rest, hover and
    /// pressed states. `accent_text` is the brand color adjusted to 4.5:1
    /// against the surfaces. `on_danger` is recomputed too. Assign palette
    /// fields directly afterwards to opt out.
    pub fn set_accent(&mut self, accent: Color) {
        let brand = Color { a: 255, ..accent };
        let p = &mut self.palette;
        let (fill, on) = if contrast_ratio(Color::WHITE, brand) >= LABEL_SWITCH {
            let fill = adjust(brand, Color::BLACK, |c| {
                contrast_ratio(Color::WHITE, c.lighten(HOVER_LIGHTEN)) >= TEXT_CONTRAST
            });
            (fill, Color::WHITE)
        } else {
            let fill =
                adjust(brand, Color::WHITE, |c| contrast_ratio(DARK_LABEL, c.darken(PRESS_DARKEN)) >= TEXT_CONTRAST);
            (fill, DARK_LABEL)
        };
        p.accent = fill;
        p.accent_hover = fill.lighten(HOVER_LIGHTEN);
        p.accent_active = fill.darken(PRESS_DARKEN);
        p.on_accent = on;
        p.accent_soft = fill.with_alpha(if self.dark { 0.16 } else { 0.12 });
        let surfaces = p.text_surfaces();
        let soft = [over(p.accent_soft, p.surface), over(p.accent_soft, p.panel)];
        let toward = if self.dark { Color::WHITE } else { Color::BLACK };
        let surface = p.surface;
        p.accent_text = adjust(brand, toward, |c| {
            let badge = over(c.with_alpha(BADGE_TINT), surface);
            surfaces.iter().chain(&soft).chain([badge].iter()).all(|&bg| contrast_ratio(c, bg) >= TEXT_CONTRAST)
        });
        p.on_danger = label_for(p.danger);
    }

    /// Adjusts the secondary text, status and control-outline colors of a
    /// built-in theme until they meet WCAG AA against its surfaces.
    fn meet_contrast(&mut self) {
        let p = &mut self.palette;
        let surfaces = p.text_surfaces();
        let readable = |c: Color| surfaces.iter().all(|&bg| contrast_ratio(c, bg) >= TEXT_CONTRAST);
        p.text_muted = adjust(p.text_muted, p.text, readable);
        p.text_faint = adjust(p.text_faint, p.text, readable);
        let toward = if self.dark { Color::WHITE } else { Color::BLACK };
        let surface = p.surface;
        // Status colors are also drawn on their own tint (badges).
        let status =
            |c: Color| readable(c) && contrast_ratio(c, over(c.with_alpha(BADGE_TINT), surface)) >= TEXT_CONTRAST;
        p.success = adjust(p.success, toward, status);
        p.warning = adjust(p.warning, toward, status);
        p.danger = adjust(p.danger, toward, status);
        p.control_border =
            adjust(p.border_strong, p.text, |c| surfaces.iter().all(|&bg| contrast_ratio(c, bg) >= NON_TEXT_CONTRAST));
    }

    /// Uses `family` for every text role (the whole UI font).
    pub fn set_font_family(&mut self, family: FontFamily) {
        for s in self.typography.styles_mut() {
            s.family = family;
        }
    }

    /// Multiplies every text size by `factor` (e.g. 1.15 for larger UI text).
    pub fn scale_text(&mut self, factor: f32) {
        let f = if factor.is_finite() && factor > 0.0 { factor } else { 1.0 };
        for s in self.typography.styles_mut() {
            s.size *= f;
        }
    }

    /// Derives a dark theme from a background, text color and accent; surfaces,
    /// borders and secondary text levels are generated from them.
    pub fn dark_tinted(name: &'static str, background: Color, text: Color, accent: Color) -> Theme {
        let mut t = Theme::dark();
        t.name = Cow::Borrowed(name);
        let bg = background;
        let p = &mut t.palette;
        p.background = bg;
        p.background_alt = bg.darken(0.25);
        p.panel = bg.lighten(0.025);
        p.surface = bg.lighten(0.05);
        p.surface_highlight = bg.lighten(0.07);
        p.surface_hover = bg.lighten(0.09);
        p.surface_active = bg.lighten(0.13);
        p.sunken = bg.darken(0.15);
        p.border = bg.lighten(0.10);
        p.border_strong = bg.lighten(0.18);
        p.text = text;
        p.text_muted = text.lerp(bg, 0.38);
        p.text_faint = text.lerp(bg, 0.62);
        t.meet_contrast();
        t.set_accent(accent);
        t
    }

    /// Warm dark theme with a rose accent.
    pub fn rose() -> Theme {
        Theme::dark_tinted("Rose", Color::hex(0x17101A), Color::hex(0xF4E9EE), Color::hex(0xF0789A))
    }

    /// Coffee-brown dark theme with a caramel accent.
    pub fn mocha() -> Theme {
        Theme::dark_tinted("Mocha", Color::hex(0x16120F), Color::hex(0xF1E8DF), Color::hex(0xD2A374))
    }

    /// Deep green dark theme with a mint accent.
    pub fn forest() -> Theme {
        Theme::dark_tinted("Forest", Color::hex(0x0D1411), Color::hex(0xE6F1EA), Color::hex(0x5CC98F))
    }

    /// Neutral near-black theme with a white accent (monochrome glass UIs).
    pub fn graphite() -> Theme {
        let mut t = Theme::dark_tinted("Graphite", Color::hex(0x0F0F11), Color::hex(0xF2F2F4), Color::hex(0xE9E9EC));
        t.palette.accent_soft = Color::rgba(255, 255, 255, 26);
        t
    }

    /// All built-in themes (dark, light and the tinted presets).
    pub fn presets() -> Vec<Theme> {
        vec![Theme::dark(), Theme::light(), Theme::rose(), Theme::mocha(), Theme::forest(), Theme::graphite()]
    }

    /// Builder-style [`Theme::set_accent`].
    pub fn with_accent(mut self, accent: Color) -> Theme {
        self.set_accent(accent);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_derivation() {
        let t = Theme::dark().with_accent(Color::hex(0xFF0000));
        let p = &t.palette;
        assert_eq!(p.on_accent, Color::WHITE);
        assert!(p.accent.r > 150 && p.accent.g < 10 && p.accent.b < 10, "{:?}", p.accent);
        assert!(contrast_ratio(p.on_accent, p.accent_hover) >= TEXT_CONTRAST);
        assert!(p.accent_soft.a < 64);
        // Light accents keep their color and get a dark label.
        let y = Theme::dark().with_accent(Color::hex(0xFFE066));
        assert_eq!(y.palette.on_accent, DARK_LABEL);
        assert_eq!(y.palette.accent, Color::hex(0xFFE066));
        // An already accessible accent is left alone.
        let ok = Theme::dark().with_accent(Color::hex(0x0050B0));
        assert_eq!(ok.palette.accent, Color::hex(0x0050B0));
    }

    #[test]
    fn presets_are_readable() {
        for t in Theme::presets() {
            let p = &t.palette;
            // Text must contrast with the background in luminance.
            assert!((p.text.luma() - p.background.luma()).abs() > 0.6, "{}", t.name);
            assert!((p.text_muted.luma() - p.background.luma()).abs() > 0.3, "{}", t.name);
        }
    }

    #[test]
    fn font_helpers() {
        let mut t = Theme::dark();
        t.set_font_family(FontFamily::Custom(3));
        t.scale_text(2.0);
        assert_eq!(t.typography.caption.family, FontFamily::Custom(3));
        assert_eq!(t.typography.body.size, 27.0);
    }

    #[test]
    fn themes_differ() {
        assert!(Theme::dark().dark && !Theme::light().dark);
        assert_ne!(Theme::dark().palette.background, Theme::light().palette.background);
    }
}
