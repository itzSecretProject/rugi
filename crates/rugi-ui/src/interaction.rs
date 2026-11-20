//! Pointer and keyboard interaction shared by all widgets, plus the
//! off-screen fast paths widgets use to skip work for invisible items.

use crate::Ui;
use rugi_core::{CursorIcon, Id, Key, Rect, Vec2, WidgetInfo};

/// Result of an interactive widget.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Response {
    /// Widget id.
    pub id: Id,
    /// Widget bounds.
    pub rect: Rect,
    /// Pointer is over the widget.
    pub hovered: bool,
    /// Pointer button held on the widget.
    pub pressed: bool,
    /// Activated this frame (mouse click, or Enter/Space while focused).
    pub clicked: bool,
    /// Has keyboard focus.
    pub focused: bool,
    /// The bound value changed this frame.
    pub changed: bool,
    /// Enter pressed in a text input.
    pub submitted: bool,
}

/// What a widget reacts to.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Sense {
    pub click: bool,
    pub drag: bool,
    pub focusable: bool,
    /// Enter/Space activate when focused.
    pub keyboard_activate: bool,
}

impl Sense {
    pub const BUTTON: Sense = Sense { click: true, drag: false, focusable: true, keyboard_activate: true };
    pub const DRAG: Sense = Sense { click: true, drag: true, focusable: true, keyboard_activate: false };
    pub const HOVER: Sense = Sense { click: false, drag: false, focusable: false, keyboard_activate: false };
}

impl Ui {
    pub(crate) fn interact(&mut self, id: Id, rect: Rect, sense: Sense) -> Response {
        self.widget_count += 1;
        self.last_item = rect;
        let clip = self.out.draw_list.clip();
        let in_rect =
            self.input.pointer.is_some_and(|p| rect.contains(p) && clip.contains(p)) && !self.pointer_blocked();
        let interactive = sense.click || sense.drag;
        let hovered = in_rect && (self.active.is_none() || self.active == Some(id));
        let keyboard_blocked = self.keyboard_blocked();
        if sense.focusable && !keyboard_blocked {
            self.focus_order.push(id);
        }
        if self.active == Some(id) {
            self.active_seen = true;
        }
        if self.focused == Some(id) && !keyboard_blocked {
            self.focus_seen = true;
            if self.reveal_focus {
                self.reveal_focus = false;
                self.focus_reveal = self.innermost_scroll().map(|area| (area, rect));
            }
        }
        if interactive && hovered && self.input.pressed[0] && self.active.is_none() {
            self.active = Some(id);
            self.active_seen = true;
            self.pointer_claimed = true;
            if sense.focusable {
                self.focused = Some(id);
                self.focus_seen = true;
                self.focus_visible = false;
            }
        }
        let is_active = self.active == Some(id);
        let focused = self.focused == Some(id) && !keyboard_blocked;
        let mut clicked = sense.click && is_active && self.input.released[0] && in_rect;
        if focused
            && sense.keyboard_activate
            && (self.input.key_pressed(Key::Enter) || self.input.key_pressed(Key::Space))
        {
            clicked = true;
        }
        if hovered && interactive && !sense.drag {
            self.out.cursor = CursorIcon::Pointer;
        }
        if hovered {
            if self.hover_since.0 != Some(id) {
                self.hover_since = (Some(id), self.time);
            }
        } else if self.hover_since.0 == Some(id) {
            self.hover_since = (None, 0.0);
        }
        let resp = Response {
            id,
            rect,
            hovered,
            // Includes a press and release that both happened within this frame.
            pressed: is_active && (self.input.down[0] || self.input.pressed[0]),
            clicked,
            focused,
            changed: false,
            submitted: false,
        };
        self.last_response = resp;
        resp
    }

    /// True if the focus ring should be drawn (keyboard navigation).
    pub(crate) fn focus_ring_visible(&self, id: Id) -> bool {
        self.focus_visible && self.focused == Some(id)
    }

    /// Records inspection data for a widget when inspection or the inspector is on.
    pub(crate) fn record(&mut self, kind: &'static str, label: &str, r: &Response) {
        if self.inspection || self.tools.inspector {
            self.out.widgets.push(WidgetInfo {
                id: r.id,
                kind,
                label: label.to_owned(),
                rect: r.rect,
                clip: self.out.draw_list.clip(),
                hovered: r.hovered,
                focused: r.focused,
            });
        }
    }

    /// True when an item of height `h` placed next in the current layout would
    /// be entirely outside the clip rectangle (e.g. scrolled out of view).
    /// Widgets use it to skip text shaping, animation and painting.
    #[inline(always)]
    pub(crate) fn next_offscreen(&mut self, h: f32) -> bool {
        let clip = self.out.draw_list.clip();
        let (o, _) = self.top().peek();
        o.y > clip.max.y || o.y + h < clip.min.y
    }

    /// Reserves layout space for a focusable widget that is entirely
    /// off-screen, keeping it in the Tab order.
    pub(crate) fn skip_item(&mut self, id: Id, size: Vec2, fill: bool) -> Response {
        if self.tab_pressed && !self.keyboard_blocked() {
            self.focus_order.push(id);
        }
        let rect = self.allocate_offscreen(size, fill);
        Response { id, rect, ..Default::default() }
    }

    /// Fastest off-screen path: when no widget holds the pointer or keyboard
    /// focus, an item of height `h` that would land outside the clip rect is
    /// skipped before its id is even computed.
    #[inline]
    pub(crate) fn skip_offscreen_early(&mut self, h: f32, fill: bool) -> Option<Response> {
        if self.active.is_none() && self.focused.is_none() && self.next_offscreen(h) {
            let rect = self.allocate_offscreen(Vec2::new(0.0, h), fill);
            return Some(Response { rect, ..Default::default() });
        }
        None
    }

    /// The off-screen fast path is allowed unless the widget is being interacted with.
    pub(crate) fn can_skip(&self, id: Id) -> bool {
        self.active != Some(id) && self.focused != Some(id)
    }
}
