//! Window chrome of the demo: the page list, view settings and the command
//! palette.

use super::{set_compact, show_all_code_id, toggle_theme, DemoPage, DemoState, ShowAllCode, PAGES};
use crate::{ButtonKind, Ui};

/// Shortcut that opens the command palette, as shown to the user. Rugi
/// handles it in [`Ui::command_palette`].
pub(super) const PALETTE_SHORTCUT: &str = if cfg!(target_os = "macos") { "⌘K" } else { "Ctrl K" };
/// Shortcut that toggles the inspector, handled by Rugi.
pub(super) const INSPECTOR_SHORTCUT: &str = "F12";

/// Palette commands after the page list.
const COMMANDS: [(&str, &str); 11] = [
    ("Go to Welcome", ""),
    ("Go to Basics", ""),
    ("Go to Inputs & selection", ""),
    ("Go to Layout", ""),
    ("Go to Popups & overlays", ""),
    ("Go to Data & plots", ""),
    ("Go to Style editor", ""),
    ("Go to Fonts & icons", ""),
    ("Go to Metrics", ""),
    ("Switch between dark and light theme", ""),
    ("Show or hide the inspector", INSPECTOR_SHORTCUT),
];

pub(super) fn sidebar(ui: &mut Ui, st: &mut DemoState) {
    ui.separator_text("PAGES");
    for info in &PAGES {
        if ui.nav_item(info.name, None, st.page == info.page).clicked {
            st.page = info.page;
        }
    }
    ui.space(ui.theme().metrics.spacing);
    ui.separator_text("VIEW");
    let mut dark = ui.theme().dark;
    if ui.toggle("Dark theme", &mut dark).changed {
        toggle_theme(ui);
    }
    if ui.toggle("Compact controls", &mut st.compact).changed {
        set_compact(ui, st.compact);
    }
    ui.tooltip("26 px controls and tighter spacing; off restores the theme's default metrics.");
    let mut show_all = ui.state_mut::<ShowAllCode>(show_all_code_id()).0;
    if ui.toggle("Show all code", &mut show_all).changed {
        ui.state_mut::<ShowAllCode>(show_all_code_id()).0 = show_all;
    }
    let mut inspector = ui.inspector_enabled();
    if ui.toggle("Inspector", &mut inspector).changed {
        ui.set_inspector(inspector);
    }
    ui.tooltip("Outlines every widget with its id, bounds and state. Shortcut: F12.");
    ui.space(ui.theme().metrics.spacing);
    ui.horizontal_centered(|ui| {
        if ui.button_kind("Commands", ButtonKind::Secondary).clicked {
            ui.set_command_palette_open(true);
        }
        ui.kbd(PALETTE_SHORTCUT);
    });
}

pub(super) fn command_palette(ui: &mut Ui, page: &mut DemoPage) {
    match ui.command_palette(&COMMANDS) {
        Some(i) if i < PAGES.len() => *page = PAGES[i].page,
        Some(i) if i == PAGES.len() => toggle_theme(ui),
        Some(_) => {
            let on = !ui.inspector_enabled();
            ui.set_inspector(on);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_lists_every_page_in_order() {
        for (info, (name, _)) in PAGES.iter().zip(COMMANDS) {
            assert_eq!(name.strip_prefix("Go to "), Some(info.name));
        }
    }

    #[test]
    fn every_shown_shortcut_is_handled() {
        // Rugi handles F12 (inspector) itself; pages have no shortcuts.
        for (name, shortcut) in COMMANDS {
            assert!(shortcut.is_empty() || shortcut == INSPECTOR_SHORTCUT, "{name} shows an unbound shortcut");
        }
    }
}
