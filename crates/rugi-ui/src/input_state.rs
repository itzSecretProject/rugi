//! Aggregates queued input events into per-frame state.

use crate::layout::MAX_EXTENT;
use rugi_core::{InputEvent, Key, Modifiers, Vec2};

/// Clamps pointer coordinates and scroll deltas so hostile host input cannot
/// overflow layout arithmetic.
#[inline]
fn clamp_coord(v: f32) -> f32 {
    v.clamp(-MAX_EXTENT, MAX_EXTENT)
}

/// Text-editing events in arrival order (so "End then paste" or "type then
/// Backspace" within one frame behave correctly).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EditEvent {
    Key(Key, Modifiers),
    Text(String),
}

#[derive(Debug, Default)]
pub(crate) struct InputState {
    pub edits: Vec<EditEvent>,
    pub pointer: Option<Vec2>,
    pub pointer_delta: Vec2,
    pub down: [bool; 3],
    pub pressed: [bool; 3],
    pub released: [bool; 3],
    pub scroll: Vec2,
    pub scroll_consumed: bool,
    pub keys: Vec<(Key, Modifiers)>,
    /// Keys currently held (persists across frames).
    pub keys_down: Vec<Key>,
    pub text: String,
    pub paste: Option<String>,
    /// True if the secondary (right) button was pressed this frame.
    pub secondary_pressed: bool,
    pub modifiers: Modifiers,
    pub window_unfocused: bool,
    /// The window currently lacks focus (persists across frames; windows
    /// start focused).
    pub window_blurred: bool,
}

impl InputState {
    pub fn begin_frame(&mut self, events: &mut Vec<InputEvent>) {
        let prev = self.pointer;
        self.pressed = [false; 3];
        self.released = [false; 3];
        self.scroll = Vec2::ZERO;
        self.scroll_consumed = false;
        self.keys.clear();
        self.text.clear();
        self.paste = None;
        self.edits.clear();
        self.window_unfocused = false;
        for e in events.drain(..) {
            match e {
                // A NaN position carries no information; infinities mean "far away".
                InputEvent::PointerMoved { x, y } if !x.is_nan() && !y.is_nan() => {
                    self.pointer = Some(Vec2::new(clamp_coord(x), clamp_coord(y)));
                }
                InputEvent::PointerMoved { .. } => {}
                InputEvent::PointerLeft => self.pointer = None,
                InputEvent::PointerButton { button, pressed } => {
                    let i = button as usize;
                    if pressed {
                        self.pressed[i] = true;
                    } else {
                        self.released[i] = true;
                    }
                    self.down[i] = pressed;
                }
                InputEvent::Scroll { dx, dy } if dx.is_finite() && dy.is_finite() => {
                    self.scroll = Vec2::new(clamp_coord(self.scroll.x + dx), clamp_coord(self.scroll.y + dy));
                }
                InputEvent::Scroll { .. } => {}
                InputEvent::Key { key, pressed, modifiers } => {
                    self.modifiers = modifiers;
                    if pressed {
                        self.keys.push((key, modifiers));
                        self.edits.push(EditEvent::Key(key, modifiers));
                        if !self.keys_down.contains(&key) {
                            self.keys_down.push(key);
                        }
                    } else {
                        self.keys_down.retain(|k| *k != key);
                    }
                }
                InputEvent::ModifiersChanged(m) => self.modifiers = m,
                InputEvent::Text(t) => {
                    let clean: String = t.chars().filter(|c| !c.is_control()).collect();
                    self.text.push_str(&clean);
                    if !clean.is_empty() {
                        self.edits.push(EditEvent::Text(clean));
                    }
                }
                InputEvent::Paste(t) => {
                    // Single-line inputs: line breaks become spaces.
                    let clean: String = t
                        .chars()
                        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
                        .filter(|c| !c.is_control())
                        .collect();
                    if !clean.is_empty() {
                        self.edits.push(EditEvent::Text(clean.clone()));
                    }
                    self.paste = Some(clean);
                }
                InputEvent::WindowFocused(f) => {
                    self.window_blurred = !f;
                    if !f {
                        self.down = [false; 3];
                        self.keys_down.clear();
                        self.modifiers = Modifiers::default();
                        self.window_unfocused = true;
                    }
                }
            }
        }
        self.secondary_pressed = self.pressed[1];
        self.pointer_delta = match (prev, self.pointer) {
            (Some(a), Some(b)) => b - a,
            _ => Vec2::ZERO,
        };
    }

    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys.iter().any(|(k, _)| *k == key)
    }

    pub fn key_pressed_mods(&self, key: Key) -> Option<Modifiers> {
        self.keys.iter().find(|(k, _)| *k == key).map(|(_, m)| *m)
    }
}
