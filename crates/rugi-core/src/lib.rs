//! Rugi core types.
//!
//! This crate is deliberately GPU-free and windowing-free: it holds the data
//! model shared by every other Rugi crate (geometry, colors, widget ids,
//! input events, the draw list consumed by renderers, and frame output).
//! Everything here is plain data and is testable headlessly.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod anim;
pub mod color;
pub mod draw;
pub mod fmt;
pub mod fx;
pub mod geom;
pub mod id;
pub mod input;
pub mod output;
pub mod text_style;

pub use color::Color;
pub use draw::{DrawCommand, DrawList, Instance, PrimitiveKind, TextureId};
pub use fx::{Backdrop, BackdropKind, GlassStyle};
pub use geom::{Rect, Vec2};
pub use id::Id;
pub use input::{InputEvent, Key, Modifiers, PointerButton};
pub use output::{AtlasUpdate, CursorIcon, FrameOutput, FrameStats, Repaint, WidgetInfo, WindowCommand};
pub use text_style::{FontFamily, FontWeight, TextStyle};
