//! Frame-interval statistics for the display info and `RUGI_PACING_LOG`.

use std::time::Instant;

/// Number of presented-frame intervals kept.
const WINDOW: usize = 240;

/// Ring buffer of presented-frame intervals during continuous animation.
pub(crate) struct Pacing {
    intervals: Vec<f32>,
    next: usize,
    sorted: Vec<f32>,
    last_present: Option<Instant>,
    /// Print surface and pacing diagnostics to stderr.
    pub log: bool,
    pub last_log: Instant,
}

impl Pacing {
    pub fn new() -> Self {
        Self {
            intervals: Vec::with_capacity(WINDOW),
            next: 0,
            sorted: Vec::with_capacity(WINDOW),
            last_present: None,
            log: std::env::var_os("RUGI_PACING_LOG").is_some(),
            last_log: Instant::now(),
        }
    }

    /// Records a present at `now`; the interval counts only if `continuous`.
    pub fn record(&mut self, now: Instant, continuous: bool) {
        if let (Some(prev), true) = (self.last_present, continuous) {
            let ms = now.duration_since(prev).as_secs_f32() * 1000.0;
            if self.intervals.len() < WINDOW {
                self.intervals.push(ms);
            } else {
                self.intervals[self.next] = ms;
                self.next = (self.next + 1) % WINDOW;
            }
        }
        self.last_present = Some(now);
    }

    pub fn samples(&self) -> usize {
        self.intervals.len()
    }

    /// Mean and 99th-percentile interval in milliseconds.
    pub fn stats(&mut self) -> (f32, f32) {
        if self.intervals.is_empty() {
            return (0.0, 0.0);
        }
        self.sorted.clone_from(&self.intervals);
        self.sorted.sort_by(f32::total_cmp);
        let v = &self.sorted;
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let p99 = v[((v.len() - 1) as f32 * 0.99).round() as usize];
        (mean, p99)
    }
}

/// Frames per second for a mean frame interval, 0 if unknown.
pub(crate) fn fps(mean_interval_ms: f32) -> f32 {
    if mean_interval_ms > 0.0 {
        1000.0 / mean_interval_ms
    } else {
        0.0
    }
}
