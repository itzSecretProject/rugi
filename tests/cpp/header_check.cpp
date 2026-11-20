// Compiled with -std=c++17/c++20 -Wall -Wextra -Wpedantic -Werror (ctest).
#include <rugi/rugi.hpp>
#include <rugi/rugi_icons_lucide.h>

int main() {
    rugi::Context ctx;
    ctx.frame(100, 100, 1, 0.016f, [](rugi::Ui ui) { ui.central_panel([&] { ui.button("x"); }); });
    return 0;
}
