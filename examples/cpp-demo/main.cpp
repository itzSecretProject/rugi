// Rugi — C++ demo. Same engine as the Rust demo, driven through the C ABI
// via the header-only C++17 wrapper. Application state lives in plain C++.

#include <rugi/rugi.hpp>
#include <rugi/rugi_icons_lucide.h>

#include <cstdio>
#include <deque>
#include <optional>
#include <string>

namespace {

struct AppState {
    uint32_t page = 0;
    bool dark = true;
    bool live = false;
    bool batching = true;
    float exposure = 1.0f;
    float threads = 8.0f;
    float progress = 0.35f;
    uint32_t backend = 1;
    uint32_t tab = 0;
    int32_t mode = 1;
    bool about = false;
    std::string project = "Native C++ host";
    std::string filter;
    std::deque<std::string> log;
    std::optional<size_t> row = 2;
    int events = 0;

    void add_event(const char* what) {
        log.push_front(std::string(what) + " #" + std::to_string(++events));
        if (log.size() > 200) log.pop_back();
    }
};

void sidebar(rugi::Ui& ui, AppState& s) {
    ui.heading("Rugi");
    ui.caption("C++ host · same Rust engine");
    ui.space(16);
    ui.caption("WORKSPACE");
    const char* pages[] = {"Dashboard", "Assets", "Event log"};
    const RugiIcon icons[] = {RUGI_ICON_LUCIDE_LAYOUT_DASHBOARD, RUGI_ICON_LUCIDE_TABLE_2, RUGI_ICON_LUCIDE_ACTIVITY};
    for (uint32_t i = 0; i < 3; ++i) {
        if (ui.nav_item(pages[i], icons[i], s.page == i)) s.page = i;
    }
    ui.space(16);
    ui.caption("PREFERENCES");
    if (ui.toggle("Dark mode", s.dark)) ui.set_theme(s.dark ? rugi::Theme::Dark : rugi::Theme::Light);
    ui.toggle("Live progress", s.live);
    ui.tooltip("Animates continuously at the display refresh rate");
}

void menu_bar(rugi::Ui& ui, AppState& s) {
    ui.horizontal([&] {
        ui.menu("File", [&] {
            if (ui.menu_item("New event", "Ctrl N")) s.add_event("Menu: new event");
            ui.menu_separator();
            if (ui.menu_item("Quit", "Ctrl Q")) ui.request_exit();
        });
        ui.menu("View", [&] {
            if (ui.menu_item("Command palette", "Ctrl K")) ui.open_command_palette();
            if (ui.menu_item("Inspector", "F12")) ui.set_inspector(true);
        });
        ui.menu("Help", [&] {
            if (ui.menu_item("About")) s.about = true;
        });
    });
}

void event_list(rugi::Ui& ui, AppState& s, float height) {
    ui.scroll_area("events", height, [&] {
        uint64_t i = 0;
        for (const auto& e : s.log) {
            ++i;
            if (!s.filter.empty() && e.find(s.filter) == std::string::npos) continue;
            ui.with_id(i, [&] { ui.label(e); });
        }
        if (s.log.empty()) ui.muted("No events yet.");
    });
}

void dashboard(rugi::Ui& ui, AppState& s) {
    ui.horizontal_right([&] {
        if (ui.button("Log event", rugi::ButtonKind::Primary, RUGI_ICON_LUCIDE_PLUS)) s.add_event("Button pressed");
        ui.set_next_item_width(200);
        ui.text_input("filter", s.filter, "Filter events…");
        ui.vertical([&] { ui.title(s.page == 0 ? "Dashboard" : s.page == 1 ? "Assets" : "Event log"); });
    });
    ui.muted("This window is a C++17 application linked against librugi_ffi.");
    ui.space(8);

    if (s.page == 1) {
        ui.card([&] {
            ui.heading("Assets (100,000 rows, virtualized)");
            ui.table("assets", {"Name", "Kind", "Size"}, {3.0f, 1.5f, 1.0f}, 100000, s.row, 380,
                     [&](rugi::Ui cell, size_t row, size_t col) {
                         if (col == 0) cell.label(RUGI_LUCIDE_FILE "  asset_" + std::to_string(row) + ".bin");
                         else if (col == 1) cell.muted(row % 3 == 0 ? "Texture" : "Mesh");
                         else cell.muted(std::to_string((row * 37) % 900 + 12) + " KB");
                     });
            ui.context_menu("asset-menu", [&] {
                if (ui.menu_item("Delete…", "Del")) s.about = true;
            });
        });
        return;
    }
    if (s.page == 2) {
        ui.card([&] {
            ui.heading("All events");
            event_list(ui, s, 420);
        });
        return;
    }

    RugiFrameStats st = ui.last_frame_stats();
    RugiDisplayInfo disp = ui.display_info();
    ui.columns(3, [&] {
        const std::string values[] = {std::to_string(st.build_us) + " us", std::to_string(st.instances),
                                      disp.refresh_hz > 0 ? std::to_string(int(disp.refresh_hz + 0.5f)) + " Hz" : "-"};
        const char* labels[] = {"Frame build (CPU)", "GPU instances", "Display refresh"};
        for (int i = 0; i < 3; ++i) {
            ui.card([&] {
                ui.caption(labels[i]);
                ui.text(values[i], rugi::TextRole::Display);
            });
        }
    });
    ui.space(4);
    ui.columns(2, [&] {
        ui.card([&] {
            ui.tabs("ctl", s.tab, {"Controls", "Choices"});
            if (s.tab == 0) {
                ui.text_input("project", s.project, "Project name");
                if (ui.slider("Exposure", s.exposure, 0.0f, 4.0f)) s.add_event("Exposure changed");
                ui.spin_box("threads", s.threads, 1, 64, 1);
                ui.checkbox("GPU batching", s.batching);
                ui.progress_bar(s.progress);
            } else {
                ui.combo_box("backend", s.backend, {"Direct3D 12", "Vulkan", "Metal", "OpenGL"});
                ui.radio("Balanced", s.mode, 1);
                ui.radio("Performance", s.mode, 2);
            }
            ui.horizontal([&] {
                if (ui.button("Run")) s.add_event("Run started");
                if (ui.button("Quit", rugi::ButtonKind::Danger)) ui.request_exit();
            });
        });
        ui.card([&] {
            ui.heading("Recent events");
            event_list(ui, s, 260);
        });
    });
}

}  // namespace

int main(int argc, char** argv) {
    // `rugi_cpp_demo --showcase` runs the built-in demo through the C ABI.
    for (int i = 1; i < argc; ++i) {
        if (std::string(argv[i]) == "--showcase") {
            try {
                rugi::AppConfig cfg;
                cfg.title = "Rugi Demo (C++)";
                cfg.width = 1440;
                cfg.height = 900;
                rugi::run_app(cfg, [](rugi::Ui ui) { ui.show_demo(); });
            } catch (const rugi::Error& e) {
                std::fprintf(stderr, "rugi error %d: %s\n", e.code(), e.what());
                return 1;
            }
            return 0;
        }
    }
    std::printf("Rugi C++ demo: library version 0x%06x, ABI %u (header ABI %u)\n", rugi_version(),
                rugi_abi_version(), RUGI_ABI_VERSION);
    if (rugi_abi_version() != RUGI_ABI_VERSION) {
        std::fprintf(stderr, "ABI mismatch between header and library\n");
        return 1;
    }
    AppState state;
    for (int i = 0; i < 12; ++i) state.add_event("Startup task");
    try {
        rugi::AppConfig cfg;
        cfg.title = "Rugi — C++ demo";
        rugi::run_app(cfg, [&](rugi::Ui ui) {
            if (state.live) {
                state.progress += 0.004f;
                if (state.progress > 1.0f) state.progress = 0.0f;
                ui.request_repaint();
            }
            ui.side_panel(230, [&] { sidebar(ui, state); });
            ui.top_bar(52, [&] { menu_bar(ui, state); });
            ui.central_panel([&] { ui.scroll_area("page", 0, [&] { dashboard(ui, state); }); });
            ui.modal("about", "About", state.about, 420, [&] {
                ui.paragraph("Rugi: Rust engine, C ABI, C++17 wrapper.");
            });
            if (auto cmd = ui.command_palette({{"Go to Dashboard", ""}, {"Go to Assets", ""}, {"Log event", ""}})) {
                if (*cmd == 2) state.add_event("Palette: log event");
                else state.page = *cmd;
            }
        });
    } catch (const rugi::Error& e) {
        std::fprintf(stderr, "rugi error %d: %s\n", e.code(), e.what());
        return 1;
    }
    return 0;
}
