//! Rugi desktop runner.
//!
//! Creates a native window (winit), a wgpu device and surface, translates OS
//! input into [`rugi_core::InputEvent`]s and drives frames on demand:
//!
//! * **Event-driven**: the loop sleeps (`ControlFlow::Wait`) until input
//!   arrives or the UI asks for a repaint.
//! * **Input-to-frame sync**: all events received before a redraw are applied
//!   to that frame; press+release pairs inside one frame still register as
//!   clicks.
//! * **DPI-aware**: logical coordinates throughout; the renderer receives the
//!   window scale factor and text is rasterized at physical resolution.
//!
//! [`HeadlessRunner`] renders the same UI offscreen (no window) for
//! screenshots, visual tests and GPU benchmarks.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod headless;
mod input;
mod keymap;
mod pacing;
mod runner;

pub use headless::HeadlessRunner;
pub use input::WinitInput;
use rugi_core::TextureId;
pub use rugi_render::{Renderer, Viewport};
pub use rugi_ui::DisplayInfo;
use rugi_ui::{Ui, UiOptions};
use runner::Runner;
pub use winit;
use winit::event_loop::{ControlFlow, EventLoop};

/// Errors from the runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// The event loop could not be created or failed (e.g. created twice).
    EventLoop(String),
    /// The window could not be created.
    Window(String),
    /// No suitable GPU adapter / device / surface.
    Graphics(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::EventLoop(e) => write!(f, "event loop error: {e}"),
            RunError::Window(e) => write!(f, "window error: {e}"),
            RunError::Graphics(e) => write!(f, "graphics error: {e}"),
        }
    }
}

impl std::error::Error for RunError {}

/// How frames are presented to the display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PresentPolicy {
    /// Synchronized to the display refresh rate.
    #[default]
    Vsync,
    /// Lowest input latency without tearing: mailbox presentation when the
    /// platform supports it, and at most one frame queued.
    LowLatency,
    /// Present as fast as possible (for benchmarking; may tear).
    Uncapped,
}

/// Window and runtime options.
#[derive(Clone, Debug)]
pub struct AppOptions {
    /// Window title.
    pub title: String,
    /// Initial inner size in logical pixels.
    pub size: [u32; 2],
    /// Minimum inner size in logical pixels.
    pub min_size: [u32; 2],
    /// Presentation policy.
    pub present: PresentPolicy,
    /// UI context options.
    pub ui: UiOptions,
    /// Native title bar and border. Set to `false` for custom window chrome
    /// (see `Ui::window_chrome`), which drags/minimizes/closes the window.
    pub decorations: bool,
    /// Transparent window background: pixels the UI leaves transparent show
    /// the desktop (rounded custom windows, overlays). Requires a compositor
    /// that supports it; otherwise the window is opaque.
    pub transparent: bool,
    /// The user can resize the window.
    pub resizable: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            title: "Rugi".into(),
            size: [1280, 800],
            min_size: [640, 400],
            present: PresentPolicy::Vsync,
            ui: UiOptions::default(),
            decorations: true,
            transparent: false,
            resizable: true,
        }
    }
}

/// Access to GPU resources during application initialization.
pub struct InitContext<'a> {
    /// The UI context.
    pub ui: &'a mut Ui,
    renderer: &'a mut Renderer,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    adapter_info: &'a wgpu::AdapterInfo,
}

impl InitContext<'_> {
    /// Uploads an RGBA8 image for use with [`Ui::image`].
    pub fn register_texture(&mut self, width: u32, height: u32, rgba: &[u8]) -> TextureId {
        self.renderer.register_texture(self.device, self.queue, width, height, rgba)
    }

    /// The GPU adapter the UI renders with (name, backend, driver).
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        self.adapter_info
    }
}

/// An application driven by the runner.
pub trait App {
    /// Called once after the window and GPU are ready.
    fn init(&mut self, _ctx: &mut InitContext<'_>) {}
    /// Builds one frame.
    fn frame(&mut self, ui: &mut Ui);
}

impl<F: FnMut(&mut Ui)> App for F {
    fn frame(&mut self, ui: &mut Ui) {
        self(ui)
    }
}

/// Runs `app` in a new native window until it is closed or the UI requests
/// exit. Must be called on the main thread. Most platforms allow only one
/// event loop per process, so call this at most once.
pub fn run<A: App + 'static>(options: AppOptions, app: A) -> Result<(), RunError> {
    let event_loop = EventLoop::new().map_err(|e| RunError::EventLoop(e.to_string()))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut runner = Runner::new(options, app);
    event_loop.run_app(&mut runner).map_err(|e| RunError::EventLoop(e.to_string()))?;
    runner.take_error().map_or(Ok(()), Err)
}
