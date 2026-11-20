//! Icon fonts for Rugi.
//!
//! Bundles the [Lucide](https://lucide.dev) icon font (ISC license;
//! Feather-derived icons MIT) and a constant per icon in [`lucide`]. The text
//! system registers the font at start-up, so icons can be embedded in any
//! string and inherit its color and size. Other icon fonts can be added at
//! runtime with `Ui::add_font` and used the same way.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod lucide;

/// The bundled Lucide font file.
pub const LUCIDE_TTF: &[u8] = include_bytes!("../assets/lucide.ttf");

/// Family name of the bundled Lucide font.
pub const LUCIDE_FAMILY: &str = "lucide";

#[cfg(test)]
mod tests {
    use super::lucide;

    #[test]
    fn lookup_by_name() {
        assert_eq!(lucide::by_name("save"), Some(lucide::SAVE));
        assert_eq!(lucide::by_name("does-not-exist"), None);
        assert!(lucide::ALL.windows(2).all(|w| w[0].0 < w[1].0), "sorted for binary search");
        assert_eq!(lucide::SAVE.chars().count(), 1);
    }
}
