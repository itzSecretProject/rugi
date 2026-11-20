//! Bullets, help markers, section separators, code blocks, plots, a color
//! picker and a list box.

use crate::{Channel, Response, Sense, Ui};
use rugi_core::draw::Fill;
use rugi_core::{id::split_label, Color, FontFamily, Key, Rect, TextStyle, Vec2};
use std::rc::Rc;

/// How long the code block's copy button shows its confirmation (seconds).
const COPIED_FEEDBACK: f64 = 1.2;
/// Inset of plot content from the plot frame.
const PLOT_INSET: f32 = 8.0;
/// Gap between the y-axis labels and the plot area.
const PLOT_AXIS_GAP: f32 = 6.0;
const SWATCH_SIZE: f32 = 30.0;
/// Largest hue the picker produces; 360 would wrap back to red.
const MAX_HUE: f32 = 359.964;

/// The smallest of 1, 2, 2.5 or 5 times a power of ten that is at least `x`
/// (for axis ranges that end on round numbers).
fn nice_step(x: f32) -> f32 {
    if !(x.is_finite() && x > 0.0) {
        return 1.0;
    }
    let pow = 10f32.powf(x.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].into_iter().map(|m| m * pow).find(|s| *s >= x * 0.999).unwrap_or(10.0 * pow)
}

/// Appends a plot value with as many decimals as a range of `span` needs,
/// without trailing zeros.
fn push_axis_value(out: &mut String, v: f32, span: f32) {
    let span = span.abs();
    let decimals = if span >= 20.0 {
        0
    } else if span >= 2.0 {
        1
    } else if span >= 0.2 {
        2
    } else {
        3
    };
    let start = out.len();
    rugi_core::fmt::push_fixed(out, v, decimals);
    if out[start..].contains('.') {
        let trimmed = out.trim_end_matches('0').trim_end_matches('.').len();
        out.truncate(trimmed);
    }
}

#[derive(Default)]
struct CopiedAt(f64);

#[cfg(test)]
mod tests {
    use super::{nice_step, push_axis_value};

    #[test]
    fn axis_values_are_round_and_short() {
        assert_eq!(nice_step(20.25), 25.0);
        assert!((nice_step(0.07) - 0.1).abs() < 1e-6);
        assert_eq!(nice_step(3.0), 5.0);
        let fmt = |v: f32, span: f32| {
            let mut s = String::new();
            push_axis_value(&mut s, v, span);
            s
        };
        assert_eq!(fmt(100.0, 100.0), "100");
        assert_eq!(fmt(-1.0, 2.0), "-1");
        assert_eq!(fmt(0.5, 1.0), "0.5");
        assert_eq!(fmt(0.125, 0.5), "0.13");
        assert_eq!(fmt(0.0, 0.1), "0");
    }
}

#[derive(Default)]
struct HexEdit(String);

#[derive(Default)]
struct HsvState {
    h: f32,
    s: f32,
    v: f32,
    /// The color these HSV values were derived from (to keep the hue stable
    /// while saturation or value is zero).
    from: Option<Color>,
}

impl Ui {
    /// Text with a leading bullet.
    pub fn bullet_text(&mut self, text: &str) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let style = self.theme.typography.body;
        if let Some(r) = self.skip_offscreen_early(style.line_px(), false) {
            return r.rect;
        }
        let th = Rc::clone(&self.theme);
        let layout = self.layout_text(text, &style, None);
        let r = self.allocate(Vec2::new(layout.size().x + 18.0, layout.size().y), false);
        self.out.draw_list.circle(
            Vec2::new(r.min.x + 5.0, r.min.y + style.line_px() * 0.5),
            2.5,
            th.palette.text_muted,
            None,
        );
        self.paint_text(&layout, Vec2::new(r.min.x + 18.0, r.min.y), th.palette.text);
        r
    }

    /// A small "?" marker that shows `text` as a tooltip on hover.
    pub fn help_marker(&mut self, text: &str) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let id = self.make_id(text).with("help");
        let line = self.theme.typography.body.line_px();
        let r = self.allocate(Vec2::new(18.0, line), false);
        let resp = self.interact(id, r, Sense::HOVER);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let h = self.animate(id, Channel::Hover, resp.hovered as u8 as f32, th.motion.hover);
        let c = Vec2::new(r.center().x, r.center().y);
        self.out.draw_list.circle(
            c,
            7.5,
            p.surface_hover.lerp(p.accent_soft, h),
            Some((1.0, p.border_strong.lerp(p.accent, h))),
        );
        let cap = th.typography.caption.weight(rugi_core::FontWeight::SEMIBOLD);
        let l = self.layout_text("?", &cap, None);
        self.paint_text(&l, Vec2::new(c.x - l.size().x * 0.5, c.y - l.size().y * 0.5), p.text_muted.lerp(p.text, h));
        self.tooltip(text);
        resp
    }

    /// A section title followed by a hairline that fills the rest of the row.
    pub fn separator_text(&mut self, label: &str) -> Rect {
        if !self.frame_guard() {
            return Rect::ZERO;
        }
        let th = Rc::clone(&self.theme);
        let style = th.typography.caption.weight(rugi_core::FontWeight::SEMIBOLD).letter_spacing(0.04);
        let layout = self.layout_text(label, &style, None);
        let r = self.allocate(Vec2::new(0.0, layout.size().y + 8.0), true);
        let ty = r.min.y + 6.0;
        self.paint_text(&layout, Vec2::new(r.min.x, ty), th.palette.text_muted);
        let x = r.min.x + layout.size().x + 10.0;
        let y = (ty + layout.size().y * 0.5).round();
        if x < r.max.x {
            self.out.draw_list.rect(Rect::from_xywh(x, y, r.max.x - x, 1.0), 0.0, th.palette.border, None);
        }
        r
    }

    /// Monospace code (or any preformatted text) in a recessed box with a
    /// copy-to-clipboard button. Lines wider than the box wrap.
    pub fn code_block(&mut self, code: &str) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let id = self.make_id("##code").with(code);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let mut style = TextStyle::new(th.typography.body.size - 1.0);
        style.family = FontFamily::Mono;
        style.line_height = 1.5;
        let pad = 12.0;
        // Lines wider than the box wrap rather than being cut off.
        let wrap = (self.available_width() - pad * 2.0).max(1.0);
        let layout = self.layout_text(code, &style, Some(wrap));
        let r = self.allocate(Vec2::new(0.0, layout.size().y + pad * 2.0), true);
        self.out.draw_list.rect(r, th.metrics.radius_md, p.sunken, Some((1.0, p.border)));
        self.out.draw_list.push_clip(r.shrink2(1.0, 1.0));
        self.paint_text(&layout, Vec2::new(r.min.x + pad, r.min.y + pad), p.text.lerp(p.text_muted, 0.15));
        self.out.draw_list.pop_clip();
        let br = Rect::from_xywh(r.max.x - 34.0, r.min.y + 6.0, 28.0, 28.0);
        let resp = self.interact(id, br, Sense::BUTTON);
        if resp.clicked {
            self.out.clipboard = Some(code.to_owned());
            self.state_mut::<CopiedAt>(id).0 = self.time;
        }
        let copied_at = self.state_mut::<CopiedAt>(id).0;
        let copied = copied_at > 0.0 && self.time - copied_at < COPIED_FEEDBACK;
        if copied {
            self.request_repaint_after(std::time::Duration::from_millis(200));
        }
        let h = self.animate(
            id,
            Channel::Hover,
            (resp.hovered || resp.focused || r.contains(self.input.pointer.unwrap_or(Vec2::new(-1.0, -1.0)))) as u8
                as f32,
            th.motion.hover,
        );
        if h > 0.0 || copied {
            self.out.draw_list.rect(
                br,
                th.metrics.radius_sm,
                p.surface_hover.fade(h.max(copied as u8 as f32)),
                Some((1.0, p.border.fade(h))),
            );
            let (icon, col) =
                if copied { (crate::lucide::CHECK, p.success) } else { (crate::lucide::COPY, p.text_muted) };
            self.paint_icon(
                icon.into(),
                Rect::from_xywh(br.min.x + 7.0, br.min.y + 7.0, 14.0, 14.0),
                col.fade(h.max(copied as u8 as f32)),
            );
        }
        if self.focus_ring_visible(id) {
            self.focus_ring(br, th.metrics.radius_sm, 1.0);
        }
        self.record("code_block", "Copy to clipboard", &resp);
        self.tooltip(if copied { "Copied" } else { "Copy to clipboard" });
        Response { rect: r, ..resp }
    }

    /// Allocates a plot of `height` with its frame, grid lines and, when
    /// `axis` is given, the y-axis range labels in a left gutter. Returns the
    /// data area and the hover response.
    fn plot_frame(&mut self, id_source: &str, height: f32, axis: Option<(f32, f32)>) -> (Rect, Response) {
        let id = self.make_id(id_source);
        let r = self.allocate(Vec2::new(0.0, height), true);
        let resp = self.interact(id, r, Sense::HOVER);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        self.out.draw_list.rect(r, th.metrics.radius_md, p.sunken, Some((1.0, p.border)));
        let mut inner = r.shrink2(PLOT_INSET, PLOT_INSET);
        if let Some((lo, hi)) = axis {
            let style = th.typography.caption;
            let mut buf = std::mem::take(&mut self.fmt_buf);
            buf.clear();
            push_axis_value(&mut buf, hi, hi - lo);
            let top = self.layout_text(&buf, &style, None);
            buf.clear();
            push_axis_value(&mut buf, lo, hi - lo);
            let bottom = self.layout_text(&buf, &style, None);
            self.fmt_buf = buf;
            let gutter = top.size().x.max(bottom.size().x).ceil();
            if gutter + PLOT_AXIS_GAP < inner.width() * 0.5 {
                let half = (top.size().y * 0.5).ceil();
                inner = Rect::from_min_max(
                    Vec2::new(inner.min.x + gutter + PLOT_AXIS_GAP, r.min.y + PLOT_INSET.max(half + 2.0)),
                    Vec2::new(inner.max.x, r.max.y - PLOT_INSET.max(half + 2.0)),
                );
                let right = inner.min.x - PLOT_AXIS_GAP;
                let color = p.text_muted;
                self.paint_text(&top, Vec2::new(right - top.size().x, (inner.min.y - half).round()), color);
                self.paint_text(&bottom, Vec2::new(right - bottom.size().x, (inner.max.y - half).round()), color);
            }
        }
        for i in 0..=4 {
            let y = (inner.min.y + inner.height() * i as f32 / 4.0).round();
            let alpha = if i == 0 || i == 4 { 0.9 } else { 0.5 };
            self.out.draw_list.rect(
                Rect::from_xywh(inner.min.x, y, inner.width(), 1.0),
                0.0,
                p.border.fade(alpha),
                None,
            );
        }
        (inner, resp)
    }

    /// Position of `v` within `lo..hi` as 0..=1; NaN (from a NaN sample,
    /// bound or an empty range) maps to the bottom of the plot.
    #[inline]
    fn plot_t(v: f32, lo: f32, hi: f32) -> f32 {
        let t = (v - lo) / (hi - lo);
        if t.is_nan() {
            0.0
        } else {
            t.clamp(0.0, 1.0)
        }
    }

    /// `range`, or the data's range widened to round numbers.
    fn plot_range(values: &[f32], range: Option<(f32, f32)>) -> (f32, f32) {
        range.unwrap_or_else(|| {
            let lo = values.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            if lo.is_finite() && hi > lo {
                let step = nice_step((hi - lo) / 4.0);
                ((lo / step).floor() * step, (hi / step).ceil() * step)
            } else {
                (lo.min(0.0), lo.max(0.0) + 1.0)
            }
        })
    }

    /// Line plot of `values` (oldest first). `range` fixes the y axis
    /// (auto-fit otherwise). Hover shows the nearest sample.
    pub fn plot_lines(&mut self, id_source: &str, values: &[f32], range: Option<(f32, f32)>, height: f32) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (lo, hi) = Self::plot_range(values, range);
        let axis = (!values.is_empty()).then_some((lo, hi));
        let (inner, resp) = self.plot_frame(id_source, height, axis);
        if values.len() < 2 {
            return resp;
        }
        let n = values.len();
        let pt = |i: usize| {
            let t = Self::plot_t(values[i], lo, hi);
            Vec2::new(inner.min.x + inner.width() * i as f32 / (n - 1) as f32, inner.max.y - inner.height() * t)
        };
        let accent = self.theme.palette.accent;
        // Soft area under the curve: thin vertical strips with a fading gradient.
        let strip = (inner.width() / (n - 1) as f32).max(1.0);
        for i in 0..n {
            let p = pt(i);
            let rect = Rect::from_min_max(Vec2::new(p.x - strip * 0.5, p.y), Vec2::new(p.x + strip * 0.5, inner.max.y));
            self.out.draw_list.rect(rect, 0.0, Fill::Vertical(accent.with_alpha(0.22), accent.with_alpha(0.0)), None);
        }
        for i in 1..n {
            self.out.draw_list.line(pt(i - 1), pt(i), 1.8, accent);
        }
        self.plot_hover(&resp, inner, n, hi - lo, |i| (pt(i), values[i]));
        resp
    }

    /// Bar chart of `values`. `range` fixes the y axis (auto-fit from 0 otherwise).
    pub fn plot_histogram(
        &mut self,
        id_source: &str,
        values: &[f32],
        range: Option<(f32, f32)>,
        height: f32,
    ) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (lo, hi) = range.unwrap_or_else(|| {
            let max = values.iter().copied().fold(0.0f32, f32::max).max(1e-6);
            let step = nice_step(max / 4.0);
            (0.0, (max / step).ceil() * step)
        });
        let axis = (!values.is_empty()).then_some((lo, hi));
        let (inner, resp) = self.plot_frame(id_source, height, axis);
        if values.is_empty() {
            return resp;
        }
        let n = values.len();
        let slot = inner.width() / n as f32;
        let gap = (slot * 0.25).min(4.0);
        let th = Rc::clone(&self.theme);
        let hovered_i =
            self.input.pointer.filter(|_| resp.hovered).map(|p| (((p.x - inner.min.x) / slot) as usize).min(n - 1));
        for (i, v) in values.iter().enumerate() {
            let t = Self::plot_t(*v, lo, hi);
            let x = inner.min.x + i as f32 * slot + gap * 0.5;
            let bar = Rect::from_min_max(
                Vec2::new(x, inner.max.y - inner.height() * t),
                Vec2::new(x + slot - gap, inner.max.y),
            );
            let c = if hovered_i == Some(i) { th.palette.accent_hover } else { th.palette.accent };
            let rad = ((slot - gap) * 0.3).min(4.0);
            self.out.draw_list.rect_corners(bar, [rad, rad, 0.0, 0.0], Fill::Solid(c), None);
        }
        self.plot_hover(&resp, inner, n, hi - lo, |i| {
            let t = Self::plot_t(values[i], lo, hi);
            (Vec2::new(inner.min.x + (i as f32 + 0.5) * slot, inner.max.y - inner.height() * t), values[i])
        });
        resp
    }

    fn plot_hover(&mut self, resp: &Response, inner: Rect, n: usize, span: f32, at: impl Fn(usize) -> (Vec2, f32)) {
        let Some(p) = self.input.pointer.filter(|_| resp.hovered) else { return };
        let i = if n <= 1 {
            0
        } else {
            (((p.x - inner.min.x) / inner.width()) * (n - 1) as f32).round().clamp(0.0, (n - 1) as f32) as usize
        };
        let (pt, v) = at(i);
        let th = Rc::clone(&self.theme);
        self.out.draw_list.rect(
            Rect::from_xywh(pt.x - 0.5, inner.min.y, 1.0, inner.height()),
            0.0,
            th.palette.text_faint.fade(0.6),
            None,
        );
        self.out.draw_list.circle(pt, 4.0, th.palette.surface, Some((2.0, th.palette.accent)));
        let mut buf = std::mem::take(&mut self.fmt_buf);
        buf.clear();
        buf.push('#');
        rugi_core::fmt::push_fixed(&mut buf, i as f32, 0);
        buf.push_str("   ");
        push_axis_value(&mut buf, v, span);
        self.last_response = *resp;
        self.hover_since = (Some(resp.id), self.time - 1.0);
        self.tooltip(&buf);
        self.fmt_buf = buf;
    }

    /// Color editor: a swatch and hex value that open a picker with a
    /// saturation/value square, hue bar, alpha slider and hex input.
    /// Arrow-key step of the color picker: 1%, or 10% with Shift.
    fn color_key_step(&self) -> f32 {
        if self.input.modifiers.shift {
            0.1
        } else {
            0.01
        }
    }

    /// `.changed` is set while the color is edited.
    pub fn color_edit(&mut self, label: &str, color: &mut Color) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let h = th.metrics.control_height;
        let body = th.typography.body;
        let label_layout = self.layout_text(text, &body, None);
        let mut hex = std::mem::take(&mut self.fmt_buf);
        hex.clear();
        hex.push('#');
        for b in [color.r, color.g, color.b] {
            const D: &[u8; 16] = b"0123456789ABCDEF";
            hex.push(D[(b >> 4) as usize] as char);
            hex.push(D[(b & 15) as usize] as char);
        }
        let mut mono = TextStyle::new(body.size - 0.5);
        mono.family = FontFamily::Mono;
        let hex_layout = self.layout_text(&hex, &mono, None);
        self.fmt_buf = hex;
        let w =
            SWATCH_SIZE + 10.0 + hex_layout.size().x + if text.is_empty() { 0.0 } else { 14.0 + label_layout.size().x };
        let r = self.allocate(Vec2::new(w, h), false);
        let sw = Rect::from_xywh(r.min.x, r.min.y + (h - SWATCH_SIZE) * 0.5, SWATCH_SIZE, SWATCH_SIZE);
        let mut resp = self.interact(
            id,
            Rect::from_min_max(r.min, Vec2::new(sw.max.x + 10.0 + hex_layout.size().x, r.max.y)),
            Sense::BUTTON,
        );
        if resp.clicked {
            if self.is_popup_open(id) {
                self.close_popup();
            } else {
                self.open_popup(id, sw);
            }
        }
        let v = self.visual(&resp);
        // Checkerboard behind translucent colors.
        if color.a < 255 {
            let half = SWATCH_SIZE * 0.5;
            self.out.draw_list.rect(sw, th.metrics.radius_sm + 1.0, Color::WHITE, None);
            for (x, y) in [(0.0, 0.0), (half, half)] {
                self.out.draw_list.rect(
                    Rect::from_xywh(sw.min.x + x, sw.min.y + y, half, half),
                    0.0,
                    Color::hex(0xCCCCCC),
                    None,
                );
            }
        }
        self.out.draw_list.rect(
            sw,
            th.metrics.radius_sm + 1.0,
            *color,
            Some((1.0, p.border_strong.lerp(p.text_faint, v.hover))),
        );
        let ty = r.min.y + ((h - hex_layout.size().y) * 0.5).round();
        self.paint_text(&hex_layout, Vec2::new(sw.max.x + 10.0, ty), p.text_muted.lerp(p.text, v.hover));
        if !text.is_empty() {
            let lx = sw.max.x + 10.0 + hex_layout.size().x + 14.0;
            self.paint_text(
                &label_layout,
                Vec2::new(lx, r.min.y + ((h - label_layout.size().y) * 0.5).round()),
                p.text,
            );
        }
        self.focus_ring(sw, th.metrics.radius_sm + 1.0, v.focus);
        self.record("color_edit", text, &resp);

        if self.begin_popup_below(id, sw, 248.0) {
            // HSV state survives while the picker is open; resync when the color changed outside.
            let st = self.state_mut::<HsvState>(id);
            if st.from != Some(*color) {
                let (hh, ss, vv) = color.to_hsv();
                if ss > 0.0 && vv > 0.0 {
                    st.h = hh;
                }
                if vv > 0.0 {
                    st.s = ss;
                }
                st.v = vv;
                st.from = Some(*color);
            }
            let (mut hh, mut ss, mut vv) = (st.h, st.s, st.v);
            let before = (hh, ss, vv, color.a);
            let mut alpha = color.a as f32 / 255.0;
            // Saturation / value square.
            let sq = self.allocate(Vec2::new(0.0, 150.0), true);
            let sq_resp = self.interact(id.with("sv"), sq, Sense::DRAG);
            self.record("color_square", "Saturation and value", &sq_resp);
            if sq_resp.pressed {
                if let Some(ptr) = self.input.pointer {
                    ss = ((ptr.x - sq.min.x) / sq.width()).clamp(0.0, 1.0);
                    vv = 1.0 - ((ptr.y - sq.min.y) / sq.height()).clamp(0.0, 1.0);
                }
            }
            if sq_resp.focused {
                let step = self.color_key_step();
                for i in 0..self.input.keys.len() {
                    match self.input.keys[i].0 {
                        Key::ArrowLeft => ss -= step,
                        Key::ArrowRight => ss += step,
                        Key::ArrowDown => vv -= step,
                        Key::ArrowUp => vv += step,
                        _ => {}
                    }
                }
                (ss, vv) = (ss.clamp(0.0, 1.0), vv.clamp(0.0, 1.0));
            }
            let hue = Color::from_hsv(hh, 1.0, 1.0);
            self.out.draw_list.rect(sq, 6.0, Fill::Horizontal(Color::WHITE, hue), None);
            self.out.draw_list.rect(
                sq,
                6.0,
                Fill::Vertical(Color::rgba(0, 0, 0, 0), Color::BLACK),
                Some((1.0, p.border)),
            );
            let knob = Vec2::new(sq.min.x + ss * sq.width(), sq.min.y + (1.0 - vv) * sq.height());
            self.out.draw_list.circle(knob, 7.0, Color::from_hsv(hh, ss, vv), Some((2.5, Color::WHITE)));
            let v = self.visual(&sq_resp);
            self.focus_ring(sq, 6.0, v.focus);
            // Hue bar: six gradient segments.
            let hb = self.allocate(Vec2::new(0.0, 14.0), true);
            let hb_resp = self.interact(id.with("hue"), hb, Sense::DRAG);
            self.record("hue_bar", "Hue", &hb_resp);
            if hb_resp.pressed {
                if let Some(ptr) = self.input.pointer {
                    hh = ((ptr.x - hb.min.x) / hb.width() * 360.0).clamp(0.0, MAX_HUE);
                }
            }
            if hb_resp.focused {
                let step = self.color_key_step() * 360.0;
                for i in 0..self.input.keys.len() {
                    match self.input.keys[i].0 {
                        Key::ArrowLeft | Key::ArrowDown => hh -= step,
                        Key::ArrowRight | Key::ArrowUp => hh += step,
                        Key::Home => hh = 0.0,
                        Key::End => hh = MAX_HUE,
                        _ => {}
                    }
                }
                hh = hh.clamp(0.0, MAX_HUE);
            }
            let seg = hb.width() / 6.0;
            for k in 0..6 {
                let a = Color::from_hsv(k as f32 * 60.0, 1.0, 1.0);
                let b = Color::from_hsv((k + 1) as f32 * 60.0, 1.0, 1.0);
                let rr = [
                    if k == 0 { 7.0 } else { 0.0 },
                    if k == 5 { 7.0 } else { 0.0 },
                    if k == 5 { 7.0 } else { 0.0 },
                    if k == 0 { 7.0 } else { 0.0 },
                ];
                self.out.draw_list.rect_corners(
                    Rect::from_xywh(hb.min.x + seg * k as f32, hb.min.y, seg + 0.5, hb.height()),
                    rr,
                    Fill::Horizontal(a, b),
                    None,
                );
            }
            let hx = hb.min.x + hh / 360.0 * hb.width();
            self.out.draw_list.circle(Vec2::new(hx, hb.center().y), 8.0, hue, Some((2.5, Color::WHITE)));
            let v = self.visual(&hb_resp);
            self.focus_ring(hb, 7.0, v.focus);
            self.slider("Alpha##alpha", &mut alpha, 0.0, 1.0);
            let mut text_buf = std::mem::take(&mut self.scratch);
            text_buf.clear();
            let cur = Color::from_hsv(hh, ss, vv);
            for b in [cur.r, cur.g, cur.b] {
                use std::fmt::Write as _;
                let _ = write!(text_buf, "{b:02X}");
            }
            let mut edit = self.state_mut::<HexEdit>(id).0.clone();
            let field_focused = self.focused == Some(self.make_id("##hex"));
            if !field_focused {
                edit = text_buf.clone();
            }
            let tr = self.text_input("##hex", &mut edit, "RRGGBB");
            if tr.changed {
                let digits: String = edit.chars().filter(|c| c.is_ascii_hexdigit()).take(6).collect();
                if digits.len() == 6 {
                    if let Ok(n) = u32::from_str_radix(&digits, 16) {
                        let (nh, ns, nv) = Color::hex(n).to_hsv();
                        if ns > 0.0 && nv > 0.0 {
                            hh = nh;
                        }
                        ss = ns;
                        vv = nv;
                    }
                }
            }
            self.state_mut::<HexEdit>(id).0 = edit;
            self.scratch = text_buf;
            self.end();

            if (hh, ss, vv, (alpha * 255.0 + 0.5) as u8) != before {
                let c = Color::from_hsv(hh, ss, vv);
                *color = Color::rgba(c.r, c.g, c.b, (alpha * 255.0 + 0.5) as u8);
                resp.changed = true;
            }
            let st = self.state_mut::<HsvState>(id);
            st.h = hh;
            st.s = ss;
            st.v = vv;
            st.from = Some(*color);
        }
        if resp.focused && self.input.key_pressed(Key::Enter) && !self.is_popup_open(id) {
            self.open_popup(id, sw);
        }
        resp
    }

    /// A bordered, scrollable list of selectable items (QListWidget).
    pub fn list_box(&mut self, id_source: &str, selected: &mut usize, items: &[&str], height: f32) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let id = self.make_id(id_source);
        let th = Rc::clone(&self.theme);
        let (origin, w) = self.top().peek();
        let height = crate::layout::extent(height);
        let frame = Rect::from_min_size(origin, Vec2::new(w, height));
        self.out.draw_list.rect(frame, th.metrics.radius_md, th.palette.sunken, Some((1.0, th.palette.border)));
        let mut out = Response { id, rect: frame, ..Default::default() };
        self.begin_scroll_area_id(id.with("scroll"), Some(height));
        self.top().spacing = 1.0;
        self.space(4.0);
        for (i, item) in items.iter().enumerate() {
            self.push_id_u64(i as u64);
            self.begin_horizontal(crate::Align::Start);
            self.space(4.0);
            self.set_next_item_width((w - 22.0).max(0.0));
            if self.selectable(item, i == *selected).clicked && *selected != i {
                *selected = i;
                out.changed = true;
            }
            self.end();
            self.pop_id();
        }
        self.space(4.0);
        self.end();
        out
    }
}
