//! Visual effects: frosted / liquid glass surfaces and animated procedural
//! backdrops. Both are single GPU instances evaluated in the fragment shader.
//!
//! Glass samples a blurred copy of everything drawn *before* it in the frame
//! (the renderer snapshots and blurs the frame when it reaches a glass
//! instance), then adds tint, refraction at the rim, chromatic dispersion,
//! a specular rim light and film grain.

use crate::Color;

/// Appearance of a glass surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassStyle {
    /// Blur strength, 0 (clear) .. 1 (heavily frosted).
    pub blur: f32,
    /// Tint composited over the blurred backdrop; alpha = tint strength.
    pub tint: Color,
    /// Refraction displacement at the rim, in logical pixels (0 = flat).
    pub refraction: f32,
    /// Width of the curved rim that refracts, in logical pixels.
    pub bevel: f32,
    /// Chromatic dispersion (0..1) of the refracted rim.
    pub dispersion: f32,
    /// Saturation multiplier for the backdrop (1 = unchanged).
    pub saturation: f32,
    /// Specular rim light strength (0..1).
    pub highlight: f32,
    /// Rim light color.
    pub highlight_color: Color,
    /// Film-grain amount (0..1); breaks up banding.
    pub grain: f32,
    /// Border width in logical pixels (0 = none).
    pub border_width: f32,
    /// Border color.
    pub border: Color,
    /// Corner radius.
    pub radius: f32,
}

impl Default for GlassStyle {
    fn default() -> Self {
        Self::frosted()
    }
}

impl GlassStyle {
    /// Soft frosted panel (Windows acrylic / macOS sidebar look).
    pub const fn frosted() -> Self {
        Self {
            blur: 0.7,
            tint: Color::rgba(18, 18, 24, 120),
            refraction: 0.0,
            bevel: 0.0,
            dispersion: 0.0,
            saturation: 1.25,
            highlight: 0.35,
            highlight_color: Color::rgb(255, 255, 255),
            grain: 0.25,
            border_width: 1.0,
            border: Color::rgba(255, 255, 255, 28),
            radius: 16.0,
        }
    }

    /// "Liquid glass": light blur, strong refraction and dispersion at the rim.
    pub const fn liquid() -> Self {
        Self {
            blur: 0.35,
            tint: Color::rgba(255, 255, 255, 14),
            refraction: 14.0,
            bevel: 18.0,
            dispersion: 0.6,
            saturation: 1.35,
            highlight: 0.8,
            highlight_color: Color::rgb(255, 255, 255),
            grain: 0.0,
            border_width: 1.0,
            border: Color::rgba(255, 255, 255, 40),
            radius: 22.0,
        }
    }

    /// Dark smoked glass for dense menus over busy backgrounds.
    pub const fn smoked() -> Self {
        Self {
            blur: 0.85,
            tint: Color::rgba(10, 10, 14, 185),
            refraction: 0.0,
            bevel: 0.0,
            dispersion: 0.0,
            saturation: 1.1,
            highlight: 0.2,
            highlight_color: Color::rgb(255, 255, 255),
            grain: 0.2,
            border_width: 1.0,
            border: Color::rgba(255, 255, 255, 18),
            radius: 14.0,
        }
    }

    /// Returns a copy with another radius.
    pub const fn with_radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    /// Returns a copy with another tint.
    pub const fn with_tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }

    /// Returns a copy with another blur strength.
    pub const fn with_blur(mut self, blur: f32) -> Self {
        self.blur = blur;
        self
    }
}

/// Procedural backdrop algorithms (all animated, all resolution independent).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum BackdropKind {
    /// Slow aurora curtains blending three colors.
    #[default]
    Aurora = 0,
    /// Domain-warped smoke / ink.
    Smoke = 1,
    /// Classic sine plasma.
    Plasma = 2,
    /// Drifting, twinkling star particles over a faint nebula.
    Starfield = 3,
    /// A glowing electric filament.
    Filament = 4,
    /// Moving mesh gradient with film grain.
    Mesh = 5,
}

impl BackdropKind {
    /// All kinds.
    pub const ALL: [BackdropKind; 6] = [
        BackdropKind::Aurora,
        BackdropKind::Smoke,
        BackdropKind::Plasma,
        BackdropKind::Starfield,
        BackdropKind::Filament,
        BackdropKind::Mesh,
    ];

    /// Converts from the C ABI integer.
    pub fn from_u32(v: u32) -> Option<BackdropKind> {
        BackdropKind::ALL.get(v as usize).copied()
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            BackdropKind::Aurora => "Aurora",
            BackdropKind::Smoke => "Smoke",
            BackdropKind::Plasma => "Plasma",
            BackdropKind::Starfield => "Starfield",
            BackdropKind::Filament => "Filament",
            BackdropKind::Mesh => "Mesh",
        }
    }
}

/// An animated procedural background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    /// Algorithm.
    pub kind: BackdropKind,
    /// Base (darkest) color.
    pub base: Color,
    /// First accent color.
    pub color_a: Color,
    /// Second accent color.
    pub color_b: Color,
    /// Feature scale (1 = default; larger = bigger shapes).
    pub scale: f32,
    /// Animation speed multiplier (0 = frozen).
    pub speed: f32,
    /// Brightness of the accents (0..2).
    pub intensity: f32,
    /// Variation seed.
    pub seed: f32,
    /// How much the backdrop reacts to the pointer (0..1).
    pub pointer_response: f32,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self::new(BackdropKind::Aurora)
    }
}

impl Backdrop {
    /// A backdrop with the default palette for `kind`.
    pub const fn new(kind: BackdropKind) -> Self {
        let (base, a, b) = match kind {
            BackdropKind::Aurora => (Color::hex(0x07080C), Color::hex(0x2B6CFF), Color::hex(0x19D3A2)),
            BackdropKind::Smoke => (Color::hex(0x050506), Color::hex(0x8A8C94), Color::hex(0xE6E7EB)),
            BackdropKind::Plasma => (Color::hex(0x0B0610), Color::hex(0xE0457B), Color::hex(0x5B4BFF)),
            BackdropKind::Starfield => (Color::hex(0x07080D), Color::hex(0xC9D6FF), Color::hex(0x3A4B8C)),
            BackdropKind::Filament => (Color::hex(0x120A0E), Color::hex(0xF06A93), Color::hex(0xFFE1EA)),
            BackdropKind::Mesh => (Color::hex(0x0C0D12), Color::hex(0xFF7A45), Color::hex(0x4C6FFF)),
        };
        Self {
            kind,
            base,
            color_a: a,
            color_b: b,
            scale: 1.0,
            speed: 1.0,
            intensity: 1.0,
            seed: 0.0,
            pointer_response: 0.35,
        }
    }

    /// Returns a copy with another palette.
    pub const fn with_colors(mut self, base: Color, a: Color, b: Color) -> Self {
        self.base = base;
        self.color_a = a;
        self.color_b = b;
        self
    }

    /// Returns a copy with another seed (different pattern, same style).
    pub const fn with_seed(mut self, seed: f32) -> Self {
        self.seed = seed;
        self
    }
}
