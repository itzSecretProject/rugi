//! Construction options and per-frame parameters supplied by the platform layer.

use rugi_core::Vec2;
use rugi_text::TextOptions;
use rugi_theme::Theme;

/// Frame-rate budget for ambient animation (animated backdrops), which would
/// otherwise keep the app redrawing at the display refresh rate. Interaction
/// and widget animations always run at the full rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbientRate {
    /// While the window has focus. `None` = the display refresh rate.
    pub focused: Option<f32>,
    /// While another window has focus (e.g. a settings panel over an application's own content).
    /// `None` = the display refresh rate.
    pub unfocused: Option<f32>,
}

impl Default for AmbientRate {
    fn default() -> Self {
        Self { focused: None, unfocused: Some(30.0) }
    }
}

/// Options for creating a [`Ui`](crate::Ui).
#[derive(Clone, Debug, Default)]
pub struct UiOptions {
    /// Initial theme.
    pub theme: Theme,
    /// Text system options.
    pub text: TextOptions,
    /// Record per-widget inspection data in [`FrameOutput::widgets`](crate::FrameOutput::widgets).
    pub inspection: bool,
}

/// Per-frame parameters supplied by the platform layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInput {
    /// Logical size of the drawable area.
    pub size: Vec2,
    /// Physical pixels per logical pixel.
    pub scale: f32,
    /// Seconds since the previous frame (clamped internally).
    pub dt: f32,
}

/// Display and frame-pacing information supplied by the platform layer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DisplayInfo {
    /// Refresh rate of the window's monitor, if the OS reports it.
    pub refresh_hz: Option<f32>,
    /// Mean presented-frame interval over the recent animated frames (ms).
    pub frame_interval_ms: f32,
    /// 99th-percentile presented-frame interval (ms).
    pub frame_interval_p99_ms: f32,
    /// Frames presented per second over the recent animated frames.
    pub fps: f32,
}
