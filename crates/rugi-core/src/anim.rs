//! Animation helpers.

/// Frame-rate independent exponential approach of `current` towards `target`.
///
/// `time_constant` is the time (seconds) to cover ~63% of the distance. Values
/// within `1e-3` of the target snap to it so animations settle and the
/// application can return to idle (no repaint).
#[inline]
pub fn approach(current: f32, target: f32, dt: f32, time_constant: f32) -> f32 {
    if time_constant <= 0.0 || dt <= 0.0 {
        return if dt <= 0.0 { current } else { target };
    }
    let k = 1.0 - (-dt / time_constant).exp();
    let v = current + (target - current) * k;
    if (v - target).abs() < 1e-3 {
        target
    } else {
        v
    }
}

/// A critically damped spring (no overshoot). Unlike a fixed tween it keeps
/// its velocity when the target changes mid-flight, so motion stays continuous.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    /// Current value.
    pub pos: f32,
    /// Current velocity (units per second).
    pub vel: f32,
}

impl Spring {
    /// A spring at rest at `pos`.
    pub const fn at(pos: f32) -> Self {
        Spring { pos, vel: 0.0 }
    }

    /// Advances the spring towards `target`. `omega` is the natural frequency
    /// (rad/s; ~20–30 feels snappy for UI). Exact solution, stable for any `dt`.
    /// Returns true while still moving.
    pub fn step(&mut self, target: f32, dt: f32, omega: f32) -> bool {
        if dt <= 0.0 {
            return self.pos != target;
        }
        let x = self.pos - target;
        let e = (-omega * dt).exp();
        let t = (self.vel + omega * x) * dt;
        let nx = (x + t) * e;
        self.vel = (self.vel - omega * t) * e;
        self.pos = target + nx;
        if nx.abs() < 0.02 && self.vel.abs() < 0.5 {
            self.pos = target;
            self.vel = 0.0;
            return false;
        }
        true
    }
}

/// Cubic ease-out on 0..=1.
#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = 1.0 - t.clamp(0.0, 1.0);
    1.0 - t * t * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approach_settles() {
        let mut v = 0.0;
        for _ in 0..200 {
            v = approach(v, 1.0, 1.0 / 60.0, 0.08);
        }
        assert_eq!(v, 1.0);
        assert_eq!(approach(0.3, 1.0, 0.0, 0.1), 0.3);
    }

    #[test]
    fn spring_settles_without_overshoot_at_any_refresh_rate() {
        for hz in [30.0f32, 60.0, 120.0, 144.0, 240.0] {
            let mut s = Spring::at(0.0);
            let mut max = 0.0f32;
            let mut frames = 0;
            while s.step(100.0, 1.0 / hz, 25.0) {
                max = max.max(s.pos);
                frames += 1;
                assert!(frames < 10_000);
            }
            assert!(max <= 100.0 + 1e-3, "overshoot at {hz} Hz: {max}");
            assert_eq!(s.pos, 100.0);
            // Settling time is the same in seconds regardless of refresh rate.
            let secs = frames as f32 / hz;
            assert!(secs > 0.2 && secs < 0.6, "{hz} Hz settled in {secs}s");
        }
    }

    #[test]
    fn approach_is_framerate_independent() {
        let mut a = 0.0;
        for _ in 0..6 {
            a = approach(a, 1.0, 1.0 / 60.0, 0.1);
        }
        let b = approach(0.0, 1.0, 0.1, 0.1);
        assert!((a - b).abs() < 1e-4);
    }
}
