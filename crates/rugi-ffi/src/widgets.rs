//! Text, buttons, value editors, selectors and item state.

use std::ffi::c_char;

use rugi_core::{FontWeight, TextStyle};
use rugi_ui::{ButtonKind, Response, TextInputOptions, TextRole};

use crate::args::{check_array, out_mut, str_arg, str_list, with_c_bool, with_c_bools, with_index};
use crate::error::{guard, FfiError, RugiResult, RUGI_ERROR_INVALID_UTF8, RUGI_ERROR_NULL_POINTER};
use crate::fonts::{font_from, RugiFont};
use crate::handle::{handle, ui_call, with_ui, Handle, RugiUi};
use crate::icons::{icon_from, RugiIcon};
use crate::types::{RugiColor, RugiRect, RugiStr};

/// Button emphasis.
pub type RugiButtonKind = u32;
/// Accent-filled.
pub const RUGI_BUTTON_PRIMARY: RugiButtonKind = 0;
/// Neutral raised.
pub const RUGI_BUTTON_SECONDARY: RugiButtonKind = 1;
/// Text-only until hovered.
pub const RUGI_BUTTON_GHOST: RugiButtonKind = 2;
/// Destructive.
pub const RUGI_BUTTON_DANGER: RugiButtonKind = 3;

/// Text role.
pub type RugiTextRole = u32;
/// Body text.
pub const RUGI_TEXT_BODY: RugiTextRole = 0;
/// Secondary text.
pub const RUGI_TEXT_MUTED: RugiTextRole = 1;
/// Section heading.
pub const RUGI_TEXT_HEADING: RugiTextRole = 2;
/// Page title.
pub const RUGI_TEXT_TITLE: RugiTextRole = 3;
/// Caption.
pub const RUGI_TEXT_CAPTION: RugiTextRole = 4;
/// Large numeric display.
pub const RUGI_TEXT_DISPLAY: RugiTextRole = 5;

/// Text input flags for rugi_text_input_ex.
pub type RugiInputFlags = u32;
/// Mask characters and disable copy/cut.
pub const RUGI_INPUT_PASSWORD: RugiInputFlags = 1;

/// State of the most recent widget.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RugiItemState {
    /// Bounds.
    pub rect: RugiRect,
    /// Pointer over it.
    pub hovered: u8,
    /// Held down.
    pub pressed: u8,
    /// Activated this frame.
    pub clicked: u8,
    /// Has keyboard focus.
    pub focused: u8,
    /// Value changed this frame.
    pub changed: u8,
    /// Enter pressed (text inputs).
    pub submitted: u8,
}

impl From<Response> for RugiItemState {
    fn from(r: Response) -> RugiItemState {
        RugiItemState {
            rect: r.rect.into(),
            hovered: r.hovered as u8,
            pressed: r.pressed as u8,
            clicked: r.clicked as u8,
            focused: r.focused as u8,
            changed: r.changed as u8,
            submitted: r.submitted as u8,
        }
    }
}

fn text_role(role: RugiTextRole) -> Result<TextRole, FfiError> {
    TextRole::from_u32(role).ok_or_else(|| FfiError::invalid(format!("unknown text role {role}")))
}

/// Edits the caller's NUL-terminated buffer with a text input.
///
/// # Safety
/// `buf` must be NULL or point to `buf_size` readable and writable bytes.
unsafe fn edit_text_buffer(
    h: &mut Handle,
    id: &str,
    buf: *mut c_char,
    buf_size: usize,
    hint: &str,
    options: TextInputOptions<'_>,
) -> Result<Response, FfiError> {
    if buf.is_null() || buf_size == 0 {
        return Err(FfiError::new(RUGI_ERROR_NULL_POINTER, "buf is NULL or empty"));
    }
    check_array(buf, buf_size, "buf")?;
    // SAFETY: checked non-null; caller guarantees `buf_size` readable and writable bytes.
    let bytes = unsafe { std::slice::from_raw_parts_mut(buf as *mut u8, buf_size) };
    let nul = bytes
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| FfiError::invalid("buf is not NUL-terminated within buf_size"))?;
    let current = std::str::from_utf8(&bytes[..nul])
        .map_err(|_| FfiError::new(RUGI_ERROR_INVALID_UTF8, "buf is not valid UTF-8"))?;
    let mut text = std::mem::take(&mut h.scratch);
    text.clear();
    text.push_str(current);
    let r = h.ui().text_input_ex(id, &mut text, hint, options);
    if r.changed {
        let mut n = text.len().min(buf_size - 1);
        while !text.is_char_boundary(n) {
            n -= 1;
        }
        bytes[..n].copy_from_slice(&text.as_bytes()[..n]);
        bytes[n] = 0;
    }
    h.scratch = text;
    Ok(h.track(r))
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// Text in a RUGI_TEXT_* role.
#[no_mangle]
pub unsafe extern "C" fn rugi_text(ui: *mut RugiUi, text: *const c_char, len: usize, role: RugiTextRole) -> RugiResult {
    guard(|| {
        let role = text_role(role)?;
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, s) = unsafe { (handle(ui)?, str_arg(text, len, "text")?) };
        h.ui().text(s, role);
        h.check()
    })
}

/// Text with an explicit font, size (logical px), weight (100–900) and color.
#[no_mangle]
pub unsafe extern "C" fn rugi_text_ex(
    ui: *mut RugiUi,
    text: *const c_char,
    len: usize,
    font: RugiFont,
    size: f32,
    weight: u16,
    color: RugiColor,
) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, s) = unsafe { (handle(ui)?, str_arg(text, len, "text")?) };
        if !(size.is_finite() && size > 0.0) {
            return Err(FfiError::invalid("size must be > 0"));
        }
        let mut style = TextStyle::new(size).weight(FontWeight(weight.clamp(100, 900)));
        style.family = font_from(font);
        h.ui().text_styled(s, style, color.into(), false);
        h.check()
    })
}

/// Text shortened with "…" to fit the available width.
#[no_mangle]
pub unsafe extern "C" fn rugi_text_truncated(
    ui: *mut RugiUi,
    text: *const c_char,
    len: usize,
    role: RugiTextRole,
) -> RugiResult {
    guard(|| {
        let role = text_role(role)?;
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, s) = unsafe { (handle(ui)?, str_arg(text, len, "text")?) };
        h.ui().label_truncated(s, role, None);
        h.check()
    })
}

/// Text centered in a full-width row (RUGI_TEXT_* role).
#[no_mangle]
pub unsafe extern "C" fn rugi_text_centered(
    ui: *mut RugiUi,
    text: *const c_char,
    len: usize,
    role: RugiTextRole,
) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, s) = unsafe { (handle(ui)?, str_arg(text, len, "text")?) };
        h.ui().text_centered(s, text_role(role)?);
        h.check()
    })
}

/// Wrapped secondary paragraph.
#[no_mangle]
pub unsafe extern "C" fn rugi_paragraph(ui: *mut RugiUi, text: *const c_char, len: usize) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, s) = unsafe { (handle(ui)?, str_arg(text, len, "text")?) };
        h.ui().paragraph(s);
        h.check()
    })
}

/// Small colored pill.
#[no_mangle]
pub unsafe extern "C" fn rugi_badge(ui: *mut RugiUi, text: *const c_char, len: usize, color: RugiColor) -> RugiResult {
    guard(|| {
        // SAFETY: contracts of `handle` and `str_arg`.
        let (h, s) = unsafe { (handle(ui)?, str_arg(text, len, "text")?) };
        h.ui().badge(s, color.into());
        h.check()
    })
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

/// Button with optional icon (RUGI_ICON_NONE). Returns true when clicked.
/// Labels support "Text##id" to disambiguate identical labels.
#[no_mangle]
pub unsafe extern "C" fn rugi_button(
    ui: *mut RugiUi,
    label: *const c_char,
    len: usize,
    kind: RugiButtonKind,
    icon: RugiIcon,
) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe {
        with_ui(ui, |h| {
            let s = str_arg(label, len, "label")?;
            let kind = ButtonKind::from_u32(kind).ok_or_else(|| FfiError::invalid("unknown button kind"))?;
            let icon = icon_from(icon)?;
            let r = h.ui().button_ex(s, kind, icon.get());
            Ok(h.track(r).clicked)
        })
    }
}

/// Sidebar navigation item. Returns true when clicked.
#[no_mangle]
pub unsafe extern "C" fn rugi_nav_item(
    ui: *mut RugiUi,
    label: *const c_char,
    len: usize,
    icon: RugiIcon,
    selected: u8,
) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe {
        with_ui(ui, |h| {
            let s = str_arg(label, len, "label")?;
            let icon = icon_from(icon)?;
            let r = h.ui().nav_item(s, icon.get(), selected != 0);
            Ok(h.track(r).clicked)
        })
    }
}

/// Ghost icon button of `size`. Returns true when clicked.
#[no_mangle]
pub unsafe extern "C" fn rugi_icon_button(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    icon: RugiIcon,
    size: f32,
    tooltip: *const c_char,
    tooltip_len: usize,
) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            let tip = str_arg(tooltip, tooltip_len, "tooltip")?;
            let arg = icon_from(icon)?;
            let icon = arg.get().ok_or_else(|| FfiError::invalid("icon is RUGI_ICON_NONE"))?;
            let r = h.ui().icon_button(id, icon, size.max(8.0), tip);
            Ok(h.track(r).clicked)
        })
    }
}

/// Square icon-only rail item (shared gliding highlight). Returns true when clicked.
#[no_mangle]
pub unsafe extern "C" fn rugi_rail_item(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    icon: RugiIcon,
    selected: bool,
    tooltip: *const c_char,
    tooltip_len: usize,
) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            let tip = str_arg(tooltip, tooltip_len, "tooltip")?;
            let arg = icon_from(icon)?;
            let icon = arg.get().ok_or_else(|| FfiError::invalid("icon is RUGI_ICON_NONE"))?;
            let r = h.ui().rail_item(id, icon, selected, tip);
            Ok(h.track(r).clicked)
        })
    }
}

// ---------------------------------------------------------------------------
// Value editors
// ---------------------------------------------------------------------------

/// Checkbox bound to `*value`. Returns true when toggled this frame.
#[no_mangle]
pub unsafe extern "C" fn rugi_checkbox(ui: *mut RugiUi, label: *const c_char, len: usize, value: *mut bool) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `with_c_bool`.
    unsafe {
        with_ui(ui, |h| {
            let s = str_arg(label, len, "label")?;
            let r = with_c_bool(value, "value", |v| h.ui().checkbox(s, v))?;
            Ok(h.track(r).changed)
        })
    }
}

/// Switch bound to `*value`. Returns true when toggled this frame.
#[no_mangle]
pub unsafe extern "C" fn rugi_toggle(ui: *mut RugiUi, label: *const c_char, len: usize, value: *mut bool) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `with_c_bool`.
    unsafe {
        with_ui(ui, |h| {
            let s = str_arg(label, len, "label")?;
            let r = with_c_bool(value, "value", |v| h.ui().toggle(s, v))?;
            Ok(h.track(r).changed)
        })
    }
}

/// Radio button setting `*value = option` when chosen. Returns true on change.
#[no_mangle]
pub unsafe extern "C" fn rugi_radio(
    ui: *mut RugiUi,
    label: *const c_char,
    len: usize,
    value: *mut i32,
    option: i32,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `out_mut`.
    unsafe {
        with_ui(ui, |h| {
            let s = str_arg(label, len, "label")?;
            let v = out_mut(value, "value")?;
            let r = h.ui().radio(s, v, option);
            Ok(h.track(r).changed)
        })
    }
}

/// Slider bound to `*value` in [min, max]. Returns true when changed.
#[no_mangle]
pub unsafe extern "C" fn rugi_slider_float(
    ui: *mut RugiUi,
    label: *const c_char,
    len: usize,
    value: *mut f32,
    min: f32,
    max: f32,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `out_mut`.
    unsafe {
        with_ui(ui, |h| {
            let s = str_arg(label, len, "label")?;
            let v = out_mut(value, "value")?;
            if !(min.is_finite() && max.is_finite()) {
                return Err(FfiError::invalid("min/max must be finite"));
            }
            let r = h.ui().slider(s, v, min, max);
            Ok(h.track(r).changed)
        })
    }
}

/// Numeric field with steppers and drag. Returns true on change.
#[no_mangle]
pub unsafe extern "C" fn rugi_spin_box(
    ui: *mut RugiUi,
    label: *const c_char,
    len: usize,
    value: *mut f32,
    min: f32,
    max: f32,
    step: f32,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `out_mut`.
    unsafe {
        with_ui(ui, |h| {
            let s = str_arg(label, len, "label")?;
            let v = out_mut(value, "value")?;
            if !(min.is_finite() && max.is_finite() && step.is_finite()) {
                return Err(FfiError::invalid("min/max/step must be finite"));
            }
            let r = h.ui().spin_box(s, v, min, max, step);
            Ok(h.track(r).changed)
        })
    }
}

/// Single-line text input editing the caller-owned, NUL-terminated UTF-8
/// buffer `buf` of capacity `buf_size` bytes (including the NUL). Edits that
/// would overflow are truncated at a character boundary. Returns true when the
/// text changed this frame; query submission with rugi_last_item.
#[no_mangle]
pub unsafe extern "C" fn rugi_text_input(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    buf: *mut c_char,
    buf_size: usize,
    hint: *const c_char,
    hint_len: usize,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `edit_text_buffer`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            let hint = str_arg(hint, hint_len, "hint")?;
            Ok(edit_text_buffer(h, id, buf, buf_size, hint, TextInputOptions::default())?.changed)
        })
    }
}

/// rugi_text_input with flags (RUGI_INPUT_PASSWORD) and a leading icon
/// (RUGI_ICON_NONE for none). Returns true when the text changed.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rugi_text_input_ex(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    buf: *mut c_char,
    buf_size: usize,
    hint: *const c_char,
    hint_len: usize,
    flags: RugiInputFlags,
    icon: RugiIcon,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `edit_text_buffer`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            let hint = str_arg(hint, hint_len, "hint")?;
            let icon = icon_from(icon)?;
            let options = TextInputOptions { password: flags & RUGI_INPUT_PASSWORD != 0, icon: icon.get() };
            Ok(edit_text_buffer(h, id, buf, buf_size, hint, options)?.changed)
        })
    }
}

// ---------------------------------------------------------------------------
// Selectors
// ---------------------------------------------------------------------------

/// Drop-down selector. `*selected` indexes `items`. Returns true on change.
#[no_mangle]
pub unsafe extern "C" fn rugi_combo_box(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    selected: *mut u32,
    items: *const RugiStr,
    count: usize,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg`, `str_list` and `with_index`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            let list = str_list(items, count, "items")?;
            let r = with_index(selected, "selected", |s| h.ui().combo_box(id, s, &list))?;
            Ok(h.track(r).changed)
        })
    }
}

/// Tab bar. Returns true when the selected tab changed.
#[no_mangle]
pub unsafe extern "C" fn rugi_tabs(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    selected: *mut u32,
    labels: *const RugiStr,
    count: usize,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg`, `str_list` and `with_index`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            let list = str_list(labels, count, "labels")?;
            let r = with_index(selected, "selected", |s| h.ui().tabs(id, s, &list))?;
            Ok(h.track(r).changed)
        })
    }
}

/// Segmented control. Returns true when the selection changed.
#[no_mangle]
pub unsafe extern "C" fn rugi_segmented(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    selected: *mut u32,
    labels: *const RugiStr,
    count: usize,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg`, `str_list` and `with_index`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            let list = str_list(labels, count, "labels")?;
            let r = with_index(selected, "selected", |s| h.ui().segmented(id, s, &list))?;
            Ok(h.track(r).changed)
        })
    }
}

/// Selectable list row. Returns true when clicked.
#[no_mangle]
pub unsafe extern "C" fn rugi_selectable(ui: *mut RugiUi, label: *const c_char, len: usize, selected: u8) -> bool {
    // SAFETY: contracts of `with_ui` and `str_arg`.
    unsafe {
        with_ui(ui, |h| {
            let s = str_arg(label, len, "label")?;
            let r = h.ui().selectable(s, selected != 0);
            Ok(h.track(r).clicked)
        })
    }
}

/// Multi-select drop-down; `flags[i]` is item i's state. Returns true on change.
#[no_mangle]
pub unsafe extern "C" fn rugi_multi_select(
    ui: *mut RugiUi,
    label: *const c_char,
    label_len: usize,
    flags: *mut bool,
    items: *const RugiStr,
    count: usize,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg`, `str_list` and
    // `with_c_bools`; `flags` has `count` writable bools.
    unsafe {
        with_ui(ui, |h| {
            let label = str_arg(label, label_len, "label")?;
            let list = str_list(items, count, "items")?;
            let r = with_c_bools(flags, count, "flags", |f| h.ui().multi_select(label, f, &list))?;
            Ok(h.track(r).changed)
        })
    }
}

/// Page indicator dots. Returns true when a dot changed the page.
#[no_mangle]
pub unsafe extern "C" fn rugi_page_dots(
    ui: *mut RugiUi,
    id: *const c_char,
    id_len: usize,
    count: usize,
    index: *mut u32,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `with_index`.
    unsafe {
        with_ui(ui, |h| {
            let id = str_arg(id, id_len, "id")?;
            Ok(with_index(index, "index", |i| h.ui().page_dots(id, count, i))?.changed)
        })
    }
}

// ---------------------------------------------------------------------------
// Display and item state
// ---------------------------------------------------------------------------

/// Progress bar, `fraction` in [0, 1].
#[no_mangle]
pub unsafe extern "C" fn rugi_progress_bar(ui: *mut RugiUi, fraction: f32) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.progress_bar(fraction)) }
}

/// Interaction state of the most recent interactive widget.
#[no_mangle]
pub unsafe extern "C" fn rugi_last_item(ui: *mut RugiUi, out_state: *mut RugiItemState) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`; `out_state` NULL or writable.
        let (h, out) = unsafe { (handle(ui)?, out_mut(out_state, "out_state")?) };
        *out = h.last.into();
        Ok(())
    })
}
