/* Compiled with -std=c99/c11 -Wall -Wextra -Wpedantic -Werror (ctest). */
#include <rugi/rugi.h>
#include <rugi/rugi_icons_lucide.h>

int main(void) {
    RugiStr s = {RUGI_LUCIDE_SAVE, sizeof RUGI_LUCIDE_SAVE - 1};
    (void)s;
    return rugi_abi_version() == RUGI_ABI_VERSION ? 0 : 1;
}
