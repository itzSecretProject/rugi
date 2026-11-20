//! User-editable key bindings.

use std::ffi::c_char;

use rugi_ui::{Bind, BindMode, KeyBind};

use crate::args::{check_array, read_c_bool, str_arg, write_c_str};
use crate::error::{guard_value, FfiError};
use crate::handle::{with_ui, RugiUi};
use crate::input::{button_code, button_from, mods_bits, mods_from, RugiModifiers};
use crate::keys::{key_code, key_from_code};

/// What a RugiKeyBind is bound to.
pub type RugiBindKind = u32;
/// Unbound.
pub const RUGI_BIND_NONE: RugiBindKind = 0;
/// `code` is a RUGI_KEY_* value.
pub const RUGI_BIND_KEY: RugiBindKind = 1;
/// `code` is a RUGI_MOUSE_* value.
pub const RUGI_BIND_MOUSE: RugiBindKind = 2;

/// Activation mode of a key binding.
pub type RugiBindMode = u32;
/// Active while held.
pub const RUGI_BIND_HOLD: RugiBindMode = 0;
/// Each press toggles.
pub const RUGI_BIND_TOGGLE: RugiBindMode = 1;
/// Always active.
pub const RUGI_BIND_ALWAYS: RugiBindMode = 2;

/// A user-editable key binding (zero-initialize for "unbound, hold").
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RugiKeyBind {
    /// RUGI_BIND_*.
    pub kind: RugiBindKind,
    /// Key or mouse code (see `kind`).
    pub code: u32,
    /// RUGI_MOD_* bits.
    pub mods: RugiModifiers,
    /// RUGI_BIND_HOLD / _TOGGLE / _ALWAYS.
    pub mode: RugiBindMode,
    /// Toggle state (RUGI_BIND_TOGGLE).
    pub toggled: bool,
}

/// Validates a caller's RugiKeyBind pointer and decodes it. Fields are read
/// through the raw pointer, `toggled` as a byte, so a non-canonical C `bool`
/// cannot become an invalid Rust `bool`.
///
/// # Safety
/// `p` must be NULL or point to a readable RugiKeyBind.
unsafe fn read_bind(p: *const RugiKeyBind, what: &str) -> Result<KeyBind, FfiError> {
    if p.is_null() {
        return Err(FfiError::null(what));
    }
    check_array(p, 1, what)?;
    // SAFETY: non-null, aligned and readable; each field is read by place,
    // without creating a reference to the whole struct.
    let b = unsafe {
        RugiKeyBind {
            kind: (*p).kind,
            code: (*p).code,
            mods: (*p).mods,
            mode: (*p).mode,
            toggled: read_c_bool(std::ptr::addr_of!((*p).toggled)),
        }
    };
    bind_from(&b)
}

fn bind_from(b: &RugiKeyBind) -> Result<KeyBind, FfiError> {
    let bind = match b.kind {
        RUGI_BIND_NONE => Bind::None,
        RUGI_BIND_KEY => Bind::Key(key_from_code(b.code)?),
        RUGI_BIND_MOUSE => Bind::Mouse(button_from(b.code)?),
        k => return Err(FfiError::invalid(format!("unknown bind kind {k}"))),
    };
    let mode = BindMode::from_u32(b.mode).ok_or_else(|| FfiError::invalid(format!("unknown bind mode {}", b.mode)))?;
    Ok(KeyBind { bind, mods: mods_from(b.mods), mode, toggled: b.toggled })
}

fn bind_to(k: &KeyBind) -> RugiKeyBind {
    let (kind, code) = match k.bind {
        Bind::None => (RUGI_BIND_NONE, 0),
        Bind::Key(key) => (RUGI_BIND_KEY, key_code(key)),
        Bind::Mouse(b) => (RUGI_BIND_MOUSE, button_code(b)),
    };
    RugiKeyBind { kind, code, mods: mods_bits(k.mods), mode: k.mode as u32, toggled: k.toggled }
}

/// Key-binding button (click, then press a key / mouse button; Esc cancels,
/// Backspace clears; right-click for Hold/Toggle/Always). Returns true on change.
#[no_mangle]
pub unsafe extern "C" fn rugi_keybind(
    ui: *mut RugiUi,
    label: *const c_char,
    label_len: usize,
    bind: *mut RugiKeyBind,
) -> bool {
    // SAFETY: contracts of `with_ui`, `str_arg` and `read_bind`; `bind` is writable.
    unsafe {
        with_ui(ui, |h| {
            let label = str_arg(label, label_len, "label")?;
            let mut kb = read_bind(bind, "bind")?;
            let r = h.ui().keybind(label, &mut kb);
            bind.write(bind_to(&kb));
            Ok(h.track(r).changed)
        })
    }
}

/// Evaluates a binding this frame (held / toggled / always). Updates `toggled`.
#[no_mangle]
pub unsafe extern "C" fn rugi_bind_active(ui: *mut RugiUi, bind: *mut RugiKeyBind) -> bool {
    // SAFETY: contracts of `with_ui` and `read_bind`; `bind` is writable.
    unsafe {
        with_ui(ui, |h| {
            let mut kb = read_bind(bind, "bind")?;
            let active = h.ui().bind_active(&mut kb);
            std::ptr::addr_of_mut!((*bind).toggled).write(kb.toggled);
            Ok(active)
        })
    }
}

/// Writes a binding's display label ("Ctrl+K", "Mouse 3", "None") as a
/// NUL-terminated string. Returns the full length in bytes (excluding NUL).
#[no_mangle]
pub unsafe extern "C" fn rugi_keybind_label(bind: *const RugiKeyBind, buf: *mut c_char, buf_size: usize) -> usize {
    guard_value(0, || {
        // SAFETY: `bind` NULL or readable.
        let b = unsafe { read_bind(bind, "bind") }?;
        let mut label = String::new();
        b.write_label(&mut label);
        // SAFETY: `buf` NULL or `buf_size` writable bytes.
        unsafe { write_c_str(&label, buf, buf_size) };
        Ok(label.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rugi_core::{Key, Modifiers};

    #[test]
    fn keybind_round_trips() {
        let kb = KeyBind {
            bind: Bind::Key(Key::F9),
            mods: Modifiers { ctrl: true, shift: true, ..Default::default() },
            mode: BindMode::Toggle,
            toggled: true,
        };
        assert_eq!(bind_from(&bind_to(&kb)).ok(), Some(kb));
        let bad = RugiKeyBind { kind: 9, ..Default::default() };
        assert!(bind_from(&bad).is_err());
    }
}
