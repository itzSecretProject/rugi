//! Library version, context lifecycle and frames for host-driven use.

use rugi_core::{Repaint, Vec2};
use rugi_ui::{FrameInput, Ui, UiOptions};

use crate::args::{in_ref, opt_out, out_mut, read_versioned};
use crate::error::{guard, FfiError, RugiResult, RUGI_ERROR_VERSION_MISMATCH, RUGI_OK};
use crate::handle::{handle, register, unregister, Handle, RugiUi};
use crate::style::{theme_from, RugiTheme};

/// ABI version. Incremented on any incompatible change to this header.
pub const RUGI_ABI_VERSION: u32 = 1;

/// Options for rugi_ui_create. Set struct_size = sizeof(RugiUiDesc).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RugiUiDesc {
    /// Must equal sizeof(RugiUiDesc).
    pub struct_size: u32,
    /// RUGI_THEME_*.
    pub theme: RugiTheme,
    /// Non-zero: record widget inspection data.
    pub inspection: u8,
    /// Non-zero: also load OS fonts for fallback (slower startup).
    pub load_system_fonts: u8,
}

/// Per-frame parameters for rugi_ui_begin_frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RugiFrameDesc {
    /// Logical width.
    pub width: f32,
    /// Logical height.
    pub height: f32,
    /// Physical pixels per logical pixel (> 0).
    pub scale: f32,
    /// Seconds since the previous frame.
    pub dt: f32,
}

/// Statistics of a completed frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RugiFrameStats {
    /// CPU microseconds spent building the frame.
    pub build_us: u32,
    /// GPU instances (primitives).
    pub instances: u32,
    /// Draw batches.
    pub batches: u32,
    /// Interactive widgets.
    pub widgets: u32,
    /// Primitives culled by clipping.
    pub culled: u32,
    /// Non-zero if the UI wants another frame immediately (animating).
    pub wants_repaint: u8,
}

/// Display refresh and measured frame pacing (filled by rugi_app_run).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RugiDisplayInfo {
    /// Monitor refresh rate in Hz, 0 if unknown.
    pub refresh_hz: f32,
    /// Mean presented-frame interval while animating (ms).
    pub frame_interval_ms: f32,
    /// 99th percentile presented-frame interval (ms).
    pub frame_interval_p99_ms: f32,
    /// Frames per second while animating.
    pub fps: f32,
}

fn stats_from(ui: &Ui) -> RugiFrameStats {
    let out = ui.output();
    let s = out.stats;
    RugiFrameStats {
        build_us: s.build_us,
        instances: s.instances,
        batches: s.batches,
        widgets: s.widgets,
        culled: s.culled,
        wants_repaint: (out.repaint() == Repaint::Immediately) as u8,
    }
}

/// Library version encoded as (major << 16) | (minor << 8) | patch.
#[no_mangle]
pub extern "C" fn rugi_version() -> u32 {
    let p = |s: &str| s.parse::<u32>().unwrap_or(0);
    (p(env!("CARGO_PKG_VERSION_MAJOR")) << 16)
        | (p(env!("CARGO_PKG_VERSION_MINOR")) << 8)
        | p(env!("CARGO_PKG_VERSION_PATCH"))
}

/// Returns RUGI_ABI_VERSION of the loaded library (compare with the header's).
#[no_mangle]
pub extern "C" fn rugi_abi_version() -> u32 {
    RUGI_ABI_VERSION
}

/// Creates a UI context not bound to any window. The host feeds input,
/// brackets frames, and may read statistics. `desc` may be NULL for defaults.
/// On success `*out_ui` receives the handle; destroy it with rugi_ui_destroy.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_create(desc: *const RugiUiDesc, out_ui: *mut *mut RugiUi) -> RugiResult {
    guard(|| {
        if out_ui.is_null() {
            return Err(FfiError::null("out_ui"));
        }
        let mut options = UiOptions::default();
        // SAFETY: `desc` is NULL or points to a readable RugiUiDesc of at
        // least its struct_size; only that many bytes are read.
        if let Some((d, size)) = unsafe { read_versioned(desc, "desc") }? {
            if size != std::mem::size_of::<RugiUiDesc>() {
                return Err(FfiError::new(RUGI_ERROR_VERSION_MISMATCH, "RugiUiDesc.struct_size mismatch"));
            }
            options.theme = theme_from(d.theme)?;
            options.inspection = d.inspection != 0;
            options.text.load_system_fonts = d.load_system_fonts != 0;
        }
        // SAFETY: `out_ui` is non-null and, per the contract, writable.
        let out_ui = unsafe { out_mut(out_ui, "out_ui") }?;
        let ui = Box::into_raw(Box::new(Ui::new(options)));
        let h = Box::into_raw(Box::new(Handle::new(ui, true)));
        register(h);
        *out_ui = h as *mut RugiUi;
        Ok(())
    })
}

/// Destroys a context created by rugi_ui_create. NULL is a no-op. Returns
/// RUGI_ERROR_INVALID_ARGUMENT for a handle that is already destroyed or was
/// not created by rugi_ui_create (including the frame-callback handle), and
/// RUGI_ERROR_INVALID_STATE inside a table/carousel callback.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_destroy(ui: *mut RugiUi) -> RugiResult {
    if ui.is_null() {
        return RUGI_OK;
    }
    guard(|| {
        // SAFETY: contract of `handle`.
        let h = unsafe { handle(ui) }?;
        if !h.owned {
            return Err(FfiError::invalid("cannot destroy the handle owned by rugi_app_run"));
        }
        h.not_in_callback("rugi_ui_destroy")?;
        unregister(ui as *mut Handle);
        // SAFETY: owned handles were created by Box::into_raw in rugi_ui_create
        // (both the Handle and its Ui). It was registered until the line above,
        // so it is destroyed exactly once here.
        unsafe {
            let h = Box::from_raw(ui as *mut Handle);
            drop(Box::from_raw(h.ui));
        }
        Ok(())
    })
}

/// Begins a frame. Input queued since the previous frame is applied.
/// Returns RUGI_ERROR_INVALID_ARGUMENT for a negative or non-finite size or
/// a scale that is not finite and > 0, and RUGI_ERROR_INVALID_STATE while a
/// frame is in progress, inside a table/carousel callback, or on the handle
/// passed to a rugi_app_run frame callback (the runner brackets its frames).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_begin_frame(ui: *mut RugiUi, desc: *const RugiFrameDesc) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`; `desc` NULL or readable.
        let (h, d) = unsafe { (handle(ui)?, in_ref(desc, "desc")?) };
        if !(d.width.is_finite() && d.height.is_finite()) || d.width < 0.0 || d.height < 0.0 {
            return Err(FfiError::invalid("frame size must be finite and >= 0"));
        }
        if !(d.scale.is_finite() && d.scale > 0.0) {
            return Err(FfiError::invalid("frame scale must be finite and > 0"));
        }
        h.not_in_callback("rugi_ui_begin_frame")?;
        if !h.owned {
            return Err(FfiError::state("rugi_app_run begins and ends the frames of its callback handle"));
        }
        // Misuse recorded since the last frame was reported by its own call.
        let _ = h.ui().take_error();
        h.ui().begin_frame(FrameInput { size: Vec2::new(d.width, d.height), scale: d.scale, dt: d.dt })?;
        h.in_frame = true;
        Ok(())
    })
}

/// Ends the frame. `out_stats` may be NULL. Returns RUGI_ERROR_INVALID_STATE
/// if API misuse was detected during the frame (e.g. unbalanced begin/end);
/// the frame is still completed. Like rugi_ui_begin_frame, it is rejected
/// inside a callback and on a frame-callback handle.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_end_frame(ui: *mut RugiUi, out_stats: *mut RugiFrameStats) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        let h = unsafe { handle(ui) }?;
        // SAFETY: `out_stats` is NULL or writable.
        let out = unsafe { opt_out(out_stats, "out_stats") }?;
        h.not_in_callback("rugi_ui_end_frame")?;
        if !h.owned {
            return Err(FfiError::state("rugi_app_run begins and ends the frames of its callback handle"));
        }
        h.ui().end_frame();
        h.in_frame = false;
        if let Some(s) = out {
            *s = stats_from(h.ui());
        }
        h.check()
    })
}

/// Statistics of the most recently completed frame.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_frame_stats(ui: *mut RugiUi, out_stats: *mut RugiFrameStats) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`; `out_stats` NULL or writable.
        let (h, s) = unsafe { (handle(ui)?, out_mut(out_stats, "out_stats")?) };
        *s = stats_from(h.ui());
        Ok(())
    })
}

/// Display refresh rate and measured frame pacing.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_display_info(ui: *mut RugiUi, out: *mut RugiDisplayInfo) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`; `out` NULL or writable.
        let (h, o) = unsafe { (handle(ui)?, out_mut(out, "out")?) };
        let d = h.ui().display_info();
        *o = RugiDisplayInfo {
            refresh_hz: d.refresh_hz.unwrap_or(0.0),
            frame_interval_ms: d.frame_interval_ms,
            frame_interval_p99_ms: d.frame_interval_p99_ms,
            fps: d.fps,
        };
        Ok(())
    })
}

/// Requests another frame as soon as possible (for host-driven animation).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_request_repaint(ui: *mut RugiUi) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        unsafe { handle(ui) }?.ui().request_repaint();
        Ok(())
    })
}

/// Asks rugi_app_run to close the window after this frame.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_request_exit(ui: *mut RugiUi) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        unsafe { handle(ui) }?.ui().request_exit();
        Ok(())
    })
}
