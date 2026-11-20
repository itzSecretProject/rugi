//! Rugi text system.
//!
//! * Shaping and line breaking: [`cosmic_text`] (HarfRust shaping, BiDi,
//!   font fallback), with the Inter typeface bundled for a consistent look on
//!   every platform.
//! * A layout cache keyed by (text, style, wrap width) so unchanged labels are
//!   never re-shaped; entries unused for a while are evicted.
//! * A single-channel glyph atlas with shelf packing and incremental (dirty
//!   rectangle) uploads. Glyphs are rasterized at the exact physical pixel size
//!   (DPI-aware) and positioned on the physical pixel grid for crisp text.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod atlas;
mod layout;

pub use atlas::GlyphAtlas;
use cosmic_text::{
    Attrs, Buffer, Family, FeatureTag, FontFeatures, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap,
};
pub use layout::TextLayout;
use layout::{GlyphRun, PlacedGlyph, RunQuad};
use rugi_core::id::{hash_bytes, IdMap};
use rugi_core::{AtlasUpdate, Color, DrawList, FontFamily, Rect, TextStyle, Vec2};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const INTER_REGULAR: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../assets/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/Inter-SemiBold.ttf");
const MONO_REGULAR: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");
const MONO_MEDIUM: &[u8] = include_bytes!("../assets/JetBrainsMono-Medium.ttf");

/// Name of the bundled monospace family.
pub const MONO_FAMILY: &str = "JetBrains Mono";

/// Name of the bundled UI family.
pub const UI_FAMILY: &str = "Inter";

/// Start of the Unicode Private Use Area, where icon fonts place their glyphs.
const PUA_START: u32 = 0xE000;
/// Length of the Private Use Area (U+E000..=U+F8FF).
const PUA_LEN: usize = 0x1900;

/// Frames a cached layout may stay unused before eviction.
const LAYOUT_TTL_FRAMES: u64 = 120;
/// How often (in frames) the layout cache is scanned for stale entries.
const EVICTION_INTERVAL_FRAMES: u64 = 60;
/// Edge length of a new glyph atlas; it doubles on overflow.
const INITIAL_ATLAS_SIZE: u32 = 1024;
/// Font sizes and line heights are shaped within this range (logical px);
/// cosmic-text rejects zero metrics and huge sizes make giant glyph bitmaps.
const TEXT_PX_RANGE: (f32, f32) = (1.0, 512.0);
/// Slack around a layout's bounds for glyphs that overhang their advance box.
const GLYPH_OVERHANG: f32 = 8.0;

/// Errors from font registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    /// The data is not a font file fontdb/swash can parse.
    InvalidFont,
    /// Reading a font file failed.
    Io(String),
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::InvalidFont => write!(f, "data is not a supported font (TTF/OTF/TTC)"),
            FontError::Io(e) => write!(f, "could not read font file: {e}"),
        }
    }
}

impl std::error::Error for FontError {}

/// Options for creating a [`TextSystem`].
#[derive(Clone, Debug, Default)]
pub struct TextOptions {
    /// Also load the operating system's fonts (enables fallback for scripts and
    /// symbols Inter does not cover, and the `Mono` family). Costs startup time.
    pub load_system_fonts: bool,
    /// Additional font files (TTF/OTF bytes) to register.
    pub extra_fonts: Vec<Vec<u8>>,
}

struct CacheEntry {
    text: String,
    style: TextStyle,
    wrap_bits: u32,
    layout: Rc<TextLayout>,
    last_used: u64,
}

/// Owns fonts, the shaping cache and the glyph atlas.
pub struct TextSystem {
    fonts: FontSystem,
    swash: SwashCache,
    scratch: Buffer,
    cache: IdMap<u64, CacheEntry>,
    atlas: GlyphAtlas,
    /// Family names of fonts registered at runtime; index = `FontFamily::Custom`.
    families: Vec<String>,
    icon_family: Option<FontFamily>,
    /// Private-use codepoints covered by the bundled icon font, indexed from
    /// [`PUA_START`]. Inter and JetBrains Mono also define private glyphs at
    /// some of these codepoints, so these are pinned to the icon font when
    /// shaping instead of relying on fallback order.
    icon_cps: Vec<bool>,
    frame: u64,
    shaped_this_frame: u32,
    rasterized_this_frame: u32,
}

impl std::fmt::Debug for TextSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextSystem").field("cached_layouts", &self.cache.len()).finish_non_exhaustive()
    }
}

impl TextSystem {
    /// Creates a text system with the bundled Inter fonts.
    pub fn new(options: &TextOptions) -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        db.load_font_data(INTER_REGULAR.to_vec());
        db.load_font_data(INTER_MEDIUM.to_vec());
        db.load_font_data(INTER_SEMIBOLD.to_vec());
        db.load_font_data(MONO_REGULAR.to_vec());
        db.load_font_data(MONO_MEDIUM.to_vec());
        for f in &options.extra_fonts {
            db.load_font_data(f.clone());
        }
        if options.load_system_fonts {
            db.load_system_fonts();
        }
        db.set_sans_serif_family(UI_FAMILY);
        db.set_monospace_family(MONO_FAMILY);
        let mut fonts = FontSystem::new_with_locale_and_db("en-US".to_string(), db);
        let scratch = Buffer::new(&mut fonts, Metrics::new(14.0, 18.0));
        let mut ts = Self {
            fonts,
            swash: SwashCache::new(),
            scratch,
            cache: IdMap::default(),
            atlas: GlyphAtlas::new(INITIAL_ATLAS_SIZE),
            families: Vec::new(),
            icon_family: None,
            icon_cps: Vec::new(),
            frame: 0,
            shaped_this_frame: 0,
            rasterized_this_frame: 0,
        };
        #[cfg(feature = "lucide")]
        ts.register_lucide();
        ts
    }

    #[cfg(feature = "lucide")]
    fn register_lucide(&mut self) {
        self.icon_family = self.add_font(rugi_icons::LUCIDE_TTF.to_vec()).ok();
        self.icon_cps = vec![false; PUA_LEN];
        for (_, glyph) in rugi_icons::lucide::ALL {
            let Some(c) = glyph.chars().next() else { continue };
            if let Some(slot) = (c as u32).checked_sub(PUA_START).and_then(|i| self.icon_cps.get_mut(i as usize)) {
                *slot = true;
            }
        }
    }

    /// Registers a font (TTF/OTF/TTC bytes) and returns its family. Fonts of
    /// the same family (e.g. Regular + Bold files) share one handle; weights are
    /// selected through [`TextStyle::weight`]. Registered fonts also act as
    /// fallback for characters missing from the style's family, so icon fonts
    /// work inside any label ("merge mode").
    pub fn add_font(&mut self, data: Vec<u8>) -> Result<FontFamily, FontError> {
        let ids = self.fonts.db_mut().load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(data)));
        let name = ids
            .first()
            .and_then(|id| self.fonts.db().face(*id))
            .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
            .ok_or(FontError::InvalidFont)?;
        // New fonts can change fallback results: drop cached layouts.
        self.cache.clear();
        Ok(self.family_handle(&name))
    }

    /// Reads and registers a font file.
    pub fn add_font_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<FontFamily, FontError> {
        let data = std::fs::read(path).map_err(|e| FontError::Io(e.to_string()))?;
        self.add_font(data)
    }

    /// Handle for an already-registered family name (e.g. a system font when
    /// `load_system_fonts` is enabled). Returns a handle even if no face with
    /// that name exists; shaping then falls back.
    pub fn family_handle(&mut self, name: &str) -> FontFamily {
        let idx = match self.families.iter().position(|f| f.eq_ignore_ascii_case(name)) {
            Some(i) => i,
            None => {
                self.families.push(name.to_owned());
                self.families.len() - 1
            }
        };
        FontFamily::Custom(idx as u32)
    }

    /// Family name behind a handle.
    pub fn family_name(&self, family: FontFamily) -> &str {
        resolve_family(&self.families, family)
    }

    /// The bundled icon font family (Lucide), if the `lucide` feature is on.
    pub fn icon_family(&self) -> Option<FontFamily> {
        self.icon_family
    }

    /// Starts a frame: evicts stale layouts and applies a pending atlas reset.
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.shaped_this_frame = 0;
        self.rasterized_this_frame = 0;
        if self.frame.is_multiple_of(EVICTION_INTERVAL_FRAMES) {
            let frame = self.frame;
            self.cache.retain(|_, e| frame - e.last_used <= LAYOUT_TTL_FRAMES);
        }
        self.atlas.apply_pending_reset();
    }

    /// Layouts shaped (cache misses) since `begin_frame`.
    pub fn shaped_this_frame(&self) -> u32 {
        self.shaped_this_frame
    }

    /// Glyphs rasterized since `begin_frame`.
    pub fn rasterized_this_frame(&self) -> u32 {
        self.rasterized_this_frame
    }

    /// True if the atlas overflowed this frame (some glyphs were skipped and a
    /// repaint is needed after the atlas grows).
    pub fn atlas_overflowed(&self) -> bool {
        self.atlas.reset_pending()
    }

    /// Takes the pending atlas upload, if any.
    pub fn take_atlas_update(&mut self) -> Option<AtlasUpdate> {
        self.atlas.take_update()
    }

    /// Forces the next atlas update to contain the whole atlas (e.g. after the
    /// renderer lost its texture).
    pub fn invalidate_atlas_upload(&mut self) {
        self.atlas.mark_all_dirty();
    }

    /// Shapes `text` (cached). `wrap_width` enables word wrapping.
    pub fn layout(&mut self, text: &str, style: &TextStyle, wrap_width: Option<f32>) -> Rc<TextLayout> {
        let wrap_bits = wrap_width.map(|w| w.max(1.0).floor().to_bits()).unwrap_or(0);
        let mut meta = [0u8; 24];
        meta[0..4].copy_from_slice(&style.size.to_bits().to_le_bytes());
        meta[4..8].copy_from_slice(&style.line_height.to_bits().to_le_bytes());
        meta[8..12].copy_from_slice(&style.letter_spacing.to_bits().to_le_bytes());
        meta[12..14].copy_from_slice(&style.weight.0.to_le_bytes());
        meta[14..18].copy_from_slice(&family_key(style.family).to_le_bytes());
        meta[18..22].copy_from_slice(&wrap_bits.to_le_bytes());
        let key = hash_bytes(hash_bytes(0, &meta), text.as_bytes());
        if let Some(e) = self.cache.get_mut(&key) {
            if e.text == text && e.style == *style && e.wrap_bits == wrap_bits {
                e.last_used = self.frame;
                return e.layout.clone();
            }
        }
        let layout = Rc::new(self.shape(text, style, wrap_width));
        self.shaped_this_frame += 1;
        self.cache.insert(
            key,
            CacheEntry {
                text: text.to_owned(),
                style: *style,
                wrap_bits,
                layout: layout.clone(),
                last_used: self.frame,
            },
        );
        layout
    }

    fn shape(&mut self, text: &str, style: &TextStyle, wrap_width: Option<f32>) -> TextLayout {
        let (lo, hi) = TEXT_PX_RANGE;
        let sane = |v: f32| if v.is_nan() { lo } else { v.clamp(lo, hi) };
        let line_height = sane(style.line_px());
        let metrics = Metrics::new(sane(style.size), line_height);
        let family = Family::Name(resolve_family(&self.families, style.family));
        let spacing = if style.letter_spacing.is_finite() { style.letter_spacing } else { 0.0 };
        let mut attrs = Attrs::new().family(family).weight(Weight(style.weight.0)).letter_spacing(spacing);
        if style.family == FontFamily::Mono {
            // Code reads as typed: `=>` and `>=` stay two characters.
            let mut features = FontFeatures::new();
            features.disable(FeatureTag::CONTEXTUAL_ALTERNATES).disable(FeatureTag::STANDARD_LIGATURES);
            attrs = attrs.font_features(features);
        }
        let buf = &mut self.scratch;
        buf.set_metrics(metrics);
        buf.set_wrap(if wrap_width.is_some() { Wrap::WordOrGlyph } else { Wrap::None });
        buf.set_size(wrap_width, None);
        let is_icon = |c: char| {
            (c as u32).checked_sub(PUA_START).and_then(|i| self.icon_cps.get(i as usize)).copied().unwrap_or(false)
        };
        match self.icon_family.map(|f| resolve_family(&self.families, f)) {
            Some(icon_name) if text.chars().any(is_icon) => {
                // Split into runs so icon codepoints are shaped with the icon font.
                let icon_attrs = attrs.clone().family(Family::Name(icon_name)).weight(Weight(400));
                let mut spans: Vec<(&str, Attrs)> = Vec::new();
                let mut start = 0;
                let mut cur_icon = None;
                for (i, c) in text.char_indices() {
                    let ic = is_icon(c);
                    if cur_icon.is_some_and(|prev| prev != ic) {
                        spans.push((
                            &text[start..i],
                            if cur_icon == Some(true) { icon_attrs.clone() } else { attrs.clone() },
                        ));
                        start = i;
                    }
                    cur_icon = Some(ic);
                }
                spans.push((&text[start..], if cur_icon == Some(true) { icon_attrs.clone() } else { attrs.clone() }));
                buf.set_rich_text(spans, &attrs, Shaping::Advanced, None);
            }
            _ => buf.set_text(text, &attrs, Shaping::Advanced, None),
        }
        buf.shape_until_scroll(&mut self.fonts, false);

        let mut glyphs = Vec::new();
        let mut width: f32 = 0.0;
        let mut lines = 0usize;
        for run in buf.layout_runs() {
            lines += 1;
            width = width.max(run.line_w);
            for g in run.glyphs {
                glyphs.push(PlacedGlyph { glyph: g.clone(), line_y: run.line_y });
            }
        }
        TextLayout {
            size: Vec2::new(width.ceil(), line_height * lines.max(1) as f32),
            glyphs,
            text_len: text.len(),
            line_height,
            run: RefCell::new(None),
        }
    }

    /// Emits glyph quads for `layout` with its top-left at `pos` (logical).
    /// `scale` is the physical-pixels-per-logical-pixel factor.
    pub fn paint(&mut self, dl: &mut DrawList, layout: &TextLayout, pos: Vec2, color: Color, scale: f32) {
        if color.a == 0 || layout.glyphs.is_empty() || !(scale.is_finite() && scale > 0.0) {
            return;
        }
        let clip = dl.clip();
        let bounds = Rect::from_min_size(pos, layout.size).expand(GLYPH_OVERHANG);
        if !bounds.intersects(&clip) {
            dl.add_culled(layout.glyphs.len() as u32);
            return;
        }
        let scale_bits = scale.to_bits();
        let generation = self.atlas.generation();
        let fresh =
            matches!(&*layout.run.borrow(), Some(r) if r.scale_bits == scale_bits && r.generation == generation);
        if !fresh {
            if let Some(run) = self.build_run(layout, scale) {
                *layout.run.borrow_mut() = Some(run);
            } else {
                // The atlas is full this frame: paint what fits, retry next frame.
                self.paint_uncached(dl, layout, pos, color, scale);
                return;
            }
        }
        // Origin snapped to the physical pixel grid, back in logical units.
        let inv = 1.0 / scale;
        let ox = (pos.x * scale).round() * inv;
        let oy = (pos.y * scale).round() * inv;
        let fully_visible = clip.min.x <= bounds.min.x
            && clip.min.y <= bounds.min.y
            && clip.max.x >= bounds.max.x
            && clip.max.y >= bounds.max.y;
        let run = layout.run.borrow();
        let quads = &run.as_ref().expect("built or validated above").quads;
        dl.glyph_run(quads.iter().map(|q| (q.rect, q.uv)), ox, oy, color, fully_visible);
    }

    /// Resolves every glyph of `layout` at `scale` against the atlas,
    /// relative to a pixel-aligned origin. Returns `None` if the atlas filled up.
    fn build_run(&mut self, layout: &TextLayout, scale: f32) -> Option<GlyphRun> {
        // A large positive reference origin keeps cosmic-text's truncation
        // equivalent to floor for every glyph; it is subtracted again below.
        const REF: f32 = 8192.0;
        let inv = 1.0 / scale;
        let mut quads = Vec::with_capacity(layout.glyphs.len());
        for pg in &layout.glyphs {
            let line_y = REF + (pg.line_y * scale).round();
            let phys = pg.glyph.physical((REF, line_y), scale);
            let fonts = &mut self.fonts;
            let swash = &mut self.swash;
            let mut rasterized = false;
            let entry = self.atlas.get_or_insert(phys.cache_key, || {
                rasterized = true;
                swash.get_image_uncached(fonts, phys.cache_key)
            });
            if rasterized {
                self.rasterized_this_frame += 1;
            }
            if self.atlas.reset_pending() {
                return None;
            }
            let Some(e) = entry.filter(|e| e.w > 0 && e.h > 0) else { continue };
            let x = (phys.x + e.left) as f32 - REF;
            let y = (phys.y - e.top) as f32 - REF;
            quads.push(RunQuad {
                rect: [x * inv, y * inv, e.w as f32 * inv, e.h as f32 * inv],
                uv: e.uv(self.atlas.size()),
            });
        }
        Some(GlyphRun { scale_bits: scale.to_bits(), generation: self.atlas.generation(), quads })
    }

    /// Per-glyph path used only while the atlas is overflowing.
    fn paint_uncached(&mut self, dl: &mut DrawList, layout: &TextLayout, pos: Vec2, color: Color, scale: f32) {
        let ox = (pos.x * scale).round();
        let oy = (pos.y * scale).round();
        let inv = 1.0 / scale;
        for pg in &layout.glyphs {
            let line_y = oy + (pg.line_y * scale).round();
            let phys = pg.glyph.physical((ox, line_y), scale);
            let fonts = &mut self.fonts;
            let swash = &mut self.swash;
            let entry = self.atlas.get_or_insert(phys.cache_key, || swash.get_image_uncached(fonts, phys.cache_key));
            let Some(e) = entry.filter(|e| e.w > 0 && e.h > 0) else { continue };
            let rect = Rect::from_xywh(
                (phys.x + e.left) as f32 * inv,
                (phys.y - e.top) as f32 * inv,
                e.w as f32 * inv,
                e.h as f32 * inv,
            );
            dl.glyph(rect, e.uv(self.atlas.size()), color);
        }
    }
}

/// Cache-key discriminant of a family handle.
fn family_key(f: FontFamily) -> u32 {
    match f {
        FontFamily::Sans => 0,
        FontFamily::Mono => 1,
        FontFamily::Custom(i) => 2 + i,
    }
}

/// Family name behind a handle; unknown custom handles resolve to the UI family.
fn resolve_family(families: &[String], family: FontFamily) -> &str {
    match family {
        FontFamily::Sans => UI_FAMILY,
        FontFamily::Mono => MONO_FAMILY,
        FontFamily::Custom(i) => families.get(i as usize).map(String::as_str).unwrap_or(UI_FAMILY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rugi_core::FontWeight;

    #[test]
    fn degenerate_metrics_shape_without_panicking() {
        let mut ts = TextSystem::new(&TextOptions::default());
        ts.begin_frame();
        let mut dl = DrawList::new();
        for (size, line_height) in [
            (0.1, 1.5),
            (0.0, 1.35),
            (-5.0, 1.0),
            (14.0, 0.0),
            (14.0, -1.0),
            (f32::NAN, 1.0),
            (14.0, f32::NAN),
            (f32::INFINITY, 1.0),
            (1.0e30, 1.0e30),
        ] {
            let mut style = TextStyle::new(size);
            style.line_height = line_height;
            let l = ts.layout("Hello", &style, Some(100.0));
            assert!(l.size().x.is_finite() && l.size().y.is_finite(), "{size} {line_height}: {:?}", l.size());
            ts.paint(&mut dl, &l, Vec2::new(0.0, 0.0), Color::WHITE, 1.0);
        }
        for wrap in [f32::NAN, -1.0, 0.0, f32::INFINITY, f32::MAX] {
            let l = ts.layout("Hello wrapping world", &TextStyle::new(14.0), Some(wrap));
            assert!(l.size().x.is_finite() && l.size().y.is_finite(), "wrap {wrap}: {:?}", l.size());
        }
        let mut style = TextStyle::new(14.0);
        style.letter_spacing = f32::NAN;
        let l = ts.layout("Hello", &style, None);
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            ts.paint(&mut dl, &l, Vec2::new(0.0, 0.0), Color::WHITE, scale);
        }
        for inst in dl.instances() {
            assert!(inst.rect.iter().chain(&inst.uv).all(|f| f.is_finite()), "{inst:?}");
        }
    }

    #[test]
    fn shapes_and_caches() {
        let mut ts = TextSystem::new(&TextOptions::default());
        ts.begin_frame();
        let style = TextStyle::new(14.0);
        let a = ts.layout("Hello Rugi", &style, None);
        assert!(a.size().x > 40.0 && a.size().x < 120.0, "width {}", a.size().x);
        assert_eq!(a.size().y, style.line_px());
        assert_eq!(a.glyph_count(), 10);
        assert_eq!(ts.shaped_this_frame(), 1);
        let b = ts.layout("Hello Rugi", &style, None);
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(ts.shaped_this_frame(), 1);
        // Heavier weight is wider.
        let bold = ts.layout("Hello Rugi", &style.weight(FontWeight::SEMIBOLD), None);
        assert!(bold.size().x > a.size().x);
    }

    #[test]
    fn custom_fonts_and_icon_fallback() {
        let mut ts = TextSystem::new(&TextOptions::default());
        assert_eq!(ts.add_font(vec![1, 2, 3]), Err(FontError::InvalidFont));
        let inter = ts.add_font(INTER_SEMIBOLD.to_vec()).expect("valid font");
        assert_eq!(ts.family_name(inter), "Inter");
        // Same family name -> same handle.
        assert_eq!(ts.add_font(INTER_REGULAR.to_vec()).unwrap(), inter);
        let icons = ts.icon_family().expect("lucide bundled");
        assert_eq!(ts.family_name(icons), "lucide");
        // An icon codepoint inside a normal Inter label is shaped with the
        // icon font (fallback), producing a real (non-zero) glyph.
        let style = TextStyle::new(16.0);
        let plain = ts.layout("Save", &style, None);
        let with_icon = ts.layout(&format!("{} Save", rugi_icons::lucide::SAVE), &style, None);
        assert_eq!(with_icon.glyph_count(), plain.glyph_count() + 2);
        assert!(with_icon.size().x > plain.size().x + 10.0);
        let mut dl = DrawList::new();
        ts.paint(&mut dl, &with_icon, Vec2::ZERO, Color::WHITE, 1.0);
        assert!(dl.instances().len() >= 5, "icon glyph rasterized");
    }

    #[test]
    fn inline_icons_use_the_icon_font_even_where_inter_has_private_glyphs() {
        let mut ts = TextSystem::new(&TextOptions::default());
        for icon in [
            rugi_icons::lucide::MAIL,
            rugi_icons::lucide::CIRCLE_CHECK,
            rugi_icons::lucide::ROCKET,
            rugi_icons::lucide::SAVE,
        ] {
            for family in [FontFamily::Sans, FontFamily::Mono] {
                let mut style = TextStyle::new(16.0);
                style.family = family;
                let l = ts.layout(&format!("{icon}  label"), &style, None);
                let first = &l.glyphs[0].glyph;
                let face = ts.fonts.db().face(first.font_id).expect("face");
                assert_eq!(face.families[0].0, "lucide", "{:?} in {family:?} shaped with {}", icon, face.families[0].0);
                // The rest of the label stays in the UI font.
                let last = &l.glyphs.last().unwrap().glyph;
                assert_ne!(ts.fonts.db().face(last.font_id).unwrap().families[0].0, "lucide");
            }
        }
    }

    #[test]
    fn bundled_mono_is_monospaced() {
        let mut ts = TextSystem::new(&TextOptions::default());
        let mut style = TextStyle::new(14.0);
        style.family = FontFamily::Mono;
        let narrow = ts.layout("iiiiiiii", &style, None).size().x;
        let wide = ts.layout("MMMMMMMM", &style, None).size().x;
        assert!(narrow > 40.0 && (narrow - wide).abs() < 1.0, "{narrow} vs {wide}");
    }

    #[test]
    fn wrapping_produces_multiple_lines() {
        let mut ts = TextSystem::new(&TextOptions::default());
        let style = TextStyle::new(14.0);
        let l = ts.layout("one two three four five six seven eight nine ten", &style, Some(80.0));
        assert!(l.size().y >= style.line_px() * 3.0);
        assert!(l.size().x <= 80.0);
    }

    #[test]
    fn caret_positions_are_monotonic() {
        let mut ts = TextSystem::new(&TextOptions::default());
        let l = ts.layout("abc", &TextStyle::new(14.0), None);
        let xs: Vec<f32> = (0..=3).map(|i| l.caret_x(i)).collect();
        assert_eq!(xs[0], 0.0);
        assert!(xs.windows(2).all(|w| w[1] > w[0]));
        assert_eq!(l.index_at_x(-5.0), 0);
        assert_eq!(l.index_at_x(1000.0), 3);
        assert_eq!(l.index_at_x(xs[2] + 0.1), 2);
    }

    #[test]
    fn paint_rasterizes_into_atlas() {
        let mut ts = TextSystem::new(&TextOptions::default());
        ts.begin_frame();
        let l = ts.layout("Rugi", &TextStyle::new(16.0), None);
        let mut dl = DrawList::new();
        ts.paint(&mut dl, &l, Vec2::new(10.0, 10.0), Color::WHITE, 2.0);
        assert_eq!(dl.instances().len(), 4);
        assert_eq!(ts.rasterized_this_frame(), 4);
        let up = ts.take_atlas_update().expect("atlas update");
        assert_eq!(up.data.len() as u32, up.region[2] * up.region[3]);
        assert!(up.data.iter().any(|&b| b > 0));
        // Second paint reuses the atlas and the cached glyph run, and lands on
        // exactly the same pixels as the first.
        let first: Vec<_> = dl.instances().to_vec();
        let mut dl2 = DrawList::new();
        ts.begin_frame();
        ts.paint(&mut dl2, &l, Vec2::new(10.0, 10.0), Color::WHITE, 2.0);
        assert_eq!(ts.rasterized_this_frame(), 0);
        assert!(ts.take_atlas_update().is_none());
        assert_eq!(dl2.instances(), &first[..]);
        // A moved layout keeps glyph shapes; positions shift by the snapped offset.
        let mut dl3 = DrawList::new();
        ts.paint(&mut dl3, &l, Vec2::new(110.0, 60.0), Color::WHITE, 2.0);
        for (a, b) in first.iter().zip(dl3.instances()) {
            assert!((b.rect[0] - a.rect[0] - 100.0).abs() < 1e-3 && (b.rect[1] - a.rect[1] - 50.0).abs() < 1e-3);
            assert_eq!(a.uv, b.uv);
        }
        // The cached path is pixel-identical to resolving every glyph directly,
        // at fractional DPI scales too.
        for (scale, pos) in [(2.0, Vec2::new(10.0, 10.0)), (1.25, Vec2::new(33.3, 7.7)), (1.0, Vec2::new(0.4, 99.6))] {
            let mut cached = DrawList::new();
            let mut direct = DrawList::new();
            let ml = ts.layout("Wavy ffi façade 123", &TextStyle::new(15.0), None);
            ts.paint(&mut cached, &ml, pos, Color::WHITE, scale);
            ts.paint_uncached(&mut direct, &ml, pos, Color::WHITE, scale);
            assert_eq!(cached.instances().len(), direct.instances().len());
            for (a, b) in cached.instances().iter().zip(direct.instances()) {
                for k in 0..4 {
                    assert!((a.rect[k] - b.rect[k]).abs() < 1e-3, "scale {scale}: {:?} vs {:?}", a.rect, b.rect);
                }
                assert_eq!(a.uv, b.uv);
            }
        }
        // Painting at another scale rebuilds the run (different glyph sizes).
        let mut dl4 = DrawList::new();
        ts.paint(&mut dl4, &l, Vec2::new(10.0, 10.0), Color::WHITE, 1.0);
        assert!(dl4.instances()[0].rect[2] != first[0].rect[2] || dl4.instances()[0].uv != first[0].uv);
    }
}
