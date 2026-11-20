//! Rugi stable C ABI.
//!
//! cbindgen generates `include/rugi/rugi.h` from this crate; the doc comments
//! of exported items become the header's documentation.
//!
//! # Safety model
//!
//! * Every exported function is `extern "C"`, catches Rust panics with
//!   [`std::panic::catch_unwind`] and converts them into
//!   `RUGI_ERROR_PANIC`; unwinding never crosses into C/C++.
//! * Handles are opaque pointers to a private `Handle`. Every call looks the
//!   pointer up in a registry of live handles before dereferencing it, so
//!   NULL, destroyed and foreign pointers and calls from another thread are
//!   reported (`RUGI_ERROR_NULL_POINTER`, `_INVALID_ARGUMENT`,
//!   `_WRONG_THREAD`). A destroyed handle's address may be reused by a later
//!   rugi_ui_create, so keeping one is still a caller bug.
//! * Pointer and length arguments are checked for alignment and for sizes
//!   that no object can have (> `isize::MAX`) before slices are formed. C
//!   `bool`s are read as bytes. Versioned descriptors are read only up to
//!   their `struct_size`.
//! * Enumerations cross the ABI as `uint32_t` constants and are validated, so
//!   out-of-range values from C are reported as `RUGI_ERROR_INVALID_ARGUMENT`
//!   instead of becoming invalid Rust enums.
//! * Strings are `(const char* utf8, size_t len)` pairs borrowed for the call.
//!   No Rust `String`/`Vec` or Rust-layout type crosses the boundary.
//!
//! Each `unsafe` block documents the invariant it relies on.

#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::missing_safety_doc)] // safety contracts are documented in the generated header

mod args;
mod handle;

// Exported API. cbindgen emits declarations in module order, so the modules
// are listed (in blank-line groups that rustfmt keeps) in header order.
mod error;
mod types;

mod context;

mod app;

mod input;
mod keys;

mod style;

mod fonts;

mod layout;

mod icons;
mod widgets;

mod popups;

mod callbacks;

mod tools;

mod effects;
mod window;

mod keybind;

pub use app::*;
pub use callbacks::*;
pub use context::*;
pub use effects::*;
pub use error::*;
pub use fonts::*;
pub use handle::RugiUi;
pub use icons::*;
pub use input::*;
pub use keybind::*;
pub use keys::*;
pub use layout::*;
pub use popups::*;
pub use style::*;
pub use tools::*;
pub use types::*;
pub use widgets::*;
pub use window::*;

#[cfg(test)]
mod tests;
