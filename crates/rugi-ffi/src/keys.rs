//! Keyboard key codes.

use rugi_core::Key;

use crate::error::FfiError;

/// Keyboard key (RUGI_KEY_*). Digits are RUGI_KEY_NUM0 + n; F1–F11 are
/// RUGI_KEY_F1 + (n - 1); F12 is RUGI_KEY_F12.
pub type RugiKey = u32;
pub const RUGI_KEY_TAB: RugiKey = 0;
pub const RUGI_KEY_ENTER: RugiKey = 1;
pub const RUGI_KEY_ESCAPE: RugiKey = 2;
pub const RUGI_KEY_SPACE: RugiKey = 3;
pub const RUGI_KEY_BACKSPACE: RugiKey = 4;
pub const RUGI_KEY_DELETE: RugiKey = 5;
pub const RUGI_KEY_LEFT: RugiKey = 6;
pub const RUGI_KEY_RIGHT: RugiKey = 7;
pub const RUGI_KEY_UP: RugiKey = 8;
pub const RUGI_KEY_DOWN: RugiKey = 9;
pub const RUGI_KEY_HOME: RugiKey = 10;
pub const RUGI_KEY_END: RugiKey = 11;
pub const RUGI_KEY_A: RugiKey = 12;
pub const RUGI_KEY_C: RugiKey = 13;
pub const RUGI_KEY_V: RugiKey = 14;
pub const RUGI_KEY_X: RugiKey = 15;
pub const RUGI_KEY_K: RugiKey = 16;
pub const RUGI_KEY_I: RugiKey = 17;
pub const RUGI_KEY_F12: RugiKey = 18;
pub const RUGI_KEY_PAGE_UP: RugiKey = 19;
pub const RUGI_KEY_PAGE_DOWN: RugiKey = 20;
pub const RUGI_KEY_Y: RugiKey = 21;
pub const RUGI_KEY_Z: RugiKey = 22;
pub const RUGI_KEY_B: RugiKey = 23;
pub const RUGI_KEY_D: RugiKey = 24;
pub const RUGI_KEY_E: RugiKey = 25;
pub const RUGI_KEY_F: RugiKey = 26;
pub const RUGI_KEY_G: RugiKey = 27;
pub const RUGI_KEY_H: RugiKey = 28;
pub const RUGI_KEY_J: RugiKey = 29;
pub const RUGI_KEY_L: RugiKey = 30;
pub const RUGI_KEY_M: RugiKey = 31;
pub const RUGI_KEY_N: RugiKey = 32;
pub const RUGI_KEY_O: RugiKey = 33;
pub const RUGI_KEY_P: RugiKey = 34;
pub const RUGI_KEY_Q: RugiKey = 35;
pub const RUGI_KEY_R: RugiKey = 36;
pub const RUGI_KEY_S: RugiKey = 37;
pub const RUGI_KEY_T: RugiKey = 38;
pub const RUGI_KEY_U: RugiKey = 39;
pub const RUGI_KEY_W: RugiKey = 40;
/// Digit 0; digit n is RUGI_KEY_NUM0 + n.
pub const RUGI_KEY_NUM0: RugiKey = 41;
/// F1; F2–F11 follow consecutively.
pub const RUGI_KEY_F1: RugiKey = 51;
pub const RUGI_KEY_INSERT: RugiKey = 62;
pub const RUGI_KEY_SHIFT: RugiKey = 63;
pub const RUGI_KEY_CONTROL: RugiKey = 64;
pub const RUGI_KEY_ALT: RugiKey = 65;
pub const RUGI_KEY_CAPS_LOCK: RugiKey = 66;
/// Context-menu (application) key; Shift+F10 opens context menus too.
pub const RUGI_KEY_CONTEXT_MENU: RugiKey = 67;

/// Every key with its code.
const KEYS: [(RugiKey, Key); 68] = {
    use Key::*;
    [
        (RUGI_KEY_TAB, Tab),
        (RUGI_KEY_ENTER, Enter),
        (RUGI_KEY_ESCAPE, Escape),
        (RUGI_KEY_SPACE, Space),
        (RUGI_KEY_BACKSPACE, Backspace),
        (RUGI_KEY_DELETE, Delete),
        (RUGI_KEY_LEFT, ArrowLeft),
        (RUGI_KEY_RIGHT, ArrowRight),
        (RUGI_KEY_UP, ArrowUp),
        (RUGI_KEY_DOWN, ArrowDown),
        (RUGI_KEY_HOME, Home),
        (RUGI_KEY_END, End),
        (RUGI_KEY_A, A),
        (RUGI_KEY_C, C),
        (RUGI_KEY_V, V),
        (RUGI_KEY_X, X),
        (RUGI_KEY_K, K),
        (RUGI_KEY_I, I),
        (RUGI_KEY_F12, F12),
        (RUGI_KEY_PAGE_UP, PageUp),
        (RUGI_KEY_PAGE_DOWN, PageDown),
        (RUGI_KEY_Y, Y),
        (RUGI_KEY_Z, Z),
        (RUGI_KEY_B, B),
        (RUGI_KEY_D, D),
        (RUGI_KEY_E, E),
        (RUGI_KEY_F, F),
        (RUGI_KEY_G, G),
        (RUGI_KEY_H, H),
        (RUGI_KEY_J, J),
        (RUGI_KEY_L, L),
        (RUGI_KEY_M, M),
        (RUGI_KEY_N, N),
        (RUGI_KEY_O, O),
        (RUGI_KEY_P, P),
        (RUGI_KEY_Q, Q),
        (RUGI_KEY_R, R),
        (RUGI_KEY_S, S),
        (RUGI_KEY_T, T),
        (RUGI_KEY_U, U),
        (RUGI_KEY_W, W),
        (RUGI_KEY_NUM0, Num0),
        (RUGI_KEY_NUM0 + 1, Num1),
        (RUGI_KEY_NUM0 + 2, Num2),
        (RUGI_KEY_NUM0 + 3, Num3),
        (RUGI_KEY_NUM0 + 4, Num4),
        (RUGI_KEY_NUM0 + 5, Num5),
        (RUGI_KEY_NUM0 + 6, Num6),
        (RUGI_KEY_NUM0 + 7, Num7),
        (RUGI_KEY_NUM0 + 8, Num8),
        (RUGI_KEY_NUM0 + 9, Num9),
        (RUGI_KEY_F1, F1),
        (RUGI_KEY_F1 + 1, F2),
        (RUGI_KEY_F1 + 2, F3),
        (RUGI_KEY_F1 + 3, F4),
        (RUGI_KEY_F1 + 4, F5),
        (RUGI_KEY_F1 + 5, F6),
        (RUGI_KEY_F1 + 6, F7),
        (RUGI_KEY_F1 + 7, F8),
        (RUGI_KEY_F1 + 8, F9),
        (RUGI_KEY_F1 + 9, F10),
        (RUGI_KEY_F1 + 10, F11),
        (RUGI_KEY_INSERT, Insert),
        (RUGI_KEY_SHIFT, Shift),
        (RUGI_KEY_CONTROL, Control),
        (RUGI_KEY_ALT, Alt),
        (RUGI_KEY_CAPS_LOCK, CapsLock),
        (RUGI_KEY_CONTEXT_MENU, ContextMenu),
    ]
};

pub(crate) fn key_from_code(code: RugiKey) -> Result<Key, FfiError> {
    KEYS.iter()
        .find(|(c, _)| *c == code)
        .map(|(_, key)| *key)
        .ok_or_else(|| FfiError::invalid(format!("unknown key {code}")))
}

pub(crate) fn key_code(key: Key) -> RugiKey {
    KEYS.iter().find(|(_, k)| *k == key).map_or(u32::MAX, |(code, _)| *code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_are_a_bijection() {
        assert_eq!(KEYS.len(), Key::ALL.len());
        for (i, (code, key)) in KEYS.iter().enumerate() {
            assert_eq!(*code as usize, i);
            assert_eq!(key_code(*key), *code);
        }
        assert!(key_from_code(KEYS.len() as RugiKey).is_err());
    }
}
