/* Hostile-caller test of the C ABI.
 *
 * Calls every exported function with invalid handles (NULL, destroyed,
 * foreign, other thread), NULL pointer arguments, tiny output buffers,
 * malformed strings, out-of-range enums and indices, non-finite floats,
 * calls outside a frame, unbalanced begin/end pairs, and re-entrant or
 * destructive calls from table/carousel callbacks. Every call must return its
 * documented result code (or be a documented no-op). Run it under the
 * RUGI_SANITIZE build to have ASan/UBSan check the C side and every
 * memcpy/strlen the library makes.
 */
#include <rugi/rugi.h>

#include <math.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures = 0;
static int checks = 0;

#define FAIL(...)                                          \
    do {                                                   \
        fprintf(stderr, "FAIL %s:%d: ", __FILE__, __LINE__); \
        fprintf(stderr, __VA_ARGS__);                      \
        fputc('\n', stderr);                               \
        ++failures;                                        \
    } while (0)

#define CHECK(cond)                                \
    do {                                           \
        ++checks;                                  \
        if (!(cond)) FAIL("%s", #cond);            \
    } while (0)

/* A RugiResult-returning call must return `want` and record it. */
#define RC(expr, want)                                                                         \
    do {                                                                                       \
        ++checks;                                                                              \
        rugi_clear_error();                                                                   \
        RugiResult r_ = (expr);                                                               \
        if (r_ != (want)) FAIL("%s returned %d, want %d (%s)", #expr, r_, (want), last_msg()); \
        if ((want) != RUGI_OK && rugi_last_error_code() != (want))                           \
            FAIL("%s recorded %d, want %d", #expr, rugi_last_error_code(), (want));          \
    } while (0)

/* A value-returning call must return `fallback` and record `want`. */
#define VAL(expr, fallback, want)                                                                   \
    do {                                                                                            \
        ++checks;                                                                                   \
        rugi_clear_error();                                                                        \
        unsigned long long v_ = (unsigned long long)(expr);                                         \
        if (v_ != (unsigned long long)(fallback)) FAIL("%s returned %llu", #expr, v_);              \
        if (rugi_last_error_code() != (want))                                                      \
            FAIL("%s recorded %d, want %d (%s)", #expr, rugi_last_error_code(), (want), last_msg()); \
    } while (0)

/* A call whose return value is not checked must record `want`. */
#define ERR(expr, want)                                                                             \
    do {                                                                                            \
        ++checks;                                                                                   \
        rugi_clear_error();                                                                        \
        (void)(expr);                                                                               \
        if (rugi_last_error_code() != (want))                                                      \
            FAIL("%s recorded %d, want %d (%s)", #expr, rugi_last_error_code(), (want), last_msg()); \
    } while (0)

static const char* last_msg(void) {
    static char buf[256];
    buf[0] = 0;
    rugi_last_error_message(buf, sizeof buf);
    return buf;
}

static const RugiFrameDesc FRAME = {800.0f, 600.0f, 1.0f, 1.0f / 60.0f};
static const char BAD_UTF8[] = {(char)0xff, (char)0xfe, 'x'};
static const char CUT_UTF8[] = {'a', (char)0xc3, (char)0xa9}; /* "aé"; len 2 cuts the é */
static const size_t HUGE_LEN = SIZE_MAX;
static const size_t OVER_ISIZE = (SIZE_MAX >> 1) + 1;

static RugiUi* create(void) {
    RugiUi* ui = NULL;
    if (rugi_ui_create(NULL, &ui) != RUGI_OK || !ui) {
        fprintf(stderr, "rugi_ui_create failed: %s\n", last_msg());
        exit(2);
    }
    return ui;
}

/* --------------------------------------------------------------------------
 * Every function that takes a RugiUi*, called with otherwise valid
 * arguments. Returns the recorded result code of call `i`, or -1 past the end.
 * rugi_ui_destroy is excluded (tested separately).
 * ------------------------------------------------------------------------ */

static void noop_cell(RugiUi* ui, size_t row, size_t col, void* user) {
    (void)ui;
    (void)row;
    (void)col;
    (void)user;
}
static void noop_card(RugiUi* ui, size_t i, RugiRect r, float e, void* user) {
    (void)ui;
    (void)i;
    (void)r;
    (void)e;
    (void)user;
}

#define NUM_UI_CALLS 95
static const char* ui_call_name[NUM_UI_CALLS];

static int ui_call(RugiUi* ui, int i) {
    static bool b;
    static float f;
    static int32_t i32;
    static uint32_t u32;
    static int64_t i64;
    static char buf[32];
    static bool flags[2];
    static RugiKeyBind kb;
    static RugiFrameStats st;
    static RugiDisplayInfo di;
    static RugiItemState is;
    static RugiRect rect;
    static RugiFont font;
    static RugiColor got;
    RugiStr items[2] = {{"a", 1}, {"b", 1}};
    RugiColor c = {1, 2, 3, 255};
    RugiRect r = {10, 10, 100, 50};
    RugiGlassStyle g;
    RugiBackdrop bd;
    rugi_glass_style_preset(RUGI_GLASS_FROSTED, &g);
    rugi_backdrop_preset(RUGI_BACKDROP_AURORA, &bd);
    buf[0] = 0;
    memset(&kb, 0, sizeof kb);
    rugi_clear_error();
    RugiResult rc = RUGI_OK;
    const char* n = NULL;
#define R(expr) (n = #expr, rc = (expr))
#define V(expr) (n = #expr, (void)(expr), rc = rugi_last_error_code())
    switch (i) {
        case 0: R(rugi_ui_begin_frame(ui, &FRAME)); break;
        case 1: R(rugi_ui_end_frame(ui, &st)); break;
        case 2: R(rugi_ui_frame_stats(ui, &st)); break;
        case 3: R(rugi_ui_display_info(ui, &di)); break;
        case 4: R(rugi_ui_request_repaint(ui)); break;
        case 5: R(rugi_ui_request_exit(ui)); break;
        case 6: R(rugi_ui_mouse_move(ui, 1, 1)); break;
        case 7: R(rugi_ui_mouse_leave(ui)); break;
        case 8: R(rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 0)); break;
        case 9: R(rugi_ui_scroll(ui, 0, 1)); break;
        case 10: R(rugi_ui_key(ui, RUGI_KEY_TAB, 0, 0)); break;
        case 11: R(rugi_ui_modifiers(ui, 0)); break;
        case 12: R(rugi_ui_text(ui, "a", 1)); break;
        case 13: R(rugi_ui_paste(ui, "a", 1)); break;
        case 14: V(rugi_ui_clipboard_text(ui, buf, sizeof buf)); break;
        case 15: R(rugi_ui_set_theme(ui, RUGI_THEME_DARK)); break;
        case 16: R(rugi_ui_set_accent(ui, c)); break;
        case 17: R(rugi_ui_set_color(ui, RUGI_COLOR_TEXT, c)); break;
        case 18: R(rugi_ui_set_metric(ui, RUGI_METRIC_SPACING, 8)); break;
        case 19: R(rugi_push_color(ui, RUGI_COLOR_TEXT, c)); break;
        case 20: R(rugi_push_metric(ui, RUGI_METRIC_SPACING, 8)); break;
        case 21: R(rugi_pop_style(ui)); break;
        case 22: R(rugi_ui_add_font(ui, (const uint8_t*)"junk", 4, &font)); break;
        case 23: R(rugi_ui_add_font_file(ui, "/nonexistent.ttf", 16, &font)); break;
        case 24: V(rugi_ui_icon_font(ui)); break;
        case 25: R(rugi_ui_set_font(ui, RUGI_FONT_UI, 1)); break;
        case 26: R(rugi_push_font(ui, RUGI_FONT_MONO)); break;
        case 27: R(rugi_begin_side_panel(ui, 100)); break;
        case 28: R(rugi_begin_top_bar(ui, 30)); break;
        case 29: R(rugi_begin_central_panel(ui)); break;
        case 30: R(rugi_begin_vertical(ui)); break;
        case 31: R(rugi_begin_horizontal(ui, RUGI_ALIGN_START)); break;
        case 32: R(rugi_begin_columns(ui, 2)); break;
        case 33: R(rugi_begin_card(ui)); break;
        case 34: R(rugi_begin_scroll_area(ui, "s", 1, 100)); break;
        case 35: R(rugi_begin_region(ui, r, false)); break;
        case 36: V(rugi_begin_collapsing(ui, "c", 1, 1)); break;
        case 37: R(rugi_end(ui)); break;
        case 38: R(rugi_space(ui, 4)); break;
        case 39: R(rugi_separator(ui)); break;
        case 40: R(rugi_set_next_item_width(ui, 50)); break;
        case 41: R(rugi_push_id(ui, "id", 2)); break;
        case 42: R(rugi_push_id_int(ui, 7)); break;
        case 43: R(rugi_pop_id(ui)); break;
        case 44: R(rugi_text(ui, "t", 1, RUGI_TEXT_BODY)); break;
        case 45: R(rugi_text_ex(ui, "t", 1, RUGI_FONT_UI, 14, 400, c)); break;
        case 46: R(rugi_text_truncated(ui, "t", 1, RUGI_TEXT_BODY)); break;
        case 47: R(rugi_text_centered(ui, "t", 1, RUGI_TEXT_BODY)); break;
        case 48: R(rugi_paragraph(ui, "p", 1)); break;
        case 49: R(rugi_badge(ui, "b", 1, c)); break;
        case 50: V(rugi_button(ui, "b", 1, RUGI_BUTTON_PRIMARY, RUGI_ICON_NONE)); break;
        case 51: V(rugi_nav_item(ui, "n", 1, RUGI_ICON_CHART, 0)); break;
        case 52: V(rugi_icon_button(ui, "i", 1, RUGI_ICON_PLUS, 24, NULL, 0)); break;
        case 53: V(rugi_rail_item(ui, "r", 1, RUGI_ICON_PLUS, false, "tip", 3)); break;
        case 54: V(rugi_checkbox(ui, "c", 1, &b)); break;
        case 55: V(rugi_toggle(ui, "t", 1, &b)); break;
        case 56: V(rugi_radio(ui, "r", 1, &i32, 1)); break;
        case 57: V(rugi_slider_float(ui, "s", 1, &f, 0, 1)); break;
        case 58: V(rugi_spin_box(ui, "s", 1, &f, 0, 10, 1)); break;
        case 59: V(rugi_text_input(ui, "ti", 2, buf, sizeof buf, "h", 1)); break;
        case 60: V(rugi_text_input_ex(ui, "tx", 2, buf, sizeof buf, NULL, 0, RUGI_INPUT_PASSWORD, RUGI_ICON_NONE)); break;
        case 61: V(rugi_combo_box(ui, "cb", 2, &u32, items, 2)); break;
        case 62: V(rugi_tabs(ui, "tb", 2, &u32, items, 2)); break;
        case 63: V(rugi_segmented(ui, "sg", 2, &u32, items, 2)); break;
        case 64: V(rugi_selectable(ui, "s", 1, 0)); break;
        case 65: V(rugi_multi_select(ui, "m", 1, flags, items, 2)); break;
        case 66: V(rugi_page_dots(ui, "pd", 2, 3, &u32)); break;
        case 67: R(rugi_progress_bar(ui, 0.5f)); break;
        case 68: R(rugi_last_item(ui, &is)); break;
        case 69: V(rugi_begin_menu(ui, "m", 1)); break;
        case 70: V(rugi_begin_context_menu(ui, "cm", 2)); break;
        case 71: V(rugi_menu_item(ui, "mi", 2, NULL, 0)); break;
        case 72: R(rugi_menu_separator(ui)); break;
        case 73: R(rugi_tooltip(ui, "t", 1)); break;
        case 74: V(rugi_begin_modal(ui, "md", 2, "T", 1, &b, 300)); break;
        case 75: V(rugi_table(ui, "tbl", 3, items, NULL, 2, 5, &i64, 100, noop_cell, NULL)); break;
        case 76: V(rugi_carousel(ui, "car", 3, 3, &u32, 50, 50, noop_card, NULL)); break;
        case 77: V(rugi_command_palette(ui, items, NULL, 2, &u32)); break;
        case 78: R(rugi_ui_set_command_palette_open(ui, 0)); break;
        case 79: R(rugi_ui_set_inspector(ui, 0)); break;
        case 80: R(rugi_show_demo(ui)); break;
        case 81: R(rugi_set_demo_page(ui, RUGI_DEMO_BASICS)); break;
        case 82: R(rugi_backdrop(ui, &bd)); break;
        case 83: R(rugi_backdrop_rect(ui, r, 4, &bd)); break;
        case 84: R(rugi_glass(ui, r, &g)); break;
        case 85: R(rugi_begin_glass_panel(ui, r, &g)); break;
        case 86: R(rugi_begin_glass_card(ui, &g)); break;
        case 87: R(rugi_begin_transition(ui, "tr", 2, 1, RUGI_SLIDE_UP)); break;
        case 88: R(rugi_ui_set_ambient_fps(ui, 0, 30)); break;
        case 89: R(rugi_screen_rect(ui, &rect)); break;
        case 90: R(rugi_title_bar(ui, "T", 1, 30)); break;
        case 91: R(rugi_window_command(ui, RUGI_WINDOW_MINIMIZE)); break;
        case 92: V(rugi_keybind(ui, "k", 1, &kb)); break;
        case 93: V(rugi_bind_active(ui, &kb)); break;
        case 94: R(rugi_ui_color(ui, RUGI_COLOR_TEXT, &got)); break;
        default: return -1;
    }
#undef R
#undef V
    ui_call_name[i] = n;
    /* A function that returned an error must also have recorded it. */
    if (rc != RUGI_OK && rugi_last_error_code() != rc) FAIL("%s: returned %d, recorded %d", n, rc, rugi_last_error_code());
    return rc;
}

/* All calls with handle `ui` must record `want`. */
static void all_calls_fail(RugiUi* ui, RugiResult want, const char* what) {
    for (int i = 0; i < NUM_UI_CALLS; ++i) {
        ++checks;
        int rc = ui_call(ui, i);
        if (rc != want) FAIL("%s handle: %s -> %d, want %d (%s)", what, ui_call_name[i], rc, want, last_msg());
    }
}

/* --------------------------------------------------------------------------
 * Handles
 * ------------------------------------------------------------------------ */

static void test_null_and_dead_handles(void) {
    all_calls_fail(NULL, RUGI_ERROR_NULL_POINTER, "NULL");
    RC(rugi_ui_destroy(NULL), RUGI_OK);

    /* Use after destroy and double destroy are detected (the handle registry
     * is consulted before the pointer is dereferenced). */
    RugiUi* dead = create();
    RC(rugi_ui_destroy(dead), RUGI_OK);
    all_calls_fail(dead, RUGI_ERROR_INVALID_ARGUMENT, "destroyed");
    RC(rugi_ui_destroy(dead), RUGI_ERROR_INVALID_ARGUMENT);

    /* Foreign pointers: stack memory, a heap block, misaligned. */
    uint64_t junk[16];
    memset(junk, 0xab, sizeof junk);
    all_calls_fail((RugiUi*)junk, RUGI_ERROR_INVALID_ARGUMENT, "foreign");
    RC(rugi_ui_destroy((RugiUi*)junk), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_destroy((RugiUi*)((char*)junk + 1)), RUGI_ERROR_INVALID_ARGUMENT);
    void* heap = malloc(256);
    memset(heap, 0, 256);
    RC(rugi_ui_destroy((RugiUi*)heap), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_mouse_move((RugiUi*)heap, 1, 1), RUGI_ERROR_INVALID_ARGUMENT);
    free(heap);

    /* create: NULL out pointer, bad struct sizes, bad theme. */
    RC(rugi_ui_create(NULL, NULL), RUGI_ERROR_NULL_POINTER);
    RugiUiDesc d;
    memset(&d, 0, sizeof d);
    RugiUi* out = (RugiUi*)0x1;
    d.struct_size = 0;
    RC(rugi_ui_create(&d, &out), RUGI_ERROR_VERSION_MISMATCH);
    d.struct_size = UINT32_MAX;
    RC(rugi_ui_create(&d, &out), RUGI_ERROR_VERSION_MISMATCH);
    CHECK(out == (RugiUi*)0x1); /* untouched on failure */
    d.struct_size = sizeof d;
    d.theme = 1000;
    RC(rugi_ui_create(&d, &out), RUGI_ERROR_INVALID_ARGUMENT);
    d.theme = RUGI_THEME_GRAPHITE;
    d.inspection = 0xff;
    d.load_system_fonts = 0;
    RC(rugi_ui_create(&d, &out), RUGI_OK);
    RC(rugi_ui_destroy(out), RUGI_OK);

    /* A truncated RugiUiDesc at the end of a heap block: only struct_size is
     * read, so ASan sees no overflow. */
    uint32_t* tiny = (uint32_t*)malloc(sizeof(uint32_t));
    *tiny = 4;
    RC(rugi_ui_create((const RugiUiDesc*)tiny, &out), RUGI_ERROR_VERSION_MISMATCH);
    free(tiny);
}

/* --------------------------------------------------------------------------
 * Global functions
 * ------------------------------------------------------------------------ */

static void test_globals(void) {
    CHECK(rugi_abi_version() == RUGI_ABI_VERSION);
    CHECK(rugi_version() != 0);

    /* Last error: none, then a message into 0/1/small/NULL buffers. */
    rugi_clear_error();
    CHECK(rugi_last_error_code() == RUGI_OK);
    char buf[8];
    memset(buf, 'z', sizeof buf);
    CHECK(rugi_last_error_message(buf, sizeof buf) == 0);
    CHECK(buf[0] == 'z'); /* untouched when there is no error */
    rugi_ui_mouse_move(NULL, 0, 0);
    size_t need = rugi_last_error_message(NULL, 0);
    CHECK(need > 1);
    CHECK(rugi_last_error_message(NULL, 100) == need);
    CHECK(rugi_last_error_message(buf, 0) == need);
    CHECK(buf[0] == 'z');
    CHECK(rugi_last_error_message(buf, 1) == need);
    CHECK(buf[0] == 0 && buf[1] == 'z');
    char* exact = (char*)malloc(need);
    CHECK(rugi_last_error_message(exact, need) == need);
    CHECK(strlen(exact) == need - 1);
    free(exact);
    char* small = (char*)malloc(3);
    rugi_last_error_message(small, 3);
    CHECK(strlen(small) == 2);
    free(small);
    rugi_clear_error();
    CHECK(rugi_last_error_message(NULL, 0) == 0);

    /* Lucide icon lookup. */
    char icon[8];
    VAL(rugi_lucide_icon("save", 4, icon, sizeof icon), 3, RUGI_OK);
    CHECK(strlen(icon) == 3);
    VAL(rugi_lucide_icon("no-such-icon", 12, icon, sizeof icon), 0, RUGI_OK);
    VAL(rugi_lucide_icon(NULL, 0, icon, sizeof icon), 0, RUGI_OK);
    VAL(rugi_lucide_icon(NULL, 4, icon, sizeof icon), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_lucide_icon(BAD_UTF8, 3, icon, sizeof icon), 0, RUGI_ERROR_INVALID_UTF8);
    VAL(rugi_lucide_icon("save", HUGE_LEN, icon, sizeof icon), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_lucide_icon("save", 4, NULL, 0), 0, RUGI_ERROR_INVALID_ARGUMENT);
    char* one = (char*)malloc(1);
    VAL(rugi_lucide_icon("save", 4, one, 1), 0, RUGI_ERROR_INVALID_ARGUMENT);
    free(one);
    char* four = (char*)malloc(4); /* exactly glyph + NUL */
    VAL(rugi_lucide_icon("save", 4, four, 4), 3, RUGI_OK);
    free(four);

    /* Key-binding labels. */
    RugiKeyBind kb;
    memset(&kb, 0, sizeof kb);
    char label[32];
    VAL(rugi_keybind_label(NULL, label, sizeof label), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_keybind_label(&kb, label, sizeof label), 4, RUGI_OK); /* "None" */
    VAL(rugi_keybind_label(&kb, NULL, 0), 4, RUGI_OK);
    char* lab1 = (char*)malloc(1);
    VAL(rugi_keybind_label(&kb, lab1, 1), 4, RUGI_OK);
    CHECK(lab1[0] == 0);
    free(lab1);
    kb.kind = RUGI_BIND_KEY;
    kb.code = 100000;
    VAL(rugi_keybind_label(&kb, label, sizeof label), 0, RUGI_ERROR_INVALID_ARGUMENT);
    kb.kind = RUGI_BIND_MOUSE;
    kb.code = 3;
    VAL(rugi_keybind_label(&kb, label, sizeof label), 0, RUGI_ERROR_INVALID_ARGUMENT);
    kb.code = RUGI_MOUSE_MIDDLE;
    kb.mode = 3;
    VAL(rugi_keybind_label(&kb, label, sizeof label), 0, RUGI_ERROR_INVALID_ARGUMENT);
    kb.kind = 77;
    kb.mode = 0;
    VAL(rugi_keybind_label(&kb, label, sizeof label), 0, RUGI_ERROR_INVALID_ARGUMENT);
    /* A non-canonical C bool in `toggled` is read as a byte, not UB. */
    memset(&kb, 0, sizeof kb);
    kb.kind = RUGI_BIND_KEY;
    kb.code = RUGI_KEY_K;
    kb.mods = 0xffffffffu; /* unknown modifier bits are ignored */
    kb.mode = RUGI_BIND_TOGGLE;
    memset(&kb.toggled, 0x7f, 1);
    CHECK(rugi_keybind_label(&kb, label, sizeof label) > 0);

    /* Presets. */
    RugiGlassStyle g;
    RugiBackdrop bd;
    RC(rugi_glass_style_preset(RUGI_GLASS_SMOKED, &g), RUGI_OK);
    RC(rugi_glass_style_preset(RUGI_GLASS_SMOKED, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_glass_style_preset(3, &g), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_glass_style_preset(UINT32_MAX, NULL), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_backdrop_preset(RUGI_BACKDROP_MESH, &bd), RUGI_OK);
    RC(rugi_backdrop_preset(RUGI_BACKDROP_MESH, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_backdrop_preset(6, &bd), RUGI_ERROR_INVALID_ARGUMENT);
    /* Misaligned output pointers are rejected, not written. */
    _Alignas(8) unsigned char raw[sizeof(RugiGlassStyle) + 8];
    RC(rugi_glass_style_preset(RUGI_GLASS_FROSTED, (RugiGlassStyle*)(raw + 1)), RUGI_ERROR_INVALID_ARGUMENT);

    /* rugi_app_run: every argument error is reported before a window opens. */
    RugiAppDesc ad;
    memset(&ad, 0, sizeof ad);
    ad.struct_size = sizeof ad;
    RC(rugi_app_run(&ad, NULL, NULL), RUGI_ERROR_NULL_POINTER);
}

static void frame_cb(RugiUi* ui, void* user) {
    (void)ui;
    (void)user;
}

static void test_app_run_args(void) {
    RugiAppDesc ad;
    memset(&ad, 0, sizeof ad);
    ad.struct_size = 1;
    RC(rugi_app_run(&ad, frame_cb, NULL), RUGI_ERROR_VERSION_MISMATCH);
    ad.struct_size = sizeof ad;
    ad.theme = 99;
    RC(rugi_app_run(&ad, frame_cb, NULL), RUGI_ERROR_INVALID_ARGUMENT);
    ad.theme = 0;
    ad.present = 3;
    RC(rugi_app_run(&ad, frame_cb, NULL), RUGI_ERROR_INVALID_ARGUMENT);
    ad.present = 0;
    ad.window_flags = 8;
    RC(rugi_app_run(&ad, frame_cb, NULL), RUGI_ERROR_INVALID_ARGUMENT);
    ad.window_flags = 0;
    ad.title = "\xff\xfe";
    RC(rugi_app_run(&ad, frame_cb, NULL), RUGI_ERROR_INVALID_UTF8);
    /* A descriptor that ends before window_flags (an older, shorter struct) at the end of a heap
     * block: the library must not read past struct_size. */
    size_t shorter = offsetof(RugiAppDesc, window_flags);
    RugiAppDesc* p = (RugiAppDesc*)malloc(shorter);
    memset(p, 0, shorter);
    p->struct_size = (uint32_t)shorter;
    p->present = 9;
    RC(rugi_app_run(p, frame_cb, NULL), RUGI_ERROR_INVALID_ARGUMENT);
    free(p);
}

/* rugi_app_run off the main thread must fail cleanly (no window). */
static void* app_run_thread(void* arg) {
    (void)arg;
    RugiAppDesc ad;
    memset(&ad, 0, sizeof ad);
    ad.struct_size = sizeof ad;
    return (void*)(intptr_t)rugi_app_run(&ad, frame_cb, NULL);
}

/* --------------------------------------------------------------------------
 * Frame state
 * ------------------------------------------------------------------------ */

/* Calls that need no frame. */
static int frame_free(int i) {
    switch (i) {
        case 0: case 2: case 3: case 4: case 5: case 6: case 7: case 8: case 9: case 10: case 11:
        case 12: case 13: case 14: case 15: case 16: case 17: case 18: case 24: case 25:
        case 40: /* next-item width: reset by begin_frame, a no-op outside a frame */
        case 68: case 78: case 79: case 81: case 88: case 89: case 91:
        case 93: /* bind_active evaluates input state only */
        case 94:
            return 1;
        default:
            return 0;
    }
}

static void test_outside_frame(void) {
    RugiUi* ui = create();
    for (int i = 0; i < NUM_UI_CALLS; ++i) {
        if (i == 0 || i == 22 || i == 23) continue; /* begin_frame, font loads */
        int rc = ui_call(ui, i);
        int want = frame_free(i) ? RUGI_OK : RUGI_ERROR_INVALID_STATE;
        ++checks;
        if (rc != want) FAIL("outside frame: %s -> %d, want %d (%s)", ui_call_name[i], rc, want, last_msg());
    }
    /* An error from one call never leaks into the next valid call. */
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    RC(rugi_text(ui, "ok", 2, RUGI_TEXT_BODY), RUGI_OK);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    rugi_ui_destroy(ui);
}

static void test_inside_frame(void) {
    RugiUi* ui = create();
    /* Every call inside a frame, in a central panel: no crash, and each
     * returns a documented code. Containers opened here are auto-closed. */
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    for (int i = 2; i < NUM_UI_CALLS; ++i) {
        int rc = ui_call(ui, i);
        ++checks;
        if (rc < RUGI_OK || rc > RUGI_ERROR_VERSION_MISMATCH || rc == RUGI_ERROR_PANIC)
            FAIL("inside frame: %s -> %d (%s)", ui_call_name[i], rc, last_msg());
    }
    RC(rugi_ui_end_frame(ui, NULL), RUGI_ERROR_INVALID_STATE); /* unclosed / unbalanced */
    /* The next frame starts clean. */
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    rugi_ui_destroy(ui);
}

static void test_unbalanced(void) {
    RugiUi* ui = create();
    RC(rugi_ui_end_frame(ui, NULL), RUGI_ERROR_INVALID_STATE); /* end without begin */
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_ERROR_INVALID_STATE); /* begin twice */
    RC(rugi_end(ui), RUGI_ERROR_INVALID_STATE);                    /* end with nothing open */
    RC(rugi_pop_id(ui), RUGI_ERROR_INVALID_STATE);
    RC(rugi_pop_style(ui), RUGI_ERROR_INVALID_STATE);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    RC(rugi_begin_side_panel(ui, 100), RUGI_ERROR_INVALID_STATE); /* panel not at root */
    for (int i = 0; i < 1000; ++i) rugi_begin_vertical(ui);       /* deep nesting, never closed */
    for (int i = 0; i < 1000; ++i) rugi_push_id_int(ui, (uint64_t)i);
    for (int i = 0; i < 100; ++i) rugi_push_metric(ui, RUGI_METRIC_SPACING, (float)i);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_ERROR_INVALID_STATE);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_ERROR_INVALID_STATE); /* end twice */
    /* Recovered: a balanced frame is clean. */
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    RC(rugi_push_id(ui, "a", 1), RUGI_OK);
    RC(rugi_pop_id(ui), RUGI_OK);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_end(ui), RUGI_ERROR_INVALID_STATE);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    rugi_ui_destroy(ui);
}

/* --------------------------------------------------------------------------
 * Arguments
 * ------------------------------------------------------------------------ */

static void test_null_pointer_args(RugiUi* ui) {
    RugiColor c = {0, 0, 0, 255};
    RugiRect r = {0, 0, 10, 10};
    uint32_t u = 0;
    RugiStr items[2] = {{"a", 1}, {"b", 1}};
    RC(rugi_ui_begin_frame(ui, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    RC(rugi_ui_frame_stats(ui, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_display_info(ui, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_text(ui, NULL, 1), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_text(ui, NULL, 0), RUGI_OK);
    RC(rugi_ui_paste(ui, NULL, 5), RUGI_ERROR_NULL_POINTER);
    VAL(rugi_ui_clipboard_text(ui, NULL, 0), 0, RUGI_OK);
    RC(rugi_ui_add_font(ui, NULL, 0, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_add_font(ui, NULL, 10, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_add_font(ui, (const uint8_t*)"x", 0, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_add_font_file(ui, NULL, 4, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_add_font_file(ui, NULL, 0, NULL), RUGI_ERROR_INVALID_ARGUMENT); /* "" is not a file */
    /* A begin that rejects its arguments still opens an empty group, so the
     * caller's unconditional rugi_end stays balanced. */
    RC(rugi_begin_scroll_area(ui, NULL, 3, 10), RUGI_ERROR_NULL_POINTER);
    RC(rugi_end(ui), RUGI_OK);
    VAL(rugi_begin_collapsing(ui, NULL, 2, 0), 0, RUGI_ERROR_NULL_POINTER);
    RC(rugi_push_id(ui, NULL, 1), RUGI_ERROR_NULL_POINTER);
    RC(rugi_text(ui, NULL, 1, RUGI_TEXT_BODY), RUGI_ERROR_NULL_POINTER);
    RC(rugi_text(ui, NULL, 0, RUGI_TEXT_BODY), RUGI_OK);
    RC(rugi_text_ex(ui, NULL, 1, 0, 12, 400, c), RUGI_ERROR_NULL_POINTER);
    RC(rugi_text_truncated(ui, NULL, 1, 0), RUGI_ERROR_NULL_POINTER);
    RC(rugi_text_centered(ui, NULL, 1, 0), RUGI_ERROR_NULL_POINTER);
    RC(rugi_paragraph(ui, NULL, 1), RUGI_ERROR_NULL_POINTER);
    RC(rugi_badge(ui, NULL, 1, c), RUGI_ERROR_NULL_POINTER);
    VAL(rugi_button(ui, NULL, 1, 0, RUGI_ICON_NONE), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_button(ui, NULL, 0, 0, RUGI_ICON_NONE), 0, RUGI_OK);
    VAL(rugi_nav_item(ui, NULL, 1, RUGI_ICON_NONE, 0), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_icon_button(ui, NULL, 1, RUGI_ICON_PLUS, 20, NULL, 0), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_icon_button(ui, "i", 1, RUGI_ICON_PLUS, 20, NULL, 3), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_rail_item(ui, NULL, 1, RUGI_ICON_PLUS, false, NULL, 0), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_rail_item(ui, "r", 1, RUGI_ICON_PLUS, false, NULL, 2), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_checkbox(ui, "c", 1, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_toggle(ui, "t", 1, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_radio(ui, "r", 1, NULL, 0), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_slider_float(ui, "s", 1, NULL, 0, 1), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_spin_box(ui, "s", 1, NULL, 0, 1, 1), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_text_input(ui, "t", 1, NULL, 10, NULL, 0), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_text_input(ui, NULL, 1, NULL, 10, NULL, 0), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_text_input(ui, "t", 1, NULL, 0, NULL, 4), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_text_input_ex(ui, "t", 1, NULL, 10, NULL, 0, 0, RUGI_ICON_NONE), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_combo_box(ui, "c", 1, NULL, items, 2), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_combo_box(ui, "c", 1, &u, NULL, 2), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_combo_box(ui, "c", 1, &u, NULL, 0), 0, RUGI_OK);
    VAL(rugi_tabs(ui, "t", 1, NULL, items, 2), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_tabs(ui, "t", 1, &u, NULL, 1), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_segmented(ui, "s", 1, NULL, items, 2), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_segmented(ui, "s", 1, &u, NULL, 1), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_selectable(ui, NULL, 1, 0), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_multi_select(ui, "m", 1, NULL, items, 2), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_multi_select(ui, "m", 1, NULL, NULL, 0), 0, RUGI_OK);
    VAL(rugi_page_dots(ui, "p", 1, 3, NULL), 0, RUGI_ERROR_NULL_POINTER);
    RC(rugi_last_item(ui, NULL), RUGI_ERROR_NULL_POINTER);
    VAL(rugi_begin_menu(ui, NULL, 1), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_begin_context_menu(ui, NULL, 1), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_menu_item(ui, NULL, 1, NULL, 0), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_menu_item(ui, "m", 1, NULL, 1), 0, RUGI_ERROR_NULL_POINTER);
    RC(rugi_tooltip(ui, NULL, 1), RUGI_ERROR_NULL_POINTER);
    VAL(rugi_begin_modal(ui, "m", 1, "t", 1, NULL, 100), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_begin_modal(ui, NULL, 1, "t", 1, NULL, 100), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_table(ui, "t", 1, items, NULL, 2, 5, NULL, 100, NULL, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_table(ui, "t", 1, NULL, NULL, 2, 5, NULL, 100, noop_cell, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_table(ui, NULL, 1, items, NULL, 2, 5, NULL, 100, noop_cell, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_table(ui, "t", 1, items, NULL, 2, 5, NULL, 100, noop_cell, NULL), 0, RUGI_OK); /* selected NULL ok */
    VAL(rugi_carousel(ui, "c", 1, 3, NULL, 10, 10, noop_card, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_carousel(ui, "c", 1, 3, &u, 10, 10, NULL, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_command_palette(ui, NULL, NULL, 2, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_command_palette(ui, items, NULL, 2, NULL), 0, RUGI_OK); /* out_index may be NULL */
    VAL(rugi_command_palette(ui, NULL, NULL, 0, NULL), 0, RUGI_OK);
    RC(rugi_backdrop(ui, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_backdrop_rect(ui, r, 2, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_glass(ui, r, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_begin_glass_panel(ui, r, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_begin_glass_card(ui, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_begin_transition(ui, NULL, 1, 0, 0), RUGI_ERROR_NULL_POINTER);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_screen_rect(ui, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_title_bar(ui, NULL, 1, 30), RUGI_ERROR_NULL_POINTER);
    VAL(rugi_keybind(ui, "k", 1, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_keybind(ui, NULL, 1, NULL), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_bind_active(ui, NULL), 0, RUGI_ERROR_NULL_POINTER);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
}

static void test_strings(RugiUi* ui) {
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    /* Invalid, truncated-multibyte and embedded-NUL strings. */
    RC(rugi_text(ui, BAD_UTF8, 3, RUGI_TEXT_BODY), RUGI_ERROR_INVALID_UTF8);
    RC(rugi_text(ui, CUT_UTF8, 2, RUGI_TEXT_BODY), RUGI_ERROR_INVALID_UTF8);
    RC(rugi_text(ui, CUT_UTF8, 3, RUGI_TEXT_BODY), RUGI_OK);
    RC(rugi_text(ui, "a\0b", 3, RUGI_TEXT_BODY), RUGI_OK);
    RC(rugi_text(ui, "\xed\xa0\x80", 3, RUGI_TEXT_BODY), RUGI_ERROR_INVALID_UTF8); /* surrogate */
    RC(rugi_text(ui, "\xc0\x80", 2, RUGI_TEXT_BODY), RUGI_ERROR_INVALID_UTF8);     /* overlong */
    RC(rugi_ui_text(ui, BAD_UTF8, 3), RUGI_ERROR_INVALID_UTF8);
    RC(rugi_ui_paste(ui, CUT_UTF8, 2), RUGI_ERROR_INVALID_UTF8);
    RC(rugi_ui_text(ui, "\x01\x02\x7f\t\n", 5), RUGI_OK); /* control characters ignored */
    RC(rugi_push_id(ui, BAD_UTF8, 1), RUGI_ERROR_INVALID_UTF8);
    RC(rugi_tooltip(ui, BAD_UTF8, 2), RUGI_ERROR_INVALID_UTF8);
    VAL(rugi_button(ui, BAD_UTF8, 3, 0, RUGI_ICON_NONE), 0, RUGI_ERROR_INVALID_UTF8);
    VAL(rugi_menu_item(ui, "m", 1, BAD_UTF8, 3), 0, RUGI_ERROR_INVALID_UTF8);
    RC(rugi_ui_add_font_file(ui, BAD_UTF8, 3, NULL), RUGI_ERROR_INVALID_UTF8);
    /* Strings that are not NUL-terminated, at the very end of a heap block:
     * ASan reports any read past `len` (e.g. a strlen in the library). */
    char* raw3 = (char*)malloc(3);
    memcpy(raw3, "abc", 3);
    RC(rugi_text(ui, raw3, 3, RUGI_TEXT_BODY), RUGI_OK);
    RC(rugi_push_id(ui, raw3, 3), RUGI_OK);
    RC(rugi_pop_id(ui), RUGI_OK);
    RC(rugi_ui_text(ui, raw3, 3), RUGI_OK);
    RC(rugi_ui_add_font_file(ui, raw3, 3, NULL), RUGI_ERROR_INVALID_ARGUMENT); /* no such file */
    VAL(rugi_lucide_icon(raw3, 3, NULL, 0), 0, RUGI_OK);
    RugiStr raw_items[1] = {{raw3, 3}};
    uint32_t raw_sel = 0;
    VAL(rugi_tabs(ui, "rt", 2, &raw_sel, raw_items, 1), 0, RUGI_OK);
    free(raw3);
    /* Lengths that cannot describe a real object (> PTRDIFF_MAX). */
    RC(rugi_text(ui, "abc", HUGE_LEN, RUGI_TEXT_BODY), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_text(ui, "abc", OVER_ISIZE, RUGI_TEXT_BODY), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_text(ui, "abc", HUGE_LEN), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_push_id(ui, "abc", HUGE_LEN), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_add_font(ui, (const uint8_t*)"abc", HUGE_LEN, NULL), RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_button(ui, "b", HUGE_LEN, 0, RUGI_ICON_NONE), 0, RUGI_ERROR_INVALID_ARGUMENT);
    /* RugiStr arrays: NULL entry with length, bad UTF-8 entry, huge count. */
    uint32_t sel = 0;
    RugiStr bad_null[2] = {{"a", 1}, {NULL, 3}};
    RugiStr bad_utf8[2] = {{"a", 1}, {BAD_UTF8, 3}};
    RugiStr bad_len[2] = {{"a", 1}, {"b", HUGE_LEN}};
    RugiStr empty[2] = {{NULL, 0}, {"", 0}};
    VAL(rugi_combo_box(ui, "c", 1, &sel, bad_null, 2), 0, RUGI_ERROR_NULL_POINTER);
    VAL(rugi_tabs(ui, "t", 1, &sel, bad_utf8, 2), 0, RUGI_ERROR_INVALID_UTF8);
    VAL(rugi_segmented(ui, "s", 1, &sel, bad_len, 2), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_segmented(ui, "s", 1, &sel, empty, 2), 0, RUGI_OK);
    VAL(rugi_combo_box(ui, "c", 1, &sel, empty, HUGE_LEN), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_command_palette(ui, empty, bad_utf8, 2, NULL), 0, RUGI_ERROR_INVALID_UTF8);
    VAL(rugi_command_palette(ui, empty, NULL, HUGE_LEN, NULL), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_command_palette(ui, empty, NULL, HUGE_LEN / sizeof(RugiStr) + 1, NULL), 0, RUGI_ERROR_INVALID_ARGUMENT);
    bool flags[2] = {false, false};
    VAL(rugi_multi_select(ui, "m", 1, flags, empty, HUGE_LEN), 0, RUGI_ERROR_INVALID_ARGUMENT);
    /* Misaligned RugiStr array. */
    _Alignas(16) unsigned char raw[sizeof(RugiStr) * 2 + 8];
    memcpy(raw + 4, empty, sizeof empty);
    VAL(rugi_tabs(ui, "t", 1, &sel, (const RugiStr*)(raw + 4), 2), 0, RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
}

static void test_text_buffers(RugiUi* ui) {
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    /* Exactly-sized heap buffers so ASan catches any write past buf_size. */
    char* one = (char*)malloc(1);
    one[0] = 0;
    VAL(rugi_text_input(ui, "t1", 2, one, 1, NULL, 0), 0, RUGI_OK);
    CHECK(one[0] == 0);
    one[0] = 'x'; /* not NUL-terminated within buf_size */
    VAL(rugi_text_input(ui, "t1", 2, one, 1, NULL, 0), 0, RUGI_ERROR_INVALID_ARGUMENT);
    CHECK(one[0] == 'x');
    free(one);
    char* zero = (char*)malloc(1);
    VAL(rugi_text_input(ui, "t0", 2, zero, 0, NULL, 0), 0, RUGI_ERROR_NULL_POINTER);
    free(zero);
    char* bad = (char*)malloc(4);
    memcpy(bad, "\xff\xfe\x00", 4);
    VAL(rugi_text_input(ui, "tb", 2, bad, 4, NULL, 0), 0, RUGI_ERROR_INVALID_UTF8);
    free(bad);
    char* big = (char*)malloc(8);
    memcpy(big, "abc", 4);
    VAL(rugi_text_input(ui, "tg", 2, big, HUGE_LEN, NULL, 0), 0, RUGI_ERROR_INVALID_ARGUMENT);
    free(big);
    /* Clipboard into 0- and 1-byte buffers. */
    char* c1 = (char*)malloc(1);
    VAL(rugi_ui_clipboard_text(ui, c1, 1), 0, RUGI_OK);
    free(c1);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);

    /* Type into a focused 4-byte buffer: the edit is truncated at a character
     * boundary and never written past the buffer. */
    char* four = (char*)malloc(4);
    four[0] = 0;
    RugiItemState st;
    for (int frame = 0; frame < 4; ++frame) {
        if (frame == 1) {
            rugi_ui_mouse_move(ui, st.rect.x + 5, st.rect.y + st.rect.h / 2);
            rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 1);
            rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 0);
        }
        if (frame == 2) rugi_ui_text(ui, "\xc3\xa9\xc3\xa9\xc3\xa9", 6); /* "ééé" */
        if (frame == 3) {
            /* Copy the focused text into a 1-byte and 2-byte clipboard buffer. */
            rugi_ui_key(ui, RUGI_KEY_A, 1, RUGI_MOD_CTRL | RUGI_MOD_LOGO);
            rugi_ui_key(ui, RUGI_KEY_C, 1, RUGI_MOD_CTRL | RUGI_MOD_LOGO);
        }
        rugi_ui_begin_frame(ui, &FRAME);
        rugi_begin_central_panel(ui);
        rugi_text_input(ui, "four", 4, four, 4, NULL, 0);
        rugi_last_item(ui, &st);
        rugi_end(ui);
        RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    }
    CHECK(strcmp(four, "\xc3\xa9") == 0); /* "é" + NUL = 3 bytes; a second é would not fit */
    size_t need = rugi_ui_clipboard_text(ui, NULL, 0);
    CHECK(need == 3);
    char* c2 = (char*)malloc(2);
    CHECK(rugi_ui_clipboard_text(ui, c2, 2) == need);
    CHECK(c2[0] == 0); /* "é" does not fit in 1 byte + NUL: truncated at a boundary */
    free(c2);
    free(four);
}

static void test_enums_and_indices(RugiUi* ui) {
    RugiColor c = {1, 2, 3, 4};
    RugiRect r = {0, 0, 10, 10};
    RC(rugi_ui_set_theme(ui, 6), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_color(ui, 19, c), RUGI_ERROR_INVALID_ARGUMENT);
    RugiColor got = {0, 0, 0, 0};
    RC(rugi_ui_color(ui, 19, &got), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_color(ui, RUGI_COLOR_TEXT, NULL), RUGI_ERROR_NULL_POINTER);
    RC(rugi_ui_set_color(ui, RUGI_COLOR_SHADOW, c), RUGI_OK);
    RC(rugi_ui_color(ui, RUGI_COLOR_SHADOW, &got), RUGI_OK);
    CHECK(got.r == c.r && got.g == c.g && got.b == c.b && got.a == c.a);
    RC(rugi_ui_set_metric(ui, 12, 1), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_mouse_button(ui, 3, 1), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_key(ui, UINT32_MAX, 1, 0), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_key(ui, RUGI_KEY_TAB, 0xff, 0xffffffffu), RUGI_OK); /* any non-zero pressed, unknown mods ignored */
    RC(rugi_set_demo_page(ui, 9), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_window_command(ui, 4), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_push_color(ui, UINT32_MAX, c), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_push_metric(ui, UINT32_MAX, 1), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_begin_horizontal(ui, 2), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_begin_transition(ui, "t", 1, 0, 4), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    RC(rugi_text(ui, "t", 1, 6), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_text_truncated(ui, "t", 1, UINT32_MAX), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_text_centered(ui, "t", 1, 6), RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_button(ui, "b", 1, 4, RUGI_ICON_NONE), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_button(ui, "b", 1, 0, 9), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_button(ui, "b", 1, 0, RUGI_ICON_GLYPH_FLAG | 0xD800), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_button(ui, "b", 1, 0, RUGI_ICON_GLYPH_FLAG | 0x110000), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_button(ui, "b", 1, 0, RUGI_ICON_GLYPH_FLAG | 0x7ffffffe), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_button(ui, "b", 1, 0, RUGI_ICON_GLYPH_FLAG | 0x41), 0, RUGI_OK);
    VAL(rugi_button(ui, "b", 1, 0, RUGI_ICON_GLYPH_FLAG), 0, RUGI_OK); /* U+0000 glyph */
    VAL(rugi_nav_item(ui, "n", 1, 1000, 0xff), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_icon_button(ui, "i", 1, RUGI_ICON_NONE, 20, NULL, 0), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_rail_item(ui, "r", 1, RUGI_ICON_NONE, true, NULL, 0), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_text_input_ex(ui, "t", 1, (char[4]){0}, 4, NULL, 0, 0xffffffffu, 12345), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_text_input_ex(ui, "t", 1, (char[4]){0}, 4, NULL, 0, 0xffffffffu, RUGI_ICON_NONE), 0, RUGI_OK);
    /* Indices out of range for their item count are clamped, never read OOB. */
    RugiStr items[3] = {{"a", 1}, {"b", 1}, {"c", 1}};
    uint32_t idx = 1000;
    VAL(rugi_combo_box(ui, "cb", 2, &idx, items, 3), 0, RUGI_OK);
    idx = UINT32_MAX;
    VAL(rugi_tabs(ui, "tb", 2, &idx, items, 3), 0, RUGI_OK);
    idx = UINT32_MAX;
    VAL(rugi_segmented(ui, "sg", 2, &idx, items, 3), 0, RUGI_OK);
    idx = 5;
    VAL(rugi_segmented(ui, "sg0", 3, &idx, items, 0), 0, RUGI_OK);
    VAL(rugi_combo_box(ui, "cb0", 3, &idx, items, 0), 0, RUGI_OK);
    VAL(rugi_tabs(ui, "tb0", 3, &idx, items, 0), 0, RUGI_OK);
    idx = 50;
    VAL(rugi_page_dots(ui, "pd", 2, 3, &idx), 0, RUGI_OK);
    VAL(rugi_page_dots(ui, "pd0", 3, 0, &idx), 0, RUGI_OK);
    /* page_dots does O(count) work per frame in rugi-ui, so SIZE_MAX would
     * not return in reasonable time; a large finite count is used instead. */
    VAL(rugi_page_dots(ui, "pdh", 3, 5000, &idx), 0, RUGI_OK);
    RC(rugi_begin_columns(ui, 0), RUGI_OK);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_begin_columns(ui, UINT32_MAX), RUGI_OK);
    RC(rugi_text(ui, "x", 1, 0), RUGI_OK);
    RC(rugi_end(ui), RUGI_OK);
    int32_t radio = INT32_MIN;
    VAL(rugi_radio(ui, "r", 1, &radio, INT32_MAX), 0, RUGI_OK);
    /* Unregistered font handles fall back, never index out of bounds. */
    RC(rugi_text_ex(ui, "f", 1, UINT32_MAX, 14, 0, c), RUGI_OK);
    RC(rugi_text_ex(ui, "f", 1, 7, 14, 65535, c), RUGI_OK);
    RC(rugi_push_font(ui, UINT32_MAX), RUGI_OK);
    RC(rugi_text(ui, "f", 1, 0), RUGI_OK);
    RC(rugi_pop_style(ui), RUGI_OK);
    /* Bindings with bad kind/mode/code. */
    RugiKeyBind kb;
    memset(&kb, 0, sizeof kb);
    kb.kind = 3;
    VAL(rugi_keybind(ui, "k", 1, &kb), 0, RUGI_ERROR_INVALID_ARGUMENT);
    VAL(rugi_bind_active(ui, &kb), 0, RUGI_ERROR_INVALID_ARGUMENT);
    kb.kind = RUGI_BIND_MOUSE;
    kb.code = 7;
    VAL(rugi_bind_active(ui, &kb), 0, RUGI_ERROR_INVALID_ARGUMENT);
    kb.code = 0;
    kb.mode = 9;
    VAL(rugi_bind_active(ui, &kb), 0, RUGI_ERROR_INVALID_ARGUMENT);
    kb.mode = RUGI_BIND_ALWAYS;
    memset(&kb.toggled, 0xff, 1); /* non-canonical C bool */
    VAL(rugi_bind_active(ui, &kb), 1, RUGI_OK);
    /* Backdrop with bad kind. */
    RugiBackdrop bd;
    rugi_backdrop_preset(0, &bd);
    bd.kind = 6;
    RC(rugi_backdrop(ui, &bd), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_backdrop_rect(ui, r, 0, &bd), RUGI_ERROR_INVALID_ARGUMENT);
    /* Multi-select flags holding non-canonical bool bytes are normalized. */
    bool flags[3];
    memset(flags, 0x02, sizeof flags);
    VAL(rugi_multi_select(ui, "ms", 2, flags, items, 3), 0, RUGI_OK);
    unsigned char fb[3];
    memcpy(fb, flags, 3);
    CHECK(fb[0] <= 1 && fb[1] <= 1 && fb[2] <= 1);
    /* Non-canonical bools in checkbox / modal open. */
    bool b;
    memset(&b, 0x80, 1);
    VAL(rugi_checkbox(ui, "cbx", 3, &b), 0, RUGI_OK);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
}

static void test_floats(RugiUi* ui) {
    const float nan = NAN, inf = INFINITY;
    const float bad[] = {NAN, INFINITY, -INFINITY, -1e30f, 1e30f, -0.0f, 1e-45f};
    RugiColor c = {9, 9, 9, 255};
    /* Frame descriptors. */
    RugiFrameDesc fd = FRAME;
    fd.width = nan;
    RC(rugi_ui_begin_frame(ui, &fd), RUGI_ERROR_INVALID_ARGUMENT);
    fd.width = 800;
    fd.height = -inf;
    RC(rugi_ui_begin_frame(ui, &fd), RUGI_ERROR_INVALID_ARGUMENT);
    fd.height = 600;
    fd.scale = nan;
    RC(rugi_ui_begin_frame(ui, &fd), RUGI_ERROR_INVALID_ARGUMENT);
    fd.scale = 0;
    RC(rugi_ui_begin_frame(ui, &fd), RUGI_ERROR_INVALID_ARGUMENT);
    fd.scale = -2;
    RC(rugi_ui_begin_frame(ui, &fd), RUGI_ERROR_INVALID_ARGUMENT);
    fd.scale = inf;
    RC(rugi_ui_begin_frame(ui, &fd), RUGI_ERROR_INVALID_ARGUMENT);
    fd.scale = 1;
    fd.width = 0;
    fd.height = 0;
    fd.dt = nan; /* non-finite dt falls back to a default step */
    RC(rugi_ui_begin_frame(ui, &fd), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    /* Finite but huge sizes are clamped to 1e6 by the engine. */
    fd.width = 1e30f;
    fd.height = 1e30f;
    fd.dt = -inf;
    RC(rugi_ui_begin_frame(ui, &fd), RUGI_OK);
    RugiRect sr;
    RC(rugi_screen_rect(ui, &sr), RUGI_OK);
    CHECK(sr.w > 0 && sr.w <= 1e6f && sr.h > 0 && sr.h <= 1e6f);
    RC(rugi_show_demo(ui), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    /* Finite positive scales outside 1/16..16 are clamped, not rejected. */
    fd = FRAME;
    const float scales[] = {1e-30f, 1e-3f, 100.0f, 3e38f};
    for (size_t k = 0; k < sizeof scales / sizeof scales[0]; ++k) {
        fd.scale = scales[k];
        RC(rugi_ui_begin_frame(ui, &fd), RUGI_OK);
        RC(rugi_begin_central_panel(ui), RUGI_OK);
        RC(rugi_text(ui, "scaled", 6, RUGI_TEXT_BODY), RUGI_OK);
        RC(rugi_end(ui), RUGI_OK);
        RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    }
    /* NaN pointer/scroll events are ignored (the pointer keeps its previous
     * state): after leaving the window, a NaN move does not hover or click. */
    RugiItemState bs;
    memset(&bs, 0, sizeof bs);
    RC(rugi_ui_mouse_leave(ui), RUGI_OK);
    for (int f = 0; f < 2; ++f) {
        if (f == 1) {
            RC(rugi_ui_mouse_move(ui, nan, nan), RUGI_OK);
            RC(rugi_ui_scroll(ui, nan, inf), RUGI_OK);
            RC(rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 1), RUGI_OK);
            RC(rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 0), RUGI_OK);
        }
        RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
        RC(rugi_begin_central_panel(ui), RUGI_OK);
        VAL(rugi_button(ui, "nanbtn", 6, 0, RUGI_ICON_NONE), 0, RUGI_OK);
        RC(rugi_last_item(ui, &bs), RUGI_OK);
        RC(rugi_end(ui), RUGI_OK);
        RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    }
    CHECK(!bs.hovered && !bs.clicked);
    fd = FRAME;

    /* Settings outside a frame. */
    RC(rugi_ui_set_metric(ui, RUGI_METRIC_SPACING, nan), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_metric(ui, RUGI_METRIC_SPACING, inf), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_metric(ui, RUGI_METRIC_SPACING, -1), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_ambient_fps(ui, nan, 0), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_ambient_fps(ui, 0, -inf), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_ambient_fps(ui, 1e-30f, 1e30f), RUGI_OK);
    RC(rugi_ui_set_font(ui, RUGI_FONT_UI, nan), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_font(ui, RUGI_FONT_UI, inf), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_font(ui, RUGI_FONT_UI, 0), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_font(ui, RUGI_FONT_UI, -1), RUGI_ERROR_INVALID_ARGUMENT);
    RC(rugi_ui_set_font(ui, RUGI_FONT_UI, 1), RUGI_OK);

    /* Input events with non-finite coordinates, then a frame of every widget. */
    for (size_t k = 0; k < sizeof bad / sizeof bad[0]; ++k) {
        float v = bad[k];
        RC(rugi_ui_mouse_move(ui, v, v), RUGI_OK);
        RC(rugi_ui_scroll(ui, v, v), RUGI_OK);
        RC(rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 1), RUGI_OK);
        RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
        RugiRect r = {v, v, 10, 10};
        RugiRect rs = {0, 0, v, v};
        int rect_ok = isfinite(v);
        int size_ok = isfinite(v) && v >= 0;
        RC(rugi_begin_region(ui, r, false), rect_ok ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        RC(rugi_end(ui), RUGI_OK);
        RC(rugi_begin_region(ui, rs, true), size_ok ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        RC(rugi_end(ui), RUGI_OK);
        RC(rugi_begin_side_panel(ui, v), RUGI_OK);
        RC(rugi_end(ui), RUGI_OK);
        RC(rugi_begin_top_bar(ui, v), RUGI_OK);
        RC(rugi_end(ui), RUGI_OK);
        RC(rugi_begin_central_panel(ui), RUGI_OK);
        RC(rugi_space(ui, v), RUGI_OK);
        RC(rugi_set_next_item_width(ui, v), RUGI_OK);
        RC(rugi_text(ui, "w", 1, 0), RUGI_OK);
        RC(rugi_progress_bar(ui, v), RUGI_OK);
        int pos = isfinite(v) && v > 0;
        RC(rugi_text_ex(ui, "t", 1, 0, v, 400, c), pos ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        VAL(rugi_icon_button(ui, "i", 1, RUGI_ICON_PLUS, v, NULL, 0), 0, RUGI_OK);
        float fv = v;
        VAL(rugi_slider_float(ui, "s", 1, &fv, 0, 1), 0, RUGI_OK);
        VAL(rugi_slider_float(ui, "s2", 2, &fv, v, 1), 0, isfinite(v) ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        VAL(rugi_slider_float(ui, "s3", 2, &fv, 1, 0), 0, RUGI_OK); /* min > max */
        fv = v;
        VAL(rugi_spin_box(ui, "sp", 2, &fv, v, v, v), 0, isfinite(v) ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        fv = 1;
        VAL(rugi_spin_box(ui, "sp2", 3, &fv, 10, 0, v), 0, isfinite(v) ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        RC(rugi_begin_scroll_area(ui, "sa", 2, v), isfinite(v) ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        RC(rugi_text(ui, "in", 2, 0), RUGI_OK);
        ERR(rugi_end(ui), RUGI_OK); /* short scroll areas: see test_engine_regressions */
        RC(rugi_title_bar(ui, "t", 1, v), RUGI_OK);
        bool open = true;
        if (rugi_begin_modal(ui, "m", 1, "M", 1, &open, v)) rugi_end(ui);
        CHECK(rugi_last_error_code() == RUGI_OK);
        int64_t sel = -5;
        float widths[2] = {v, -v};
        RugiStr hs[2] = {{"a", 1}, {"b", 1}};
        ERR(rugi_table(ui, "tb", 2, hs, widths, 2, 1000, &sel, v, noop_cell, NULL),
               v == 0 ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT); /* a negative or non-finite weight */
        float ok_widths[2] = {1, 2};
        VAL(rugi_table(ui, "tb", 2, hs, ok_widths, 2, 1000, &sel, isfinite(v) ? 200 : v, noop_cell, NULL), 0,
            isfinite(v) ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        uint32_t ci = 0;
        VAL(rugi_carousel(ui, "c", 1, 3, &ci, v, 10, noop_card, NULL), 0,
            pos ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        RC(rugi_push_metric(ui, RUGI_METRIC_RADIUS_MD, v), isfinite(v) && v >= 0 ? RUGI_OK : RUGI_ERROR_INVALID_ARGUMENT);
        rugi_clear_error();
        rugi_pop_style(ui);
        RugiGlassStyle g;
        rugi_glass_style_preset(RUGI_GLASS_LIQUID, &g);
        g.blur = v;
        g.radius = v;
        g.refraction = v;
        g.saturation = v;
        RugiRect gr = {10, 10, 100, 100};
        RC(rugi_glass(ui, gr, &g), RUGI_OK);
        RugiBackdrop bd;
        rugi_backdrop_preset(RUGI_BACKDROP_FILAMENT, &bd);
        bd.scale = v;
        bd.speed = v;
        bd.seed = v;
        bd.intensity = v;
        RC(rugi_backdrop_rect(ui, gr, v, &bd), RUGI_OK);
        RC(rugi_end(ui), RUGI_OK);
        RC(rugi_backdrop(ui, &bd), RUGI_OK);
        RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
        RC(rugi_ui_mouse_button(ui, RUGI_MOUSE_LEFT, 0), RUGI_OK);
    }
    rugi_ui_mouse_move(ui, 0, 0);
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
}

static void test_big_counts(RugiUi* ui) {
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    RugiStr hs[1] = {{"a", 1}};
    int64_t sel = INT64_MAX;
    VAL(rugi_table(ui, "huge", 4, hs, NULL, 1, SIZE_MAX, &sel, 200, noop_cell, NULL), 0, RUGI_OK);
    sel = INT64_MIN;
    VAL(rugi_table(ui, "huge2", 5, hs, NULL, 1, SIZE_MAX / 2, &sel, 200, noop_cell, NULL), 0, RUGI_OK);
    CHECK(sel == -1);
    VAL(rugi_table(ui, "nocols", 6, NULL, NULL, 0, 10, &sel, 200, noop_cell, NULL), 0, RUGI_OK);
    VAL(rugi_table(ui, "cols", 4, hs, NULL, HUGE_LEN, 10, &sel, 200, noop_cell, NULL), 0, RUGI_ERROR_INVALID_ARGUMENT);
    uint32_t idx = UINT32_MAX;
    VAL(rugi_carousel(ui, "car", 3, SIZE_MAX, &idx, 50, 50, noop_card, NULL), 0, RUGI_OK);
    idx = 7;
    VAL(rugi_carousel(ui, "car0", 4, 0, &idx, 50, 50, noop_card, NULL), 0, RUGI_OK);
    RC(rugi_end(ui), RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
}

/* Inputs that made rugi-ui panic (caught as RUGI_ERROR_PANIC) when this
 * test was written; fixed in rugi-ui, kept as regression checks. */
static void test_engine_regressions(void) {
    RugiUi* ui = create();
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    RC(rugi_begin_central_panel(ui), RUGI_OK);
    /* A scroll area shorter than 32 px whose content overflows: the scrollbar
     * thumb clamp(24, height - 8) panicked (rugi-ui layout.rs). */
    RC(rugi_begin_scroll_area(ui, "short", 5, 20), RUGI_OK);
    for (int i = 0; i < 10; ++i) rugi_text(ui, "row", 3, 0);
    ERR(rugi_end(ui), RUGI_OK);
    /* The same through a table of height 0. */
    RugiStr hs[1] = {{"a", 1}};
    ERR(rugi_table(ui, "flat", 4, hs, NULL, 1, 100, NULL, 0, noop_cell, NULL), RUGI_OK);
    /* A huge finite height: the visible-row range ((offset + height) / row_h)
     * saturated to usize::MAX and `+ 1` overflowed (rugi-ui table). */
    ERR(rugi_table(ui, "tall", 4, hs, NULL, 1, 100, NULL, 1e30f, noop_cell, NULL), RUGI_OK);
    ERR(rugi_table(ui, "tall2", 5, hs, NULL, 1, SIZE_MAX, NULL, 3e38f, noop_cell, NULL), RUGI_OK);
    RC(rugi_begin_scroll_area(ui, "tallsa", 6, 3e38f), RUGI_OK);
    for (int i = 0; i < 10; ++i) rugi_text(ui, "row", 3, 0);
    ERR(rugi_end(ui), RUGI_OK);
    rugi_ui_end_frame(ui, NULL);
    RC(rugi_ui_destroy(ui), RUGI_OK);
}

/* --------------------------------------------------------------------------
 * Callbacks: re-entrancy and destructive calls
 * ------------------------------------------------------------------------ */

typedef struct {
    int depth;
    int destroy_rc;
    int begin_rc;
    int end_frame_rc;
    int nested;
    int calls;
    int extra_ends;
} CbState;

static void hostile_card(RugiUi* ui, size_t i, RugiRect r, float e, void* user);

static void hostile_cell(RugiUi* ui, size_t row, size_t col, void* user) {
    CbState* s = (CbState*)user;
    s->calls++;
    (void)col;
    if (row == 0 && col == 0) {
        s->destroy_rc = rugi_ui_destroy(ui);
        s->begin_rc = rugi_ui_begin_frame(ui, &FRAME);
        s->end_frame_rc = rugi_ui_end_frame(ui, NULL);
        /* Error state set inside the callback does not break the table. */
        rugi_text(ui, BAD_UTF8, 3, 0);
    }
    if (row == 1 && s->depth < 3) {
        /* Nested table and carousel on the same handle. */
        s->depth++;
        RugiStr hs[1] = {{"n", 1}};
        int64_t sel = 0;
        rugi_table(ui, "nested", 6, hs, NULL, 1, 3, &sel, 60, hostile_cell, s);
        uint32_t idx = 0;
        rugi_carousel(ui, "nc", 2, 2, &idx, 20, 20, hostile_card, s);
        s->nested++;
        s->depth--;
    }
    if (row == 2) {
        /* Unbalanced calls inside a cell must not corrupt the table. */
        for (int k = 0; k < s->extra_ends; ++k) rugi_end(ui);
        rugi_begin_vertical(ui);
        rugi_push_id(ui, "leak", 4);
        rugi_push_metric(ui, RUGI_METRIC_SPACING, 3);
    }
    rugi_text(ui, "cell", 4, 0);
}

static void hostile_card(RugiUi* ui, size_t i, RugiRect r, float e, void* user) {
    CbState* s = (CbState*)user;
    (void)e;
    s->calls++;
    if (i == 0) {
        s->destroy_rc = rugi_ui_destroy(ui);
        s->end_frame_rc = rugi_ui_end_frame(ui, NULL);
    }
    RugiBackdrop bd;
    rugi_backdrop_preset(RUGI_BACKDROP_PLASMA, &bd);
    rugi_backdrop_rect(ui, r, 4, &bd);
    if (s->depth < 2) {
        s->depth++;
        uint32_t idx = 0;
        rugi_carousel(ui, "inner", 5, 2, &idx, 10, 10, hostile_card, s);
        s->depth--;
    }
}

static void test_callbacks(void) {
    for (int extra = 0; extra <= 3; extra += 3) {
        RugiUi* ui = create();
        CbState s;
        memset(&s, 0, sizeof s);
        s.extra_ends = extra;
        RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
        RC(rugi_begin_central_panel(ui), RUGI_OK);
        RugiStr hs[2] = {{"A", 1}, {"B", 1}};
        int64_t sel = -1;
        rugi_clear_error();
        rugi_table(ui, "t", 1, hs, NULL, 2, 10, &sel, 300, hostile_cell, &s);
        CHECK(s.calls > 0);
        CHECK(s.nested > 0);
        CHECK(s.destroy_rc == RUGI_ERROR_INVALID_STATE);
        CHECK(s.begin_rc == RUGI_ERROR_INVALID_STATE);
        CHECK(s.end_frame_rc == RUGI_ERROR_INVALID_STATE);
        /* Handle still alive and usable. */
        RC(rugi_text(ui, "after", 5, 0), RUGI_OK);
        memset(&s, 0, sizeof s);
        uint32_t idx = 0;
        rugi_carousel(ui, "c", 1, 4, &idx, 60, 60, hostile_card, &s);
        CHECK(s.calls > 0);
        CHECK(s.destroy_rc == RUGI_ERROR_INVALID_STATE);
        CHECK(s.end_frame_rc == RUGI_ERROR_INVALID_STATE);
        RC(rugi_text(ui, "after", 5, 0), RUGI_OK);
        rugi_end(ui);
        rugi_ui_end_frame(ui, NULL); /* may report the unbalanced cell calls */
        /* The next frame is clean. */
        RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
        RC(rugi_begin_central_panel(ui), RUGI_OK);
        RC(rugi_end(ui), RUGI_OK);
        RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
        RC(rugi_ui_destroy(ui), RUGI_OK);
    }
}

/* --------------------------------------------------------------------------
 * Threads
 * ------------------------------------------------------------------------ */

static void* other_thread(void* arg) {
    RugiUi* ui = (RugiUi*)arg;
    all_calls_fail(ui, RUGI_ERROR_WRONG_THREAD, "other-thread");
    RC(rugi_ui_destroy(ui), RUGI_ERROR_WRONG_THREAD);
    /* The last error is thread-local: this thread's error is not visible on
     * the creating thread. */
    rugi_ui_mouse_move(NULL, 0, 0);
    /* A context created here is independent and destroyed here. */
    RugiUi* mine = NULL;
    if (rugi_ui_create(NULL, &mine) != RUGI_OK) ++failures;
    RC(rugi_ui_begin_frame(mine, &FRAME), RUGI_OK);
    RC(rugi_ui_end_frame(mine, NULL), RUGI_OK);
    RC(rugi_ui_destroy(mine), RUGI_OK);
    return NULL;
}

static void test_threads(void) {
    RugiUi* ui = create();
    RC(rugi_ui_begin_frame(ui, &FRAME), RUGI_OK);
    rugi_clear_error();
    pthread_t t;
    pthread_create(&t, NULL, other_thread, ui);
    pthread_join(t, NULL);
    CHECK(rugi_last_error_code() == RUGI_OK);
    RC(rugi_ui_end_frame(ui, NULL), RUGI_OK);
    RC(rugi_ui_destroy(ui), RUGI_OK);

    void* rc = NULL;
    pthread_create(&t, NULL, app_run_thread, NULL);
    pthread_join(t, &rc);
    ++checks;
    if ((intptr_t)rc != RUGI_ERROR_WRONG_THREAD && (intptr_t)rc != RUGI_ERROR_WINDOW)
        FAIL("rugi_app_run off the main thread -> %d", (int)(intptr_t)rc);
}

int main(void) {
    test_globals();
    test_app_run_args();
    test_null_and_dead_handles();
    test_outside_frame();
    test_inside_frame();
    test_unbalanced();
    RugiUi* ui = create();
    test_null_pointer_args(ui);
    test_strings(ui);
    test_text_buffers(ui);
    test_enums_and_indices(ui);
    test_floats(ui);
    test_big_counts(ui);
    RC(rugi_ui_destroy(ui), RUGI_OK);
    test_callbacks();
    test_threads();
    test_engine_regressions();
    printf("rugi hostile test: %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
