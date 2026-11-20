//! Windowed applications: rugi_app_run.

use std::ffi::{c_char, c_void, CStr};

use rugi_ui::Ui;
use rugi_winit::{AppOptions, PresentPolicy, RunError};

use crate::args::read_versioned;
use crate::error::{
    guard, FfiError, RugiResult, RUGI_ERROR_GRAPHICS, RUGI_ERROR_INVALID_UTF8, RUGI_ERROR_VERSION_MISMATCH,
    RUGI_ERROR_WINDOW, RUGI_ERROR_WRONG_THREAD,
};
use crate::handle::{register, unregister, Handle, RugiUi};
use crate::style::{theme_from, RugiTheme};

/// Presentation policy for rugi_app_run.
pub type RugiPresent = u32;
/// Synchronized to the monitor refresh (60/120/144 Hz…). Default.
pub const RUGI_PRESENT_VSYNC: RugiPresent = 0;
/// Lowest latency without tearing (mailbox when available, 1 frame queued).
pub const RUGI_PRESENT_LOW_LATENCY: RugiPresent = 1;
/// As fast as possible (benchmarks; may tear).
pub const RUGI_PRESENT_UNCAPPED: RugiPresent = 2;

/// Bit flags for RugiAppDesc.window_flags.
pub type RugiWindowFlags = u32;
/// No native title bar or border (draw your own with rugi_title_bar).
pub const RUGI_WINDOW_BORDERLESS: RugiWindowFlags = 1;
/// Transparent window background (rounded / shaped windows).
pub const RUGI_WINDOW_TRANSPARENT: RugiWindowFlags = 2;
/// The user cannot resize the window.
pub const RUGI_WINDOW_FIXED_SIZE: RugiWindowFlags = 4;

const KNOWN_WINDOW_FLAGS: RugiWindowFlags = RUGI_WINDOW_BORDERLESS | RUGI_WINDOW_TRANSPARENT | RUGI_WINDOW_FIXED_SIZE;

/// Options for rugi_app_run. Set struct_size = sizeof(RugiAppDesc).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RugiAppDesc {
    /// Must equal sizeof(RugiAppDesc), or offsetof(RugiAppDesc, window_flags)
    /// to omit `window_flags`.
    pub struct_size: u32,
    /// NUL-terminated UTF-8 window title (may be NULL).
    pub title: *const c_char,
    /// Initial logical width.
    pub width: u32,
    /// Initial logical height.
    pub height: u32,
    /// RUGI_THEME_*.
    pub theme: RugiTheme,
    /// RUGI_PRESENT_*: presentation policy.
    pub present: RugiPresent,
    /// RUGI_WINDOW_* bit flags.
    pub window_flags: RugiWindowFlags,
}

/// Size of a RugiAppDesc that ends before `window_flags`.
const APP_DESC_SIZE_WITHOUT_FLAGS: usize = std::mem::offset_of!(RugiAppDesc, window_flags);

/// Called once per frame by rugi_app_run between begin/end frame. `ui` is
/// valid only for the duration of the call. Must not unwind or longjmp.
pub type RugiFrameCallback = Option<unsafe extern "C" fn(ui: *mut RugiUi, user_data: *mut c_void)>;

/// Applies a caller-supplied RugiAppDesc of `size` bytes to `options`
/// (fields past `size` are zero).
///
/// # Safety
/// `d.title` must be NULL or a NUL-terminated string valid for the call.
unsafe fn apply_desc(options: &mut AppOptions, d: &RugiAppDesc, size: usize) -> Result<(), FfiError> {
    if size != std::mem::size_of::<RugiAppDesc>() && size != APP_DESC_SIZE_WITHOUT_FLAGS {
        return Err(FfiError::new(RUGI_ERROR_VERSION_MISMATCH, "RugiAppDesc.struct_size mismatch"));
    }
    if size == std::mem::size_of::<RugiAppDesc>() {
        let f = d.window_flags;
        if f & !KNOWN_WINDOW_FLAGS != 0 {
            return Err(FfiError::invalid(format!("unknown window flags {f:#x}")));
        }
        options.decorations = f & RUGI_WINDOW_BORDERLESS == 0;
        options.transparent = f & RUGI_WINDOW_TRANSPARENT != 0;
        options.resizable = f & RUGI_WINDOW_FIXED_SIZE == 0;
    }
    if !d.title.is_null() {
        // SAFETY: non-null and NUL-terminated per the contract above.
        let t = unsafe { CStr::from_ptr(d.title) };
        options.title =
            t.to_str().map_err(|_| FfiError::new(RUGI_ERROR_INVALID_UTF8, "title is not UTF-8"))?.to_owned();
    }
    if d.width > 0 && d.height > 0 {
        options.size = [d.width, d.height];
    }
    options.ui.theme = theme_from(d.theme)?;
    options.present = match d.present {
        RUGI_PRESENT_VSYNC => PresentPolicy::Vsync,
        RUGI_PRESENT_LOW_LATENCY => PresentPolicy::LowLatency,
        RUGI_PRESENT_UNCAPPED => PresentPolicy::Uncapped,
        other => return Err(FfiError::invalid(format!("unknown present policy {other}"))),
    };
    Ok(())
}

/// The Handle passed to frame callbacks; unregistered and freed on drop
/// (also when rugi_winit::run panics).
struct FrameHandle(*mut Handle);

impl FrameHandle {
    fn new() -> FrameHandle {
        let h = Box::into_raw(Box::new(Handle::new(std::ptr::null_mut(), false)));
        register(h);
        FrameHandle(h)
    }
}

impl Drop for FrameHandle {
    fn drop(&mut self) {
        unregister(self.0);
        // SAFETY: created by Box::into_raw in `new` and freed only here.
        drop(unsafe { Box::from_raw(self.0) });
    }
}

#[cfg(target_os = "macos")]
fn is_main_thread() -> bool {
    extern "C" {
        fn pthread_main_np() -> std::ffi::c_int;
    }
    // SAFETY: pthread_main_np has no preconditions.
    unsafe { pthread_main_np() == 1 }
}

#[cfg(not(target_os = "macos"))]
fn is_main_thread() -> bool {
    // Other platforms report a non-main thread from the event loop itself.
    true
}

/// Opens a native window and calls `callback` once per frame until the window
/// is closed or rugi_ui_request_exit is called. Blocks. Must be called on
/// the main thread, at most once per process (OS event-loop restriction);
/// otherwise it returns an error (RUGI_ERROR_WRONG_THREAD off the main
/// thread on macOS) without opening a window.
#[no_mangle]
pub unsafe extern "C" fn rugi_app_run(
    desc: *const RugiAppDesc,
    callback: RugiFrameCallback,
    user_data: *mut c_void,
) -> RugiResult {
    guard(|| {
        let cb = callback.ok_or_else(|| FfiError::null("callback"))?;
        let mut options = AppOptions::default();
        // SAFETY: `desc` is NULL or points to a RugiAppDesc of at least its
        // struct_size bytes; only that many are read.
        if let Some((d, size)) = unsafe { read_versioned(desc, "desc") }? {
            // SAFETY: `title` is NULL or a NUL-terminated string valid for this call.
            unsafe { apply_desc(&mut options, &d, size) }?;
        }
        if !is_main_thread() {
            return Err(FfiError::new(RUGI_ERROR_WRONG_THREAD, "rugi_app_run must be called on the main thread"));
        }
        // The frame-callback handle: registered (so calls on it are
        // validated like any other handle) for as long as the window runs.
        let frame = FrameHandle::new();
        let (hp, user) = (frame.0 as usize, user_data as usize);
        let app = move |ui: &mut Ui| {
            let h = hp as *mut Handle;
            // SAFETY: `h` is the live, registered Handle owned by `frame`,
            // which outlives rugi_winit::run; no reference to it is held here.
            unsafe {
                (*h).ui = ui as *mut Ui;
                (*h).in_frame = true;
            }
            // SAFETY: `cb` is a valid C function pointer supplied by the
            // caller; the handle stays valid for the duration of the call.
            unsafe { cb(h as *mut RugiUi, user as *mut c_void) };
            // SAFETY: as above.
            unsafe {
                (*h).ui = std::ptr::null_mut();
                (*h).in_frame = false;
            }
        };
        let result = rugi_winit::run(options, app);
        drop(frame);
        result.map_err(|e| match e {
            RunError::Graphics(m) => FfiError::new(RUGI_ERROR_GRAPHICS, m),
            other => FfiError::new(RUGI_ERROR_WINDOW, other.to_string()),
        })
    })
}
