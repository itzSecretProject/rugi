//! Overlay layer: popups, menus, context menus, tooltips and modal dialogs.
//!
//! Overlays are drawn into a separate draw list that is appended after the
//! main content, so a popup opened from the sidebar still covers the central
//! panel drawn later in the frame. Widgets underneath an open popup or modal
//! stop receiving the pointer (blocking uses the previous frame's popup
//! rectangles, because popups are usually built after the widgets they cover).

use crate::layout::{extent, Container, Dir, Layout};
use crate::{Channel, Response, Sense, Ui};
use rugi_core::draw::{DrawList, Fill};
use rugi_core::{id::split_label, Color, Id, Key, Rect, Vec2};
use std::rc::Rc;

/// What kind of floating container is being built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PopupKind {
    Popup,
    Modal,
}

#[derive(Default, Debug)]
pub(crate) struct PopupState {
    pub open: Option<Id>,
    pub anchor: Rect,
    pub opened_frame: u64,
    pub seen: bool,
    pub rects: Vec<Rect>,
    pub prev_rects: Vec<Rect>,
    pub modal_active: bool,
    pub modal_active_prev: bool,
    /// Depth of popup containers currently being built (not modals).
    pub popup_depth: u32,
    /// Menu rows and selectable rows of the open popup, in build order; the
    /// arrow keys move focus between them.
    pub items: Vec<Id>,
    /// The popup was opened from the keyboard; its first item takes focus.
    pub focus_first: bool,
    /// Widget that had focus when the popup opened; it gets focus back when
    /// the popup closes during keyboard navigation.
    pub opener: Option<Id>,
}

impl PopupState {
    pub fn begin_frame(&mut self) {
        std::mem::swap(&mut self.rects, &mut self.prev_rects);
        self.rects.clear();
        self.modal_active_prev = self.modal_active;
        self.modal_active = false;
        self.seen = false;
        self.popup_depth = 0;
        self.items.clear();
    }
}

#[derive(Default)]
struct PopupSize(Vec2);

/// Hover time before a tooltip appears (seconds).
const TOOLTIP_DELAY: f64 = 0.45;
const TOOLTIP_WRAP_WIDTH: f32 = 280.0;
const TOOLTIP_BG_LIGHT: Color = Color::hex(0x1B1D24);
const TOOLTIP_TEXT_LIGHT: Color = Color::hex(0xF2F3F6);
/// Fade-in time constant of popups and modals (seconds).
const APPEAR_TIME: f32 = 0.05;
/// Width of menus and context menus.
const MENU_WIDTH: f32 = 220.0;
/// Horizontal inset of a menu row's label and shortcut.
const MENU_ROW_INSET: f32 = 10.0;
/// Minimum distance kept between a popup and the window edge.
const SCREEN_MARGIN: f32 = 8.0;
/// Where a context menu opened from the keyboard appears, relative to the
/// top-left corner of the focused widget.
const MENU_KEY_INSET: f32 = 8.0;
const MENU_KEY_DROP: f32 = 28.0;
/// Gap between a popup and the widget it drops down from.
const ANCHOR_GAP: f32 = 4.0;
const POPUP_PADDING: f32 = 6.0;
const POPUP_SPACING: f32 = 2.0;

impl Ui {
    pub(crate) fn enter_overlay(&mut self) {
        if self.overlay_depth == 0 {
            std::mem::swap(&mut self.out.draw_list, &mut self.overlay);
        }
        self.overlay_depth += 1;
    }

    pub(crate) fn exit_overlay(&mut self) {
        if self.overlay_depth == 0 {
            return;
        }
        self.overlay_depth -= 1;
        if self.overlay_depth == 0 {
            std::mem::swap(&mut self.out.draw_list, &mut self.overlay);
        }
    }

    /// True when the pointer is covered by a popup/modal the current widget is not part of.
    pub(crate) fn pointer_blocked(&self) -> bool {
        let Some(p) = self.input.pointer else { return false };
        if self.popups.popup_depth == 0 && self.popups.prev_rects.iter().any(|r| r.contains(p)) {
            return true;
        }
        self.keyboard_blocked()
    }

    /// True when an open modal dialog covers the current widget, which then
    /// leaves the Tab order and ignores keyboard activation.
    #[inline]
    pub(crate) fn keyboard_blocked(&self) -> bool {
        self.overlay_depth == 0 && self.popups.modal_active_prev
    }

    pub(crate) fn finish_popups(&mut self) {
        let ps = &mut self.popups;
        if ps.open.is_some() && !ps.seen && ps.opened_frame != self.frame {
            ps.open = None;
        }
        let pressed = self.input.pressed[0] || self.input.pressed[1];
        if let (Some(_), Some(p), true) = (ps.open, self.input.pointer, pressed) {
            let inside = ps.rects.iter().any(|r| r.contains(p)) || ps.anchor.contains(p);
            if !inside && ps.opened_frame != self.frame {
                ps.open = None;
            }
        }
        if self.input.key_pressed(Key::Escape) {
            ps.open = None;
        }
    }

    /// Opens the popup `id` anchored to `anchor` (closes any other popup).
    pub fn open_popup(&mut self, id: Id, anchor: Rect) {
        self.popups.open = Some(id);
        self.popups.anchor = anchor;
        self.popups.opened_frame = self.frame;
        // No pointer button this frame: opened by Enter, Space or a menu key.
        let pointer = self.input.pressed[0] || self.input.released[0] || self.input.secondary_pressed;
        self.popups.focus_first = !pointer;
        self.popups.opener = self.focused;
        // Start the fade-in from zero.
        self.anims.remove(&id.with("appear"));
        self.request_repaint();
    }

    /// Closes the open popup, if any.
    pub fn close_popup(&mut self) {
        self.popups.open = None;
    }

    /// True if popup `id` is open.
    pub fn is_popup_open(&self, id: Id) -> bool {
        self.popups.open == Some(id)
    }

    pub(crate) fn push_floating(&mut self, id: Id, pos: Vec2, width: f32, kind: PopupKind, pad: f32, spacing: f32) {
        self.enter_overlay();
        if kind == PopupKind::Popup {
            self.popups.popup_depth += 1;
        }
        let start = self.out.draw_list.len();
        let shadow = self.out.draw_list.reserve();
        let bg = self.out.draw_list.reserve();
        self.out.draw_list.push_clip(self.screen);
        let region =
            Rect::from_min_size(pos + Vec2::new(pad, pad), Vec2::new((width - pad * 2.0).max(0.0), f32::INFINITY));
        self.layouts.push(Layout::new(
            Dir::Vertical,
            region,
            spacing,
            Container::Popup { id, shadow, bg, origin: pos, width, pad, kind, start },
        ));
    }

    /// Called by [`Ui::end`] for floating containers.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn end_floating(
        &mut self,
        id: Id,
        shadow: rugi_core::draw::Slot,
        bg: rugi_core::draw::Slot,
        origin: Vec2,
        width: f32,
        pad: f32,
        kind: PopupKind,
        start: usize,
        content: Vec2,
    ) {
        let size = Vec2::new(width, content.y + pad * 2.0);
        self.state_mut::<PopupSize>(id).0 = size;
        let rect = Rect::from_min_size(origin, size);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let radius = if kind == PopupKind::Modal { th.metrics.radius_lg } else { th.metrics.radius_md + 2.0 };
        let sigma = if kind == PopupKind::Modal { 24.0 } else { 14.0 };
        let sh = DrawList::shadow_instance(rect.translate(Vec2::new(0.0, 6.0)), radius, sigma, p.shadow);
        self.out.draw_list.fill_slot(shadow, sh);
        // Raised surfaces are lighter than the page in dark themes.
        let fill = if th.dark { p.surface_highlight } else { p.surface };
        let inst = DrawList::rect_instance(rect, [radius; 4], Fill::Solid(fill), Some((1.0, p.border_strong)));
        self.out.draw_list.fill_slot(bg, inst);
        let appear = self.animate(id.with("appear"), Channel::Value, 1.0, APPEAR_TIME);
        self.out.draw_list.fade_from(start, appear);
        self.out.draw_list.pop_clip();
        if kind == PopupKind::Popup {
            // Popups block the pointer for content beneath them; modals block
            // everything outside the overlay through `modal_active` instead.
            self.popups.rects.push(rect);
            self.popups.popup_depth = self.popups.popup_depth.saturating_sub(1);
        }
        self.exit_overlay();
    }

    fn appear_start(&mut self, id: Id) {
        // New animation entries start at their target; force 0 for fade-in.
        if !self.anims.contains_key(&id.with("appear")) {
            self.animate(id.with("appear"), Channel::Value, 0.0, APPEAR_TIME);
        }
    }

    /// Begins popup `id` placed below `anchor` (flipped above if it would not
    /// fit). Returns `true` if the popup is open; then add items and call
    /// [`Ui::end`]. `width` 0 uses the anchor width (min 160).
    pub fn begin_popup_below(&mut self, id: Id, anchor: Rect, width: f32) -> bool {
        if !self.frame_guard() || self.popups.open != Some(id) {
            return false;
        }
        self.popups.seen = true;
        self.appear_start(id);
        let w = if width > 0.0 { extent(width) } else { anchor.width().max(160.0) };
        let h = self.state_mut::<PopupSize>(id).0.y;
        let mut y = anchor.max.y + ANCHOR_GAP;
        if y + h > self.screen.max.y - SCREEN_MARGIN && anchor.min.y - ANCHOR_GAP - h > SCREEN_MARGIN {
            y = anchor.min.y - ANCHOR_GAP - h;
        }
        let x = anchor.min.x.min(self.screen.max.x - w - SCREEN_MARGIN).max(SCREEN_MARGIN);
        self.push_floating(id, Vec2::new(x, y), w, PopupKind::Popup, POPUP_PADDING, POPUP_SPACING);
        true
    }

    /// Context menu for the most recent widget: opens on right-click over it.
    /// Returns `true` while open; add [`Ui::menu_item`]s and call [`Ui::end`].
    pub fn begin_context_menu(&mut self, id_source: &str) -> bool {
        if !self.frame_guard() {
            return false;
        }
        let id = self.make_id(id_source);
        let target = self.last_response.rect;
        let menu_key =
            self.input.key_pressed(Key::ContextMenu) || self.input.key_pressed_mods(Key::F10).is_some_and(|m| m.shift);
        if self.last_response.focused && menu_key {
            let anchor = Vec2::new(target.min.x + MENU_KEY_INSET, target.min.y + target.height().min(MENU_KEY_DROP));
            self.open_popup(id, Rect::from_min_size(anchor, Vec2::ZERO));
        }
        if self.input.secondary_pressed && !self.pointer_blocked() {
            if let Some(p) = self.input.pointer {
                if target.contains(p) && self.out.draw_list.clip().contains(p) {
                    self.open_popup(id, Rect::from_min_size(p, Vec2::ZERO));
                }
            }
        }
        if self.popups.open != Some(id) {
            return false;
        }
        self.popups.seen = true;
        self.appear_start(id);
        let size = self.state_mut::<PopupSize>(id).0;
        let w = MENU_WIDTH;
        let a = self.popups.anchor.min;
        let x = a.x.min(self.screen.max.x - w - SCREEN_MARGIN).max(SCREEN_MARGIN);
        let y = if a.y + size.y > self.screen.max.y - SCREEN_MARGIN { (a.y - size.y).max(SCREEN_MARGIN) } else { a.y };
        self.push_floating(id, Vec2::new(x, y), w, PopupKind::Popup, POPUP_PADDING, POPUP_SPACING);
        true
    }

    /// A menu-bar style button that toggles a dropdown menu. Returns `true`
    /// while the menu is open; add [`Ui::menu_item`]s and call [`Ui::end`].
    pub fn begin_menu(&mut self, label: &str) -> bool {
        if !self.frame_guard() {
            return false;
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src).with("menu");
        let style = self.theme.typography.label;
        let layout = self.layout_text(text, &style, None);
        let h = self.theme.metrics.control_height - 4.0;
        let r = self.allocate(Vec2::new(layout.size().x + 20.0, h), false);
        let resp = self.interact(id, r, Sense::BUTTON);
        let open = self.popups.open == Some(id);
        if resp.clicked {
            if open {
                self.close_popup();
            } else {
                self.open_popup(id, r);
            }
        } else if resp.hovered && !open {
            // Menu-bar behaviour: hovering a sibling while a menu is open switches to it.
            if self.popups.open.is_some() && (self.popups.anchor.min.y - r.min.y).abs() < 0.5 {
                self.open_popup(id, r);
            }
        }
        let open = self.popups.open == Some(id);
        let v = self.visual(&resp);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let bg = p.surface_hover.fade(v.hover.max(open as u8 as f32));
        self.out.draw_list.rect(r, th.metrics.radius_sm + 1.0, bg, None);
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        let fg = if open { p.text } else { p.text_muted.lerp(p.text, v.hover) };
        self.paint_text(&layout, Vec2::new(r.min.x + 10.0, ty), fg);
        self.record("menu", text, &resp);
        self.begin_popup_below(id, r, MENU_WIDTH)
    }

    /// A selectable row (list item). Returns its response; `.clicked` when chosen.
    pub fn selectable(&mut self, label: &str, selected: bool) -> Response {
        self.menu_row(label, "", selected, false)
    }

    /// A menu entry with an optional right-aligned shortcut hint. Clicking it
    /// closes the open popup.
    pub fn menu_item(&mut self, label: &str, shortcut: &str) -> Response {
        let r = self.menu_row(label, shortcut, false, true);
        if r.clicked {
            self.close_popup();
        }
        r
    }

    fn menu_row(&mut self, label: &str, shortcut: &str, selected: bool, menu: bool) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (text, src) = split_label(label);
        let id = self.make_id(src);
        // 30 and 32 px with the default 34 px controls.
        let h = (self.theme.metrics.control_height - if menu { 4.0 } else { 2.0 }).max(0.0);
        if self.next_offscreen(h) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(0.0, h), true);
        }
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let r = self.allocate(Vec2::new(0.0, h), true);
        let resp = self.interact(id, r, Sense::BUTTON);
        if self.popups.popup_depth > 0 && !self.keyboard_blocked() {
            self.popups.items.push(id);
        }
        let v = self.visual(&resp);
        let bg = if selected { p.accent_soft } else { p.surface_hover.fade(v.hover).lerp(p.surface_active, v.press) };
        self.out.draw_list.rect(r, th.metrics.radius_sm + 1.0, bg, None);
        let mut right = r.max.x - MENU_ROW_INSET;
        if !shortcut.is_empty() {
            let sl = self.layout_text(shortcut, &th.typography.caption, None);
            let sy = r.min.y + ((h - sl.size().y) * 0.5).round();
            right -= sl.size().x;
            self.paint_text(&sl, Vec2::new(right, sy), p.text_faint);
            right -= MENU_ROW_INSET * 2.0;
        }
        if selected {
            right -= 26.0;
        }
        let x = r.min.x + MENU_ROW_INSET;
        let layout = self.layout_text_fit(text, &th.typography.body, (right - x).max(0.0));
        let ty = r.min.y + ((h - layout.size().y) * 0.5).round();
        self.paint_text(&layout, Vec2::new(x, ty), p.text);
        if selected {
            let ir = Rect::from_xywh(r.max.x - 26.0, r.center().y - 8.0, 16.0, 16.0);
            self.paint_icon(crate::lucide::CHECK.into(), ir, p.accent_text);
        }
        self.focus_ring(r, th.metrics.radius_sm + 1.0, v.focus);
        self.record(if menu { "menu_item" } else { "selectable" }, text, &resp);
        resp
    }

    /// Thin separator inside menus and popups.
    pub fn menu_separator(&mut self) {
        if !self.frame_guard() {
            return;
        }
        let r = self.allocate(Vec2::new(0.0, 9.0), true);
        let c = self.theme.palette.border;
        self.out.draw_list.rect(Rect::from_xywh(r.min.x + 4.0, r.center().y, r.width() - 8.0, 1.0), 0.0, c, None);
    }

    /// Shows a tooltip for the most recent widget after a short hover delay.
    pub fn tooltip(&mut self, text: &str) {
        if !self.frame_guard() {
            return;
        }
        let r = self.last_response;
        if !r.hovered || self.hover_since.0 != Some(r.id) {
            return;
        }
        let elapsed = self.time - self.hover_since.1;
        if elapsed < TOOLTIP_DELAY {
            self.request_repaint_after(std::time::Duration::from_secs_f64(TOOLTIP_DELAY - elapsed));
            return;
        }
        let Some(ptr) = self.input.pointer else { return };
        let style = self.theme.typography.caption;
        let layout = self.layout_text(text, &style, Some(TOOLTIP_WRAP_WIDTH));
        let size = layout.size() + Vec2::new(20.0, 12.0);
        let mut pos = ptr + Vec2::new(12.0, 18.0);
        pos.x = pos.x.min(self.screen.max.x - size.x - 6.0).max(6.0);
        if pos.y + size.y > self.screen.max.y - 6.0 {
            pos.y = ptr.y - size.y - 8.0;
        }
        let rect = Rect::from_min_size(pos, size);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let tip_id = r.id.with("tooltip");
        let appear = self.animate(tip_id, Channel::Value, 1.0, 0.06);
        self.enter_overlay();
        let start = self.out.draw_list.len();
        self.out.draw_list.push_clip(self.screen);
        self.out.draw_list.shadow(rect.translate(Vec2::new(0.0, 3.0)), 7.0, 8.0, p.shadow.fade(0.8));
        // Light themes use a dark tooltip for contrast with the page.
        let bg = if th.dark { p.surface_active } else { TOOLTIP_BG_LIGHT };
        let fg = if th.dark { p.text } else { TOOLTIP_TEXT_LIGHT };
        self.out.draw_list.rect(rect, 7.0, bg, Some((1.0, p.border_strong.fade(if th.dark { 1.0 } else { 0.0 }))));
        self.paint_text(&layout, pos + Vec2::new(10.0, 6.0), fg);
        self.out.draw_list.fade_from(start, appear);
        self.out.draw_list.pop_clip();
        self.exit_overlay();
    }

    /// Begins a modal dialog while `*open` is true: dims the window, blocks
    /// input to everything else, and shows a centered card with `title` and a
    /// close button (Escape also closes). Returns `true` when visible; add
    /// content and call [`Ui::end`].
    pub fn begin_modal(&mut self, id_source: &str, title: &str, open: &mut bool, width: f32) -> bool {
        if !self.frame_guard() || !*open {
            return false;
        }
        let id = self.make_id(id_source);
        if self.input.key_pressed(Key::Escape) {
            *open = false;
            return false;
        }
        self.popups.modal_active = true;
        self.appear_start(id);
        let w = if width > 0.0 { extent(width) } else { 440.0 };
        let size = self.state_mut::<PopupSize>(id).0;
        let h = if size.y > 0.0 { size.y } else { 200.0 };
        let pos = Vec2::new(
            (self.screen.center().x - w * 0.5).round(),
            ((self.screen.center().y - h * 0.5).max(16.0)).round(),
        );
        let appear = self.anims.get(&id.with("appear")).map(|a| a.v[Channel::Value as usize]).unwrap_or(0.0);
        self.enter_overlay();
        let scrim = if self.theme.dark { Color::rgba(0, 0, 0, 140) } else { Color::rgba(15, 18, 30, 90) };
        self.out.draw_list.push_clip(self.screen);
        self.out.draw_list.rect(self.screen, 0.0, scrim.fade(appear.max(0.0)), None);
        self.out.draw_list.pop_clip();
        let pad = self.theme.metrics.card_padding + 4.0;
        let spacing = self.theme.metrics.spacing;
        self.push_floating(id, pos, w, PopupKind::Modal, pad, spacing);
        // Title row with close button (right-aligned row, title fills the rest).
        self.begin_horizontal_centered(crate::Align::End);
        let close = self.button_ex("##close", crate::ButtonKind::Ghost, Some(crate::lucide::X.into())).clicked;
        self.vertical(|ui| ui.heading(title));
        self.end();
        // Balance the extra overlay level opened for the scrim.
        self.overlay_depth -= 1;
        if close {
            *open = false;
        }
        true
    }

    /// Closure form of [`Ui::begin_popup_below`], sized to the anchor width.
    pub fn popup_below<R>(&mut self, id: Id, anchor: Rect, f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        if self.begin_popup_below(id, anchor, 0.0) {
            let r = f(self);
            self.end();
            Some(r)
        } else {
            None
        }
    }

    /// Closure form of [`Ui::begin_menu`].
    pub fn menu<R>(&mut self, label: &str, f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        if self.begin_menu(label) {
            let r = f(self);
            self.end();
            Some(r)
        } else {
            None
        }
    }

    /// Closure form of [`Ui::begin_context_menu`].
    pub fn context_menu<R>(&mut self, id_source: &str, f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        if self.begin_context_menu(id_source) {
            let r = f(self);
            self.end();
            Some(r)
        } else {
            None
        }
    }

    /// Closure form of [`Ui::begin_modal`].
    pub fn modal<R>(
        &mut self,
        id_source: &str,
        title: &str,
        open: &mut bool,
        width: f32,
        f: impl FnOnce(&mut Ui) -> R,
    ) -> Option<R> {
        if self.begin_modal(id_source, title, open, width) {
            let r = f(self);
            self.end();
            Some(r)
        } else {
            None
        }
    }
}
