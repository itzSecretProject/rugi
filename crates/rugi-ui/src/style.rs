//! Theme access, the style stack and font registration.

use crate::{FontError, FontFamily, Theme, Ui, UiError};
use rugi_core::Repaint;
use std::rc::Rc;

impl Ui {
    /// Current theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Replaces the theme.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = Rc::new(theme);
        self.repaint = Repaint::Immediately;
    }

    /// Mutable theme access.
    pub fn theme_mut(&mut self) -> &mut Theme {
        self.repaint = Repaint::Immediately;
        Rc::make_mut(&mut self.theme)
    }

    /// Temporarily modifies the theme for the widgets that follow, until the
    /// matching [`Ui::pop_style`]. Covers every design token (colors, metrics,
    /// typography, motion).
    pub fn push_style(&mut self, modify: impl FnOnce(&mut Theme)) {
        self.style_stack.push(Rc::clone(&self.theme));
        modify(Rc::make_mut(&mut self.theme));
    }

    /// Restores the theme saved by the last [`Ui::push_style`].
    pub fn pop_style(&mut self) {
        match self.style_stack.pop() {
            Some(t) => self.theme = t,
            None => self.report(UiError::NothingToEnd),
        }
    }

    /// Runs `body` with a temporarily modified theme.
    pub fn with_style<R>(&mut self, modify: impl FnOnce(&mut Theme), body: impl FnOnce(&mut Ui) -> R) -> R {
        self.push_style(modify);
        let r = body(self);
        self.pop_style();
        r
    }

    /// Uses `family` for all text until [`Ui::pop_style`].
    pub fn push_font(&mut self, family: FontFamily) {
        self.push_style(|t| t.set_font_family(family));
    }

    /// Registers a font from TTF/OTF bytes. See [`rugi_text::TextSystem::add_font`].
    pub fn add_font(&mut self, data: Vec<u8>) -> Result<FontFamily, FontError> {
        self.repaint = Repaint::Immediately;
        self.text.add_font(data)
    }

    /// Registers a font file.
    pub fn add_font_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<FontFamily, FontError> {
        self.repaint = Repaint::Immediately;
        self.text.add_font_file(path)
    }

    /// Handle for a family by name (system fonts, or fonts registered earlier).
    pub fn font_family(&mut self, name: &str) -> FontFamily {
        self.text.family_handle(name)
    }

    /// The bundled icon font (Lucide), if enabled.
    pub fn icon_font(&self) -> Option<FontFamily> {
        self.text.icon_family()
    }
}
