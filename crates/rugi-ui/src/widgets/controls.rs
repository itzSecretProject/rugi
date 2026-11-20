//! Buttons, toggles, sliders and navigation items.

use super::basic::KBD_HEIGHT;
use super::numeric::Numeric;
use super::{GLIDE_OMEGA, ICON_SIZE, LABEL_GAP};
use crate::{Channel, IconRef, Response, Sense, Ui};
use rugi_core::anim::Spring;
use rugi_core::draw::{DrawList, Fill};
use rugi_core::{id::split_label, Color, CursorIcon, Key, Rect, Vec2};
use std::rc::Rc;

/// Gap between a button's icon and its label.
const ICON_GAP: f32 = 8.0;
/// Distance between a widget's edge and its focus ring.
const FOCUS_RING_GAP: f32 = 3.0;
/// Track size of a toggle switch.
const TOGGLE_SIZE: Vec2 = Vec2 { x: 34.0, y: 20.0 };
/// Default width of a search button.
const SEARCH_BUTTON_WIDTH: f32 = 280.0;
/// Edge length of the magnifier in a search button.
const SEARCH_ICON: f32 = 14.0;
/// Fill of toggle knobs and slider thumbs.
const KNOB_COLOR: Color = Color::hex(0xF7F8FA);

/// Visual emphasis of a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum ButtonKind {
    /// Filled with the accent color.
    #[default]
    Primary = 0,
    /// Neutral raised surface.
    Secondary = 1,
    /// Text-only until hovered.
    Ghost = 2,
    /// Destructive action.
    Danger = 3,
}

impl ButtonKind {
    /// Converts from the C ABI integer.
    pub fn from_u32(v: u32) -> Option<ButtonKind> {
        Some(match v {
            0 => ButtonKind::Primary,
            1 => ButtonKind::Secondary,
            2 => ButtonKind::Ghost,
            3 => ButtonKind::Danger,
            _ => return None,
        })
    }
}

#[derive(Default)]
struct NavHighlight {
    y: Spring,
    h: Spring,
    init: bool,
}

/// Animated interaction state of a widget.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Visual {
    pub hover: f32,
    pub press: f32,
    pub focus: f32,
}

impl Ui {
    pub(crate) fn visual(&mut self, r: &Response) -> Visual {
        let ring = self.focus_ring_visible(r.id);
        let m = &self.theme.motion;
        let (th, tp) = (m.hover, m.press);
        let targets = [(r.hovered || r.pressed) as u8 as f32, r.pressed as u8 as f32, ring as u8 as f32];
        // One map lookup for all three channels.
        let [hover, press, focus] = self.animate3(r.id, targets, [th, tp, th]);
        Visual { hover, press, focus }
    }

    pub(crate) fn focus_ring(&mut self, r: Rect, radius: f32, t: f32) {
        if t <= 0.0 {
            return;
        }
        let c = self.theme.palette.accent.with_alpha(0.55 * t);
        let w = self.theme.metrics.focus_ring_width;
        self.out.draw_list.rect(r.expand(FOCUS_RING_GAP), radius + FOCUS_RING_GAP, Color::TRANSPARENT, Some((w, c)));
    }

    /// A primary button. Returns its [`Response`]; check `.clicked`.
    pub fn button(&mut self, label: &str) -> Response {
        self.button_kind(label, ButtonKind::Primary)
    }

    /// A button with explicit emphasis.
    pub fn button_kind(&mut self, label: &str, kind: ButtonKind) -> Response {
        self.button_ex(label, kind, None)
    }

    /// A button with an optional leading icon.
    pub fn button_ex(&mut self, label: &str, kind: ButtonKind, icon: Option<IconRef<'_>>) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        if let Some(r) = self.skip_offscreen_early(self.theme.metrics.control_height, false) {
            return r;
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        if self.next_offscreen(m.control_height) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(0.0, m.control_height), false);
        }
        let layout = self.layout_text(text, &th.typography.label, None);
        let icon_w = match (icon.is_some(), text.is_empty()) {
            (false, _) => 0.0,
            (true, true) => ICON_SIZE,
            (true, false) => ICON_SIZE + ICON_GAP,
        };
        let size = Vec2::new((layout.size().x + icon_w + m.control_padding_x * 2.0).ceil(), m.control_height);
        let r = self.allocate(size, false);
        let resp = self.interact(id, r, Sense::BUTTON);
        let v = self.visual(&resp);
        let radius = m.radius_md;
        let dl = &mut self.out.draw_list;
        let fg = match kind {
            ButtonKind::Primary | ButtonKind::Danger => {
                let (base, text) =
                    if kind == ButtonKind::Danger { (p.danger, p.on_danger) } else { (p.accent, p.on_accent) };
                dl.rect(r, radius, base.lighten(0.08 * v.hover).darken(0.12 * v.press), None);
                text
            }
            ButtonKind::Secondary => {
                let fill = p.surface_hover.lerp(p.surface_active, v.hover * 0.7 + v.press * 0.3);
                dl.rect(r, radius, fill, Some((m.border_width, p.border_strong.lerp(p.text_faint, v.hover * 0.3))));
                p.text
            }
            ButtonKind::Ghost => {
                dl.rect(r, radius, p.surface_hover.fade(v.hover).lerp(p.surface_active, v.press), None);
                p.text_muted.lerp(p.text, v.hover)
            }
        };
        let mut x = r.min.x + (r.width() - icon_w - layout.size().x) * 0.5;
        if let Some(icon) = icon {
            let ir = Rect::from_xywh(x, r.center().y - ICON_SIZE * 0.5, ICON_SIZE, ICON_SIZE);
            self.paint_icon(icon, ir, fg);
            x += icon_w;
        }
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(x, ty), fg);
        self.focus_ring(r, radius, v.focus);
        self.record("button", text, &resp);
        resp
    }

    /// A button drawn as a search field, with a hint and a shortcut key cap.
    /// The caller opens its command palette or search when it is clicked.
    ///
    /// It is 280 px wide unless [`Ui::set_next_item_width`] says otherwise;
    /// an empty `shortcut` hides the pill.
    pub fn search_button(&mut self, hint: &str, shortcut: &str) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (text, src) = split_label(hint);
        let id = self.make_id(src).with("search_button");
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        if self.next_offscreen(m.control_height) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(SEARCH_BUTTON_WIDTH, m.control_height), false);
        }
        let r = self.allocate(Vec2::new(SEARCH_BUTTON_WIDTH, m.control_height), false);
        let resp = self.interact(id, r, Sense::BUTTON);
        let v = self.visual(&resp);
        let radius = m.radius_md;
        let border = p.border.lerp(p.border_strong, v.hover);
        self.out.draw_list.rect(r, radius, p.sunken.lerp(p.surface_hover, v.hover * 0.6), Some((1.0, border)));
        let pad = m.control_padding_x - 2.0;
        let icon = Rect::from_xywh(r.min.x + pad, r.center().y - SEARCH_ICON * 0.5, SEARCH_ICON, SEARCH_ICON);
        self.paint_icon(crate::lucide::SEARCH.into(), icon, p.text_muted);
        let mut right = r.max.x - pad;
        if !shortcut.is_empty() {
            let w = self.kbd_width(shortcut);
            let kr = Rect::from_xywh(right - w, r.center().y - KBD_HEIGHT * 0.5, w, KBD_HEIGHT);
            self.paint_kbd(shortcut, kr);
            right = kr.min.x - ICON_GAP;
        }
        let x = icon.max.x + ICON_GAP;
        let layout = self.layout_text_fit(text, &th.typography.body, (right - x).max(0.0));
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(x, ty), p.text_muted);
        self.focus_ring(r, radius, v.focus);
        self.record("search_button", text, &resp);
        resp
    }

    /// A checkbox bound to `value`. `.changed` is set when toggled.
    pub fn checkbox(&mut self, label: &str, value: &mut bool) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let early_h = self.theme.metrics.checkbox_size.max(self.theme.typography.body.line_px());
        if let Some(r) = self.skip_offscreen_early(early_h, false) {
            return r;
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let style = th.typography.body;
        let bs = m.checkbox_size;
        if self.next_offscreen(bs.max(style.line_px())) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(0.0, bs.max(style.line_px())), false);
        }
        let layout = self.layout_text(text, &style, None);
        let gap = if text.is_empty() { 0.0 } else { LABEL_GAP };
        let h = bs.max(layout.size().y);
        let r = self.allocate(Vec2::new(bs + gap + layout.size().x, h), false);
        let mut resp = self.interact(id, r, Sense::BUTTON);
        if resp.clicked {
            *value = !*value;
            resp.changed = true;
        }
        let v = self.visual(&resp);
        let on = self.animate(id, Channel::Value, if *value { 1.0 } else { 0.0 }, th.motion.value);
        let b = Rect::from_xywh(r.min.x, r.min.y + (h - bs) * 0.5, bs, bs);
        let dl = &mut self.out.draw_list;
        let border = p.control_border.lerp(p.text_muted, v.hover).lerp(p.accent, on);
        let fill = p.sunken.lerp(p.accent.lighten(0.06 * v.hover), on);
        dl.rect(b, m.radius_sm, fill, Some((1.5 - 0.5 * on, border)));
        if on > 0.01 {
            let s = bs;
            let pt = |x: f32, y: f32| Vec2::new(b.min.x + x * s, b.min.y + y * s);
            let (a, k, c) = (pt(0.26, 0.52), pt(0.43, 0.68), pt(0.75, 0.33));
            let col = p.on_accent.fade(on);
            // The tick draws itself: first stroke, then second.
            let t1 = (on * 2.0).min(1.0);
            let t2 = ((on - 0.5) * 2.0).clamp(0.0, 1.0);
            dl.line(a, a + (k - a) * t1, 2.0, col);
            if t2 > 0.0 {
                dl.line(k, k + (c - k) * t2, 2.0, col);
            }
        }
        let ty = r.min.y + ((h - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(b.max.x + gap, ty), p.text);
        self.focus_ring(b, m.radius_sm, v.focus);
        self.record("checkbox", text, &resp);
        resp
    }

    /// A switch bound to `value`.
    pub fn toggle(&mut self, label: &str, value: &mut bool) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (track_w, track_h) = (TOGGLE_SIZE.x, TOGGLE_SIZE.y);
        if let Some(r) = self.skip_offscreen_early(track_h.max(self.theme.typography.body.line_px()), false) {
            return r;
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let style = th.typography.body;
        if self.next_offscreen(track_h.max(style.line_px())) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(0.0, track_h.max(style.line_px())), false);
        }
        let layout = self.layout_text(text, &style, None);
        let gap = if text.is_empty() { 0.0 } else { LABEL_GAP };
        let h = track_h.max(layout.size().y);
        let r = self.allocate(Vec2::new(track_w + gap + layout.size().x, h), false);
        let mut resp = self.interact(id, r, Sense::BUTTON);
        if resp.clicked {
            *value = !*value;
            resp.changed = true;
        }
        let v = self.visual(&resp);
        let on = self.animate(id, Channel::Value, if *value { 1.0 } else { 0.0 }, th.motion.value);
        let track = Rect::from_xywh(r.min.x, r.min.y + (h - track_h) * 0.5, track_w, track_h);
        let dl = &mut self.out.draw_list;
        let off_col = p.border_strong.lerp(p.text_faint, v.hover * 0.6);
        dl.rect(track, track_h * 0.5, off_col.lerp(p.accent, on), None);
        let knob_r = track_h * 0.5 - 2.5;
        let cx = track.min.x + track_h * 0.5 + (track_w - track_h) * on;
        let c = Vec2::new(cx, track.center().y);
        dl.shadow(
            Rect::from_min_size(c - Vec2::new(knob_r, knob_r - 1.0), Vec2::new(knob_r * 2.0, knob_r * 2.0)),
            knob_r,
            2.0,
            Color::rgba(0, 0, 0, 90),
        );
        dl.circle(c, knob_r, KNOB_COLOR, None);
        let ty = r.min.y + ((h - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(track.max.x + gap, ty), p.text);
        self.focus_ring(track, track_h * 0.5, v.focus);
        self.record("toggle", text, &resp);
        resp
    }

    /// A horizontal slider bound to `value` in `min..=max`, with the label and
    /// formatted value shown above the track. Arrow keys adjust it when focused
    /// (Shift for larger steps).
    pub fn slider(&mut self, label: &str, value: &mut f32, min: f32, max: f32) -> Response {
        self.slider_impl(label, value, min, max)
    }

    /// Integer slider; shows the value without decimals. Arrow keys step by
    /// one (Shift: a tenth of the range).
    pub fn slider_int(&mut self, label: &str, value: &mut i32, min: i32, max: i32) -> Response {
        self.slider_impl(label, value, min, max)
    }

    fn slider_impl<T: Numeric>(&mut self, label: &str, value: &mut T, min: T, max: T) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        if let Some(r) = self.skip_offscreen_early(self.theme.typography.body.line_px() + 26.0, true) {
            return r;
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let (lo, hi) = T::range(min, max);
        value.sanitize(lo);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let style = th.typography.body;
        let track_h = 20.0;
        let est_h = style.line_px() + 6.0 + track_h;
        if self.next_offscreen(est_h) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(0.0, est_h), true);
        }
        let label_layout = self.layout_text(text, &style, None);
        let mut value_text = std::mem::take(&mut self.scratch);
        value_text.clear();
        value.write(&mut value_text, T::slider_decimals(lo, hi));
        let value_style = style.weight(rugi_core::FontWeight::MEDIUM);
        let value_layout = self.layout_text(&value_text, &value_style, None);
        self.scratch = value_text;
        let line_h = label_layout.size().y.max(value_layout.size().y);
        let r = self.allocate(Vec2::new(0.0, line_h + 6.0 + track_h), true);
        let track_rect = Rect::from_xywh(r.min.x, r.min.y + line_h + 6.0, r.width(), track_h);
        let thumb_r = m.slider_thumb * 0.5;
        let usable = Rect::from_min_max(
            Vec2::new(track_rect.min.x + thumb_r, track_rect.min.y),
            Vec2::new(track_rect.max.x - thumb_r, track_rect.max.y),
        );
        let mut resp = self.interact(id, track_rect, Sense::DRAG);
        let before = *value;
        if resp.pressed {
            if let Some(ptr) = self.input.pointer {
                let t = ((ptr.x - usable.min.x) / usable.width().max(1.0)).clamp(0.0, 1.0);
                *value = T::at_fraction(t, lo, hi);
            }
            self.out.cursor = CursorIcon::Grab;
        }
        if resp.focused {
            let step = T::slider_step(lo, hi, self.input.modifiers.shift);
            for i in 0..self.input.keys.len() {
                match self.input.keys[i].0 {
                    Key::ArrowLeft | Key::ArrowDown => *value = value.add_steps(step, -1),
                    Key::ArrowRight | Key::ArrowUp => *value = value.add_steps(step, 1),
                    Key::Home => *value = lo,
                    Key::End => *value = hi,
                    _ => {}
                }
            }
            *value = value.clamp_to(lo, hi);
        }
        if *value != before {
            resp.changed = true;
        }
        if resp.hovered && !resp.pressed {
            self.out.cursor = CursorIcon::Pointer;
        }
        let v = self.visual(&resp);
        let t = value.fraction(lo, hi);
        self.paint_text(&label_layout, r.min, p.text_muted.lerp(p.text, v.hover * 0.6));
        let vx = r.max.x - value_layout.size().x;
        self.paint_text(&value_layout, Vec2::new(vx, r.min.y), p.text);

        let dl = &mut self.out.draw_list;
        let track_w = m.slider_track;
        let cy = track_rect.center().y;
        let track =
            Rect::from_xywh(usable.min.x - thumb_r * 0.5, cy - track_w * 0.5, usable.width() + thumb_r, track_w);
        dl.rect(track, track_w * 0.5, p.surface_active, None);
        let tx = usable.min.x + usable.width() * t;
        let filled = Rect::from_min_max(track.min, Vec2::new(tx, track.max.y));
        if filled.width() > 0.5 {
            dl.rect(filled, track_w * 0.5, p.accent, None);
        }
        let c = Vec2::new(tx, cy);
        let halo = thumb_r + 5.0 * v.hover.max(v.press);
        if halo > thumb_r + 0.1 {
            dl.circle(c, halo, p.accent.with_alpha(0.16 + 0.1 * v.press), None);
        }
        dl.shadow(
            Rect::from_xywh(c.x - thumb_r, c.y - thumb_r + 1.0, thumb_r * 2.0, thumb_r * 2.0),
            thumb_r,
            2.5,
            Color::rgba(0, 0, 0, 110),
        );
        dl.circle(c, thumb_r, KNOB_COLOR, Some((1.0, Color::rgba(0, 0, 0, 30))));
        dl.circle(c, thumb_r * 0.35 * (1.0 - v.press * 0.3), p.accent, None);
        let ring = Rect::from_xywh(c.x - thumb_r, c.y - thumb_r, thumb_r * 2.0, thumb_r * 2.0);
        self.focus_ring(ring, thumb_r, v.focus);
        self.record("slider", text, &resp);
        resp
    }

    /// A full-width navigation item (sidebar entry) with an optional icon.
    ///
    /// Items in the same container share one selection highlight that moves
    /// between them on a critically damped spring.
    pub fn nav_item(&mut self, label: &str, icon: Option<IconRef<'_>>, selected: bool) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let h = m.control_height + 2.0;
        let group = self.current_id().with("nav-group").with_u64(self.layouts.len() as u64);
        let frame = self.frame;
        let slots = match self.nav_slots {
            Some((g, f, bg, bar)) if g == group && f == frame => (bg, bar),
            _ => {
                let bg = self.out.draw_list.reserve();
                let bar = self.out.draw_list.reserve();
                self.nav_slots = Some((group, frame, bg, bar));
                (bg, bar)
            }
        };
        if !selected && self.next_offscreen(h) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(0.0, h), true);
        }
        let style = th.typography.label;
        let r = self.allocate(Vec2::new(0.0, h), true);
        let resp = self.interact(id, r, Sense::BUTTON);
        let v = self.visual(&resp);
        let sel = self.animate(id, Channel::Value, if selected { 1.0 } else { 0.0 }, th.motion.value);
        if selected {
            let dt = self.dt;
            let hl = self.state_mut::<NavHighlight>(group);
            if !hl.init {
                *hl = NavHighlight { y: Spring::at(r.min.y), h: Spring::at(r.height()), init: true };
            }
            let moving = hl.y.step(r.min.y, dt, GLIDE_OMEGA) | hl.h.step(r.height(), dt, GLIDE_OMEGA);
            let hr = Rect::from_xywh(r.min.x, hl.y.pos, r.width(), hl.h.pos);
            if moving {
                self.request_repaint();
            }
            let bg = DrawList::rect_instance(hr, [m.radius_md; 4], Fill::Solid(p.accent_soft), None);
            self.out.draw_list.fill_slot(slots.0, bg);
            let bar = Rect::from_xywh(hr.min.x, hr.min.y + 9.0, 3.0, (hr.height() - 18.0).max(2.0));
            self.out.draw_list.fill_slot(slots.1, DrawList::rect_instance(bar, [1.5; 4], Fill::Solid(p.accent), None));
        }
        if !selected && v.hover > 0.0 {
            self.out.draw_list.rect(r, m.radius_md, p.surface_hover.fade(v.hover * 0.85), None);
        }
        let fg = p.text_muted.lerp(p.text, v.hover.max(sel));
        let mut x = r.min.x + 14.0;
        if let Some(icon) = icon {
            let ic = p.text_muted.lerp(p.text, v.hover).lerp(p.accent_text, sel);
            self.paint_icon(icon, Rect::from_xywh(x, r.center().y - ICON_SIZE * 0.5, ICON_SIZE, ICON_SIZE), ic);
            x += 28.0;
        }
        let layout = self.layout_text_fit(text, &style, (r.max.x - 8.0 - x).max(0.0));
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(x, ty), fg);
        self.focus_ring(r, m.radius_md, v.focus);
        self.record("nav_item", text, &resp);
        resp
    }
}
