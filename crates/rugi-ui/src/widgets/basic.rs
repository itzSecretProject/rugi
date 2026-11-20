//! Text, separators, badges, progress bars, images and icons.

use crate::{Icon, Response, Sense, Ui};
use rugi_core::{id::split_label, Color, Rect, TextStyle, TextureId, Vec2};

const PROGRESS_HEIGHT: f32 = 6.0;
const BADGE_HEIGHT: f32 = 22.0;
const BADGE_PADDING_X: f32 = 8.0;
/// Height of a key cap drawn by [`Ui::kbd`].
pub(crate) const KBD_HEIGHT: f32 = 20.0;
const KBD_PADDING_X: f32 = 6.0;

/// Semantic text roles mapped to theme typography and colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum TextRole {
    /// Primary body text.
    Body = 0,
    /// Secondary text.
    Muted = 1,
    /// Section heading.
    Heading = 2,
    /// Page title.
    Title = 3,
    /// Small caption.
    Caption = 4,
    /// Large numeric display.
    Display = 5,
}

impl TextRole {
    /// Converts from the C ABI integer.
    pub fn from_u32(v: u32) -> Option<TextRole> {
        Some(match v {
            0 => TextRole::Body,
            1 => TextRole::Muted,
            2 => TextRole::Heading,
            3 => TextRole::Title,
            4 => TextRole::Caption,
            5 => TextRole::Display,
            _ => return None,
        })
    }
}

impl Ui {
    pub(crate) fn role_style(&self, role: TextRole) -> (TextStyle, Color) {
        let t = &self.theme.typography;
        let p = &self.theme.palette;
        match role {
            TextRole::Body => (t.body, p.text),
            TextRole::Muted => (t.body, p.text_muted),
            TextRole::Heading => (t.heading, p.text),
            TextRole::Title => (t.title, p.text),
            TextRole::Caption => (t.caption, p.text_muted),
            TextRole::Display => (t.display, p.text),
        }
    }

    /// Text with an explicit style and color. `wrap` wraps to the available width.
    pub fn text_styled(&mut self, text: &str, style: TextStyle, color: Color, wrap: bool) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        if !wrap && !self.inspection && !self.tools.inspector && self.next_offscreen(style.line_px()) {
            return self.allocate_offscreen(Vec2::new(0.0, style.line_px()), false);
        }
        let wrap_w = if wrap { Some(self.available_width()) } else { None };
        let mut layout = self.layout_text(text, &style, wrap_w);
        let r = self.allocate(layout.size(), false);
        // Text a clip rectangle would cut off ends in an ellipsis instead.
        let clip_x = self.out.draw_list.clip().max.x;
        if !wrap && r.max.x > clip_x + 0.5 && r.min.x < clip_x && !text.contains('\n') {
            layout = self.layout_text_fit(text, &style, clip_x - r.min.x);
        }
        self.paint_text(&layout, r.min, color);
        if self.inspection {
            let resp = Response { rect: r, ..Default::default() };
            self.record("label", text, &resp);
        }
        r
    }

    /// Text in a semantic role, horizontally centered in a full-width row.
    pub fn text_centered(&mut self, text: &str, role: TextRole) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let (style, color) = self.role_style(role);
        let layout = self.layout_text(text, &style, None);
        let row = self.allocate(Vec2::new(0.0, layout.size().y), true);
        let x = (row.min.x + (row.width() - layout.size().x) * 0.5).round();
        self.paint_text(&layout, Vec2::new(x, row.min.y), color);
        let r = Rect::from_min_size(Vec2::new(x, row.min.y), layout.size());
        if self.inspection {
            let resp = Response { rect: r, ..Default::default() };
            self.record("label", text, &resp);
        }
        r
    }

    /// Text in a semantic role.
    pub fn text(&mut self, text: &str, role: TextRole) -> Rect {
        let (style, color) = self.role_style(role);
        self.text_styled(text, style, color, false)
    }

    /// Body text.
    pub fn label(&mut self, text: &str) -> Rect {
        self.text(text, TextRole::Body)
    }

    /// Body text from format arguments, formatted only if it will be visible:
    /// `ui.label_fmt(format_args!("Row {i}: {v:.1}"))`. Rows scrolled out of
    /// view cost no formatting at all, and nothing is allocated per frame.
    pub fn label_fmt(&mut self, args: std::fmt::Arguments<'_>) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let (style, color) = self.role_style(TextRole::Body);
        if !self.inspection && !self.tools.inspector && self.next_offscreen(style.line_px()) {
            return self.allocate_offscreen(Vec2::new(0.0, style.line_px()), false);
        }
        if let Some(s) = args.as_str() {
            return self.text_styled(s, style, color, false);
        }
        let mut buf = std::mem::take(&mut self.fmt_buf);
        buf.clear();
        let _ = std::fmt::Write::write_fmt(&mut buf, args);
        let r = self.text_styled(&buf, style, color, false);
        self.fmt_buf = buf;
        r
    }

    /// Secondary text.
    pub fn muted(&mut self, text: &str) -> Rect {
        self.text(text, TextRole::Muted)
    }

    /// Section heading.
    pub fn heading(&mut self, text: &str) -> Rect {
        self.text(text, TextRole::Heading)
    }

    /// Page title.
    pub fn title(&mut self, text: &str) -> Rect {
        self.text(text, TextRole::Title)
    }

    /// Small caption.
    pub fn caption(&mut self, text: &str) -> Rect {
        self.text(text, TextRole::Caption)
    }

    /// Single-line text that is shortened with "…" when wider than the
    /// available width (or `max_width` if given).
    pub fn label_truncated(&mut self, text: &str, role: TextRole, max_width: Option<f32>) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let (style, color) = self.role_style(role);
        let max_w = max_width.unwrap_or_else(|| self.available_width());
        let layout = self.layout_text_fit(text, &style, max_w);
        let r = self.allocate(layout.size(), false);
        self.paint_text(&layout, r.min, color);
        if self.inspection {
            let resp = Response { rect: r, ..Default::default() };
            self.record("label", text, &resp);
        }
        r
    }

    /// Wrapped secondary paragraph.
    pub fn paragraph(&mut self, text: &str) -> Rect {
        let (style, color) = self.role_style(TextRole::Muted);
        self.text_styled(text, style, color, true)
    }

    /// Horizontal hairline.
    pub fn separator(&mut self) {
        if !self.frame_guard() {
            return;
        }
        let r = self.allocate(Vec2::new(0.0, 1.0), true);
        let c = self.theme.palette.border;
        self.out.draw_list.rect(r, 0.0, c, None);
    }

    /// Small pill with text, tinted by `color`.
    pub fn badge(&mut self, text: &str, color: Color) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let style = self.theme.typography.caption.weight(rugi_core::FontWeight::MEDIUM);
        let layout = self.layout_text(text, &style, None);
        let size = Vec2::new(layout.size().x + BADGE_PADDING_X * 2.0, BADGE_HEIGHT);
        let r = self.allocate(size, false);
        self.out.draw_list.rect(r, BADGE_HEIGHT * 0.5, color.with_alpha(0.14), Some((1.0, color.with_alpha(0.22))));
        let ty = r.min.y + (r.height() - layout.size().y) * 0.5;
        self.paint_text(&layout, Vec2::new(r.min.x + BADGE_PADDING_X, ty), color);
        r
    }

    /// A keyboard shortcut drawn as a key cap, e.g. `ui.kbd("Ctrl K")`.
    pub fn kbd(&mut self, keys: &str) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let w = self.kbd_width(keys);
        let r = self.allocate(Vec2::new(w, KBD_HEIGHT), false);
        self.paint_kbd(keys, r);
        r
    }

    pub(crate) fn kbd_width(&mut self, keys: &str) -> f32 {
        let style = self.theme.typography.caption.weight(rugi_core::FontWeight::MEDIUM);
        (self.layout_text(keys, &style, None).size().x + KBD_PADDING_X * 2.0).ceil()
    }

    pub(crate) fn paint_kbd(&mut self, keys: &str, r: Rect) {
        let th = std::rc::Rc::clone(&self.theme);
        let p = &th.palette;
        let style = th.typography.caption.weight(rugi_core::FontWeight::MEDIUM);
        let l = self.layout_text(keys, &style, None);
        self.out.draw_list.rect(r, th.metrics.radius_sm, p.surface_hover, Some((1.0, p.border_strong)));
        let pos = Vec2::new(r.min.x + KBD_PADDING_X, r.min.y + ((r.height() - l.size().y) * 0.5).round());
        self.paint_text(&l, pos, p.text_muted);
    }

    /// Progress bar (`fraction` in 0..=1) filling the available width.
    pub fn progress_bar(&mut self, fraction: f32) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let f = if fraction.is_finite() { fraction.clamp(0.0, 1.0) } else { 0.0 };
        let h = PROGRESS_HEIGHT;
        let r = self.allocate(Vec2::new(0.0, h), true);
        let p = &self.theme.palette;
        self.out.draw_list.rect(r, h * 0.5, p.surface_active, None);
        if f > 0.0 {
            let fill = Rect::from_min_size(r.min, Vec2::new((r.width() * f).max(h), h));
            self.out.draw_list.rect(fill, h * 0.5, p.accent, None);
        }
        r
    }

    /// Draws an icon of `size` in the current layout.
    pub fn icon(&mut self, icon: Icon, size: f32, color: Color) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let r = self.allocate(Vec2::new(size, size), false);
        icon.paint(&mut self.out.draw_list, r, color);
        r
    }

    /// Displays a renderer-registered texture.
    pub fn image(&mut self, texture: TextureId, size: Vec2, radius: f32) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let r = self.allocate(size, false);
        self.out.draw_list.image(r, texture, [0.0, 0.0, 1.0, 1.0], Color::WHITE, radius);
        r
    }

    /// An invisible hover-sensing region (useful for custom widgets).
    pub fn hover_region(&mut self, id_source: &str, size: Vec2) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (_, src) = split_label(id_source);
        let id = self.make_id(src);
        let r = self.allocate(size, false);
        self.interact(id, r, Sense::HOVER)
    }
}
