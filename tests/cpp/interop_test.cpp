// End-to-end C++ ⇄ Rust interoperability test (no window, no GPU).
// Drives the real engine through the C ABI and the C++ wrapper with synthetic
// input and checks widget results, error reporting and exception safety.

#include <rugi/rugi.hpp>

#include <cstdio>
#include <cstring>
#include <optional>
#include <stdexcept>
#include <string>

static int failures = 0;
#define EXPECT(cond)                                                                 \
    do {                                                                             \
        if (!(cond)) {                                                               \
            std::fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond);    \
            ++failures;                                                              \
        }                                                                            \
    } while (0)

static constexpr float W = 800, H = 600, SCALE = 1, DT = 1.0f / 60;

static void click(rugi::Context& ctx, float x, float y) {
    ctx.mouse_move(x, y);
    ctx.mouse_button(RUGI_MOUSE_LEFT, true);
    ctx.mouse_button(RUGI_MOUSE_LEFT, false);
}

int main() {
    EXPECT(rugi_abi_version() == RUGI_ABI_VERSION);
    EXPECT(rugi_version() != 0);

    rugi::Context ctx(rugi::Theme::Dark, /*inspection=*/true);

    // --- Button: layout, then click inside the reported rect.
    RugiItemState button{};
    bool clicked = false;
    auto build = [&](rugi::Ui ui) {
        ui.central_panel([&] {
            clicked = ui.button("Press me");
            button = ui.last_item();
        });
    };
    auto stats = ctx.frame(W, H, SCALE, DT, build);
    EXPECT(stats.instances > 0);
    EXPECT(stats.widgets == 1);
    EXPECT(button.rect.w > 40 && button.rect.h > 20);
    EXPECT(!clicked);
    click(ctx, button.rect.x + button.rect.w / 2, button.rect.y + button.rect.h / 2);
    ctx.frame(W, H, SCALE, DT, build);
    EXPECT(clicked);
    ctx.frame(W, H, SCALE, DT, build);
    EXPECT(!clicked);

    // --- Checkbox, slider, text input bound to C++ state.
    bool check = false;
    float value = 0.0f;
    std::string name = "abc";
    RugiItemState cb{}, sl{}, ti{};
    bool submitted = false;
    auto controls = [&](rugi::Ui ui) {
        ui.central_panel([&] {
            ui.checkbox("Enabled", check);
            cb = ui.last_item();
            ui.slider("Value", value, 0.0f, 10.0f);
            sl = ui.last_item();
            ui.text_input("name", name, "Name");
            ti = ui.last_item();
            submitted = ti.submitted != 0;
        });
    };
    ctx.frame(W, H, SCALE, DT, controls);
    click(ctx, cb.rect.x + 5, cb.rect.y + cb.rect.h / 2);
    ctx.frame(W, H, SCALE, DT, controls);
    EXPECT(check);
    click(ctx, sl.rect.x + sl.rect.w - 2, sl.rect.y + sl.rect.h / 2);
    ctx.frame(W, H, SCALE, DT, controls);
    EXPECT(value > 9.5f && value <= 10.0f);
    // Keyboard on the focused slider.
    ctx.key(RUGI_KEY_HOME, true);
    ctx.frame(W, H, SCALE, DT, controls);
    EXPECT(value == 0.0f);
    // Focus the text input at its end and type UTF-8.
    click(ctx, ti.rect.x + ti.rect.w - 4, ti.rect.y + ti.rect.h / 2);
    ctx.frame(W, H, SCALE, DT, controls);
    ctx.text(u8"dé");
    ctx.frame(W, H, SCALE, DT, controls);
    EXPECT(name == u8"abcdé");
    ctx.key(RUGI_KEY_BACKSPACE, true);
    ctx.frame(W, H, SCALE, DT, controls);
    EXPECT(name == "abcd");
    ctx.key(RUGI_KEY_ENTER, true);
    ctx.frame(W, H, SCALE, DT, controls);
    EXPECT(submitted);

    // --- Scroll area with many rows.
    auto list = [&](rugi::Ui ui) {
        ui.central_panel([&] {
            ui.scroll_area("rows", 120, [&] {
                for (uint64_t i = 0; i < 100; ++i) ui.with_id(i, [&] { ui.label("Row " + std::to_string(i)); });
            });
        });
    };
    auto s1 = ctx.frame(W, H, SCALE, DT, list);
    EXPECT(s1.culled > 0);

    // --- Themes.
    EXPECT(rugi_ui_set_theme(ctx.ui().handle(), RUGI_THEME_LIGHT) == RUGI_OK);
    EXPECT(rugi_ui_set_theme(ctx.ui().handle(), 77) == RUGI_ERROR_INVALID_ARGUMENT);
    EXPECT(rugi::last_error_message().find("theme") != std::string::npos);

    // --- Error reporting through the C ABI.
    EXPECT(!rugi_button(nullptr, "x", 1, RUGI_BUTTON_PRIMARY, RUGI_ICON_NONE));
    EXPECT(rugi_last_error_code() == RUGI_ERROR_NULL_POINTER);
    // Widget outside a frame -> INVALID_STATE, no crash.
    EXPECT(!rugi_button(ctx.ui().handle(), "x", 1, RUGI_BUTTON_PRIMARY, RUGI_ICON_NONE));
    EXPECT(rugi_last_error_code() == RUGI_ERROR_INVALID_STATE);
    // Unbalanced containers -> end_frame reports, wrapper throws.
    bool threw = false;
    try {
        ctx.frame(W, H, SCALE, DT, [](rugi::Ui ui) { rugi_begin_card(ui.handle()); });
    } catch (const rugi::Error& e) {
        threw = e.code() == RUGI_ERROR_INVALID_STATE;
    }
    EXPECT(threw);

    // --- Exception thrown inside a container: scopes close, frame ends, the
    // exception propagates, and the context stays usable.
    threw = false;
    try {
        ctx.frame(W, H, SCALE, DT, [](rugi::Ui ui) {
            ui.central_panel([&] { ui.card([&] { throw std::runtime_error("boom"); }); });
        });
    } catch (const std::runtime_error& e) {
        threw = std::strcmp(e.what(), "boom") == 0;
    }
    EXPECT(threw);
    auto after = ctx.frame(W, H, SCALE, DT, build);
    EXPECT(after.widgets == 1);

    // --- Qt-style widgets, popups and the re-entrant table callback.
    {
        rugi::Context c2(rugi::Theme::Dark, true);
        uint32_t combo = 0, tab = 0;
        int32_t radio = 1;
        std::optional<size_t> row;
        size_t cells = 0;
        RugiItemState combo_rect{};
        auto ui_fn = [&](rugi::Ui ui) {
            ui.central_panel([&] {
                ui.tabs("tabs", tab, {"One", "Two"});
                ui.combo_box("combo", combo, {"Alpha", "Beta", "Gamma"});
                combo_rect = ui.last_item();
                ui.radio("R2", radio, 2);
                ui.table("t", {"A", "B"}, {1.0f, 1.0f}, 50000, row, 200, [&](rugi::Ui cell, size_t r, size_t col) {
                    ++cells;
                    if (col == 0 && r == 0) cell.label("first");
                });
            });
        };
        c2.frame(W, H, SCALE, DT, ui_fn);
        EXPECT(cells > 0 && cells < 40);  // virtualized: only visible rows
        // Open the combo, then click the third row of its popup (rows are
        // 34 px apart after 6 px padding, starting 4 px below the combo).
        click(c2, combo_rect.rect.x + 10, combo_rect.rect.y + combo_rect.rect.h / 2);
        c2.frame(W, H, SCALE, DT, ui_fn);
        float third_row_y = combo_rect.rect.y + combo_rect.rect.h + 4 + 6 + 2 * 34 + 16;
        click(c2, combo_rect.rect.x + 30, third_row_y);
        c2.frame(W, H, SCALE, DT, ui_fn);
        EXPECT(combo == 2);
        // Exceptions from a table cell propagate after the table completes.
        bool threw = false;
        try {
            c2.frame(W, H, SCALE, DT, [&](rugi::Ui ui) {
                ui.central_panel([&] {
                    std::optional<size_t> sel;
                    ui.table("t2", {"A"}, {}, 10, sel, 100, [](rugi::Ui, size_t, size_t) {
                        throw std::runtime_error("cell");
                    });
                });
            });
        } catch (const std::runtime_error& e) {
            threw = std::string(e.what()) == "cell";
        }
        EXPECT(threw);
        // Styles: a metric override and an invalid color role.
        EXPECT(rugi_ui_set_metric(c2.ui().handle(), RUGI_METRIC_RADIUS_MD, 4.0f) == RUGI_OK);
        EXPECT(rugi_ui_set_color(c2.ui().handle(), 999, RugiColor{1, 2, 3, 255}) == RUGI_ERROR_INVALID_ARGUMENT);
        // Fonts: invalid data is rejected; the bundled icon font exists.
        RugiFont f = 0;
        const uint8_t junk[4] = {1, 2, 3, 4};
        EXPECT(rugi_ui_add_font(c2.ui().handle(), junk, 4, &f) == RUGI_ERROR_INVALID_ARGUMENT);
        EXPECT(rugi_ui_icon_font(c2.ui().handle()) >= 2);
        char icon[8];
        EXPECT(rugi_lucide_icon("save", 4, icon, sizeof icon) == 3);
        // Clipboard round trip (host-driven mode).
        std::string field = "copy me";
        RugiItemState tf{};
        auto text_fn = [&](rugi::Ui ui) { ui.central_panel([&] { ui.text_input("tf", field); tf = ui.last_item(); }); };
        c2.frame(W, H, SCALE, DT, text_fn);
        click(c2, tf.rect.x + 5, tf.rect.y + 5);
        c2.frame(W, H, SCALE, DT, text_fn);
        RugiModifiers cmd =
#ifdef __APPLE__
            RUGI_MOD_LOGO;
#else
            RUGI_MOD_CTRL;
#endif
        c2.key(RUGI_KEY_A, true, cmd);
        c2.key(RUGI_KEY_C, true, cmd);
        c2.frame(W, H, SCALE, DT, text_fn);
        EXPECT(c2.clipboard() == "copy me");
        c2.key(RUGI_KEY_END, true);
        c2.paste("!");
        c2.frame(W, H, SCALE, DT, text_fn);
        EXPECT(field == "copy me!");
    }

    // --- The built-in demo runs through the C ABI on every page.
    {
        rugi::Context demo(rugi::Theme::Dark);
        for (RugiDemoPage page = RUGI_DEMO_WELCOME; page <= RUGI_DEMO_METRICS; ++page) {
            EXPECT(rugi_set_demo_page(demo.ui().handle(), page) == RUGI_OK);
            auto s = demo.frame(1280, 800, 2, DT, [](rugi::Ui ui) { ui.show_demo(); });
            EXPECT(s.instances > 100);
        }
        EXPECT(rugi_set_demo_page(demo.ui().handle(), 99) == RUGI_ERROR_INVALID_ARGUMENT);
    }

    // --- Glass, backdrops, window chrome, key bindings and custom widgets.
    {
        rugi::Context c3(rugi::Theme::Dark, true);
        EXPECT(rugi_ui_set_theme(c3.ui().handle(), RUGI_THEME_ROSE) == RUGI_OK);
        RugiGlassStyle glass = rugi::glass_style(RUGI_GLASS_LIQUID);
        EXPECT(glass.refraction > 0);
        RugiBackdrop bg = rugi::backdrop_preset(RUGI_BACKDROP_SMOKE);
        RugiKeyBind bind{};
        uint32_t seg = 0, card = 0;
        bool flags[3] = {false, true, false};
        std::string pw;
        RugiItemState kb{}, sg{}, ms{}, car{}, pwi{};
        size_t painted = 0;
        auto fn = [&](rugi::Ui ui) {
            ui.backdrop(bg);
            ui.glass_panel(rugi::rect(20, 20, 760, 560), glass, [&] {
                ui.title_bar("Overlay", 30);
                ui.transition("page", seg, rugi::Slide::Up, [&] {
                    ui.keybind("Record", bind);
                    kb = ui.last_item();
                    ui.segmented("seg", seg, {"One", "Two", "Three"});
                    sg = ui.last_item();
                    ui.multi_select("Parts", flags, {"Head", "Chest", "Legs"});
                    ms = ui.last_item();
                    ui.text_input_ex("##pw", pw, "Password", RUGI_INPUT_PASSWORD, RUGI_ICON_NONE);
                    pwi = ui.last_item();
                    painted = 0;
                    ui.carousel("cards", 6, card, 120, 160, [&](rugi::Ui cui, size_t, RugiRect r, float) {
                        ++painted;
                        cui.backdrop_rect(r, 12, bg);
                    });
                    car = ui.last_item();
                });
            });
        };
        auto st = c3.frame(W, H, SCALE, DT, fn);
        EXPECT(st.instances > 20);
        EXPECT(painted >= 2 && painted <= 6);
        // Bind F9 with Ctrl.
        click(c3, kb.rect.x + kb.rect.w / 2, kb.rect.y + kb.rect.h / 2);
        c3.frame(W, H, SCALE, DT, fn);
        c3.frame(W, H, SCALE, DT, fn);
        c3.key(RUGI_KEY_F1 + 8, true, RUGI_MOD_CTRL);
        c3.frame(W, H, SCALE, DT, fn);
        EXPECT(bind.kind == RUGI_BIND_KEY && bind.code == RUGI_KEY_F1 + 8 && bind.mods == RUGI_MOD_CTRL);
        char label[32];
        EXPECT(rugi_keybind_label(&bind, label, sizeof label) == 7);
        EXPECT(std::string(label) == "Ctrl+F9");
        // Segmented: click the last segment.
        click(c3, sg.rect.x + sg.rect.w - 10, sg.rect.y + sg.rect.h / 2);
        c3.frame(W, H, SCALE, DT, fn);
        EXPECT(seg == 2);
        // Carousel: click the right neighbour.
        click(c3, car.rect.x + car.rect.w / 2 + 110, car.rect.y + car.rect.h / 2);
        c3.frame(W, H, SCALE, DT, fn);
        EXPECT(card == 1);
        // Password input stores the real text.
        click(c3, pwi.rect.x + 20, pwi.rect.y + pwi.rect.h / 2);
        c3.frame(W, H, SCALE, DT, fn);
        c3.text("s3cret");
        c3.frame(W, H, SCALE, DT, fn);
        EXPECT(pw == "s3cret");
        // Invalid arguments are rejected, not crashes.
        RugiBackdrop bad = bg;
        bad.kind = 42;
        EXPECT(rugi_glass_style_preset(9, &glass) == RUGI_ERROR_INVALID_ARGUMENT);
        RugiKeyBind badbind{};
        badbind.kind = 7;
        EXPECT(rugi_keybind_label(&badbind, label, sizeof label) == 0);
        // Exceptions from a carousel card propagate after the carousel completes.
        bool threw = false;
        try {
            c3.frame(W, H, SCALE, DT, [&](rugi::Ui ui) {
                ui.central_panel([&] {
                    ui.carousel("c2", 3, card, 100, 100, [](rugi::Ui, size_t, RugiRect, float) {
                        throw std::runtime_error("card");
                    });
                });
            });
        } catch (const std::runtime_error& e) {
            threw = std::string(e.what()) == "card";
        }
        EXPECT(threw);
        bool bad_rejected = false;
        c3.frame(W, H, SCALE, DT, [&](rugi::Ui ui) {
            bad_rejected = rugi_backdrop(ui.handle(), &bad) == RUGI_ERROR_INVALID_ARGUMENT;
        });
        EXPECT(bad_rejected);
    }

    // --- Move semantics.
    rugi::Context moved = std::move(ctx);
    EXPECT(moved.ui().handle() != nullptr);

    if (failures == 0) std::printf("rugi C++ interop test: all checks passed\n");
    return failures == 0 ? 0 : 1;
}
