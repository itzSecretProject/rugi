//! Built-in tools: the command palette and the inspector overlay.

use crate::popup::PopupKind;
use crate::{Response, Sense, Ui};
use rugi_core::{Color, Id, Key, Rect, Vec2};
use std::rc::Rc;

/// The inspector draws in fixed dark colors so it reads the same on any theme.
const INSPECTOR_BG: Color = Color::hex(0x101116);
const INSPECTOR_TEXT: Color = Color::hex(0xE6E7EB);
/// Height of a command palette row.
const PALETTE_ROW_HEIGHT: f32 = 34.0;
/// Maximum number of commands listed by the palette.
const PALETTE_MAX_RESULTS: usize = 9;

#[derive(Default, Debug)]
pub(crate) struct ToolsState {
    /// Live inspector overlay enabled (F12 or Cmd/Ctrl+Shift+I).
    pub inspector: bool,
    palette_open: bool,
    palette_query: String,
    palette_sel: usize,
    palette_focus: bool,
}

/// Subsequence fuzzy match. Returns a score (higher is better) or `None` when
/// `query` is not a subsequence of `text`. Rewards consecutive runs and
/// matches at word starts, so "nev" ranks "New event" above "Open view".
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let mut q = query.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).peekable();
    if q.peek().is_none() {
        return Some(0);
    }
    let mut score = 0;
    let mut prev_matched = false;
    let mut prev_char = ' ';
    let mut gap = 0;
    for c in text.chars() {
        let lc = c.to_lowercase().next().unwrap_or(c);
        match q.peek() {
            Some(&want) if want == lc => {
                q.next();
                score += 10;
                if prev_matched {
                    score += 15;
                }
                if !prev_char.is_alphanumeric() {
                    score += 20;
                }
                score -= gap.min(10);
                gap = 0;
                prev_matched = true;
            }
            Some(_) => {
                prev_matched = false;
                gap += 1;
            }
            None => break,
        }
        prev_char = c;
    }
    if q.peek().is_some() {
        return None;
    }
    // Contiguous matches beat scattered ones, especially at a word start.
    let needle: String = query.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect();
    let hay: String = text.chars().flat_map(char::to_lowercase).collect();
    if let Some(pos) = hay.find(&needle) {
        score += 60;
        if hay[..pos].chars().next_back().is_none_or(|c| !c.is_alphanumeric()) {
            score += 40;
        }
    }
    Some(score - text.len() as i32 / 8)
}

impl Ui {
    /// Global shortcuts for the built-in tools (called from `begin_frame`).
    pub(crate) fn tool_shortcuts(&mut self) {
        for i in 0..self.input.keys.len() {
            let (k, m) = self.input.keys[i];
            if k == Key::F12 || (k == Key::I && m.command() && m.shift) {
                self.tools.inspector = !self.tools.inspector;
                self.request_repaint();
            }
        }
    }

    /// True while the live inspector overlay is enabled.
    pub fn inspector_enabled(&self) -> bool {
        self.tools.inspector
    }

    /// Enables or disables the live inspector overlay.
    pub fn set_inspector(&mut self, on: bool) {
        self.tools.inspector = on;
        self.request_repaint();
    }

    /// True while the command palette is open.
    pub fn command_palette_open(&self) -> bool {
        self.tools.palette_open
    }

    /// Opens or closes the command palette programmatically.
    pub fn set_command_palette_open(&mut self, open: bool) {
        self.tools.palette_open = open;
        if open {
            self.tools.palette_query.clear();
            self.tools.palette_sel = 0;
            self.tools.palette_focus = true;
        }
        self.request_repaint();
    }

    /// Command palette (Cmd/Ctrl+K): fuzzy search over `commands`
    /// (`(name, hint)` pairs; the hint is shown right-aligned, e.g. a
    /// shortcut). Returns the index of the command the user ran this frame.
    /// Call once per frame, after the rest of the UI.
    pub fn command_palette(&mut self, commands: &[(&str, &str)]) -> Option<usize> {
        if !self.frame_guard() {
            return None;
        }
        let toggle = self.input.keys.iter().any(|(k, m)| *k == Key::K && m.command());
        if toggle {
            let open = !self.tools.palette_open;
            self.set_command_palette_open(open);
        }
        if !self.tools.palette_open {
            return None;
        }
        if self.input.key_pressed(Key::Escape) {
            self.tools.palette_open = false;
            return None;
        }
        let mut matches: Vec<(i32, usize)> = commands
            .iter()
            .enumerate()
            .filter_map(|(i, (name, _))| fuzzy_score(&self.tools.palette_query, name).map(|s| (s, i)))
            .collect();
        matches.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        matches.truncate(PALETTE_MAX_RESULTS);
        let n = matches.len();
        for i in 0..self.input.keys.len() {
            match self.input.keys[i].0 {
                Key::ArrowDown if n > 0 => self.tools.palette_sel = (self.tools.palette_sel + 1) % n,
                Key::ArrowUp if n > 0 => self.tools.palette_sel = (self.tools.palette_sel + n - 1) % n,
                _ => {}
            }
        }
        if n > 0 {
            self.tools.palette_sel = self.tools.palette_sel.min(n - 1);
        }

        let id = Id::ROOT.with("##rugi-command-palette");
        self.popups.modal_active = true;
        let w = (self.screen.width() - 32.0).clamp(240.0, 600.0);
        let pos =
            Vec2::new((self.screen.center().x - w * 0.5).round(), (self.screen.height() * 0.14).round().max(16.0));
        self.enter_overlay();
        let scrim = if self.theme.dark { Color::rgba(0, 0, 0, 110) } else { Color::rgba(15, 18, 30, 60) };
        self.out.draw_list.push_clip(self.screen);
        self.out.draw_list.rect(self.screen, 0.0, scrim, None);
        self.out.draw_list.pop_clip();
        self.push_floating(id, pos, w, PopupKind::Modal, 10.0, 6.0);
        self.overlay_depth -= 1;

        self.push_id("##rugi-palette");
        let input_id = self.make_id("query");
        if std::mem::take(&mut self.tools.palette_focus) {
            self.focused = Some(input_id);
        }
        let mut query = std::mem::take(&mut self.tools.palette_query);
        let resp = self.with_style(
            |t| t.metrics.control_height = 42.0,
            |ui| ui.text_input("query", &mut query, "Search or run a command…"),
        );
        if resp.changed {
            self.tools.palette_sel = 0;
        }
        self.tools.palette_query = query;
        let mut chosen = None;
        if resp.submitted && n > 0 {
            chosen = Some(matches[self.tools.palette_sel].1);
        }
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        if n == 0 {
            self.space(4.0);
            self.muted("No matching commands");
        }
        for (row, &(_, idx)) in matches.iter().enumerate() {
            let (name, hint) = commands[idx];
            let rid = id.with_u64(idx as u64);
            let r = self.allocate(Vec2::new(0.0, PALETTE_ROW_HEIGHT), true);
            let rr: Response = self.interact(rid, r, Sense { focusable: false, ..Sense::BUTTON });
            if rr.hovered && self.input.pointer_delta != Vec2::ZERO {
                self.tools.palette_sel = row;
            }
            if rr.clicked {
                chosen = Some(idx);
            }
            let active = row == self.tools.palette_sel;
            if active {
                self.out.draw_list.rect(r, th.metrics.radius_md, p.accent_soft, None);
                self.out.draw_list.rect(Rect::from_xywh(r.min.x, r.min.y + 9.0, 3.0, 16.0), 1.5, p.accent, None);
            }
            let body = th.typography.body;
            let l = self.layout_text_fit(name, &body, (r.width() - 120.0).max(0.0));
            let ty = r.min.y + ((PALETTE_ROW_HEIGHT - l.size().y) * 0.5).round();
            self.paint_text(
                &l,
                Vec2::new(r.min.x + 14.0, ty),
                if active { p.text } else { p.text_muted.lerp(p.text, 0.6) },
            );
            if !hint.is_empty() {
                let cap = th.typography.caption;
                let hl = self.layout_text(hint, &cap, None);
                let hy = r.min.y + ((PALETTE_ROW_HEIGHT - hl.size().y) * 0.5).round();
                let hr = Rect::from_xywh(r.max.x - hl.size().x - 22.0, hy - 3.0, hl.size().x + 12.0, hl.size().y + 6.0);
                self.out.draw_list.rect(hr, 5.0, p.surface_active, Some((1.0, p.border)));
                self.paint_text(&hl, Vec2::new(hr.min.x + 6.0, hy), p.text_muted);
            }
            self.record("command", name, &rr);
        }
        self.separator();
        self.caption("↑ ↓ navigate   ↵ run   esc close");
        self.pop_id();
        self.end();
        if chosen.is_some() {
            self.tools.palette_open = false;
            self.request_repaint();
        }
        chosen
    }

    /// Draws the live inspector (widget bounds, hovered widget details and a
    /// performance HUD) into the overlay. Called from `end_frame`.
    pub(crate) fn inspector_overlay(&mut self) {
        if !self.tools.inspector {
            return;
        }
        let widgets = self.out.widgets.clone();
        let th = Rc::clone(&self.theme);
        let p = &th.palette;
        let accent = p.accent;
        self.enter_overlay();
        self.out.draw_list.push_clip(self.screen);
        let pointer = self.input.pointer;
        let mut hovered = None;
        for (i, w) in widgets.iter().enumerate() {
            let visible = w.rect.intersect(&w.clip);
            if visible.is_empty() {
                continue;
            }
            self.out.draw_list.push_clip(w.clip);
            self.out.draw_list.rect(w.rect, 2.0, Color::TRANSPARENT, Some((1.0, accent.with_alpha(0.35))));
            self.out.draw_list.pop_clip();
            if pointer.is_some_and(|ptr| visible.contains(ptr)) {
                hovered = Some(i);
            }
        }
        let mono = th.typography.caption;
        if let (Some(i), Some(ptr)) = (hovered, pointer) {
            let w = &widgets[i];
            self.out.draw_list.rect(w.rect, 2.0, accent.with_alpha(0.12), Some((1.5, accent)));
            let text = format!(
                "{}  \u{201c}{}\u{201d}\nid {:016x}\nx {:.1}  y {:.1}  w {:.1}  h {:.1}\nhovered {}  focused {}",
                w.kind,
                w.label,
                w.id.0,
                w.rect.min.x,
                w.rect.min.y,
                w.rect.width(),
                w.rect.height(),
                w.hovered,
                w.focused
            );
            let l = self.layout_text(&text, &mono, None);
            let size = l.size() + Vec2::new(20.0, 14.0);
            let mut pos = ptr + Vec2::new(14.0, 16.0);
            pos.x = pos.x.min(self.screen.max.x - size.x - 6.0).max(6.0);
            pos.y = pos.y.min(self.screen.max.y - size.y - 6.0).max(6.0);
            let r = Rect::from_min_size(pos, size);
            self.out.draw_list.shadow(r.translate(Vec2::new(0.0, 4.0)), 8.0, 10.0, p.shadow);
            self.out.draw_list.rect(r, 8.0, INSPECTOR_BG, Some((1.0, accent.with_alpha(0.6))));
            self.paint_text(&l, pos + Vec2::new(10.0, 7.0), INSPECTOR_TEXT);
        }
        // Performance HUD in the bottom-right corner.
        let s = self.last_stats;
        let d = self.display;
        let hz = d.refresh_hz.map(|h| format!("{h:.0} Hz")).unwrap_or_else(|| "? Hz".into());
        let hud = format!(
            "RUGI INSPECTOR   F12\nbuild {:.3} ms   {} widgets\n{} instances   {} batches   {} culled\ndisplay {}   {:.0} fps   {:.2} ms (p99 {:.2})",
            s.build_us as f32 / 1000.0,
            s.widgets,
            s.instances,
            s.batches,
            s.culled,
            hz,
            d.fps,
            d.frame_interval_ms,
            d.frame_interval_p99_ms
        );
        let l = self.layout_text(&hud, &mono, None);
        let size = l.size() + Vec2::new(20.0, 14.0);
        let r = Rect::from_min_size(self.screen.max - size - Vec2::new(12.0, 12.0), size);
        self.out.draw_list.rect(r, 8.0, INSPECTOR_BG.with_alpha(0.92), Some((1.0, accent.with_alpha(0.5))));
        self.paint_text(&l, r.min + Vec2::new(10.0, 7.0), INSPECTOR_TEXT);
        self.out.draw_list.pop_clip();
        self.exit_overlay();
        self.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn fuzzy_ranks_word_starts_and_runs() {
        assert!(fuzzy_score("xyz", "New event").is_none());
        let a = fuzzy_score("nev", "New event").unwrap();
        let b = fuzzy_score("nev", "Open view").unwrap_or(i32::MIN);
        assert!(a > b);
        assert!(fuzzy_score("theme", "Toggle theme").unwrap() > fuzzy_score("theme", "The menu").unwrap_or(i32::MIN));
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }
}
