//! Result codes, the thread-local last error and the panic guard.

use std::cell::RefCell;
use std::ffi::c_char;
use std::fmt::Display;
use std::panic::{catch_unwind, AssertUnwindSafe};

use rugi_ui::UiError;

use crate::args::write_c_str;

/// Status code returned by fallible functions.
pub type RugiResult = i32;
/// Success.
pub const RUGI_OK: RugiResult = 0;
/// A required pointer argument was NULL.
pub const RUGI_ERROR_NULL_POINTER: RugiResult = 1;
/// An argument was out of range or malformed.
pub const RUGI_ERROR_INVALID_ARGUMENT: RugiResult = 2;
/// A string argument was not valid UTF-8.
pub const RUGI_ERROR_INVALID_UTF8: RugiResult = 3;
/// The call is not valid in the current state (e.g. widget outside a frame).
pub const RUGI_ERROR_INVALID_STATE: RugiResult = 4;
/// GPU adapter/device/surface failure.
pub const RUGI_ERROR_GRAPHICS: RugiResult = 5;
/// Window or event loop failure.
pub const RUGI_ERROR_WINDOW: RugiResult = 6;
/// An internal panic was caught. The handle should be destroyed.
pub const RUGI_ERROR_PANIC: RugiResult = 7;
/// The handle was used from a thread other than the one that created it.
pub const RUGI_ERROR_WRONG_THREAD: RugiResult = 8;
/// The caller's struct_size does not match a supported layout.
pub const RUGI_ERROR_VERSION_MISMATCH: RugiResult = 9;

/// An error on its way to the caller: a result code and a message.
pub(crate) struct FfiError {
    code: RugiResult,
    message: String,
}

impl FfiError {
    pub(crate) fn new(code: RugiResult, message: impl Into<String>) -> Self {
        FfiError { code, message: message.into() }
    }

    /// `RUGI_ERROR_NULL_POINTER`: "`what` is NULL".
    pub(crate) fn null(what: &str) -> Self {
        Self::new(RUGI_ERROR_NULL_POINTER, format!("{what} is NULL"))
    }

    /// `RUGI_ERROR_INVALID_ARGUMENT`.
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::new(RUGI_ERROR_INVALID_ARGUMENT, message)
    }

    /// `RUGI_ERROR_INVALID_STATE`.
    pub(crate) fn state(error: impl Display) -> Self {
        Self::new(RUGI_ERROR_INVALID_STATE, error.to_string())
    }
}

impl From<UiError> for FfiError {
    /// Misuse detected by the engine: an invalid argument it rejected is
    /// `RUGI_ERROR_INVALID_ARGUMENT`, everything else `RUGI_ERROR_INVALID_STATE`.
    fn from(e: UiError) -> Self {
        match e {
            UiError::InvalidArgument(_) => Self::invalid(e.to_string()),
            _ => Self::state(e),
        }
    }
}

thread_local! {
    static LAST_ERROR: RefCell<Option<(RugiResult, String)>> = const { RefCell::new(None) };
}

/// Records the last error. During thread teardown, after the thread-local is
/// destroyed, the error is dropped (the result code is still returned).
fn set_last_error(code: RugiResult, message: String) {
    let _ = LAST_ERROR.try_with(|e| *e.borrow_mut() = Some((code, message)));
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".into()
    }
}

/// Runs `f`, converting errors and panics into a result code and the last error.
pub(crate) fn guard(f: impl FnOnce() -> Result<(), FfiError>) -> RugiResult {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => RUGI_OK,
        Ok(Err(FfiError { code, message })) => {
            set_last_error(code, message);
            code
        }
        Err(p) => {
            set_last_error(RUGI_ERROR_PANIC, format!("internal panic: {}", panic_message(&*p)));
            RUGI_ERROR_PANIC
        }
    }
}

/// [`guard`] for functions that return a value: `fallback` on error or panic.
pub(crate) fn guard_value<R>(fallback: R, f: impl FnOnce() -> Result<R, FfiError>) -> R {
    let mut out = None;
    guard(|| {
        out = Some(f()?);
        Ok(())
    });
    out.unwrap_or(fallback)
}

/// Code of the last error on this thread (RUGI_OK if none).
#[no_mangle]
pub extern "C" fn rugi_last_error_code() -> RugiResult {
    LAST_ERROR.try_with(|e| e.borrow().as_ref().map(|(c, _)| *c).unwrap_or(RUGI_OK)).unwrap_or(RUGI_OK)
}

/// Copies the last error message on this thread into `buf` (NUL-terminated,
/// truncated to `buf_size`). Returns the full message length + 1 (the buffer
/// size needed), or 0 if there is no error. `buf` may be NULL to query size.
#[no_mangle]
pub unsafe extern "C" fn rugi_last_error_message(buf: *mut c_char, buf_size: usize) -> usize {
    LAST_ERROR
        .try_with(|e| {
            let e = e.borrow();
            let Some((_, msg)) = e.as_ref() else { return 0 };
            // SAFETY: caller guarantees `buf` is NULL or has `buf_size` writable bytes.
            unsafe { write_c_str(msg, buf, buf_size) };
            msg.len() + 1
        })
        .unwrap_or(0)
}

/// Clears the last error on this thread.
#[no_mangle]
pub extern "C" fn rugi_clear_error() {
    let _ = LAST_ERROR.try_with(|e| *e.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_errors_map_to_documented_codes() {
        // rugi_ui_begin_frame forwards Ui::begin_frame errors with `?`.
        assert_eq!(FfiError::from(UiError::InvalidArgument("size must be finite")).code, RUGI_ERROR_INVALID_ARGUMENT);
        assert_eq!(
            FfiError::from(UiError::InvalidArgument("scale must be positive")).code,
            RUGI_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(FfiError::from(UiError::FrameInProgress).code, RUGI_ERROR_INVALID_STATE);
        assert_eq!(FfiError::from(UiError::NoFrame).code, RUGI_ERROR_INVALID_STATE);
    }
}
