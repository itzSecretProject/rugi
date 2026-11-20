//! Input events and the clipboard for host-driven contexts.

use std::ffi::c_char;

use rugi_core::{InputEvent, Modifiers, PointerButton};

use crate::args::{str_arg, write_c_str};
use crate::error::{guard, guard_value, FfiError, RugiResult};
use crate::handle::{handle, RugiUi};
use crate::keys::{key_from_code, RugiKey};

/// Mouse button.
pub type RugiMouseButton = u32;
/// Left.
pub const RUGI_MOUSE_LEFT: RugiMouseButton = 0;
/// Right.
pub const RUGI_MOUSE_RIGHT: RugiMouseButton = 1;
/// Middle.
pub const RUGI_MOUSE_MIDDLE: RugiMouseButton = 2;

/// Modifier bit flags.
pub type RugiModifiers = u32;
/// Shift.
pub const RUGI_MOD_SHIFT: RugiModifiers = 1;
/// Control.
pub const RUGI_MOD_CTRL: RugiModifiers = 2;
/// Alt / Option.
pub const RUGI_MOD_ALT: RugiModifiers = 4;
/// Command / Windows / Super.
pub const RUGI_MOD_LOGO: RugiModifiers = 8;

pub(crate) fn button_from(b: RugiMouseButton) -> Result<PointerButton, FfiError> {
    match b {
        RUGI_MOUSE_LEFT => Ok(PointerButton::Primary),
        RUGI_MOUSE_RIGHT => Ok(PointerButton::Secondary),
        RUGI_MOUSE_MIDDLE => Ok(PointerButton::Middle),
        _ => Err(FfiError::invalid(format!("unknown mouse button {b}"))),
    }
}

pub(crate) fn button_code(b: PointerButton) -> RugiMouseButton {
    match b {
        PointerButton::Primary => RUGI_MOUSE_LEFT,
        PointerButton::Secondary => RUGI_MOUSE_RIGHT,
        PointerButton::Middle => RUGI_MOUSE_MIDDLE,
    }
}

pub(crate) fn mods_from(m: RugiModifiers) -> Modifiers {
    Modifiers {
        shift: m & RUGI_MOD_SHIFT != 0,
        ctrl: m & RUGI_MOD_CTRL != 0,
        alt: m & RUGI_MOD_ALT != 0,
        logo: m & RUGI_MOD_LOGO != 0,
    }
}

pub(crate) fn mods_bits(m: Modifiers) -> RugiModifiers {
    (m.shift as u32 * RUGI_MOD_SHIFT)
        | (m.ctrl as u32 * RUGI_MOD_CTRL)
        | (m.alt as u32 * RUGI_MOD_ALT)
        | (m.logo as u32 * RUGI_MOD_LOGO)
}

/// Queues `event` on the handle.
///
/// # Safety
/// See [`handle`].
unsafe fn push(ui: *mut RugiUi, event: InputEvent) -> Result<(), FfiError> {
    // SAFETY: forwarded contract.
    unsafe { handle(ui) }?.ui().push_event(event);
    Ok(())
}

/// Pointer moved to logical (x, y).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_mouse_move(ui: *mut RugiUi, x: f32, y: f32) -> RugiResult {
    // SAFETY: contract of `handle`.
    guard(|| unsafe { push(ui, InputEvent::PointerMoved { x, y }) })
}

/// Pointer left the window.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_mouse_leave(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    guard(|| unsafe { push(ui, InputEvent::PointerLeft) })
}

/// Mouse button pressed (pressed != 0) or released.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_mouse_button(ui: *mut RugiUi, button: RugiMouseButton, pressed: u8) -> RugiResult {
    guard(|| {
        let button = button_from(button)?;
        // SAFETY: contract of `handle`.
        unsafe { push(ui, InputEvent::PointerButton { button, pressed: pressed != 0 }) }
    })
}

/// Scroll by logical pixels (positive dy scrolls content down / up the page).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_scroll(ui: *mut RugiUi, dx: f32, dy: f32) -> RugiResult {
    // SAFETY: contract of `handle`.
    guard(|| unsafe { push(ui, InputEvent::Scroll { dx, dy }) })
}

/// Key pressed (pressed != 0) or released, with RUGI_MOD_* flags.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_key(
    ui: *mut RugiUi,
    key: RugiKey,
    pressed: u8,
    modifiers: RugiModifiers,
) -> RugiResult {
    guard(|| {
        let key = key_from_code(key)?;
        let event = InputEvent::Key { key, pressed: pressed != 0, modifiers: mods_from(modifiers) };
        // SAFETY: contract of `handle`.
        unsafe { push(ui, event) }
    })
}

/// Modifier state changed.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_modifiers(ui: *mut RugiUi, modifiers: RugiModifiers) -> RugiResult {
    // SAFETY: contract of `handle`.
    guard(|| unsafe { push(ui, InputEvent::ModifiersChanged(mods_from(modifiers))) })
}

/// Committed text input (UTF-8). Control characters are ignored.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_text(ui: *mut RugiUi, utf8: *const c_char, len: usize) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `str_arg` and `handle`.
        unsafe { push(ui, InputEvent::Text(str_arg(utf8, len, "text")?.to_owned())) }
    })
}

/// Queues pasted text for the focused text input (host-driven mode; the
/// window runner handles the system clipboard itself).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_paste(ui: *mut RugiUi, utf8: *const c_char, len: usize) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `str_arg` and `handle`.
        unsafe { push(ui, InputEvent::Paste(str_arg(utf8, len, "text")?.to_owned())) }
    })
}

/// Copies text the last frame wanted on the clipboard (copy/cut) into `buf`.
/// Returns the byte length + 1 needed, or 0 if nothing was copied.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_clipboard_text(ui: *mut RugiUi, buf: *mut c_char, buf_size: usize) -> usize {
    guard_value(0, || {
        // SAFETY: contract of `handle`.
        let h = unsafe { handle(ui) }?;
        let Some(text) = &h.ui().output().clipboard else { return Ok(0) };
        // SAFETY: `buf` is NULL or has `buf_size` writable bytes.
        unsafe { write_c_str(text, buf, buf_size) };
        Ok(text.len() + 1)
    })
}
