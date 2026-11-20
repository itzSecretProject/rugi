//! Panels, containers, spacing and id scopes.

use std::ffi::c_char;

use rugi_ui::Align;

use crate::args::str_arg;
use crate::error::{guard, FfiError, RugiResult};
use crate::handle::{begin_container, handle, ui_call, with_handle, with_ui, RugiUi};
use crate::types::{rect_from, RugiRect};

/// Row alignment.
pub type RugiAlign = u32;
/// Left to right.
pub const RUGI_ALIGN_START: RugiAlign = 0;
/// Right to left from the right edge.
pub const RUGI_ALIGN_END: RugiAlign = 1;

/// Opens a panel docked to the left of the remaining window area. Root level only.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_side_panel(ui: *mut RugiUi, width: f32) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.begin_side_panel(width)) }
}

/// Opens a bar docked to the top of the remaining window area. Root level only.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_top_bar(ui: *mut RugiUi, height: f32) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.begin_top_bar(height)) }
}

/// Opens the central panel (fills the remaining area). Root level only.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_central_panel(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.begin_central_panel()) }
}

/// Opens a vertical group.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_vertical(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.begin_vertical()) }
}

/// Opens a horizontal row with RUGI_ALIGN_START or RUGI_ALIGN_END.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_horizontal(ui: *mut RugiUi, align: RugiAlign) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe {
        begin_container(ui, |h| {
            let align = match align {
                RUGI_ALIGN_START => Align::Start,
                RUGI_ALIGN_END => Align::End,
                _ => return Err(FfiError::invalid(format!("unknown align {align}"))),
            };
            h.ui().begin_horizontal(align);
            Ok(())
        })
    }
}

/// Opens an equal-width column grid.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_columns(ui: *mut RugiUi, columns: u32) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.begin_columns(columns)) }
}

/// Opens a card (raised surface sized to its content).
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_card(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.begin_card()) }
}

/// Opens a vertical scroll area. `height` <= 0 fills the remaining height;
/// a non-finite `height` is RUGI_ERROR_INVALID_ARGUMENT.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_scroll_area(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    height: f32,
) -> RugiResult {
    // SAFETY: contracts of `handle` and `str_arg`.
    unsafe {
        begin_container(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            if !height.is_finite() {
                return Err(FfiError::invalid("height must be finite"));
            }
            h.ui().begin_scroll_area(id, (height > 0.0).then_some(height));
            Ok(())
        })
    }
}

/// Starts a layout region at an explicit rectangle (no space taken in the
/// parent; content is clipped). Close with rugi_end.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_region(ui: *mut RugiUi, r: RugiRect, horizontal: bool) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe {
        begin_container(ui, |h| {
            h.ui().begin_region(rect_from(r)?, horizontal);
            Ok(())
        })
    }
}

/// Collapsible section. Returns true when its body should be built; then
/// add content and call rugi_end.
#[no_mangle]
pub unsafe extern "C" fn rugi_begin_collapsing(
    ui: *mut RugiUi,
    label: *const c_char,
    len: usize,
    default_open: u8,
) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe { with_ui(ui, |h| Ok(h.ui().begin_collapsing(str_arg(label, len, "label")?, default_open != 0))) }
}

/// Closes the innermost open container.
#[no_mangle]
pub unsafe extern "C" fn rugi_end(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.end()) }
}

/// Adds empty space along the layout direction.
#[no_mangle]
pub unsafe extern "C" fn rugi_space(ui: *mut RugiUi, amount: f32) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.space(amount)) }
}

/// Horizontal hairline.
#[no_mangle]
pub unsafe extern "C" fn rugi_separator(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.separator()) }
}

/// Fixes the width of the next item.
#[no_mangle]
pub unsafe extern "C" fn rugi_set_next_item_width(ui: *mut RugiUi, width: f32) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.set_next_item_width(width)) }
}

/// Pushes a string id scope.
#[no_mangle]
pub unsafe extern "C" fn rugi_push_id(ui: *mut RugiUi, id: *const c_char, id_len: usize) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, id) = unsafe { (handle(ui)?, str_arg(id, id_len, "id")?) };
        h.require_frame()?;
        h.ui().push_id(id);
        h.check()
    })
}

/// Pushes an integer id scope (e.g. list index).
#[no_mangle]
pub unsafe extern "C" fn rugi_push_id_int(ui: *mut RugiUi, value: u64) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe {
        with_handle(ui, |h| {
            h.require_frame()?;
            h.ui().push_id_u64(value);
            Ok(())
        })
    }
}

/// Pops an id scope.
#[no_mangle]
pub unsafe extern "C" fn rugi_pop_id(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe {
        with_handle(ui, |h| {
            h.require_frame()?;
            h.ui().pop_id();
            Ok(())
        })
    }
}
