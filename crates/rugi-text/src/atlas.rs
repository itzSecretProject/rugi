//! Shelf-packed single-channel glyph atlas with dirty-rectangle tracking.

use cosmic_text::{CacheKey, SwashContent, SwashImage};
use rugi_core::id::FastMap;
use rugi_core::AtlasUpdate;

/// Gap between packed glyphs so bilinear sampling never bleeds across.
const PADDING: u32 = 1;
const MAX_SIZE: u32 = 4096;

/// A rasterized glyph's location in the atlas and its bearing.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AtlasGlyph {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub left: i32,
    pub top: i32,
}

impl AtlasGlyph {
    /// Normalized `[u0, v0, u1, v1]` in an atlas of edge length `atlas_size`.
    #[inline]
    pub fn uv(&self, atlas_size: u32) -> [f32; 4] {
        let size = atlas_size as f32;
        [self.x as f32 / size, self.y as f32 / size, (self.x + self.w) as f32 / size, (self.y + self.h) as f32 / size]
    }
}

#[derive(Clone, Copy, Debug)]
struct Shelf {
    y: u32,
    h: u32,
    x: u32,
}

/// CPU-side glyph atlas. The renderer mirrors it in an `R8Unorm` texture.
#[derive(Debug)]
pub struct GlyphAtlas {
    size: u32,
    data: Vec<u8>,
    shelves: Vec<Shelf>,
    next_y: u32,
    map: FastMap<CacheKey, Option<AtlasGlyph>>,
    dirty: Option<[u32; 4]>, // x0, y0, x1, y1
    pending_reset: bool,
    /// Incremented whenever existing atlas contents become invalid; cached
    /// glyph runs compare against it.
    generation: u64,
}

impl GlyphAtlas {
    /// Creates an empty atlas of `size`×`size` pixels.
    pub fn new(size: u32) -> Self {
        Self {
            size,
            data: vec![0; (size * size) as usize],
            shelves: Vec::new(),
            next_y: 0,
            map: FastMap::default(),
            dirty: Some([0, 0, size, size]),
            pending_reset: false,
            generation: 0,
        }
    }

    /// Atlas edge length in pixels.
    pub fn size(&self) -> u32 {
        self.size
    }

    pub(crate) fn reset_pending(&self) -> bool {
        self.pending_reset
    }

    pub(crate) fn mark_all_dirty(&mut self) {
        self.dirty = Some([0, 0, self.size, self.size]);
    }

    /// Applies a reset requested by an overflow during the previous frame:
    /// grows (up to [`MAX_SIZE`]²) and clears. Done between frames so glyphs already
    /// emitted keep valid UVs for the frame that referenced them.
    pub(crate) fn apply_pending_reset(&mut self) {
        if !self.pending_reset {
            return;
        }
        let size = (self.size * 2).min(MAX_SIZE);
        let generation = self.generation + 1;
        *self = GlyphAtlas::new(size);
        self.generation = generation;
    }

    /// Current atlas generation (changes when UVs of existing glyphs change).
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn get_or_insert(
        &mut self,
        key: CacheKey,
        rasterize: impl FnOnce() -> Option<SwashImage>,
    ) -> Option<AtlasGlyph> {
        if let Some(e) = self.map.get(&key) {
            return *e;
        }
        if self.pending_reset {
            return None;
        }
        let entry = match rasterize() {
            Some(img) => match self.insert_image(&img) {
                Ok(e) => e,
                Err(()) => {
                    self.pending_reset = true;
                    return None;
                }
            },
            None => None,
        };
        self.map.insert(key, entry);
        entry
    }

    fn insert_image(&mut self, img: &SwashImage) -> Result<Option<AtlasGlyph>, ()> {
        let w = img.placement.width;
        let h = img.placement.height;
        if w == 0 || h == 0 {
            return Ok(Some(AtlasGlyph { x: 0, y: 0, w: 0, h: 0, left: 0, top: 0 }));
        }
        let (x, y) = self.allocate(w + PADDING, h + PADDING).ok_or(())?;
        for row in 0..h {
            for col in 0..w {
                let i = (row * w + col) as usize;
                let v = match img.content {
                    SwashContent::Mask => img.data[i],
                    // The atlas is single-channel: color glyphs (emoji) keep
                    // only their coverage.
                    SwashContent::Color => img.data[i * 4 + 3],
                    SwashContent::SubpixelMask => {
                        let p = &img.data[i * 4..i * 4 + 3];
                        ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u8
                    }
                };
                self.data[((y + row) * self.size + x + col) as usize] = v;
            }
        }
        let r = [x, y, x + w, y + h];
        self.dirty = Some(match self.dirty {
            Some(d) => [d[0].min(r[0]), d[1].min(r[1]), d[2].max(r[2]), d[3].max(r[3])],
            None => r,
        });
        Ok(Some(AtlasGlyph { x, y, w, h, left: img.placement.left, top: img.placement.top }))
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w > self.size || h > self.size {
            return None;
        }
        // Best-fitting existing shelf.
        let mut best: Option<usize> = None;
        for (i, s) in self.shelves.iter().enumerate() {
            let fits = s.h >= h && s.h <= h + h / 2 + 2 && s.x + w <= self.size;
            if fits && best.is_none_or(|b| self.shelves[b].h > s.h) {
                best = Some(i);
            }
        }
        if let Some(i) = best {
            let s = &mut self.shelves[i];
            let pos = (s.x, s.y);
            s.x += w;
            return Some(pos);
        }
        if self.next_y + h > self.size {
            return None;
        }
        let shelf = Shelf { y: self.next_y, h, x: w };
        self.next_y += h;
        self.shelves.push(shelf);
        Some((0, shelf.y))
    }

    /// Returns pixels changed since the last call.
    pub fn take_update(&mut self) -> Option<AtlasUpdate> {
        let d = self.dirty.take()?;
        let (x0, y0, x1, y1) = (d[0], d[1], d[2], d[3]);
        let (w, h) = (x1 - x0, y1 - y0);
        let mut data = Vec::with_capacity((w * h) as usize);
        for row in y0..y1 {
            let start = (row * self.size + x0) as usize;
            data.extend_from_slice(&self.data[start..start + w as usize]);
        }
        Some(AtlasUpdate { size: self.size, region: [x0, y0, w, h], data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shelves_pack_and_overflow() {
        let mut a = GlyphAtlas::new(64);
        let mut n = 0;
        while a.allocate(9, 9).is_some() {
            n += 1;
            assert!(n < 100);
        }
        assert_eq!(n, 49); // 7 per shelf, 7 shelves
    }
}
