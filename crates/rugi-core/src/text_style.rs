//! Text style description shared by the theme, the text system and widgets.

/// Font weight (CSS-style numeric scale).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
    /// 400.
    pub const REGULAR: FontWeight = FontWeight(400);
    /// 500.
    pub const MEDIUM: FontWeight = FontWeight(500);
    /// 600.
    pub const SEMIBOLD: FontWeight = FontWeight(600);
}

/// Font family selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// The UI sans-serif family (bundled Inter by default).
    Sans,
    /// The monospace family (bundled JetBrains Mono).
    Mono,
    /// A font family registered at runtime (see `Ui::add_font`). The index is
    /// assigned by the text system in registration order.
    Custom(u32),
}

/// Describes how a run of text is shaped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Font size in logical pixels.
    pub size: f32,
    /// Line height as a multiple of `size`.
    pub line_height: f32,
    /// Weight.
    pub weight: FontWeight,
    /// Family.
    pub family: FontFamily,
    /// Extra letter spacing in em (may be negative).
    pub letter_spacing: f32,
}

impl TextStyle {
    /// A regular-weight sans style at `size`.
    #[inline]
    pub const fn new(size: f32) -> Self {
        Self { size, line_height: 1.35, weight: FontWeight::REGULAR, family: FontFamily::Sans, letter_spacing: 0.0 }
    }

    /// Returns the style with a different weight.
    #[inline]
    pub const fn weight(mut self, w: FontWeight) -> Self {
        self.weight = w;
        self
    }

    /// Returns the style with different letter spacing (em).
    #[inline]
    pub const fn letter_spacing(mut self, em: f32) -> Self {
        self.letter_spacing = em;
        self
    }

    /// Line height in logical pixels.
    #[inline]
    pub fn line_px(&self) -> f32 {
        (self.size * self.line_height).round()
    }
}
