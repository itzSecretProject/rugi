//! Widgets that call back into C for their content: table and carousel.
//!
//! The callback receives the caller's own `RugiUi*` and may call any widget
//! function on it while the widget holds the Ui mutably. To make that sound,
//! only raw pointers to the Handle are kept across the callback, and the
//! handle points at the Ui reborrow the widget passes in while C runs.

use std::ffi::{c_char, c_void};

use rugi_core::Vec2;
use rugi_ui::{Response, Ui};

use crate::args::{check_array, slice_arg, str_arg, str_list};
use crate::error::{guard_value, FfiError};
use crate::handle::{handle, Handle, RugiUi};
use crate::types::{RugiRect, RugiStr};

/// Table cell callback: build the content of cell (`row`, `col`) with
/// regular widget calls on `ui`. Must not unwind or longjmp.
pub type RugiTableCellFn =
    Option<unsafe extern "C" fn(ui: *mut RugiUi, row: usize, col: usize, user_data: *mut c_void)>;

/// Carousel card callback: paint card `index` into `rect` (e.g. with
/// rugi_backdrop_rect). `emphasis` is 1 for the centered card. Must not
/// unwind or longjmp.
pub type RugiCarouselCardFn =
    Option<unsafe extern "C" fn(ui: *mut RugiUi, index: usize, rect: RugiRect, emphasis: f32, user_data: *mut c_void)>;

/// A validated handle that C callbacks may re-enter.
struct Reentry {
    handle: *mut Handle,
    base: *mut Ui,
}

impl Reentry {
    /// # Safety
    /// See [`handle`]. The returned value must not outlive the call.
    unsafe fn new(ui: *mut RugiUi) -> Result<Reentry, FfiError> {
        // SAFETY: contract of `handle`; the reference ends immediately.
        let base = unsafe { handle(ui) }?.ui;
        Ok(Reentry { handle: ui as *mut Handle, base })
    }

    /// The Ui behind the handle.
    ///
    /// # Safety
    /// Must not be used while a callback runs (see [`Reentry::call`]).
    #[allow(clippy::mut_from_ref)]
    unsafe fn ui(&self) -> &mut Ui {
        // SAFETY: `base` is the live Ui behind the handle (see `Handle::ui`).
        unsafe { &mut *self.base }
    }

    /// Runs the C callback `f` with the handle pointing at `inner`.
    fn call(&self, inner: &mut Ui, f: impl FnOnce()) {
        // SAFETY: `handle` is the caller's live handle; rugi_ui_destroy
        // refuses to free it while `callbacks` is non-zero, so it stays valid
        // across `f`. While C runs, the handle points at `inner` (the reborrow
        // the widget passes in), so re-entrant calls derive from it and the
        // outer `&mut Ui` is not used meanwhile. Nested callbacks save and
        // restore the pointer in LIFO order.
        let saved = unsafe {
            let h = &mut *self.handle;
            h.callbacks += 1;
            std::mem::replace(&mut h.ui, inner as *mut Ui)
        };
        f();
        // SAFETY: as above.
        unsafe {
            let h = &mut *self.handle;
            h.ui = saved;
            h.callbacks -= 1;
        }
    }

    /// Records the widget's response and reports misuse it recorded.
    fn finish(&self, r: Response) -> Result<bool, FfiError> {
        // SAFETY: the handle is still valid and no callback is running;
        // short-lived reference.
        let h = unsafe { &mut *self.handle };
        h.last = r;
        h.check()?;
        Ok(r.changed)
    }
}

/// Virtualized table: `cell` is called only for visible rows. `widths` are
/// relative column weights (may be NULL for equal columns). `*selected` is the
/// selected row or -1. Returns true when the selection changed.
///
/// `cell` receives the same `ui` handle and may call any widget function on
/// it (including nested tables and carousels). rugi_ui_destroy and
/// begin/end frame return RUGI_ERROR_INVALID_STATE there. It must not unwind
/// or longjmp. A non-finite `height` or a negative or non-finite width is
/// RUGI_ERROR_INVALID_ARGUMENT.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rugi_table(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    headers: *const RugiStr,
    widths: *const f32,
    columns: usize,
    rows: usize,
    selected: *mut i64,
    height: f32,
    cell: RugiTableCellFn,
    user_data: *mut c_void,
) -> bool {
    guard_value(false, || {
        let cb = cell.ok_or_else(|| FfiError::null("cell callback"))?;
        // SAFETY: contract of `handle`.
        let re = unsafe { Reentry::new(ui) }?;
        // SAFETY: contracts of `str_arg` / `str_list`; `widths` NULL or `columns` floats.
        // The values are copied so the callback may modify the caller's arrays.
        let (id, headers, widths) = unsafe {
            (
                str_arg(id, id_len, "id")?,
                str_list(headers, columns, "headers")?,
                if widths.is_null() { Vec::new() } else { slice_arg(widths, columns, "widths")?.to_vec() },
            )
        };
        if !height.is_finite() || widths.iter().any(|w| !(w.is_finite() && *w >= 0.0)) {
            return Err(FfiError::invalid("height and widths must be finite, widths >= 0"));
        }
        // `selected` is read before and written after the table through the
        // raw pointer, so no reference to it is held while C runs.
        check_array(selected, 1, "selected")?;
        // SAFETY: `selected` is NULL or a readable, aligned i64.
        let mut sel = (!selected.is_null()).then(|| unsafe { *selected }).and_then(|s| usize::try_from(s).ok());
        // SAFETY: no callback is running yet; `re.call` scopes the re-entry.
        let u = unsafe { re.ui() };
        let r = u.table(id, &headers, &widths, rows, &mut sel, height.max(0.0), |inner, row, col| {
            // SAFETY: `cb` is the caller's C function pointer.
            re.call(inner, || unsafe { cb(ui, row, col, user_data) });
        });
        if !selected.is_null() {
            // SAFETY: checked above; writable per the contract.
            unsafe { *selected = sel.and_then(|v| i64::try_from(v).ok()).unwrap_or(-1) };
        }
        re.finish(r)
    })
}

/// Horizontal card carousel (`count` cards of card_w × card_h). Returns true
/// when `index` changed; rugi_last_item reports a click on the centered card.
/// `card` may re-enter Rugi with `ui` under the same rules as a table cell.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rugi_carousel(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    count: usize,
    index: *mut u32,
    card_w: f32,
    card_h: f32,
    card: RugiCarouselCardFn,
    user_data: *mut c_void,
) -> bool {
    guard_value(false, || {
        let cb = card.ok_or_else(|| FfiError::null("card callback"))?;
        // SAFETY: contract of `handle`.
        let re = unsafe { Reentry::new(ui) }?;
        // SAFETY: contract of `str_arg`.
        let id = unsafe { str_arg(id, id_len, "id") }?;
        if index.is_null() {
            return Err(FfiError::null("index"));
        }
        check_array(index, 1, "index")?;
        if !(card_w.is_finite() && card_h.is_finite() && card_w > 0.0 && card_h > 0.0) {
            return Err(FfiError::invalid("card size must be positive"));
        }
        // `index` is read before and written after the carousel through the
        // raw pointer, so no reference to it is held while C runs.
        // SAFETY: checked non-null and aligned; readable and writable per the contract.
        let mut i = unsafe { *index } as usize;
        // SAFETY: no callback is running yet; `re.call` scopes the re-entry.
        let u = unsafe { re.ui() };
        let r = u.carousel(id, count, &mut i, Vec2::new(card_w, card_h), |inner, n, rect, emphasis| {
            let rect = RugiRect::from(rect);
            // SAFETY: `cb` is the caller's C function pointer.
            re.call(inner, || unsafe { cb(ui, n, rect, emphasis, user_data) });
        });
        // SAFETY: as above.
        unsafe { *index = u32::try_from(i).unwrap_or(u32::MAX) };
        re.finish(r)
    })
}
