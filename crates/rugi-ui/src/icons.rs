//! A small built-in vector icon set drawn with SDF primitives (no assets).

use rugi_core::{Color, DrawList, Rect, Vec2};

/// An icon parameter: a built-in vector icon or a glyph from an icon font
/// (e.g. [`crate::lucide::SAVE`] or a Font Awesome codepoint).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconRef<'a> {
    /// Built-in SDF vector icon.
    Vector(Icon),
    /// One or more characters rendered with the text system (icon fonts).
    Glyph(&'a str),
}

impl From<Icon> for IconRef<'_> {
    fn from(i: Icon) -> Self {
        IconRef::Vector(i)
    }
}

impl<'a> From<&'a str> for IconRef<'a> {
    fn from(s: &'a str) -> Self {
        IconRef::Glyph(s)
    }
}

/// Built-in icons. Each is drawn into a square box using lines and rounded
/// rectangles, so they scale to any DPI without assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Icon {
    /// 2×2 grid of tiles.
    Dashboard = 0,
    /// Bar chart.
    Chart = 1,
    /// Three slider controls.
    Sliders = 2,
    /// Pulse line.
    Activity = 3,
    /// Stacked layers.
    Layers = 4,
    /// Magnifier.
    Search = 5,
    /// Plus sign.
    Plus = 6,
    /// Check mark.
    Check = 7,
    /// Folder.
    Folder = 8,
}

impl Icon {
    /// Converts from the C ABI integer.
    pub fn from_u32(v: u32) -> Option<Icon> {
        Some(match v {
            0 => Icon::Dashboard,
            1 => Icon::Chart,
            2 => Icon::Sliders,
            3 => Icon::Activity,
            4 => Icon::Layers,
            5 => Icon::Search,
            6 => Icon::Plus,
            7 => Icon::Check,
            8 => Icon::Folder,
            _ => return None,
        })
    }

    /// Paints the icon into `r` (should be square).
    pub fn paint(self, dl: &mut DrawList, r: Rect, color: Color) {
        let s = r.width().min(r.height());
        let o = r.min;
        let p = |x: f32, y: f32| Vec2::new(o.x + x * s, o.y + y * s);
        let t = (s * 0.1).max(1.3);
        match self {
            Icon::Dashboard => {
                let k = 0.40;
                for (x, y) in [(0.08, 0.08), (0.52, 0.08), (0.08, 0.52), (0.52, 0.52)] {
                    let rr = Rect::from_min_size(p(x, y), Vec2::new(k * s, k * s));
                    dl.rect(rr, s * 0.1, Color::TRANSPARENT, Some((t * 0.9, color)));
                }
            }
            Icon::Chart => {
                for (x, h) in [(0.2, 0.35), (0.5, 0.75), (0.8, 0.55)] {
                    dl.line(p(x, 0.9), p(x, 0.9 - h), t * 1.25, color);
                }
            }
            Icon::Sliders => {
                for (y, k) in [(0.22, 0.65), (0.5, 0.3), (0.78, 0.55)] {
                    dl.line(p(0.08, y), p(0.92, y), t * 0.85, color.fade(0.55));
                    dl.circle(p(k, y), s * 0.11, color, None);
                }
            }
            Icon::Activity => {
                let pts = [p(0.05, 0.55), p(0.28, 0.55), p(0.4, 0.2), p(0.6, 0.85), p(0.72, 0.55), p(0.95, 0.55)];
                for w in pts.windows(2) {
                    dl.line(w[0], w[1], t, color);
                }
            }
            Icon::Layers => {
                let diamond = |dl: &mut DrawList, cy: f32, c: Color| {
                    let pts = [p(0.5, cy - 0.22), p(0.92, cy), p(0.5, cy + 0.22), p(0.08, cy)];
                    for i in 0..4 {
                        dl.line(pts[i], pts[(i + 1) % 4], t, c);
                    }
                };
                diamond(dl, 0.62, color.fade(0.55));
                diamond(dl, 0.38, color);
            }
            Icon::Search => {
                dl.circle(p(0.42, 0.42), s * 0.3, Color::TRANSPARENT, Some((t, color)));
                dl.line(p(0.64, 0.64), p(0.9, 0.9), t * 1.2, color);
            }
            Icon::Plus => {
                dl.line(p(0.5, 0.15), p(0.5, 0.85), t * 1.2, color);
                dl.line(p(0.15, 0.5), p(0.85, 0.5), t * 1.2, color);
            }
            Icon::Check => {
                dl.line(p(0.18, 0.52), p(0.42, 0.75), t * 1.3, color);
                dl.line(p(0.42, 0.75), p(0.84, 0.28), t * 1.3, color);
            }
            Icon::Folder => {
                let body = Rect::from_min_max(p(0.06, 0.28), p(0.94, 0.86));
                dl.rect(body, s * 0.1, Color::TRANSPARENT, Some((t * 0.9, color)));
                dl.line(p(0.12, 0.2), p(0.4, 0.2), t * 0.9, color);
            }
        }
    }
}

impl crate::Ui {
    /// Paints `icon` centered in the square `r`.
    pub fn paint_icon(&mut self, icon: IconRef<'_>, r: rugi_core::Rect, color: Color) {
        match icon {
            IconRef::Vector(i) => i.paint(&mut self.out.draw_list, r, color),
            IconRef::Glyph(g) => {
                let size = r.width().min(r.height());
                let mut style = rugi_core::TextStyle::new(size);
                style.line_height = 1.0;
                if let Some(f) = self.text.icon_family() {
                    style.family = f;
                }
                let layout = self.layout_text(g, &style, None);
                let s = layout.size();
                let pos = Vec2::new(r.min.x + (r.width() - s.x) * 0.5, r.min.y + (r.height() - s.y) * 0.5);
                self.paint_text(&layout, pos, color);
            }
        }
    }

    /// Draws an icon (vector or font glyph) of `size` in the current layout.
    pub fn icon_ex(&mut self, icon: IconRef<'_>, size: f32, color: Color) -> rugi_core::Rect {
        if !self.frame_guard() {
            return rugi_core::Rect::ZERO;
        }
        let r = self.allocate(Vec2::new(size, size), false);
        self.paint_icon(icon, r, color);
        r
    }
}
