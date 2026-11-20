//! Rugi context, layout and widgets.
//!
//! # Model
//!
//! Rugi exposes an **immediate-mode API** (you call `ui.button("Save")` every
//! frame and get the interaction result back) on top of **retained per-widget
//! state** keyed by stable [`Id`]s: animation progress, scroll offsets, text
//! cursors and focus survive across frames without the application storing
//! them. Containers are expressed as explicit `begin_*` / [`Ui::end`] pairs
//! (which map 1:1 onto the C ABI) with closure-based sugar for Rust.
//!
//! # Frame lifecycle
//!
//! 1. Queue input with [`Ui::push_event`] (any time between frames).
//! 2. [`Ui::begin_frame`] consumes queued input.
//! 3. Build the UI with widget calls.
//! 4. [`Ui::end_frame`] returns the [`FrameOutput`] (draw list, atlas updates,
//!    repaint request) for a renderer.
//!
//! A `Ui` is single-threaded (`!Sync`); it may be moved between threads
//! between frames.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod animation;
mod demo;
mod error;
mod frame;
mod fx;
mod icons;
mod input_state;
mod interaction;
mod layout;
mod options;
mod popup;
mod scroll;
mod state;
mod style;
mod tools;
pub mod widgets;

pub use demo::DemoPage;
pub use error::UiError;
pub use fx::Slide;
pub use icons::{Icon, IconRef};
pub use interaction::Response;
pub use layout::Align;
pub use options::{AmbientRate, DisplayInfo, FrameInput, UiOptions};
pub use rugi_core::{
    self as core, Backdrop, BackdropKind, Color, CursorIcon, FontFamily, FontWeight, FrameOutput, FrameStats,
    GlassStyle, Id, Key, Modifiers, PointerButton, Rect, Repaint, TextStyle, TextureId, Vec2, WindowCommand,
};
pub use rugi_icons::lucide;
pub use rugi_text::FontError;
pub use rugi_text::{TextLayout, TextOptions};
pub use rugi_theme::Theme;
pub use tools::fuzzy_score;
pub use widgets::{Bind, BindMode, ButtonKind, KeyBind, TextInputOptions, TextRole};

use animation::{Channel, WidgetAnim};
use input_state::InputState;
use interaction::Sense;
use layout::Layout;
use rugi_core::draw::Slot;
use rugi_core::id::IdMap;
use rugi_core::{DrawList, InputEvent};
use rugi_text::TextSystem;
use std::any::Any;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Appended to text shortened to fit.
const ELLIPSIS: &str = "…";
/// Sub-pixel overflow that does not count as not fitting.
const FIT_TOLERANCE: f32 = 0.5;

/// A scroll area seen this frame: id, visible outer rect, nesting depth and
/// whether its content overflows.
type ScrollArea = (Id, Rect, u32, bool);

/// The UI context. See the [crate docs](crate).
pub struct Ui {
    /// Shared so widgets can hold the theme without borrowing `self`.
    theme: Rc<Theme>,
    /// Reusable buffer for number formatting.
    pub(crate) scratch: String,
    /// Reusable buffer for lazily formatted labels (`label_fmt`).
    pub(crate) fmt_buf: String,
    /// Reusable buffer for text shortened with an ellipsis.
    fit_buf: String,
    text: TextSystem,
    inspection: bool,

    events: Vec<InputEvent>,
    input: InputState,
    out: FrameOutput,
    pub(crate) last_stats: FrameStats,

    id_stack: Vec<Id>,
    anims: IdMap<Id, WidgetAnim>,
    states: IdMap<Id, Box<dyn Any>>,

    active: Option<Id>,
    active_seen: bool,
    focused: Option<Id>,
    focus_seen: bool,
    focus_visible: bool,
    focus_order: Vec<Id>,
    /// Tab was pressed this frame, so widgets skipped off-screen must still
    /// join `focus_order`.
    tab_pressed: bool,
    /// Keyboard navigation moved focus; the next frame scrolls it into view.
    reveal_focus: bool,
    /// Scroll area that must scroll to show the focused widget's rect.
    focus_reveal: Option<(Id, Rect)>,
    pointer_claimed: bool,

    frame: u64,
    time: f64,
    dt: f32,
    scale: f32,
    screen: Rect,
    in_frame: bool,
    build_start: Option<Instant>,

    layouts: Vec<Layout>,
    remaining: Rect,
    last_item: Rect,
    widget_count: u32,
    repaint: Repaint,
    misuse: Option<UiError>,
    next_width: Option<f32>,
    scroll_areas: Vec<ScrollArea>,
    prev_scroll_areas: Vec<ScrollArea>,
    wheel_target: Option<Id>,
    /// Themes saved by `push_style`.
    style_stack: Vec<Rc<Theme>>,
    /// Overlay layer (popups, menus, tooltips, dialogs) drawn above everything.
    pub(crate) overlay: DrawList,
    pub(crate) overlay_depth: u32,
    pub(crate) popups: popup::PopupState,
    /// Response of the last interactive widget (target of tooltips and context menus).
    pub(crate) last_response: Response,
    /// Hovered widget and the time it became hovered, for tooltip delays.
    pub(crate) hover_since: (Option<Id>, f64),
    pub(crate) display: DisplayInfo,
    /// Group id, frame, background slot and bar slot of the current nav group.
    pub(crate) nav_slots: Option<(Id, u64, Slot, Slot)>,
    /// Group id, frame and highlight slot of the current rail group.
    pub(crate) rail_slots: Option<(Id, u64, Slot)>,
    pub(crate) tools: tools::ToolsState,
    /// Centered rows begun this frame; numbers their retained heights.
    centered_rows: u32,
    /// An ambient animation (backdrop) wants frames this frame.
    pub(crate) ambient: bool,
    ambient_rate: AmbientRate,
    transparent_canvas: bool,
}

impl std::fmt::Debug for Ui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ui").field("frame", &self.frame).field("theme", &self.theme.name).finish_non_exhaustive()
    }
}

impl Ui {
    /// Creates a context.
    pub fn new(options: UiOptions) -> Self {
        Self {
            theme: Rc::new(options.theme),
            scratch: String::new(),
            fmt_buf: String::new(),
            fit_buf: String::new(),
            text: TextSystem::new(&options.text),
            inspection: options.inspection,
            events: Vec::new(),
            input: InputState::default(),
            out: FrameOutput::default(),
            last_stats: FrameStats::default(),
            id_stack: Vec::new(),
            anims: IdMap::default(),
            states: IdMap::default(),
            active: None,
            active_seen: false,
            focused: None,
            focus_seen: false,
            focus_visible: false,
            focus_order: Vec::new(),
            tab_pressed: false,
            reveal_focus: false,
            focus_reveal: None,
            pointer_claimed: false,
            frame: 0,
            time: 0.0,
            dt: frame::DEFAULT_DT,
            scale: 1.0,
            screen: Rect::ZERO,
            in_frame: false,
            build_start: None,
            layouts: Vec::new(),
            remaining: Rect::ZERO,
            last_item: Rect::ZERO,
            widget_count: 0,
            repaint: Repaint::Idle,
            misuse: None,
            next_width: None,
            scroll_areas: Vec::new(),
            prev_scroll_areas: Vec::new(),
            wheel_target: None,
            style_stack: Vec::new(),
            overlay: DrawList::new(),
            overlay_depth: 0,
            popups: popup::PopupState::default(),
            last_response: Response::default(),
            hover_since: (None, 0.0),
            display: DisplayInfo::default(),
            nav_slots: None,
            rail_slots: None,
            tools: tools::ToolsState::default(),
            centered_rows: 0,
            ambient: false,
            ambient_rate: AmbientRate::default(),
            transparent_canvas: false,
        }
    }

    /// Sets display/pacing information (called by the platform runner).
    pub fn set_display_info(&mut self, info: DisplayInfo) {
        self.display = info;
    }

    /// Display refresh rate and measured frame pacing.
    pub fn display_info(&self) -> DisplayInfo {
        self.display
    }

    /// Seconds since the previous frame.
    pub fn dt(&self) -> f32 {
        self.dt
    }

    /// Accumulated UI time in seconds.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Frame counter.
    pub fn frame_index(&self) -> u64 {
        self.frame
    }

    /// Physical pixels per logical pixel.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Logical screen rectangle.
    pub fn screen_rect(&self) -> Rect {
        self.screen
    }

    /// Statistics of the previous completed frame.
    pub fn last_frame_stats(&self) -> FrameStats {
        self.last_stats
    }

    /// Bounds of the most recently added item.
    pub fn last_item_rect(&self) -> Rect {
        self.last_item
    }

    /// Currently focused widget.
    pub fn focused_id(&self) -> Option<Id> {
        self.focused
    }

    /// Sets the frame-rate budget for ambient animation (animated backdrops).
    /// Default: full refresh while focused, 30 fps while unfocused.
    pub fn set_ambient_rate(&mut self, rate: AmbientRate) {
        self.ambient_rate = rate;
    }

    /// Current ambient animation budget.
    pub fn ambient_rate(&self) -> AmbientRate {
        self.ambient_rate
    }

    /// When `true`, frames start fully transparent instead of with the theme
    /// background, so empty pixels show what is behind the window. The window
    /// runner enables this for `AppOptions { transparent: true, .. }`.
    pub fn set_transparent_canvas(&mut self, on: bool) {
        self.transparent_canvas = on;
    }

    /// Requests frames for an ambient animation, subject to [`AmbientRate`].
    pub fn request_ambient_repaint(&mut self) {
        self.ambient = true;
    }

    /// Requests another frame as soon as possible.
    pub fn request_repaint(&mut self) {
        self.repaint = Repaint::Immediately;
    }

    /// Requests another frame after `delay` (capped at one hour).
    pub fn request_repaint_after(&mut self, delay: Duration) {
        self.repaint = self.repaint.merge(Repaint::After(delay.min(frame::MAX_REPAINT_DELAY)));
    }

    /// Puts `text` on the system clipboard at the end of the frame (the window
    /// runner applies it; host-driven apps read it via the C ABI).
    pub fn set_clipboard(&mut self, text: &str) {
        self.out.clipboard = Some(text.to_owned());
    }

    /// Asks the host to close the application after this frame.
    pub fn request_exit(&mut self) {
        self.out.exit_requested = true;
    }

    /// Direct access to the draw list for custom painting.
    pub fn painter(&mut self) -> &mut DrawList {
        &mut self.out.draw_list
    }

    /// Shapes text (cached across frames).
    pub fn layout_text(&mut self, text: &str, style: &TextStyle, wrap_width: Option<f32>) -> Rc<TextLayout> {
        self.text.layout(text, style, wrap_width)
    }

    /// Shapes single-line `text`, shortened with a trailing "…" when it is
    /// wider than `max_width`. Every widget that would otherwise cut text
    /// off at an edge uses this, so clipped text always ends in an ellipsis.
    pub fn layout_text_fit(&mut self, text: &str, style: &TextStyle, max_width: f32) -> Rc<TextLayout> {
        let full = self.text.layout(text, style, None);
        if full.size().x <= max_width + FIT_TOLERANCE || text.is_empty() {
            return full;
        }
        let ellipsis_w = self.text.layout(ELLIPSIS, style, None).size().x;
        let mut cut = full.index_at_x((max_width - ellipsis_w).max(0.0)).min(text.len());
        let mut buf = std::mem::take(&mut self.fit_buf);
        let fitted = loop {
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            buf.clear();
            buf.push_str(text[..cut].trim_end());
            buf.push_str(ELLIPSIS);
            let l = self.text.layout(&buf, style, None);
            if l.size().x <= max_width + FIT_TOLERANCE || cut == 0 {
                break l;
            }
            cut -= 1;
        };
        self.fit_buf = buf;
        fitted
    }

    /// Paints a shaped layout with its top-left at `pos`.
    pub fn paint_text(&mut self, layout: &TextLayout, pos: Vec2, color: Color) {
        self.text.paint(&mut self.out.draw_list, layout, pos, color, self.scale);
    }
}
