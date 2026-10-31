//! **The quality governor** (docs/archive/projects/quality-governor.md, order of
//! work item 5): one target frame rate, held while the user interacts or
//! an animation plays, by giving up fidelity on the rungs of each engine's
//! ladder -- never the look, and never in a still or an export.
//!
//! This is the core, without the app: a ladder, a cost model and the rule
//! that moves between rungs. A picture's cost is a fixed part and a part
//! that goes with its render pixels (the render scale squared times the
//! supersampling squared). The fixed part is learned as a ratio to the
//! other, in render units (`Governor::fixed`), from any two rungs measured
//! close together, and kept; every prediction scales the current rung's
//! measurement through it. So one fresh measurement predicts every rung,
//! and the governor goes straight to the rung that fits rather than
//! probing down a step at a time. The fixed part is for what does not follow
//! the pixels -- the tail and the upscale at the display's size, work done
//! once a picture -- where a pixels-only model would shed a picture for a
//! saving that is not there. When no rung fits the share, the governor takes the
//! best that still fits `PACE_SHARE` (frames still come evenly there, if
//! tightly), and when none does, the best within `NO_FIT_SLACK` of the
//! cheapest: the target is out of reach, and quality given away for little
//! is a bad trade. Every decision is made over a window of pictures
//! (`WINDOW`), on their median, so a hiccup -- a picture something else on
//! the machine held up -- moves nothing (the user, 2026-10-30: "I'd rather
//! it just drop a frame now and then, unless it's consistently
//! struggling"). It probes UP a
//! step at a time, after a run of pictures with room,
//! because predictions are wrong at the edges (an interior that iterates
//! to the cap, a fill), and a failed probe waits twice as long next time,
//! as `aa_governor` learned. The frames right after a change are not its
//! cost (`SETTLE`).
//!
//! The measure is the GPU's own time for a picture (the frame timer's
//! engine span, summed over the frames a picture takes to settle), against
//! a share of the frame budget: phase 1 measured frames pacing evenly with
//! the GPU at ~65% of the budget and alternating at ~84%. A CPU-bound frame
//! is not shed for: a cheaper rung would not make it faster. Nor is a frame
//! something else holds back: a shed is judged by the frames after it
//! (`SHED_CHECK`, `Pace`), and one that neither raised the frame rate nor
//! ended an alternation of long and short frames is undone, and not tried
//! again while the frame rate stays -- measured 2026-10-30, a window
//! presented at 60 fps on a 120 Hz display whatever it drew, and the
//! governor shed to its lowest rung for nothing.

/// What a picture is rendered with: escape-time's rung.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rung {
    /// The supersampling factor, per axis.
    pub supersample: u32,
    /// The render scale, per axis (app/render_scale.rs).
    pub scale: f32,
}

impl Rung {
    /// Render pixels per display pixel: what a picture's cost goes with.
    pub fn units(&self) -> f64 {
        let s = self.scale as f64 * self.supersample as f64;
        s * s
    }
}

/// The share of the frame budget a picture's GPU time may take: the rest
/// is the UI's, the compositor's and the margin even pacing needs.
pub(crate) const GPU_SHARE: f64 = 0.7;
/// The share past which frames stop coming evenly (phase 1: even at ~65%,
/// alternating at ~84%): the rung to take when none fits `GPU_SHARE`.
const PACE_SHARE: f64 = 0.8;
/// Pictures a rung is judged over: the median of these is its cost, and
/// the decisions are made on that.
const WINDOW: usize = 20;
/// The one quicker shed: on a rung just entered, its first this many
/// pictures with their median past `QUICK_OVER` budgets -- a rung that
/// cannot be afforded at all, entered before its cost was known.
const QUICK: usize = 4;
const QUICK_OVER: f64 = 3.0;
/// Pictures with the rung above predicted to fit before it is probed, at
/// first; doubled after each failed probe, up to `PROBE_WAIT_MAX`.
const PROBE_WAIT: u32 = 30;
const PROBE_WAIT_MAX: u32 = 480;
/// Pictures after a change that are not its cost: the resize, the new
/// pipelines, the first chunk sizes.
pub(crate) const SETTLE: u32 = 2;
/// How fast the learned fixed part follows a new estimate.
const COST_EMA: f64 = 0.3;
/// Pictures a rung's measurement stays fresh for: the content changes as a
/// view moves, so two measurements further apart do not describe one
/// picture's costs.
const FRESH: u64 = 120;
/// The largest fixed part, in render units: a cost that does not move
/// with the pixels at all.
const MAX_FIXED: f64 = 1000.0;
/// When no rung fits: the best rung within this factor of the cheapest.
const NO_FIT_SLACK: f64 = 1.2;
/// Pictures after a shed before its effect on the frames is judged.
const SHED_CHECK: u32 = 30;
/// A shed must raise the frame rate by this factor, or end an alternation,
/// or it is undone.
const SHED_GAIN: f64 = 1.1;
/// A frame rate that moves by more than this share is a new situation: a
/// verdict that shedding does not help lapses.
const RATE_MOVED: f64 = 0.15;

/// How the frames are coming (the app's measure, over the last 32): their
/// rate, frames a second, and the 75th percentile of their intervals, ms,
/// which sees frames alternating long and short (half are long) and not the
/// odd slow one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pace {
    pub rate: f64,
    pub p75_ms: f64,
}

impl Pace {
    /// Frames alternating long and short: a quarter or more a refresh late.
    fn alternating(&self, budget_ms: f64) -> bool {
        self.p75_ms >= 1.5 * budget_ms
    }
}

/// Escape-time's ladder, best first (phase 1, *Results*). A 2D picture
/// keeps 2x supersampling longest -- aliasing costs more than resolution --
/// so it steps from the ceiling down to 2x, then to 75% and 50%, and to 1x
/// last. A mode-D solid renders the same picture at 2x and above, so its
/// ladder starts at 2x at most, then 1x, then 75%, 50% and 25% (D5).
pub(crate) fn escape_ladder(ceiling: u32, solid: bool) -> Vec<Rung> {
    let ceiling = ceiling.max(1);
    let r = |supersample, scale| Rung { supersample, scale };
    let mut v = Vec::new();
    if solid {
        if ceiling >= 2 {
            v.push(r(2, 1.0));
        }
        for scale in [1.0, 0.75, 0.5, 0.25] {
            v.push(r(1, scale));
        }
        return v;
    }
    for ss in (2..=ceiling).rev() {
        v.push(r(ss, 1.0));
    }
    let keep = ceiling.min(2);
    for scale in [0.75, 0.5] {
        v.push(r(keep, scale));
    }
    if keep > 1 {
        v.push(r(1, 0.5));
    }
    if ceiling == 1 {
        v.insert(0, r(1, 1.0));
    }
    v
}

/// An escape terrain's ladder: the render scale of its lit tier, 100%,
/// 75% and 50% (D5's floor). While it moves a frame draws one lit pass of
/// the new view (`TerrainTiers::viewport`), whose cost goes with the
/// render pixels; its antialiasing grid is drawn over frames at rest and
/// costs a moving frame nothing, so it is not a rung. The sections it
/// builds as the view moves are the cost's fixed part.
pub(crate) fn terrain_ladder() -> Vec<Rung> {
    [1.0, 0.75, 0.5].into_iter().map(|scale| Rung { supersample: 1, scale }).collect()
}

/// A flame's render scale while governed, from the samples a frame per
/// DISPLAY pixel (phase 1, *Results*): a flame meets its frame rate by
/// shedding samples (the batch governor), and what it loses is grain, so the
/// question is which loses less at a given sample count -- the full size,
/// grainy, or a smaller render with more samples a pixel, softer. Measured,
/// 2D: the smaller won below about 2 samples a display pixel (1: 13.1 at
/// 75% against 15.7 full; 4: 10.0 full against 10.1), and 50% only below
/// about a half. 3D, softer, holds on longer: 50% won at 16 (7.0 against
/// 8.5 full and 7.2 at 75%) and full size at 64 (5.4 against 6.4), so the
/// steps sit between, at 32 and 16.
pub(crate) fn flame_scale(samples_per_display_px: f64, three_d: bool) -> f32 {
    let (full, three_quarters) = if three_d { (32.0, 16.0) } else { (2.0, 0.5) };
    if samples_per_display_px >= full {
        1.0
    } else if samples_per_display_px >= three_quarters {
        0.75
    } else {
        0.5
    }
}

/// Frames a flame's new scale must hold before it is taken: a scale
/// change restarts the accumulation and reallocates, so not on a flicker of
/// the batch.
const FLAME_HOLD: u32 = 30;

/// Frames the batch's cap is learned over (the median of their GPU time
/// besides the dispatch), and the fewest it is given from.
const CAP_FRAMES: usize = 16;
const CAP_MIN_FRAMES: usize = 4;
/// How fast a dispatch's cost a thread-iteration follows a new measure.
const CAP_EMA: f64 = 0.3;

/// **The flame batch's pacing cap** (the plan's *The second run*): the most
/// thread-iterations a frame may dispatch for the frame's whole GPU span
/// to fit `GPU_SHARE` of the budget. The batch governor reads frame
/// intervals, and under vsync a frame on time reads the refresh however
/// full the GPU is, so it grows the batch until frames miss; at the
/// budget's edge they then alternate long and short (about 1 and 32 ms at
/// 60 Hz, measured 2026-10-29 at 3,600 iterations a thread), the median on
/// time and the animation's clock jumping. A cap on the dispatch alone did
/// not end it: the batch sat at a third and frames still alternated at ~84%
/// of the budget in GPU time, since the tail, the UI and the effects take
/// their share, and it varies. So a frame's span is split by a linear
/// model into its dispatch (the dispatch's own timed cost a
/// thread-iteration, times what the frame dispatched) and the rest, and the
/// cap is the room the rest leaves. The rest does not follow the batch, so
/// neither does the cap: there is no loop through the few frames the GPU's
/// times lag by. When the rest alone overruns the share, the dispatch keeps
/// `NO_FIT_SLACK` of it: the frame rate is out of the batch's reach, and
/// samples given away for nothing are a bad trade.
#[derive(Debug, Default)]
pub(crate) struct BatchCap {
    /// Thread-iterations recent frames dispatched, by frame.
    dispatched: std::collections::VecDeque<(u64, f64)>,
    /// The dispatch's GPU ms a thread-iteration, smoothed.
    per_iteration: Option<f64>,
    /// Recent frames' GPU ms besides their dispatch.
    rest: std::collections::VecDeque<f64>,
}

impl BatchCap {
    /// The frame `frame` dispatched `iterations` thread-iterations.
    pub fn dispatched(&mut self, frame: u64, iterations: f64) {
        self.dispatched.push_back((frame, iterations));
        while self.dispatched.len() > FRAMES_KEPT {
            self.dispatched.pop_front();
        }
    }

    /// A dispatch timed on the GPU: `ms` for `iterations` thread-iterations.
    pub fn dispatch_timed(&mut self, ms: f64, iterations: f64) {
        if !(ms > 0.0) || !(iterations > 0.0) {
            return;
        }
        let b = ms / iterations;
        self.per_iteration = Some(self.per_iteration.map_or(b, |p| p + CAP_EMA * (b - p)));
    }

    /// Frames' GPU times heard back: each one that dispatched, its span
    /// less its dispatch.
    pub fn frames_timed(&mut self, timings: &[crate::gpu::timer::FrameTiming]) {
        let Some(b) = self.per_iteration else { return };
        for t in timings {
            if let Some(&(_, n)) = self.dispatched.iter().find(|(f, _)| *f == t.frame) {
                self.rest.push_back((t.ui_ms + t.engine_ms - b * n).max(0.0));
            }
        }
        while self.rest.len() > CAP_FRAMES {
            self.rest.pop_front();
        }
    }

    /// The most thread-iterations a frame may dispatch against `budget_ms`;
    /// None until enough frames are timed.
    pub fn cap(&self, budget_ms: f64) -> Option<f64> {
        let b = self.per_iteration?;
        if self.rest.len() < CAP_MIN_FRAMES || !(budget_ms > 0.0) {
            return None;
        }
        let mut v: Vec<f64> = self.rest.iter().copied().collect();
        v.sort_by(f64::total_cmp);
        let rest = v[v.len() / 2];
        let room = (GPU_SHARE * budget_ms - rest).max((NO_FIT_SLACK - 1.0) * rest);
        Some(room / b)
    }
}

/// **Without a GPU timer** (a browser without `timestamp-query`): the rung
/// at the dropped-frame walker's `position` on escape-time's ladder
/// (`aa_governor::AaGovernor`, which counts 1 to the ladder's length, the
/// length the best rung).
pub(crate) fn fallback_rung(ladder: &[Rung], position: u32) -> Rung {
    let n = ladder.len().max(1);
    let p = (position as usize).clamp(1, n);
    ladder.get(n - p).copied().unwrap_or(Rung { supersample: 1, scale: 1.0 })
}

/// The batch scale under the pacing cap (`BatchCap`, as a share of the
/// full batch): never above it, and coming down to it from `before`, the
/// last frame's, at the batch governor's slew (`adjust_iter_scale`), since
/// in playback a jump in the samples a frame is a jump in the grain.
pub(crate) fn capped_scale(scale: f64, before: f64, cap: f64) -> f64 {
    if scale > cap {
        cap.max(before / 1.15).min(scale)
    } else {
        scale
    }
}

/// One engine's governor: its ladder, the rung in use, and what it has
/// learned of the cost.
#[derive(Debug, Clone)]
pub(crate) struct Governor {
    ladder: Vec<Rung>,
    rung: usize,
    /// Each rung's measured GPU ms (the median of its window), and the
    /// picture it was last measured at.
    measured: Vec<Option<(f64, u64)>>,
    /// Pictures observed.
    pictures: u64,
    /// The fixed part of a picture's cost, in render units: a picture of
    /// `u` units costs `b * (u + fixed)` ms. Learned from two rungs, kept.
    fixed: f64,
    /// Whether `fixed` has been estimated at all: the first estimate is
    /// taken whole, later ones smoothed in.
    fixed_known: bool,
    settle: u32,
    /// The current rung's last pictures, ms (up to `WINDOW`).
    window: std::collections::VecDeque<f64>,
    room: u32,
    probe_wait: u32,
    /// The rung a probe left, to go back to if it fails.
    probing_from: Option<usize>,
    /// A shed being judged: the rung it left, the frames then, and the
    /// pictures before the verdict.
    shed_from: Option<(usize, Pace, u32)>,
    /// A shed that did not help: no shedding below this rung while the
    /// frame rate stays near this one.
    no_shed_below: Option<(usize, f64)>,
    /// How the frames come now.
    pace: Pace,
}

impl Governor {
    pub fn new(ladder: Vec<Rung>) -> Self {
        assert!(!ladder.is_empty(), "a ladder has a rung");
        let n = ladder.len();
        Governor {
            ladder,
            rung: 0,
            measured: vec![None; n],
            pictures: 0,
            fixed: 0.0,
            fixed_known: false,
            settle: SETTLE,
            window: Default::default(),
            room: 0,
            probe_wait: PROBE_WAIT,
            probing_from: None,
            shed_from: None,
            no_shed_below: None,
            pace: Pace { rate: 0.0, p75_ms: 0.0 },
        }
    }

    pub fn rung(&self) -> Rung {
        self.ladder[self.rung]
    }

    /// The rung's place on the ladder, 0 the best (the frame log's).
    pub fn rung_index(&self) -> usize {
        self.rung
    }

    /// The learned fixed part, in render units (the frame log's).
    pub fn fixed(&self) -> f64 {
        self.fixed
    }

    /// A new ladder (the ceiling changed, or the kind of picture): the rung
    /// with the nearest cost, and the measurements of the rungs it keeps.
    pub fn set_ladder(&mut self, ladder: Vec<Rung>) {
        if ladder == self.ladder || ladder.is_empty() {
            return;
        }
        let units = self.rung().units();
        let nearest = (0..ladder.len())
            .min_by(|&a, &b| (ladder[a].units() - units).abs().total_cmp(&(ladder[b].units() - units).abs()))
            .unwrap_or(0);
        let measured = ladder
            .iter()
            .map(|r| self.ladder.iter().position(|o| o == r).and_then(|i| self.measured[i]))
            .collect();
        self.ladder = ladder;
        self.measured = measured;
        self.rung = nearest;
        self.changed();
    }

    /// Back to the best rung, as a still is drawn: the measurements and the
    /// probe wait are kept, so the next governed run starts informed.
    pub fn release(&mut self) {
        if self.rung != 0 {
            self.rung = 0;
            self.changed();
        }
        self.probing_from = None;
        self.shed_from = None;
    }

    /// The display grew or shrank by `factor` in pixels: the measurements
    /// scale with it (the fixed part too, roughly: it is mostly per pixel
    /// of the display, the tail's and the upscale's).
    pub fn scale_cost(&mut self, factor: f64) {
        if factor.is_finite() && factor > 0.0 {
            for (ms, _) in self.measured.iter_mut().flatten() {
                *ms *= factor;
            }
        }
    }

    /// The size changed (after `scale_cost`), or a governed run begins:
    /// the rung is predicted at once, a resize being a new cost, not
    /// noise. True if the rung changed.
    pub fn resized(&mut self, budget_ms: f64) -> bool {
        let Some(mut best) = self.choose(GPU_SHARE * budget_ms) else { return false };
        if let Some((floor, _)) = self.no_shed_below {
            best = best.min(floor.max(self.rung));
        }
        self.window.clear();
        self.room = 0;
        if best != self.rung {
            // A prediction downward is judged like any shed.
            if best > self.rung && self.pace.rate > 0.0 {
                self.shed_from = Some((self.rung, self.pace, SHED_CHECK));
            }
            self.rung = best;
            self.probing_from = None;
            self.changed();
            return true;
        }
        false
    }

    fn changed(&mut self) {
        self.settle = SETTLE;
        self.window.clear();
        self.room = 0;
    }

    /// The median of the window's pictures.
    fn median(&self) -> f64 {
        let mut v: Vec<f64> = self.window.iter().copied().collect();
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    }

    /// Rung `i`'s measurement, if it is fresh.
    fn fresh(&self, i: usize) -> Option<f64> {
        self.measured[i].filter(|(_, at)| self.pictures - at <= FRESH).map(|(ms, _)| ms)
    }

    /// Rung `i`'s cost, scaled from the current rung's fresh measurement
    /// through the fixed part. None before the current rung is measured.
    fn predict(&self, i: usize) -> Option<f64> {
        let m0 = self.fresh(self.rung)?;
        let per_unit = m0 / (self.rung().units() + self.fixed);
        Some(per_unit * (self.ladder[i].units() + self.fixed))
    }

    /// The fixed part again, from the current rung's measurement and the
    /// freshest of another rung's: `m = b (u + fixed)` at both.
    fn learn_fixed(&mut self) {
        let Some(mk) = self.fresh(self.rung) else { return };
        let uk = self.rung().units();
        let other = (0..self.ladder.len())
            .filter(|&j| j != self.rung && (self.ladder[j].units() - uk).abs() > 1e-9)
            .filter_map(|j| self.measured[j].filter(|(_, at)| self.pictures - at <= FRESH).map(|(m, at)| (j, m, at)))
            .max_by_key(|&(_, _, at)| at);
        let Some((j, mj, _)) = other else { return };
        let uj = self.ladder[j].units();
        // From mk (uj + f) = mj (uk + f).
        let estimate = if (mk - mj).abs() < 1e-9 {
            MAX_FIXED
        } else {
            ((mj * uk - mk * uj) / (mk - mj)).clamp(0.0, MAX_FIXED)
        };
        if self.fixed_known {
            self.fixed += COST_EMA * (estimate - self.fixed);
        } else {
            self.fixed = estimate;
            self.fixed_known = true;
        }
    }

    /// The rung to be on for `target` ms (`GPU_SHARE` of the budget): the
    /// best predicted to fit it; else the best to fit `PACE_SHARE`; else,
    /// with nothing making the frame rate, the best within `NO_FIT_SLACK`
    /// of the cheapest. None before anything has been measured.
    fn choose(&self, target: f64) -> Option<usize> {
        let costs: Vec<f64> = (0..self.ladder.len()).map(|i| self.predict(i)).collect::<Option<_>>()?;
        let tight = target * PACE_SHARE / GPU_SHARE;
        if let Some(i) = costs.iter().position(|&c| c <= target).or_else(|| costs.iter().position(|&c| c <= tight)) {
            return Some(i);
        }
        let cheapest = costs.iter().copied().fold(f64::INFINITY, f64::min);
        costs.iter().position(|&c| c <= NO_FIT_SLACK * cheapest)
    }

    /// A settled picture: its GPU time, the frame budget, the CPU's own
    /// time a frame (smoothed), and how the frames are coming. True if the
    /// rung changed.
    pub fn observe(&mut self, picture_ms: f64, budget_ms: f64, cpu_ms: f64, pace: Pace) -> bool {
        if !(picture_ms > 0.0) || !(budget_ms > 0.0) {
            return false;
        }
        self.pace = pace;
        // A frame rate that moved is a new situation: an old verdict that
        // shedding does not help no longer holds.
        if self.no_shed_below.is_some_and(|(_, at)| (pace.rate - at).abs() > RATE_MOVED * at) {
            self.no_shed_below = None;
        }
        if self.settle > 0 {
            self.settle -= 1;
            return false;
        }
        self.pictures += 1;
        // A shed judged: kept if the frames came faster or stopped
        // alternating, else undone.
        if let Some((from, before, left)) = self.shed_from {
            if left > 1 {
                self.shed_from = Some((from, before, left - 1));
            } else {
                self.shed_from = None;
                let faster = pace.rate >= SHED_GAIN * before.rate;
                let evened = before.alternating(budget_ms) && pace.p75_ms < 1.25 * budget_ms;
                if !faster && !evened {
                    self.no_shed_below = Some((from, pace.rate));
                    self.rung = from;
                    self.changed();
                    return true;
                }
            }
        }
        // The rung's cost is its window's median, once a few are in: a
        // hiccup does not move it.
        self.window.push_back(picture_ms);
        while self.window.len() > WINDOW {
            self.window.pop_front();
        }
        let median = self.median();
        if self.window.len() >= QUICK {
            self.measured[self.rung] = Some((median, self.pictures));
            self.learn_fixed();
        }
        let target = GPU_SHARE * budget_ms;
        // A frame the CPU holds up gains nothing from a cheaper picture.
        if cpu_ms >= target {
            self.room = 0;
            return false;
        }
        // Struggling: the window full and over the share; or, on a rung just
        // entered, its first few far past the budget (one that cannot be
        // afforded at all).
        let full = self.window.len() >= WINDOW;
        let over = if full { median > target } else { self.window.len() >= QUICK && median > QUICK_OVER * budget_ms };
        if over {
            self.room = 0;
            // A probe that does not fit goes back, and waits longer before
            // the next.
            if let Some(from) = self.probing_from.take() {
                self.rung = from;
                self.probe_wait = (self.probe_wait * 2).min(PROBE_WAIT_MAX);
                self.changed();
                return true;
            }
            // The rung the model now asks for: lower if one fits, or
            // higher if none does and a better one costs about as little
            // as the cheapest.
            let mut next = self.choose(target).unwrap_or(self.rung);
            if let Some((floor, _)) = self.no_shed_below {
                next = next.min(floor.max(self.rung));
            }
            if next != self.rung {
                if next > self.rung {
                    // A shed during another's check is judged with it,
                    // against where the first began.
                    self.shed_from = Some(match self.shed_from {
                        Some((from, before, _)) => (from, before, SHED_CHECK),
                        None => (self.rung, self.pace, SHED_CHECK),
                    });
                }
                self.rung = next;
                self.changed();
                return true;
            }
            return false;
        }
        if !full {
            return false;
        }
        // A window that fits: a probe that held is the new rung.
        if self.probing_from.take().is_some() {
            self.probe_wait = PROBE_WAIT;
        }
        if self.rung == 0 {
            return false;
        }
        let up = self.rung - 1;
        if self.predict(up).is_some_and(|c| c <= target) {
            self.room += 1;
            if self.room >= self.probe_wait {
                self.probing_from = Some(self.rung);
                self.rung = up;
                self.changed();
                return true;
            }
        } else {
            self.room = 0;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How frames come for a picture of `ms`: one a refresh while the GPU
    /// is under 80% of the budget; past it, alternating (phase 1), every
    /// other one late; past the budget, a refresh late for each missed.
    fn pace(ms: f64, budget: f64) -> Pace {
        let refreshes = (ms / budget).ceil().max(1.0);
        let alternating = ms > 0.8 * budget && ms <= budget;
        let p75 = if alternating { 2.0 * budget } else { refreshes * budget };
        let mean = if alternating { 1.5 * budget } else { refreshes * budget };
        Pace { rate: 1000.0 / mean, p75_ms: p75 }
    }

    /// A model of the GPU: `ms_per_unit` a render unit.
    fn run(g: &mut Governor, ms_per_unit: f64, budget: f64, pictures: usize) -> Vec<Rung> {
        (0..pictures)
            .map(|_| {
                let ms = ms_per_unit * g.rung().units();
                g.observe(ms, budget, 1.0, pace(ms, budget));
                g.rung()
            })
            .collect()
    }

    #[test]
    fn the_ladders_follow_phase_1() {
        let at = |l: Vec<Rung>| l.iter().map(|r| (r.supersample, r.scale)).collect::<Vec<_>>();
        assert_eq!(at(escape_ladder(4, false)), vec![(4, 1.0), (3, 1.0), (2, 1.0), (2, 0.75), (2, 0.5), (1, 0.5)]);
        assert_eq!(at(escape_ladder(2, false)), vec![(2, 1.0), (2, 0.75), (2, 0.5), (1, 0.5)]);
        assert_eq!(at(escape_ladder(1, false)), vec![(1, 1.0), (1, 0.75), (1, 0.5)]);
        assert_eq!(at(escape_ladder(4, true)), vec![(2, 1.0), (1, 1.0), (1, 0.75), (1, 0.5), (1, 0.25)]);
        assert_eq!(at(escape_ladder(1, true)), vec![(1, 1.0), (1, 0.75), (1, 0.5), (1, 0.25)]);
        for l in [escape_ladder(4, false), escape_ladder(3, true)] {
            assert!(l.windows(2).all(|w| w[0].units() >= w[1].units()), "cheaper down the ladder: {l:?}");
        }
    }

    /// Phase 1's escape zoom at 120 Hz: 2x took ~20 ms against 1x's 5.3,
    /// in an 8.3 ms budget. The governor goes straight to the best rung
    /// that fits, 2x at 50% (the cost of 1x at full size), not to 1x.
    #[test]
    fn it_goes_straight_to_the_rung_that_fits() {
        let budget = 1000.0 / 120.0;
        let mut g = Governor::new(escape_ladder(4, false));
        let rungs = run(&mut g, 5.3, budget, 12);
        let last = *rungs.last().unwrap();
        assert_eq!((last.supersample, last.scale), (2, 0.5), "{rungs:?}");
        let changes = rungs.windows(2).filter(|w| w[0] != w[1]).count();
        assert!(changes <= 2, "a step or two, not a walk down the ladder: {rungs:?}");
    }

    /// Held under a steady load: no hunting, no probing up into a rung that
    /// does not fit (the antialiasing governor probed 3x every 4 s).
    #[test]
    fn it_holds_a_steady_load_without_hunting() {
        let budget = 1000.0 / 120.0;
        let mut g = Governor::new(escape_ladder(4, false));
        run(&mut g, 5.3, budget, 20);
        let held = g.rung();
        let rungs = run(&mut g, 5.3, budget, 2000);
        assert!(rungs.iter().all(|r| *r == held), "it moved off {held:?}");
    }

    /// When the load lightens it climbs back, a step at a time after a run
    /// with room; a probe that fails goes back and waits twice as long.
    #[test]
    fn it_probes_up_with_backoff() {
        let budget = 1000.0 / 60.0;
        let mut g = Governor::new(escape_ladder(4, false));
        run(&mut g, 8.0, budget, 20);
        let low = g.rung;
        assert!(low > 0, "heavy: shed");
        let rungs = run(&mut g, 0.5, budget, 400);
        assert_eq!(g.rung, 0, "light: back to the top, {rungs:?}");
        // Just over the edge for the rung above: a probe, a failure, a longer wait.
        let mut g = Governor::new(escape_ladder(2, false));
        // 2x at 75% = 2.25 units fits 11.67 ms at 4 ms/unit? 9 ms: yes; 2x full = 4 units, 16 ms: no.
        run(&mut g, 4.0, budget, 30);
        assert_eq!(g.rung().scale, 0.75);
        let before = g.probe_wait;
        // A model where the prediction is optimistic: below the top the
        // pictures cost 2 ms a unit, so the top looks like 8 ms and room
        // to spare; the top itself costs 30. Every probe fails, and each
        // waits twice as long as the last (30, 60, ... 480 pictures).
        let mut probes = 0;
        for _ in 0..2000 {
            let ms = if g.rung == 0 { 30.0 } else { 2.0 * g.rung().units() };
            if g.observe(ms, budget, 1.0, pace(ms, budget)) && g.rung == 0 {
                probes += 1;
            }
        }
        assert!((2..=8).contains(&probes), "{probes} probes in 2000 pictures");
        assert!(g.probe_wait > before, "the wait grew");
        assert_eq!(g.rung().scale, 0.75, "it ends where the top does not fit");
    }

    /// Hiccups -- a picture or two something else held up, now and then --
    /// move nothing: a rung's cost is its window's median, and the frame
    /// rate a trimmed mean (so the frames' pace here is the picture's
    /// without them). A real load sheds, and once it lifts the governor
    /// climbs back through the same hiccups instead of sticking low.
    #[test]
    fn hiccups_neither_shed_nor_hold_it_down() {
        let budget = 1000.0 / 120.0;
        let mut g = Governor::new(escape_ladder(2, false));
        let feed = |g: &mut Governor, per_unit: f64, pictures: usize| {
            (0..pictures)
                .map(|i| {
                    let ms = per_unit * g.rung().units();
                    let seen = match i % 40 {
                        17 => 60.0,
                        18 => 30.0,
                        _ => ms,
                    };
                    g.observe(seen, budget, 1.0, pace(ms, budget));
                    g.rung
                })
                .collect::<Vec<_>>()
        };
        // 2x at full size, 4 units at 1.2 ms: 4.8 ms, inside the 5.8 share.
        let calm = feed(&mut g, 1.2, 2000);
        assert!(calm.iter().all(|&r| r == 0), "it shed for a hiccup");
        // 12 ms at the top: over, consistently.
        feed(&mut g, 3.0, 100);
        assert!(g.rung > 0, "a real load sheds");
        let after = feed(&mut g, 1.2, 1000);
        assert_eq!(g.rung, 0, "back to the top: {after:?}");
    }

    /// A simulation picks the best grid its steps fit at, from the
    /// plan's scales, and never above the config's.
    #[test]
    fn a_simulation_picks_the_grid_its_steps_fit() {
        // Lenia at 1080p: 27 ms a step. Two steps a frame in the 11.7 ms
        // share of 60 Hz fit at 37.5% (7.6 ms), not 50% (13.5).
        assert_eq!(sim_pick(1.0, 1.0, 27.0, 2.0, 11.7), 0.375);
        // Cheap: the config's own scale.
        assert_eq!(sim_pick(1.0, 1.0, 0.3, 4.0, 11.7), 1.0);
        // A config at 60% picks from 60% and below.
        assert_eq!(sim_pick(0.6, 0.6, 1.0, 4.0, 11.7), 0.6);
        assert_eq!(sim_pick(0.6, 0.6, 5.0, 4.0, 11.7), 0.375);
        // Measured on a smaller grid, predicted up: 1.7 ms at 25% is
        // 27 at full size.
        assert_eq!(sim_pick(1.0, 0.25, 1.7, 2.0, 11.7), 0.375);
        // Nothing fits: below the step's floor a smaller grid saves
        // nothing, so it stops where the cost stops falling.
        let s = sim_pick(1.0, 1.0, 400.0, 200.0, 11.7);
        assert!(s <= 0.1875, "{s}");
    }

    /// Held through a run: picked on its first measured frame, kept
    /// while it is not far wrong, picked again after a while far wrong,
    /// never while edits come; the config's scale on stop.
    #[test]
    fn a_simulation_holds_its_pick_for_the_run() {
        let id = 7;
        let full = 1920.0 * 1080.0;
        let mut g = SimGovernor::default();
        assert_eq!(g.frame(id, 1.0, full, 11.7, 16.7, false), None, "nothing measured");
        g.steps = Some(2.0);
        // The first batch reads high; the cost is the median of four.
        let mut batch = 0;
        let mut time = |g: &mut SimGovernor, ms: f64, cells: f64| {
            batch += 1;
            g.measured(id, batch, ms, cells);
        };
        for ms in [60.0, 27.0, 27.0] {
            time(&mut g, ms, full);
            assert_eq!(g.frame(id, 1.0, full, 11.7, 16.7, false), None, "not yet");
        }
        time(&mut g, 27.0, full);
        assert_eq!(g.frame(id, 1.0, full, 11.7, 16.7, false), Some((0.375, true)));
        let held = full * 0.375 * 0.375;
        // The same cost at the held grid: held.
        for _ in 0..500 {
            time(&mut g, 27.0 * 0.375 * 0.375, held);
            assert_eq!(g.frame(id, 1.0, held, 11.7, 16.7, false), Some((0.375, false)));
        }
        // The content gets six times dearer (46 ms of steps a frame, under
        // half the target rate): too slow, but an edit holds
        // the pick until it stops.
        let mut changed = None;
        for i in 0..400 {
            time(&mut g, 6.0 * 27.0 * 0.375 * 0.375, held);
            let editing = i < 200;
            if let Some((s, true)) = g.frame(id, 1.0, held, 11.7, 16.7, editing) {
                changed = Some((i, s));
                break;
            }
        }
        let (at, s) = changed.expect("picked again");
        assert!(at >= 200 + SIM_FAR_FRAMES as usize - 1, "not while editing: {at}");
        assert!(s < 0.375, "{s}");
        assert!(g.release(1.0), "let go: the field is carried back");
        assert!(g.step_ms(id, full).is_some(), "the cost is kept");
        assert!(!g.release(1.0));
    }

    /// The timeline's steps a frame come from its step rate, at the
    /// budget's frame, whatever the frames come at now.
    #[test]
    fn a_timelines_steps_a_frame_follow_its_rate() {
        let mut g = SimGovernor::default();
        // 600 steps a second, the frames coming at 20 fps.
        for i in 0..40 {
            g.timeline(i as f64 * 0.05, i * 30, 1000.0 / 60.0);
        }
        assert!((g.steps.unwrap() - 10.0).abs() < 1e-6, "{:?}", g.steps);
    }

    /// A CPU-bound frame is not shed for.
    #[test]
    fn a_cpu_bound_frame_is_not_shed_for() {
        let budget = 1000.0 / 60.0;
        let mut g = Governor::new(escape_ladder(4, false));
        for _ in 0..50 {
            let ms = 4.0 * g.rung().units();
            g.observe(ms, budget, 30.0, pace(ms, budget));
        }
        assert_eq!(g.rung, 0);
    }

    /// A resize re-predicts at once from the learned cost: twice the
    /// pixels, twice the cost a unit.
    #[test]
    fn a_resize_is_predicted_at_once() {
        let budget = 1000.0 / 60.0;
        let mut g = Governor::new(escape_ladder(2, false));
        run(&mut g, 2.0, budget, 10);
        assert_eq!(g.rung, 0, "fits at 8 ms");
        // A bigger window: the app scales the learned cost by the pixels.
        g.scale_cost(2.0);
        assert!(g.resized(budget));
        assert_eq!(g.rung().scale, 0.75, "4 ms a unit: 2x at 75% is 9 ms and fits 11.7; full size, 16, does not");
    }

    /// Frames held to 60 fps on a 120 Hz display while the app's own work
    /// is light: the budget is the 60 fps frame. The same interval with the
    /// GPU's span past the refresh is the GPU's own doing, and the budget
    /// stays the refresh's. Frames that come faster again lift the cap.
    #[test]
    fn a_cap_from_elsewhere_sets_the_budget() {
        let refresh = 1000.0 / 120.0;
        let held = |span: f64| {
            let mut g = GovernorState::default();
            g.cpu_ms = 3.0;
            for _ in 0..32 {
                g.frame_interval(1000.0 / 60.0);
                g.spans.push_back(span);
            }
            g
        };
        let mut light = held(2.5);
        assert!((light.effective_budget(refresh) - 2.0 * refresh).abs() < 1e-9, "held elsewhere: 60 fps");
        let mut heavy = held(9.0);
        assert_eq!(heavy.effective_budget(refresh), refresh, "the GPU's own: the refresh");
        for _ in 0..32 {
            light.frame_interval(refresh);
        }
        assert_eq!(light.effective_budget(refresh), refresh, "frames came faster: the cap lifted");
    }

    /// The frame rate leaves out a hiccup: one frame of 100 ms among 120 Hz
    /// ones is still 120 fps; frames alternating one and two refreshes are
    /// still 80.
    #[test]
    fn a_hiccup_does_not_move_the_frame_rate() {
        let refresh = 1000.0 / 120.0;
        let mut g = GovernorState::default();
        for i in 0..32 {
            g.frame_interval(if i == 20 { 100.0 } else { refresh });
        }
        assert!((g.pace().rate - 120.0).abs() < 0.5, "{}", g.pace().rate);
        for i in 0..32 {
            g.frame_interval(if i % 2 == 0 { refresh } else { 2.0 * refresh });
        }
        assert!((g.pace().rate - 80.0).abs() < 0.5, "{}", g.pace().rate);
    }

    /// A flame's scale follows phase 1's break-evens: 2D gives way below 2
    /// samples a display pixel, 3D below 32.
    #[test]
    fn a_flames_scale_follows_its_samples() {
        assert_eq!(flame_scale(7.2, false), 1.0);
        assert_eq!(flame_scale(1.0, false), 0.75);
        assert_eq!(flame_scale(0.25, false), 0.5);
        assert_eq!(flame_scale(64.0, true), 1.0);
        assert_eq!(flame_scale(20.0, true), 0.75);
        assert_eq!(flame_scale(7.2, true), 0.5);
    }

    /// Stopped, the best rung again, the cost remembered.
    #[test]
    fn release_returns_to_the_top_and_keeps_the_cost() {
        let budget = 1000.0 / 60.0;
        let mut g = Governor::new(escape_ladder(4, false));
        run(&mut g, 8.0, budget, 20);
        assert!(g.rung > 0);
        g.release();
        assert_eq!(g.rung, 0);
        assert!(g.measured.iter().any(Option::is_some));
    }

    /// Phase 2's 60 fps: a window something else holds to every other
    /// refresh of a 120 Hz display, whatever it draws. Pictures over the
    /// share shed, the interval does not move, and the shed is undone; the
    /// governor stays near the top instead of walking down the ladder.
    #[test]
    fn a_shed_that_does_not_help_is_undone() {
        let budget = 1000.0 / 120.0;
        let held = Pace { rate: 60.0, p75_ms: 1000.0 / 60.0 };
        let mut g = Governor::new(escape_ladder(2, false));
        let mut seen = Vec::new();
        for _ in 0..600 {
            let ms = 1.6 * g.rung().units() + 0.5;
            g.observe(ms, budget, 1.0, held);
            seen.push(g.rung);
        }
        assert!(seen[300..].iter().all(|&r| r <= 1), "it stayed near the top: {:?}", &seen[290..310]);
        assert!(g.no_shed_below.is_some(), "and knows why");
    }

    /// A cost that is mostly fixed: 12 ms whatever the picture's size, and
    /// 4 a unit. No rung fits the 11.7 ms share of 60 Hz; 50% (13 ms) fits
    /// the 13.3 at which frames still come evenly, and 75% (14.25) does not.
    /// The model learns the fixed part on the first shed and settles at
    /// 50%, the most it can keep and still make the frame rate, not at 25%.
    /// (A mode-D solid is not such a picture: at 1079x1056 it took 13 frames
    /// and ~200 ms at full size, one frame and 12.7 ms at 25%, its pixels'
    /// share.)
    #[test]
    fn a_mostly_fixed_cost_is_not_shed_for() {
        let budget = 1000.0 / 60.0;
        let mut g = Governor::new(escape_ladder(1, true));
        let mut seen = Vec::new();
        for _ in 0..200 {
            let ms = 12.0 + 4.0 * g.rung().units();
            g.observe(ms, budget, 1.0, pace(ms, budget));
            seen.push(g.rung());
        }
        let last = *seen.last().unwrap();
        assert_eq!(last.scale, 0.5, "{seen:?}");
        assert!(seen[100..].iter().all(|r| *r == last), "settled");
        assert!(g.fixed() > 2.0, "the fixed part was learned: {}", g.fixed());
    }

    /// The flame batch's cap is the room the rest of the frame leaves in
    /// the share, whatever the frames dispatched; when the rest alone
    /// overruns the share, the dispatch keeps a fifth of it.
    #[test]
    fn a_batch_cap_is_the_room_the_rest_of_the_frame_leaves() {
        use crate::gpu::timer::FrameTiming;
        let budget = 1000.0 / 60.0;
        let mut c = BatchCap::default();
        assert_eq!(c.cap(budget), None, "nothing timed");
        c.dispatch_timed(8.0, 8e6); // 1 ms a million thread-iterations
        for (f, n) in [(0, 8e6), (1, 4e6), (2, 8e6), (3, 2e6)] {
            c.dispatched(f, n);
            // 4 ms of the UI and the tail besides the dispatch.
            c.frames_timed(&[FrameTiming { frame: f, ui_ms: 1.0, engine_ms: 3.0 + n * 1e-6 }]);
        }
        let cap = c.cap(budget).unwrap();
        assert!((cap - (GPU_SHARE * budget - 4.0) * 1e6).abs() < 1.0, "{cap}");

        let mut c = BatchCap::default();
        c.dispatch_timed(1.0, 1e6);
        for f in 0..4 {
            c.dispatched(f, 1e6);
            c.frames_timed(&[FrameTiming { frame: f, ui_ms: 1.0, engine_ms: 14.0 }]);
        }
        let cap = c.cap(budget).unwrap();
        assert!((cap - 0.2 * 14.0 * 1e6).abs() < 1.0, "{cap}");
    }

    /// Under vsync every frame on time reads as on budget, so the batch
    /// governor grows the batch until the GPU is full. The cap holds it
    /// where the frame's span fits the share, with the GPU's times three
    /// frames late, and holds it there without hunting.
    #[test]
    fn the_batch_settles_at_its_cap_with_late_timings() {
        use crate::gpu::timer::FrameTiming;
        let budget = 1000.0 / 60.0;
        let full = 128.0 * 64.0 * 3600.0;
        let b = 0.55e-6; // ms a thread-iteration: the full batch ~16 ms
        let rest = 3.0;
        let mut c = BatchCap::default();
        let (mut scale, mut seen) = (0.1_f64, Vec::new());
        for f in 0..300u64 {
            let before = scale;
            scale = (scale * 1.15).min(1.0);
            if let Some(cap) = c.cap(budget) {
                scale = capped_scale(scale, before, (cap / full).max(1.0 / 256.0));
            }
            let n = scale * full;
            c.dispatched(f, n);
            if f % 3 == 0 {
                c.dispatch_timed(b * n, n);
            }
            if f >= 3 {
                let g = f - 3;
                let span = rest + b * seen[g as usize] * full;
                c.frames_timed(&[FrameTiming { frame: g, ui_ms: 0.5, engine_ms: span - 0.5 }]);
            }
            seen.push(scale);
        }
        let want = (GPU_SHARE * budget - rest) / b / full;
        assert!(want < 1.0);
        assert!(seen[100..].iter().all(|s| (s - want).abs() < 1e-9), "{want}: {:?}", &seen[95..105]);
    }

    /// Without a GPU timer the walker's position counts up from the
    /// cheapest rung: its ceiling, the ladder's length, is the best.
    #[test]
    fn the_fallback_walks_the_ladder_best_at_the_top() {
        let ladder = escape_ladder(4, false);
        let n = ladder.len() as u32;
        assert_eq!(fallback_rung(&ladder, n), ladder[0], "the top is the best");
        assert_eq!(fallback_rung(&ladder, 1), *ladder.last().unwrap(), "1 is the cheapest");
        assert_eq!(fallback_rung(&ladder, n + 3), ladder[0], "clamped");
        assert_eq!(fallback_rung(&ladder, 0), *ladder.last().unwrap(), "clamped");
        let rungs: Vec<Rung> = (1..=n).rev().map(|p| fallback_rung(&ladder, p)).collect();
        assert_eq!(rungs, ladder, "every rung, in order");
    }

    /// Over the cap the batch comes down at the governor's slew, not at once.
    #[test]
    fn a_batch_over_its_cap_comes_down_at_the_slew() {
        assert_eq!(capped_scale(0.5, 0.5, 0.8), 0.5, "under the cap: untouched");
        assert_eq!(capped_scale(0.92, 0.8, 0.85), 0.85, "a growth stops at the cap");
        assert!((capped_scale(1.0, 1.0, 0.2) - 1.0 / 1.15).abs() < 1e-12, "a fall at the slew");
        assert_eq!(capped_scale(0.1, 1.0, 0.2), 0.1, "a shed below the cap stands");
    }
}

/// The grid scales a governed simulation picks from, best first (the
/// plan's *Simulations are different too*); a run's ceiling is its
/// config's scale, and 12.5% is the grid's own clamp (D5).
pub(crate) const SIM_SCALES: [f32; 7] = [1.0, 0.75, 0.5, 0.375, 0.25, 0.1875, 0.125];
/// A step's cost below which a smaller grid saves nothing: phase 1
/// measured a fixed overhead of about 0.1 ms a step taking over below
/// 18.75-37.5% of 1080p.
const SIM_STEP_FLOOR_MS: f64 = 0.1;
/// A held pick is far wrong when the steps take over this many frame
/// budgets (frames under half the target rate) ...
const SIM_TOO_SLOW: f64 = 2.0;
/// ... or under this share of the GPU share with a better scale left.
const SIM_ROOM: f64 = 0.25;
/// Frames a pick must be far wrong in a row, with no edit, before it is
/// picked again: each pick resamples the field, a visible change.
const SIM_FAR_FRAMES: u32 = 120;
/// Timed batches on a grid before its cost is taken: their median. The
/// first after a new grid or kernel reads high (the pipeline's creation
/// and the seed in flight), and picked from it a run held 50% where 75%
/// fit (2026-10-30).
const SIM_SAMPLES: usize = 4;
/// The batches a grid's median is of, at most.
const SIM_SAMPLES_KEPT: usize = 16;

/// The bound grid scale at which `steps` steps a frame fit `allowance_ms`,
/// from a step's cost `ms_now` on the grid at `scale_now`: the best of
/// `ceiling` and the [`SIM_SCALES`] below it predicted to fit, cells going
/// with the scale squared down to [`SIM_STEP_FLOOR_MS`] a step. When none
/// fits, the best within `NO_FIT_SLACK` of the cheapest: playback then
/// paces itself, as it always has (the step rate is the content's speed,
/// and is not cut).
pub(crate) fn sim_pick(ceiling: f32, scale_now: f32, ms_now: f64, steps: f64, allowance_ms: f64) -> f32 {
    let mut options = vec![ceiling];
    options.extend(SIM_SCALES.iter().copied().filter(|&s| s < ceiling - 1e-4));
    let cost = |s: f32| {
        let r = s as f64 / scale_now.max(1e-3) as f64;
        steps.max(0.0) * (ms_now * r * r).max(SIM_STEP_FLOOR_MS)
    };
    if let Some(&s) = options.iter().find(|&&s| cost(s) <= allowance_ms) {
        return s;
    }
    let cheapest = options.iter().map(|&s| cost(s)).fold(f64::INFINITY, f64::min);
    options.into_iter().find(|&s| cost(s) <= NO_FIT_SLACK * cheapest).unwrap_or(ceiling)
}

/// What a simulation's step cost is of: the model, its layers and
/// parameters, not the seed, the init, the colouring or the grid.
pub(crate) fn sim_cost_identity(cfg: &crate::config::sim::SimConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut v = serde_json::to_value(cfg).unwrap_or_default();
    if let Some(o) = v.as_object_mut() {
        for k in [
            "seed", "init", "steps", "steps_per_frame", "coloring", "coloring_params", "color_layers", "view", "matte",
            "upscale", "downscale", "fit", "terrain", "grid",
        ] {
            o.remove(k);
        }
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.to_string().hash(&mut h);
    h.finish()
}

/// A simulation's governor (D6): a bound grid's scale, picked when a run
/// starts from the measured cost of a step and held for the run, picked
/// again only when far wrong; let go on stop. The cost a cell-step is
/// remembered for the model, so the next run starts on its pick.
#[derive(Debug, Default)]
pub(crate) struct SimGovernor {
    /// The grid scale the run holds, once picked.
    pub scale: Option<f32>,
    /// The config's scale it was picked under.
    ceiling: f32,
    /// ms a cell-step, for the model it was measured on.
    cost: Option<(u64, f64)>,
    /// The batches timed on the current grid (ms a step, raw), the grid's
    /// cells and model, and the renderer's count of batches last seen.
    samples: std::collections::VecDeque<f64>,
    sampled: (u64, f64),
    batches: u64,
    /// The steps a frame the run asks for, once known.
    pub steps: Option<f64>,
    /// The timeline's step rate (steps a second, smoothed) and its last
    /// playhead and step count.
    rate: Option<f64>,
    last: Option<(f64, u32)>,
    far: u32,
    /// The timeline walk's budget a frame while governed, ms.
    pub walk_ms: Option<f64>,
}

impl SimGovernor {
    /// The renderer's last timed batch: its count of batches, the batch's
    /// ms a step, on a grid of `cells`, for `identity`. A new count is a
    /// sample; once `SIM_SAMPLES` are in on a grid, their median is the
    /// cost a cell-step, kept across grids and runs.
    pub fn measured(&mut self, identity: u64, batches: u64, ms: f64, cells: f64) {
        if batches == self.batches || !(ms > 0.0) || !(cells > 0.0) {
            return;
        }
        self.batches = batches;
        if self.sampled != (identity, cells) {
            self.sampled = (identity, cells);
            self.samples.clear();
        }
        self.samples.push_back(ms);
        while self.samples.len() > SIM_SAMPLES_KEPT {
            self.samples.pop_front();
        }
        if self.samples.len() >= SIM_SAMPLES {
            let mut v: Vec<f64> = self.samples.iter().copied().collect();
            v.sort_by(f64::total_cmp);
            self.cost = Some((identity, v[v.len() / 2] / cells));
        }
    }

    /// A step's cost on a grid of `cells`, for `identity`, if measured.
    pub fn step_ms(&self, identity: u64, cells: f64) -> Option<f64> {
        self.cost.filter(|(id, _)| *id == identity).map(|(_, c)| (c * cells).max(SIM_STEP_FLOOR_MS))
    }

    /// The timeline's playhead `t` (s) and step count this frame: the
    /// steps a frame it asks for at `budget_ms` a frame.
    pub fn timeline(&mut self, t: f64, steps: u32, budget_ms: f64) {
        if let Some((t0, s0)) = self.last {
            if t > t0 && steps >= s0 {
                let r = (steps - s0) as f64 / (t - t0);
                self.rate = Some(self.rate.map_or(r, |old| old + 0.1 * (r - old)));
            }
        }
        self.last = Some((t, steps));
        self.steps = self.rate.map(|r| r * budget_ms / 1000.0);
    }

    /// One frame of an active run, on a grid of `cells_now` cells at the
    /// scale it holds (or the config's, `ceiling`, before a pick): the
    /// scale to hold, and whether it changed. Nothing before a step has
    /// been measured or the steps a frame are known.
    pub fn frame(&mut self, identity: u64, ceiling: f32, cells_now: f64, allowance_ms: f64, budget_ms: f64, editing: bool) -> Option<(f32, bool)> {
        let scale_now = self.scale.unwrap_or(ceiling);
        let ms_now = self.step_ms(identity, cells_now)?;
        let steps = self.steps?;
        // A run's first frame, or the config's scale edited: picked at once.
        if self.scale.is_none() || (ceiling - self.ceiling).abs() > 1e-6 {
            let pick = sim_pick(ceiling, scale_now, ms_now, steps, allowance_ms);
            self.ceiling = ceiling;
            self.far = 0;
            let changed = (pick - scale_now).abs() > 1e-6;
            self.scale = Some(pick);
            return Some((pick, changed));
        }
        // Held, unless far wrong for a while with nothing being edited.
        let now = steps * ms_now;
        let lowest = SIM_SCALES[SIM_SCALES.len() - 1].min(ceiling);
        let slow = now > SIM_TOO_SLOW * budget_ms && scale_now > lowest + 1e-6;
        let roomy = now < SIM_ROOM * allowance_ms && scale_now < ceiling - 1e-6;
        self.far = if (slow || roomy) && !editing { self.far + 1 } else { 0 };
        if self.far >= SIM_FAR_FRAMES {
            self.far = 0;
            let pick = sim_pick(ceiling, scale_now, ms_now, steps, allowance_ms);
            if (pick - scale_now).abs() > 1e-6 {
                self.scale = Some(pick);
                return Some((pick, true));
            }
        }
        Some((scale_now, false))
    }

    /// The run stopped: the config's scale again. True if the held one
    /// was another, so the field is carried back. The cost is kept.
    pub fn release(&mut self, ceiling: f32) -> bool {
        let held = self.scale.take();
        self.steps = None;
        self.rate = None;
        self.last = None;
        self.far = 0;
        self.walk_ms = None;
        held.is_some_and(|s| (s - ceiling).abs() > 1e-6)
    }
}

/// A settled escape picture, waiting for its frames' GPU times: the frames
/// it took, first and last, whether it was governed, and whether it is an
/// escape terrain's (a frame that drew a moving view's lit pass) rather
/// than escape-time's.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Picture {
    pub first: u64,
    pub last: u64,
    pub governed: bool,
    pub terrain: bool,
}

/// Frames a picture waits for its GPU times before it is dropped.
const PICTURE_WAIT: u64 = 16;
/// Recent frames' engine times kept, by frame.
const FRAMES_KEPT: usize = 64;

/// What the app keeps for the governor between frames.
pub(crate) struct GovernorState {
    /// Escape-time's ladder and rung.
    pub escape: Governor,
    /// Whether escape-time was governed last frame.
    pub escape_on: bool,
    /// An escape terrain's ladder and rung, whether it was governed last
    /// frame, and the display size its cost was learned at.
    pub terrain: Governor,
    pub terrain_on: bool,
    pub terrain_display: (u32, u32),
    /// The render scale the governor asks for (app/render_scale.rs), 1
    /// when nothing is governed.
    pub scale: f32,
    /// The frame the escape picture in progress began at.
    pub picture_from: Option<u64>,
    /// Settled pictures waiting for their frames' GPU times.
    pub pictures: std::collections::VecDeque<Picture>,
    /// Recent frames' engine GPU times, ms.
    pub frames: std::collections::VecDeque<(u64, f64)>,
    /// The CPU's own time a frame, ms, smoothed.
    pub cpu_ms: f64,
    /// The display size the cost was learned at.
    pub display: (u32, u32),
    /// The last picture observed: its GPU ms and the frames it took.
    pub last_picture: Option<(f64, u64)>,
    /// The last frames' intervals, ms.
    pub intervals: std::collections::VecDeque<f64>,
    /// The last frames' GPU spans (the UI's and the engine's), ms.
    pub spans: std::collections::VecDeque<f64>,
    /// Frames held back by something other than this app's work: the
    /// budget they allow, ms (`effective_budget`).
    pub capped: Option<f64>,
    /// A flame's render scale while governed (`flame_scale`), 1 otherwise;
    /// the samples its last frame plotted; their smoothed share a display
    /// pixel; and the frames a different scale has been wanted.
    pub flame_scale: f32,
    pub flame_samples: Option<u64>,
    pub flame_spp: f64,
    pub flame_hold: u32,
    /// The flame batch's pacing cap, and the batch scale it held the last
    /// governed frame to (the frame log's), if it did.
    pub batch_cap: BatchCap,
    pub flame_cap: Option<f64>,
    /// A simulation's grid scale (`SimGovernor`).
    pub sim: SimGovernor,
    /// While escape-time is governed without a GPU timer
    /// (`fallback_rung`): the ladder's length, the walker's ceiling, and
    /// the budget its settled frames are judged against, ms.
    pub fallback: Option<(u32, f64)>,
}

impl Default for GovernorState {
    fn default() -> Self {
        GovernorState {
            escape: Governor::new(escape_ladder(1, false)),
            escape_on: false,
            terrain: Governor::new(terrain_ladder()),
            terrain_on: false,
            terrain_display: (0, 0),
            scale: 1.0,
            picture_from: None,
            pictures: Default::default(),
            frames: Default::default(),
            cpu_ms: 0.0,
            display: (0, 0),
            last_picture: None,
            intervals: Default::default(),
            spans: Default::default(),
            capped: None,
            flame_scale: 1.0,
            flame_samples: None,
            flame_spp: 0.0,
            flame_hold: 0,
            batch_cap: BatchCap::default(),
            flame_cap: None,
            sim: SimGovernor::default(),
            fallback: None,
        }
    }
}

impl GovernorState {
    /// The frame's interval since the last, ms.
    pub fn frame_interval(&mut self, ms: f64) {
        if ms.is_finite() && ms > 0.0 {
            self.intervals.push_back(ms);
            while self.intervals.len() > 32 {
                self.intervals.pop_front();
            }
        }
    }

    /// The budget the frames can be held to: the refresh's (`refresh_ms`),
    /// unless something other than this app's work holds them back. That
    /// is taken to be so when they come two or more refreshes apart while
    /// the GPU's span and the CPU's own time take under `CAP_SHARE` of the
    /// interval -- the app could go at least three times as fast -- and it
    /// lasts until they come faster again. The budget is then the interval
    /// they come at, in whole refreshes. Measured 2026-10-30: a window held
    /// to 60 fps on a 120 Hz display, whatever it drew; budgeted to the
    /// refresh, the governor shed for a rate it could not have.
    pub fn effective_budget(&mut self, refresh_ms: f64) -> f64 {
        const CAP_SHARE: f64 = 0.35;
        if self.intervals.len() < 32 || !(refresh_ms > 0.0) {
            return self.capped.unwrap_or(refresh_ms);
        }
        let median = |d: &std::collections::VecDeque<f64>, q: f64| {
            let mut v: Vec<f64> = d.iter().copied().collect();
            v.sort_by(f64::total_cmp);
            v.get(((v.len().max(1) - 1) as f64 * q).round() as usize).copied().unwrap_or(0.0)
        };
        let interval = median(&self.intervals, 0.5);
        let span = median(&self.spans, 0.75);
        match self.capped {
            Some(_) if interval < 1.3 * refresh_ms => self.capped = None,
            None if interval >= 1.8 * refresh_ms
                && span <= CAP_SHARE * interval
                && self.cpu_ms <= CAP_SHARE * interval =>
            {
                self.capped = Some((interval / refresh_ms).round() * refresh_ms);
            }
            _ => {}
        }
        self.capped.unwrap_or(refresh_ms)
    }

    /// How the last frames came: their rate (from the mean interval with
    /// the slowest and fastest tenth left out, so a hiccup does not move it
    /// and frames alternating long and short still average right) and the
    /// 75th percentile of their intervals.
    pub fn pace(&self) -> Pace {
        let mut v: Vec<f64> = self.intervals.iter().copied().collect();
        if v.is_empty() {
            return Pace { rate: 0.0, p75_ms: 0.0 };
        }
        v.sort_by(f64::total_cmp);
        let cut = v.len() / 10;
        let kept = &v[cut..v.len() - cut];
        let mean = kept.iter().sum::<f64>() / kept.len().max(1) as f64;
        Pace { rate: 1000.0 / mean.max(1e-3), p75_ms: v[((v.len() - 1) as f64 * 0.75).round() as usize] }
    }

    /// The frames' GPU times just heard back; then every waiting picture
    /// whose frames are all in is observed, against the budget the frames
    /// can be held to (`effective_budget` of the refresh's, `budget_ms`),
    /// and those that will not be are dropped. True if escape-time's rung
    /// changed.
    pub fn take_timings(&mut self, timings: &[crate::gpu::timer::FrameTiming], frame: u64, budget_ms: f64) -> bool {
        for t in timings {
            self.frames.push_back((t.frame, t.engine_ms));
            self.spans.push_back(t.ui_ms + t.engine_ms);
        }
        while self.spans.len() > 32 {
            self.spans.pop_front();
        }
        self.batch_cap.frames_timed(timings);
        let budget_ms = self.effective_budget(budget_ms);
        while self.frames.len() > FRAMES_KEPT {
            self.frames.pop_front();
        }
        let mut changed = false;
        while let Some(p) = self.pictures.front().copied() {
            let ms: Option<f64> = (p.first..=p.last)
                .map(|f| self.frames.iter().find(|(g, _)| *g == f).map(|(_, ms)| *ms))
                .sum();
            match ms {
                Some(ms) => {
                    self.pictures.pop_front();
                    self.last_picture = Some((ms, p.last - p.first + 1));
                    let pace = self.pace();
                    if p.governed && p.terrain && self.terrain_on {
                        changed |= self.terrain.observe(ms, budget_ms, self.cpu_ms, pace);
                    } else if p.governed && !p.terrain && self.escape_on {
                        changed |= self.escape.observe(ms, budget_ms, self.cpu_ms, pace);
                    }
                }
                None if frame.saturating_sub(p.last) > PICTURE_WAIT => {
                    self.pictures.pop_front();
                }
                None => break,
            }
        }
        changed
    }
}

impl super::App {
    /// A flame's render scale for the next frame (`flame_scale`): while it
    /// plays or is being edited (the overwrite window), with the frame
    /// governor on and Focused Rendering off, from the samples the last
    /// frame plotted; full size otherwise, and in every other mode.
    pub(super) fn govern_flame(&mut self) {
        use crate::scene::transforms::RenderMode;
        let c = self.config_manager.active_config();
        let three_d = c.render_mode == RenderMode::ThreeD;
        let on = matches!(c.render_mode, RenderMode::TwoD | RenderMode::ThreeD)
            && !c.cylinder_targeting
            && self.config_manager.system_settings().frame_governor
            && (self.animation_controller.is_playing() || self.use_overwrite_next_frame);
        let display_px = self.fractal_viewport_size.0.max(1) as f64 * self.fractal_viewport_size.1.max(1) as f64;
        let ceiling = self.user_render_scale();
        let g = &mut self.governor;
        let samples = g.flame_samples.take();
        if !on {
            g.flame_scale = 1.0;
            g.flame_spp = 0.0;
            g.flame_hold = 0;
            return;
        }
        if let Some(n) = samples {
            let spp = n as f64 / display_px;
            g.flame_spp = if g.flame_spp > 0.0 { g.flame_spp + 0.1 * (spp - g.flame_spp) } else { spp };
        }
        if g.flame_spp <= 0.0 {
            return;
        }
        let want = flame_scale(g.flame_spp, three_d).min(ceiling);
        if (want - g.flame_scale).abs() < 1e-6 {
            g.flame_hold = 0;
        } else {
            g.flame_hold += 1;
            if g.flame_hold >= FLAME_HOLD {
                g.flame_scale = want;
                g.flame_hold = 0;
            }
        }
    }

    /// A simulation's grid scale this frame (`SimGovernor`, D6): while a
    /// bound grid's run is active (the transport running, or the timeline
    /// driving its steps), with the governor on, picked as the run starts
    /// from the measured cost of a step, held, and picked again only when
    /// far wrong; the config's scale again when it stops, the field carried
    /// back. Never during a background export, which has the GPU. Also
    /// *Fit to target*'s scale, for the Simulation panel.
    #[cfg(feature = "engine-sim")]
    pub(super) fn govern_sim(&mut self) {
        use crate::config::sim::SimGrid;
        if self.export_status.lock().map(|s| s.active).unwrap_or(false) {
            return;
        }
        let c = self.config_manager.active_config();
        let identity = sim_cost_identity(&c.sim);
        let bound = match c.sim.grid {
            SimGrid::Viewport { scale } => Some(scale),
            SimGrid::Fixed { .. } => None,
        };
        let timeline = self.timeline_owns_sim();
        let on = c.render_mode == crate::scene::transforms::RenderMode::Simulation
            && (self.sim_running || timeline)
            && self.config_manager.system_settings().frame_governor;
        let editing = self.sim_last_edit.is_some_and(|t| t.elapsed() < super::ESCAPE_INTERACTION_WINDOW);
        let (steps_per_frame, steps_now) = (c.sim.steps_per_frame, c.sim.steps);
        let t = self.animation_controller.current_time;
        let refresh = self.governor_budget_ms();
        let budget = self.governor.capped.unwrap_or(refresh);
        let allowance = GPU_SHARE * budget;
        let grid = self.sim_renderer.as_ref().map(|s| {
            let (w, h) = s.grid_size();
            (w as f64 * h as f64, s.last_step())
        });
        let g = &mut self.governor.sim;
        if let Some((cells, (batches, ms))) = grid {
            g.measured(identity, batches, ms, cells);
        }
        match bound.filter(|_| on) {
            None => {
                if g.release(bound.unwrap_or(1.0)) {
                    self.sim_resample = true;
                }
            }
            Some(ceiling) => {
                if timeline {
                    g.timeline(t, steps_now, budget);
                } else {
                    g.steps = Some(steps_per_frame as f64);
                }
                g.walk_ms = Some(allowance);
                if let Some((cells, _)) = grid {
                    if let Some((_, true)) = g.frame(identity, ceiling, cells, allowance, budget, editing) {
                        self.sim_resample = true;
                    }
                }
            }
        }
        // Fit to target: the best scale for Steps per Frame at the target,
        // from 100% (or the config's, when above) down, as measured.
        let g = &self.governor.sim;
        let fit = bound.zip(grid).and_then(|(config, (cells, _))| {
            let ms = g.step_ms(identity, cells)?;
            let steps = g.steps.unwrap_or(steps_per_frame as f64);
            Some(sim_pick(config.max(1.0), g.scale.unwrap_or(config), ms, steps, allowance))
        });
        let held = g.scale.filter(|s| bound.is_some_and(|b| (s - b).abs() > 1e-6));
        self.egui_layer.set_sim_governor(crate::ui::SimGovernorUi { held, fit });
    }

    /// An escape terrain's render scale this frame (`terrain_ladder`), or
    /// None when it is not governed: while an animation plays or the view
    /// is being edited (a camera gesture or fly mode, as any escape edit),
    /// in the Lit or Auto tier, with the governor on and a GPU that can
    /// time its frames. Path traced is not: the tier is the look, and the
    /// path tracer fills its frame's time at any size. The ceiling is the
    /// Rendering menu's render scale.
    pub(super) fn govern_terrain(&mut self, user_scale: f32) -> Option<f32> {
        let playing = self.animation_controller.is_playing();
        let editing = self.escape_last_edit.is_some_and(|t| t.elapsed() < super::ESCAPE_INTERACTION_WINDOW);
        let on = (playing || editing)
            && self.config_manager.system_settings().frame_governor
            && self.frame_timer.is_some();
        let refresh = self.governor_budget_ms();
        let g = &mut self.governor;
        let budget = g.capped.unwrap_or(refresh);
        if !on {
            self.govern_terrain_off();
            self.governor.scale = 1.0;
            return None;
        }
        let mut ladder: Vec<Rung> = terrain_ladder()
            .into_iter()
            .map(|r| Rung { scale: r.scale.min(user_scale), ..r })
            .collect();
        ladder.dedup();
        g.terrain.set_ladder(ladder);
        let display = self.fractal_viewport_size;
        if g.terrain_display != (0, 0) && display != g.terrain_display {
            let px = |s: (u32, u32)| s.0.max(1) as f64 * s.1.max(1) as f64;
            g.terrain.scale_cost(px(display) / px(g.terrain_display));
            g.terrain.resized(budget);
        }
        g.terrain_display = display;
        if !g.terrain_on {
            g.terrain.resized(budget);
            g.terrain_on = true;
        }
        g.scale = g.terrain.rung().scale;
        Some(g.scale)
    }

    /// No terrain governed this frame: it lets go (the render scale is
    /// whoever else's is governed, or 1).
    pub(super) fn govern_terrain_off(&mut self) {
        let g = &mut self.governor;
        if g.terrain_on {
            g.terrain.release();
        }
        g.terrain_on = false;
    }

    /// Nothing governed this frame (another mode, a terrain, a traced
    /// solid): escape-time lets go.
    pub(super) fn govern_escape_off(&mut self) {
        let g = &mut self.governor;
        if g.escape_on {
            g.escape.release();
        }
        g.escape_on = false;
        g.scale = 1.0;
        g.picture_from = None;
    }

    /// The governor's frame budget, ms (`TargetRate`): never shorter than
    /// the display's own frame.
    pub(super) fn governor_budget_ms(&self) -> f64 {
        use crate::storage::settings::TargetRate;
        let s = self.config_manager.system_settings();
        let display = super::frame_budget(self.vsync_enabled(), s.target_fps, self.display_refresh_mhz) * 1000.0;
        let fps = match self.target_rate() {
            TargetRate::Display => return display,
            TargetRate::Animation => self.egui_layer.export_fps() as f64,
            TargetRate::Fixed(f) => f as f64,
        };
        (1000.0 / fps.max(1.0)).max(display)
    }

    /// Escape-time's rung this frame (supersampling, render scale), or None
    /// when it is not governed: while an animation plays, or a 2D picture
    /// is being edited (a solid has its own preview then), with the frame
    /// governor on. Updates the ladder, predicts at once on entering, and
    /// lets go on leaving. Without a GPU timer the ladder is walked by
    /// dropped frames instead (`fallback_rung`), in playback alone: the
    /// walker reads settled playback frames.
    pub(super) fn govern_escape(&mut self, config_ss: u32, solid: bool, user_scale: f32) -> Option<Rung> {
        let playing = self.animation_controller.is_playing();
        let editing = !solid && self.escape_last_edit.is_some_and(|t| t.elapsed() < super::ESCAPE_INTERACTION_WINDOW);
        let timed = self.frame_timer.is_some();
        let on = (playing || (editing && timed)) && self.config_manager.system_settings().frame_governor;
        let refresh = self.governor_budget_ms();
        let g = &mut self.governor;
        let budget = g.capped.unwrap_or(refresh);
        if !on {
            if g.escape_on {
                g.escape.release();
            }
            g.escape_on = false;
            g.scale = 1.0;
            g.fallback = None;
            return None;
        }
        // The ceiling is the settings': the config's supersampling, and the
        // render scale chosen in the Rendering menu.
        let mut ladder: Vec<Rung> = escape_ladder(config_ss.max(1), solid)
            .into_iter()
            .map(|r| Rung { supersample: r.supersample, scale: r.scale.min(user_scale) })
            .collect();
        ladder.dedup();
        if !timed {
            // Judged against the target's budget, never the one frames
            // held back elsewhere allow (`effective_budget`): with no GPU
            // span to tell the two apart, a GPU too slow for the target
            // would read as frames held back, and nothing would shed.
            let steps = ladder.len() as u32;
            let rung = fallback_rung(&ladder, self.escape_aa.factor(steps));
            g.scale = rung.scale;
            g.fallback = Some((steps, refresh));
            return Some(rung);
        }
        g.fallback = None;
        g.escape.set_ladder(ladder);
        let display = self.fractal_viewport_size;
        if g.display != (0, 0) && display != g.display {
            let px = |s: (u32, u32)| s.0.max(1) as f64 * s.1.max(1) as f64;
            g.escape.scale_cost(px(display) / px(g.display));
            g.escape.resized(budget);
        }
        g.display = display;
        if !g.escape_on {
            // Entering: straight to the rung the remembered cost predicts.
            g.escape.resized(budget);
            g.escape_on = true;
        }
        let rung = g.escape.rung();
        g.scale = rung.scale;
        Some(rung)
    }
}
