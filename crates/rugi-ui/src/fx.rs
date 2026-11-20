//! Effects and window-level composition: animated backdrops, glass surfaces,
//! glass panels/cards, screen transitions and custom window chrome.
//!
//! ```no_run
//! # use rugi_ui::*;
//! # fn f(ui: &mut Ui) {
//! ui.backdrop(&Backdrop::new(BackdropKind::Smoke));
//! let r = Rect::from_xywh(40.0, 40.0, 420.0, 300.0);
//! ui.begin_glass_panel(r, &GlassStyle::liquid());
//! ui.heading("Glass over smoke");
//! ui.end();
//! # }
//! ```

use crate::layout::{Container, Dir, Layout};
use crate::{Channel, Response, Sense, Ui};
use rugi_core::anim::{ease_out_cubic, Spring};
use rugi_core::{Backdrop, Color, CursorIcon, DrawList, GlassStyle, Rect, Repaint, Vec2, WindowCommand};
use std::rc::Rc;

#[derive(Default)]
struct TransitionState {
    key: Option<u64>,
    start: f64,
    dir: f32,
}

/// Direction a screen transition slides in from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Slide {
    /// Fade only.
    #[default]
    None,
    /// From the right (forward navigation).
    Forward,
    /// From the left (back navigation).
    Back,
    /// From slightly below.
    Up,
}

impl Ui {
    /// Animation clock for effects, wrapped to keep `f32` precision.
    fn fx_time(&self) -> f32 {
        (self.time % 3600.0) as f32
    }

    /// Fills the whole window with an animated procedural backdrop. Call it
    /// first in the frame. While `speed > 0` it requests frames at the
    /// [`crate::AmbientRate`] (full refresh when focused, 30 fps unfocused by
    /// default).
    pub fn backdrop(&mut self, b: &Backdrop) {
        let r = self.screen;
        self.backdrop_rect(r, 0.0, b);
    }

    /// Paints an animated backdrop into `rect` with rounded corners (cards,
    /// banners, generated cover art).
    pub fn backdrop_rect(&mut self, rect: Rect, radius: f32, b: &Backdrop) {
        if !self.frame_guard() {
            return;
        }
        let t = self.fx_time();
        let ptr = self.input.pointer;
        self.out.draw_list.backdrop(rect, radius, b, t, ptr);
        if b.speed != 0.0 {
            self.ambient = true;
        }
    }

    /// Paints a glass surface over `rect`: everything drawn earlier in the
    /// frame shows through blurred, tinted and refracted.
    pub fn glass(&mut self, rect: Rect, style: &GlassStyle) {
        if self.frame_guard() {
            self.out.draw_list.glass(rect, style);
        }
    }

    /// Starts a floating glass panel at an explicit rectangle (does not take
    /// space in the parent). Content is laid out vertically inside with the
    /// theme's panel padding and clipped to the panel. Close with [`Ui::end`].
    pub fn begin_glass_panel(&mut self, rect: Rect, style: &GlassStyle) {
        if !self.frame_guard() {
            return;
        }
        let pad = self.theme.metrics.panel_padding;
        self.paint_glass_surface(rect, style, 1.0);
        self.out.draw_list.push_clip(rect);
        let spacing = self.theme.metrics.spacing;
        self.layouts.push(Layout::new(Dir::Vertical, rect.shrink2(pad, pad), spacing, Container::Panel));
    }

    /// Glass + soft drop shadow.
    pub(crate) fn paint_glass_surface(&mut self, rect: Rect, style: &GlassStyle, opacity: f32) {
        let shadow = self.theme.palette.shadow.fade(0.75 * opacity);
        self.out.draw_list.shadow(rect.translate(Vec2::new(0.0, 10.0)), style.radius, 24.0, shadow);
        let inst = DrawList::glass_instance(rect, style, opacity);
        let slot = self.out.draw_list.reserve();
        self.out.draw_list.fill_slot(slot, inst);
    }

    /// Starts a card whose surface is glass instead of the theme surface.
    /// Behaves like [`Ui::begin_card`] (size follows content). Close with [`Ui::end`].
    pub fn begin_glass_card(&mut self, style: &GlassStyle) {
        if !self.frame_guard() {
            return;
        }
        let pad = self.theme.metrics.card_padding;
        let shadow = self.out.draw_list.reserve();
        let bg = self.out.draw_list.reserve();
        let (origin, w) = self.top().peek();
        let region =
            Rect::from_min_size(origin + Vec2::new(pad, pad), Vec2::new((w - pad * 2.0).max(0.0), f32::INFINITY));
        let spacing = self.theme.metrics.spacing;
        self.layouts.push(Layout::new(
            Dir::Vertical,
            region,
            spacing,
            Container::Card { shadow, bg, padding: pad, min_height: 0.0, elevated: true, glass: Some(*style) },
        ));
    }

    /// Closure form of [`Ui::begin_glass_card`].
    pub fn glass_card<R>(&mut self, style: &GlassStyle, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.begin_glass_card(style);
        let r = f(self);
        self.end();
        r
    }

    /// Starts a screen whose content fades (and optionally slides) in
    /// whenever `key` changes, e.g. the index of the current page. Wrap the
    /// page content and close with [`Ui::end`].
    pub fn begin_transition(&mut self, id_source: &str, key: u64, slide: Slide) {
        if !self.frame_guard() {
            return;
        }
        let id = self.make_id(id_source);
        let now = self.time;
        let st = self.state_mut::<TransitionState>(id);
        if st.key != Some(key) {
            let first = st.key.is_none();
            st.key = Some(key);
            st.start = if first { now - 10.0 } else { now };
            st.dir = match slide {
                Slide::None => 0.0,
                Slide::Forward | Slide::Up => 1.0,
                Slide::Back => -1.0,
            };
        }
        let (start, dir) = (st.start, st.dir);
        let raw = ((now - start) / 0.32).clamp(0.0, 1.0) as f32;
        let e = ease_out_cubic(raw);
        if raw < 1.0 {
            self.repaint = Repaint::Immediately;
        }
        let shift = (1.0 - e) * 22.0 * dir;
        let offset = if slide == Slide::Up { Vec2::new(0.0, shift) } else { Vec2::new(shift, 0.0) };
        let spacing = self.theme.metrics.spacing;
        let (origin, w) = self.top().peek();
        // Keep the parent's bottom bound so fill-height children (scroll
        // areas) still know how much room they have.
        let bottom = self.top().region.max.y;
        let h = if bottom.is_finite() { (bottom - origin.y).max(0.0) } else { f32::INFINITY };
        let region = Rect::from_min_size(origin + offset, Vec2::new(w, h));
        let start_idx = self.out.draw_list.len();
        self.layouts.push(Layout::new(
            Dir::Vertical,
            region,
            spacing,
            Container::Transition { start: start_idx, fade: e, offset },
        ));
    }

    /// Queues a window request (drag, minimize, maximize, close) for the
    /// platform layer.
    pub fn window_command(&mut self, cmd: WindowCommand) {
        self.out.window_commands.push(cmd);
    }

    /// A custom title bar for borderless windows: `title` on the left,
    /// minimize and close buttons on the right, and the empty area drags the
    /// window. Allocates the full available width. Returns the bar's
    /// response (`.clicked` on double-click is not reported).
    pub fn title_bar(&mut self, title: &str, height: f32) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let id = self.make_id("##rugi-title-bar");
        let r = self.allocate(Vec2::new(0.0, height), true);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        // Buttons first so they win the press over the drag area.
        let bs = 28.0;
        let close_r = Rect::from_xywh(r.max.x - bs, r.center().y - bs * 0.5, bs, bs);
        let min_r = close_r.translate(Vec2::new(-bs - 6.0, 0.0));
        let close = self.chrome_button(id.with("close"), close_r, crate::lucide::X, true);
        let min = self.chrome_button(id.with("min"), min_r, crate::lucide::MINUS, false);
        if close.clicked {
            self.window_command(WindowCommand::Close);
        }
        if min.clicked {
            self.window_command(WindowCommand::Minimize);
        }
        let drag_r = Rect::from_min_max(r.min, Vec2::new(min_r.min.x - 6.0, r.max.y));
        let resp = self.interact(id, drag_r, Sense { focusable: false, ..Sense::DRAG });
        if resp.pressed && self.input.pressed[0] {
            self.window_command(WindowCommand::StartDrag);
        }
        if !title.is_empty() {
            let style = th.typography.label.weight(rugi_core::FontWeight::SEMIBOLD);
            let layout = self.layout_text(title, &style, None);
            let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
            self.paint_text(&layout, Vec2::new(r.min.x, ty), p.text);
        }
        self.record("title_bar", title, &resp);
        resp
    }

    fn chrome_button(&mut self, id: rugi_core::Id, r: Rect, icon: &str, danger: bool) -> Response {
        let resp = self.interact(id, r, Sense { focusable: false, ..Sense::BUTTON });
        let v = self.visual(&resp);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let hover_bg = if danger { p.danger.with_alpha(0.85) } else { p.surface_active };
        if v.hover > 0.0 {
            self.out.draw_list.rect(r, th.metrics.radius_md, hover_bg.fade(v.hover), None);
        }
        let fg = if danger { p.text_muted.lerp(Color::WHITE, v.hover) } else { p.text_muted.lerp(p.text, v.hover) };
        self.paint_icon(icon.into(), r.shrink2(7.0, 7.0), fg);
        resp
    }

    /// A ghost icon button of `size` (square). Shows `tooltip` on hover when
    /// non-empty.
    pub fn icon_button(&mut self, id_source: &str, icon: crate::IconRef<'_>, size: f32, tooltip: &str) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let id = self.make_id(id_source);
        let r = self.allocate(Vec2::new(size, size), false);
        let resp = self.interact(id, r, Sense::BUTTON);
        let v = self.visual(&resp);
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        if v.hover > 0.0 || v.press > 0.0 {
            let bg = p.surface_hover.lerp(p.surface_active, v.press).fade(v.hover.max(v.press));
            self.out.draw_list.rect(r, th.metrics.radius_md, bg, None);
        }
        let inset = (size * 0.25).round();
        self.paint_icon(icon, r.shrink2(inset, inset), p.text_muted.lerp(p.text, v.hover));
        self.focus_ring(r, th.metrics.radius_md, v.focus);
        self.record("icon_button", id_source, &resp);
        if !tooltip.is_empty() {
            self.tooltip(tooltip);
        }
        resp
    }

    /// Animated value helper for custom widgets: smoothly approaches
    /// `target` (exponential, `time_constant` seconds) and keeps the frame
    /// loop running until it settles.
    pub fn animate_value(&mut self, id: rugi_core::Id, target: f32, time_constant: f32) -> f32 {
        self.animate(id, Channel::Value, target, time_constant)
    }

    /// Spring-animated value for custom widgets (critically damped; higher
    /// `omega` responds faster, 20–30 is typical). Keeps repainting until settled.
    pub fn spring_value(&mut self, id: rugi_core::Id, target: f32, omega: f32) -> f32 {
        let dt = self.dt;
        #[derive(Default)]
        struct S(Option<Spring>);
        let st = self.state_mut::<S>(id);
        let s = st.0.get_or_insert(Spring::at(target));
        let moving = s.step(target, dt, omega);
        let pos = s.pos;
        if moving {
            self.repaint = Repaint::Immediately;
        }
        pos
    }

    /// Sets the requested mouse cursor for this frame.
    pub fn set_cursor(&mut self, cursor: CursorIcon) {
        self.out.cursor = cursor;
    }
}
