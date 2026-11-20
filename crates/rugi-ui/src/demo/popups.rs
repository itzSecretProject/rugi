//! Menus, context menus, popups, tooltips and modal dialogs.

use super::two_columns;
use crate::{lucide, ButtonKind, Key, Ui};

pub(super) const SOURCE: &str = include_str!("popups.rs");

const FILES: [&str; 4] = ["budget.xlsx", "photo.jpg", "notes.md", "song.flac"];
const KINDS: [&str; 3] = ["Documents", "Images", "Audio"];
/// Files of the "Popup" section and the index of their kind in `KINDS`.
const KIND_FILES: [(&str, usize); 6] =
    [("budget.xlsx", 0), ("notes.md", 0), ("photo.jpg", 1), ("diagram.png", 1), ("song.flac", 2), ("interview.mp3", 2)];
/// Shortcuts shown in the menus. Each one is handled in its section.
const NEW_SHORTCUT: &str = if cfg!(target_os = "macos") { "⌘N" } else { "Ctrl N" };
const DUPLICATE_SHORTCUT: &str = if cfg!(target_os = "macos") { "⌘D" } else { "Ctrl D" };
const DELETE_SHORTCUT: &str = "Delete";
const MENUS_NOTE: &str = if cfg!(target_os = "macos") {
    "Click a menu, then hover its neighbours to switch between them. ⌘N also works with the menu closed."
} else {
    "Click a menu, then hover its neighbours to switch between them. Ctrl N also works with the menu closed."
};

pub(super) struct State {
    documents: Vec<String>,
    created: u32,
    numbered: bool,
    files: Vec<String>,
    selected: usize,
    copied: bool,
    kinds: [bool; 3],
    dialog: bool,
    keep_backup: bool,
    result: &'static str,
}

impl Default for State {
    fn default() -> Self {
        Self {
            documents: vec!["Untitled 1".into()],
            created: 1,
            numbered: false,
            files: FILES.map(String::from).to_vec(),
            selected: 0,
            copied: false,
            kinds: [true, true, false],
            dialog: false,
            keep_backup: true,
            result: "not opened yet",
        }
    }
}

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    two_columns(ui, app, left, right);
}

fn left(ui: &mut Ui, app: &mut State) {
    demo_section!(ui, "Menus", MENUS_NOTE, |ui| {
        let mut new = ui.command_pressed(Key::N);
        ui.horizontal(|ui| {
            ui.menu("File", |ui| {
                new |= ui.menu_item("New document", NEW_SHORTCUT).clicked;
                if ui.menu_item("Close all", "").clicked {
                    app.documents.clear();
                }
            });
            ui.menu("View", |ui| {
                if ui.selectable("Numbered list", app.numbered).clicked {
                    app.numbered = !app.numbered;
                }
            });
        });
        if new {
            app.created += 1;
            app.documents.push(format!("Untitled {}", app.created));
        }
        for (i, doc) in app.documents.iter().enumerate() {
            if app.numbered {
                ui.label_fmt(format_args!("{}. {doc}", i + 1));
            } else {
                ui.label(doc);
            }
        }
        if app.documents.is_empty() {
            ui.muted("No documents.");
        }
    });
    demo_section!(
        ui,
        "Context menu",
        "Right-click a file, or focus it and press Shift+F10. The shortcuts act on the focused file.",
        |ui| {
            let mut action = None;
            for (i, name) in app.files.iter().enumerate() {
                ui.push_id_u64(i as u64);
                let r = ui.selectable(name, app.selected == i);
                if r.clicked {
                    app.selected = i;
                }
                if r.focused && ui.command_pressed(Key::D) {
                    action = Some((i, true));
                }
                if r.focused && ui.key_pressed(Key::Delete) {
                    action = Some((i, false));
                }
                ui.context_menu("##file", |ui| {
                    if ui.menu_item("Duplicate", DUPLICATE_SHORTCUT).clicked {
                        action = Some((i, true));
                    }
                    if ui.menu_item("Delete", DELETE_SHORTCUT).clicked {
                        action = Some((i, false));
                    }
                });
                ui.pop_id();
            }
            match action {
                Some((i, true)) => {
                    let copy = format!("{} copy", app.files[i]);
                    app.files.insert(i + 1, copy);
                }
                Some((i, false)) => {
                    app.files.remove(i);
                    app.selected = app.selected.min(app.files.len().saturating_sub(1));
                }
                None => {}
            }
            if ui.button_kind("Restore", ButtonKind::Secondary).clicked {
                app.files = FILES.map(String::from).to_vec();
            }
        }
    );
}

fn right(ui: &mut Ui, app: &mut State) {
    demo_section!(ui, "Popup", "Any widget can anchor a popup. Click outside or press Esc to close it.", |ui| {
        let id = ui.make_id("##kinds");
        let icon = Some(lucide::FUNNEL.into());
        let r = ui.button_ex("Filter", ButtonKind::Secondary, icon);
        if r.clicked {
            ui.open_popup(id, r.rect);
        }
        ui.popup_below(id, r.rect, |ui| {
            for (on, kind) in app.kinds.iter_mut().zip(KINDS) {
                ui.checkbox(kind, on);
            }
        });
        for (name, kind) in KIND_FILES {
            if app.kinds[kind] {
                ui.label(name);
            }
        }
        if !app.kinds.contains(&true) {
            ui.muted("No kinds selected.");
        }
    });
    demo_section!(ui, "Tooltips", "Shown after a short hover delay; long text wraps.", |ui| {
        let path = "~/Documents/notes.md";
        ui.horizontal_centered(|ui| {
            let size = ui.theme().metrics.control_height;
            if ui.icon_button("##copy", lucide::COPY.into(), size, "Copy the path").clicked {
                ui.set_clipboard(path);
                app.copied = true;
            }
            ui.label(path);
            ui.help_marker("A help marker shows its text as a tooltip, for details most people can skip.");
        });
        if app.copied {
            ui.muted("Copied to the clipboard.");
        }
    });
    demo_section!(ui, "Modal dialog", "Dims the window and takes all input until it closes.", |ui| {
        if ui.button_kind("Open dialog", ButtonKind::Secondary).clicked {
            app.dialog = true;
        }
        ui.label_fmt(format_args!("Result: {}", app.result));
        let choice = ui.modal("##discard", "Discard changes?", &mut app.dialog, 420.0, |ui| {
            ui.paragraph("Unsaved edits to notes.md are lost.");
            ui.checkbox("Keep a backup", &mut app.keep_backup);
            let mut choice = None;
            ui.horizontal_right(|ui| {
                if ui.button_kind("Discard", ButtonKind::Danger).clicked {
                    choice = Some(if app.keep_backup { "discarded, backup kept" } else { "discarded" });
                }
                if ui.button_kind("Cancel", ButtonKind::Secondary).clicked {
                    choice = Some("cancelled");
                }
            });
            choice
        });
        if let Some(Some(result)) = choice {
            app.result = result;
            app.dialog = false;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shortcut constants shown in menus, and the call that handles each.
    const HANDLED: [(&str, &str); 3] = [
        ("NEW_SHORTCUT", "ui.command_pressed(Key::N)"),
        ("DUPLICATE_SHORTCUT", "ui.command_pressed(Key::D)"),
        ("DELETE_SHORTCUT", "ui.key_pressed(Key::Delete)"),
    ];

    #[test]
    fn every_shown_shortcut_is_handled() {
        let code = &SOURCE[..SOURCE.find("#[cfg(test)]").unwrap_or(SOURCE.len())];
        let mut items = 0;
        for (at, _) in code.match_indices("ui.menu_item(") {
            let call = &code[at..];
            let args = &call[..call.find(')').unwrap_or(call.len())];
            let shortcut = args.rsplit(',').next().unwrap_or("").trim();
            items += 1;
            if shortcut == "\"\"" {
                continue;
            }
            let handler = HANDLED.iter().find(|(name, _)| *name == shortcut).map(|(_, h)| *h);
            let handler = handler.unwrap_or_else(|| panic!("{args}) shows an unknown shortcut"));
            assert!(code.contains(handler), "{shortcut} is shown but `{handler}` is never called");
        }
        assert!(items >= 3, "found only {items} menu items");
    }
}
