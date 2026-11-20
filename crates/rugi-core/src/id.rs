//! Widget identity.
//!
//! Every interactive widget is identified by an [`Id`]: a 64-bit FNV-1a hash
//! of its label (or explicit id string) combined with the id of its parent
//! scope. Ids are stable across frames as long as the hierarchy and labels are
//! stable, which is what lets an immediate-mode API keep retained state
//! (animations, scroll offsets, text cursors) per widget.

/// A stable widget identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Id(pub u64);

const SEED: u64 = 0xcbf2_9ce4_8422_2325;
const K: u64 = 0x517c_c1b7_2722_0a95;
/// 2^64 / φ; decorrelates child ids from their parent.
const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;

/// murmur3 64-bit finalizer: every input bit affects every output bit.
#[inline]
fn fmix64(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// Fast non-cryptographic hash of `bytes` continuing from `h` (8 bytes per
/// step, xorshift-multiply mixing, murmur3 finalizer). Deterministic within a
/// build; not a stable on-disk format.
#[inline]
pub fn hash_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    let mut chunks = bytes.chunks_exact(8);
    for c in &mut chunks {
        let w = u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
        h = (h ^ w).wrapping_mul(K);
        h ^= h >> 29;
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        let mut buf = [0u8; 8];
        buf[..rest.len()].copy_from_slice(rest);
        h = (h ^ u64::from_le_bytes(buf)).wrapping_mul(K);
        h ^= h >> 29;
    }
    fmix64(h ^ (bytes.len() as u64).wrapping_mul(GOLDEN))
}

impl Id {
    /// The root id.
    pub const ROOT: Id = Id(SEED);

    /// Derives a child id from bytes.
    #[inline]
    pub fn with_bytes(self, bytes: &[u8]) -> Id {
        Id(hash_bytes(self.0 ^ GOLDEN, bytes))
    }

    /// Derives a child id from a string.
    #[inline]
    pub fn with(self, s: &str) -> Id {
        self.with_bytes(s.as_bytes())
    }

    /// Derives a child id from an integer (e.g. a list index).
    #[inline]
    pub fn with_u64(self, v: u64) -> Id {
        Id(fmix64((self.0 ^ GOLDEN).rotate_left(5) ^ v.wrapping_mul(K)))
    }
}

/// A `Hasher` for keys that are already well-mixed 64-bit hashes ([`Id`],
/// text cache keys): `write_u64` values pass straight through instead of
/// being re-hashed with SipHash.
#[derive(Default, Clone, Copy)]
pub struct IdHasher(u64);

impl std::hash::Hasher for IdHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.0 = hash_bytes(self.0, bytes);
    }
    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.0 ^= v;
    }
}

/// Hash map keyed by pre-hashed 64-bit values.
pub type IdMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<IdHasher>>;

/// Splits an ImGui-style label: `"Visible##hidden-id"` displays `Visible` and
/// hashes the full string; `"###id"` hashes only the part after `###`.
/// Returns `(display_text, id_source)`.
#[inline]
pub fn split_label(label: &str) -> (&str, &str) {
    let b = label.as_bytes();
    let mut i = 0;
    let first = loop {
        match b[i..].iter().position(|&c| c == b'#') {
            None => return (label, label),
            Some(off) => {
                let p = i + off;
                if b.get(p + 1) == Some(&b'#') {
                    break p;
                }
                i = p + 1;
            }
        }
    };
    // Display text ends at the first "##"; an id override is the first "###".
    let mut j = first;
    while j + 2 < b.len() {
        if b[j] == b'#' && b[j + 1] == b'#' && b[j + 2] == b'#' {
            return (&label[..first], &label[j + 3..]);
        }
        j += 1;
    }
    (&label[..first], label)
}

/// A general fast hasher (FxHash-style mixing per write, murmur3 finalizer)
/// for hot maps with structured keys, e.g. glyph cache keys.
#[derive(Default, Clone, Copy)]
pub struct FastHasher(u64);

impl std::hash::Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        fmix64(self.0)
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.0 = hash_bytes(self.0, bytes);
    }
    #[inline]
    fn write_u8(&mut self, v: u8) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_u16(&mut self, v: u16) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_u32(&mut self, v: u32) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(K);
    }
    #[inline]
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
}

/// Hash map using [`FastHasher`].
pub type FastMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<FastHasher>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_scoped() {
        let a = Id::ROOT.with("panel").with("OK");
        let b = Id::ROOT.with("panel").with("OK");
        let c = Id::ROOT.with("other").with("OK");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(Id::ROOT.with_u64(1), Id::ROOT.with_u64(2));
    }

    #[test]
    fn hash_separates_similar_inputs() {
        let ids: std::collections::HashSet<u64> =
            (0..200_000u64).map(|i| Id::ROOT.with(&format!("Row {i}")).0).collect();
        assert_eq!(ids.len(), 200_000);
        assert_ne!(hash_bytes(0, b"ab"), hash_bytes(0, b"ab\0"));
        let mut m: IdMap<Id, u32> = IdMap::default();
        m.insert(Id(7), 1);
        assert_eq!(m.get(&Id(7)), Some(&1));
    }

    #[test]
    fn label_splitting() {
        assert_eq!(split_label("Save"), ("Save", "Save"));
        assert_eq!(split_label("Save##a"), ("Save", "Save##a"));
        assert_eq!(split_label("Save###x"), ("Save", "x"));
        assert_eq!(split_label("a#b"), ("a#b", "a#b"));
        assert_eq!(split_label("##"), ("", "##"));
        assert_eq!(split_label("a##b###c"), ("a", "c"));
        assert_eq!(split_label("ñ##ü"), ("ñ", "ñ##ü"));
        assert_eq!(split_label("x#"), ("x#", "x#"));
    }
}
