//! End-to-end tests of the exported functions.

use std::ffi::{c_char, CStr};

use crate::*;

#[test]
fn null_handles_are_reported() {
    unsafe {
        assert_eq!(rugi_ui_begin_frame(std::ptr::null_mut(), std::ptr::null()), RUGI_ERROR_NULL_POINTER);
        assert_eq!(rugi_last_error_code(), RUGI_ERROR_NULL_POINTER);
        let mut buf = [0 as c_char; 64];
        let n = rugi_last_error_message(buf.as_mut_ptr(), buf.len());
        assert!(n > 1);
        assert!(!rugi_button(std::ptr::null_mut(), std::ptr::null(), 0, 0, RUGI_ICON_NONE));
        assert_eq!(rugi_ui_destroy(std::ptr::null_mut()), RUGI_OK);
    }
}

#[test]
fn wrong_thread_is_rejected() {
    unsafe {
        let mut ui = std::ptr::null_mut();
        assert_eq!(rugi_ui_create(std::ptr::null(), &mut ui), RUGI_OK);
        let addr = ui as usize;
        let code = std::thread::spawn(move || rugi_ui_mouse_move(addr as *mut RugiUi, 1.0, 1.0)).join().unwrap();
        assert_eq!(code, RUGI_ERROR_WRONG_THREAD);
        assert_eq!(rugi_ui_destroy(ui), RUGI_OK);
    }
}

#[test]
fn invalid_enums_and_utf8() {
    unsafe {
        let mut ui = std::ptr::null_mut();
        rugi_ui_create(std::ptr::null(), &mut ui);
        let fd = RugiFrameDesc { width: 400.0, height: 300.0, scale: 1.0, dt: 0.016 };
        assert_eq!(rugi_ui_begin_frame(ui, &fd), RUGI_OK);
        assert!(!rugi_button(ui, c"x".as_ptr(), 1, 99, RUGI_ICON_NONE));
        assert_eq!(rugi_last_error_code(), RUGI_ERROR_INVALID_ARGUMENT);
        let bad = [0xffu8, 0xfe];
        assert_eq!(rugi_text(ui, bad.as_ptr() as *const c_char, 2, RUGI_TEXT_BODY), RUGI_ERROR_INVALID_UTF8);
        assert_eq!(rugi_ui_set_theme(ui, 42), RUGI_ERROR_INVALID_ARGUMENT);
        assert_eq!(rugi_ui_end_frame(ui, std::ptr::null_mut()), RUGI_OK);
        rugi_ui_destroy(ui);
    }
}

#[test]
fn text_input_respects_capacity() {
    unsafe {
        let mut ui = std::ptr::null_mut();
        rugi_ui_create(std::ptr::null(), &mut ui);
        let fd = RugiFrameDesc { width: 400.0, height: 300.0, scale: 1.0, dt: 0.016 };
        let mut buf = [0 as c_char; 6];
        let frame = |ui: *mut RugiUi, buf: &mut [c_char; 6]| {
            rugi_ui_begin_frame(ui, &fd);
            rugi_begin_central_panel(ui);
            let changed = rugi_text_input(ui, c"t".as_ptr(), 1, buf.as_mut_ptr(), buf.len(), std::ptr::null(), 0);
            let mut st = RugiItemState::default();
            rugi_last_item(ui, &mut st);
            rugi_end(ui);
            rugi_ui_end_frame(ui, std::ptr::null_mut());
            (changed, st)
        };
        let (_, st) = frame(ui, &mut buf);
        rugi_ui_mouse_move(ui, st.rect.x + 5.0, st.rect.y + 5.0);
        rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 1);
        rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 0);
        frame(ui, &mut buf);
        let s = "añbcdefg";
        rugi_ui_text(ui, s.as_ptr() as *const c_char, s.len());
        let (changed, _) = frame(ui, &mut buf);
        assert!(changed);
        let out = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
        assert_eq!(out, "añbc"); // 5 bytes + NUL = 6
        rugi_ui_destroy(ui);
    }
}

fn frame_desc() -> RugiFrameDesc {
    RugiFrameDesc { width: 400.0, height: 300.0, scale: 1.0, dt: 0.016 }
}

fn new_ui() -> *mut RugiUi {
    let mut ui = std::ptr::null_mut();
    assert_eq!(unsafe { rugi_ui_create(std::ptr::null(), &mut ui) }, RUGI_OK);
    ui
}

#[test]
fn destroyed_and_foreign_handles_are_rejected() {
    unsafe {
        let ui = new_ui();
        assert_eq!(rugi_ui_destroy(ui), RUGI_OK);
        // Use after destroy and double destroy: detected before dereferencing.
        assert_eq!(rugi_ui_mouse_move(ui, 1.0, 1.0), RUGI_ERROR_INVALID_ARGUMENT);
        assert!(!rugi_button(ui, c"x".as_ptr(), 1, 0, RUGI_ICON_NONE));
        assert_eq!(rugi_last_error_code(), RUGI_ERROR_INVALID_ARGUMENT);
        assert_eq!(rugi_ui_destroy(ui), RUGI_ERROR_INVALID_ARGUMENT);
        let mut junk = [0xabu64; 8];
        let foreign = junk.as_mut_ptr() as *mut RugiUi;
        assert_eq!(rugi_ui_begin_frame(foreign, &frame_desc()), RUGI_ERROR_INVALID_ARGUMENT);
        assert_eq!(rugi_ui_destroy(foreign), RUGI_ERROR_INVALID_ARGUMENT);
    }
}

#[test]
fn oversized_lengths_and_misaligned_pointers_are_rejected() {
    unsafe {
        let ui = new_ui();
        rugi_ui_begin_frame(ui, &frame_desc());
        // Lengths > isize::MAX would be UB in slice::from_raw_parts.
        assert_eq!(rugi_text(ui, c"abc".as_ptr(), usize::MAX, RUGI_TEXT_BODY), RUGI_ERROR_INVALID_ARGUMENT);
        assert_eq!(rugi_lucide_icon(c"save".as_ptr(), usize::MAX, std::ptr::null_mut(), 0), 0);
        assert_eq!(rugi_last_error_code(), RUGI_ERROR_INVALID_ARGUMENT);
        let items = [RugiStr { ptr: c"a".as_ptr(), len: 1 }];
        let mut sel = 0u32;
        assert!(!rugi_combo_box(ui, c"c".as_ptr(), 1, &mut sel, items.as_ptr(), usize::MAX / 2));
        assert_eq!(rugi_last_error_code(), RUGI_ERROR_INVALID_ARGUMENT);
        let mut raw = [0u64; 4];
        let misaligned = (raw.as_mut_ptr() as *mut u8).add(1) as *mut RugiRect;
        assert_eq!(rugi_screen_rect(ui, misaligned), RUGI_ERROR_INVALID_ARGUMENT);
        rugi_ui_end_frame(ui, std::ptr::null_mut());
        rugi_ui_destroy(ui);
    }
}

struct Destroyer {
    destroy: RugiResult,
    end_frame: RugiResult,
}

unsafe extern "C" fn destroy_in_cell(ui: *mut RugiUi, _row: usize, _col: usize, user: *mut std::ffi::c_void) {
    let d = unsafe { &mut *(user as *mut Destroyer) };
    d.destroy = unsafe { rugi_ui_destroy(ui) };
    d.end_frame = unsafe { rugi_ui_end_frame(ui, std::ptr::null_mut()) };
}

#[test]
fn destroy_and_end_frame_inside_a_callback_are_rejected() {
    unsafe {
        let ui = new_ui();
        rugi_ui_begin_frame(ui, &frame_desc());
        rugi_begin_central_panel(ui);
        let headers = [RugiStr { ptr: c"a".as_ptr(), len: 1 }];
        let mut d = Destroyer { destroy: RUGI_OK, end_frame: RUGI_OK };
        let user = &mut d as *mut Destroyer as *mut std::ffi::c_void;
        rugi_table(
            ui,
            c"t".as_ptr(),
            1,
            headers.as_ptr(),
            std::ptr::null(),
            1,
            3,
            std::ptr::null_mut(),
            100.0,
            Some(destroy_in_cell),
            user,
        );
        assert_eq!(d.destroy, RUGI_ERROR_INVALID_STATE);
        assert_eq!(d.end_frame, RUGI_ERROR_INVALID_STATE);
        assert_eq!(rugi_end(ui), RUGI_OK);
        assert_eq!(rugi_ui_end_frame(ui, std::ptr::null_mut()), RUGI_OK);
        assert_eq!(rugi_ui_destroy(ui), RUGI_OK);
    }
}

#[test]
fn non_canonical_c_bools_are_read_as_bytes() {
    unsafe {
        let ui = new_ui();
        rugi_ui_begin_frame(ui, &frame_desc());
        rugi_begin_central_panel(ui);
        let items = [RugiStr { ptr: c"a".as_ptr(), len: 1 }, RugiStr { ptr: c"b".as_ptr(), len: 1 }];
        let mut flags = [2u8, 0x80u8];
        rugi_multi_select(ui, c"m".as_ptr(), 1, flags.as_mut_ptr() as *mut bool, items.as_ptr(), 2);
        assert_eq!(rugi_last_error_code(), RUGI_OK);
        assert_eq!(flags, [1, 1]);
        let mut bind = RugiKeyBind { mode: RUGI_BIND_ALWAYS, ..Default::default() };
        std::ptr::write(std::ptr::addr_of_mut!(bind.toggled) as *mut u8, 7);
        assert!(rugi_bind_active(ui, &mut bind));
        rugi_end(ui);
        rugi_ui_end_frame(ui, std::ptr::null_mut());
        rugi_ui_destroy(ui);
    }
}

#[test]
fn errors_are_reported_by_the_call_that_caused_them() {
    unsafe {
        let ui = new_ui();
        // Frame-only calls outside a frame fail themselves...
        assert_eq!(rugi_paragraph(ui, c"p".as_ptr(), 1), RUGI_ERROR_INVALID_STATE);
        assert_eq!(rugi_push_id(ui, c"p".as_ptr(), 1), RUGI_ERROR_INVALID_STATE);
        let c = RugiColor::default();
        assert_eq!(rugi_push_color(ui, RUGI_COLOR_TEXT, c), RUGI_ERROR_INVALID_STATE);
        // ...and never leak into the next frame.
        assert_eq!(rugi_ui_begin_frame(ui, &frame_desc()), RUGI_OK);
        assert_eq!(rugi_begin_central_panel(ui), RUGI_OK);
        assert_eq!(rugi_text(ui, c"t".as_ptr(), 1, RUGI_TEXT_BODY), RUGI_OK);
        // A rejected begin still opens a group, keeping rugi_end balanced.
        let bad = RugiRect { x: f32::NAN, y: 0.0, w: 1.0, h: 1.0 };
        assert_eq!(rugi_begin_region(ui, bad, false), RUGI_ERROR_INVALID_ARGUMENT);
        assert_eq!(rugi_end(ui), RUGI_OK);
        assert_eq!(rugi_end(ui), RUGI_OK);
        assert_eq!(rugi_ui_end_frame(ui, std::ptr::null_mut()), RUGI_OK);
        // A bad scale is an invalid argument, not a state error.
        let mut fd = frame_desc();
        fd.scale = f32::NAN;
        assert_eq!(rugi_ui_begin_frame(ui, &fd), RUGI_ERROR_INVALID_ARGUMENT);
        rugi_ui_destroy(ui);
    }
}

#[test]
fn versioned_descriptors_are_read_up_to_struct_size() {
    unsafe {
        // Only `struct_size` bytes exist: nothing past them may be read.
        let short = std::alloc::Layout::from_size_align(4, 4).unwrap();
        let p = std::alloc::alloc(short) as *mut u32;
        *p = 4;
        let mut ui = std::ptr::null_mut();
        assert_eq!(rugi_ui_create(p as *const RugiUiDesc, &mut ui), RUGI_ERROR_VERSION_MISMATCH);
        assert!(ui.is_null());
        std::alloc::dealloc(p as *mut u8, short);
    }
}
