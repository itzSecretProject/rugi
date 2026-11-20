//! Font registration and selection.

use std::ffi::c_char;

use rugi_core::FontFamily;

use crate::args::{opt_out, slice_arg, str_arg};
use crate::error::{guard, guard_value, FfiError, RugiResult, RUGI_ERROR_NULL_POINTER};
use crate::handle::{handle, with_handle, RugiUi};

/// Font handle: RUGI_FONT_UI, RUGI_FONT_MONO, or a value from rugi_ui_add_font.
pub type RugiFont = u32;
/// The UI font (bundled Inter unless changed).
pub const RUGI_FONT_UI: RugiFont = 0;
/// Monospace (bundled JetBrains Mono).
pub const RUGI_FONT_MONO: RugiFont = 1;

/// Handles of registered fonts start after the two built-in ones.
const FIRST_CUSTOM_FONT: RugiFont = 2;

pub(crate) fn font_from(f: RugiFont) -> FontFamily {
    match f {
        RUGI_FONT_UI => FontFamily::Sans,
        RUGI_FONT_MONO => FontFamily::Mono,
        n => FontFamily::Custom(n - FIRST_CUSTOM_FONT),
    }
}

fn font_to(f: FontFamily) -> RugiFont {
    match f {
        FontFamily::Sans => RUGI_FONT_UI,
        FontFamily::Mono => RUGI_FONT_MONO,
        FontFamily::Custom(i) => i + FIRST_CUSTOM_FONT,
    }
}

/// Registers a TTF/OTF/TTC font from memory (the bytes are copied). On success
/// `*out_font` receives its handle. Fonts of the same family share a handle;
/// registered fonts also serve as fallback for missing glyphs (icon fonts).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_add_font(
    ui: *mut RugiUi,
    data: *const u8,
    len: usize,
    out_font: *mut RugiFont,
) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        let h = unsafe { handle(ui) }?;
        if data.is_null() || len == 0 {
            return Err(FfiError::new(RUGI_ERROR_NULL_POINTER, "font data is NULL or empty"));
        }
        // SAFETY: `out_font` is NULL or writable; `data` has `len` readable bytes.
        let (out, bytes) = unsafe { (opt_out(out_font, "out_font")?, slice_arg(data, len, "data")?) };
        let family = h.ui().add_font(bytes.to_vec()).map_err(|e| FfiError::invalid(e.to_string()))?;
        if let Some(out) = out {
            *out = font_to(family);
        }
        Ok(())
    })
}

/// Registers a font file (UTF-8 path).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_add_font_file(
    ui: *mut RugiUi,
    path: *const c_char,
    path_len: usize,
    out_font: *mut RugiFont,
) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, path) = unsafe { (handle(ui)?, str_arg(path, path_len, "path")?) };
        // SAFETY: `out_font` is NULL or writable.
        let out = unsafe { opt_out(out_font, "out_font") }?;
        let family = h.ui().add_font_file(path).map_err(|e| FfiError::invalid(e.to_string()))?;
        if let Some(out) = out {
            *out = font_to(family);
        }
        Ok(())
    })
}

/// Handle of the bundled icon font (Lucide), or RUGI_FONT_UI if disabled.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_icon_font(ui: *mut RugiUi) -> RugiFont {
    guard_value(RUGI_FONT_UI, || {
        // SAFETY: contract of `handle`.
        Ok(unsafe { handle(ui) }?.ui().icon_font().map_or(RUGI_FONT_UI, font_to))
    })
}

/// Uses `font` for all UI text and multiplies text sizes by `scale` (1 = no
/// change; must be finite and > 0).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_set_font(ui: *mut RugiUi, font: RugiFont, scale: f32) -> RugiResult {
    guard(|| {
        if !(scale.is_finite() && scale > 0.0) {
            return Err(FfiError::invalid("font scale must be finite and > 0"));
        }
        // SAFETY: contract of `handle`.
        let t = unsafe { handle(ui) }?.ui().theme_mut();
        t.set_font_family(font_from(font));
        t.scale_text(scale);
        Ok(())
    })
}

/// Uses `font` for following text until rugi_pop_style.
#[no_mangle]
pub unsafe extern "C" fn rugi_push_font(ui: *mut RugiUi, font: RugiFont) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe {
        with_handle(ui, |h| {
            h.require_frame()?;
            h.ui().push_font(font_from(font));
            Ok(())
        })
    }
}
