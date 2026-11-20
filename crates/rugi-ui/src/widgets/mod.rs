//! The built-in widget library.
//!
//! Widgets are `impl Ui` methods. Custom widgets in other crates follow the
//! same pattern using the public building blocks: [`Ui::allocate`],
//! [`Ui::make_id`], [`Ui::painter`], [`Ui::layout_text`], [`Ui::paint_text`]
//! and [`Ui::state_mut`].
//!
//! [`Ui::allocate`]: crate::Ui::allocate
//! [`Ui::make_id`]: crate::Ui::make_id
//! [`Ui::painter`]: crate::Ui::painter
//! [`Ui::layout_text`]: crate::Ui::layout_text
//! [`Ui::paint_text`]: crate::Ui::paint_text
//! [`Ui::state_mut`]: crate::Ui::state_mut

mod advanced;
mod basic;
mod controls;
mod custom;
mod extra;
mod numeric;
mod text_input;

pub use basic::TextRole;
pub use controls::ButtonKind;
pub use custom::{Bind, BindMode, KeyBind};
pub use text_input::TextInputOptions;

/// Edge length of icons inside controls (buttons, nav items, dropdowns).
const ICON_SIZE: f32 = 16.0;
/// Gap between a check mark, switch or radio dot and its label.
const LABEL_GAP: f32 = 10.0;
/// Spring rate of selection highlights that move between items.
const GLIDE_OMEGA: f32 = 26.0;
/// Largest magnitude of a value-widget bound, so `max - min` stays finite.
const MAX_BOUND: f32 = 1.0e18;

/// `min..=max` in ascending order, finite, with a NaN bound replaced by the
/// other one (or zero).
fn ordered_range(min: f32, max: f32) -> (f32, f32) {
    let (a, b) = match (min.is_nan(), max.is_nan()) {
        (false, false) => (min, max),
        (true, false) => (max, max),
        (false, true) => (min, min),
        (true, true) => (0.0, 0.0),
    };
    let (a, b) = (a.clamp(-MAX_BOUND, MAX_BOUND), b.clamp(-MAX_BOUND, MAX_BOUND));
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
