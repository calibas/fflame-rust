//! The viewport's effect clock (docs/archive/projects/multipass-effects.md, T1-T2):
//! where the shown picture is in the animation, for the effects that read
//! the time and a temporal effect's history.
//!
//! The viewport draws many frames of one moment -- converging, edited
//! while paused -- and those re-read the history rather than add to it.
//! Playback moves the moment on a little each frame: that carries the
//! history on, `dt` the real interval. A seek, a loop's wrap, a scrub
//! while paused or a Reset starts a new history.

use crate::renderer::effect_chain::EffectClock;

/// A jump in time larger than this, while playing, is a seek or a loop's
/// wrap rather than playback's next frame.
const LARGEST_STEP: f64 = 0.5;

#[derive(Default)]
pub(crate) struct EffectTimeline {
    epoch: u64,
    frame: u64,
    /// The moment the last viewport frame showed; `None` before one, and
    /// after a Reset.
    last: Option<f64>,
}

impl EffectTimeline {
    /// The clock for the viewport frame showing `time`, `playing` or not.
    pub(crate) fn tick(&mut self, playing: bool, time: f64) -> EffectClock {
        let mut dt = 0.0;
        match self.last {
            Some(t) if t == time => {}
            Some(t) if playing && (time - t).abs() <= LARGEST_STEP => {
                self.frame += 1;
                dt = (time - t).abs();
            }
            _ => {
                self.epoch += 1;
                self.frame = 0;
            }
        }
        self.last = Some(time);
        EffectClock::frame(self.epoch, self.frame, time as f32, dt as f32)
    }

    /// Start a new history at the next frame (the Effects panel's Reset).
    pub(crate) fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_carries_the_history_and_anything_else_starts_one() {
        let mut t = EffectTimeline::default();
        let first = t.tick(false, 0.0);
        assert_eq!((first.frame, first.dt), (0, 0.0));
        // Paused: the same frame, however many times it is drawn.
        assert_eq!(t.tick(false, 0.0), first);
        // Playing: each moment the next frame, `dt` the step.
        let a = t.tick(true, 0.016);
        assert_eq!((a.epoch, a.frame), (first.epoch, 1));
        assert!((a.dt - 0.016).abs() < 1e-6);
        let b = t.tick(true, 0.032);
        assert_eq!((b.epoch, b.frame), (first.epoch, 2));
        // Playing backwards (ping-pong) carries on too.
        let c = t.tick(true, 0.020);
        assert_eq!((c.epoch, c.frame), (first.epoch, 3));
        // Held while paused.
        assert_eq!(t.tick(false, 0.020).frame, 3);
        // A loop's wrap, a seek, a scrub: a new history.
        let wrap = t.tick(true, 4.0);
        assert_eq!((wrap.epoch, wrap.frame), (first.epoch + 1, 0));
        let scrub = t.tick(false, 3.9);
        assert_eq!((scrub.epoch, scrub.frame), (first.epoch + 2, 0));
        // Reset: a new history at the same moment.
        t.reset();
        let reset = t.tick(false, 3.9);
        assert_eq!((reset.epoch, reset.frame), (first.epoch + 3, 0));
        assert!(!reset.still);
    }
}
