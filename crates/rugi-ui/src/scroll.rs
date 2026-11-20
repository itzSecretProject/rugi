//! Scroll areas: their per-id state, the scrollbar, and scrolling a focused
//! widget into view.

use crate::layout::{extent, Container, Dir, Layout};
use crate::{Channel, Response, Sense, Ui};
use rugi_core::{anim, Id, Rect, Vec2};
use std::rc::Rc;

/// Shortest scrollbar thumb, unless the track itself is shorter.
const MIN_SCROLL_THUMB: f32 = 24.0;

/// Space kept between a widget scrolled into view and the area's edge.
const REVEAL_MARGIN: f32 = 8.0;

#[derive(Default)]
pub(crate) struct ScrollState {
    pub offset: f32,
    content_h: f32,
    drag_anchor: Option<(f32, f32)>,
}

impl Ui {
    /// Starts a vertically scrolling area. `height` defaults to the remaining
    /// height of the parent region (or 240 px if unbounded).
    pub fn begin_scroll_area(&mut self, id_source: &str, height: Option<f32>) {
        let id = self.make_id(id_source);
        self.begin_scroll_area_id(id, height);
    }

    /// [`Ui::begin_scroll_area`] with an explicit id.
    pub fn begin_scroll_area_id(&mut self, id: Id, height: Option<f32>) {
        if !self.frame_guard() {
            return;
        }
        let (origin, w) = self.child_origin();
        let parent_region = self.top().region;
        let h = height.filter(|h| h.is_finite()).map(extent).unwrap_or_else(|| {
            let rem = parent_region.max.y - origin.y;
            if rem.is_finite() {
                rem.max(40.0)
            } else {
                240.0
            }
        });
        let outer = Rect::from_min_size(origin, Vec2::new(w, h));
        let clip = self.out.draw_list.clip();

        // Wheel input is applied before laying out children so content
        // reflects this frame's input without a frame of latency.
        let pointer_inside = self.input.pointer.is_some_and(|p| outer.contains(p) && clip.contains(p));
        let targeted = self.wheel_target.is_none_or(|t| t == id);
        let wheel = if targeted
            && pointer_inside
            && !self.pointer_blocked()
            && !self.input.scroll_consumed
            && self.input.scroll.y != 0.0
        {
            self.input.scroll_consumed = true;
            self.input.scroll.y
        } else {
            0.0
        };
        let st = self.state_mut::<ScrollState>(id);
        let max = (st.content_h - h).max(0.0);
        st.offset = (st.offset - wheel).clamp(0.0, max);
        let offset = st.offset;

        let gutter = self.theme.metrics.scrollbar_width + 6.0;
        self.out.draw_list.push_clip(outer);
        let region = Rect::from_min_size(
            Vec2::new(origin.x, origin.y - offset),
            Vec2::new((w - gutter).max(0.0), f32::INFINITY),
        );
        let spacing = self.theme.metrics.spacing;
        self.layouts.push(Layout::new(Dir::Vertical, region, spacing, Container::Scroll { id, outer, offset }));
    }

    /// The innermost scroll area being built, unless a popup is open above it.
    pub(crate) fn innermost_scroll(&self) -> Option<Id> {
        self.layouts.iter().rev().find_map(|l| match l.container {
            Container::Scroll { id, .. } => Some(Some(id)),
            Container::Popup { .. } => Some(None),
            _ => None,
        })?
    }

    /// Scrolls area `id` so the widget that keyboard navigation just focused
    /// inside it becomes visible, then asks the enclosing area to show this
    /// one. Returns the new offset.
    pub(crate) fn scroll_focus_into_view(&mut self, id: Id, outer: Rect, offset: f32, max: f32) -> f32 {
        let Some((area, r)) = self.focus_reveal else { return offset };
        if area != id {
            return offset;
        }
        self.focus_reveal = self.innermost_scroll().map(|parent| (parent, outer));
        let delta = if r.min.y < outer.min.y {
            r.min.y - outer.min.y - REVEAL_MARGIN
        } else if r.max.y > outer.max.y {
            // Prefer the top edge when the widget is taller than the area.
            (r.max.y - outer.max.y).min(r.min.y - outer.min.y) + REVEAL_MARGIN
        } else {
            return offset;
        };
        let new = (offset + delta).clamp(0.0, max);
        self.state_mut::<ScrollState>(id).offset = new;
        self.request_repaint();
        new
    }

    pub(crate) fn scrollbar(&mut self, id: Id, outer: Rect, offset: f32, content_h: f32, max: f32) {
        let thumb_id = id.with("thumb");
        {
            let st = self.state_mut::<ScrollState>(id);
            st.content_h = content_h;
            st.offset = st.offset.clamp(0.0, max);
        }
        let hovered_area = self.input.pointer.is_some_and(|p| outer.contains(p));
        let vis = self.animate(id, Channel::Hover, if hovered_area { 1.0 } else { 0.0 }, 0.12);
        if max <= 0.0 {
            return;
        }
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let track_h = (outer.height() - 8.0).max(0.0);
        let thumb_h = (outer.height() / content_h * track_h).clamp(MIN_SCROLL_THUMB.min(track_h), track_h);
        let t = offset / max;
        let x = outer.max.x - m.scrollbar_width - 2.0;
        let y = outer.min.y + 4.0 + (track_h - thumb_h) * t;
        let thumb = Rect::from_xywh(x, y, m.scrollbar_width, thumb_h);
        let resp: Response = self.interact(thumb_id, thumb.expand(3.0), Sense { focusable: false, ..Sense::DRAG });
        let pointer_y = self.input.pointer.map(|p| p.y).unwrap_or(0.0);
        let st = self.state_mut::<ScrollState>(id);
        if resp.pressed {
            let (anchor_y, anchor_off) = *st.drag_anchor.get_or_insert((pointer_y, offset));
            let per_px = max / (track_h - thumb_h).max(1.0);
            st.offset = (anchor_off + (pointer_y - anchor_y) * per_px).clamp(0.0, max);
        } else {
            st.drag_anchor = None;
        }
        let h = self.animate(thumb_id, Channel::Hover, if resp.hovered || resp.pressed { 1.0 } else { 0.0 }, 0.08);
        let alpha = anim::ease_out_cubic(0.35 + 0.65 * vis.max(h));
        let base = if th.dark { p.text_faint } else { p.border_strong };
        let color = base.lerp(p.text_muted, h).fade(alpha);
        self.out.draw_list.rect(thumb, m.scrollbar_width * 0.5, color, None);
        if resp.pressed {
            self.request_repaint();
        }
    }

    /// Scroll area.
    pub fn scroll_area<R>(&mut self, id_source: &str, height: Option<f32>, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_scroll_area(id_source, height);
        let r = f(self);
        self.end();
        r
    }
}
