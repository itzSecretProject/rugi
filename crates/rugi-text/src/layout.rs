//! Shaped text and its cached glyph quads.

use cosmic_text::LayoutGlyph;
use rugi_core::Vec2;
use std::cell::RefCell;

/// A glyph positioned within a [`TextLayout`].
#[derive(Clone, Debug)]
pub(crate) struct PlacedGlyph {
    pub glyph: LayoutGlyph,
    pub line_y: f32,
}

/// The result of shaping a string: logical size plus positioned glyphs.
#[derive(Clone, Debug)]
pub struct TextLayout {
    pub(crate) size: Vec2,
    pub(crate) glyphs: Vec<PlacedGlyph>,
    pub(crate) text_len: usize,
    pub(crate) line_height: f32,
    /// Quads resolved for the last (scale, atlas generation) this layout was
    /// painted with. Text is placed on whole physical pixels, so the quads
    /// relative to the origin never change and steady-state painting needs no
    /// glyph hashing or atlas lookups.
    pub(crate) run: RefCell<Option<GlyphRun>>,
}

/// A glyph quad relative to the layout origin, in logical pixels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RunQuad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
}

#[derive(Clone, Debug)]
pub(crate) struct GlyphRun {
    pub scale_bits: u32,
    pub generation: u64,
    pub quads: Vec<RunQuad>,
}

impl TextLayout {
    /// Logical size (width of the longest line, total height).
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Height of one line.
    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    /// Number of glyphs.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// X offset of the caret placed before byte `index` (single-line text).
    pub fn caret_x(&self, index: usize) -> f32 {
        let mut end_x: f32 = 0.0;
        for g in &self.glyphs {
            let g = &g.glyph;
            if g.start == index {
                return g.x;
            }
            if index > g.start && index < g.end {
                // Inside a ligature/cluster: interpolate.
                let t = (index - g.start) as f32 / (g.end - g.start) as f32;
                return g.x + g.w * t;
            }
            end_x = end_x.max(g.x + g.w);
        }
        if index >= self.text_len {
            end_x
        } else {
            0.0
        }
    }

    /// Byte index of the caret position closest to logical `x` (single-line text).
    pub fn index_at_x(&self, x: f32) -> usize {
        for g in &self.glyphs {
            let g = &g.glyph;
            if x < g.x + g.w * 0.5 {
                return g.start;
            }
        }
        self.text_len
    }
}
