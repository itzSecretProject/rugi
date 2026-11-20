//! Value types shared by the slider and spin box widgets.

use super::ordered_range;

/// A number a slider or spin box can edit.
pub(crate) trait Numeric: Copy + PartialEq {
    /// `min..=max` in ascending order, with invalid bounds repaired.
    fn range(min: Self, max: Self) -> (Self, Self);
    /// Replaces a value that cannot be shown (NaN, infinity) with `lo`.
    fn sanitize(&mut self, lo: Self);
    /// Position of `self` within `lo..=hi` as 0..=1.
    fn fraction(self, lo: Self, hi: Self) -> f32;
    /// The value at position `t` (0..=1) of `lo..=hi`.
    fn at_fraction(t: f32, lo: Self, hi: Self) -> Self;
    /// Arrow-key step of a slider over `lo..=hi`; `coarse` with Shift.
    fn slider_step(lo: Self, hi: Self, coarse: bool) -> Self;
    /// `self + n * step`, without overflow.
    fn add_steps(self, step: Self, n: i32) -> Self;
    /// Applies a horizontal drag of `units` steps; whole steps for integers,
    /// keeping the remainder in `carry`.
    fn drag(self, step: Self, units: f32, carry: &mut f32) -> Self;
    fn clamp_to(self, lo: Self, hi: Self) -> Self;
    /// Fraction digits a slider over `lo..=hi` shows.
    fn slider_decimals(lo: Self, hi: Self) -> u32;
    /// Fraction digits a spin box with `step` shows.
    fn spin_decimals(step: Self) -> u32;
    /// A usable spin box step: positive and finite.
    fn valid_step(step: Self) -> Self;
    /// Appends the value with `decimals` fraction digits.
    fn write(self, out: &mut String, decimals: u32);
}

impl Numeric for f32 {
    #[inline]
    fn range(min: f32, max: f32) -> (f32, f32) {
        ordered_range(min, max)
    }
    #[inline]
    fn sanitize(&mut self, lo: f32) {
        if !self.is_finite() {
            *self = lo;
        }
    }
    #[inline]
    fn fraction(self, lo: f32, hi: f32) -> f32 {
        ((self - lo) / (hi - lo).max(f32::EPSILON)).clamp(0.0, 1.0)
    }
    #[inline]
    fn at_fraction(t: f32, lo: f32, hi: f32) -> f32 {
        lo + t * (hi - lo).max(f32::EPSILON)
    }
    #[inline]
    fn slider_step(lo: f32, hi: f32, coarse: bool) -> f32 {
        let span = (hi - lo).max(f32::EPSILON);
        if coarse {
            span / 10.0
        } else {
            span / 100.0
        }
    }
    #[inline]
    fn add_steps(self, step: f32, n: i32) -> f32 {
        self + step * n as f32
    }
    #[inline]
    fn drag(self, step: f32, units: f32, _carry: &mut f32) -> f32 {
        self + units * step
    }
    #[inline]
    fn clamp_to(self, lo: f32, hi: f32) -> f32 {
        self.clamp(lo, hi)
    }
    #[inline]
    fn slider_decimals(lo: f32, hi: f32) -> u32 {
        let span = (hi - lo).max(f32::EPSILON);
        if span <= 2.0 {
            2
        } else if span <= 20.0 {
            1
        } else {
            0
        }
    }
    #[inline]
    fn spin_decimals(step: f32) -> u32 {
        if step >= 1.0 {
            0
        } else if step >= 0.1 {
            1
        } else {
            2
        }
    }
    #[inline]
    fn valid_step(step: f32) -> f32 {
        if step.is_finite() && step > 0.0 {
            step
        } else {
            1.0
        }
    }
    #[inline]
    fn write(self, out: &mut String, decimals: u32) {
        rugi_core::fmt::push_fixed(out, self, decimals);
    }
}

impl Numeric for i32 {
    #[inline]
    fn range(min: i32, max: i32) -> (i32, i32) {
        (min.min(max), min.max(max))
    }
    #[inline]
    fn sanitize(&mut self, _lo: i32) {}
    #[inline]
    fn fraction(self, lo: i32, hi: i32) -> f32 {
        let span = (hi as i64 - lo as i64).max(1) as f64;
        ((self as i64 - lo as i64) as f64 / span).clamp(0.0, 1.0) as f32
    }
    #[inline]
    fn at_fraction(t: f32, lo: i32, hi: i32) -> i32 {
        let span = (hi as i64 - lo as i64) as f64;
        (lo as i64 + (t.clamp(0.0, 1.0) as f64 * span).round() as i64).clamp(lo as i64, hi as i64) as i32
    }
    #[inline]
    fn slider_step(lo: i32, hi: i32, coarse: bool) -> i32 {
        if coarse {
            ((hi as i64 - lo as i64) / 10).clamp(1, i32::MAX as i64) as i32
        } else {
            1
        }
    }
    #[inline]
    fn add_steps(self, step: i32, n: i32) -> i32 {
        (self as i64 + step as i64 * n as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
    #[inline]
    fn drag(self, step: i32, units: f32, carry: &mut f32) -> i32 {
        *carry += units;
        let whole = carry.trunc();
        *carry -= whole;
        self.add_steps(step, whole.clamp(i32::MIN as f32, i32::MAX as f32) as i32)
    }
    #[inline]
    fn clamp_to(self, lo: i32, hi: i32) -> i32 {
        self.clamp(lo, hi)
    }
    #[inline]
    fn slider_decimals(_lo: i32, _hi: i32) -> u32 {
        0
    }
    #[inline]
    fn spin_decimals(_step: i32) -> u32 {
        0
    }
    #[inline]
    fn valid_step(step: i32) -> i32 {
        step.max(1)
    }
    #[inline]
    fn write(self, out: &mut String, _decimals: u32) {
        use std::fmt::Write as _;
        let _ = write!(out, "{self}");
    }
}

#[cfg(test)]
mod tests {
    use super::Numeric;

    #[test]
    fn integer_positions_round_to_whole_values() {
        assert_eq!(i32::at_fraction(0.0, -5, 5), -5);
        assert_eq!(i32::at_fraction(0.54, -5, 5), 0);
        assert_eq!(i32::at_fraction(1.0, i32::MIN, i32::MAX), i32::MAX);
        assert_eq!(5.fraction(0, 10), 0.5);
        assert_eq!(0.fraction(3, 3), 0.0);
    }

    #[test]
    fn integer_steps_saturate() {
        assert_eq!(i32::MAX.add_steps(1, 1), i32::MAX);
        assert_eq!(i32::slider_step(0, 100, true), 10);
        assert_eq!(i32::slider_step(0, 5, true), 1);
        let mut carry = 0.0;
        let mut v = 0;
        for _ in 0..7 {
            v = v.drag(2, 0.5, &mut carry);
        }
        assert_eq!(v, 6);
        let mut s = String::new();
        (-42).write(&mut s, 3);
        assert_eq!(s, "-42");
    }
}
