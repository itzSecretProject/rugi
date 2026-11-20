//! Plain data types shared across the API and their engine conversions.

use std::ffi::c_char;

use rugi_core::{Color, Rect};

use crate::error::FfiError;

/// An sRGB color with straight alpha.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RugiColor {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha.
    pub a: u8,
}

/// A rectangle in logical pixels.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RugiRect {
    /// Left.
    pub x: f32,
    /// Top.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

/// A borrowed UTF-8 string: `ptr` may be NULL only when `len` is 0.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RugiStr {
    /// UTF-8 bytes.
    pub ptr: *const c_char,
    /// Length in bytes.
    pub len: usize,
}

impl From<RugiColor> for Color {
    fn from(c: RugiColor) -> Color {
        Color::rgba(c.r, c.g, c.b, c.a)
    }
}

impl From<Color> for RugiColor {
    fn from(c: Color) -> RugiColor {
        RugiColor { r: c.r, g: c.g, b: c.b, a: c.a }
    }
}

impl From<Rect> for RugiRect {
    fn from(r: Rect) -> RugiRect {
        RugiRect { x: r.min.x, y: r.min.y, w: r.width(), h: r.height() }
    }
}

/// Validates a caller rectangle: finite, with non-negative size.
pub(crate) fn rect_from(r: RugiRect) -> Result<Rect, FfiError> {
    if [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) && r.w >= 0.0 && r.h >= 0.0 {
        Ok(Rect::from_xywh(r.x, r.y, r.w, r.h))
    } else {
        Err(FfiError::invalid("rect must be finite with non-negative size"))
    }
}
