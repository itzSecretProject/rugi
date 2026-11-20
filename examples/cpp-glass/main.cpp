// Rugi C++ example: glass panels and animated backdrops. A glass panel in a
// borderless window over one of Rugi's procedural backdrops; the C++
// counterpart of examples/effects.
//
//   cmake --build build && ./build/rugi_cpp_glass
//
// The Backdrop page changes the backdrop behind the panel, the Panel page the
// theme and the glass of the panel itself, and About lists the build and the
// licenses. Every control applies at once. Drag the window by the panel's
// title bar.

#include <rugi/rugi.hpp>
#include <rugi/rugi_icons_lucide.h>

#include <algorithm>
#include <cstdio>

namespace {

constexpr float kPanelWidth = 640, kPanelHeight = 580, kPanelMargin = 28;

// Accent overrides; option 0 keeps the theme's own accent.
constexpr RugiColor kAccents[] = {{0x0A, 0x84, 0xFF, 255}, {0x30, 0xD1, 0x58, 255}, {0xFF, 0x9F, 0x0A, 255},
                                  {0xFF, 0x37, 0x5F, 255}};
constexpr RugiGlassPreset kGlassPresets[] = {RUGI_GLASS_LIQUID, RUGI_GLASS_FROSTED, RUGI_GLASS_SMOKED};
constexpr float kGlassTints[] = {0.3f, 0.45f, 0.75f};
constexpr float kSpeeds[] = {0, 0.4f, 1, 2};
constexpr float kTextScales[] = {0.9f, 1, 1.15f};
// Lowest glass tint for the light theme: its dark text needs a light surface
// over the dark backdrop.
constexpr float kLightThemeMinTint = 0.75f;

float min_tint(uint32_t theme) { return theme == RUGI_THEME_LIGHT ? kLightThemeMinTint : 0; }

struct BackdropSettings {
    uint32_t kind = RUGI_BACKDROP_AURORA;
    float variation = 0, size = 1, pointer_response = 0.35f, brightness = 1;
    uint32_t speed = 2;
    bool use_accent = false;
};

struct PanelSettings {
    uint32_t theme = RUGI_THEME_DARK, accent = 0, text_size = 1, glass = 1;
    float blur = 0.7f, tint = kGlassTints[1];
};

struct State {
    uint32_t page = 0;
    BackdropSettings backdrop;
    PanelSettings panel;
    // Theme, accent and text size the current theme was built from.
    uint32_t applied[3] = {~0u, ~0u, ~0u};
};

RugiBackdrop backdrop(const State& s, RugiColor accent) {
    const BackdropSettings& w = s.backdrop;
    RugiBackdrop b = rugi::backdrop_preset(w.kind);
    if (w.use_accent) {
        RugiColor a = accent;
        b.color_a = a;
        b.color_b = RugiColor{uint8_t(a.r + (255 - a.r) / 2), uint8_t(a.g + (255 - a.g) / 2),
                              uint8_t(a.b + (255 - a.b) / 2), 255};
    }
    b.seed = w.variation;
    b.scale = w.size;
    b.speed = kSpeeds[w.speed];
    b.intensity = w.brightness;
    b.pointer_response = w.pointer_response;
    return b;
}

// Tinted with the theme background so text on the panel stays readable.
RugiGlassStyle panel_glass(const PanelSettings& a, RugiColor background) {
    RugiGlassStyle g = rugi::glass_style(kGlassPresets[a.glass]);
    g.blur = a.blur;
    g.tint = background;
    g.tint.a = uint8_t(std::max(a.tint, min_tint(a.theme)) * 255);
    return g;
}

RugiGlassStyle card_glass() {
    RugiGlassStyle g = rugi::glass_style(RUGI_GLASS_FROSTED);
    g.radius = 12;
    g.blur = 0.35f;
    g.tint = RugiColor{255, 255, 255, 8};
    g.highlight = 0.12f;
    g.grain = 0;
    g.border = RugiColor{255, 255, 255, 16};
    return g;
}

void select_glass(PanelSettings& a, uint32_t preset) {
    a.glass = preset;
    a.blur = rugi::glass_style(kGlassPresets[preset]).blur;
    a.tint = kGlassTints[preset];
}

void backdrop_page(rugi::Ui ui, BackdropSettings& w, const RugiGlassStyle& card) {
    ui.glass_card(card, [&] {
        ui.heading("Backdrop");
        ui.segmented("backdrop", w.kind, {"Aurora", "Smoke", "Plasma", "Starfield", "Filament", "Mesh"});
        ui.spin_box("Variation", w.variation, 0, 99, 1);
        ui.slider("Size", w.size, 0.5f, 2);
    });
    ui.glass_card(card, [&] {
        ui.heading("Motion");
        ui.segmented("speed", w.speed, {"Still", "Slow", "Normal", "Fast"});
        ui.slider("Follow the pointer", w.pointer_response, 0, 1);
    });
    ui.glass_card(card, [&] {
        ui.heading("Color");
        ui.checkbox("Use the accent color", w.use_accent);
        ui.slider("Brightness", w.brightness, 0.2f, 1.5f);
    });
    if (ui.button("Restore defaults", rugi::ButtonKind::Secondary)) w = BackdropSettings{};
}

void panel_page(rugi::Ui ui, PanelSettings& a, const RugiGlassStyle& card) {
    ui.glass_card(card, [&] {
        ui.heading("Theme");
        ui.segmented("theme", a.theme, {"Dark", "Light", "Rose", "Mocha", "Forest", "Graphite"});
        ui.caption("Accent color");
        ui.segmented("accent", a.accent, {"Theme", "Blue", "Green", "Orange", "Pink"});
        ui.caption("Text size");
        ui.segmented("text", a.text_size, {"Small", "Default", "Large"});
    });
    ui.glass_card(card, [&] {
        ui.heading("Glass");
        uint32_t preset = a.glass;
        if (ui.segmented("glass", preset, {"Light blur", "Strong blur", "Tinted"})) select_glass(a, preset);
        ui.slider("Blur", a.blur, 0, 1);
        float min = min_tint(a.theme);
        a.tint = std::max(a.tint, min);
        ui.slider("Tint", a.tint, min, 1);
        if (min > 0) ui.caption("Light themes start at 0.75 so dark text stays readable.");
    });
    if (ui.button("Restore defaults", rugi::ButtonKind::Secondary)) a = PanelSettings{};
}

void about_page(rugi::Ui ui, const RugiGlassStyle& card) {
    uint32_t v = rugi_version();
    RugiRect screen = ui.screen_rect();
    char version[64], window[64];
    std::snprintf(version, sizeof version, "Rugi %u.%u.%u (C ABI v%u)", v >> 16, (v >> 8) & 0xFF, v & 0xFF,
                  rugi_abi_version());
    std::snprintf(window, sizeof window, "%.0f x %.0f", screen.w, screen.h);
    ui.glass_card(card, [&] {
        ui.heading("Effects (C++)");
        ui.paragraph("An example of Rugi's glass panels, procedural backdrops and custom window chrome.");
        ui.columns(2, [&] {
            ui.muted("Version");
            ui.label(version);
            ui.muted("Window");
            ui.label(window);
        });
    });
    ui.glass_card(card, [&] {
        ui.heading("Licenses");
        ui.columns(2, [&] {
            ui.muted("Rugi");
            ui.label("MIT OR Apache-2.0");
            ui.muted("Inter, JetBrains Mono");
            ui.label("SIL Open Font License 1.1");
            ui.muted("Lucide icons");
            ui.label("ISC");
        });
    });
}

void frame(rugi::Ui ui, State& s) {
    PanelSettings& a = s.panel;
    if (s.applied[0] != a.theme || s.applied[1] != a.accent || s.applied[2] != a.text_size) {
        ui.set_theme(static_cast<rugi::Theme>(a.theme));
        if (a.accent > 0) ui.set_accent(kAccents[a.accent - 1]);
        ui.set_font(RUGI_FONT_UI, kTextScales[a.text_size]);
        s.applied[0] = a.theme;
        s.applied[1] = a.accent;
        s.applied[2] = a.text_size;
    }
    ui.backdrop(backdrop(s, ui.color(RUGI_COLOR_ACCENT)));

    RugiRect screen = ui.screen_rect();
    float w = std::min(kPanelWidth, screen.w - 2 * kPanelMargin);
    float h = std::min(kPanelHeight, screen.h - 2 * kPanelMargin);
    RugiGlassStyle card = card_glass();
    RugiGlassStyle glass = panel_glass(a, ui.color(RUGI_COLOR_BACKGROUND));
    ui.glass_panel(rugi::rect((screen.w - w) / 2, (screen.h - h) / 2, w, h), glass, [&] {
        ui.title_bar("Effects", 30);
        ui.segmented("pages", s.page, {"Backdrop", "Panel", "About"});
        ui.transition("page", s.page, rugi::Slide::Up, [&] {
            ui.scroll_area("page", 0, [&] {
                if (s.page == 0)
                    backdrop_page(ui, s.backdrop, card);
                else if (s.page == 1)
                    panel_page(ui, a, card);
                else
                    about_page(ui, card);
            });
        });
    });
}

}  // namespace

int main() {
    State state;
    rugi::AppConfig cfg;
    cfg.title = "Effects (C++)";
    cfg.width = 1000;
    cfg.height = 680;
    cfg.window_flags = RUGI_WINDOW_BORDERLESS;
    try {
        rugi::run_app(cfg, [&](rugi::Ui ui) { frame(ui, state); });
    } catch (const std::exception& e) {
        std::fprintf(stderr, "rugi_cpp_glass: %s\n", e.what());
        return 1;
    }
    return 0;
}
