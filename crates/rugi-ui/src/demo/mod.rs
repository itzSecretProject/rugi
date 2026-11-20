//! `Ui::show_demo()`: an interactive tour of the widget library, in the spirit
//! of Dear ImGui's demo window. It only uses the public API. Every section is
//! written with [`demo_section!`], which runs a block and shows that block's
//! source text next to it, so the code on screen is the code that runs.

/// Runs `$body` as a demo section titled `$title` and shows the block's own
/// source (read from the page's `SOURCE`) as its code sample. With
/// `code_open`, the code is shown until the user hides it.
macro_rules! demo_section {
    ($ui:expr, $title:expr, $note:expr, |$u:ident| $body:block) => {
        demo_section!(@run $ui, $title, $note, false, |$u| $body)
    };
    ($ui:expr, $title:expr, $note:expr, code_open, |$u:ident| $body:block) => {
        demo_section!(@run $ui, $title, $note, true, |$u| $body)
    };
    (@run $ui:expr, $title:expr, $note:expr, $open:expr, |$u:ident| $body:block) => {{
        static CODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let code = CODE.get_or_init(|| $crate::demo::block_source(SOURCE, line!()));
        $crate::demo::section($ui, $title, $note, code, $open, |$u: &mut $crate::Ui| $body);
    }};
}

mod basics;
mod data;
mod fonts;
mod inputs;
mod layout;
mod metrics;
mod popups;
mod shell;
mod style;
mod welcome;

use crate::{lucide, ButtonKind, Id, Theme, Ui};
use rugi_theme::Metrics;

/// Pages of the demo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DemoPage {
    /// Introduction, shortcuts and getting started.
    #[default]
    Welcome,
    /// Buttons, toggles, sliders, text.
    Basics,
    /// Text input, combo boxes, lists, tabs.
    Inputs,
    /// Rows, columns, cards, scrolling, trees.
    Layout,
    /// Menus, context menus, tooltips, dialogs, command palette.
    Popups,
    /// Virtualized table and plots.
    Data,
    /// Live theme editor.
    Style,
    /// Typography and the icon set.
    Fonts,
    /// Frame timing and engine statistics.
    Metrics,
}

struct PageInfo {
    page: DemoPage,
    name: &'static str,
}

const PAGES: [PageInfo; 9] = [
    PageInfo { page: DemoPage::Welcome, name: "Welcome" },
    PageInfo { page: DemoPage::Basics, name: "Basics" },
    PageInfo { page: DemoPage::Inputs, name: "Inputs & selection" },
    PageInfo { page: DemoPage::Layout, name: "Layout" },
    PageInfo { page: DemoPage::Popups, name: "Popups & overlays" },
    PageInfo { page: DemoPage::Data, name: "Data & plots" },
    PageInfo { page: DemoPage::Style, name: "Style editor" },
    PageInfo { page: DemoPage::Fonts, name: "Fonts & icons" },
    PageInfo { page: DemoPage::Metrics, name: "Metrics" },
];

/// Content width above which pages use two columns of sections.
const TWO_COLUMN_WIDTH: f32 = 900.0;
/// Space between the two columns of a page.
const COLUMN_GAP: f32 = 24.0;
/// Width of the page list.
const SIDEBAR_WIDTH: f32 = 200.0;

struct DemoState {
    page: DemoPage,
    compact: bool,
    /// Whether the compact metrics were applied to the theme.
    started: bool,
    welcome: welcome::State,
    basics: basics::State,
    inputs: inputs::State,
    layout: layout::State,
    popups: popups::State,
    data: data::State,
    style: style::State,
    fonts: fonts::State,
    metrics: metrics::State,
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            page: DemoPage::default(),
            compact: true,
            started: false,
            welcome: Default::default(),
            basics: Default::default(),
            inputs: Default::default(),
            layout: Default::default(),
            popups: Default::default(),
            data: Default::default(),
            style: Default::default(),
            fonts: Default::default(),
            metrics: Default::default(),
        }
    }
}

/// Whether every section shows its code, kept in the `Ui` state so that
/// [`section`] can read it.
#[derive(Default)]
struct ShowAllCode(bool);

/// Whether one section shows its code, once the user has toggled it.
#[derive(Default)]
struct CodeOpen(Option<bool>);

fn state_id() -> Id {
    Id::ROOT.with("##rugi-demo")
}

fn show_all_code_id() -> Id {
    state_id().with("##show-all-code")
}

impl Ui {
    /// Shows the Rugi demo as the whole window. Call it at the root level,
    /// once per frame; like `ImGui::ShowDemoWindow()`, it keeps its own state.
    /// On its first frame it switches the theme to compact metrics; the
    /// "Compact controls" switch in its sidebar restores the default ones.
    pub fn show_demo(&mut self) {
        if !self.frame_guard() {
            return;
        }
        let id = state_id();
        let mut st = std::mem::take(self.state_mut::<Option<Box<DemoState>>>(id)).unwrap_or_default();
        if !st.started {
            set_compact(self, st.compact);
            st.started = true;
        }
        demo_frame(self, &mut st);
        *self.state_mut::<Option<Box<DemoState>>>(id) = Some(st);
    }

    /// Selects the page shown by [`Ui::show_demo`].
    pub fn set_demo_page(&mut self, page: DemoPage) {
        let st = self.state_mut::<Option<Box<DemoState>>>(state_id());
        st.get_or_insert_with(Default::default).page = page;
    }
}

fn demo_frame(ui: &mut Ui, st: &mut DemoState) {
    st.metrics.record(ui.frame_index().saturating_sub(1), ui.last_frame_stats());
    ui.side_panel(SIDEBAR_WIDTH, |ui| shell::sidebar(ui, st));
    ui.central_panel(|ui| {
        ui.scroll_area("##demo-page", None, |ui| match st.page {
            DemoPage::Welcome => welcome::page(ui, &mut st.welcome),
            DemoPage::Basics => basics::page(ui, &mut st.basics),
            DemoPage::Inputs => inputs::page(ui, &mut st.inputs),
            DemoPage::Layout => layout::page(ui, &mut st.layout),
            DemoPage::Popups => popups::page(ui, &mut st.popups),
            DemoPage::Data => data::page(ui, &mut st.data),
            DemoPage::Style => style::page(ui, &mut st.style),
            DemoPage::Fonts => fonts::page(ui, &mut st.fonts),
            DemoPage::Metrics => metrics::page(ui, &mut st.metrics),
        });
    });
    shell::command_palette(ui, &mut st.page);
}

/// The default theme of the current kind (dark or light).
fn base_theme(dark: bool) -> Theme {
    if dark {
        Theme::dark()
    } else {
        Theme::light()
    }
}

/// Switches between the dark and light themes, keeping metrics, typography,
/// motion and accent.
fn toggle_theme(ui: &mut Ui) {
    let old = ui.theme().clone();
    let mut t = base_theme(!old.dark);
    t.metrics = old.metrics;
    t.typography = old.typography;
    t.motion = old.motion;
    t.set_accent(old.palette.accent);
    ui.set_theme(t);
}

/// Sets the theme's metrics to the demo's compact ones (26 px controls) or
/// to the theme defaults.
fn set_compact(ui: &mut Ui, compact: bool) {
    let mut m = base_theme(ui.theme().dark).metrics;
    if compact {
        compact_metrics(&mut m);
    }
    ui.theme_mut().metrics = m;
}

/// Metrics for dense, tool-like layouts.
fn compact_metrics(m: &mut Metrics) {
    m.radius_sm = 3.0;
    m.radius_md = 5.0;
    m.radius_lg = 6.0;
    m.spacing = 6.0;
    m.spacing_sm = 4.0;
    m.spacing_lg = 12.0;
    m.panel_padding = 12.0;
    m.card_padding = 10.0;
    m.control_height = 26.0;
    m.control_padding_x = 10.0;
    m.checkbox_size = 16.0;
    m.slider_thumb = 14.0;
    m.label_width = 130.0;
}

/// Lays out a page as two independent columns of sections, so sections of
/// different heights leave no gaps. Narrow windows, and pages showing all
/// code, use one column.
fn two_columns<S>(ui: &mut Ui, state: &mut S, left: fn(&mut Ui, &mut S), right: fn(&mut Ui, &mut S)) {
    let all_code = ui.state_mut::<ShowAllCode>(show_all_code_id()).0;
    let width = ui.available_width();
    if width > TWO_COLUMN_WIDTH && !all_code {
        let column = ((width - COLUMN_GAP - ui.theme().metrics.spacing * 2.0) * 0.5).floor();
        ui.horizontal(|ui| {
            ui.begin_vertical_width(column);
            left(ui, state);
            ui.end();
            ui.space(COLUMN_GAP);
            ui.begin_vertical_width(column);
            right(ui, state);
            ui.end();
        });
    } else {
        left(ui, state);
        right(ui, state);
    }
}

/// A demo section: a header with the title and a code toggle, an optional
/// one-line note, the live widgets and, when shown, the code that produced
/// them, followed by a separator.
fn section(ui: &mut Ui, title: &str, note: &str, code: &str, code_open: bool, body: impl FnOnce(&mut Ui)) {
    ui.scope(title, |ui| {
        let all = ui.state_mut::<ShowAllCode>(show_all_code_id()).0;
        let id = ui.make_id("##code-open");
        let toggled = ui.state_mut::<CodeOpen>(id).0;
        let mut open = toggled.unwrap_or(code_open);
        ui.horizontal_centered(|ui| {
            ui.heading(title);
            if !all {
                ui.horizontal_right(|ui| {
                    let label = if open { "Hide code###code" } else { "Show code###code" };
                    let icon = Some(lucide::CODE.into());
                    if ui.button_ex(label, ButtonKind::Ghost, icon).clicked {
                        open = !open;
                    }
                });
            }
        });
        if open != toggled.unwrap_or(code_open) {
            ui.state_mut::<CodeOpen>(id).0 = Some(open);
        }
        if !note.is_empty() {
            ui.paragraph(note);
        }
        body(ui);
        if open || all {
            ui.code_block(code);
        }
        let gap = ui.theme().metrics.spacing;
        ui.space(gap);
        ui.separator();
        ui.space(gap);
    });
}

/// The source text of the block passed to the `demo_section!` invocation on
/// `line` (1-based) of `source`, without its braces and common indentation.
fn block_source(source: &str, line: u32) -> String {
    let start = source.split_inclusive('\n').take(line.saturating_sub(1) as usize).map(str::len).sum::<usize>();
    let rest = &source[start.min(source.len())..];
    let Some(open) = rest.find("| {") else { return String::new() };
    let body = &rest[open + 3..];
    let end = matching_brace(body).unwrap_or(body.len());
    dedent(&body[..end])
}

/// Byte offset of the `}` closing a block whose `{` precedes `s`. Skips
/// braces inside string and character literals and comments.
fn matching_brace(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let (mut depth, mut i) = (0usize, 0usize);
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'\'' if i + 2 < b.len() && (b[i + 2] == b'\'' || b[i + 1] == b'\\') => {
                i += if b[i + 1] == b'\\' { 3 } else { 2 };
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'{' => depth += 1,
            b'}' if depth == 0 => return Some(i),
            b'}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Removes the indentation common to all non-blank lines, and blank lines at
/// both ends.
fn dedent(s: &str) -> String {
    let indent = s.lines().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let mut out = String::with_capacity(s.len());
    for l in s.lines().skip_while(|l| l.trim().is_empty()) {
        out.push_str(l.get(indent..).unwrap_or("").trim_end());
        out.push('\n');
    }
    out.truncate(out.trim_end().len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Longest code line. A code panel in a two-column page of a 1440 px wide
    /// window fits 76 characters and wraps longer lines; "Show all code"
    /// switches to one column, which fits these.
    const MAX_CODE_LINE: usize = 100;

    #[test]
    fn block_source_returns_the_dedented_body() {
        let src = "fn f() {\n    demo_section!(ui, \"T\", \"\", |ui| {\n        ui.label(\"{ not a brace\");\n        if x {\n            y('}');\n        } // }\n    });\n}\n";
        assert_eq!(block_source(src, 2), "ui.label(\"{ not a brace\");\nif x {\n    y('}');\n} // }");
    }

    #[test]
    fn every_page_section_has_code() {
        let mut long = Vec::new();
        for (name, source) in [
            ("basics", basics::SOURCE),
            ("data", data::SOURCE),
            ("fonts", fonts::SOURCE),
            ("inputs", inputs::SOURCE),
            ("layout", layout::SOURCE),
            ("metrics", metrics::SOURCE),
            ("popups", popups::SOURCE),
            ("style", style::SOURCE),
            ("welcome", welcome::SOURCE),
        ] {
            let lines: Vec<u32> = source
                .lines()
                .enumerate()
                .filter(|(_, l)| l.trim_start().starts_with("demo_section!("))
                .map(|(i, _)| i as u32 + 1)
                .collect();
            assert!(!lines.is_empty(), "{name} has no sections");
            for line in lines {
                let code = block_source(source, line);
                assert!(code.lines().count() >= 1 && !code.starts_with(' '), "{name}:{line} → {code:?}");
                long.extend(
                    code.lines().filter(|l| l.chars().count() > MAX_CODE_LINE).map(|l| format!("{name}:{line}: {l}")),
                );
            }
        }
        assert!(long.is_empty(), "code lines that wrap in the code panel:\n{}", long.join("\n"));
    }
}
