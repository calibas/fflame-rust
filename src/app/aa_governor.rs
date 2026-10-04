//! **The frame governor's antialiasing axis**, for escape-time playback.
//!
//! An escape animation advances only on settled frames
//! (`update_animation`), so a frame that outlasts the refresh interval
//! shows as a dropped frame: playback stutters. The viewport's
//! supersampling factor is the knob that dominates an escape frame's cost
//! -- its render pixels grow with its square -- and while an animation
//! plays this sheds it until frames fit the governor's budget
//! (`frame_budget`), and probes back up when there is room.
//!
//! It governs ONLY live playback, and only with the frame governor on
//! (Rendering menu). A still frame, an export and a video render at the
//! config's factor; the moment playback stops the viewport is drawn at it
//! again.
//!
//! The signal is DROPPED frames: a settled frame that came more than
//! half a refresh late. Under vsync a frame that fits always reads as
//! about one refresh interval however light it is, so intervals show
//! overload but never how much room is left -- and they jitter: measured
//! with the viewport at 1x, a twentieth of the frames came 20-23 ms
//! apart against a 16.7 ms budget, with not one frame dropped. An
//! earlier rule shed on the median of three intervals past 1.3 budgets,
//! which that jitter crosses every few seconds; it shed 4x to 1x where 3x
//! held 60 fps, and never found its way back. A dropped frame is a whole
//! refresh late (about 33 ms), which jitter does not reach.
//!
//! So: shed a step when the recent window holds several dropped frames;
//! probe a step up after a run with none. A step up costs `((s+1)/s)^2`
//! -- 3x to 4x is 1.78 times the work -- so a probe that drops frames is
//! reverted at once, and the next try at that step waits twice as long
//! as the last. Each change of factor is a visible change of quality,
//! which is what the waits keep rare.

/// A settled frame this many budgets after the last one was dropped.
const DROPPED: f64 = 1.5;
/// The window overload is judged over, in settled frames.
const WINDOW: usize = 30;
/// Dropped frames in the window that shed a step.
const SHED_AT: usize = 4;
/// Settled frames skipped after a change: the first after a resize
/// reallocates and renders from scratch, and is not the new factor's cost.
const SETTLE: u32 = 2;
/// Settled frames without a drop before the first probe up (two seconds
/// at 60 Hz), and the probe's wait before its first failure.
const GROW_AFTER: u32 = 120;
/// Settled frames a probe must hold to be kept...
const PROBE_FRAMES: u32 = 60;
/// ...dropping fewer than this many.
const PROBE_FAILS_AT: u32 = 3;
/// The longest wait between probes of a step that keeps failing.
const MAX_BACKOFF: u32 = 60 * 64;

#[derive(Debug, Clone, Default)]
pub(crate) struct AaGovernor {
    /// The governed factor while playback runs; `None` when not governing.
    factor: Option<u32>,
    /// Whether each of the last `WINDOW` settled frames was dropped.
    recent: std::collections::VecDeque<bool>,
    /// Settled frames still to skip after a change.
    settle: u32,
    /// Settled frames since the last dropped one, at this factor.
    clean: u32,
    /// While probing a step up: the factor it came from, how long it has
    /// held, and how many frames it dropped.
    probe: Option<(u32, u32, u32)>,
    /// Clean frames required before the next probe: doubles with each
    /// failed one.
    wait: u32,
    /// The factor playback last settled on, to start the next playback
    /// from rather than from the top.
    remembered: Option<u32>,
}

impl AaGovernor {
    /// The factor to render at this frame: the governed one, never above
    /// `ceiling` (the config's factor, as far as the device affords it).
    /// The first call of a playback starts where the last one settled.
    pub(crate) fn factor(&mut self, ceiling: u32) -> u32 {
        let ceiling = ceiling.max(1);
        let start = self.remembered.unwrap_or(ceiling);
        let f = self.factor.unwrap_or(start).min(ceiling);
        self.factor = Some(f);
        f
    }

    /// Playback stopped (or the governor was switched off): render at the
    /// config's factor again.
    pub(crate) fn release(&mut self) {
        if let Some(f) = self.factor.take() {
            // A probe that never finished is not where playback settled.
            self.remembered = Some(self.probe.map_or(f, |(from, _, _)| from));
        }
        self.recent.clear();
        self.settle = 0;
        self.clean = 0;
        self.probe = None;
    }

    /// Whether a playback is being governed.
    pub(crate) fn active(&self) -> bool {
        self.factor.is_some()
    }

    /// One settled frame of playback: its interval over the budget.
    /// Returns whether the factor changed.
    pub(crate) fn observe(&mut self, ratio: f64, ceiling: u32) -> bool {
        let Some(f) = self.factor else { return false };
        if self.settle > 0 {
            self.settle -= 1;
            return false;
        }
        let dropped = ratio > DROPPED;

        if let Some((from, held, drops)) = self.probe {
            let drops = drops + u32::from(dropped);
            if drops >= PROBE_FAILS_AT {
                // Not affordable: back down, and wait twice as long before
                // trying this step again.
                self.probe = None;
                self.wait = (self.wait.max(GROW_AFTER) * 2).min(MAX_BACKOFF);
                return self.set(from);
            }
            if held + 1 >= PROBE_FRAMES {
                self.probe = None;
                self.wait = GROW_AFTER;
            } else {
                self.probe = Some((from, held + 1, drops));
            }
            return false;
        }

        self.recent.push_back(dropped);
        if self.recent.len() > WINDOW {
            self.recent.pop_front();
        }
        if self.recent.iter().filter(|d| **d).count() >= SHED_AT && f > 1 {
            return self.set(f - 1);
        }
        self.clean = if dropped { 0 } else { self.clean + 1 };
        if self.clean >= self.wait.max(GROW_AFTER) && f < ceiling {
            self.probe = Some((f, 0, 0));
            return self.set(f + 1);
        }
        false
    }

    fn set(&mut self, f: u32) -> bool {
        let changed = self.factor != Some(f);
        self.factor = Some(f);
        self.recent.clear();
        self.settle = SETTLE;
        self.clean = 0;
        changed
    }
}

/// What the viewport's corner says: the factor in use, the setting, and
/// why the one is below the other -- the governor when it holds the
/// factor under what the device affords, the view's size when the device
/// affords less than the setting.
pub(crate) fn readout(in_use: u32, requested: u32, governed: bool, ceiling: u32) -> crate::ui::EscapeAa {
    let requested = requested.clamp(1, crate::config::escape::MAX_SUPERSAMPLE);
    let reason = (in_use < requested).then_some(if governed && in_use < ceiling {
        crate::ui::AaReason::Governor
    } else {
        crate::ui::AaReason::Size
    });
    crate::ui::EscapeAa { in_use, requested, reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_readout_names_why_the_factor_is_lower() {
        use crate::ui::AaReason;
        assert_eq!(readout(4, 4, false, 4).reason, None);
        assert_eq!(readout(4, 4, true, 4).reason, None, "governing at the full setting");
        assert_eq!(readout(3, 4, true, 4).reason, Some(AaReason::Governor));
        assert_eq!(readout(4, 8, false, 4).reason, Some(AaReason::Size));
        // Governed at what the size allows: the size is why it is not 8.
        assert_eq!(readout(4, 8, true, 4).reason, Some(AaReason::Size));
        assert_eq!(readout(3, 8, true, 4).reason, Some(AaReason::Governor));
        assert_eq!(readout(8, 12, false, 8).requested, 8, "the setting is read as the renderer clamps it");
    }

    /// Feed settled frames at `ratio` until the factor changes or `n`
    /// frames pass; returns the factor after.
    fn run(g: &mut AaGovernor, ratio: f64, ceiling: u32, n: u32) -> u32 {
        for _ in 0..n {
            if g.observe(ratio, ceiling) {
                break;
            }
        }
        g.factor(ceiling)
    }

    #[test]
    fn dropped_frames_shed_one_step_at_a_time() {
        let mut g = AaGovernor::default();
        assert_eq!(g.factor(4), 4, "playback starts at the config's factor");
        assert_eq!(run(&mut g, 2.0, 4, 100), 3);
        assert_eq!(run(&mut g, 2.0, 4, 100), 2);
        assert_eq!(run(&mut g, 2.0, 4, 100), 1);
        assert_eq!(run(&mut g, 2.0, 4, 100), 1, "1x is the floor");
    }

    /// The jitter measured at 1x (frames 20-23 ms apart against 16.7) is
    /// not overload, and does not stop growth either.
    #[test]
    fn vsync_jitter_neither_sheds_nor_blocks_growth() {
        let mut g = AaGovernor::default();
        g.factor(4);
        assert_eq!(run(&mut g, 2.0, 4, 100), 3);
        let jitter = [1.0, 0.8, 1.3, 1.0, 1.38, 0.7, 1.2];
        let mut grew = false;
        for i in 0..400 {
            if g.observe(jitter[i % jitter.len()], 4) {
                grew = true;
                break;
            }
        }
        assert!(grew && g.factor(4) == 4, "jittery but undropped frames should let it grow back");
    }

    #[test]
    fn an_occasional_dropped_frame_is_noise() {
        let mut g = AaGovernor::default();
        g.factor(4);
        for i in 0..300 {
            let r = if i % 20 == 0 { 2.0 } else { 1.0 };
            assert!(!g.observe(r, 4), "frame {i}");
        }
        assert_eq!(g.factor(4), 4);
    }

    #[test]
    fn with_room_it_probes_up_and_keeps_a_step_that_fits() {
        let mut g = AaGovernor::default();
        g.factor(4);
        assert_eq!(run(&mut g, 2.0, 4, 100), 3);
        assert_eq!(run(&mut g, 1.0, 4, 400), 4);
        // It holds: kept, and no further step past the ceiling.
        assert_eq!(run(&mut g, 1.0, 4, 1000), 4);
    }

    #[test]
    fn a_failed_probe_reverts_and_waits_longer_each_time() {
        let mut g = AaGovernor::default();
        g.factor(4);
        assert_eq!(run(&mut g, 2.0, 4, 100), 3);
        let mut waits = Vec::new();
        for _ in 0..3 {
            // Count the clean frames until the probe.
            let mut n = 0;
            while !g.observe(1.0, 4) {
                n += 1;
                assert!(n < 100_000, "never probed");
            }
            waits.push(n);
            assert_eq!(g.factor(4), 4, "probing");
            // 4x drops frames: straight back to 3.
            assert_eq!(run(&mut g, 2.0, 4, 100), 3);
        }
        assert!(waits[1] > waits[0] && waits[2] > waits[1], "the waits must grow: {waits:?}");
    }

    #[test]
    fn the_ceiling_bounds_it_and_the_next_playback_starts_where_this_settled() {
        let mut g = AaGovernor::default();
        assert_eq!(g.factor(6), 6);
        assert_eq!(run(&mut g, 2.0, 6, 100), 5);
        assert_eq!(run(&mut g, 2.0, 6, 100), 4);
        // The viewport grew and the device affords less.
        assert_eq!(g.factor(3), 3);
        g.release();
        assert!(!g.active());
        assert_eq!(g.factor(6), 3, "starts where the last playback settled");
        g.release();
        assert_eq!(g.factor(2), 2, "...but never above the ceiling");
    }

    #[test]
    fn frames_right_after_a_change_are_not_its_cost() {
        let mut g = AaGovernor::default();
        g.factor(4);
        assert_eq!(run(&mut g, 2.0, 4, 100), 3);
        // The resize frames are slow; they must not shed another step.
        assert!(!g.observe(5.0, 4));
        assert!(!g.observe(5.0, 4));
        for _ in 0..10 {
            assert!(!g.observe(1.0, 4));
        }
        assert_eq!(g.factor(4), 3);
    }
}
