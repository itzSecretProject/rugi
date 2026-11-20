//! Single-line text input.

use crate::input_state::EditEvent;
use crate::{Channel, IconRef, Response, Sense, Ui};
use rugi_core::{id::split_label, CursorIcon, Key, Rect, Vec2};
use std::rc::Rc;

/// Options for [`Ui::text_input_ex`].
#[derive(Clone, Copy, Debug, Default)]
pub struct TextInputOptions<'a> {
    /// Show `•` instead of the characters and disable copy/cut.
    pub password: bool,
    /// Icon drawn at the start of the field.
    pub icon: Option<IconRef<'a>>,
}

const MASK: char = '•';

/// Byte index in `value` → byte index in the masked string.
fn to_masked(value: &str, i: usize) -> usize {
    value[..i].chars().count() * MASK.len_utf8()
}

/// Byte index in the masked string → byte index in `value`.
fn from_masked(value: &str, i: usize) -> usize {
    let n = i / MASK.len_utf8();
    value.char_indices().nth(n).map(|(j, _)| j).unwrap_or(value.len())
}

#[derive(Default)]
struct EditState {
    cursor: usize,
    anchor: usize,
    scroll_x: f32,
    blink_epoch: f64,
}

fn prev_boundary(s: &str, i: usize) -> usize {
    s[..i].char_indices().next_back().map(|(j, _)| j).unwrap_or(0)
}

fn next_boundary(s: &str, i: usize) -> usize {
    s[i..].chars().next().map(|c| i + c.len_utf8()).unwrap_or(s.len())
}

fn prev_word(s: &str, mut i: usize) -> usize {
    while i > 0 && s[..i].chars().next_back().is_some_and(|c| c.is_whitespace()) {
        i = prev_boundary(s, i);
    }
    while i > 0 && s[..i].chars().next_back().is_some_and(|c| !c.is_whitespace()) {
        i = prev_boundary(s, i);
    }
    i
}

fn next_word(s: &str, mut i: usize) -> usize {
    while i < s.len() && s[i..].chars().next().is_some_and(|c| c.is_whitespace()) {
        i = next_boundary(s, i);
    }
    while i < s.len() && s[i..].chars().next().is_some_and(|c| !c.is_whitespace()) {
        i = next_boundary(s, i);
    }
    i
}

fn clamp_boundary(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

impl Ui {
    /// A single-line text field editing `value`. `hint` is shown while empty.
    ///
    /// Supports click-to-place caret, drag selection, Shift+arrows, word
    /// navigation (Alt on macOS, Ctrl elsewhere), Home/End, select-all
    /// (Cmd/Ctrl+A), Backspace/Delete. `.changed` is set on edits and
    /// `.submitted` on Enter.
    pub fn text_input(&mut self, label: &str, value: &mut String, hint: &str) -> Response {
        self.text_input_ex(label, value, hint, TextInputOptions::default())
    }

    /// [`Ui::text_input`] with options: password masking, a leading icon.
    pub fn text_input_ex(
        &mut self,
        label: &str,
        value: &mut String,
        hint: &str,
        opts: TextInputOptions<'_>,
    ) -> Response {
        if !self.frame_guard() {
            return Response::default();
        }
        let (_, src) = split_label(label);
        let id = self.make_id(src);
        let th = Rc::clone(&self.theme);
        let (m, p) = (&th.metrics, &th.palette);
        let style = th.typography.body;
        if self.next_offscreen(m.control_height) && self.can_skip(id) {
            return self.skip_item(id, Vec2::new(0.0, m.control_height), true);
        }
        let r = self.allocate(Vec2::new(0.0, m.control_height), true);
        let mut resp =
            self.interact(id, r, Sense { click: true, drag: true, focusable: true, keyboard_activate: false });
        if resp.hovered {
            self.out.cursor = CursorIcon::Text;
        }
        let pad = m.control_padding_x - 2.0;
        let icon_w = if opts.icon.is_some() { 26.0 } else { 0.0 };
        let inner = Rect::from_min_max(Vec2::new(r.min.x + pad + icon_w, r.min.y), Vec2::new(r.max.x - pad, r.max.y));
        let time = self.time;
        let pw = opts.password;
        let shown = |v: &str| -> String { v.chars().map(|_| MASK).collect() };
        let to_shown = |v: &str, i: usize| if pw { to_masked(v, i) } else { i };

        let mut layout =
            if pw { self.layout_text(&shown(value), &style, None) } else { self.layout_text(value, &style, None) };
        let pointer = self.input.pointer;
        let pressed_now = self.input.pressed[0];
        let mods = self.input.modifiers;
        let edits = std::mem::take(&mut self.input.edits);
        let mut copied: Option<String> = None;
        let mut changed = false;
        let mut submitted = false;
        let mut moved = false;
        {
            let st = self.state_mut::<EditState>(id);
            st.cursor = clamp_boundary(value, st.cursor);
            st.anchor = clamp_boundary(value, st.anchor);
            if resp.pressed {
                if let Some(ptr) = pointer {
                    let raw = layout.index_at_x(ptr.x - inner.min.x + st.scroll_x);
                    let idx = if pw { from_masked(value, raw) } else { raw };
                    st.cursor = idx;
                    if pressed_now && !mods.shift {
                        st.anchor = idx;
                    }
                    moved = true;
                }
            }
            if resp.focused {
                for ev in &edits {
                    let has_sel = st.anchor != st.cursor;
                    let (a, b) = (st.anchor.min(st.cursor), st.anchor.max(st.cursor));
                    let (k, km) = match ev {
                        EditEvent::Text(t) => {
                            value.replace_range(a..b, t);
                            st.cursor = a + t.len();
                            st.anchor = st.cursor;
                            changed = true;
                            continue;
                        }
                        EditEvent::Key(k, m) => (*k, *m),
                    };
                    match k {
                        Key::Backspace => {
                            if has_sel {
                                value.replace_range(a..b, "");
                                st.cursor = a;
                            } else if st.cursor > 0 {
                                let start = if km.word() {
                                    prev_word(value, st.cursor)
                                } else {
                                    prev_boundary(value, st.cursor)
                                };
                                value.replace_range(start..st.cursor, "");
                                st.cursor = start;
                            }
                            st.anchor = st.cursor;
                            changed = true;
                        }
                        Key::Delete => {
                            if has_sel {
                                value.replace_range(a..b, "");
                                st.cursor = a;
                            } else if st.cursor < value.len() {
                                let end = if km.word() {
                                    next_word(value, st.cursor)
                                } else {
                                    next_boundary(value, st.cursor)
                                };
                                value.replace_range(st.cursor..end, "");
                            }
                            st.anchor = st.cursor;
                            changed = true;
                        }
                        Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End => {
                            let left = matches!(k, Key::ArrowLeft | Key::Home);
                            let line = matches!(k, Key::Home | Key::End) || km.command();
                            st.cursor = if line {
                                if left {
                                    0
                                } else {
                                    value.len()
                                }
                            } else if has_sel && !km.shift {
                                if left {
                                    a
                                } else {
                                    b
                                }
                            } else if km.word() {
                                if left {
                                    prev_word(value, st.cursor)
                                } else {
                                    next_word(value, st.cursor)
                                }
                            } else if left {
                                prev_boundary(value, st.cursor)
                            } else {
                                next_boundary(value, st.cursor)
                            };
                            if !km.shift {
                                st.anchor = st.cursor;
                            }
                            moved = true;
                        }
                        Key::A if km.command() => {
                            st.anchor = 0;
                            st.cursor = value.len();
                            moved = true;
                        }
                        Key::C | Key::X if km.command() && has_sel && !pw => {
                            copied = Some(value[a..b].to_owned());
                            if k == Key::X {
                                value.replace_range(a..b, "");
                                st.cursor = a;
                                st.anchor = a;
                                changed = true;
                            }
                        }
                        Key::Enter => submitted = true,
                        _ => {}
                    }
                }
            }
            if changed || moved {
                st.blink_epoch = time;
            }
        }
        // Text-editing events are consumed only by the focused input.
        if !resp.focused {
            self.input.edits = edits;
        } else {
            self.input.text.clear();
        }
        if copied.is_some() {
            self.out.clipboard = copied;
        }
        if changed {
            layout =
                if pw { self.layout_text(&shown(value), &style, None) } else { self.layout_text(value, &style, None) };
        }
        resp.changed = changed;
        resp.submitted = submitted;

        let v = self.visual(&resp);
        let foc = self.animate(id, Channel::Value, if resp.focused { 1.0 } else { 0.0 }, th.motion.hover);
        let border = p.control_border.lerp(p.text_muted, v.hover * 0.5).lerp(p.accent, foc);
        if foc > 0.0 {
            self.out.draw_list.rect(
                r.expand(3.0),
                m.radius_md + 3.0,
                rugi_core::Color::TRANSPARENT,
                Some((3.0, p.accent.with_alpha(0.22 * foc))),
            );
        }
        self.out.draw_list.rect(r, m.radius_md, p.sunken, Some((1.0, border)));
        if let Some(icon) = opts.icon {
            let ir = Rect::from_xywh(r.min.x + pad, r.center().y - 8.0, 16.0, 16.0);
            self.paint_icon(icon, ir, p.text_muted.lerp(p.accent_text, foc));
        }

        let (cursor, anchor, scroll_x, epoch) = {
            let st = self.state_mut::<EditState>(id);
            // Keep the caret visible.
            let cx = layout.caret_x(to_shown(value, st.cursor));
            let w = inner.width() - 2.0;
            if cx - st.scroll_x > w {
                st.scroll_x = cx - w;
            }
            if cx - st.scroll_x < 0.0 {
                st.scroll_x = cx;
            }
            st.scroll_x = st.scroll_x.clamp(0.0, (layout.size().x - w).max(0.0));
            (st.cursor, st.anchor, st.scroll_x, st.blink_epoch)
        };
        let ty = r.min.y + ((r.height() - layout.size().y) * 0.5).round();
        let origin = Vec2::new(inner.min.x - scroll_x, ty);
        self.out.draw_list.push_clip(inner.expand(1.0));
        if resp.focused && cursor != anchor {
            let (a, b) = (anchor.min(cursor), anchor.max(cursor));
            let sel = Rect::from_min_max(
                Vec2::new(origin.x + layout.caret_x(to_shown(value, a)), ty),
                Vec2::new(origin.x + layout.caret_x(to_shown(value, b)), ty + layout.size().y),
            );
            self.out.draw_list.rect(sel, 3.0, p.accent.with_alpha(0.32), None);
        }
        if value.is_empty() {
            let hl = self.layout_text(hint, &style, None);
            self.paint_text(&hl, origin, p.text_faint);
        } else {
            self.paint_text(&layout, origin, p.text);
        }
        if resp.focused {
            let blink = th.motion.caret_blink as f64;
            let visible = if blink > 0.0 {
                let phase = ((time - epoch) / blink).floor() as i64;
                let next = (phase + 1) as f64 * blink + epoch - time;
                self.request_repaint_after(crate::frame::repaint_delay(next.max(0.01)));
                phase % 2 == 0
            } else {
                true
            };
            if visible {
                let cx = (origin.x + layout.caret_x(to_shown(value, cursor))).round();
                let caret = Rect::from_xywh(cx - 0.75, ty + 1.0, 1.5, layout.size().y - 2.0);
                self.out.draw_list.rect(caret, 0.75, p.accent_text, None);
            }
        }
        self.out.draw_list.pop_clip();
        self.record("text_input", if hint.is_empty() { src } else { hint }, &resp);
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_motion() {
        let s = "hello  wide world";
        assert_eq!(next_word(s, 0), 5);
        assert_eq!(next_word(s, 5), 11);
        assert_eq!(prev_word(s, s.len()), 12);
        assert_eq!(prev_word(s, 7), 0);
        assert_eq!(prev_boundary("aé", 3), 1);
        assert_eq!(clamp_boundary("aé", 2), 1);
    }

    #[test]
    fn mask_mapping_round_trips() {
        let v = "pä$s";
        for (i, _) in v.char_indices().chain(std::iter::once((v.len(), ' '))) {
            assert_eq!(from_masked(v, to_masked(v, i)), i);
        }
    }
}
