/* Compiles the header as C11 and exercises the plain C API. */
#include <rugi/rugi.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    RugiUi* ui = NULL;
    RugiUiDesc desc;
    memset(&desc, 0, sizeof desc);
    desc.struct_size = sizeof desc;
    desc.theme = RUGI_THEME_LIGHT;
    if (rugi_ui_create(&desc, &ui) != RUGI_OK) return 1;
    RugiFrameDesc fd = {640.0f, 480.0f, 2.0f, 1.0f / 60.0f};
    RugiFrameStats st;
    bool on = true;
    float v = 0.5f;
    if (rugi_ui_begin_frame(ui, &fd) != RUGI_OK) return 2;
    rugi_begin_central_panel(ui);
    rugi_text(ui, "Hello from C", strlen("Hello from C"), RUGI_TEXT_TITLE);
    rugi_checkbox(ui, "On", 2, &on);
    rugi_slider_float(ui, "V", 1, &v, 0.0f, 1.0f);
    rugi_end(ui);
    if (rugi_ui_end_frame(ui, &st) != RUGI_OK) return 3;
    /* Wrong struct size is rejected. */
    RugiUi* other = NULL;
    desc.struct_size = 3;
    if (rugi_ui_create(&desc, &other) != RUGI_ERROR_VERSION_MISMATCH) return 4;
    char msg[128];
    rugi_last_error_message(msg, sizeof msg);
    printf("C smoke: %u instances, %u widgets; expected error: %s\n", st.instances, st.widgets, msg);
    rugi_ui_destroy(ui);
    return st.instances > 0 ? 0 : 5;
}
