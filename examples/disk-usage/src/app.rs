//! The window: a breadcrumb and filter on top, the current folder's entries
//! below, largest first.

use crate::scan::{Finished, Kind, Node, Scan};
use rugi::{lucide, App, ButtonKind, Key, Rect, TextInputOptions, Ui, Vec2};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

const TOP_BAR_HEIGHT: f32 = 52.0;
const FILTER_WIDTH: f32 = 220.0;
/// Breadcrumb segments shown before older ones collapse into "…".
const BREADCRUMB_SEGMENTS: usize = 4;
const PADDING_X: f32 = 12.0;
const COLUMN_GAP: f32 = 16.0;
const SIZE_WIDTH: f32 = 90.0;
const SHARE_WIDTH: f32 = 150.0;
const SHARE_BAR_WIDTH: f32 = 100.0;
const SHARE_BAR_HEIGHT: f32 = 6.0;
const FILES_WIDTH: f32 = 80.0;
const ICON_SIZE: f32 = 16.0;
const SCAN_REDRAW: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Column {
    Name,
    Size,
    Files,
}

impl Column {
    /// Names read A to Z; sizes and counts largest first.
    fn default_descending(self) -> bool {
        self != Column::Name
    }
}

struct Loaded {
    tree: Node,
    elapsed: Duration,
    stopped: bool,
    errors: u64,
}

pub struct DiskUsage {
    root: PathBuf,
    scan: Option<Scan>,
    loaded: Option<Loaded>,
    error: Option<String>,
    /// Child indices from the root to the open folder.
    path: Vec<usize>,
    /// Folder names to reopen after a rescan.
    reopen: Vec<String>,
    sort: Column,
    descending: bool,
    filter: String,
    /// Children of the open folder that match the filter, in display order.
    rows: Vec<usize>,
    selected: Option<usize>,
}

impl DiskUsage {
    pub fn new(root: PathBuf) -> Self {
        Self {
            scan: Some(Scan::start(root.clone())),
            root,
            loaded: None,
            error: None,
            path: Vec::new(),
            reopen: Vec::new(),
            sort: Column::Size,
            descending: true,
            filter: String::new(),
            rows: Vec::new(),
            selected: None,
        }
    }

    fn folder(&self) -> Option<&Node> {
        let mut node = &self.loaded.as_ref()?.tree;
        for &i in &self.path {
            node = &node.children[i];
        }
        Some(node)
    }

    /// Path on disk of child `index` of the open folder, or of the folder.
    fn full_path(&self, index: Option<usize>) -> PathBuf {
        let mut path = self.root.clone();
        let Some(mut node) = self.loaded.as_ref().map(|l| &l.tree) else {
            return path;
        };
        for &i in self.path.iter().chain(&index) {
            node = &node.children[i];
            path.push(&node.name);
        }
        path
    }

    fn rescan(&mut self) {
        self.reopen = self.names();
        self.loaded = None;
        self.error = None;
        self.scan = Some(Scan::start(self.root.clone()));
    }

    fn names(&self) -> Vec<String> {
        let Some(mut node) = self.loaded.as_ref().map(|l| &l.tree) else {
            return Vec::new();
        };
        self.path
            .iter()
            .map(|&i| {
                node = &node.children[i];
                node.name.clone()
            })
            .collect()
    }

    fn finish(&mut self, done: Finished) {
        self.scan = None;
        match done.tree {
            Ok(tree) => {
                self.path.clear();
                let mut node = &tree;
                for name in std::mem::take(&mut self.reopen) {
                    let Some(i) = node.children.iter().position(|c| c.name == name && c.kind == Kind::Folder) else {
                        break;
                    };
                    self.path.push(i);
                    node = &node.children[i];
                }
                self.loaded = Some(Loaded { tree, elapsed: done.elapsed, stopped: done.stopped, errors: done.errors });
                self.refresh();
            }
            Err(e) => self.error = Some(format!("Cannot read {}: {e}", self.root.display())),
        }
    }

    fn open(&mut self, index: usize) {
        self.path.push(index);
        self.filter.clear();
        self.refresh();
    }

    /// Opens the folder `depth` levels below the root.
    fn go_to(&mut self, depth: usize) {
        if depth < self.path.len() {
            let child = self.path[depth];
            self.path.truncate(depth);
            self.filter.clear();
            self.refresh();
            self.selected = Some(child);
        }
    }

    fn sort_by(&mut self, column: Column) {
        if self.sort == column {
            self.descending = !self.descending;
        } else {
            self.sort = column;
            self.descending = column.default_descending();
        }
        self.refresh();
    }

    /// Rebuilds `rows` from the open folder, the filter and the sort order.
    fn refresh(&mut self) {
        self.selected = None;
        let Some(folder) = self.folder() else {
            self.rows.clear();
            return;
        };
        let query = self.filter.trim().to_lowercase();
        let children = &folder.children;
        let mut rows: Vec<usize> =
            (0..children.len()).filter(|&i| children[i].name.to_lowercase().contains(&query)).collect();
        rows.sort_by(|&a, &b| {
            let (a, b) = (&children[a], &children[b]);
            let order = match self.sort {
                Column::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                Column::Size => a.size.cmp(&b.size),
                Column::Files => a.files.cmp(&b.files),
            };
            let order = if self.descending { order.reverse() } else { order };
            order.then_with(|| a.name.cmp(&b.name))
        });
        self.rows = rows;
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.begin_horizontal_centered(rugi::Align::End);
        let scanning = self.scan.is_some();
        let (label, icon) = if scanning { ("Stop", lucide::SQUARE) } else { ("Rescan", lucide::REFRESH_CW) };
        if ui.button_ex(label, ButtonKind::Secondary, Some(icon.into())).clicked {
            match &self.scan {
                Some(scan) => scan.stop(),
                None => self.rescan(),
            }
        }
        ui.set_next_item_width(FILTER_WIDTH);
        let options = TextInputOptions { icon: Some(lucide::SEARCH.into()), ..Default::default() };
        let filter = ui.text_input_ex("Filter", &mut self.filter, "Filter this folder", options);
        if filter.changed {
            self.refresh();
        }
        if !filter.focused && ui.key_pressed(Key::Backspace) && !self.path.is_empty() {
            self.go_to(self.path.len() - 1);
        }
        ui.with_style(
            |t| {
                t.metrics.spacing = 2.0;
                t.metrics.control_padding_x = 8.0;
            },
            |ui| ui.horizontal_centered(|ui| self.breadcrumb(ui)),
        );
        ui.end();
    }

    fn breadcrumb(&mut self, ui: &mut Ui) {
        let names = self.names();
        let root = self.root.display().to_string();
        let segments: Vec<&str> = std::iter::once(root.as_str()).chain(names.iter().map(String::as_str)).collect();
        let last = segments.len() - 1;
        let hidden = segments.len().saturating_sub(BREADCRUMB_SEGMENTS);
        let mut target = None;
        for (depth, name) in segments.iter().enumerate() {
            if depth > 0 && depth <= hidden {
                if depth == 1 {
                    ui.muted("/ …");
                }
                continue;
            }
            if depth > 0 {
                ui.muted("/");
            }
            ui.push_id_u64(depth as u64);
            if depth == last {
                ui.space(6.0);
                ui.label(name);
            } else if ui.button_kind(name, ButtonKind::Ghost).clicked {
                target = Some(depth);
            }
            ui.pop_id();
        }
        if let Some(depth) = target {
            self.go_to(depth);
        }
    }

    fn scanning(&self, ui: &mut Ui, scan: &Scan) {
        let p = &scan.progress;
        ui.heading(&format!("Scanning {}", scan.root.display()));
        let files = p.files.load(Ordering::Relaxed);
        let bytes = p.bytes.load(Ordering::Relaxed);
        let seconds = scan.elapsed().as_secs_f32();
        ui.label(&format!("{files} files, {} so far ({seconds:.1} s)", format_size(bytes)));
        if let Ok(current) = p.current.lock() {
            ui.label_truncated(&current.display().to_string(), rugi::TextRole::Muted, None);
        }
        ui.request_repaint_after(SCAN_REDRAW);
    }

    fn listing(&mut self, ui: &mut Ui) {
        let Some(folder) = self.folder() else { return };
        let mut summary = format!("{} in {} files", format_size(folder.size), folder.files);
        if !self.filter.trim().is_empty() {
            summary += &format!(" · {} of {} items match", self.rows.len(), folder.children.len());
        }
        if let Some(l) = &self.loaded {
            if l.stopped {
                summary += " · scan stopped, totals are incomplete";
            } else {
                summary += &format!(" · scanned in {}", format_duration(l.elapsed));
            }
            if l.errors > 0 {
                summary += &format!(" · {} could not be read", count(l.errors, "entry", "entries"));
            }
        }
        ui.begin_horizontal_centered(rugi::Align::End);
        let copy = ui.button_ex("Copy path", ButtonKind::Secondary, Some(lucide::COPY.into())).clicked;
        ui.tooltip("Copies the path of the selected file, or of this folder");
        ui.horizontal_centered(|ui| ui.label_truncated(&summary, rugi::TextRole::Muted, None));
        ui.end();
        if copy {
            ui.set_clipboard(&self.full_path(self.selected).display().to_string());
        }

        self.header(ui);
        let row_h = ui.theme().metrics.control_height;
        let mut action = None;
        ui.with_style(
            |t| t.metrics.spacing = 0.0,
            |ui| ui.scroll_area("entries", None, |ui| action = self.entries(ui, row_h)),
        );
        match action {
            Some(Action::Up) => self.go_to(self.path.len() - 1),
            Some(Action::Open(i)) => self.open(i),
            Some(Action::Select(i)) => self.selected = Some(i),
            Some(Action::Copy(text)) => ui.set_clipboard(&text),
            None => {}
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        let row_h = ui.theme().metrics.control_height - 4.0;
        let r = ui.hover_region("##header", Vec2::new(ui.available_width(), row_h)).rect;
        let cols = Columns::new(r);
        let p = ui.theme().palette.clone();
        let style = ui.theme().typography.caption.weight(rugi::core::FontWeight::MEDIUM);
        let mut clicked = None;
        for (column, label, (x0, x1), right) in [
            (Some(Column::Name), "Name", (cols.icon, cols.name.1), false),
            (Some(Column::Size), "Size", cols.size, true),
            (None, "Share", cols.share, false),
            (Some(Column::Files), "Files", cols.files, true),
        ] {
            let cell = Rect::from_xywh(x0, r.min.y, x1 - x0, r.height());
            let active = column.is_some() && column == Some(self.sort);
            let mut color = if active { p.text } else { p.text_muted };
            if let Some(column) = column {
                ui.begin_region(cell, true);
                let resp = ui.allocate_interactive(&format!("Sort by {}", label.to_lowercase()), cell.size());
                ui.end();
                if resp.hovered {
                    color = p.text;
                }
                if resp.clicked {
                    clicked = Some(column);
                }
            }
            let text = ui.layout_text(label, &style, None);
            let w = text.size().x + if active { ICON_SIZE } else { 0.0 };
            let x = if right { x1 - w } else { x0 };
            let y = r.min.y + ((r.height() - text.size().y) * 0.5).round();
            ui.paint_text(&text, Vec2::new(x, y), color);
            if active {
                let chevron = if self.descending { lucide::CHEVRON_DOWN } else { lucide::CHEVRON_UP };
                let icon = Rect::from_xywh(x + text.size().x + 2.0, r.center().y - 6.0, 12.0, 12.0);
                ui.paint_icon(chevron.into(), icon, color);
            }
        }
        ui.painter().rect(Rect::from_xywh(r.min.x, r.max.y - 1.0, r.width(), 1.0), 0.0, p.border, None);
        if let Some(column) = clicked {
            self.sort_by(column);
        }
    }

    /// The visible rows of the open folder. Rows outside the scroll view are
    /// replaced by empty space, so a folder with 100 000 entries costs the
    /// same as one with 20.
    fn entries(&self, ui: &mut Ui, row_h: f32) -> Option<Action> {
        let folder = self.folder()?;
        let up = usize::from(!self.path.is_empty());
        let count = up + self.rows.len();
        if count == 0 {
            ui.space(8.0);
            let empty = folder.error.as_deref().unwrap_or("Empty folder");
            ui.muted(if folder.children.is_empty() { empty } else { "No names match the filter" });
            return None;
        }
        let top = ui.hover_region("##top", Vec2::ZERO).rect.min.y;
        let view = ui.painter().clip();
        let first = (((view.min.y - top) / row_h).floor().max(0.0) as usize).min(count);
        let last = (((view.max.y - top) / row_h).ceil().max(0.0) as usize + 1).min(count);
        ui.space(first as f32 * row_h);
        let mut action = None;
        for row in first..last {
            let entry = if row < up { None } else { Some(self.rows[row - up]) };
            ui.push_id_u64(entry.map_or(0, |i| i as u64 + 1));
            if let Some(a) = self.entry(ui, folder, entry, row_h) {
                action = Some(a);
            }
            ui.pop_id();
        }
        ui.space((count - last) as f32 * row_h);
        action
    }

    /// One row: child `index` of `folder`, or the parent folder for `None`.
    fn entry(&self, ui: &mut Ui, folder: &Node, index: Option<usize>, row_h: f32) -> Option<Action> {
        let node = index.map(|i| &folder.children[i]);
        let name = node.map_or("..", |n| n.name.as_str());
        let resp = ui.allocate_interactive(name, Vec2::new(ui.available_width(), row_h));
        let r = resp.rect;
        let (hover, _, focus) = ui.interaction_anim(&resp);
        let p = ui.theme().palette.clone();
        let (style, caption) = (ui.theme().typography.body, ui.theme().typography.caption);
        let selected = index.is_some() && index == self.selected;
        let background = if selected { p.accent_soft } else { p.surface_hover.fade(hover) };
        let radius = ui.theme().metrics.radius_sm;
        ui.painter().rect(r, radius, background, None);
        ui.paint_focus_ring(r, radius, focus);

        let cols = Columns::new(r);
        let icon = match node.map(|n| n.kind) {
            None => lucide::CORNER_LEFT_UP,
            Some(Kind::Folder) => lucide::FOLDER,
            Some(Kind::File) => lucide::FILE,
            Some(Kind::Link) => lucide::LINK,
        };
        let icon_color = if node.is_some_and(|n| n.kind == Kind::Folder) { p.accent } else { p.text_muted };
        let mid = r.center().y;
        ui.paint_icon(icon.into(), Rect::from_xywh(cols.icon, mid - ICON_SIZE * 0.5, ICON_SIZE, ICON_SIZE), icon_color);
        let warn = node.is_some_and(|n| n.error.is_some() || n.unreadable > 0);
        let name_w = cols.name.1 - cols.name.0 - if warn { ICON_SIZE + 6.0 } else { 0.0 };
        let text = ui.layout_text_fit(name, &style, name_w.max(0.0));
        let ty = r.min.y + ((r.height() - text.size().y) * 0.5).round();
        ui.paint_text(&text, Vec2::new(cols.name.0, ty), p.text);
        if warn {
            let x = cols.name.0 + text.size().x + 6.0;
            ui.paint_icon(lucide::TRIANGLE_ALERT.into(), Rect::from_xywh(x, mid - 7.0, 14.0, 14.0), p.warning);
        }

        let Some(node) = node else {
            return resp.clicked.then_some(Action::Up);
        };
        let size = ui.layout_text(&format_size(node.size), &style, None);
        ui.paint_text(&size, Vec2::new(cols.size.1 - size.size().x, ty), p.text);
        let share = if folder.size > 0 { node.size as f32 / folder.size as f32 } else { 0.0 };
        let bar = Rect::from_xywh(cols.share.0, mid - SHARE_BAR_HEIGHT * 0.5, SHARE_BAR_WIDTH, SHARE_BAR_HEIGHT);
        let round = SHARE_BAR_HEIGHT * 0.5;
        ui.painter().rect(bar, round, p.surface_active, None);
        if share > 0.0 {
            let fill = Rect::from_xywh(bar.min.x, bar.min.y, (bar.width() * share).max(SHARE_BAR_HEIGHT), bar.height());
            ui.painter().rect(fill, round, p.accent, None);
        }
        let percent = ui.layout_text(&format!("{:.0}%", share * 100.0), &caption, None);
        let py = r.min.y + ((r.height() - percent.size().y) * 0.5).round();
        ui.paint_text(&percent, Vec2::new(cols.share.1 - percent.size().x, py), p.text_muted);
        if node.kind == Kind::Folder {
            let files = ui.layout_text(&node.files.to_string(), &style, None);
            ui.paint_text(&files, Vec2::new(cols.files.1 - files.size().x, ty), p.text_muted);
        }

        let index = index?;
        let folder_kind = node.kind == Kind::Folder;
        let mut action = resp.clicked.then_some(if folder_kind { Action::Open(index) } else { Action::Select(index) });
        let menu = ui.context_menu("menu", |ui| {
            if folder_kind && ui.menu_item("Open", "").clicked {
                return Some(Action::Open(index));
            }
            if ui.menu_item("Copy path", "").clicked {
                return Some(Action::Copy(self.full_path(Some(index)).display().to_string()));
            }
            if ui.menu_item("Copy name", "").clicked {
                return Some(Action::Copy(node.name.clone()));
            }
            None
        });
        match menu {
            // The row stays selected while its menu is open.
            Some(None) if self.selected != Some(index) => action = Some(Action::Select(index)),
            Some(Some(a)) => action = Some(a),
            _ => {}
        }
        if let Some(e) = &node.error {
            ui.tooltip(e);
        } else if node.unreadable > 0 {
            ui.tooltip(&format!("{} inside could not be read", count(node.unreadable, "entry", "entries")));
        }
        action
    }
}

enum Action {
    Up,
    Open(usize),
    Select(usize),
    Copy(String),
}

/// Column positions of a row: x of the icon, and (left, right) of the rest.
struct Columns {
    icon: f32,
    name: (f32, f32),
    size: (f32, f32),
    share: (f32, f32),
    files: (f32, f32),
}

impl Columns {
    fn new(r: Rect) -> Self {
        let right = r.max.x - PADDING_X;
        let files = (right - FILES_WIDTH, right);
        let share = (files.0 - COLUMN_GAP - SHARE_WIDTH, files.0 - COLUMN_GAP);
        let size = (share.0 - COLUMN_GAP - SIZE_WIDTH, share.0 - COLUMN_GAP);
        let icon = r.min.x + PADDING_X;
        Self { icon, name: (icon + ICON_SIZE + 10.0, size.0 - COLUMN_GAP), size, share, files }
    }
}

impl App for DiskUsage {
    fn frame(&mut self, ui: &mut Ui) {
        if let Some(done) = self.scan.as_mut().and_then(Scan::poll) {
            self.finish(done);
        }
        ui.top_bar(TOP_BAR_HEIGHT, |ui| self.toolbar(ui));
        ui.central_panel(|ui| {
            if let Some(scan) = &self.scan {
                self.scanning(ui, scan);
            } else if let Some(e) = &self.error {
                let (style, color) = (ui.theme().typography.body, ui.theme().palette.danger);
                ui.text_styled(e, style, color, true);
            } else {
                self.listing(ui);
            }
        });
    }
}

/// Bytes in binary units: 512 B, 4.0 KiB, 1.2 GiB.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["KiB", "MiB", "GiB", "TiB", "PiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// "1 entry", "2 entries".
fn count(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A duration as "40 ms" or "2.5 s".
fn format_duration(d: Duration) -> String {
    if d < Duration::from_secs(1) {
        format!("{} ms", d.as_millis())
    } else {
        format!("{:.1} s", d.as_secs_f32())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rugi::core::{InputEvent, PointerButton};
    use rugi::{FrameInput, UiOptions};
    use std::fs;

    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        /// big/ (30 kB in two files), small.txt (5 kB), mid.bin (12 kB).
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("rugi-disk-usage-{name}-{}", std::process::id()));
            fs::create_dir_all(dir.join("big")).unwrap();
            fs::write(dir.join("big/a.bin"), vec![0u8; 20_000]).unwrap();
            fs::write(dir.join("big/b.bin"), vec![0u8; 10_000]).unwrap();
            fs::write(dir.join("small.txt"), vec![0u8; 5_000]).unwrap();
            fs::write(dir.join("mid.bin"), vec![0u8; 12_000]).unwrap();
            Self { dir: fs::canonicalize(&dir).unwrap() }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn frame(ui: &mut Ui, app: &mut DiskUsage, events: &[InputEvent]) {
        for e in events {
            ui.push_event(e.clone());
        }
        let input = FrameInput { size: Vec2::new(1000.0, 680.0), scale: 1.0, dt: 1.0 / 60.0 };
        ui.begin_frame(input).unwrap();
        app.frame(ui);
        ui.end_frame();
    }

    fn scanned(dir: &std::path::Path) -> (Ui, DiskUsage) {
        let mut ui = Ui::new(UiOptions { inspection: true, ..Default::default() });
        let mut app = DiskUsage::new(dir.to_path_buf());
        for _ in 0..500 {
            frame(&mut ui, &mut app, &[]);
            if app.loaded.is_some() {
                return (ui, app);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("scan did not finish");
    }

    fn names(app: &DiskUsage) -> Vec<&str> {
        let folder = app.folder().unwrap();
        app.rows.iter().map(|&i| folder.children[i].name.as_str()).collect()
    }

    /// Clicks the widget labelled `label`; returns what the click put on the clipboard.
    fn click(ui: &mut Ui, app: &mut DiskUsage, label: &str) -> Option<String> {
        let w = ui.output().widgets.iter().find(|w| w.label == label).unwrap_or_else(|| panic!("no {label}"));
        let p = w.rect.center();
        let button = PointerButton::Primary;
        frame(ui, app, &[InputEvent::PointerMoved { x: p.x, y: p.y }]);
        frame(ui, app, &[InputEvent::PointerButton { button, pressed: true }]);
        frame(ui, app, &[InputEvent::PointerButton { button, pressed: false }]);
        let clipboard = ui.output().clipboard.clone();
        frame(ui, app, &[]);
        clipboard
    }

    #[test]
    fn lists_largest_first_and_sorts_by_name() {
        let fixture = Fixture::new("sort");
        let (mut ui, mut app) = scanned(&fixture.dir);
        assert_eq!(names(&app), ["big", "mid.bin", "small.txt"]);
        click(&mut ui, &mut app, "Sort by name");
        assert_eq!(names(&app), ["big", "mid.bin", "small.txt"]);
        click(&mut ui, &mut app, "Sort by name");
        assert_eq!(names(&app), ["small.txt", "mid.bin", "big"]);
    }

    #[test]
    fn opens_folders_and_copies_paths() {
        let fixture = Fixture::new("open");
        let (mut ui, mut app) = scanned(&fixture.dir);
        click(&mut ui, &mut app, "big");
        assert_eq!(names(&app), ["a.bin", "b.bin"]);
        click(&mut ui, &mut app, "b.bin");
        let copied = click(&mut ui, &mut app, "Copy path");
        assert_eq!(copied, Some(fixture.dir.join("big").join("b.bin").display().to_string()));
        click(&mut ui, &mut app, "..");
        assert!(app.path.is_empty());
    }

    #[test]
    fn filter_matches_names() {
        let fixture = Fixture::new("filter");
        let (_ui, mut app) = scanned(&fixture.dir);
        app.filter = "BIN".into();
        app.refresh();
        assert_eq!(names(&app), ["mid.bin"]);
    }

    #[test]
    fn sizes_in_binary_units() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(4096), "4.0 KiB");
        assert_eq!(format_size(3 * 1024 * 1024 / 2), "1.5 MiB");
    }
}
