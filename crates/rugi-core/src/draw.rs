//! The draw list: GPU-ready instanced primitives grouped into batches.
//!
//! Rugi renders every UI primitive as one instanced quad whose fragment
//! shader evaluates an analytic signed-distance function (rounded rectangle,
//! box shadow, line segment) or samples a texture (glyph atlas, image). The
//! CPU side therefore never tessellates curves: a rounded, bordered,
//! gradient-filled rectangle is a single 80-byte [`Instance`].
//!
//! [`Instance`] is `#[repr(C)]` and `Pod`, so renderers upload the instance
//! array as-is with no per-frame conversion.

use crate::fx::{Backdrop, GlassStyle};
use crate::{Color, Rect, Vec2};
use bytemuck::{Pod, Zeroable};

/// Identifies a texture owned by the renderer. `TextureId::NONE` means "no image".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TextureId(pub u64);

impl TextureId {
    /// No texture / default white texture.
    pub const NONE: TextureId = TextureId(0);
}

/// Primitive kinds understood by the shader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PrimitiveKind {
    /// Rounded rectangle with optional gradient fill and border.
    Rect = 0,
    /// Gaussian-blurred rounded-rectangle shadow.
    Shadow = 1,
    /// Alpha-mask glyph sampled from the font atlas.
    Glyph = 2,
    /// RGBA image with rounded-corner mask.
    Image = 3,
    /// Anti-aliased line segment with round caps.
    Line = 4,
    /// Glass surface sampling the blurred frame behind it.
    Glass = 5,
    /// Animated procedural backdrop.
    Backdrop = 6,
    /// Degenerate (reserved slot that was never filled).
    None = 255,
}

/// One GPU instance (80 bytes). Field meaning depends on `kind`:
///
/// | kind     | `rect`            | `uv`                               | `params`                                  |
/// |----------|-------------------|------------------------------------|-------------------------------------------|
/// | Rect     | x, y, w, h        | gradient start/end (rect-relative) | x = border width                          |
/// | Shadow   | caster x, y, w, h | unused                             | y = blur sigma                            |
/// | Glyph    | x, y, w, h        | atlas u0, v0, u1, v1               | unused                                    |
/// | Image    | x, y, w, h        | texture u0, v0, u1, v1             | unused                                    |
/// | Line     | unused            | x0, y0, x1, y1                     | x = thickness                             |
/// | Glass    | x, y, w, h        | blur, refraction, bevel, grain     | border, saturation, highlight, dispersion |
/// | Backdrop | x, y, w, h        | time, scale, intensity, kind       | seed, pointer x, pointer y, response      |
///
/// Glass: `color0` = tint, `color1` = rim light (alpha = overall opacity),
/// `border_color` = border. Backdrop: `color0` = base (alpha = opacity),
/// `color1` / `border_color` = accents.
///
/// Colors are packed sRGB `[r, g, b, a]` bytes with straight alpha.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct Instance {
    /// Rectangle in logical pixels.
    pub rect: [f32; 4],
    /// Corner radii: top-left, top-right, bottom-right, bottom-left.
    pub radii: [f32; 4],
    /// Kind-specific (see table).
    pub uv: [f32; 4],
    /// Kind-specific (see table).
    pub params: [f32; 4],
    /// Fill / tint color (gradient start).
    pub color0: u32,
    /// Gradient end color.
    pub color1: u32,
    /// Border color.
    pub border_color: u32,
    /// [`PrimitiveKind`] as `u32`.
    pub kind: u32,
}

impl Instance {
    const EMPTY: Instance = Instance {
        rect: [0.0; 4],
        radii: [0.0; 4],
        uv: [0.0; 4],
        params: [0.0; 4],
        color0: 0,
        color1: 0,
        border_color: 0,
        kind: PrimitiveKind::None as u32,
    };

    /// True if every float field is finite.
    #[inline]
    #[expect(clippy::eq_op, reason = "`x - x` is the finiteness test")]
    pub fn is_finite(&self) -> bool {
        // `x - x` is 0 for finite x and NaN for inf/NaN, so the sum is 0 only
        // when every field is finite. Unlike summing the fields it cannot
        // overflow, and the four lanes are independent, so it vectorizes.
        let lane = |i: usize| {
            (self.rect[i] - self.rect[i])
                + (self.radii[i] - self.radii[i])
                + (self.uv[i] - self.uv[i])
                + (self.params[i] - self.params[i])
        };
        lane(0) + lane(1) + lane(2) + lane(3) == 0.0
    }

    /// Logical-space bounds this instance can touch (used for CPU culling).
    #[inline]
    pub fn bounds(&self) -> Rect {
        let r = Rect::from_xywh(self.rect[0], self.rect[1], self.rect[2], self.rect[3]);
        match self.kind {
            k if k == PrimitiveKind::Shadow as u32 => r.expand(self.params[1] * 3.0),
            k if k == PrimitiveKind::Line as u32 => {
                let h = self.params[0] * 0.5 + 1.0;
                Rect::from_min_max(
                    Vec2::new(self.uv[0].min(self.uv[2]), self.uv[1].min(self.uv[3])),
                    Vec2::new(self.uv[0].max(self.uv[2]), self.uv[1].max(self.uv[3])),
                )
                .expand(h)
            }
            _ => r.expand(1.0),
        }
    }
}

/// A contiguous range of instances drawn with the same scissor and image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawCommand {
    /// Clip rectangle in logical pixels.
    pub clip: Rect,
    /// Image texture bound for `Image` primitives in this range.
    pub texture: TextureId,
    /// First instance index.
    pub start: u32,
    /// One past the last instance index.
    pub end: u32,
}

/// Fill description for rectangles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    /// Solid color.
    Solid(Color),
    /// Vertical gradient, top to bottom.
    Vertical(Color, Color),
    /// Horizontal gradient, left to right.
    Horizontal(Color, Color),
}

impl From<Color> for Fill {
    fn from(c: Color) -> Self {
        Fill::Solid(c)
    }
}

/// A handle to a reserved instance slot (see [`DrawList::reserve`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(u32);

/// Per-frame list of instances and batches. Reused across frames (cleared,
/// not reallocated) so steady-state frames do not allocate.
#[derive(Debug, Default)]
pub struct DrawList {
    instances: Vec<Instance>,
    commands: Vec<DrawCommand>,
    clip_stack: Vec<Rect>,
    culled: u32,
    glass: u32,
    backdrops: u32,
}

impl DrawList {
    /// Creates an empty draw list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears for a new frame while keeping allocations.
    pub fn clear(&mut self) {
        self.instances.clear();
        self.commands.clear();
        self.clip_stack.clear();
        self.culled = 0;
        self.glass = 0;
        self.backdrops = 0;
    }

    /// Number of procedural backdrop instances.
    pub fn backdrop_count(&self) -> u32 {
        self.backdrops
    }

    /// Number of glass instances (the renderer needs backdrop snapshots only
    /// when this is non-zero).
    pub fn glass_count(&self) -> u32 {
        self.glass
    }

    /// All instances.
    #[inline]
    pub fn instances(&self) -> &[Instance] {
        &self.instances
    }

    /// All draw commands (batches).
    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    /// Number of primitives skipped because they were fully clipped.
    pub fn culled(&self) -> u32 {
        self.culled
    }

    /// Appends all of `other`'s instances and batches after this list's
    /// (used to draw overlay layers such as popups above the main content).
    pub fn append(&mut self, other: &DrawList) {
        let base = self.instances.len() as u32;
        self.instances.extend_from_slice(&other.instances);
        self.commands.extend(other.commands.iter().map(|c| DrawCommand {
            start: c.start + base,
            end: c.end + base,
            ..*c
        }));
        self.culled += other.culled;
        self.glass += other.glass;
        self.backdrops += other.backdrops;
    }

    /// Number of instances so far (a marker for [`DrawList::fade_from`]).
    #[inline]
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// True if no instances were added.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Multiplies the alpha of every instance added since `start` by `factor`
    /// (used for fade-in of popups whose content is already emitted).
    pub fn fade_from(&mut self, start: usize, factor: f32) {
        if factor >= 1.0 {
            return;
        }
        let f = factor.clamp(0.0, 1.0);
        let fade = |c: &mut u32| {
            let a = ((*c >> 24) as f32 * f + 0.5) as u32;
            *c = (*c & 0x00ff_ffff) | (a << 24);
        };
        for inst in self.instances.iter_mut().skip(start) {
            fade(&mut inst.color0);
            fade(&mut inst.color1);
            fade(&mut inst.border_color);
        }
    }

    /// Records `n` primitives skipped by a caller-side visibility test.
    pub fn add_culled(&mut self, n: u32) {
        self.culled += n;
    }

    /// Current clip rectangle.
    #[inline]
    pub fn clip(&self) -> Rect {
        self.clip_stack.last().copied().unwrap_or(Rect::EVERYTHING)
    }

    /// Pushes a clip rectangle, intersected with the current one.
    #[inline]
    pub fn push_clip(&mut self, r: Rect) {
        let c = self.clip().intersect(&r);
        self.clip_stack.push(c);
    }

    /// Pops the most recent clip rectangle.
    #[inline]
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    #[inline(always)]
    fn push(&mut self, inst: Instance, texture: TextureId) {
        let clip = self.clip();
        // Non-finite geometry (e.g. from NaN input) is dropped like clipped geometry.
        if clip.is_empty() || !inst.bounds().intersects(&clip) || !inst.is_finite() {
            self.culled += 1;
            return;
        }
        self.push_unculled(inst, texture, clip);
    }

    /// Ensures the last command can receive more texture-less instances under
    /// `clip` (opening a new one if needed).
    #[inline]
    fn open_batch(&mut self, clip: Rect) {
        let idx = self.instances.len() as u32;
        match self.commands.last() {
            Some(cmd) if cmd.clip == clip && cmd.end == idx => {}
            _ => self.commands.push(DrawCommand { clip, texture: TextureId::NONE, start: idx, end: idx }),
        }
    }

    #[inline(always)]
    fn push_unculled(&mut self, inst: Instance, texture: TextureId, clip: Rect) {
        let idx = self.instances.len() as u32;
        let needs_new = match self.commands.last_mut() {
            Some(cmd) if cmd.clip == clip && cmd.end == idx => {
                if texture == TextureId::NONE || cmd.texture == texture {
                    false
                } else if cmd.texture == TextureId::NONE {
                    cmd.texture = texture;
                    false
                } else {
                    true
                }
            }
            _ => true,
        };
        if needs_new {
            self.commands.push(DrawCommand { clip, texture, start: idx, end: idx });
        }
        self.instances.push(inst);
        if let Some(cmd) = self.commands.last_mut() {
            cmd.end = idx + 1;
        }
    }

    /// Reserves a slot to be filled later with [`DrawList::fill_slot`]. Used by
    /// containers whose background size is only known after their children
    /// have been laid out. The slot keeps the clip active at reservation time.
    #[inline]
    pub fn reserve(&mut self) -> Slot {
        let clip = self.clip();
        let idx = self.instances.len() as u32;
        self.push_unculled(Instance::EMPTY, TextureId::NONE, clip);
        Slot(idx)
    }

    /// Fills a reserved slot. The instance must not be an `Image` (slots never
    /// change their batch texture).
    #[inline]
    pub fn fill_slot(&mut self, slot: Slot, inst: Instance) {
        debug_assert!(inst.kind != PrimitiveKind::Image as u32);
        if !inst.is_finite() {
            return;
        }
        if let Some(dst) = self.instances.get_mut(slot.0 as usize) {
            let (g, b) = (PrimitiveKind::Glass as u32, PrimitiveKind::Backdrop as u32);
            let (old, new) = (dst.kind, inst.kind);
            *dst = inst;
            self.glass = self.glass + (new == g) as u32 - (old == g) as u32;
            self.backdrops = self.backdrops + (new == b) as u32 - (old == b) as u32;
        }
    }

    /// Builds a rounded-rectangle instance (not pushed).
    #[inline]
    pub fn rect_instance(rect: Rect, radius: [f32; 4], fill: Fill, border: Option<(f32, Color)>) -> Instance {
        let (c0, c1, uv) = match fill {
            Fill::Solid(c) => (c, c, [0.0, 0.0, 0.0, 0.0]),
            Fill::Vertical(a, b) => (a, b, [0.0, 0.0, 0.0, 1.0]),
            Fill::Horizontal(a, b) => (a, b, [0.0, 0.0, 1.0, 0.0]),
        };
        let max_r = rect.width().min(rect.height()) * 0.5;
        let (bw, bc) = border.unwrap_or((0.0, Color::TRANSPARENT));
        Instance {
            rect: [rect.min.x, rect.min.y, rect.width(), rect.height()],
            radii: radius.map(|r| r.clamp(0.0, max_r.max(0.0))),
            uv,
            params: [bw, 0.0, 0.0, 0.0],
            color0: c0.to_packed(),
            color1: c1.to_packed(),
            border_color: bc.to_packed(),
            kind: PrimitiveKind::Rect as u32,
        }
    }

    /// Builds a shadow instance (not pushed).
    #[inline]
    pub fn shadow_instance(rect: Rect, radius: f32, sigma: f32, color: Color) -> Instance {
        let max_r = rect.width().min(rect.height()) * 0.5;
        let r = radius.clamp(0.0, max_r.max(0.0));
        Instance {
            rect: [rect.min.x, rect.min.y, rect.width(), rect.height()],
            radii: [r; 4],
            uv: [0.0; 4],
            params: [0.0, sigma.max(0.5), 0.0, 0.0],
            color0: color.to_packed(),
            color1: 0,
            border_color: 0,
            kind: PrimitiveKind::Shadow as u32,
        }
    }

    /// Adds a filled, optionally bordered, rounded rectangle.
    #[inline]
    pub fn rect(&mut self, rect: Rect, radius: f32, fill: impl Into<Fill>, border: Option<(f32, Color)>) {
        let inst = Self::rect_instance(rect, [radius; 4], fill.into(), border);
        if inst.color0 >> 24 == 0 && inst.color1 >> 24 == 0 && (border.is_none() || inst.border_color >> 24 == 0) {
            return;
        }
        self.push(inst, TextureId::NONE);
    }

    /// Adds a rounded rectangle with per-corner radii (tl, tr, br, bl).
    pub fn rect_corners(&mut self, rect: Rect, radii: [f32; 4], fill: impl Into<Fill>, border: Option<(f32, Color)>) {
        let inst = Self::rect_instance(rect, radii, fill.into(), border);
        self.push(inst, TextureId::NONE);
    }

    /// Adds a soft drop shadow for a rounded rectangle.
    #[inline]
    pub fn shadow(&mut self, rect: Rect, radius: f32, sigma: f32, color: Color) {
        if color.a == 0 {
            return;
        }
        self.push(Self::shadow_instance(rect, radius, sigma, color), TextureId::NONE);
    }

    /// Adds an anti-aliased line segment with round caps.
    #[inline]
    pub fn line(&mut self, a: Vec2, b: Vec2, thickness: f32, color: Color) {
        if color.a == 0 {
            return;
        }
        let inst = Instance {
            rect: [0.0; 4],
            radii: [0.0; 4],
            uv: [a.x, a.y, b.x, b.y],
            params: [thickness, 0.0, 0.0, 0.0],
            color0: color.to_packed(),
            color1: 0,
            border_color: 0,
            kind: PrimitiveKind::Line as u32,
        };
        self.push(inst, TextureId::NONE);
    }

    /// Adds a filled circle.
    #[inline]
    pub fn circle(&mut self, center: Vec2, radius: f32, fill: impl Into<Fill>, border: Option<(f32, Color)>) {
        let r = Rect::from_min_max(
            Vec2::new(center.x - radius, center.y - radius),
            Vec2::new(center.x + radius, center.y + radius),
        );
        self.rect(r, radius, fill, border);
    }

    /// Adds a glyph quad sampling the font atlas. `rect` should be aligned to
    /// physical pixels by the caller for crisp text.
    pub fn glyph(&mut self, rect: Rect, uv: [f32; 4], color: Color) {
        let inst = Instance {
            rect: [rect.min.x, rect.min.y, rect.width(), rect.height()],
            radii: [0.0; 4],
            uv,
            params: [0.0; 4],
            color0: color.to_packed(),
            color1: 0,
            border_color: 0,
            kind: PrimitiveKind::Glyph as u32,
        };
        self.push(inst, TextureId::NONE);
    }

    /// Adds a run of glyph quads (`(rect, uv)` relative to `(ox, oy)`) in one
    /// call. `fully_visible` skips per-glyph clip tests when the caller knows
    /// the whole run lies inside the current clip rectangle.
    pub fn glyph_run(
        &mut self,
        quads: impl Iterator<Item = ([f32; 4], [f32; 4])>,
        ox: f32,
        oy: f32,
        color: Color,
        fully_visible: bool,
    ) {
        let c = color.to_packed();
        let make = |rect: [f32; 4], uv: [f32; 4]| Instance {
            rect: [ox + rect[0], oy + rect[1], rect[2], rect[3]],
            radii: [0.0; 4],
            uv,
            params: [0.0; 4],
            color0: c,
            color1: 0,
            border_color: 0,
            kind: PrimitiveKind::Glyph as u32,
        };
        if fully_visible {
            // One batch check for the whole run, then a straight append.
            let clip = self.clip();
            self.open_batch(clip);
            self.instances.extend(quads.map(|(rect, uv)| make(rect, uv)));
            let end = self.instances.len() as u32;
            if let Some(cmd) = self.commands.last_mut() {
                cmd.end = end;
            }
        } else {
            for (rect, uv) in quads {
                self.push(make(rect, uv), TextureId::NONE);
            }
        }
    }

    /// Builds a glass instance (not pushed). `opacity` fades the whole surface.
    pub fn glass_instance(rect: Rect, style: &GlassStyle, opacity: f32) -> Instance {
        let max_r = rect.width().min(rect.height()) * 0.5;
        let o = opacity.clamp(0.0, 1.0);
        let hl = style.highlight_color.with_alpha(o);
        Instance {
            rect: rect_fields(rect),
            radii: [style.radius.clamp(0.0, max_r.max(0.0)); 4],
            uv: [style.blur.clamp(0.0, 1.0), style.refraction, style.bevel.max(0.0), style.grain.clamp(0.0, 1.0)],
            params: [style.border_width, style.saturation.max(0.0), style.highlight, style.dispersion.clamp(0.0, 1.0)],
            color0: style.tint.to_packed(),
            color1: hl.to_packed(),
            border_color: style.border.to_packed(),
            kind: PrimitiveKind::Glass as u32,
        }
    }

    /// Adds a glass surface that blurs and refracts everything drawn before it.
    pub fn glass(&mut self, rect: Rect, style: &GlassStyle) {
        // Effects are counted here rather than in the generic push path so the
        // hot path for ordinary primitives carries no extra branch.
        let n = self.instances.len();
        self.push(Self::glass_instance(rect, style, 1.0), TextureId::NONE);
        self.glass += (self.instances.len() > n) as u32;
    }

    /// Builds a procedural backdrop instance (not pushed). `time` is in seconds;
    /// `pointer` is the pointer position in logical pixels, if known.
    pub fn backdrop_instance(rect: Rect, radius: f32, b: &Backdrop, time: f32, pointer: Option<Vec2>) -> Instance {
        let max_r = rect.width().min(rect.height()) * 0.5;
        let p = pointer.unwrap_or(Vec2::new(-1.0e5, -1.0e5));
        Instance {
            rect: rect_fields(rect),
            radii: [radius.clamp(0.0, max_r.max(0.0)); 4],
            uv: [time * b.speed, b.scale.max(0.05), b.intensity.max(0.0), b.kind as u32 as f32],
            params: [b.seed, p.x, p.y, b.pointer_response],
            color0: b.base.to_packed(),
            color1: b.color_a.to_packed(),
            border_color: b.color_b.to_packed(),
            kind: PrimitiveKind::Backdrop as u32,
        }
    }

    /// Adds an animated procedural backdrop filling `rect`.
    pub fn backdrop(&mut self, rect: Rect, radius: f32, b: &Backdrop, time: f32, pointer: Option<Vec2>) {
        let n = self.instances.len();
        self.push(Self::backdrop_instance(rect, radius, b, time, pointer), TextureId::NONE);
        self.backdrops += (self.instances.len() > n) as u32;
    }

    /// Adds an image (sampled from a renderer-registered texture) with rounded corners.
    pub fn image(&mut self, rect: Rect, texture: TextureId, uv: [f32; 4], tint: Color, radius: f32) {
        let max_r = rect.width().min(rect.height()) * 0.5;
        let inst = Instance {
            rect: [rect.min.x, rect.min.y, rect.width(), rect.height()],
            radii: [radius.clamp(0.0, max_r.max(0.0)); 4],
            uv,
            params: [0.0; 4],
            color0: tint.to_packed(),
            color1: 0,
            border_color: 0,
            kind: PrimitiveKind::Image as u32,
        };
        self.push(inst, texture);
    }
}

/// `rect` as the `[x, y, w, h]` layout of [`Instance::rect`].
fn rect_fields(r: Rect) -> [f32; 4] {
    [r.min.x, r.min.y, r.width(), r.height()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_finite_instances_are_dropped() {
        let mut dl = DrawList::new();
        let ok = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        dl.rect(Rect::from_xywh(0.0, 0.0, f32::INFINITY, 10.0), 2.0, Color::WHITE, None);
        dl.rect(ok, f32::NAN, Color::WHITE, Some((f32::INFINITY, Color::BLACK)));
        dl.line(Vec2::new(0.0, 0.0), Vec2::new(f32::NAN, 5.0), 1.0, Color::WHITE);
        dl.shadow(ok, 2.0, f32::INFINITY, Color::BLACK);
        assert!(dl.is_empty());
        assert_eq!(dl.culled(), 4);
        let slot = dl.reserve();
        dl.fill_slot(slot, DrawList::rect_instance(ok, [f32::NAN; 4], Fill::Solid(Color::WHITE), None));
        assert!(dl.instances().iter().all(Instance::is_finite));
    }

    #[test]
    fn huge_finite_fields_count_as_finite() {
        let mut inst =
            DrawList::rect_instance(Rect::from_xywh(0.0, 0.0, 10.0, 10.0), [0.0; 4], Fill::Solid(Color::WHITE), None);
        inst.params = [f32::MAX; 4];
        inst.uv = [f32::MAX; 4];
        assert!(inst.is_finite());
        inst.radii[3] = f32::NEG_INFINITY;
        assert!(!inst.is_finite());
    }

    #[test]
    fn instance_is_80_bytes() {
        assert_eq!(std::mem::size_of::<Instance>(), 80);
    }

    #[test]
    fn batching_merges_and_splits() {
        let mut dl = DrawList::new();
        let r = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        dl.rect(r, 2.0, Color::WHITE, None);
        dl.rect(r, 2.0, Color::WHITE, None);
        assert_eq!(dl.commands().len(), 1);
        dl.push_clip(Rect::from_xywh(0.0, 0.0, 5.0, 5.0));
        dl.rect(r, 2.0, Color::WHITE, None);
        dl.pop_clip();
        assert_eq!(dl.commands().len(), 2);
        // An image joins a texture-less batch, a second image texture splits.
        dl.image(r, TextureId(7), [0.0, 0.0, 1.0, 1.0], Color::WHITE, 0.0);
        assert_eq!(dl.commands().len(), 3);
        dl.image(r, TextureId(8), [0.0, 0.0, 1.0, 1.0], Color::WHITE, 0.0);
        assert_eq!(dl.commands().len(), 4);
        assert_eq!(dl.instances().len(), 5);
    }

    #[test]
    fn append_rebases_batches() {
        let mut a = DrawList::new();
        let r = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        a.rect(r, 0.0, Color::WHITE, None);
        let mut b = DrawList::new();
        b.rect(r, 0.0, Color::BLACK, None);
        b.rect(r, 0.0, Color::BLACK, None);
        a.append(&b);
        assert_eq!(a.instances().len(), 3);
        assert_eq!(a.commands()[1].start, 1);
        assert_eq!(a.commands()[1].end, 3);
    }

    #[test]
    fn culling_skips_clipped_primitives() {
        let mut dl = DrawList::new();
        dl.push_clip(Rect::from_xywh(0.0, 0.0, 100.0, 100.0));
        dl.rect(Rect::from_xywh(500.0, 500.0, 10.0, 10.0), 0.0, Color::WHITE, None);
        assert_eq!(dl.instances().len(), 0);
        assert_eq!(dl.culled(), 1);
    }

    #[test]
    fn glass_is_counted_through_slots_and_append() {
        let mut dl = DrawList::new();
        let r = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        dl.glass(r, &GlassStyle::frosted());
        assert_eq!(dl.glass_count(), 1);
        let slot = dl.reserve();
        dl.fill_slot(slot, DrawList::glass_instance(r, &GlassStyle::liquid(), 1.0));
        assert_eq!(dl.glass_count(), 2);
        dl.fill_slot(slot, DrawList::rect_instance(r, [0.0; 4], Fill::Solid(Color::WHITE), None));
        assert_eq!(dl.glass_count(), 1);
        let mut other = DrawList::new();
        other.glass(r, &GlassStyle::smoked());
        dl.append(&other);
        assert_eq!(dl.glass_count(), 2);
        dl.clear();
        assert_eq!(dl.glass_count(), 0);
    }

    #[test]
    fn reserved_slots_keep_order() {
        let mut dl = DrawList::new();
        let slot = dl.reserve();
        dl.rect(Rect::from_xywh(0.0, 0.0, 1.0, 1.0), 0.0, Color::WHITE, None);
        let bg =
            DrawList::rect_instance(Rect::from_xywh(0.0, 0.0, 9.0, 9.0), [0.0; 4], Fill::Solid(Color::BLACK), None);
        dl.fill_slot(slot, bg);
        assert_eq!(dl.instances()[0], bg);
        assert_eq!(dl.commands().len(), 1);
    }
}
