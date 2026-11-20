//! winit integration for applications that own their event loop.

use crate::keymap;
use rugi_core::{CursorIcon, InputEvent, Key, Modifiers, PointerButton};
use rugi_ui::{FrameOutput, Ui};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::window::Window;

/// Logical pixels scrolled per wheel line.
const LINE_SCROLL_PX: f32 = 40.0;

/// Feeds winit window events to a [`Ui`] and applies its output (cursor,
/// clipboard) to the window.
///
/// [`run`](crate::run) uses it internally. Use it directly when Rugi is one
/// part of an application that already has a winit event loop and a wgpu
/// device; see `crates/rugi/examples/integration.rs`.
pub struct WinitInput {
    modifiers: Modifiers,
    cursor: CursorIcon,
    /// `None` if the system clipboard is unavailable.
    clipboard: Option<arboard::Clipboard>,
}

impl Default for WinitInput {
    fn default() -> Self {
        Self::new()
    }
}

impl WinitInput {
    /// Creates the translator and opens the system clipboard.
    pub fn new() -> Self {
        Self { modifiers: Modifiers::default(), cursor: CursorIcon::Default, clipboard: arboard::Clipboard::new().ok() }
    }

    /// Modifier keys currently held.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Translates `event` into input for the next frame of `ui`. Returns true
    /// if `ui` received input, in which case the window should be redrawn.
    /// `scale_factor` is the window's (`Window::scale_factor`).
    pub fn on_window_event(&mut self, ui: &mut Ui, event: &WindowEvent, scale_factor: f64) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(scale_factor);
                ui.push_event(InputEvent::PointerMoved { x: p.x, y: p.y });
            }
            WindowEvent::CursorLeft { .. } => ui.push_event(InputEvent::PointerLeft),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => PointerButton::Primary,
                    MouseButton::Right => PointerButton::Secondary,
                    MouseButton::Middle => PointerButton::Middle,
                    _ => return false,
                };
                ui.push_event(InputEvent::PointerButton { button, pressed: *state == ElementState::Pressed });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * LINE_SCROLL_PX, y * LINE_SCROLL_PX),
                    MouseScrollDelta::PixelDelta(p) => {
                        let l = p.to_logical::<f32>(scale_factor);
                        (l.x, l.y)
                    }
                };
                ui.push_event(InputEvent::Scroll { dx, dy });
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.modifiers =
                    Modifiers { shift: s.shift_key(), ctrl: s.control_key(), alt: s.alt_key(), logo: s.super_key() };
                ui.push_event(InputEvent::ModifiersChanged(self.modifiers));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let Some(key) = keymap::map_key(&event.logical_key) {
                    ui.push_event(InputEvent::Key { key, pressed, modifiers: self.modifiers });
                    if pressed && key == Key::V && self.modifiers.command() {
                        if let Some(text) = self.clipboard.as_mut().and_then(|c| c.get_text().ok()) {
                            ui.push_event(InputEvent::Paste(text));
                        }
                    }
                }
                // Text typed with Ctrl (not AltGr) or Cmd is a shortcut, not input.
                let m = self.modifiers;
                let shortcut = (m.ctrl && !m.alt) || m.logo;
                if pressed && !shortcut {
                    if let Some(text) = &event.text {
                        if text.chars().any(|c| !c.is_control()) {
                            ui.push_event(InputEvent::Text(text.to_string()));
                        }
                    }
                }
            }
            WindowEvent::Focused(f) => ui.push_event(InputEvent::WindowFocused(*f)),
            _ => return false,
        }
        true
    }

    /// Applies the cursor shape and clipboard writes of a finished frame.
    pub fn apply_output(&mut self, window: &Window, output: &FrameOutput) {
        if output.cursor != self.cursor {
            self.cursor = output.cursor;
            window.set_cursor(winit_cursor(output.cursor));
        }
        if let (Some(text), Some(cb)) = (&output.clipboard, self.clipboard.as_mut()) {
            // A clipboard write can fail transiently (another app holds it);
            // the text stays in the UI, so there is nothing to recover.
            let _ = cb.set_text(text.clone());
        }
    }
}

fn winit_cursor(cursor: CursorIcon) -> winit::window::CursorIcon {
    match cursor {
        CursorIcon::Default => winit::window::CursorIcon::Default,
        CursorIcon::Pointer => winit::window::CursorIcon::Pointer,
        CursorIcon::Text => winit::window::CursorIcon::Text,
        CursorIcon::Grab => winit::window::CursorIcon::Grabbing,
    }
}
