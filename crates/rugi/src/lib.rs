//! Rugi — a GPU-accelerated immediate-mode UI engine.
//!
//! This facade re-exports the engine crates:
//!
//! | crate          | responsibility                                              |
//! |----------------|-------------------------------------------------------------|
//! | `rugi-core`   | geometry, colors, ids, input, draw list, output             |
//! | `rugi-text`   | shaping, layout cache, glyph atlas                          |
//! | `rugi-theme`  | design tokens and themes                                    |
//! | `rugi-ui`     | context, layout, widgets                                    |
//! | `rugi-render` | wgpu renderer                                               |
//! | `rugi-winit`  | native window runner and headless runner (feature `winit`)  |
//!
//! ```no_run
//! let mut checked = false;
//! rugi::run(rugi::AppOptions::default(), move |ui: &mut rugi::Ui| {
//!     ui.central_panel(|ui| {
//!         ui.title("Hello Rugi");
//!         if ui.button("Click me").clicked {
//!             checked = !checked;
//!         }
//!         ui.checkbox("Checked", &mut checked);
//!     });
//! })
//! .unwrap();
//! ```

#![forbid(unsafe_code)]

pub use rugi_core as core;
pub use rugi_render as render;
pub use rugi_text as text;
pub use rugi_theme as theme;
pub use rugi_ui::*;
#[cfg(feature = "winit")]
pub use rugi_winit::{run, winit, App, AppOptions, HeadlessRunner, InitContext, PresentPolicy, RunError, WinitInput};
/// The wgpu version Rugi renders with; use it to share a device with Rugi.
pub use wgpu;
