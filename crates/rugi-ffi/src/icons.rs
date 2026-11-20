//! Icon arguments: built-in vector icons and font glyphs.

use std::ffi::c_char;

use rugi_ui::{Icon, IconRef};

use crate::args::{str_arg, write_c_str};
use crate::error::{guard_value, FfiError};

/// Icon argument: a built-in RUGI_ICON_* value or a font glyph
/// (RUGI_ICON_GLYPH_FLAG | codepoint).
pub type RugiIcon = u32;
/// No icon.
pub const RUGI_ICON_NONE: RugiIcon = 0xFFFF_FFFF;
/// Grid.
pub const RUGI_ICON_DASHBOARD: RugiIcon = 0;
/// Bar chart.
pub const RUGI_ICON_CHART: RugiIcon = 1;
/// Sliders.
pub const RUGI_ICON_SLIDERS: RugiIcon = 2;
/// Pulse line.
pub const RUGI_ICON_ACTIVITY: RugiIcon = 3;
/// Layers.
pub const RUGI_ICON_LAYERS: RugiIcon = 4;
/// Magnifier.
pub const RUGI_ICON_SEARCH: RugiIcon = 5;
/// Plus.
pub const RUGI_ICON_PLUS: RugiIcon = 6;
/// Check mark.
pub const RUGI_ICON_CHECK: RugiIcon = 7;
/// Folder.
pub const RUGI_ICON_FOLDER: RugiIcon = 8;
/// Flag marking a RugiIcon as a font glyph: `RUGI_ICON_GLYPH_FLAG | codepoint`
/// (see rugi_icons_lucide.h for ready-made RUGI_ICON_LUCIDE_* values).
pub const RUGI_ICON_GLYPH_FLAG: RugiIcon = 0x8000_0000;

/// A decoded `RugiIcon`: none, a built-in vector icon, or a font glyph whose
/// UTF-8 bytes are stored inline.
pub(crate) struct IconArg {
    vector: Option<Icon>,
    glyph: [u8; 4],
    glyph_len: usize,
}

impl IconArg {
    pub(crate) fn get(&self) -> Option<IconRef<'_>> {
        match self.vector {
            Some(v) => Some(IconRef::Vector(v)),
            None if self.glyph_len > 0 => std::str::from_utf8(&self.glyph[..self.glyph_len]).ok().map(IconRef::Glyph),
            None => None,
        }
    }
}

pub(crate) fn icon_from(icon: RugiIcon) -> Result<IconArg, FfiError> {
    let mut arg = IconArg { vector: None, glyph: [0; 4], glyph_len: 0 };
    if icon == RUGI_ICON_NONE {
        return Ok(arg);
    }
    if icon & RUGI_ICON_GLYPH_FLAG != 0 {
        let c = char::from_u32(icon & !RUGI_ICON_GLYPH_FLAG)
            .ok_or_else(|| FfiError::invalid(format!("invalid glyph codepoint in icon {icon:#x}")))?;
        arg.glyph_len = c.encode_utf8(&mut arg.glyph).len();
        return Ok(arg);
    }
    arg.vector = Some(Icon::from_u32(icon).ok_or_else(|| FfiError::invalid(format!("unknown icon {icon}")))?);
    Ok(arg)
}

/// Writes the UTF-8 encoding of the Lucide icon `name` (e.g. "save") into
/// `out` (at least 5 bytes, NUL-terminated). Returns the byte length, or 0 if
/// the name is unknown.
#[no_mangle]
pub unsafe extern "C" fn rugi_lucide_icon(name: *const c_char, len: usize, out: *mut c_char, out_size: usize) -> usize {
    guard_value(0, || {
        // SAFETY: contract of `str_arg`.
        let name = unsafe { str_arg(name, len, "name") }?;
        let Some(glyph) = rugi_ui::lucide::by_name(name) else { return Ok(0) };
        if out.is_null() || out_size <= glyph.len() {
            return Err(FfiError::invalid("out buffer too small (need 5 bytes)"));
        }
        // SAFETY: `out` has `out_size` > glyph.len() writable bytes.
        unsafe { write_c_str(glyph, out, out_size) };
        Ok(glyph.len())
    })
}
