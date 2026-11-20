//! Composite widgets: combo box, tabs, radio buttons, collapsible sections,
//! spin box and a virtualized table.

use super::numeric::Numeric;
use super::{ICON_SIZE, LABEL_GAP};
use crate::layout::{extent, Container, Dir, Layout};
use crate::scroll::ScrollState;
use crate::{Channel, Response, Sense, Ui};
use rugi_core::anim::Spring;
use rugi_core::{anim, id::split_label, Color, CursorIcon, Key, Rect, Vec2};
use std::rc::Rc;

/// Space a combo box keeps for its chevron at the right edge.
const COMBO_CHEVRON_SPACE: f32 = 32.0;
/// Width of a spin box.
const SPIN_BOX_WIDTH: f32 = 160.0;

/// Fraction of a step dragged but not yet applied by an integer spin box.
#[derive(Default)]
struct DragCarry(f32);

#[derive(Default)]
struct TabIndicator {
    x: Spring,
    w: Spring,
    init: bool,
}

impl Ui {
    /// Drop-down selector (QComboBox). `selected` indexes `items`; `.changed`
    /// reports a new choice. Up/Down change the value while focused; Enter or
    /// Space opens the list.
    pub fn combo_box(&mut self, label: &str, selected: &mut usize, items: &[&str]) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (_, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        if self.next_offscreen(m.control_height) && self.can_skip(id) && !self.is_popup_open(id) {
            return self.skip_item(id, Vec2::new(0.0, m.control_height), true);
        }
        if !items.is_empty() {
            *selected = (*selected).min(items.len() - 1);
        }
        let r = self.allocate(Vec2::new(0.0, m.control_height), true);
        let mut resp = self.interact(id, r, Sense::BUTTON);
        let open = self.is_popup_open(id);
        if resp.clicked {
            if open {
                self.close_popup();
            } else {
                self.open_popup(id, r);
            }
        }
        if resp.focused && !open && !items.is_empty() {
            for &(key, _) in &self.input.keys {
                match key {
                    Key::ArrowDown if *selected + 1 < items.len() => {
                        *selected += 1;
                        resp.changed = true;
                    }
                    Key::ArrowUp if *selected > 0 => {
                        *selected -= 1;
                        resp.changed = true;
                    }
                    _ => {}
                }
            }
        }
        let open = self.is_popup_open(id);
        let v = self.visual(&resp);
        let border = p.border.lerp(p.border_strong, v.hover).lerp(p.accent, open as u8 as f32);
        self.out.draw_list.rect(
            r,
            m.radius_md,
            p.surface_hover.lerp(p.surface_active, v.hover * 0.5),
            Some((1.0, border)),
        );
        let style = th.typography.body;
        let text = items.get(*selected).copied().unwrap_or("");
        let text_x = r.min.x + m.control_padding_x - 2.0;
        let layout = self.layout_text_fit(text, &style, (r.max.x - COMBO_CHEVRON_SPACE - text_x).max(0.0));
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(text_x, ty), p.text);
        let chev = if open { crate::lucide::CHEVRON_UP } else { crate::lucide::CHEVRON_DOWN };
        self.paint_icon(
            chev.into(),
            Rect::from_xywh(r.max.x - 28.0, r.center().y - ICON_SIZE * 0.5, ICON_SIZE, ICON_SIZE),
            p.text_muted.lerp(p.text, v.hover),
        );
        self.focus_ring(r, m.radius_md, v.focus);
        self.record("combo_box", text, &resp);

        if self.begin_popup_below(id, r, 0.0) {
            let many = items.len() > 8;
            if many {
                self.begin_scroll_area("##items", Some(8.0 * 34.0));
            }
            for (i, item) in items.iter().enumerate() {
                self.push_id_u64(i as u64);
                if self.selectable(item, i == *selected).clicked {
                    resp.changed |= *selected != i;
                    *selected = i;
                    self.close_popup();
                    // Return keyboard focus to the combo, like Qt.
                    self.focused = Some(id);
                }
                self.pop_id();
            }
            if many {
                self.end();
            }
            self.end();
        }
        resp
    }

    /// Tab bar (QTabBar) with an indicator that animates to the selected tab.
    /// Left/Right switch tabs while focused.
    pub fn tabs(&mut self, id_source: &str, selected: &mut usize, labels: &[&str]) -> Response {
        if !self.frame_guard() || labels.is_empty() {
            return Response::default();
        }
        let id = self.make_id(id_source);
        *selected = (*selected).min(labels.len() - 1);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let style = th.typography.label;
        let h = th.metrics.control_height + 4.0;
        let bar = self.allocate(Vec2::new(0.0, h), true);
        let mut out = Response { id, rect: bar, ..Default::default() };
        let mut x = bar.min.x;
        let mut sel_x = (0.0, 0.0);
        for (i, label) in labels.iter().enumerate() {
            let layout = self.layout_text(label, &style, None);
            let w = layout.size().x + 28.0;
            let r = Rect::from_xywh(x, bar.min.y, w, h);
            let tid = id.with_u64(i as u64);
            let resp = self.interact(tid, r, Sense::BUTTON);
            if resp.clicked && *selected != i {
                *selected = i;
                out.changed = true;
            }
            if resp.focused {
                out.focused = true;
                for &(key, _) in &self.input.keys {
                    match key {
                        Key::ArrowRight if *selected + 1 < labels.len() => {
                            *selected += 1;
                            out.changed = true;
                        }
                        Key::ArrowLeft if *selected > 0 => {
                            *selected -= 1;
                            out.changed = true;
                        }
                        _ => {}
                    }
                }
            }
            let v = self.visual(&resp);
            let is_sel = i == *selected;
            let fg = if is_sel { p.text } else { p.text_muted.lerp(p.text, v.hover) };
            if v.hover > 0.0 && !is_sel {
                self.out.draw_list.rect(r.shrink2(2.0, 5.0), th.metrics.radius_sm, p.surface_hover.fade(v.hover), None);
            }
            let ty = r.min.y + ((h - 3.0 - layout.size().y) * 0.5).round();
            self.paint_text(&layout, Vec2::new(r.min.x + 14.0, ty), fg);
            self.focus_ring(r.shrink2(2.0, 4.0), th.metrics.radius_sm, v.focus);
            self.record("tab", label, &resp);
            if is_sel {
                sel_x = (r.min.x + 10.0, w - 20.0);
            }
            x += w;
        }
        // Baseline and the indicator, on a critically damped spring (no overshoot).
        self.out.draw_list.rect(Rect::from_xywh(bar.min.x, bar.max.y - 1.0, bar.width(), 1.0), 0.0, p.border, None);
        let dt = self.dt;
        let ind = self.state_mut::<TabIndicator>(id);
        if !ind.init {
            *ind = TabIndicator { x: Spring::at(sel_x.0), w: Spring::at(sel_x.1), init: true };
        }
        let moving = ind.x.step(sel_x.0, dt, 24.0) | ind.w.step(sel_x.1, dt, 24.0);
        let (ix, iw) = (ind.x.pos, ind.w.pos);
        if moving {
            self.request_repaint();
        }
        self.out.draw_list.rect(Rect::from_xywh(ix, bar.max.y - 2.5, iw, 2.5), 1.25, p.accent, None);
        self.last_item = bar;
        out
    }

    /// Radio button (QRadioButton) for one `option` of a shared `value`.
    pub fn radio<T: PartialEq + Copy>(&mut self, label: &str, value: &mut T, option: T) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let style = th.typography.body;
        let s = th.metrics.checkbox_size;
        let h = s.max(style.line_px());
        if self.next_offscreen(h) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(0.0, h), false);
        }
        let layout = self.layout_text(text, &style, None);
        let gap = if text.is_empty() { 0.0 } else { LABEL_GAP };
        let r = self.allocate(Vec2::new(s + gap + layout.size().x, h), false);
        let mut resp = self.interact(id, r, Sense::BUTTON);
        if resp.clicked && *value != option {
            *value = option;
            resp.changed = true;
        }
        let on = *value == option;
        let v = self.visual(&resp);
        let t = self.animate(id, Channel::Value, on as u8 as f32, th.motion.value);
        let c = Vec2::new(r.min.x + s * 0.5, r.min.y + h * 0.5);
        let border = p.control_border.lerp(p.text_muted, v.hover).lerp(p.accent, t);
        self.out.draw_list.circle(c, s * 0.5, p.sunken.lerp(p.accent, t), Some((1.5, border)));
        if t > 0.01 {
            self.out.draw_list.circle(c, s * 0.2 * anim::ease_out_cubic(t), p.on_accent, None);
        }
        let ty = r.min.y + ((h - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(r.min.x + s + gap, ty), p.text);
        self.focus_ring(Rect::from_xywh(c.x - s * 0.5, c.y - s * 0.5, s, s), s * 0.5, v.focus);
        self.record("radio", text, &resp);
        resp
    }

    /// Collapsible section header (QGroupBox / QToolBox page). Returns `true`
    /// when the body should be built; then add content and call [`Ui::end`].
    /// The body height animates open and closed.
    pub fn begin_collapsing(&mut self, label: &str, default_open: bool) -> bool {
        self.collapsing_impl(label, default_open, false)
    }

    /// Tree node (QTreeWidget item / ImGui TreeNode): a compact collapsible
    /// row whose children are indented. Returns `true` when the children
    /// should be built; then add them and call [`Ui::end`].
    pub fn begin_tree_node(&mut self, label: &str, default_open: bool) -> bool {
        self.collapsing_impl(label, default_open, true)
    }

    /// Closure form of [`Ui::begin_collapsing`].
    pub fn collapsing<R>(&mut self, label: &str, default_open: bool, f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        if self.begin_collapsing(label, default_open) {
            let r = f(self);
            self.end();
            Some(r)
        } else {
            None
        }
    }

    /// Closure form of [`Ui::begin_tree_node`].
    pub fn tree_node<R>(&mut self, label: &str, default_open: bool, f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        if self.begin_tree_node(label, default_open) {
            let r = f(self);
            self.end();
            Some(r)
        } else {
            None
        }
    }

    fn collapsing_impl(&mut self, label: &str, default_open: bool, tree: bool) -> bool {
        if !self.frame_guard() {
            return false;
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let style = if tree { th.typography.body } else { th.typography.heading };
        let open_state = self.state_mut::<Option<bool>>(id);
        let open = *open_state.get_or_insert(default_open);
        let layout = self.layout_text(text, &style, None);
        let h = if tree { th.metrics.control_height - 6.0 } else { th.metrics.control_height + 2.0 };
        let r = self.allocate(Vec2::new(0.0, h), true);
        let resp = self.interact(id, r, Sense::BUTTON);
        let open = if resp.clicked { !open } else { open };
        *self.state_mut::<Option<bool>>(id) = Some(open);
        let v = self.visual(&resp);
        let t = self.animate(id, Channel::Value, open as u8 as f32, th.motion.value * 1.4);
        let radius = if tree { th.metrics.radius_sm } else { th.metrics.radius_md };
        self.out.draw_list.rect(r, radius, p.surface_hover.fade(v.hover * 0.8), None);
        // Chevron rotates from right (closed) to down (open).
        let (chev_x, text_x, indent) = if tree { (10.0, 24.0, 20.0) } else { (16.0, 32.0, 8.0) };
        let c = Vec2::new(r.min.x + chev_x, r.center().y);
        let ang = t * std::f32::consts::FRAC_PI_2;
        let rot = |x: f32, y: f32| Vec2::new(c.x + x * ang.cos() - y * ang.sin(), c.y + x * ang.sin() + y * ang.cos());
        let col = p.text_muted.lerp(p.text, v.hover);
        self.out.draw_list.line(rot(-2.0, -4.5), rot(2.5, 0.0), 1.8, col);
        self.out.draw_list.line(rot(2.5, 0.0), rot(-2.0, 4.5), 1.8, col);
        let ty = r.min.y + ((h - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(r.min.x + text_x, ty), p.text);
        self.focus_ring(r, radius, v.focus);
        self.record(if tree { "tree_node" } else { "collapsing" }, text, &resp);
        if t <= 0.0 {
            return false;
        }
        // Body: laid out at full size, clipped to the animated height.
        let full = self.state_mut::<crate::layout::CollapseHeight>(id).0;
        let (origin, w) = self.top().peek();
        let visible_h = if full > 0.0 {
            (full * t).round()
        } else if open {
            10_000.0
        } else {
            0.0
        };
        self.out.draw_list.push_clip(Rect::from_min_size(
            Vec2::new(origin.x - 4.0, origin.y - 4.0),
            Vec2::new(w + 8.0, visible_h + 4.0),
        ));
        let spacing = if tree { th.metrics.spacing_sm } else { th.metrics.spacing };
        let region =
            Rect::from_min_size(origin + Vec2::new(indent, 0.0), Vec2::new((w - indent).max(0.0), f32::INFINITY));
        let open_t = if full > 0.0 { t } else { 1.0 };
        self.layouts.push(Layout::new(Dir::Vertical, region, spacing, Container::Collapse { id, open_t }));
        true
    }

    /// Numeric field with − / + steppers and horizontal drag (QDoubleSpinBox).
    pub fn spin_box(&mut self, label: &str, value: &mut f32, min: f32, max: f32, step: f32) -> Response {
        self.spin_box_impl(label, value, min, max, step)
    }

    /// Integer spin box.
    pub fn spin_box_int(&mut self, label: &str, value: &mut i32, min: i32, max: i32, step: i32) -> Response {
        self.spin_box_impl(label, value, min, max, step)
    }

    fn spin_box_impl<T: Numeric>(&mut self, label: &str, value: &mut T, min: T, max: T, step: T) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (_, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let (lo, hi) = T::range(min, max);
        let step = T::valid_step(step);
        value.sanitize(lo);
        if self.next_offscreen(m.control_height) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(SPIN_BOX_WIDTH, m.control_height), false);
        }
        let r = self.allocate(Vec2::new(SPIN_BOX_WIDTH, m.control_height), false);
        let bw = 30.0;
        let minus = Rect::from_xywh(r.min.x, r.min.y, bw, r.height());
        let plus = Rect::from_xywh(r.max.x - bw, r.min.y, bw, r.height());
        let mid = Rect::from_min_max(Vec2::new(minus.max.x, r.min.y), Vec2::new(plus.min.x, r.max.y));
        let before = *value;
        let rm = self.interact(id.with("-"), minus, Sense { focusable: false, ..Sense::BUTTON });
        let rp = self.interact(id.with("+"), plus, Sense { focusable: false, ..Sense::BUTTON });
        let mut resp = self.interact(id, mid, Sense::DRAG);
        if rm.clicked {
            *value = value.add_steps(step, -1);
        }
        if rp.clicked {
            *value = value.add_steps(step, 1);
        }
        if resp.pressed {
            self.out.cursor = CursorIcon::Grab;
            let dx = self.input.pointer_delta.x;
            if self.input.down[0] && dx != 0.0 {
                let speed = if self.input.modifiers.shift { 0.1 } else { 1.0 };
                let carry = &mut self.state_mut::<DragCarry>(id).0;
                *value = value.drag(step, dx / 4.0 * speed, carry);
            }
        }
        if resp.focused {
            for &(key, _) in &self.input.keys {
                match key {
                    Key::ArrowUp | Key::ArrowRight => *value = value.add_steps(step, 1),
                    Key::ArrowDown | Key::ArrowLeft => *value = value.add_steps(step, -1),
                    Key::Home => *value = lo,
                    Key::End => *value = hi,
                    _ => {}
                }
            }
        }
        *value = value.clamp_to(lo, hi);
        resp.changed = *value != before;
        resp.rect = r;
        let v = self.visual(&resp);
        let border = p.border.lerp(p.border_strong, v.hover).lerp(p.accent, v.focus.max(resp.pressed as u8 as f32));
        self.out.draw_list.rect(r, m.radius_md, p.sunken, Some((1.0, border)));
        for (br, rr, glyph) in [(minus, &rm, crate::lucide::MINUS), (plus, &rp, crate::lucide::PLUS)] {
            let hv = self.animate(rr.id, Channel::Hover, rr.hovered as u8 as f32, th.motion.hover);
            if hv > 0.0 {
                self.out.draw_list.rect(br.shrink2(3.0, 3.0), m.radius_sm, p.surface_hover.fade(hv), None);
            }
            self.paint_icon(
                glyph.into(),
                Rect::from_xywh(br.center().x - 7.0, br.center().y - 7.0, 14.0, 14.0),
                p.text_muted.lerp(p.text, hv),
            );
        }
        let mut buf = std::mem::take(&mut self.scratch);
        buf.clear();
        value.write(&mut buf, T::spin_decimals(step));
        let style = th.typography.body.weight(rugi_core::FontWeight::MEDIUM);
        let layout = self.layout_text(&buf, &style, None);
        self.scratch = buf;
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(mid.center().x - layout.size().x * 0.5, ty), p.text);
        self.focus_ring(r, m.radius_md, v.focus);
        self.record("spin_box", src, &resp);
        resp
    }

    /// Starts a layout region at an explicit rectangle (does not take space in
    /// the parent). Used for table cells and custom compositions.
    pub fn begin_region(&mut self, rect: Rect, horizontal: bool) {
        if !self.frame_guard() {
            return;
        }
        let dir = if horizontal { Dir::Horizontal(crate::Align::Start) } else { Dir::Vertical };
        let spacing = self.theme.metrics.spacing_sm;
        self.out.draw_list.push_clip(rect);
        self.layouts.push(Layout::new(dir, rect, spacing, Container::Region));
    }

    /// Virtualized table (QTableView): only rows inside the viewport call
    /// `cell`, so 100 000 rows cost the same as 20. `widths` are relative
    /// column weights. Clicking a row selects it.
    #[allow(clippy::too_many_arguments)]
    pub fn table(
        &mut self,
        id_source: &str,
        headers: &[&str],
        widths: &[f32],
        rows: usize,
        selected: &mut Option<usize>,
        height: f32,
        mut cell: impl FnMut(&mut Ui, usize, usize),
    ) -> Response {
        if !self.frame_guard() || headers.is_empty() {
            return Response::default();
        }
        let id = self.make_id(id_source);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let row_h = (th.metrics.control_height - 2.0).max(24.0);
        let height = extent(height);
        let weight = |i: usize| extent(widths.get(i).copied().unwrap_or(1.0)).max(0.01);
        let total_w: f32 = (0..headers.len()).map(weight).sum();
        let col_x = |i: usize, w: f32| -> (f32, f32) {
            let mut x = 0.0;
            for j in 0..i {
                x += weight(j) / total_w * w;
            }
            (x, weight(i) / total_w * w)
        };
        // Header.
        let hr = self.allocate(Vec2::new(0.0, row_h), true);
        let cap = th.typography.caption.weight(rugi_core::FontWeight::MEDIUM);
        for (i, h) in headers.iter().enumerate() {
            let (x, w) = col_x(i, hr.width());
            let l = self.layout_text_fit(h, &cap, (w - 16.0).max(0.0));
            let ty = hr.min.y + ((row_h - l.size().y) * 0.5).round();
            self.paint_text(&l, Vec2::new(hr.min.x + x + 12.0, ty), p.text_muted);
        }
        self.out.draw_list.rect(Rect::from_xywh(hr.min.x, hr.max.y - 1.0, hr.width(), 1.0), 0.0, p.border, None);
        // The table is one Tab stop; the arrow keys move the selection.
        let table_rect = Rect::from_min_size(hr.min, Vec2::new(hr.width(), hr.height() + height));
        let focus_id = id.with("focus");
        let focus = self.interact(focus_id, table_rect, Sense { click: false, drag: false, ..Sense::BUTTON });
        self.record("table", id_source, &focus);
        let body_id = id.with("body");
        let mut changed = false;
        if focus.focused && rows > 0 {
            let (last, page) = (rows - 1, ((height / row_h) as usize).max(1));
            let mut sel = *selected;
            for &(key, _) in &self.input.keys {
                sel = Some(match (key, sel) {
                    (Key::ArrowDown, Some(s)) => s.saturating_add(1).min(last),
                    (Key::ArrowUp, Some(s)) => s.min(last).saturating_sub(1),
                    (Key::PageDown, Some(s)) => s.saturating_add(page).min(last),
                    (Key::PageUp, Some(s)) => s.min(last).saturating_sub(page),
                    (Key::ArrowDown | Key::PageDown | Key::Home, _) => 0,
                    (Key::ArrowUp | Key::PageUp | Key::End, _) => last,
                    _ => continue,
                });
            }
            if let (true, Some(s)) = (sel != *selected, sel) {
                *selected = sel;
                changed = true;
                let (top, st) = (s as f32 * row_h, self.state_mut::<ScrollState>(body_id));
                if top < st.offset {
                    st.offset = top;
                } else if top + row_h > st.offset + height {
                    st.offset = top + row_h - height;
                }
                self.request_repaint();
            }
        }
        // Body.
        let offset = self.state_mut::<ScrollState>(body_id).offset;
        self.begin_scroll_area_id(body_id, Some(height));
        self.top().spacing = 0.0;
        let first = ((offset / row_h).floor() as usize).min(rows);
        let last = (((offset + height) / row_h).ceil() as usize + 1).min(rows);
        if first > 0 {
            self.top().space(first as f32 * row_h);
        }
        let body_style_h = th.typography.body.line_px();
        let mut out = Response { id, rect: hr, changed, focused: focus.focused, ..Default::default() };
        for row in first..last {
            let rid = id.with_u64(row as u64);
            let r = self.allocate(Vec2::new(0.0, row_h), true);
            let resp = self.interact(rid, r, Sense { focusable: false, ..Sense::BUTTON });
            if resp.clicked {
                *selected = Some(row);
                out.changed = true;
                self.focused = Some(focus_id);
                self.focus_seen = true;
                self.focus_visible = false;
            }
            let is_sel = *selected == Some(row);
            let hv = self.animate(rid, Channel::Hover, resp.hovered as u8 as f32, th.motion.hover);
            let stripe = if row % 2 == 1 { p.surface_hover.fade(0.35) } else { Color::TRANSPARENT };
            let bg = if is_sel { p.accent_soft } else { stripe.lerp(p.surface_hover, hv) };
            self.out.draw_list.rect(r, th.metrics.radius_sm, bg, None);
            let pad_y = ((row_h - body_style_h) * 0.5).floor();
            self.push_id_u64(row as u64);
            for col in 0..headers.len() {
                let (x, w) = col_x(col, r.width());
                let cr = Rect::from_xywh(r.min.x + x + 12.0, r.min.y + pad_y, (w - 16.0).max(0.0), row_h - pad_y);
                self.begin_region(cr, true);
                cell(self, row, col);
                self.end();
            }
            self.pop_id();
        }
        if last < rows {
            self.top().space((rows - last) as f32 * row_h);
        }
        self.end();
        let v = self.visual(&focus);
        self.focus_ring(table_rect, th.metrics.radius_sm, v.focus);
        // The whole table is the target for tooltips and context menus.
        out.rect = table_rect;
        self.last_item = out.rect;
        self.last_response = Response {
            id,
            rect: out.rect,
            hovered: self.input.pointer.is_some_and(|p| out.rect.contains(p)),
            focused: focus.focused || self.focused == Some(focus_id),
            ..Default::default()
        };
        out
    }
}
