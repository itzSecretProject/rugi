//! Themes, theme colors and metrics, and scoped style overrides.

use rugi_core::Color;
use rugi_theme::{Palette, Theme};

use crate::args::out_mut;
use crate::error::{guard, FfiError, RugiResult};
use crate::handle::{handle, with_handle, RugiUi};
use crate::types::RugiColor;

/// Built-in theme selector.
pub type RugiTheme = u32;
/// Dark theme (default).
pub const RUGI_THEME_DARK: RugiTheme = 0;
/// Light theme.
pub const RUGI_THEME_LIGHT: RugiTheme = 1;
/// Warm dark theme with a rose accent.
pub const RUGI_THEME_ROSE: RugiTheme = 2;
/// Coffee-brown dark theme.
pub const RUGI_THEME_MOCHA: RugiTheme = 3;
/// Deep green dark theme.
pub const RUGI_THEME_FOREST: RugiTheme = 4;
/// Near-black monochrome theme.
pub const RUGI_THEME_GRAPHITE: RugiTheme = 5;

/// Semantic color role for rugi_ui_set_color / rugi_push_color.
pub type RugiColorRole = u32;
/// Window background.
pub const RUGI_COLOR_BACKGROUND: RugiColorRole = 0;
/// Bottom of the window background gradient.
pub const RUGI_COLOR_BACKGROUND_ALT: RugiColorRole = 1;
/// Side panels and chrome.
pub const RUGI_COLOR_PANEL: RugiColorRole = 2;
/// Cards and raised surfaces.
pub const RUGI_COLOR_SURFACE: RugiColorRole = 3;
/// Top of the card gradient.
pub const RUGI_COLOR_SURFACE_HIGHLIGHT: RugiColorRole = 4;
/// Hovered neutral surface.
pub const RUGI_COLOR_SURFACE_HOVER: RugiColorRole = 5;
/// Pressed neutral surface.
pub const RUGI_COLOR_SURFACE_ACTIVE: RugiColorRole = 6;
/// Recessed controls (text inputs, slider tracks).
pub const RUGI_COLOR_SUNKEN: RugiColorRole = 7;
/// Hairline borders.
pub const RUGI_COLOR_BORDER: RugiColorRole = 8;
/// Borders of hovered or emphasized elements.
pub const RUGI_COLOR_BORDER_STRONG: RugiColorRole = 9;
/// Primary text.
pub const RUGI_COLOR_TEXT: RugiColorRole = 10;
/// Secondary text.
pub const RUGI_COLOR_TEXT_MUTED: RugiColorRole = 11;
/// Tertiary text and placeholders.
pub const RUGI_COLOR_TEXT_FAINT: RugiColorRole = 12;
/// Accent; hover/active/soft variants are derived automatically.
pub const RUGI_COLOR_ACCENT: RugiColorRole = 13;
/// Text drawn on the accent color.
pub const RUGI_COLOR_ON_ACCENT: RugiColorRole = 14;
/// Positive state.
pub const RUGI_COLOR_SUCCESS: RugiColorRole = 15;
/// Warning state.
pub const RUGI_COLOR_WARNING: RugiColorRole = 16;
/// Destructive or error state.
pub const RUGI_COLOR_DANGER: RugiColorRole = 17;
/// Drop shadows.
pub const RUGI_COLOR_SHADOW: RugiColorRole = 18;

/// Size token for rugi_ui_set_metric / rugi_push_metric (logical pixels).
pub type RugiMetric = u32;
/// Small corner radius (checkboxes, chips).
pub const RUGI_METRIC_RADIUS_SM: RugiMetric = 0;
/// Default control corner radius.
pub const RUGI_METRIC_RADIUS_MD: RugiMetric = 1;
/// Card and panel corner radius.
pub const RUGI_METRIC_RADIUS_LG: RugiMetric = 2;
/// Default gap between stacked items.
pub const RUGI_METRIC_SPACING: RugiMetric = 3;
/// Small gap.
pub const RUGI_METRIC_SPACING_SM: RugiMetric = 4;
/// Large gap between sections.
pub const RUGI_METRIC_SPACING_LG: RugiMetric = 5;
/// Inner padding of panels.
pub const RUGI_METRIC_PANEL_PADDING: RugiMetric = 6;
/// Inner padding of cards.
pub const RUGI_METRIC_CARD_PADDING: RugiMetric = 7;
/// Height of buttons, inputs and nav items.
pub const RUGI_METRIC_CONTROL_HEIGHT: RugiMetric = 8;
/// Horizontal padding inside buttons and inputs.
pub const RUGI_METRIC_CONTROL_PADDING_X: RugiMetric = 9;
/// Hairline border width.
pub const RUGI_METRIC_BORDER_WIDTH: RugiMetric = 10;
/// Focus ring width.
pub const RUGI_METRIC_FOCUS_RING_WIDTH: RugiMetric = 11;

pub(crate) fn theme_from(kind: RugiTheme) -> Result<Theme, FfiError> {
    Ok(match kind {
        RUGI_THEME_DARK => Theme::dark(),
        RUGI_THEME_LIGHT => Theme::light(),
        RUGI_THEME_ROSE => Theme::rose(),
        RUGI_THEME_MOCHA => Theme::mocha(),
        RUGI_THEME_FOREST => Theme::forest(),
        RUGI_THEME_GRAPHITE => Theme::graphite(),
        _ => return Err(FfiError::invalid(format!("unknown theme {kind}"))),
    })
}

fn apply_color(t: &mut Theme, role: RugiColorRole, c: Color) -> Result<(), FfiError> {
    if role == RUGI_COLOR_ACCENT {
        t.set_accent(c);
    } else {
        *color_slot(&mut t.palette, role)? = c;
    }
    Ok(())
}

/// The palette field behind `role`.
fn color_slot(p: &mut Palette, role: RugiColorRole) -> Result<&mut Color, FfiError> {
    Ok(match role {
        RUGI_COLOR_ACCENT => &mut p.accent,
        RUGI_COLOR_BACKGROUND => &mut p.background,
        RUGI_COLOR_BACKGROUND_ALT => &mut p.background_alt,
        RUGI_COLOR_PANEL => &mut p.panel,
        RUGI_COLOR_SURFACE => &mut p.surface,
        RUGI_COLOR_SURFACE_HIGHLIGHT => &mut p.surface_highlight,
        RUGI_COLOR_SURFACE_HOVER => &mut p.surface_hover,
        RUGI_COLOR_SURFACE_ACTIVE => &mut p.surface_active,
        RUGI_COLOR_SUNKEN => &mut p.sunken,
        RUGI_COLOR_BORDER => &mut p.border,
        RUGI_COLOR_BORDER_STRONG => &mut p.border_strong,
        RUGI_COLOR_TEXT => &mut p.text,
        RUGI_COLOR_TEXT_MUTED => &mut p.text_muted,
        RUGI_COLOR_TEXT_FAINT => &mut p.text_faint,
        RUGI_COLOR_ON_ACCENT => &mut p.on_accent,
        RUGI_COLOR_SUCCESS => &mut p.success,
        RUGI_COLOR_WARNING => &mut p.warning,
        RUGI_COLOR_DANGER => &mut p.danger,
        RUGI_COLOR_SHADOW => &mut p.shadow,
        _ => return Err(FfiError::invalid(format!("unknown color role {role}"))),
    })
}

fn apply_metric(t: &mut Theme, metric: RugiMetric, v: f32) -> Result<(), FfiError> {
    if !v.is_finite() || v < 0.0 {
        return Err(FfiError::invalid("metric value must be finite and >= 0"));
    }
    let m = &mut t.metrics;
    let slot = match metric {
        RUGI_METRIC_RADIUS_SM => &mut m.radius_sm,
        RUGI_METRIC_RADIUS_MD => &mut m.radius_md,
        RUGI_METRIC_RADIUS_LG => &mut m.radius_lg,
        RUGI_METRIC_SPACING => &mut m.spacing,
        RUGI_METRIC_SPACING_SM => &mut m.spacing_sm,
        RUGI_METRIC_SPACING_LG => &mut m.spacing_lg,
        RUGI_METRIC_PANEL_PADDING => &mut m.panel_padding,
        RUGI_METRIC_CARD_PADDING => &mut m.card_padding,
        RUGI_METRIC_CONTROL_HEIGHT => &mut m.control_height,
        RUGI_METRIC_CONTROL_PADDING_X => &mut m.control_padding_x,
        RUGI_METRIC_BORDER_WIDTH => &mut m.border_width,
        RUGI_METRIC_FOCUS_RING_WIDTH => &mut m.focus_ring_width,
        _ => return Err(FfiError::invalid(format!("unknown metric {metric}"))),
    };
    *slot = v;
    Ok(())
}

/// Switches to a built-in theme (keeps the current accent color).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_set_theme(ui: *mut RugiUi, theme: RugiTheme) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        let h = unsafe { handle(ui) }?;
        let accent = h.ui().theme().palette.accent;
        let t = theme_from(theme)?.with_accent(accent);
        h.ui().set_theme(t);
        Ok(())
    })
}

/// Sets the accent color (derived hover/active/soft colors follow).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_set_accent(ui: *mut RugiUi, color: RugiColor) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        unsafe { handle(ui) }?.ui().theme_mut().set_accent(color.into());
        Ok(())
    })
}

/// Permanently sets a theme color.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_set_color(ui: *mut RugiUi, role: RugiColorRole, c: RugiColor) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        let h = unsafe { handle(ui) }?;
        let mut t = h.ui().theme().clone();
        apply_color(&mut t, role, c.into())?;
        h.ui().set_theme(t);
        Ok(())
    })
}

/// Writes the current theme's color for `role` to `out`.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_color(ui: *mut RugiUi, role: RugiColorRole, out: *mut RugiColor) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        let h = unsafe { handle(ui) }?;
        let mut palette = h.ui().theme().palette.clone();
        let c = *color_slot(&mut palette, role)?;
        // SAFETY: caller provides a writable RugiColor or NULL (rejected).
        *unsafe { out_mut(out, "out") }? = c.into();
        Ok(())
    })
}

/// Permanently sets a theme metric.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_set_metric(ui: *mut RugiUi, metric: RugiMetric, value: f32) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        let h = unsafe { handle(ui) }?;
        let mut t = h.ui().theme().clone();
        apply_metric(&mut t, metric, value)?;
        h.ui().set_theme(t);
        Ok(())
    })
}

/// Overrides a color for following widgets until rugi_pop_style.
#[no_mangle]
pub unsafe extern "C" fn rugi_push_color(ui: *mut RugiUi, role: RugiColorRole, c: RugiColor) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe {
        with_handle(ui, |h| {
            h.require_frame()?;
            let mut t = h.ui().theme().clone();
            apply_color(&mut t, role, c.into())?;
            h.ui().push_style(move |th| *th = t);
            Ok(())
        })
    }
}

/// Overrides a metric for following widgets until rugi_pop_style.
#[no_mangle]
pub unsafe extern "C" fn rugi_push_metric(ui: *mut RugiUi, metric: RugiMetric, value: f32) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe {
        with_handle(ui, |h| {
            h.require_frame()?;
            let mut t = h.ui().theme().clone();
            apply_metric(&mut t, metric, value)?;
            h.ui().push_style(move |th| *th = t);
            Ok(())
        })
    }
}

/// Undoes the most recent rugi_push_color / rugi_push_metric / rugi_push_font.
#[no_mangle]
pub unsafe extern "C" fn rugi_pop_style(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe {
        with_handle(ui, |h| {
            h.require_frame()?;
            h.ui().pop_style();
            Ok(())
        })
    }
}
