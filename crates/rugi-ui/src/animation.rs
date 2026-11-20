//! Per-widget animation channels, eased towards their targets each frame.

use crate::Ui;
use rugi_core::{anim, Id, Repaint};

/// Animated values of one widget, indexed by [`Channel`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct WidgetAnim {
    pub v: [f32; 4],
    pub last_frame: u64,
}

/// Animation channels (indices into [`WidgetAnim::v`]). Indices 0–2 hold
/// hover, press and focus, which [`Ui::animate3`] updates together.
#[derive(Clone, Copy)]
pub(crate) enum Channel {
    Hover = 0,
    Value = 3,
}

impl Ui {
    pub(crate) fn animate(&mut self, id: Id, ch: Channel, target: f32, time_constant: f32) -> f32 {
        let frame = self.frame;
        let dt = self.dt;
        let a = self.anims.entry(id).or_insert(WidgetAnim { v: [0.0, 0.0, 0.0, target], last_frame: frame });
        a.last_frame = frame;
        let cur = a.v[ch as usize];
        if cur == target {
            // Settled channels skip the `exp` call.
            return cur;
        }
        let next = anim::approach(cur, target, dt, time_constant);
        a.v[ch as usize] = next;
        if next != target {
            self.repaint = Repaint::Immediately;
        }
        next
    }

    /// Animates the hover/press/focus channels of `id` with one map lookup.
    pub(crate) fn animate3(&mut self, id: Id, targets: [f32; 3], time_constants: [f32; 3]) -> [f32; 3] {
        let frame = self.frame;
        let dt = self.dt;
        let a = self.anims.entry(id).or_insert(WidgetAnim { v: [0.0; 4], last_frame: frame });
        a.last_frame = frame;
        let mut out = [0.0; 3];
        let mut moving = false;
        for i in 0..3 {
            let cur = a.v[i];
            let next = if cur == targets[i] { cur } else { anim::approach(cur, targets[i], dt, time_constants[i]) };
            a.v[i] = next;
            moving |= next != targets[i];
            out[i] = next;
        }
        if moving {
            self.repaint = Repaint::Immediately;
        }
        out
    }
}
