//! The object behind `RugiUi*` and the checks every call makes on it.

use std::cell::{Cell, RefCell};
use std::sync::{Mutex, PoisonError};

use rugi_ui::{Response, Ui};

use crate::error::{guard, FfiError, RugiResult, RUGI_ERROR_WRONG_THREAD};

/// Opaque UI context handle.
pub struct RugiUi {
    _private: [u8; 0],
}

/// The real object behind `RugiUi*`.
pub(crate) struct Handle {
    /// A Box-owned Ui (`owned`) or the runner's Ui during a frame callback.
    pub(crate) ui: *mut Ui,
    /// Created by rugi_ui_create, so rugi_ui_destroy may free it.
    pub(crate) owned: bool,
    /// Table/carousel callbacks currently running on this handle. While
    /// non-zero the handle must not be destroyed and frames must not change.
    pub(crate) callbacks: u32,
    /// Between a successful begin_frame and end_frame (always true during a
    /// rugi_app_run frame callback).
    pub(crate) in_frame: bool,
    /// Response of the most recent interactive widget (rugi_last_item).
    pub(crate) last: Response,
    /// Reused buffer for editing text input contents.
    pub(crate) scratch: String,
}

thread_local! {
    /// Addresses of the live handles of this thread (a cache of `ALL`).
    static LOCAL: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    /// Its address identifies the thread (a later thread may reuse it once
    /// this one has exited). It has no destructor, so it stays accessible
    /// during thread teardown (unlike `std::thread::current`).
    static TOKEN: Cell<u8> = const { Cell::new(0) };
}

/// Identifies the current thread among live threads.
fn thread_token() -> usize {
    TOKEN.with(|t| t as *const Cell<u8> as usize)
}

/// All live handles and their owning threads: tells a wrong-thread call from
/// a destroyed or foreign pointer without dereferencing it, and still works
/// while this thread's thread-locals are being destroyed (e.g. a C++
/// `thread_local` destructor that destroys its context after `LOCAL` is gone).
static ALL: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

fn all() -> std::sync::MutexGuard<'static, Vec<(usize, usize)>> {
    ALL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Makes the Handle at `h` usable on the current thread until [`unregister`].
pub(crate) fn register(h: *mut Handle) {
    let addr = h as usize;
    all().push((addr, thread_token()));
    let _ = LOCAL.try_with(|l| l.borrow_mut().push(addr));
}

/// Removes a handle registered on the current thread.
pub(crate) fn unregister(h: *mut Handle) {
    let addr = h as usize;
    let _ = LOCAL.try_with(|l| l.borrow_mut().retain(|&a| a != addr));
    all().retain(|&(a, _)| a != addr);
}

/// Checks that `addr` is a live handle of the current thread.
fn lookup(addr: usize) -> Result<(), FfiError> {
    if LOCAL.try_with(|l| l.borrow().contains(&addr)).unwrap_or(false) {
        return Ok(());
    }
    match all().iter().find(|&&(a, _)| a == addr) {
        Some(&(_, owner)) if owner == thread_token() => Ok(()),
        Some(_) => {
            Err(FfiError::new(RUGI_ERROR_WRONG_THREAD, "RugiUi used from a different thread than it was created on"))
        }
        None => Err(FfiError::invalid("ui is not a live RugiUi (destroyed, or not from rugi_ui_create)")),
    }
}

impl Handle {
    pub(crate) fn new(ui: *mut Ui, owned: bool) -> Handle {
        Handle { ui, owned, callbacks: 0, in_frame: false, last: Response::default(), scratch: String::new() }
    }

    pub(crate) fn ui(&mut self) -> &mut Ui {
        // SAFETY: `self.ui` is a Box-owned Ui (owned == true) or the runner's
        // `&mut Ui` that outlives the frame callback (owned == false). The
        // Handle is the only path to it while a call is in progress.
        unsafe { &mut *self.ui }
    }

    /// Records `r` as the most recent item (rugi_last_item) and returns it.
    pub(crate) fn track(&mut self, r: Response) -> Response {
        self.last = r;
        r
    }

    /// Reports API misuse recorded by the Ui (see `From<UiError> for FfiError`).
    pub(crate) fn check(&mut self) -> Result<(), FfiError> {
        self.ui().take_error().map_or(Ok(()), |e| Err(e.into()))
    }

    /// Rejects frame-only calls outside begin_frame/end_frame (for the calls
    /// the engine itself does not check).
    pub(crate) fn require_frame(&self) -> Result<(), FfiError> {
        if !self.in_frame {
            return Err(FfiError::from(rugi_ui::UiError::NoFrame));
        }
        Ok(())
    }

    /// Rejects calls that would invalidate the Ui a running callback uses.
    pub(crate) fn not_in_callback(&self, what: &str) -> Result<(), FfiError> {
        if self.callbacks > 0 {
            return Err(FfiError::state(format!("{what} is not allowed inside a table/carousel callback")));
        }
        Ok(())
    }
}

/// Resolves a handle pointer, checking null, liveness and thread affinity
/// before it is dereferenced.
///
/// # Safety
/// No other reference to the Handle may be in use during the call (the
/// re-entrant callbacks only keep raw pointers to it).
pub(crate) unsafe fn handle<'a>(ui: *mut RugiUi) -> Result<&'a mut Handle, FfiError> {
    if ui.is_null() {
        return Err(FfiError::null("ui"));
    }
    lookup(ui as usize)?;
    // SAFETY: `ui` is registered, so it points to a live Handle owned by this
    // thread (registered handles are unregistered before they are freed), and
    // no other reference to it exists during this call (contract above).
    Ok(unsafe { &mut *(ui as *mut Handle) })
}

/// Runs a call on the handle, then reports misuse the Ui recorded during it.
/// An error from `f` takes precedence; recorded misuse is cleared either way
/// so it is never reported by a later, valid call.
///
/// # Safety
/// See [`handle`].
pub(crate) unsafe fn with_handle(ui: *mut RugiUi, f: impl FnOnce(&mut Handle) -> Result<(), FfiError>) -> RugiResult {
    guard(|| {
        // SAFETY: forwarded contract.
        let h = unsafe { handle(ui) }?;
        let r = f(h);
        let misuse = h.check();
        r.and(misuse)
    })
}

/// Runs a call that opens a container. If `open` rejects its arguments, an
/// empty vertical group is opened instead (inside a frame), so the caller's
/// matching rugi_end stays balanced, as the engine does for a panel opened
/// below the root.
///
/// # Safety
/// See [`handle`].
pub(crate) unsafe fn begin_container(
    ui: *mut RugiUi,
    open: impl FnOnce(&mut Handle) -> Result<(), FfiError>,
) -> RugiResult {
    // SAFETY: forwarded contract.
    unsafe {
        with_handle(ui, |h| {
            let r = open(h);
            if r.is_err() && h.in_frame {
                h.ui().begin_vertical();
            }
            r
        })
    }
}

/// Runs a widget call that returns a value, then reports recorded misuse.
/// If `f` fails the result is `R::default()`; recorded misuse is reported
/// without discarding the value `f` returned.
///
/// # Safety
/// See [`handle`].
pub(crate) unsafe fn with_ui<R: Default>(ui: *mut RugiUi, f: impl FnOnce(&mut Handle) -> Result<R, FfiError>) -> R {
    let mut out = R::default();
    guard(|| {
        // SAFETY: forwarded contract.
        let h = unsafe { handle(ui) }?;
        let r = f(h);
        let misuse = h.check();
        out = r?;
        misuse
    });
    out
}

/// Runs a call that needs only the Ui, then reports recorded misuse.
///
/// # Safety
/// See [`handle`].
pub(crate) unsafe fn ui_call<T>(ui: *mut RugiUi, f: impl FnOnce(&mut Ui) -> T) -> RugiResult {
    // SAFETY: forwarded contract.
    unsafe {
        with_handle(ui, |h| {
            f(h.ui());
            Ok(())
        })
    }
}
