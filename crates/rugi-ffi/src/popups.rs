//! Menus, context menus, tooltips and modal dialogs.

use std::ffi::c_char;

use crate::args::{str_arg, with_c_bool};
use crate::error::{guard, RugiResult};
use crate::handle::{handle, ui_call, with_ui, RugiUi};

/// Menu-bar button with a dropdown. Returns true while open; then add
/// rugi_menu_item calls and rugi_end.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_menu(ui: *mut RugiUi, label: *const c_char, len: usize) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe { with_ui(ui, |h| Ok(h.ui().begin_menu(str_arg(label, len, "label")?))) }
}

/// Right-click menu for the previous widget. Returns true while open; then
/// add items and call rugi_end.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_context_menu(ui: *mut RugiUi, id: *const c_char, len: usize) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe { with_ui(ui, |h| Ok(h.ui().begin_context_menu(str_arg(id, len, "id")?))) }
}

/// Menu entry with an optional shortcut hint. Returns true when chosen (closes the menu).
#[no_mangle]
pub unsafe extern "C" fn rugi_menu_item(
    ui: *mut RugiUi,
    label: *const c_char,
    len: usize,
    shortcut: *const c_char,
    shortcut_len: usize,
) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe {
        with_ui(ui, |h| {
            let r = h.ui().menu_item(str_arg(label, len, "label")?, str_arg(shortcut, shortcut_len, "shortcut")?);
            Ok(h.track(r).clicked)
        })
    }
}

/// Separator line inside a menu.
#[no_mangle]
pub unsafe extern "C" fn rugi_menu_separator(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.menu_separator()) }
}

/// Tooltip for the previous widget (shown after a short hover delay).
#[no_mangle]
pub unsafe extern "C" fn rugi_tooltip(ui: *mut RugiUi, text: *const c_char, len: usize) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, s) = unsafe { (handle(ui)?, str_arg(text, len, "text")?) };
        h.ui().tooltip(s);
        h.check()
    })
}

/// Modal dialog shown while `*open` is true; the close button and Escape set
/// it to false. Returns true when visible; then add content and rugi_end.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_modal(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    title: *const c_char,
    title_len: usize,
    open: *mut bool,
    width: f32,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `with_c_bool`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            let title = str_arg(title, title_len, "title")?;
            with_c_bool(open, "open", |o| h.ui().begin_modal(id, title, o, width))
        })
    }
}
