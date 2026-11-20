//! sRGB colors with straight (non-premultiplied) alpha.
//!
//! Rugi blends in sRGB ("gamma") space by default, matching how design tools
//! such as Figma composite. Renderers targeting an sRGB swapchain convert in the
//! shader so the visible result is identical.

/// An 8-bit-per-channel sRGB color with straight alpha.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (0 = transparent).
    pub a: u8,
}

impl Color {
    /// Fully transparent.
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    /// Opaque white.
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    /// Opaque black.
    pub const BLACK: Color = Color::rgb(0, 0, 0);

    /// Opaque color from components.
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Color from components including alpha.
    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Opaque color from a `0xRRGGBB` literal.
    #[inline]
    pub const fn hex(rgb: u32) -> Self {
        Self::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    }

    /// Color from a `0xRRGGBBAA` literal.
    #[inline]
    pub const fn hex_rgba(rgba: u32) -> Self {
        Self::rgba((rgba >> 24) as u8, (rgba >> 16) as u8, (rgba >> 8) as u8, rgba as u8)
    }

    /// Returns the color with alpha replaced (`a` in 0..=1).
    #[inline]
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a: (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8, ..self }
    }

    /// Returns the color with alpha multiplied by `f` (0..=1).
    #[inline]
    pub fn fade(self, f: f32) -> Self {
        Self { a: (self.a as f32 * f.clamp(0.0, 1.0) + 0.5) as u8, ..self }
    }

    /// Linear interpolation (in sRGB space) between two colors, `t` in 0..=1.
    #[inline]
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t + 0.5) as u8;
        Color { r: l(self.r, o.r), g: l(self.g, o.g), b: l(self.b, o.b), a: l(self.a, o.a) }
    }

    /// Mixes towards white by `t`, keeping alpha.
    #[inline]
    pub fn lighten(self, t: f32) -> Color {
        let a = self.a;
        Color { a, ..self.lerp(Color { a, ..Color::WHITE }, t) }
    }

    /// Mixes towards black by `t`, keeping alpha.
    #[inline]
    pub fn darken(self, t: f32) -> Color {
        let a = self.a;
        Color { a, ..self.lerp(Color { a, ..Color::BLACK }, t) }
    }

    /// Packs as little-endian `[r, g, b, a]` bytes into a `u32`, matching the
    /// `Unorm8x4` vertex format used by the GPU instance layout.
    #[inline]
    pub const fn to_packed(self) -> u32 {
        u32::from_le_bytes([self.r, self.g, self.b, self.a])
    }

    /// Hue (0..360), saturation and value (0..=1) of the sRGB color.
    #[inline]
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d <= 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / d).rem_euclid(6.0))
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        let s = if max <= 0.0 { 0.0 } else { d / max };
        (h, s, max)
    }

    /// Opaque color from hue (degrees), saturation and value (0..=1).
    #[inline]
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Color {
        let h = h.rem_euclid(360.0);
        let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let to = |f: f32| ((f + m) * 255.0 + 0.5) as u8;
        Color::rgb(to(r), to(g), to(b))
    }

    /// Relative luminance approximation in 0..=1 (sRGB weights, no linearization).
    #[inline]
    pub fn luma(self) -> f32 {
        (0.2126 * self.r as f32 + 0.7152 * self.g as f32 + 0.0722 * self.b as f32) / 255.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_pack() {
        let c = Color::hex(0x5B8CFF);
        assert_eq!((c.r, c.g, c.b, c.a), (0x5B, 0x8C, 0xFF, 255));
        assert_eq!(c.to_packed().to_le_bytes(), [0x5B, 0x8C, 0xFF, 0xFF]);
        assert_eq!(Color::hex_rgba(0x01020304), Color::rgba(1, 2, 3, 4));
    }

    #[test]
    fn hsv_round_trip() {
        for c in
            [Color::hex(0x6D8BFF), Color::hex(0xF7648E), Color::hex(0x3DD68C), Color::rgb(128, 128, 128), Color::BLACK]
        {
            let (h, s, v) = c.to_hsv();
            let back = Color::from_hsv(h, s, v);
            let close = |a: u8, b: u8| (a as i16 - b as i16).abs() <= 1;
            assert!(close(c.r, back.r) && close(c.g, back.g) && close(c.b, back.b), "{c:?} -> {back:?}");
        }
        assert_eq!(Color::from_hsv(0.0, 1.0, 1.0), Color::rgb(255, 0, 0));
        assert_eq!(Color::from_hsv(240.0, 1.0, 1.0), Color::rgb(0, 0, 255));
    }

    #[test]
    fn lerp_endpoints() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(200, 100, 50);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(Color::WHITE.with_alpha(0.5).a, 128);
    }
}
