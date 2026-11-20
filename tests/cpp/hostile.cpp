// Hostile use of the C++ wrapper: exceptions thrown from callbacks at every
// nesting level, wrapper misuse and invalid configuration. Run under the
// RUGI_SANITIZE build (ASan/UBSan) and the macOS `leaks` check.

#include <rugi/rugi.hpp>

#include <cstdio>
#include <cstring>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <thread>

static int failures = 0;
#define EXPECT(cond)                                                              \
    do {                                                                          \
        if (!(cond)) {                                                            \
            std::fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); \
            ++failures;                                                           \
        }                                                                         \
    } while (0)

static constexpr float W = 800, H = 600, DT = 1.0f / 60;

// A context owned by a C++ thread_local whose destructor runs after the
// library's own thread-locals are destroyed (they are created later, so they
// are torn down first). Destroying it there must work, not abort.
static RugiResult late_destroy_rc = -1;
struct LateContext {
    RugiUi* ui = nullptr;
    ~LateContext() { late_destroy_rc = rugi_ui_destroy(ui); }
};
static thread_local LateContext late_context;

// An exception that owns heap memory, so a lost or double-destroyed exception
// shows up as a leak or an ASan report.
struct Payload : std::exception {
    std::unique_ptr<std::string> text;
    explicit Payload(const char* s) : text(std::make_unique<std::string>(s)) {}
    const char* what() const noexcept override { return text->c_str(); }
};

// Runs a frame that must throw Payload(`want`); returns true if it did.
template <class F> static bool throws_payload(rugi::Context& ctx, const char* want, F&& body) {
    try {
        ctx.frame(W, H, 1, DT, body);
    } catch (const Payload& p) {
        return std::strcmp(p.what(), want) == 0;
    } catch (...) {
        return false;
    }
    return false;
}

// A frame with only valid calls must end cleanly (no leaked misuse/state).
static bool clean_frame(rugi::Context& ctx) {
    try {
        auto st = ctx.frame(W, H, 1, DT, [](rugi::Ui ui) { ui.central_panel([&] { ui.button("ok"); }); });
        return st.widgets == 1;
    } catch (...) {
        return false;
    }
}

int main() {
    rugi::Context ctx;

    // Exception from a table cell inside a scope that pushes an id and a style.
    EXPECT(throws_payload(ctx, "cell", [](rugi::Ui ui) {
        ui.central_panel([&] {
            ui.with_id(1, [&] {
                ui.with_metric(RUGI_METRIC_SPACING, 2, [&] {
                    std::optional<size_t> sel;
                    ui.table("t", {"A", "B"}, {1, 1}, 100, sel, 200,
                             [](rugi::Ui, size_t row, size_t) {
                                 if (row == 1) throw Payload("cell");
                             });
                });
            });
        });
    }));
    EXPECT(clean_frame(ctx));

    // Exception from a table nested in a carousel card nested in a table:
    // only the first exception survives; every level completes.
    int cells_after_throw = 0;
    EXPECT(throws_payload(ctx, "deep", [&](rugi::Ui ui) {
        ui.central_panel([&] {
            std::optional<size_t> sel;
            ui.table("outer", {"A"}, {}, 20, sel, 200, [&](rugi::Ui cell, size_t row, size_t) {
                if (row != 0) {
                    ++cells_after_throw;
                    return;
                }
                uint32_t idx = 0;
                cell.carousel("car", 3, idx, 40, 40, [&](rugi::Ui card, size_t, RugiRect, float) {
                    std::optional<size_t> s2;
                    card.table("inner", {"x"}, {}, 5, s2, 40, [](rugi::Ui, size_t, size_t) {
                        throw Payload("deep");
                    });
                    throw Payload("second");  // never seen: the inner one wins
                });
            });
        });
    }));
    EXPECT(cells_after_throw == 0);  // the outer table skips cells after an exception
    EXPECT(clean_frame(ctx));

    // Exception from a container body inside a carousel card.
    EXPECT(throws_payload(ctx, "card", [](rugi::Ui ui) {
        ui.central_panel([&] {
            uint32_t idx = 0;
            ui.carousel("c", 4, idx, 50, 50, [&](rugi::Ui card, size_t i, RugiRect r, float) {
                card.region(r, false, [&] {
                    card.card([&] {
                        if (i == 0) throw Payload("card");
                    });
                });
            });
        });
    }));
    EXPECT(clean_frame(ctx));

    // A non-std exception type and a thrown int.
    bool caught_int = false;
    try {
        ctx.frame(W, H, 1, DT, [](rugi::Ui ui) {
            ui.central_panel([&] {
                std::optional<size_t> sel;
                ui.table("ti", {"A"}, {}, 3, sel, 100, [](rugi::Ui, size_t, size_t) { throw 42; });
            });
        });
    } catch (int v) {
        caught_int = v == 42;
    }
    EXPECT(caught_int);
    EXPECT(clean_frame(ctx));

    // Wrapper misuse: a Ui over a NULL handle never crashes.
    rugi::Ui null_ui(nullptr);
    EXPECT(!null_ui.button("x"));
    EXPECT(rugi_last_error_code() == RUGI_ERROR_NULL_POINTER);
    std::string s = "abc";
    EXPECT(!null_ui.text_input("t", s));
    EXPECT(s == "abc");
    null_ui.central_panel([] {});

    // Text input: capacity smaller than the value, zero capacity, invalid
    // UTF-8 and embedded NUL in the bound string.
    std::string long_value(300, 'x');
    std::string zero_cap = "q";
    std::string bad = "\xff\xfe";
    std::string nul("a\0b", 3);
    ctx.frame(W, H, 1, DT, [&](rugi::Ui ui) {
        ui.central_panel([&] {
            ui.text_input("long", long_value, "", 4);
            ui.text_input("zero", zero_cap, "", 0);
            EXPECT(!ui.text_input("bad", bad));
            EXPECT(rugi_last_error_code() == RUGI_ERROR_INVALID_UTF8);
            ui.text_input("nul", nul);
        });
    });
    EXPECT(long_value.size() == 300);
    EXPECT(bad == "\xff\xfe");

    // Invalid configuration throws rugi::Error, never opens a window.
    bool threw = false;
    try {
        rugi::Context bad_theme(static_cast<rugi::Theme>(99));
    } catch (const rugi::Error& e) {
        threw = e.code() == RUGI_ERROR_INVALID_ARGUMENT;
    }
    EXPECT(threw);
    threw = false;
    try {
        rugi::glass_style(99);
    } catch (const rugi::Error& e) {
        threw = e.code() == RUGI_ERROR_INVALID_ARGUMENT && !std::string(e.what()).empty();
    }
    EXPECT(threw);
    threw = false;
    try {
        rugi::AppConfig cfg;
        cfg.present = static_cast<rugi::Present>(7);
        rugi::run_app(cfg, [](rugi::Ui) {});
    } catch (const rugi::Error& e) {
        threw = e.code() == RUGI_ERROR_INVALID_ARGUMENT;
    }
    EXPECT(threw);
    threw = false;
    try {
        ctx.frame(W, H, 0, DT, [](rugi::Ui) {});  // scale 0
    } catch (const rugi::Error& e) {
        threw = e.code() == RUGI_ERROR_INVALID_ARGUMENT;
    }
    EXPECT(threw);

    // Begin/end misuse inside a C++ frame: end_frame reports, the wrapper
    // throws, and the next frame is clean.
    threw = false;
    RugiResult end_rc = RUGI_OK;
    try {
        ctx.frame(W, H, 1, DT, [&](rugi::Ui ui) {
            end_rc = rugi_end(ui.handle());  // nothing open
            rugi_begin_card(ui.handle());     // never closed
        });
    } catch (const rugi::Error& e) {
        threw = e.code() == RUGI_ERROR_INVALID_STATE;
    }
    EXPECT(end_rc == RUGI_ERROR_INVALID_STATE);
    EXPECT(threw);
    EXPECT(clean_frame(ctx));

    // Move semantics, including self-move-assignment.
    rugi::Context a;
    rugi::Context& alias = a;
    a = std::move(alias);
    EXPECT(a.ui().handle() != nullptr);
    rugi::Context b = std::move(a);
    EXPECT(a.ui().handle() == nullptr);
    EXPECT(clean_frame(b));

    std::thread([] {
        late_context.ui = nullptr;  // constructs the thread_local first
        rugi_ui_create(nullptr, &late_context.ui);
        rugi_ui_mouse_move(nullptr, 0, 0);  // touches the last-error thread-local
    }).join();
    EXPECT(late_destroy_rc == RUGI_OK);

    if (failures == 0) std::printf("rugi C++ hostile test: all checks passed\n");
    return failures == 0 ? 0 : 1;
}
