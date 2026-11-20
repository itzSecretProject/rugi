//! Glass surfaces, animated backdrops and screen transitions.

use std::ffi::c_char;

use rugi_core::{Backdrop, BackdropKind, GlassStyle};
use rugi_ui::{AmbientRate, Slide};

use crate::args::{in_ref, out_mut, str_arg};
use crate::error::{guard, FfiError, RugiResult};
use crate::handle::{begin_container, handle, RugiUi};
use crate::types::{rect_from, RugiColor, RugiRect};

/// Glass preset for rugi_glass_style_preset.
pub type RugiGlassPreset = u32;
/// Soft frosted panel.
pub const RUGI_GLASS_FROSTED: RugiGlassPreset = 0;
/// Liquid glass: refraction and dispersion at the rim.
pub const RUGI_GLASS_LIQUID: RugiGlassPreset = 1;
/// Dark smoked glass.
pub const RUGI_GLASS_SMOKED: RugiGlassPreset = 2;

/// Appearance of a glass surface (see rugi_glass_style_preset for defaults).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RugiGlassStyle {
    /// Blur strength 0..1.
    pub blur: f32,
    /// Tint over the blurred backdrop; alpha = strength.
    pub tint: RugiColor,
    /// Rim refraction in logical pixels.
    pub refraction: f32,
    /// Width of the refracting rim.
    pub bevel: f32,
    /// Chromatic dispersion 0..1.
    pub dispersion: f32,
    /// Saturation multiplier (1 = unchanged).
    pub saturation: f32,
    /// Rim light strength 0..1.
    pub highlight: f32,
    /// Rim light color.
    pub highlight_color: RugiColor,
    /// Film grain 0..1.
    pub grain: f32,
    /// Border width.
    pub border_width: f32,
    /// Border color.
    pub border: RugiColor,
    /// Corner radius.
    pub radius: f32,
}

/// Procedural backdrop algorithm.
pub type RugiBackdropKind = u32;
/// Slow aurora curtains blending three colors.
pub const RUGI_BACKDROP_AURORA: RugiBackdropKind = 0;
/// Domain-warped smoke / ink.
pub const RUGI_BACKDROP_SMOKE: RugiBackdropKind = 1;
/// Classic sine plasma.
pub const RUGI_BACKDROP_PLASMA: RugiBackdropKind = 2;
/// Drifting, twinkling stars over a faint nebula.
pub const RUGI_BACKDROP_STARFIELD: RugiBackdropKind = 3;
/// A glowing electric filament.
pub const RUGI_BACKDROP_FILAMENT: RugiBackdropKind = 4;
/// Moving mesh gradient with film grain.
pub const RUGI_BACKDROP_MESH: RugiBackdropKind = 5;

/// An animated procedural background (see rugi_backdrop_preset).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RugiBackdrop {
    /// RUGI_BACKDROP_*.
    pub kind: RugiBackdropKind,
    /// Base (darkest) color.
    pub base: RugiColor,
    /// First accent.
    pub color_a: RugiColor,
    /// Second accent.
    pub color_b: RugiColor,
    /// Feature scale (1 = default).
    pub scale: f32,
    /// Animation speed (0 = frozen).
    pub speed: f32,
    /// Accent brightness 0..2.
    pub intensity: f32,
    /// Variation seed.
    pub seed: f32,
    /// Pointer reactivity 0..1.
    pub pointer_response: f32,
}

/// Slide direction for rugi_begin_transition.
pub type RugiSlide = u32;
/// Fade only.
pub const RUGI_SLIDE_NONE: RugiSlide = 0;
/// Slide in from the right (forward navigation).
pub const RUGI_SLIDE_FORWARD: RugiSlide = 1;
/// Slide in from the left (back navigation).
pub const RUGI_SLIDE_BACK: RugiSlide = 2;
/// Rise in from slightly below.
pub const RUGI_SLIDE_UP: RugiSlide = 3;

impl From<&RugiGlassStyle> for GlassStyle {
    fn from(s: &RugiGlassStyle) -> GlassStyle {
        GlassStyle {
            blur: s.blur,
            tint: s.tint.into(),
            refraction: s.refraction,
            bevel: s.bevel,
            dispersion: s.dispersion,
            saturation: s.saturation,
            highlight: s.highlight,
            highlight_color: s.highlight_color.into(),
            grain: s.grain,
            border_width: s.border_width,
            border: s.border.into(),
            radius: s.radius,
        }
    }
}

impl From<GlassStyle> for RugiGlassStyle {
    fn from(g: GlassStyle) -> RugiGlassStyle {
        RugiGlassStyle {
            blur: g.blur,
            tint: g.tint.into(),
            refraction: g.refraction,
            bevel: g.bevel,
            dispersion: g.dispersion,
            saturation: g.saturation,
            highlight: g.highlight,
            highlight_color: g.highlight_color.into(),
            grain: g.grain,
            border_width: g.border_width,
            border: g.border.into(),
            radius: g.radius,
        }
    }
}

fn backdrop_kind(kind: RugiBackdropKind) -> Result<BackdropKind, FfiError> {
    BackdropKind::from_u32(kind).ok_or_else(|| FfiError::invalid(format!("unknown backdrop kind {kind}")))
}

fn backdrop_from(b: &RugiBackdrop) -> Result<Backdrop, FfiError> {
    Ok(Backdrop {
        kind: backdrop_kind(b.kind)?,
        base: b.base.into(),
        color_a: b.color_a.into(),
        color_b: b.color_b.into(),
        scale: b.scale,
        speed: b.speed,
        intensity: b.intensity,
        seed: b.seed,
        pointer_response: b.pointer_response,
    })
}

/// Fills `out` with a glass preset (RUGI_GLASS_*).
#[no_mangle]
pub unsafe extern "C" fn rugi_glass_style_preset(preset: RugiGlassPreset, out: *mut RugiGlassStyle) -> RugiResult {
    guard(|| {
        let g = match preset {
            RUGI_GLASS_FROSTED => GlassStyle::frosted(),
            RUGI_GLASS_LIQUID => GlassStyle::liquid(),
            RUGI_GLASS_SMOKED => GlassStyle::smoked(),
            _ => return Err(FfiError::invalid(format!("unknown glass preset {preset}"))),
        };
        // SAFETY: `out` NULL or writable.
        *unsafe { out_mut(out, "out") }? = g.into();
        Ok(())
    })
}

/// Fills `out` with the default palette and parameters for `kind`.
#[no_mangle]
pub unsafe extern "C" fn rugi_backdrop_preset(kind: RugiBackdropKind, out: *mut RugiBackdrop) -> RugiResult {
    guard(|| {
        let b = Backdrop::new(backdrop_kind(kind)?);
        // SAFETY: `out` NULL or writable.
        *unsafe { out_mut(out, "out") }? = RugiBackdrop {
            kind,
            base: b.base.into(),
            color_a: b.color_a.into(),
            color_b: b.color_b.into(),
            scale: b.scale,
            speed: b.speed,
            intensity: b.intensity,
            seed: b.seed,
            pointer_response: b.pointer_response,
        };
        Ok(())
    })
}

/// Fills the whole window with an animated backdrop (call first in the frame).
#[no_mangle]
pub unsafe extern "C" fn rugi_backdrop(ui: *mut RugiUi, backdrop: *const RugiBackdrop) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`; `backdrop` NULL or readable.
        let (h, b) = unsafe { (handle(ui)?, in_ref(backdrop, "backdrop")?) };
        h.ui().backdrop(&backdrop_from(b)?);
        h.check()
    })
}

/// Paints an animated backdrop into `r` with rounded corners (cover art, banners).
#[no_mangle]
pub unsafe extern "C" fn rugi_backdrop_rect(
    ui: *mut RugiUi,
    r: RugiRect,
    radius: f32,
    backdrop: *const RugiBackdrop,
) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`; `backdrop` NULL or readable.
        let (h, b) = unsafe { (handle(ui)?, in_ref(backdrop, "backdrop")?) };
        let b = backdrop_from(b)?;
        h.ui().backdrop_rect(rect_from(r)?, radius.max(0.0), &b);
        h.check()
    })
}

/// Paints a glass surface over `r` (blurs everything drawn before it).
#[no_mangle]
pub unsafe extern "C" fn rugi_glass(ui: *mut RugiUi, r: RugiRect, style: *const RugiGlassStyle) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`; `style` NULL or readable.
        let (h, s) = unsafe { (handle(ui)?, in_ref(style, "style")?) };
        h.ui().glass(rect_from(r)?, &s.into());
        h.check()
    })
}

/// Starts a floating glass panel at `r`; add widgets, then rugi_end.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_glass_panel(
    ui: *mut RugiUi,
    r: RugiRect,
    style: *const RugiGlassStyle,
) -> RugiResult {
    // SAFETY: contract of `handle`; `style` NULL or readable.
    unsafe {
        begin_container(ui, |h| {
            let s = in_ref(style, "style")?;
            h.ui().begin_glass_panel(rect_from(r)?, &s.into());
            Ok(())
        })
    }
}

/// Starts a card with a glass surface sized to its content; close with rugi_end.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_glass_card(ui: *mut RugiUi, style: *const RugiGlassStyle) -> RugiResult {
    // SAFETY: contract of `handle`; `style` NULL or readable.
    unsafe {
        begin_container(ui, |h| {
            let s = in_ref(style, "style")?;
            h.ui().begin_glass_card(&s.into());
            Ok(())
        })
    }
}

/// Content until rugi_end fades/slides in whenever `key` changes (e.g. the page index).
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_transition(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    key: u64,
    slide: RugiSlide,
) -> RugiResult {
    // SAFETY: contracts of `handle` and `str_arg`.
    unsafe {
        begin_container(ui, |h| {
            let slide = match slide {
                RUGI_SLIDE_NONE => Slide::None,
                RUGI_SLIDE_FORWARD => Slide::Forward,
                RUGI_SLIDE_BACK => Slide::Back,
                RUGI_SLIDE_UP => Slide::Up,
                _ => return Err(FfiError::invalid(format!("unknown slide {slide}"))),
            };
            let id = str_arg(id, id_len, "id")?;
            h.ui().begin_transition(id, key, slide);
            Ok(())
        })
    }
}

/// Frame-rate budget for animated backdrops while the window is focused /
/// unfocused. 0 = the display refresh rate. Defaults: 0 and 30.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_set_ambient_fps(ui: *mut RugiUi, focused_fps: f32, unfocused_fps: f32) -> RugiResult {
    guard(|| {
        let limit = |fps: f32| {
            if !fps.is_finite() || fps < 0.0 {
                return Err(FfiError::invalid("fps must be finite and >= 0"));
            }
            Ok((fps > 0.0).then_some(fps))
        };
        let rate = AmbientRate { focused: limit(focused_fps)?, unfocused: limit(unfocused_fps)? };
        // SAFETY: contract of `handle`.
        unsafe { handle(ui) }?.ui().set_ambient_rate(rate);
        Ok(())
    })
}
