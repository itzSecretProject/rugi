//! Output produced by a UI frame and consumed by a platform/render layer.

use crate::{DrawList, Id, Rect};
use std::time::Duration;

/// When the UI wants the next frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repaint {
    /// Nothing is animating; wait for input (zero idle CPU).
    Idle,
    /// An animation is running; render the next frame as soon as possible.
    Immediately,
    /// Render again after the given delay (e.g. caret blink) unless input arrives first.
    After(Duration),
}

impl Repaint {
    /// Combines two requests, keeping the most urgent.
    pub fn merge(self, o: Repaint) -> Repaint {
        use Repaint::*;
        match (self, o) {
            (Immediately, _) | (_, Immediately) => Immediately,
            (After(a), After(b)) => After(a.min(b)),
            (After(a), Idle) | (Idle, After(a)) => After(a),
            (Idle, Idle) => Idle,
        }
    }
}

/// Mouse cursor shape requested by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorIcon {
    /// Default arrow.
    #[default]
    Default,
    /// Pointing hand (clickable).
    Pointer,
    /// I-beam (text).
    Text,
    /// Horizontal resize/drag.
    Grab,
}

/// A request from the UI to the window (custom title bars, borderless windows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCommand {
    /// Start an OS-level window move with the pressed pointer (title-bar drag).
    StartDrag,
    /// Minimize the window.
    Minimize,
    /// Toggle maximized state.
    ToggleMaximize,
    /// Close the window (same as the user closing it).
    Close,
}

/// A pending update of the glyph atlas texture (R8, single channel).
#[derive(Clone, Debug, Default)]
pub struct AtlasUpdate {
    /// Atlas width/height in pixels. If this differs from the renderer's
    /// current atlas, the texture must be recreated.
    pub size: u32,
    /// Changed region `[x, y, w, h]`.
    pub region: [u32; 4],
    /// Tightly packed R8 pixels for `region` (row-major, `w * h` bytes).
    pub data: Vec<u8>,
}

/// Per-frame statistics, for profiling overlays and benchmarks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// CPU time spent between `begin_frame` and `end_frame` (microseconds).
    pub build_us: u32,
    /// Number of GPU instances.
    pub instances: u32,
    /// Number of draw commands (scissor/texture batches).
    pub batches: u32,
    /// Primitives culled on the CPU because they were fully clipped.
    pub culled: u32,
    /// Interactive widgets processed.
    pub widgets: u32,
    /// Text layouts shaped this frame (cache misses).
    pub text_shaped: u32,
    /// Glyphs rasterized into the atlas this frame.
    pub glyphs_rasterized: u32,
}

/// Inspection record for one widget (AI/tooling inspection and tests).
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetInfo {
    /// Widget id.
    pub id: Id,
    /// Widget kind, e.g. `"button"`.
    pub kind: &'static str,
    /// Display label (may be empty).
    pub label: String,
    /// Bounds in logical pixels.
    pub rect: Rect,
    /// Clip rectangle in effect (the visible part is `rect ∩ clip`).
    pub clip: Rect,
    /// Hovered this frame.
    pub hovered: bool,
    /// Has keyboard focus.
    pub focused: bool,
}

/// Everything a frame produced.
#[derive(Debug, Default)]
pub struct FrameOutput {
    /// Primitives to render.
    pub draw_list: DrawList,
    /// Glyph atlas changes to upload before rendering, if any.
    pub atlas_update: Option<AtlasUpdate>,
    /// Desired repaint behaviour.
    pub repaint: Option<Repaint>,
    /// Requested cursor shape.
    pub cursor: CursorIcon,
    /// Frame statistics.
    pub stats: FrameStats,
    /// Widget inspection records (only when inspection is enabled).
    pub widgets: Vec<WidgetInfo>,
    /// The application asked to exit.
    pub exit_requested: bool,
    /// Text the UI wants placed on the system clipboard (copy/cut).
    pub clipboard: Option<String>,
    /// Window requests (drag, minimize, close) from custom window chrome.
    pub window_commands: Vec<WindowCommand>,
}

impl FrameOutput {
    /// Desired repaint behaviour (Idle if unset).
    pub fn repaint(&self) -> Repaint {
        self.repaint.unwrap_or(Repaint::Idle)
    }
}
