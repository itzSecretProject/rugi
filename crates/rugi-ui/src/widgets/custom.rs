//! Widgets for custom application chrome: key bindings,
//! multi-select, segmented controls, icon rails, carousels and page dots.

use super::{GLIDE_OMEGA, ICON_SIZE};
use crate::{Channel, IconRef, Response, Sense, Ui};
use rugi_core::anim::Spring;
use rugi_core::draw::{DrawList, Fill, Slot};
use rugi_core::{id::split_label, Color, CursorIcon, Key, Modifiers, PointerButton, Rect, Vec2};
use std::rc::Rc;

/// Most page dots drawn at once; longer sequences show a window around the
/// current page.
const MAX_PAGE_DOTS: usize = 64;

/// What a [`KeyBind`] is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Bind {
    /// Unbound.
    #[default]
    None,
    /// A keyboard key.
    Key(Key),
    /// A mouse button (secondary / middle; primary is reserved for clicking).
    Mouse(PointerButton),
}

/// How a binding activates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum BindMode {
    /// Active while held.
    #[default]
    Hold = 0,
    /// Each press flips it on/off.
    Toggle = 1,
    /// Always active (binding ignored).
    Always = 2,
}

impl BindMode {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            BindMode::Hold => "Hold",
            BindMode::Toggle => "Toggle",
            BindMode::Always => "Always",
        }
    }

    /// Converts from the C ABI integer.
    pub fn from_u32(v: u32) -> Option<BindMode> {
        Some(match v {
            0 => BindMode::Hold,
            1 => BindMode::Toggle,
            2 => BindMode::Always,
            _ => return None,
        })
    }
}

/// A user-editable key binding (see [`Ui::keybind`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct KeyBind {
    /// Bound input.
    pub bind: Bind,
    /// Required modifiers.
    pub mods: Modifiers,
    /// Activation mode.
    pub mode: BindMode,
    /// Current toggle state (for [`BindMode::Toggle`]).
    pub toggled: bool,
}

impl KeyBind {
    /// A binding to `key`.
    pub const fn key(key: Key) -> Self {
        Self {
            bind: Bind::Key(key),
            mods: Modifiers { shift: false, ctrl: false, alt: false, logo: false },
            mode: BindMode::Hold,
            toggled: false,
        }
    }

    /// Returns a copy with another mode.
    pub const fn with_mode(mut self, mode: BindMode) -> Self {
        self.mode = mode;
        self
    }

    /// Writes a display label ("Ctrl+K", "Mouse 2", "None") into `out`.
    pub fn write_label(&self, out: &mut String) {
        out.clear();
        if self.bind == Bind::None {
            out.push_str("None");
            return;
        }
        let m = self.mods;
        if m.ctrl {
            out.push_str("Ctrl+");
        }
        if m.alt {
            out.push_str(if cfg!(target_os = "macos") { "Opt+" } else { "Alt+" });
        }
        if m.shift {
            out.push_str("Shift+");
        }
        if m.logo {
            out.push_str(if cfg!(target_os = "macos") { "Cmd+" } else { "Win+" });
        }
        match self.bind {
            Bind::Key(k) => out.push_str(k.name()),
            Bind::Mouse(PointerButton::Secondary) => out.push_str("Mouse 2"),
            Bind::Mouse(PointerButton::Middle) => out.push_str("Mouse 3"),
            Bind::Mouse(PointerButton::Primary) => out.push_str("Mouse 1"),
            Bind::None => {}
        }
    }
}

#[derive(Default)]
struct KeyBindState {
    listening: bool,
    /// A modifier pressed while listening: bound alone if released without another key.
    pending_mod: Option<Key>,
    opened_frame: u64,
}

#[derive(Default)]
struct GlideHighlight {
    x: Spring,
    y: Spring,
    w: Spring,
    h: Spring,
    init: bool,
}

impl GlideHighlight {
    fn step(&mut self, r: Rect, dt: f32) -> (Rect, bool) {
        if !self.init {
            *self = GlideHighlight {
                x: Spring::at(r.min.x),
                y: Spring::at(r.min.y),
                w: Spring::at(r.width()),
                h: Spring::at(r.height()),
                init: true,
            };
        }
        let moving = self.x.step(r.min.x, dt, GLIDE_OMEGA)
            | self.y.step(r.min.y, dt, GLIDE_OMEGA)
            | self.w.step(r.width(), dt, GLIDE_OMEGA)
            | self.h.step(r.height(), dt, GLIDE_OMEGA);
        (Rect::from_xywh(self.x.pos, self.y.pos, self.w.pos, self.h.pos), moving)
    }
}

#[derive(Default)]
struct CarouselState {
    pos: Option<Spring>,
    drag: Option<(f32, f32)>,
    dragged: bool,
    wheel: f32,
}

fn is_modifier(k: Key) -> bool {
    matches!(k, Key::Shift | Key::Control | Key::Alt)
}

impl Ui {
    /// True while `key` is held down.
    pub fn key_down(&self, key: Key) -> bool {
        self.input.keys_down.contains(&key)
    }

    /// True if `key` was pressed this frame (including auto-repeat).
    pub fn key_pressed(&self, key: Key) -> bool {
        self.input.key_pressed(key)
    }

    /// True if `key` was pressed this frame with the platform's command
    /// modifier (⌘ on macOS, Ctrl elsewhere) and without Shift or Alt.
    pub fn command_pressed(&self, key: Key) -> bool {
        self.input.keys.iter().any(|&(k, m)| k == key && m.command() && !m.shift && !m.alt)
    }

    /// Evaluates a binding for this frame: held for [`BindMode::Hold`],
    /// flips on press for [`BindMode::Toggle`] (state kept in `bind.toggled`),
    /// always true for [`BindMode::Always`]. Returns false while the binding
    /// is being edited.
    pub fn bind_active(&mut self, bind: &mut KeyBind) -> bool {
        if bind.mode == BindMode::Always {
            return true;
        }
        let mods_ok = |m: Modifiers| {
            m.ctrl == bind.mods.ctrl && m.alt == bind.mods.alt && m.shift == bind.mods.shift && m.logo == bind.mods.logo
        };
        let (down, pressed) = match bind.bind {
            Bind::None => (false, false),
            Bind::Key(k) => {
                let pressed = self.input.keys.iter().any(|&(key, m)| key == k && (is_modifier(k) || mods_ok(m)));
                (self.key_down(k) && (is_modifier(k) || mods_ok(self.input.modifiers)), pressed)
            }
            Bind::Mouse(b) => (self.input.down[b as usize], self.input.pressed[b as usize]),
        };
        match bind.mode {
            BindMode::Hold => down,
            BindMode::Toggle => {
                if pressed {
                    bind.toggled = !bind.toggled;
                }
                bind.toggled
            }
            BindMode::Always => true,
        }
    }

    /// A key-binding button. Click it, then press a key (with modifiers),
    /// or the right / middle mouse button. Escape cancels, Backspace/Delete
    /// clears. Right-click the button to choose Hold / Toggle / Always.
    /// With a non-empty label the label sits on the left and the button on
    /// the right of a full-width row. `.changed` reports a new binding/mode.
    pub fn keybind(&mut self, label: &str, bind: &mut KeyBind) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let h = m.control_height;
        let row = self.allocate(Vec2::new(0.0, h), !text.is_empty());
        let bw = 96.0f32.min(row.width());
        let r = if text.is_empty() {
            Rect::from_xywh(row.min.x, row.min.y, bw, h)
        } else {
            Rect::from_xywh(row.max.x - bw, row.min.y, bw, h)
        };
        let mut resp = self.interact(id, r, Sense::BUTTON);
        let frame = self.frame;
        let listening = self.state_mut::<KeyBindState>(id).listening;
        if resp.clicked && !listening {
            let st = self.state_mut::<KeyBindState>(id);
            st.listening = true;
            st.pending_mod = None;
            st.opened_frame = frame;
        }
        let st_frame = self.state_mut::<KeyBindState>(id).opened_frame;
        if listening && st_frame != frame {
            let mut result: Option<(Bind, Modifiers)> = None;
            let mut clear = false;
            let mut cancel = false;
            for i in 0..self.input.keys.len() {
                let (k, mods) = self.input.keys[i];
                match k {
                    Key::Escape => cancel = true,
                    Key::Backspace | Key::Delete => clear = true,
                    k if is_modifier(k) => {
                        self.state_mut::<KeyBindState>(id).pending_mod = Some(k);
                    }
                    k => result = Some((Bind::Key(k), mods)),
                }
            }
            if result.is_none() && !cancel && !clear {
                if self.input.pressed[1] {
                    result = Some((Bind::Mouse(PointerButton::Secondary), Modifiers::default()));
                } else if self.input.pressed[2] {
                    result = Some((Bind::Mouse(PointerButton::Middle), Modifiers::default()));
                } else if let Some(pm) = self.state_mut::<KeyBindState>(id).pending_mod {
                    let held = match pm {
                        Key::Shift => self.input.modifiers.shift,
                        Key::Control => self.input.modifiers.ctrl,
                        _ => self.input.modifiers.alt,
                    };
                    if !held {
                        result = Some((Bind::Key(pm), Modifiers::default()));
                    }
                } else if self.input.pressed[0] && !r.contains(self.input.pointer.unwrap_or(Vec2::ZERO)) {
                    cancel = true;
                }
            }
            if clear {
                if bind.bind != Bind::None {
                    resp.changed = true;
                }
                bind.bind = Bind::None;
                bind.mods = Modifiers::default();
                cancel = true;
            }
            if let Some((b, mods)) = result {
                resp.changed |= bind.bind != b || bind.mods != mods;
                bind.bind = b;
                bind.mods = mods;
                cancel = true;
            }
            if cancel {
                self.state_mut::<KeyBindState>(id).listening = false;
                // Keep focus but don't show a ring for the key that ended editing.
                self.focus_visible = false;
            }
        }
        let listening = self.state_mut::<KeyBindState>(id).listening;
        if listening {
            self.request_repaint();
        }
        let v = self.visual(&resp);
        let pulse = if listening { 0.55 + 0.45 * (self.time as f32 * 5.0).sin() } else { 0.0 };
        let border = p.border.lerp(p.border_strong, v.hover).lerp(p.accent, pulse);
        let fill = p
            .surface_hover
            .lerp(p.surface_active, v.hover * 0.6)
            .lerp(p.accent_soft, if listening { 1.0 } else { 0.0 });
        self.out.draw_list.rect(r, m.radius_md, fill, Some((1.0, border)));
        let mut buf = std::mem::take(&mut self.scratch);
        if listening {
            buf.clear();
            buf.push_str("Press a key…");
        } else {
            bind.write_label(&mut buf);
        }
        let style = th.typography.label;
        let layout = self.layout_text(&buf, &style, None);
        self.scratch = buf;
        let fg = if listening {
            p.accent_text
        } else if bind.bind == Bind::None {
            p.text_faint
        } else {
            p.text
        };
        self.out.draw_list.push_clip(r.shrink2(6.0, 0.0));
        let tx = r.min.x + ((r.width() - layout.size().x) * 0.5).max(6.0).round();
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(tx, ty), fg);
        self.out.draw_list.pop_clip();
        if !text.is_empty() {
            let ls = th.typography.body;
            let ll = self.layout_text(text, &ls, None);
            let ty = row.min.y + ((h - ll.size().y) * 0.5).round();
            self.paint_text(&ll, Vec2::new(row.min.x, ty), p.text);
        }
        self.focus_ring(r, m.radius_md, v.focus);
        self.record("keybind", text, &resp);
        if !listening && self.begin_context_menu("##bind-mode") {
            for mode in [BindMode::Hold, BindMode::Toggle, BindMode::Always] {
                if self.selectable(mode.name(), bind.mode == mode).clicked {
                    resp.changed |= bind.mode != mode;
                    bind.mode = mode;
                    self.close_popup();
                }
            }
            self.menu_separator();
            if self.menu_item("Clear", "Backspace").clicked {
                resp.changed |= bind.bind != Bind::None;
                bind.bind = Bind::None;
                bind.mods = Modifiers::default();
            }
            self.end();
        }
        resp
    }

    /// Drop-down that selects any number of `items` (`flags[i]` = selected).
    /// The closed box summarises the selection ("Head +2"). `.changed`
    /// reports toggles.
    pub fn multi_select(&mut self, label: &str, flags: &mut [bool], items: &[&str]) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (_, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let n = flags.len().min(items.len());
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
        let open = self.is_popup_open(id);
        let v = self.visual(&resp);
        let border = p.border.lerp(p.border_strong, v.hover).lerp(p.accent, open as u8 as f32);
        self.out.draw_list.rect(
            r,
            m.radius_md,
            p.surface_hover.lerp(p.surface_active, v.hover * 0.5),
            Some((1.0, border)),
        );
        let mut buf = std::mem::take(&mut self.scratch);
        buf.clear();
        let count = flags[..n].iter().filter(|f| **f).count();
        match flags[..n].iter().position(|f| *f) {
            None => buf.push_str("None"),
            Some(first) => {
                buf.push_str(items[first]);
                if count > 1 {
                    buf.push_str(" +");
                    rugi_core::fmt::push_fixed(&mut buf, (count - 1) as f32, 0);
                }
            }
        }
        let style = th.typography.body;
        let text_x = r.min.x + m.control_padding_x - 2.0;
        let layout = self.layout_text_fit(&buf, &style, (r.max.x - 32.0 - text_x).max(0.0));
        self.scratch = buf;
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        let fg = if count == 0 { p.text_faint } else { p.text };
        self.paint_text(&layout, Vec2::new(text_x, ty), fg);
        let chev = if open { crate::lucide::CHEVRON_UP } else { crate::lucide::CHEVRON_DOWN };
        self.paint_icon(
            chev.into(),
            Rect::from_xywh(r.max.x - 28.0, r.center().y - ICON_SIZE * 0.5, ICON_SIZE, ICON_SIZE),
            p.text_muted.lerp(p.text, v.hover),
        );
        self.focus_ring(r, m.radius_md, v.focus);
        self.record("multi_select", label, &resp);
        if self.begin_popup_below(id, r, 0.0) {
            for (i, flag) in flags.iter_mut().enumerate().take(n) {
                self.push_id_u64(i as u64);
                let row = self.selectable(items[i], false);
                if row.clicked {
                    *flag = !*flag;
                    resp.changed = true;
                }
                let on = self.animate(row.id.with("check"), Channel::Value, if *flag { 1.0 } else { 0.0 }, 0.06);
                if on > 0.01 {
                    let ir = Rect::from_xywh(
                        row.rect.max.x - 26.0,
                        row.rect.center().y - ICON_SIZE * 0.5,
                        ICON_SIZE,
                        ICON_SIZE,
                    );
                    self.paint_icon(crate::lucide::CHECK.into(), ir, p.accent_text.fade(on));
                }
                self.pop_id();
            }
            self.end();
        }
        resp
    }

    /// Segmented control (single choice among a few options) with a pill
    /// that animates to the selection. Fills the available width.
    pub fn segmented(&mut self, id_source: &str, selected: &mut usize, labels: &[&str]) -> Response {
        if !self.frame_guard() || labels.is_empty() {
            return Response::default();
        }
        let id = self.make_id(id_source);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        *selected = (*selected).min(labels.len() - 1);
        let r = self.allocate(Vec2::new(0.0, m.control_height), true);
        self.out.draw_list.rect(r, m.radius_md + 2.0, p.sunken, Some((1.0, p.border)));
        let pill_slot = self.out.draw_list.reserve();
        let inner = r.shrink2(3.0, 3.0);
        let w = inner.width() / labels.len() as f32;
        let mut group = Response { id, rect: r, ..Default::default() };
        let style = th.typography.label;
        for (i, label) in labels.iter().enumerate() {
            let cell = Rect::from_xywh(inner.min.x + w * i as f32, inner.min.y, w, inner.height());
            let cid = id.with_u64(i as u64);
            let resp = self.interact(cid, cell, Sense::BUTTON);
            if resp.clicked && *selected != i {
                *selected = i;
                group.changed = true;
            }
            group.hovered |= resp.hovered;
            group.focused |= resp.focused;
            if resp.focused {
                for k in 0..self.input.keys.len() {
                    match self.input.keys[k].0 {
                        Key::ArrowLeft if *selected > 0 => {
                            *selected -= 1;
                            group.changed = true;
                        }
                        Key::ArrowRight if *selected + 1 < labels.len() => {
                            *selected += 1;
                            group.changed = true;
                        }
                        _ => {}
                    }
                }
            }
            let v = self.visual(&resp);
            let sel = self.animate(cid.with("sel"), Channel::Value, if i == *selected { 1.0 } else { 0.0 }, 0.08);
            let (text, _) = split_label(label);
            let layout = self.layout_text(text, &style, None);
            let tx = cell.min.x + ((cell.width() - layout.size().x) * 0.5).round();
            let ty = cell.min.y + ((cell.height() - layout.size().y) * 0.5).round();
            let fg = p.text_muted.lerp(p.text, v.hover.max(sel));
            self.paint_text(&layout, Vec2::new(tx, ty), fg);
            self.focus_ring(cell, m.radius_md, v.focus);
        }
        let target = Rect::from_xywh(inner.min.x + w * *selected as f32, inner.min.y, w, inner.height());
        let dt = self.dt;
        let (hr, moving) = self.state_mut::<GlideHighlight>(id.with("pill")).step(target, dt);
        if moving {
            self.request_repaint();
        }
        let pill =
            DrawList::rect_instance(hr, [m.radius_md; 4], Fill::Solid(p.surface_active), Some((1.0, p.border_strong)));
        self.out.draw_list.fill_slot(pill_slot, pill);
        self.record("segmented", id_source, &group);
        group
    }

    /// Square icon-only navigation button for rails (vertical or horizontal
    /// strips of page icons). Rail items in the same container share one
    /// highlight that animates to the selected item. `tooltip` shows on hover.
    pub fn rail_item(&mut self, id_source: &str, icon: IconRef<'_>, selected: bool, tooltip: &str) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let id = self.make_id(id_source);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let group = self.current_id().with("rail-group").with_u64(self.layouts.len() as u64);
        let frame = self.frame;
        let slot: Slot = match self.rail_slots {
            Some((g, f, s)) if g == group && f == frame => s,
            _ => {
                let s = self.out.draw_list.reserve();
                self.rail_slots = Some((group, frame, s));
                s
            }
        };
        let size = 40.0;
        let r = self.allocate(Vec2::new(size, size), false);
        let resp = self.interact(id, r, Sense::BUTTON);
        let v = self.visual(&resp);
        let sel = self.animate(id, Channel::Value, if selected { 1.0 } else { 0.0 }, th.motion.value);
        if selected {
            let dt = self.dt;
            let (hr, moving) = self.state_mut::<GlideHighlight>(group).step(r, dt);
            if moving {
                self.request_repaint();
            }
            let bg = DrawList::rect_instance(hr, [m.radius_md + 2.0; 4], Fill::Solid(p.accent_soft), None);
            self.out.draw_list.fill_slot(slot, bg);
        } else if v.hover > 0.0 {
            self.out.draw_list.rect(r, m.radius_md + 2.0, p.surface_hover.fade(v.hover), None);
        }
        let fg = p.text_faint.lerp(p.text, v.hover).lerp(p.accent_text, sel);
        self.paint_icon(icon, r.shrink2(11.0, 11.0), fg);
        self.focus_ring(r, m.radius_md + 2.0, v.focus);
        self.record("rail_item", tooltip, &resp);
        if !tooltip.is_empty() {
            self.tooltip(tooltip);
        }
        resp
    }

    /// Horizontal carousel of `count` cards of `card` size. The selected card
    /// is centered at full size; neighbours shrink and overlap behind it.
    /// `paint(ui, index, rect, emphasis)` draws card `index` into `rect`;
    /// `emphasis` is 1 for the centered card and falls to 0 one step away.
    /// Click a side card, drag, scroll, or use ←/→ (when focused) to move.
    /// `.changed` reports a new `index`; `.clicked` a click on the centered card.
    pub fn carousel(
        &mut self,
        id_source: &str,
        count: usize,
        index: &mut usize,
        card: Vec2,
        mut paint: impl FnMut(&mut Ui, usize, Rect, f32),
    ) -> Response {
        if !self.frame_guard() || count == 0 {
            return Response::default();
        }
        *index = (*index).min(count - 1);
        let id = self.make_id(id_source);
        let area = self.allocate(Vec2::new(0.0, card.y + 8.0), true);
        let mut resp = self.interact(id, area, Sense { keyboard_activate: false, ..Sense::DRAG });
        let ptr = self.input.pointer;
        let dt = self.dt;
        let step_w = card.x * 0.78;
        let before = *index;
        {
            let pressed_now = self.input.pressed[0];
            let st = self.state_mut::<CarouselState>(id);
            let pos = st.pos.get_or_insert(Spring::at(*index as f32));
            if resp.pressed {
                if let Some(p) = ptr {
                    let (ax, apos) = *st.drag.get_or_insert((p.x, pos.pos));
                    if pressed_now {
                        st.dragged = false;
                    }
                    let d = (p.x - ax) / step_w;
                    if d.abs() > 0.04 {
                        st.dragged = true;
                    }
                    if st.dragged {
                        pos.pos = (apos - d).clamp(-0.4, count as f32 - 0.6);
                        pos.vel = 0.0;
                    }
                }
            } else if st.drag.take().is_some() && st.dragged {
                *index = pos.pos.round().clamp(0.0, (count - 1) as f32) as usize;
            }
        }
        if resp.hovered && !self.input.scroll_consumed {
            let s = self.input.scroll;
            let delta = if s.x.abs() > s.y.abs() { -s.x } else { -s.y };
            if delta != 0.0 {
                self.input.scroll_consumed = true;
                let st = self.state_mut::<CarouselState>(id);
                st.wheel += delta;
                if st.wheel.abs() >= 60.0 {
                    let dir = st.wheel.signum();
                    st.wheel = 0.0;
                    let next = *index as i64 + dir as i64;
                    *index = next.clamp(0, count as i64 - 1) as usize;
                }
            }
        }
        if resp.focused {
            for k in 0..self.input.keys.len() {
                match self.input.keys[k].0 {
                    Key::ArrowLeft if *index > 0 => *index -= 1,
                    Key::ArrowRight if *index + 1 < count => *index += 1,
                    Key::Enter | Key::Space => resp.clicked = true,
                    _ => {}
                }
            }
        }
        let st = self.state_mut::<CarouselState>(id);
        let dragging = resp.pressed && st.dragged;
        let pos = st.pos.get_or_insert(Spring::at(*index as f32));
        let moving = if dragging { true } else { pos.step(*index as f32, dt, 16.0) };
        let p = pos.pos;
        let was_dragged = st.dragged;
        if moving {
            self.request_repaint();
        }
        // Visible cards, far ones first so the centered card is drawn last.
        let center = area.center();
        let lo = (p - 2.6).floor().max(0.0) as usize;
        let hi = ((p + 2.6).ceil() as usize).min(count - 1);
        let mut order: [(usize, f32); 8] = [(0, 0.0); 8];
        let mut n = 0;
        for i in lo..=hi {
            if n < order.len() {
                order[n] = (i, i as f32 - p);
                n += 1;
            }
        }
        order[..n].sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
        let mut rects: [(usize, Rect); 8] = [(0, Rect::ZERO); 8];
        self.out.draw_list.push_clip(area.expand(2.0));
        for (k, &(i, off)) in order[..n].iter().enumerate() {
            let a = off.abs();
            let scale = 1.0 - 0.16 * a.min(1.6);
            let x = center.x + off.signum() * step_w * (a.min(1.0) + (a - 1.0).max(0.0) * 0.55);
            let size = card * scale;
            let r = Rect::from_min_size(Vec2::new(x - size.x * 0.5, center.y - size.y * 0.5), size);
            let emphasis = (1.0 - a).clamp(0.0, 1.0);
            self.push_id_u64(i as u64);
            paint(self, i, r, emphasis);
            self.pop_id();
            rects[k] = (i, r);
        }
        self.out.draw_list.pop_clip();
        // Click (without drag) selects the top-most card under the pointer.
        if self.input.released[0] && resp.hovered && !was_dragged {
            if let Some(pt) = ptr {
                if let Some(&(i, _)) = rects[..n].iter().rev().find(|(_, r)| r.contains(pt)) {
                    if i == *index {
                        resp.clicked = true;
                    } else {
                        *index = i;
                    }
                }
            }
        } else if !resp.focused {
            resp.clicked = false;
        }
        if resp.hovered {
            self.out.cursor = CursorIcon::Pointer;
        }
        resp.changed = *index != before;
        self.record("carousel", id_source, &resp);
        resp
    }

    /// Page indicator dots; the current page is a wider pill. Clicking a dot
    /// selects that page. Centered in a full-width row.
    pub fn page_dots(&mut self, id_source: &str, count: usize, index: &mut usize) -> Response {
        if !self.frame_guard() || count == 0 {
            return Response::default();
        }
        let id = self.make_id(id_source);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let row = self.allocate(Vec2::new(0.0, 14.0), true);
        let (dot, gap, wide) = (6.0, 8.0, 22.0);
        // Only the dots that fit are laid out: a window around the current page.
        let fit = (((row.width() - wide) / (dot + gap)).max(0.0) as usize + 1).min(MAX_PAGE_DOTS);
        let shown = count.min(fit);
        let first = index.saturating_sub(shown / 2).min(count - shown);
        let total = (shown - 1) as f32 * (dot + gap) + wide;
        let mut x = row.center().x - total * 0.5;
        let mut group = Response { id, rect: row, ..Default::default() };
        for i in first..first + shown {
            let sel = self.animate(id.with_u64(i as u64), Channel::Value, if i == *index { 1.0 } else { 0.0 }, 0.08);
            let w = dot + (wide - dot) * sel;
            let r = Rect::from_xywh(x, row.center().y - dot * 0.5, w, dot);
            let hit = self.interact(
                id.with_u64(i as u64).with("hit"),
                r.expand(4.0),
                Sense { focusable: false, ..Sense::BUTTON },
            );
            if hit.clicked && *index != i {
                *index = i;
                group.changed = true;
            }
            let c = p.text_faint.lerp(p.text, sel.max(if hit.hovered { 0.5 } else { 0.0 }));
            self.out.draw_list.rect(r, dot * 0.5, c, None);
            x += w + gap;
        }
        group
    }

    /// Paints a soft dark gradient over the bottom of `rect` (for legible
    /// captions on images or backdrops).
    pub fn paint_scrim(&mut self, rect: Rect, radius: f32, strength: f32) {
        let h = rect.height() * 0.55;
        let r = Rect::from_min_max(Vec2::new(rect.min.x, rect.max.y - h), rect.max);
        self.out.draw_list.rect_corners(
            r,
            [0.0, 0.0, radius, radius],
            Fill::Vertical(Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, (200.0 * strength.clamp(0.0, 1.0)) as u8)),
            None,
        );
    }
}
