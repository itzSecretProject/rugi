//! Frame lifecycle: input queueing, `begin_frame` / `end_frame`, and focus
//! bookkeeping that runs once per frame.

use crate::layout::{Layout, MAX_EXTENT};
use crate::{FrameInput, FrameOutput, Ui, UiError};
use rugi_core::draw::Fill;
use rugi_core::{CursorIcon, FrameStats, InputEvent, Key, Rect, Repaint, Vec2};
use std::time::{Duration, Instant};

/// Frame time assumed before the first frame and when the host reports garbage.
pub(crate) const DEFAULT_DT: f32 = 1.0 / 60.0;
/// Upper bound for `dt`, so a stalled frame does not make animations jump.
const MAX_DT: f32 = 0.25;
/// Accepted scale factors; values outside are clamped so glyph rasterization
/// stays within sane pixel sizes.
const SCALE_RANGE: (f32, f32) = (1.0 / 16.0, 16.0);
/// Longest delay a [`Repaint::After`] request carries: longer waits are
/// pointless, and host timer arithmetic (`Instant + Duration`) could overflow.
pub(crate) const MAX_REPAINT_DELAY: Duration = Duration::from_secs(3600);
/// Animation state of widgets not seen for this many frames is dropped.
const ANIM_GC_FRAMES: u64 = 120;

/// `secs` as a repaint delay, clamped to `0..=MAX_REPAINT_DELAY` (NaN and
/// infinity mean "as late as possible").
pub(crate) fn repaint_delay(secs: f64) -> Duration {
    if secs.is_nan() || secs >= MAX_REPAINT_DELAY.as_secs_f64() {
        MAX_REPAINT_DELAY
    } else {
        Duration::from_secs_f64(secs.max(0.0))
    }
}

impl Ui {
    /// Queues an input event for the next frame.
    pub fn push_event(&mut self, event: InputEvent) {
        self.events.push(event);
    }

    /// True if events are queued (the platform layer should render a frame).
    pub fn has_pending_events(&self) -> bool {
        !self.events.is_empty()
    }

    /// Starts a frame, consuming queued input.
    pub fn begin_frame(&mut self, input: FrameInput) -> Result<(), UiError> {
        if self.in_frame {
            return Err(UiError::FrameInProgress);
        }
        if !(input.scale.is_finite() && input.scale > 0.0) {
            return Err(UiError::InvalidArgument("scale must be positive"));
        }
        if !(input.size.x.is_finite() && input.size.y.is_finite()) {
            return Err(UiError::InvalidArgument("size must be finite"));
        }
        self.build_start = Some(Instant::now());
        self.in_frame = true;
        self.frame += 1;
        self.dt = if input.dt.is_finite() { input.dt.clamp(0.0, MAX_DT) } else { DEFAULT_DT };
        self.time += self.dt as f64;
        self.scale = input.scale.clamp(SCALE_RANGE.0, SCALE_RANGE.1);
        self.screen =
            Rect::from_min_size(Vec2::ZERO, input.size.max(Vec2::ZERO).min(Vec2::new(MAX_EXTENT, MAX_EXTENT)));
        self.remaining = self.screen;
        self.input.begin_frame(&mut self.events);
        if self.input.window_unfocused {
            self.active = None;
        }
        self.tool_shortcuts();
        // Route the wheel to the innermost scrollable area under the pointer,
        // using last frame's geometry: areas do not move without input, and
        // the target must be known before their children are laid out.
        std::mem::swap(&mut self.scroll_areas, &mut self.prev_scroll_areas);
        self.scroll_areas.clear();
        self.wheel_target = self.input.pointer.and_then(|p| {
            self.prev_scroll_areas
                .iter()
                .filter(|(_, r, _, can)| *can && r.contains(p))
                .max_by_key(|(_, _, d, _)| *d)
                .map(|(id, ..)| *id)
        });

        self.text.begin_frame();
        self.out.draw_list.clear();
        self.overlay.clear();
        self.overlay_depth = 0;
        self.popups.begin_frame();
        self.out.clipboard = None;
        self.out.window_commands.clear();
        self.ambient = false;
        self.out.widgets.clear();
        self.out.atlas_update = None;
        self.out.cursor = CursorIcon::Default;
        self.out.exit_requested = false;
        self.id_stack.clear();
        self.focus_order.clear();
        self.tab_pressed = self.input.key_pressed(Key::Tab);
        self.focus_reveal = None;
        self.active_seen = false;
        self.focus_seen = false;
        self.pointer_claimed = false;
        self.widget_count = 0;
        self.next_width = None;
        self.centered_rows = 0;
        self.repaint = Repaint::Idle;
        self.layouts.clear();
        self.layouts.push(Layout::root(self.screen, self.theme.metrics.spacing));

        if !self.transparent_canvas {
            let p = &self.theme.palette;
            self.out.draw_list.rect(self.screen, 0.0, Fill::Vertical(p.background, p.background_alt), None);
        }
        Ok(())
    }

    /// Finishes the frame and returns its output. Unclosed containers are
    /// auto-closed (reported via [`Ui::take_error`]).
    pub fn end_frame(&mut self) -> &FrameOutput {
        if !self.in_frame {
            self.misuse = Some(UiError::NoFrame);
            return &self.out;
        }
        if !self.style_stack.is_empty() {
            // Restore the base theme so an unbalanced push never leaks into the next frame.
            let base = self.style_stack.swap_remove(0);
            self.style_stack.clear();
            self.theme = base;
            self.misuse = Some(UiError::UnbalancedStyle);
        }
        let open = self.layouts.len().saturating_sub(1);
        if open > 0 {
            for _ in 0..open {
                self.end();
            }
            self.misuse = Some(UiError::UnclosedContainers(open));
        }
        while self.overlay_depth > 0 {
            self.exit_overlay();
        }
        self.inspector_overlay();
        self.finish_focus();
        self.finish_popups();
        if !self.overlay.instances().is_empty() {
            self.out.draw_list.append(&self.overlay);
        }
        if self.input.released[0] || !self.active_seen {
            self.active = None;
        }
        if self.frame.is_multiple_of(ANIM_GC_FRAMES) {
            let f = self.frame;
            self.anims.retain(|_, a| f - a.last_frame < ANIM_GC_FRAMES);
        }
        if self.text.atlas_overflowed() {
            self.repaint = Repaint::Immediately;
        }
        if self.ambient {
            let fps = if self.input.window_blurred { self.ambient_rate.unfocused } else { self.ambient_rate.focused };
            self.repaint = match fps {
                Some(f) if f.is_finite() && f > 0.0 => {
                    self.repaint.merge(Repaint::After(repaint_delay((1.0 / f) as f64)))
                }
                _ => Repaint::Immediately,
            };
        }
        self.out.atlas_update = self.text.take_atlas_update();
        self.out.repaint = Some(self.repaint);
        let dl = &self.out.draw_list;
        self.out.stats = FrameStats {
            build_us: self.build_start.map(|s| s.elapsed().as_micros() as u32).unwrap_or(0),
            instances: dl.instances().len() as u32,
            batches: dl.commands().len() as u32,
            culled: dl.culled(),
            widgets: self.widget_count,
            text_shaped: self.text.shaped_this_frame(),
            glyphs_rasterized: self.text.rasterized_this_frame(),
        };
        self.last_stats = self.out.stats;
        self.in_frame = false;
        &self.out
    }

    /// Output of the most recent frame.
    pub fn output(&self) -> &FrameOutput {
        &self.out
    }

    /// Forces the next atlas update to include the full atlas (call after the
    /// renderer was recreated).
    pub fn invalidate_gpu_resources(&mut self) {
        self.text.invalidate_atlas_upload();
    }

    /// Returns and clears the last recorded API misuse.
    pub fn take_error(&mut self) -> Option<UiError> {
        self.misuse.take()
    }

    pub(crate) fn report(&mut self, e: UiError) {
        self.misuse = Some(e);
    }

    /// Reports [`UiError::NoFrame`] and returns `false` outside a frame.
    #[inline(always)]
    pub(crate) fn frame_guard(&mut self) -> bool {
        if !self.in_frame {
            self.report(UiError::NoFrame);
            return false;
        }
        true
    }

    fn finish_focus(&mut self) {
        self.reveal_focus = false;
        if !self.focus_seen {
            self.focused = None;
        }
        if self.input.pressed[0] && !self.pointer_claimed {
            self.focused = None;
        }
        if self.input.key_pressed(Key::Escape) {
            self.focused = None;
        }
        self.navigate_popup();
        if let Some(mods) = self.input.key_pressed_mods(Key::Tab) {
            if !self.focus_order.is_empty() {
                let n = self.focus_order.len();
                let cur = self.focused.and_then(|f| self.focus_order.iter().position(|&x| x == f));
                let next = match (cur, mods.shift) {
                    (Some(i), false) => (i + 1) % n,
                    (Some(i), true) => (i + n - 1) % n,
                    (None, false) => 0,
                    (None, true) => n - 1,
                };
                self.focused = Some(self.focus_order[next]);
                self.focus_visible = true;
                self.reveal_focus = true;
                self.repaint = Repaint::Immediately;
            }
        }
    }

    /// Arrow keys between the rows of the open popup, focus on its first row
    /// when it was opened from the keyboard, and focus back to the opener
    /// when it closes.
    fn navigate_popup(&mut self) {
        let p = &mut self.popups;
        if p.open.is_none() {
            if let Some(opener) = p.opener.take() {
                // Focus is gone, or on a row of the popup that just closed.
                let lost = self.focused.is_none_or(|f| p.items.contains(&f));
                if lost && self.focus_visible {
                    self.focused = Some(opener);
                }
            }
            return;
        }
        if p.items.is_empty() {
            return;
        }
        let n = p.items.len();
        let cur = self.focused.and_then(|f| p.items.iter().position(|&x| x == f));
        let next = if std::mem::take(&mut p.focus_first) {
            Some(0)
        } else if cur.is_some() || self.focused.is_none() || self.focused == p.opener {
            let down = self.input.key_pressed(Key::ArrowDown);
            let up = self.input.key_pressed(Key::ArrowUp);
            match (cur, down, up) {
                (Some(i), true, _) => Some((i + 1) % n),
                (Some(i), _, true) => Some((i + n - 1) % n),
                (None, true, _) => Some(0),
                (None, _, true) => Some(n - 1),
                _ => None,
            }
        } else {
            None
        };
        if let Some(i) = next {
            self.focused = Some(p.items[i]);
            self.focus_visible = true;
            self.repaint = Repaint::Immediately;
        }
    }
}
