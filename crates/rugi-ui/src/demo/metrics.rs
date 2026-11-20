//! Frame timing and the statistics of the last frame.

use super::two_columns;
use crate::{FrameStats, Ui};

pub(super) const SOURCE: &str = include_str!("metrics.rs");

/// Number of frame-time samples kept.
const HISTORY_LEN: usize = 240;

pub(super) struct State {
    /// CPU build time of recent frames in milliseconds, oldest first.
    history: Vec<f32>,
}

impl Default for State {
    fn default() -> Self {
        Self { history: Vec::with_capacity(HISTORY_LEN) }
    }
}

impl State {
    /// Adds the build time of frame `index`, described by `stats`. The
    /// first frame, which loads fonts and shapes every string, is left out.
    pub(super) fn record(&mut self, index: u64, stats: FrameStats) {
        if index <= 1 {
            return;
        }
        if self.history.len() == HISTORY_LEN {
            self.history.remove(0);
        }
        self.history.push(stats.build_us as f32 / 1000.0);
    }
}

pub(super) fn page(ui: &mut Ui, app: &mut State) {
    two_columns(ui, app, left, right);
}

fn left(ui: &mut Ui, app: &mut State) {
    demo_section!(
        ui,
        "Frame build time (ms)",
        "CPU time from begin_frame to end_frame, without the first frame (font loading). An idle window draws no frames and adds no samples.",
        |ui| {
            let ms = &app.history;
            let max = ms.iter().copied().fold(0.0, f32::max);
            ui.plot_lines("##build", ms, None, 160.0);
            let mut sorted = ms.clone();
            sorted.sort_by(f32::total_cmp);
            let at = |q: f32| {
                let i = ((sorted.len() as f32 - 1.0) * q) as usize;
                sorted.get(i).copied().unwrap_or(0.0)
            };
            let (median, p99) = (at(0.5), at(0.99));
            ui.label_fmt(format_args!("{} frames", ms.len()));
            ui.label_fmt(format_args!("median {median:.3} ms, p99 {p99:.3} ms, max {max:.3} ms"));
        }
    );
}

fn right(ui: &mut Ui, _app: &mut State) {
    demo_section!(ui, "Last frame", "From ui.last_frame_stats().", |ui| {
        let s = ui.last_frame_stats();
        let rows = [
            ("Interactive widgets", s.widgets),
            ("GPU instances", s.instances),
            ("Draw batches", s.batches),
            ("Primitives culled", s.culled),
            ("Text layouts shaped", s.text_shaped),
            ("Glyphs rasterized", s.glyphs_rasterized),
        ];
        for (name, value) in rows {
            ui.labeled(name, |ui| ui.label_fmt(format_args!("{value}")));
        }
    });
    let display = ui.display_info();
    if display.refresh_hz.is_some() || display.fps > 0.0 {
        demo_section!(ui, "Display", "Measured by the window runner while frames are animating.", |ui| {
            let d = ui.display_info();
            if let Some(hz) = d.refresh_hz {
                ui.labeled("Refresh rate", |ui| ui.label_fmt(format_args!("{hz:.0} Hz")));
            }
            if d.fps > 0.0 {
                ui.labeled("Frame rate", |ui| ui.label_fmt(format_args!("{:.1} fps", d.fps)));
                let (mean, p99) = (d.frame_interval_ms, d.frame_interval_p99_ms);
                ui.labeled("Interval", |ui| ui.label_fmt(format_args!("mean {mean:.2} ms, p99 {p99:.2} ms")));
            }
        });
    }
}
