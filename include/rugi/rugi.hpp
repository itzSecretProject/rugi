// Rugi — C++17 wrapper over the C ABI (include/rugi/rugi.h).
//
// Header-only. Adds RAII ownership, std::string_view strings, scoped
// containers that always close (even if the body throws), and exception
// translation at the callback boundary so C++ exceptions never unwind through
// Rust frames.
#pragma once

#include "rugi.h"

#include <exception>
#include <initializer_list>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

namespace rugi {

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error carrying a RugiResult code and the library's message.
class Error : public std::runtime_error {
public:
    Error(RugiResult code, const std::string& msg) : std::runtime_error(msg), code_(code) {}
    RugiResult code() const noexcept { return code_; }

private:
    RugiResult code_;
};

/// The last error message recorded on this thread ("" if none).
inline std::string last_error_message() {
    size_t n = rugi_last_error_message(nullptr, 0);
    if (n == 0) return {};
    std::string s(n, '\0');
    rugi_last_error_message(s.data(), s.size());
    s.resize(n - 1);
    return s;
}

/// Throws rugi::Error when `r` is not RUGI_OK.
inline void check(RugiResult r) {
    if (r != RUGI_OK) throw Error(r, last_error_message());
}

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

enum class Theme : RugiTheme {
    Dark = RUGI_THEME_DARK,
    Light = RUGI_THEME_LIGHT,
    Rose = RUGI_THEME_ROSE,
    Mocha = RUGI_THEME_MOCHA,
    Forest = RUGI_THEME_FOREST,
    Graphite = RUGI_THEME_GRAPHITE,
};

enum class Present : RugiPresent {
    Vsync = RUGI_PRESENT_VSYNC,
    LowLatency = RUGI_PRESENT_LOW_LATENCY,
    Uncapped = RUGI_PRESENT_UNCAPPED,
};

enum class ButtonKind : RugiButtonKind {
    Primary = RUGI_BUTTON_PRIMARY,
    Secondary = RUGI_BUTTON_SECONDARY,
    Ghost = RUGI_BUTTON_GHOST,
    Danger = RUGI_BUTTON_DANGER,
};

enum class TextRole : RugiTextRole {
    Body = RUGI_TEXT_BODY,
    Muted = RUGI_TEXT_MUTED,
    Heading = RUGI_TEXT_HEADING,
    Title = RUGI_TEXT_TITLE,
    Caption = RUGI_TEXT_CAPTION,
    Display = RUGI_TEXT_DISPLAY,
};

enum class Icon : RugiIcon {
    None = RUGI_ICON_NONE,
    Dashboard = RUGI_ICON_DASHBOARD,
    Chart = RUGI_ICON_CHART,
    Sliders = RUGI_ICON_SLIDERS,
    Activity = RUGI_ICON_ACTIVITY,
    Layers = RUGI_ICON_LAYERS,
    Search = RUGI_ICON_SEARCH,
    Plus = RUGI_ICON_PLUS,
    Check = RUGI_ICON_CHECK,
    Folder = RUGI_ICON_FOLDER,
};

enum class Slide : RugiSlide {
    None = RUGI_SLIDE_NONE,
    Forward = RUGI_SLIDE_FORWARD,
    Back = RUGI_SLIDE_BACK,
    Up = RUGI_SLIDE_UP,
};

// ---------------------------------------------------------------------------
// Value helpers
// ---------------------------------------------------------------------------

/// Font handle (RUGI_FONT_UI, RUGI_FONT_MONO or from add_font).
using Font = RugiFont;

/// Glyph icon for icon parameters: `rugi::glyph(0xe14d)` or the
/// RUGI_ICON_LUCIDE_* constants from rugi_icons_lucide.h.
constexpr RugiIcon glyph(uint32_t codepoint) { return RUGI_ICON_GLYPH_FLAG | codepoint; }

inline RugiStr str(std::string_view s) { return RugiStr{s.data(), s.size()}; }

inline RugiRect rect(float x, float y, float w, float h) { return RugiRect{x, y, w, h}; }

/// Glass preset (frosted / liquid / smoked); tweak fields freely.
inline RugiGlassStyle glass_style(RugiGlassPreset preset = RUGI_GLASS_FROSTED) {
    RugiGlassStyle g{};
    check(rugi_glass_style_preset(preset, &g));
    return g;
}

/// Backdrop with its default palette; tweak fields freely.
inline RugiBackdrop backdrop_preset(RugiBackdropKind kind) {
    RugiBackdrop b{};
    check(rugi_backdrop_preset(kind, &b));
    return b;
}

/// A key binding bound to `key` (RUGI_KEY_*), held or toggled.
inline RugiKeyBind key_bind(RugiKey key, RugiBindMode mode = RUGI_BIND_HOLD) {
    RugiKeyBind b{};
    b.kind = RUGI_BIND_KEY;
    b.code = key;
    b.mode = mode;
    return b;
}

namespace detail {

inline std::vector<RugiStr> strs(std::initializer_list<std::string_view> list) {
    std::vector<RugiStr> v;
    v.reserve(list.size());
    for (auto s : list) v.push_back(str(s));
    return v;
}

/// Edits `value` through a NUL-terminated buffer of at least `capacity` bytes.
template <class Edit> bool edit_text(std::string& value, size_t capacity, Edit&& edit) {
    if (value.size() >= capacity) capacity = value.size() + 1;
    std::string buf(value);
    buf.resize(capacity, '\0');
    bool changed = edit(buf.data(), buf.size());
    if (changed) value.assign(buf.c_str());
    return changed;
}

/// Invokes a user callback from a C callback. The first exception is stored
/// (later calls are skipped) and rethrown once control is back in C++.
template <class F> struct Callback {
    F* fn;
    std::exception_ptr error = nullptr;

    template <class... Args> void operator()(Args&&... args) noexcept {
        if (error) return;
        try {
            (*fn)(std::forward<Args>(args)...);
        } catch (...) {
            error = std::current_exception();
        }
    }
    void rethrow() const {
        if (error) std::rethrow_exception(error);
    }
};

}  // namespace detail

// ---------------------------------------------------------------------------
// Ui
// ---------------------------------------------------------------------------

/// Non-owning view of a UI context, valid for the current frame. Widget calls
/// never throw; misuse is recorded and retrievable via last_error_message().
class Ui {
public:
    explicit Ui(RugiUi* handle) noexcept : h_(handle) {}
    RugiUi* handle() const noexcept { return h_; }

    // Text ------------------------------------------------------------------
    void text(std::string_view s, TextRole role = TextRole::Body) {
        rugi_text(h_, s.data(), s.size(), static_cast<RugiTextRole>(role));
    }
    void label(std::string_view s) { text(s, TextRole::Body); }
    void muted(std::string_view s) { text(s, TextRole::Muted); }
    void heading(std::string_view s) { text(s, TextRole::Heading); }
    void title(std::string_view s) { text(s, TextRole::Title); }
    void caption(std::string_view s) { text(s, TextRole::Caption); }
    void paragraph(std::string_view s) { rugi_paragraph(h_, s.data(), s.size()); }
    void badge(std::string_view s, RugiColor c) { rugi_badge(h_, s.data(), s.size(), c); }
    void text_truncated(std::string_view s, TextRole role = TextRole::Body) {
        rugi_text_truncated(h_, s.data(), s.size(), static_cast<RugiTextRole>(role));
    }
    void text_centered(std::string_view s, TextRole role = TextRole::Body) {
        rugi_text_centered(h_, s.data(), s.size(), static_cast<RugiTextRole>(role));
    }
    void text_ex(std::string_view s, Font font, float size, uint16_t weight, RugiColor color) {
        rugi_text_ex(h_, s.data(), s.size(), font, size, weight, color);
    }

    // Buttons ---------------------------------------------------------------
    bool button(std::string_view label, ButtonKind kind = ButtonKind::Primary, Icon icon = Icon::None) {
        return button(label, kind, static_cast<RugiIcon>(icon));
    }
    /// Takes a raw RugiIcon (e.g. RUGI_ICON_LUCIDE_SAVE).
    bool button(std::string_view label, ButtonKind kind, RugiIcon icon) {
        return rugi_button(h_, label.data(), label.size(), static_cast<RugiButtonKind>(kind), icon);
    }
    bool nav_item(std::string_view label, Icon icon, bool selected) {
        return nav_item(label, static_cast<RugiIcon>(icon), selected);
    }
    /// Takes a raw RugiIcon (e.g. RUGI_ICON_LUCIDE_SAVE).
    bool nav_item(std::string_view label, RugiIcon icon, bool selected) {
        return rugi_nav_item(h_, label.data(), label.size(), icon, selected ? 1 : 0);
    }
    bool icon_button(std::string_view id, RugiIcon icon, float size = 32.0f, std::string_view tooltip = {}) {
        return rugi_icon_button(h_, id.data(), id.size(), icon, size, tooltip.data(), tooltip.size());
    }
    bool rail_item(std::string_view id, RugiIcon icon, bool selected, std::string_view tooltip = {}) {
        return rugi_rail_item(h_, id.data(), id.size(), icon, selected, tooltip.data(), tooltip.size());
    }

    // Value editors ---------------------------------------------------------
    bool checkbox(std::string_view label, bool& value) {
        return rugi_checkbox(h_, label.data(), label.size(), &value);
    }
    bool toggle(std::string_view label, bool& value) { return rugi_toggle(h_, label.data(), label.size(), &value); }
    bool radio(std::string_view label, int32_t& value, int32_t option) {
        return rugi_radio(h_, label.data(), label.size(), &value, option);
    }
    bool slider(std::string_view label, float& value, float min, float max) {
        return rugi_slider_float(h_, label.data(), label.size(), &value, min, max);
    }
    bool spin_box(std::string_view label, float& value, float min, float max, float step) {
        return rugi_spin_box(h_, label.data(), label.size(), &value, min, max, step);
    }
    /// Edits `value` in place. `capacity` bounds the UTF-8 byte length.
    bool text_input(std::string_view id, std::string& value, std::string_view hint = {}, size_t capacity = 256) {
        return detail::edit_text(value, capacity, [&](char* buf, size_t size) {
            return rugi_text_input(h_, id.data(), id.size(), buf, size, hint.data(), hint.size());
        });
    }
    /// text_input with RUGI_INPUT_* flags (e.g. RUGI_INPUT_PASSWORD) and a leading icon.
    bool text_input_ex(std::string_view id, std::string& value, std::string_view hint, RugiInputFlags flags,
                       RugiIcon icon = RUGI_ICON_NONE, size_t capacity = 256) {
        return detail::edit_text(value, capacity, [&](char* buf, size_t size) {
            return rugi_text_input_ex(h_, id.data(), id.size(), buf, size, hint.data(), hint.size(), flags, icon);
        });
    }
    bool keybind(std::string_view label, RugiKeyBind& bind) {
        return rugi_keybind(h_, label.data(), label.size(), &bind);
    }
    bool bind_active(RugiKeyBind& bind) { return rugi_bind_active(h_, &bind); }

    // Selectors -------------------------------------------------------------
    bool combo_box(std::string_view id, uint32_t& selected, std::initializer_list<std::string_view> items) {
        auto v = detail::strs(items);
        return rugi_combo_box(h_, id.data(), id.size(), &selected, v.data(), v.size());
    }
    bool tabs(std::string_view id, uint32_t& selected, std::initializer_list<std::string_view> labels) {
        auto v = detail::strs(labels);
        return rugi_tabs(h_, id.data(), id.size(), &selected, v.data(), v.size());
    }
    bool segmented(std::string_view id, uint32_t& selected, std::initializer_list<std::string_view> labels) {
        auto v = detail::strs(labels);
        return rugi_segmented(h_, id.data(), id.size(), &selected, v.data(), v.size());
    }
    /// `flags` must have one entry per item.
    bool multi_select(std::string_view label, bool* flags, std::initializer_list<std::string_view> items) {
        auto v = detail::strs(items);
        return rugi_multi_select(h_, label.data(), label.size(), flags, v.data(), v.size());
    }
    bool selectable(std::string_view label, bool selected) {
        return rugi_selectable(h_, label.data(), label.size(), selected ? 1 : 0);
    }
    bool page_dots(std::string_view id, size_t count, uint32_t& index) {
        return rugi_page_dots(h_, id.data(), id.size(), count, &index);
    }
    void progress_bar(float fraction) { rugi_progress_bar(h_, fraction); }

    // Layout (containers close automatically, even on exceptions) -----------
    template <class F> void side_panel(float width, F&& f) {
        rugi_begin_side_panel(h_, width);
        scoped(f);
    }
    template <class F> void top_bar(float height, F&& f) {
        rugi_begin_top_bar(h_, height);
        scoped(f);
    }
    template <class F> void central_panel(F&& f) {
        rugi_begin_central_panel(h_);
        scoped(f);
    }
    template <class F> void vertical(F&& f) {
        rugi_begin_vertical(h_);
        scoped(f);
    }
    template <class F> void horizontal(F&& f) {
        rugi_begin_horizontal(h_, RUGI_ALIGN_START);
        scoped(f);
    }
    template <class F> void horizontal_right(F&& f) {
        rugi_begin_horizontal(h_, RUGI_ALIGN_END);
        scoped(f);
    }
    template <class F> void columns(uint32_t n, F&& f) {
        rugi_begin_columns(h_, n);
        scoped(f);
    }
    template <class F> void card(F&& f) {
        rugi_begin_card(h_);
        scoped(f);
    }
    template <class F> void scroll_area(std::string_view id, float height, F&& f) {
        rugi_begin_scroll_area(h_, id.data(), id.size(), height);
        scoped(f);
    }
    template <class F> void region(RugiRect r, bool horizontal, F&& f) {
        rugi_begin_region(h_, r, horizontal);
        scoped(f);
    }
    template <class F> void with_id(uint64_t id, F&& f) {
        rugi_push_id_int(h_, id);
        scoped(f, rugi_pop_id);
    }
    void separator() { rugi_separator(h_); }
    void space(float amount) { rugi_space(h_, amount); }
    void set_next_item_width(float w) { rugi_set_next_item_width(h_, w); }

    // Popups (the body runs, and the popup is closed, only while it is open) -
    template <class F> bool menu(std::string_view label, F&& f) {
        if (!rugi_begin_menu(h_, label.data(), label.size())) return false;
        scoped(f);
        return true;
    }
    template <class F> bool context_menu(std::string_view id, F&& f) {
        if (!rugi_begin_context_menu(h_, id.data(), id.size())) return false;
        scoped(f);
        return true;
    }
    template <class F> bool modal(std::string_view id, std::string_view title, bool& open, float width, F&& f) {
        if (!rugi_begin_modal(h_, id.data(), id.size(), title.data(), title.size(), &open, width)) return false;
        scoped(f);
        return true;
    }
    template <class F> bool collapsing(std::string_view label, bool default_open, F&& f) {
        if (!rugi_begin_collapsing(h_, label.data(), label.size(), default_open ? 1 : 0)) return false;
        scoped(f);
        return true;
    }
    bool menu_item(std::string_view label, std::string_view shortcut = {}) {
        return rugi_menu_item(h_, label.data(), label.size(), shortcut.data(), shortcut.size());
    }
    void menu_separator() { rugi_menu_separator(h_); }
    void tooltip(std::string_view s) { rugi_tooltip(h_, s.data(), s.size()); }

    // Callback widgets ------------------------------------------------------
    /// Virtualized table: `cell(ui, row, col)` runs only for visible rows.
    /// Exceptions thrown by `cell` are rethrown after the table finishes.
    template <class F>
    bool table(std::string_view id, std::initializer_list<std::string_view> headers,
               std::initializer_list<float> widths, size_t rows, std::optional<size_t>& selected, float height,
               F&& cell) {
        auto hs = detail::strs(headers);
        std::vector<float> ws(widths);
        if (!ws.empty() && ws.size() != hs.size()) ws.clear();
        detail::Callback<std::remove_reference_t<F>> cb{&cell};
        auto tramp = [](RugiUi* ui, size_t row, size_t col, void* user) noexcept {
            (*static_cast<decltype(cb)*>(user))(Ui(ui), row, col);
        };
        int64_t sel = selected ? static_cast<int64_t>(*selected) : -1;
        bool changed = rugi_table(h_, id.data(), id.size(), hs.data(), ws.empty() ? nullptr : ws.data(), hs.size(),
                                   rows, &sel, height, tramp, &cb);
        selected = sel >= 0 ? std::optional<size_t>(static_cast<size_t>(sel)) : std::nullopt;
        cb.rethrow();
        return changed;
    }
    /// Card carousel; `card(ui, index, rect, emphasis)` paints each visible card.
    /// Exceptions thrown by `card` are rethrown afterwards.
    template <class F>
    bool carousel(std::string_view id, size_t count, uint32_t& index, float card_w, float card_h, F&& card) {
        detail::Callback<std::remove_reference_t<F>> cb{&card};
        auto tramp = [](RugiUi* ui, size_t i, RugiRect r, float e, void* user) noexcept {
            (*static_cast<decltype(cb)*>(user))(Ui(ui), i, r, e);
        };
        bool changed = rugi_carousel(h_, id.data(), id.size(), count, &index, card_w, card_h, tramp, &cb);
        cb.rethrow();
        return changed;
    }

    // Effects and window chrome ---------------------------------------------
    void backdrop(const RugiBackdrop& b) { rugi_backdrop(h_, &b); }
    void backdrop_rect(RugiRect r, float radius, const RugiBackdrop& b) { rugi_backdrop_rect(h_, r, radius, &b); }
    void glass(RugiRect r, const RugiGlassStyle& g) { rugi_glass(h_, r, &g); }
    template <class F> void glass_panel(RugiRect r, const RugiGlassStyle& g, F&& f) {
        rugi_begin_glass_panel(h_, r, &g);
        scoped(f);
    }
    template <class F> void glass_card(const RugiGlassStyle& g, F&& f) {
        rugi_begin_glass_card(h_, &g);
        scoped(f);
    }
    /// Content fades/slides in whenever `key` changes.
    template <class F> void transition(std::string_view id, uint64_t key, Slide slide, F&& f) {
        rugi_begin_transition(h_, id.data(), id.size(), key, static_cast<RugiSlide>(slide));
        scoped(f);
    }
    /// Window content rectangle (logical pixels).
    RugiRect screen_rect() const {
        RugiRect r{};
        rugi_screen_rect(h_, &r);
        return r;
    }
    void title_bar(std::string_view title, float height = 32.0f) {
        rugi_title_bar(h_, title.data(), title.size(), height);
    }
    void window_command(RugiWindowCommand c) { rugi_window_command(h_, c); }

    // Theme, style and fonts ------------------------------------------------
    void set_theme(Theme t) { rugi_ui_set_theme(h_, static_cast<RugiTheme>(t)); }
    void set_accent(RugiColor c) { rugi_ui_set_accent(h_, c); }
    void set_color(RugiColorRole role, RugiColor c) { rugi_ui_set_color(h_, role, c); }
    RugiColor color(RugiColorRole role) {
        RugiColor c{};
        rugi_ui_color(h_, role, &c);
        return c;
    }
    void set_metric(RugiMetric m, float v) { rugi_ui_set_metric(h_, m, v); }
    /// Runs `f` with a temporary color override (restored even on exceptions).
    template <class F> void with_color(RugiColorRole role, RugiColor c, F&& f) {
        rugi_push_color(h_, role, c);
        scoped(f, rugi_pop_style);
    }
    template <class F> void with_metric(RugiMetric m, float v, F&& f) {
        rugi_push_metric(h_, m, v);
        scoped(f, rugi_pop_style);
    }
    template <class F> void with_font(Font font, F&& f) {
        rugi_push_font(h_, font);
        scoped(f, rugi_pop_style);
    }
    /// Registers a font file; throws rugi::Error on failure.
    Font add_font_file(std::string_view path) {
        Font f = RUGI_FONT_UI;
        check(rugi_ui_add_font_file(h_, path.data(), path.size(), &f));
        return f;
    }
    void set_font(Font font, float scale = 1.0f) { rugi_ui_set_font(h_, font, scale); }

    // Tools -----------------------------------------------------------------
    /// Command palette over `commands` (name, hint). Returns the chosen index.
    std::optional<uint32_t> command_palette(
        std::initializer_list<std::pair<std::string_view, std::string_view>> commands) {
        std::vector<RugiStr> names, hints;
        names.reserve(commands.size());
        hints.reserve(commands.size());
        for (auto& c : commands) {
            names.push_back(str(c.first));
            hints.push_back(str(c.second));
        }
        uint32_t idx = 0;
        if (rugi_command_palette(h_, names.data(), hints.data(), names.size(), &idx)) return idx;
        return std::nullopt;
    }
    void open_command_palette() { rugi_ui_set_command_palette_open(h_, 1); }
    void set_inspector(bool on) { rugi_ui_set_inspector(h_, on ? 1 : 0); }
    /// The built-in demo of every widget (like ImGui::ShowDemoWindow).
    void show_demo() { rugi_show_demo(h_); }
    void set_demo_page(RugiDemoPage page) { rugi_set_demo_page(h_, page); }

    // Frame state -----------------------------------------------------------
    RugiItemState last_item() const {
        RugiItemState s{};
        rugi_last_item(h_, &s);
        return s;
    }
    RugiFrameStats last_frame_stats() const {
        RugiFrameStats s{};
        rugi_ui_frame_stats(h_, &s);
        return s;
    }
    RugiDisplayInfo display_info() const {
        RugiDisplayInfo d{};
        rugi_ui_display_info(h_, &d);
        return d;
    }
    void request_repaint() { rugi_ui_request_repaint(h_); }
    void request_exit() { rugi_ui_request_exit(h_); }

private:
    /// Runs `f`, then `close` (rugi_end by default) even if `f` throws.
    template <class F> void scoped(F& f, RugiResult (*close)(RugiUi*) = rugi_end) {
        struct Close {
            RugiUi* h;
            RugiResult (*fn)(RugiUi*);
            ~Close() { fn(h); }
        } guard{h_, close};
        f();
    }

    RugiUi* h_;
};

// ---------------------------------------------------------------------------
// Context (host-driven, no window)
// ---------------------------------------------------------------------------

/// Owning, host-driven context (no window). Move-only.
class Context {
public:
    explicit Context(Theme theme = Theme::Dark, bool inspection = false) {
        RugiUiDesc d{};
        d.struct_size = sizeof(RugiUiDesc);
        d.theme = static_cast<RugiTheme>(theme);
        d.inspection = inspection ? 1 : 0;
        check(rugi_ui_create(&d, &h_));
    }
    ~Context() {
        if (h_) rugi_ui_destroy(h_);
    }
    Context(const Context&) = delete;
    Context& operator=(const Context&) = delete;
    Context(Context&& o) noexcept : h_(std::exchange(o.h_, nullptr)) {}
    Context& operator=(Context&& o) noexcept {
        if (this != &o) {
            if (h_) rugi_ui_destroy(h_);
            h_ = std::exchange(o.h_, nullptr);
        }
        return *this;
    }

    Ui ui() const noexcept { return Ui(h_); }

    // Input -----------------------------------------------------------------
    void mouse_move(float x, float y) { check(rugi_ui_mouse_move(h_, x, y)); }
    void mouse_button(RugiMouseButton b, bool pressed) { check(rugi_ui_mouse_button(h_, b, pressed ? 1 : 0)); }
    void scroll(float dx, float dy) { check(rugi_ui_scroll(h_, dx, dy)); }
    void key(RugiKey k, bool pressed, RugiModifiers mods = 0) { check(rugi_ui_key(h_, k, pressed ? 1 : 0, mods)); }
    void text(std::string_view s) { check(rugi_ui_text(h_, s.data(), s.size())); }
    void paste(std::string_view s) { check(rugi_ui_paste(h_, s.data(), s.size())); }
    /// Text the last frame copied/cut ("" if none).
    std::string clipboard() const {
        size_t n = rugi_ui_clipboard_text(h_, nullptr, 0);
        if (n == 0) return {};
        std::string s(n, '\0');
        rugi_ui_clipboard_text(h_, s.data(), s.size());
        s.resize(n - 1);
        return s;
    }

    // Frames ----------------------------------------------------------------
    /// Runs one frame: begin, `f(ui)`, end. Returns the frame statistics.
    template <class F> RugiFrameStats frame(float width, float height, float scale, float dt, F&& f) {
        RugiFrameDesc d{width, height, scale, dt};
        check(rugi_ui_begin_frame(h_, &d));
        RugiFrameStats stats{};
        try {
            f(Ui(h_));
        } catch (...) {
            rugi_ui_end_frame(h_, &stats);
            throw;
        }
        check(rugi_ui_end_frame(h_, &stats));
        return stats;
    }

private:
    RugiUi* h_ = nullptr;
};

// ---------------------------------------------------------------------------
// Windowed application
// ---------------------------------------------------------------------------

struct AppConfig {
    std::string title = "Rugi";
    uint32_t width = 1280;
    uint32_t height = 800;
    Theme theme = Theme::Dark;
    Present present = Present::Vsync;
    /// RUGI_WINDOW_BORDERLESS | RUGI_WINDOW_TRANSPARENT | RUGI_WINDOW_FIXED_SIZE.
    RugiWindowFlags window_flags = 0;
};

/// Opens a native window and calls `frame(Ui)` every frame until closed.
/// Exceptions thrown by `frame` close the window and are rethrown here.
template <class F> void run_app(const AppConfig& cfg, F&& frame) {
    RugiAppDesc d{};
    d.struct_size = sizeof(RugiAppDesc);
    d.title = cfg.title.c_str();
    d.width = cfg.width;
    d.height = cfg.height;
    d.theme = static_cast<RugiTheme>(cfg.theme);
    d.present = static_cast<RugiPresent>(cfg.present);
    d.window_flags = cfg.window_flags;
    detail::Callback<std::remove_reference_t<F>> cb{&frame};
    auto tramp = [](RugiUi* ui, void* user) noexcept {
        auto& c = *static_cast<decltype(cb)*>(user);
        if (c.error) return;
        c(Ui(ui));
        if (c.error) rugi_ui_request_exit(ui);
    };
    RugiResult r = rugi_app_run(&d, tramp, &cb);
    cb.rethrow();
    check(r);
}

}  // namespace rugi
