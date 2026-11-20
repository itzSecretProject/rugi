//! Argument marshalling: borrowed strings, pointers and caller-owned buffers.

use std::ffi::c_char;
use std::mem::{align_of, size_of};

use crate::error::{FfiError, RUGI_ERROR_INVALID_UTF8};
use crate::types::RugiStr;

/// Checks the parts of a `(ptr, count)` array that the caller's contract
/// cannot make true by itself: alignment and a total size that fits `isize`
/// (no real object is larger). A violation would be undefined behavior in
/// `slice::from_raw_parts`, so it is reported as `RUGI_ERROR_INVALID_ARGUMENT`.
pub(crate) fn check_array<T>(ptr: *const T, count: usize, what: &str) -> Result<(), FfiError> {
    if !ptr.is_aligned() {
        return Err(FfiError::invalid(format!("{what} is not aligned to {} bytes", align_of::<T>())));
    }
    match count.checked_mul(size_of::<T>()) {
        Some(bytes) if bytes <= isize::MAX as usize => Ok(()),
        _ => Err(FfiError::invalid(format!("{what}: length {count} is too large"))),
    }
}

/// Borrows `count` elements at `ptr`. NULL is allowed only when `count` is 0.
///
/// # Safety
/// If non-null, `ptr` must point to `count` readable elements for `'a`.
pub(crate) unsafe fn slice_arg<'a, T>(ptr: *const T, count: usize, what: &str) -> Result<&'a [T], FfiError> {
    if ptr.is_null() {
        return if count == 0 { Ok(&[]) } else { Err(FfiError::null(what)) };
    }
    check_array(ptr, count, what)?;
    // SAFETY: non-null, aligned, size checked; caller guarantees `count` readable elements.
    Ok(unsafe { std::slice::from_raw_parts(ptr, count) })
}

/// Borrows a `(ptr, len)` UTF-8 string. NULL with `len == 0` is "".
///
/// # Safety
/// If non-null, `ptr` must point to `len` readable bytes for the duration of the call.
pub(crate) unsafe fn str_arg<'a>(ptr: *const c_char, len: usize, what: &str) -> Result<&'a str, FfiError> {
    // SAFETY: forwarded contract.
    let bytes = unsafe { slice_arg(ptr as *const u8, len, what) }?;
    std::str::from_utf8(bytes).map_err(|_| FfiError::new(RUGI_ERROR_INVALID_UTF8, format!("{what} is not valid UTF-8")))
}

/// Borrows an array of `RugiStr`.
///
/// # Safety
/// `items` must point to `count` readable `RugiStr`s whose bytes stay valid for the call.
pub(crate) unsafe fn str_list<'a>(items: *const RugiStr, count: usize, what: &str) -> Result<Vec<&'a str>, FfiError> {
    // SAFETY: forwarded contract.
    let slice = unsafe { slice_arg(items, count, what) }?;
    // SAFETY: each element's (ptr, len) obeys the `str_arg` contract.
    slice.iter().map(|s| unsafe { str_arg(s.ptr, s.len, what) }).collect()
}

/// Borrows a required input pointer.
///
/// # Safety
/// `ptr` must be NULL or valid for reads for `'a`.
pub(crate) unsafe fn in_ref<'a, T>(ptr: *const T, what: &str) -> Result<&'a T, FfiError> {
    check_array(ptr, 1, what)?;
    // SAFETY: forwarded contract; alignment checked.
    unsafe { ptr.as_ref() }.ok_or_else(|| FfiError::null(what))
}

/// Borrows a required output (or in/out) pointer.
///
/// # Safety
/// `ptr` must be NULL or valid for reads and writes for `'a`, with no other
/// reference to the pointee alive.
pub(crate) unsafe fn out_mut<'a, T>(ptr: *mut T, what: &str) -> Result<&'a mut T, FfiError> {
    check_array(ptr, 1, what)?;
    // SAFETY: forwarded contract; alignment checked.
    unsafe { ptr.as_mut() }.ok_or_else(|| FfiError::null(what))
}

/// Borrows an optional output pointer (NULL is `None`).
///
/// # Safety
/// As [`out_mut`].
pub(crate) unsafe fn opt_out<'a, T>(ptr: *mut T, what: &str) -> Result<Option<&'a mut T>, FfiError> {
    if ptr.is_null() {
        return Ok(None);
    }
    // SAFETY: forwarded contract.
    unsafe { out_mut(ptr, what) }.map(Some)
}

/// Reads a C `bool` as a byte, so a non-canonical value (e.g. 2) is `true`
/// instead of an invalid Rust `bool`.
///
/// # Safety
/// `ptr` must point to a readable byte.
pub(crate) unsafe fn read_c_bool(ptr: *const bool) -> bool {
    // SAFETY: forwarded contract; any byte is a valid u8.
    unsafe { *(ptr as *const u8) != 0 }
}

/// Reads the leading `struct_size` bytes of a versioned descriptor into a
/// zero-initialized `T`, so a caller using an older (shorter) layout is never
/// read past its end. Returns the struct and its declared size.
///
/// # Safety
/// `ptr` must be NULL or point to a readable `u32` struct_size followed by at
/// least `min(struct_size, size_of::<T>())` readable bytes in total. `T` must
/// be valid when zeroed and start with a `u32` struct_size.
pub(crate) unsafe fn read_versioned<T: Copy>(ptr: *const T, what: &str) -> Result<Option<(T, usize)>, FfiError> {
    if ptr.is_null() {
        return Ok(None);
    }
    check_array(ptr as *const u32, 1, what)?;
    // SAFETY: non-null and aligned for the leading u32.
    let size = unsafe { *(ptr as *const u32) } as usize;
    // SAFETY: zeroed is valid for `T` (caller contract).
    let mut out: T = unsafe { std::mem::zeroed() };
    // SAFETY: the caller guarantees min(size, size_of::<T>()) readable bytes;
    // `out` is a distinct local of size_of::<T>() bytes.
    unsafe { std::ptr::copy_nonoverlapping(ptr as *const u8, &mut out as *mut T as *mut u8, size.min(size_of::<T>())) };
    Ok(Some((out, size)))
}

/// Calls `f` with the C `bool` at `ptr` and writes the result back.
///
/// The value is accessed as a byte, so a non-canonical C `bool` cannot become
/// an invalid Rust `bool`.
///
/// # Safety
/// `ptr` must be NULL or point to a readable and writable byte.
pub(crate) unsafe fn with_c_bool<R>(ptr: *mut bool, what: &str, f: impl FnOnce(&mut bool) -> R) -> Result<R, FfiError> {
    // SAFETY: forwarded contract.
    unsafe { with_c_bools(ptr, 1, what, |v| f(&mut v[0])) }
}

/// Calls `f` with `count` C `bool`s at `ptr` (copied as bytes, so
/// non-canonical values cannot become invalid Rust `bool`s) and writes them
/// back as 0/1. NULL is allowed only when `count` is 0.
///
/// # Safety
/// If non-null, `ptr` must point to `count` readable and writable bytes.
pub(crate) unsafe fn with_c_bools<R>(
    ptr: *mut bool,
    count: usize,
    what: &str,
    f: impl FnOnce(&mut [bool]) -> R,
) -> Result<R, FfiError> {
    // SAFETY: forwarded contract; viewed as bytes, any value is valid.
    let bytes = unsafe { slice_arg(ptr as *const u8, count, what) }?;
    let mut values: Vec<bool> = bytes.iter().map(|&b| b != 0).collect();
    let r = f(&mut values);
    for (i, v) in values.into_iter().enumerate() {
        // SAFETY: `i < count`; the caller guarantees the bytes are writable.
        unsafe { *(ptr as *mut u8).add(i) = v as u8 };
    }
    Ok(r)
}

/// Calls `f` with the `u32` index at `ptr` widened to `usize` and writes it back.
///
/// # Safety
/// `ptr` must be NULL or point to a readable and writable `u32`.
pub(crate) unsafe fn with_index<R>(ptr: *mut u32, what: &str, f: impl FnOnce(&mut usize) -> R) -> Result<R, FfiError> {
    // SAFETY: forwarded contract.
    let slot = unsafe { out_mut(ptr, what) }?;
    let mut i = *slot as usize;
    let r = f(&mut i);
    *slot = i as u32;
    Ok(r)
}

/// Writes `s` into `buf` as a NUL-terminated string, truncated at a character
/// boundary to fit. Does nothing if `buf` is NULL or `buf_size` is 0.
///
/// # Safety
/// `buf` must be NULL or point to `buf_size` writable bytes.
pub(crate) unsafe fn write_c_str(s: &str, buf: *mut c_char, buf_size: usize) {
    if buf.is_null() || buf_size == 0 {
        return;
    }
    let mut n = s.len().min(buf_size - 1);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    // SAFETY: caller guarantees `buf_size` writable bytes; n + 1 <= buf_size.
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf as *mut u8, n);
        *buf.add(n) = 0;
    }
}
