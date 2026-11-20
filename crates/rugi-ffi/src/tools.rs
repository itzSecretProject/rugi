//! Built-in tools: command palette, inspector and the demo.

use rugi_ui::DemoPage;

use crate::args::{opt_out, str_list};
use crate::error::{guard, FfiError, RugiResult};
use crate::handle::{handle, ui_call, with_ui, RugiUi};
use crate::types::RugiStr;

/// Demo page for rugi_set_demo_page.
pub type RugiDemoPage = u32;
/// Introduction, shortcuts and getting started.
pub const RUGI_DEMO_WELCOME: RugiDemoPage = 0;
/// Buttons, toggles, sliders and text.
pub const RUGI_DEMO_BASICS: RugiDemoPage = 1;
/// Text input, combo boxes, lists and tabs.
pub const RUGI_DEMO_INPUTS: RugiDemoPage = 2;
/// Rows, columns, cards, scrolling and trees.
pub const RUGI_DEMO_LAYOUT: RugiDemoPage = 3;
/// Menus, context menus, tooltips, dialogs and the command palette.
pub const RUGI_DEMO_POPUPS: RugiDemoPage = 4;
/// Virtualized table and plots.
pub const RUGI_DEMO_DATA: RugiDemoPage = 5;
/// Live theme editor.
pub const RUGI_DEMO_STYLE: RugiDemoPage = 6;
/// Typography and the icon set.
pub const RUGI_DEMO_FONTS: RugiDemoPage = 7;
/// Frame timing and engine statistics.
pub const RUGI_DEMO_METRICS: RugiDemoPage = 8;

/// Command palette (Cmd/Ctrl+K). `names`/`hints` are parallel arrays (`hints`
/// may be NULL). Returns true and writes `*out_index` when a command was run.
#[no_mangle]
pub unsafe extern "C" fn rugi_command_palette(
    ui: *mut RugiUi,
    names: *const RugiStr,
    hints: *const RugiStr,
    count: usize,
    out_index: *mut u32,
) -> bool {
    // SAFETY: contracts of `with_ui` and `str_list`; `out_index` NULL or writable.
    unsafe {
        with_ui(ui, |h| {
            let out = opt_out(out_index, "out_index")?;
            let names = str_list(names, count, "names")?;
            let hints = if hints.is_null() { vec![""; count] } else { str_list(hints, count, "hints")? };
            let commands: Vec<(&str, &str)> = names.into_iter().zip(hints).collect();
            let Some(i) = h.ui().command_palette(&commands) else { return Ok(false) };
            if let Some(o) = out {
                *o = i as u32;
            }
            Ok(true)
        })
    }
}

/// Opens (non-zero) or closes the command palette.
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_set_command_palette_open(ui: *mut RugiUi, open: u8) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        unsafe { handle(ui) }?.ui().set_command_palette_open(open != 0);
        Ok(())
    })
}

/// Enables (non-zero) or disables the live inspector overlay (also F12).
#[no_mangle]
pub unsafe extern "C" fn rugi_ui_set_inspector(ui: *mut RugiUi, on: u8) -> RugiResult {
    guard(|| {
        // SAFETY: contract of `handle`.
        unsafe { handle(ui) }?.ui().set_inspector(on != 0);
        Ok(())
    })
}

/// Shows the built-in Rugi demo as the whole window (like
/// ImGui::ShowDemoWindow). Call at the root level once per frame.
#[no_mangle]
pub unsafe extern "C" fn rugi_show_demo(ui: *mut RugiUi) -> RugiResult {
    // SAFETY: contract of `handle`.
    unsafe { ui_call(ui, |u| u.show_demo()) }
}

/// Selects the page shown by rugi_show_demo.
#[no_mangle]
pub unsafe extern "C" fn rugi_set_demo_page(ui: *mut RugiUi, page: RugiDemoPage) -> RugiResult {
    guard(|| {
        let page = match page {
            RUGI_DEMO_WELCOME => DemoPage::Welcome,
            RUGI_DEMO_BASICS => DemoPage::Basics,
            RUGI_DEMO_INPUTS => DemoPage::Inputs,
            RUGI_DEMO_LAYOUT => DemoPage::Layout,
            RUGI_DEMO_POPUPS => DemoPage::Popups,
            RUGI_DEMO_DATA => DemoPage::Data,
            RUGI_DEMO_STYLE => DemoPage::Style,
            RUGI_DEMO_FONTS => DemoPage::Fonts,
            RUGI_DEMO_METRICS => DemoPage::Metrics,
            _ => return Err(FfiError::invalid(format!("unknown demo page {page}"))),
        };
        // SAFETY: contract of `handle`.
        unsafe { handle(ui) }?.ui().set_demo_page(page);
        Ok(())
    })
}
