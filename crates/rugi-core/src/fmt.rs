//! Allocation-free number formatting for widget value labels.

/// Appends `v` with exactly `decimals` (0..=6) fraction digits to `out`.
///
/// Rounds half away from zero using integer arithmetic instead of the exact
/// decimal expansion `format!("{:.N}")` performs. The output matches `std`
/// except possibly in the last digit of exact binary ties, and values that
/// round to zero never print a minus sign. Non-finite and very large values
/// fall back to `std::fmt`.
pub fn push_fixed(out: &mut String, v: f32, decimals: u32) {
    let d = decimals.min(6);
    let scale = 10u64.pow(d);
    let x = v as f64;
    if !x.is_finite() || x.abs() >= 1e12 {
        use std::fmt::Write as _;
        let _ = write!(out, "{:.*}", d as usize, v);
        return;
    }
    let scaled = (x.abs() * scale as f64).round() as u64;
    if x < 0.0 && scaled != 0 {
        out.push('-');
    }
    push_u64(out, scaled / scale);
    if d > 0 {
        out.push('.');
        let frac = scaled % scale;
        let mut buf = [b'0'; 6];
        let mut f = frac;
        for i in (0..d as usize).rev() {
            buf[i] = b'0' + (f % 10) as u8;
            f /= 10;
        }
        out.push_str(std::str::from_utf8(&buf[..d as usize]).unwrap_or("0"));
    }
}

fn push_u64(out: &mut String, mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out.push_str(std::str::from_utf8(&buf[i..]).unwrap_or("0"));
}

#[cfg(test)]
mod tests {
    use super::push_fixed;

    fn f(v: f32, d: u32) -> String {
        let mut s = String::new();
        push_fixed(&mut s, v, d);
        s
    }

    #[test]
    fn matches_std_formatting() {
        for &(v, d) in &[(0.0, 0), (50.0, 0), (1.0, 2), (99.5, 1), (-3.25, 2), (0.004, 2), (12345.678, 3), (72.0, 0)] {
            assert_eq!(f(v, d), format!("{:.*}", d as usize, v), "{v} {d}");
        }
        // Sweep: every slider-style value in 0..100 with 0–2 decimals.
        for i in 0..10_000 {
            let v = i as f32 * 0.0137;
            for d in 0..3 {
                let a = f(v, d);
                let b = format!("{:.*}", d as usize, v);
                if a != b {
                    // Only exact binary ties may differ, and then by one unit in the last digit.
                    let (x, y): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
                    assert!((x - y).abs() <= 1.0 / 10f64.powi(d as i32) + 1e-9, "{v} {d}: {a} vs {b}");
                }
            }
        }
        assert_eq!(f(f32::NAN, 1), "NaN");
        // Deliberate difference from std: values that round to zero never show "-0".
        assert_eq!(f(-0.0001, 2), "0.00");
    }
}
