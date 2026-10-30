//! **The quality governor** (docs/projects/quality-governor.md, order of
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
//! is a bad trade. It probes UP a
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
/// Room a higher rung must have, predicted, before it is probed.
const PROBE_ROOM: f64 = 0.8;
/// Pictures with that room before a probe, at first; doubled after each
/// failed one, up to `PROBE_WAIT_MAX`.
const PROBE_WAIT: u32 = 30;
const PROBE_WAIT_MAX: u32 = 480;
/// Pictures over the share, in a row, before shedding; one far over (twice
/// the budget) sheds at once.
const OVER_RUN: u32 = 2;
/// Pictures after a change that are not its cost: the resize, the new
/// pipelines, the first chunk sizes.
pub(crate) const SETTLE: u32 = 2;
/// How fast a rung's measured cost follows a new picture's.
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

/// One engine's governor: its ladder, the rung in use, and what it has
/// learned of the cost.
#[derive(Debug, Clone)]
pub(crate) struct Governor {
    ladder: Vec<Rung>,
    rung: usize,
    /// Each rung's measured GPU ms, smoothed, and the picture it was last
    /// measured at.
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
    over: u32,
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
            over: 0,
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
        self.over = 0;
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
        self.over = 0;
        self.room = 0;
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
        let ms = match self.fresh(self.rung) {
            Some(c) => c + COST_EMA * (picture_ms - c),
            None => picture_ms,
        };
        self.measured[self.rung] = Some((ms, self.pictures));
        self.learn_fixed();
        let target = GPU_SHARE * budget_ms;
        // A frame the CPU holds up gains nothing from a cheaper picture.
        if cpu_ms >= target {
            self.over = 0;
            self.room = 0;
            return false;
        }

        if picture_ms > target {
            self.room = 0;
            // A probe that does not fit goes back at once, and waits
            // longer before the next.
            if let Some(from) = self.probing_from.take() {
                self.rung = from;
                self.probe_wait = (self.probe_wait * 2).min(PROBE_WAIT_MAX);
                self.changed();
                return true;
            }
            self.over += 1;
            if self.over >= OVER_RUN || picture_ms > 2.0 * budget_ms {
                // The rung the model now asks for: lower if one fits, or
                // higher if none does and a better one costs about as
                // little as the cheapest.
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
            }
            return false;
        }
        self.over = 0;
        // Fits: a probe that held is the new rung.
        if self.probing_from.take().is_some() {
            self.probe_wait = PROBE_WAIT;
        }
        if self.rung == 0 {
            return false;
        }
        let up = self.rung - 1;
        if self.predict(up).is_some_and(|c| c <= PROBE_ROOM * target) {
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
        run(&mut g, 4.0, budget, 10);
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
}

/// A settled escape picture, waiting for its frames' GPU times: the frames
/// it took, first and last, and whether it was governed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Picture {
    pub first: u64,
    pub last: u64,
    pub governed: bool,
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
}

impl Default for GovernorState {
    fn default() -> Self {
        GovernorState {
            escape: Governor::new(escape_ladder(1, false)),
            escape_on: false,
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

    /// How the last frames came: their rate and the 75th percentile of
    /// their intervals.
    pub fn pace(&self) -> Pace {
        let mut v: Vec<f64> = self.intervals.iter().copied().collect();
        if v.is_empty() {
            return Pace { rate: 0.0, p75_ms: 0.0 };
        }
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        v.sort_by(f64::total_cmp);
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
                    if p.governed && self.escape_on {
                        let pace = self.pace();
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
        let display = super::frame_budget(s.vsync_enabled, s.target_fps, self.display_refresh_mhz) * 1000.0;
        let fps = match s.target_rate {
            TargetRate::Display => return display,
            TargetRate::Animation => self.egui_layer.export_fps() as f64,
            TargetRate::Fixed(f) => f as f64,
        };
        (1000.0 / fps.max(1.0)).max(display)
    }

    /// Escape-time's rung this frame (supersampling, render scale), or None
    /// when it is not governed: while an animation plays, or a 2D picture
    /// is being edited (a solid has its own preview then), with the frame
    /// governor on and a GPU that can time its frames. Updates the ladder,
    /// predicts at once on entering, and lets go on leaving.
    pub(super) fn govern_escape(&mut self, config_ss: u32, solid: bool, user_scale: f32) -> Option<Rung> {
        let playing = self.animation_controller.is_playing();
        let editing = !solid && self.escape_last_edit.is_some_and(|t| t.elapsed() < super::ESCAPE_INTERACTION_WINDOW);
        let on = (playing || editing)
            && self.config_manager.system_settings().frame_governor
            && self.frame_timer.is_some();
        let refresh = self.governor_budget_ms();
        let g = &mut self.governor;
        let budget = g.capped.unwrap_or(refresh);
        if !on {
            if g.escape_on {
                g.escape.release();
            }
            g.escape_on = false;
            g.scale = 1.0;
            return None;
        }
        // The ceiling is the settings': the config's supersampling, and the
        // render scale chosen in the Rendering menu.
        let mut ladder: Vec<Rung> = escape_ladder(config_ss.max(1), solid)
            .into_iter()
            .map(|r| Rung { supersample: r.supersample, scale: r.scale.min(user_scale) })
            .collect();
        ladder.dedup();
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
