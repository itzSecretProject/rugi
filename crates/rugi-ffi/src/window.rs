//! Custom window chrome for borderless windows.

use std::ffi::c_char;

use rugi_core::WindowCommand;

use crate::args::{out_mut, str_arg};
use crate::error::{guard, FfiError, RugiResult};
use crate::handle::{handle, RugiUi};
use crate::types::RugiRect;

/// Window request for rugi_window_command.
pub type RugiWindowCommand = u32;
/// Start moving the window with the pressed mouse button.
pub const RUGI_WINDOW_START_DRAG: RugiWindowCommand = 0;
/// Minimize the window.
pub const RUGI_WINDOW_MINIMIZE: RugiWindowCommand = 1;
/// Maximize the window, or restore it if maximized.
pub const RUGI_WINDOW_TOGGLE_MAXIMIZE: RugiWindowCommand = 2;
/// Close the window.
pub const RUGI_WINDOW_CLOSE: RugiWindowCommand = 3;

/// The window's content rectangle in logical pixels (valid during a frame).
#[no_mangle]
pub unsafe extern "C" fn rugi_screen_rect(ui: *mut RugiUi, out: *mut RugiRect) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`; `out` NULL or writable.
        let (h, o) = unsafe { (handle(ui)?, out_mut(out, "out")?) };
        *o = h.ui().screen_rect().into();
        Ok(())
    })
}

/// Custom title bar (title, minimize, close; the empty area drags the window).
#[no_mangle]
pub unsafe extern "C" fn rugi_title_bar(
    ui: *mut RugiUi,
    title: *const c_char,
    title_len: usize,
    height: f32,
) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, title) = unsafe { (handle(ui)?, str_arg(title, title_len, "title")?) };
        h.last = h.ui().title_bar(title, height.max(16.0));
        h.check()
    })
}

/// Queues a window request (RUGI_WINDOW_START_DRAG, _MINIMIZE, ...).
#[no_mangle]
pub unsafe extern "C" fn rugi_window_command(ui: *mut RugiUi, cmd: RugiWindowCommand) -> RugiResult {
    guard(|| {
        let cmd = match cmd {
            RUGI_WINDOW_START_DRAG => WindowCommand::StartDrag,
            RUGI_WINDOW_MINIMIZE => WindowCommand::Minimize,
            RUGI_WINDOW_TOGGLE_MAXIMIZE => WindowCommand::ToggleMaximize,
            RUGI_WINDOW_CLOSE => WindowCommand::Close,
            _ => return Err(FfiError::invalid(format!("unknown window command {cmd}"))),
        };
        // SAFETY: contract of `handle`.
        unsafe { handle(ui) }?.ui().window_command(cmd);
        Ok(())
    })
}
