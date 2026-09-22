//! Cylinder planning by the inverse walk.
//!
//! `docs/projects/inversive-targeting.md` §24–§26. The forward planner
//! in [`cylinder`](super::cylinder) pushes a disc through a word's
//! maps and asks whether it meets the view; for a flame like
//! `grand-julian` that cannot work, and the reason is structural
//! rather than a matter of tightness. Its dominant map is
//! `z^(-1/2)`, which EXPANDS over the part of the plane the attractor
//! occupies, and a ball bound must take the worst derivative over the
//! ball -- so it cannot contract until the ball is already small and
//! cannot get small without contracting. Measured: the bound for the
//! measure-typical word sat at 0.2 for depths 3--5 while the true
//! cylinder was 1e-5, then died.
//!
//! The same flame, walked BACKWARDS, is the ideal case. Every map is
//! `julian ∘ rotation`, whose inverse is single-valued; for a given
//! output point only one arm of each transform can have produced it,
//! since the arms' images are disjoint sectors. Walked from eleven
//! orbit points to depth 24, the word whose cylinder contains a point
//! was UNIQUE at every depth.
//!
//! **What a region is.** A word's region is the set of view points
//! whose backward path follows it -- equivalently, the set of points
//! `x` with `S_w(x)` in the view. Everything here is about
//! representing that set well enough to find its children, and the
//! versions that failed all failed there:
//!
//! - A centre point and a Jacobian: by depth five the region had
//!   outgrown first order and its centre sat outside `t1`'s ring while
//!   66% of the measure passed through it.
//! - A cloud of view points pulled back: covers the region by AREA,
//!   and the measure lives on ring sectors of area 5e-3 inside regions
//!   of area 200. Jitter, boundary refinement and a wider beam moved
//!   nothing; the same three branches stayed missing.
//! - A generic point pulled back under a map that did not produce it
//!   is junk -- through `z^(-8)` it lands astronomically far away --
//!   and junk made a child's spread 3.4e9, blinded the cut, and got
//!   real branches pruned uncharged.
//!
//! **What works is the attractor sample, indexed.** The sample is
//! μ-distributed, so a region that holds sample points holds them
//! where the measure is, ring sectors included; and a sample point is
//! on the attractor exactly, so nothing about it is junk. The cost of
//! finding a region's sample points is the problem -- replaying the
//! whole sample along a word is the sample times the depth, per node
//! -- and the answer is an inverted index built once per flame: every
//! sample point landed through every symbol and filed by grid cell.
//! A child's candidates are then a union of index lists over the
//! cells its parent's points occupy, and only a capped handful of
//! candidates per child is verified exactly by replay. The view cloud
//! remains for one job: the descent from a view too small to hold any
//! sample point to a region that does.

use super::cylinder::{sym_arm, sym_of, sym_transform, Cylinder, Cylinders, NoCylinders, View, MAX_DEPTH, MAX_WORDS};
use super::ifs_analysis::{analyse_2d, Ifs2, IfsMap, Kernel, Map2};
use super::transforms::Flame;
use crate::variations::VariationRegistry;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The most of a level's probability the beam may drop.
///
/// **A beam is a mass budget, not a node count.** Ranked by per-word
/// probability and cut at 256 nodes, the walk dropped a quarter of a
/// view's measure and `lost` said so while the completeness test
/// named the words: `t1`'s fifteen arms split its 12.5% into fifteen
/// words of 0.8%, every one outranked by a `t0` word at 37.5%, and
/// the beam shed them a few at a time. Keeping nodes until what
/// remains is below this fraction of the level's total bounds `lost`
/// per level by construction.
pub const BEAM_LOSS: f64 = 1e-3;

/// The hard cap on the frontier, for cost.
pub const BEAM: usize = 2048;

/// How many points of the attractor are sampled, once per flame. The
/// walk's resolution is how many of them a region holds, so this is
/// large; it is built once and cached.
pub const SAMPLE: usize = 100_000;

/// The grid the sample is indexed on, in cells across the attractor's
/// diameter.
///
/// **Fine, because the regions it serves are small.** At 128 cells
/// the cell was 0.49 across and a region seeded at spread 8.7e-3
/// covered three parts in ten thousand of its cell, so a cell's
/// candidates were nearly all outside it and every child came back
/// empty. At 1024 the cell is a twentieth of that, and it is also
/// what makes "within a cell of a sample point" mean on the
/// attractor: the attractor's inner radius is 0.17, and a junk point
/// pulled back to the origin is no longer within a cell of anything.
pub const GRID_CELLS: usize = 1024;

/// How many sample points a cut is verified against.
pub const VERIFY: usize = 400;

/// A node whose children, between them, account for less than this
/// fraction of the node's own share of the view is forced ITSELF, and
/// its children are discarded.
///
/// **This is what makes the plan complete by construction.** Every
/// earlier failure was a child that was never formed -- starved of
/// points, pruned wrongly, cut off by a cap -- and each one was a hole
/// in the picture that no count could see. But the invariance equation
/// is exact: a word's share of the view is `Σ_a p_a` times its
/// children's shares, over EVERY symbol. So the children found can be
/// checked against their parent, and when they come up short the
/// parent is forced instead. A shallower word is less efficient --
/// more of its forced samples land outside the frame and are discarded
/// -- but it draws everything beneath it, including whatever the walk
/// failed to find. Holes become waste.
pub const COMPLETE_ENOUGH: f64 = 0.9;

/// The fewest of a node's own region points its check must rest on.
///
/// **The check counts points, it does not compare estimates.** The
/// first version compared the node's replay share against the sum of
/// its children's, two 400-sample estimates each ~15% noisy against a
/// 0.9 threshold; a false trigger deep in the tree forced its whole
/// ancestry, and at the saved view it forced a depth-1 word holding 37%
/// of the attractor -- complete, and untargeted. But every region point
/// records the child that produced it, so the question "do the
/// surviving children cover this node" is a count over the node's own
/// points, exact for the sample.
pub const CHECK_POINTS: usize = 40;

/// The fraction of replayed sample points that must land in the
/// frame for a word to be cut there rather than walked deeper. Below
/// it the cylinder spills outside the view and forcing it wastes the
/// spill; a symbol deeper, less.
pub const CUT_EFFICIENCY: f64 = 0.9;

/// A view-cloud point farther than this many extents from the
/// attractor's centre cannot be on it and is dropped where it stands.
pub const JUNK_EXTENTS: f64 = 2.0;

/// A node whose probability is below this fraction of the mass
/// already kept is dropped and charged to `lost`. Nothing below it
/// can change the picture, and the bound on what is charged is
/// `BEAM × MAX_DEPTH × MEASURE_FLOOR` of the kept mass.
pub const MEASURE_FLOOR: f64 = 1e-4;

/// A word that reached the depth cap without fitting is kept only if
/// this fraction of its replayed samples land in the frame.
pub const LAST_EFFICIENCY: f64 = 0.2;

/// The fewest sample points that turn a cloud region into an indexed
/// one, and how many candidates from the cloud's cells are tested to
/// find them. The seeding test is the whole word's replay per
/// candidate, so this is the one place the cloud phase is expensive;
/// it is also the one place being stingy costs the whole plan, since
/// a region never seeded is walked by the cloud to the depth cap.
pub const MIN_SEED: usize = 16;
pub const SEED_CANDIDATES: usize = 4096;

/// How many of a node's points contribute their cells to a child's
/// candidate search. A strided subset of the points is still
/// distributed by measure, and the cells it names are the ones the
/// region's measure lives in.
pub const CELLS_FROM: usize = 1024;

/// The most candidates one child takes from the index, and how many
/// it takes from each cell. Round-robin across the cells, so the
/// candidates cover the region rather than whichever cell sorts
/// first -- taken first-come, the survivors clustered in one corner
/// and the child's own cells named nothing else.
pub const CAND_CAP: usize = 1024;
pub const PER_CELL: usize = 2;

/// A carried child holding fewer points than this is searched again,
/// wider, before it is expanded.
///
/// **A thin node loses its children silently.** A child whose share of
/// its parent is below one part in the parent's point count has no
/// point to be found by, and under `CHECK_POINTS` the completeness check
/// does not judge the node at all. Measured at a 1e3 view: a node with
/// efficiency 0.01 -- about a thousand of the sample's points lie in its
/// region -- was carried with FIFTEEN, because the capped search spread
/// its 1024 candidates over the parent's whole region; one of its
/// children formed, and 1.7% of the view was missing beneath it. The
/// points exist; the second search finds them.
pub const TOPUP_BELOW: usize = 64;

/// How many candidates the second search may take.
pub const TOPUP_CAP: usize = 16384;

/// How many candidates are tested exactly before deciding whether the
/// rest need testing at all. A deep word's region covers whole cells,
/// so when the first probe all land the rest are taken as they are.
/// A probe that finds nothing decides nothing: at an 8% hit rate
/// eight misses happen half the time, and reading that as absence
/// dropped 60% of a view's measure under one word.
pub const PROBE: usize = 8;

/// A node with at least this many points names its cells without
/// their neighbours; fewer, and the neighbours are included so a
/// thinly represented region is not cut off at its cell walls.
pub const NEIGHBOURS_BELOW: usize = 24;

/// How long a plan may take before the walk stops and FORCES what is
/// still on its frontier -- complete, less efficient.
///
/// A safety net rather than a limit, now that the app plans on a
/// background thread: it was 1.5 s while planning froze the UI, and a
/// plan cut short there came out complete but wasteful. The caller can
/// ask for a shorter one ([`PlanOptions`]) where it must block.
pub const TIME_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);

/// What a caller can ask of one plan beyond the view.
#[derive(Clone, Copy)]
pub struct PlanOptions<'a> {
    /// See [`TIME_BUDGET`].
    pub budget: std::time::Duration,
    /// Set by the caller when the view has moved on and this plan will
    /// never be used: the walk stops at its next check and the result
    /// is meaningless. Checked once per node.
    pub cancel: Option<&'a AtomicBool>,
}

impl Default for PlanOptions<'_> {
    fn default() -> Self {
        Self { budget: TIME_BUDGET, cancel: None }
    }
}

/// Why children left the walk, counted.
#[derive(Debug, Default, Clone)]
pub struct Trace {
    pub no_preimage: usize,
    pub no_arm: usize,
    pub pruned: usize,
    pub floor: usize,
    pub beam: usize,
    pub cut: usize,
    pub not_yet: usize,
    /// Cloud regions that became indexed regions.
    pub seeded: usize,
    /// Index children whose candidates all failed the exact test.
    pub empty: usize,
    /// Index children with no candidates in the parent's cells.
    pub nocand: usize,
    /// Nodes forced as they stood: incomplete children, off the
    /// beam, or out of time.
    pub forced: usize,
    /// Where the time went, for profiling: seeding a cloud region from
    /// the grid, gathering candidates, checking them exactly, and
    /// replaying words for their efficiency. Plus how many word
    /// evaluations each did (one per sample point per call).
    pub t_seed: std::time::Duration,
    pub t_gather: std::time::Duration,
    pub t_verify: std::time::Duration,
    pub t_replay: std::time::Duration,
    pub n_verify: usize,
    pub n_replay: usize,
    pub nodes_expanded: usize,
    /// Thin children searched a second time. See `TOPUP_BELOW`.
    pub topped_up: usize,
    /// When set, every expanded node's word is recorded in `expanded`
    /// -- what a cache keyed by word would have held.
    pub record_expanded: bool,
    pub expanded: Vec<Vec<u32>>,
    /// A word suffix to follow: every child whose word ends with it
    /// reports what became of it.
    pub watch: Option<Vec<u32>>,
    pub watched: Vec<String>,
}

type Cell = (i32, i32);

/// Sample indices filed by cell: one flat sorted list, ranged by
/// binary search. A hash map of a million small vectors was most of
/// the build time and memory; this is thirty megabytes for the whole
/// alphabet.
struct Index {
    entries: Vec<(Cell, u32)>,
}

impl Index {
    fn build(mut entries: Vec<(Cell, u32)>) -> Self {
        entries.sort_unstable();
        Self { entries }
    }

    /// The span of `entries` filed under `cell`.
    fn bounds(&self, cell: Cell) -> (usize, usize) {
        let lo = self.entries.partition_point(|e| e.0 < cell);
        let hi = lo + self.entries[lo..].partition_point(|e| e.0 <= cell);
        (lo, hi)
    }
}

/// One symbol of the alphabet: a transform and, for a many-valued
/// variation, the arm.
struct Sym {
    sym: u32,
    map: usize,
    arm: u32,
    /// The probability the chaos game draws this transform AND this
    /// arm.
    prob: f64,
}

/// What a region is made of: view points pulled back (the descent from
/// a small view), or sample indices (exact).
enum Pts {
    Cloud(Vec<[f64; 2]>),
    Index(Vec<u32>),
}

/// One word on the walk's frontier.
struct Node {
    word: Vec<u32>,
    pts: Pts,
    prob: f64,
    /// The fraction of the attractor this word sends into the view,
    /// from its replay: `prob × eff` is what it holds of the view's
    /// measure, and that is what the beam ranks.
    eff: f64,
}

/// What expanding one node produced. See `Backward::expand`.
#[derive(Default)]
struct Expanded {
    kept: Vec<(Cylinder, f64)>,
    next: Vec<Node>,
    lost: f64,
    trace: Trace,
}

/// The trace line for `word`, if it is under the watched suffix.
fn watch_line(
    watch: Option<&[u32]>,
    word: &[u32],
    depth: usize,
    what: &str,
    detail: impl FnOnce() -> String,
) -> Option<String> {
    let w = watch?;
    if word.len() < w.len() || word[word.len() - w.len()..] != w[..] {
        return None;
    }
    let syms: Vec<String> = word.iter().map(|s| format!("t{}a{}", sym_transform(*s), sym_arm(*s))).collect();
    Some(format!("depth {depth:>2} {what:<8} [{}] {}", syms.join(" "), detail()))
}

fn spread_of(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len() as f64;
    if n == 0.0 {
        return 0.0;
    }
    let c = [pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n];
    pts.iter().map(|p| (p[0] - c[0]).hypot(p[1] - c[1])).fold(0.0, f64::max)
}

impl Trace {
    /// Add another trace's counts and lines to this one.
    fn absorb(&mut self, o: Trace) {
        self.no_preimage += o.no_preimage;
        self.no_arm += o.no_arm;
        self.pruned += o.pruned;
        self.floor += o.floor;
        self.beam += o.beam;
        self.cut += o.cut;
        self.not_yet += o.not_yet;
        self.seeded += o.seeded;
        self.empty += o.empty;
        self.nocand += o.nocand;
        self.forced += o.forced;
        self.t_seed += o.t_seed;
        self.t_gather += o.t_gather;
        self.t_verify += o.t_verify;
        self.t_replay += o.t_replay;
        self.n_verify += o.n_verify;
        self.n_replay += o.n_replay;
        self.nodes_expanded += o.nodes_expanded;
        self.topped_up += o.topped_up;
        self.expanded.extend(o.expanded);
        self.watched.extend(o.watched);
    }
}

/// The attractor, sampled and indexed, with the maps to walk it.
pub struct Backward {
    ifs: Ifs2,
    /// One entry per distinct transform.
    transforms: Vec<TransformInfo>,
    alphabet: Vec<Sym>,
    sample: Vec<[f64; 2]>,
    /// The alphabet index of the symbol that produced each sample point
    /// from the one before it, or `u32::MAX` after a reseed.
    ///
    /// **The sample is one orbit, so every point's past is recorded.**
    /// A region's sample points therefore split among the region's
    /// children EXACTLY: point `i` lies in child `a` if symbol `a`
    /// produced it, and its predecessor `i - 1` is then a point of that
    /// child's region. The split is in proportion to the children's
    /// shares, so every child holding more than one part in the
    /// region's point count is found by construction -- where searching
    /// the landing index by cell found them only when the cells were
    /// right.
    made_by: Vec<u32>,
    centre: [f64; 2],
    extent: f64,
    cell: f64,
    /// Sample indices by the cell the point lies in.
    grid: Index,
    /// Per alphabet entry: sample indices by the cell the point LANDS
    /// in under that symbol's map. A child's candidates are the union
    /// of these over the parent's cells.
    landing: Vec<Index>,
}

struct TransformInfo {
    index: usize,
    weight: f64,
    arms: u32,
    map: usize,
}

/// How many arms a map's FORWARD has -- a root's `|n|`; everything
/// else is single-valued going forward, whatever its inverse's branch
/// count.
fn forward_arms(map: &IfsMap<Map2>) -> u32 {
    let root = |k: &Kernel| match *k {
        Kernel::Root { n, .. } => n.unsigned_abs().max(1),
        _ => 1,
    };
    match &map.forward {
        Map2::Nonlinear(n) => root(&n.kernel),
        Map2::Sum(s) => root(&s.kernel),
        _ => 1,
    }
}

/// The forward map along arm `k`.
fn forward(map: &IfsMap<Map2>, x: [f64; 2], k: u32) -> [f64; 2] {
    match &map.forward {
        Map2::Nonlinear(n) => n.apply_branch(x, k),
        Map2::Sum(s) => s.apply_branch(x, k),
        other => other.apply(x),
    }
}

fn finite(p: [f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

/// The last flame analysed, shared across threads: the app plans on a
/// background thread, and a per-thread cache there would rebuild the
/// index on every plan.
static CACHE: Mutex<Option<(u64, Arc<Backward>)>> = Mutex::new(None);

impl Backward {
    /// The analysis for `flame`, built once and reused while the
    /// flame's transforms do not change. The index is a quarter of a
    /// second to build and depends on nothing but the flame, and
    /// `plan` runs on every pan.
    pub fn cached(flame: &Flame, registry: &VariationRegistry) -> Result<Arc<Self>, String> {
        let key = Self::flame_key(flame);
        if let Some(b) = CACHE
            .lock()
            .ok()
            .and_then(|c| c.as_ref().filter(|(k, _)| *k == key).map(|(_, b)| b.clone()))
        {
            return Ok(b);
        }
        // Built outside the lock: a second thread asking for the same
        // flame meanwhile builds its own rather than waiting, which
        // costs a duplicate build once and never a deadlock.
        let b = Arc::new(Self::read(flame, registry)?);
        if let Ok(mut c) = CACHE.lock() {
            *c = Some((key, b.clone()));
        }
        Ok(b)
    }

    /// What the analysis depends on: the transforms, and nothing else --
    /// with zero-weight variations left out. The app's reload plans
    /// against the sticky-adopted flame, which carries retained
    /// variations at weight zero, and its per-frame sync against the raw
    /// one; keyed on both, this cache held one and rebuilt the other,
    /// alternately, at 400 ms a time.
    pub fn flame_key(flame: &Flame) -> u64 {
        use std::hash::{Hash, Hasher};
        let live: Vec<_> = flame
            .transforms
            .iter()
            .map(|t| {
                let mut t = t.clone();
                t.variations.retain(|_, w| *w != 0.0);
                let names: Vec<String> = t.variations.keys().cloned().collect();
                t.variation_params
                    .retain(|k, _| names.iter().any(|n| k.split('.').next() == Some(n.as_str())));
                t
            })
            .collect();
        let json = serde_json::to_string(&live).unwrap_or_default();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        json.hash(&mut h);
        h.finish()
    }

    /// Analyse the flame, sample its attractor and index it. `Err`
    /// names what the inverse walk cannot do for this flame.
    pub fn read(flame: &Flame, registry: &VariationRegistry) -> Result<Self, String> {
        let ifs = analyse_2d(flame, registry).map_err(|errs| {
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
        })?;
        if ifs.final_map.is_some() {
            return Err("a final transform is not pulled back yet".into());
        }
        if ifs.maps.is_empty() {
            return Err("no maps".into());
        }
        let mut transforms: Vec<TransformInfo> = Vec::new();
        for (mi, m) in ifs.maps.iter().enumerate() {
            if transforms.iter().any(|t| t.index == m.transform_index) {
                continue;
            }
            let weight = flame
                .transforms
                .get(m.transform_index)
                .map_or(0.0, |t| (t.weight as f64).max(0.0));
            transforms.push(TransformInfo { index: m.transform_index, weight, arms: forward_arms(m), map: mi });
        }
        let total: f64 = transforms.iter().map(|t| t.weight).sum();
        if !(total > 0.0) {
            return Err("no weight".into());
        }
        if transforms.iter().all(|t| t.arms == 1) {
            return Err("no many-valued variation; the forward planner is exact here".into());
        }
        let mut alphabet: Vec<Sym> = Vec::new();
        for t in &transforms {
            if !(t.weight > 0.0) {
                continue;
            }
            for arm in 0..t.arms {
                alphabet.push(Sym {
                    sym: sym_of(t.index as u32, arm),
                    map: t.map,
                    arm,
                    prob: t.weight / total / t.arms as f64,
                });
            }
        }

        // The attractor, by the chaos game on the analysed maps. A
        // fixed stream, so two plans of one flame agree.
        let mut st = 0x9E37_79B9_7F4A_7C15u64;
        let mut lcg = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut x = [0.31f64, 0.17];
        let burn = SAMPLE / 20;
        let mut sample = Vec::with_capacity(SAMPLE);
        let mut made_by: Vec<u32> = Vec::with_capacity(SAMPLE);
        let mut reseeded = true;
        for k in 0..SAMPLE + burn {
            let mut u = lcg() * total;
            let mut t = &transforms[transforms.len() - 1];
            for c in &transforms {
                if u < c.weight {
                    t = c;
                    break;
                }
                u -= c.weight;
            }
            let arm = ((lcg() * t.arms as f64) as u32).min(t.arms - 1);
            let y = forward(&ifs.maps[t.map], x, arm);
            if finite(y) && y[0].abs() < 1e12 && y[1].abs() < 1e12 {
                x = y;
            } else {
                x = [0.31, 0.17];
                reseeded = true;
                continue;
            }
            if k >= burn {
                let ai = alphabet
                    .iter()
                    .position(|a| a.sym == sym_of(t.index as u32, arm))
                    .map_or(u32::MAX, |i| i as u32);
                // The first point kept, and the first after a reseed,
                // have no recorded predecessor in the sample.
                made_by.push(if reseeded || sample.is_empty() { u32::MAX } else { ai });
                sample.push(x);
            }
            reseeded = false;
        }
        if sample.len() < 1000 {
            return Err("the orbit did not settle".into());
        }
        let n = sample.len() as f64;
        let centre = [sample.iter().map(|p| p[0]).sum::<f64>() / n, sample.iter().map(|p| p[1]).sum::<f64>() / n];
        let extent = sample
            .iter()
            .map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2)).sqrt())
            .fold(0.0, f64::max);
        if !(extent > 0.0) || !extent.is_finite() {
            return Err("the attractor has no extent".into());
        }
        let cell = 2.0 * extent / GRID_CELLS as f64;
        let key = |p: [f64; 2]| -> Cell { ((p[0] / cell).floor() as i32, (p[1] / cell).floor() as i32) };
        let grid = Index::build(sample.iter().enumerate().map(|(i, p)| (key(*p), i as u32)).collect());
        // **The index.** Every sample point landed through every
        // symbol, filed by the cell it lands in.
        let mut landing: Vec<Index> = Vec::with_capacity(alphabet.len());
        for a in &alphabet {
            let mut entries: Vec<(Cell, u32)> = Vec::with_capacity(sample.len());
            for (i, p) in sample.iter().enumerate() {
                let y = forward(&ifs.maps[a.map], *p, a.arm);
                if finite(y) && y[0].abs() < 1e12 && y[1].abs() < 1e12 {
                    entries.push((key(y), i as u32));
                }
            }
            landing.push(Index::build(entries));
        }
        Ok(Self { ifs, transforms, alphabet, sample, made_by, centre, extent, cell, grid, landing })
    }

    /// How far the sample reaches from its centre.
    pub fn extent(&self) -> f64 {
        self.extent
    }

    /// A point of the sampled attractor, `frac` of the way through
    /// the orbit -- somewhere a view can be centred that is on the
    /// set.
    pub fn sample_point(&self, frac: f64) -> [f64; 2] {
        let i = ((self.sample.len() as f64 * frac.clamp(0.0, 1.0)) as usize).min(self.sample.len() - 1);
        self.sample[i]
    }

    fn cell_of(&self, p: [f64; 2]) -> Cell {
        ((p[0] / self.cell).floor() as i32, (p[1] / self.cell).floor() as i32)
    }

    /// Whether `p` lies where symbol `ai`'s map sends the attractor:
    /// some sample point lands within a cell of it under that map.
    ///
    /// **This is the cloud phase's test for a real child**, and it
    /// asks the right question where "is the pulled-back point near a
    /// sample point" asked the wrong one. A point at |p| = 0.86 is not
    /// in `t1`'s ring, and nothing lands near it under `t1`, so the
    /// child through `t1` is refused -- where its pulled-back point
    /// at 3e-10 used to pass an absolute nearness test against the
    /// attractor's inner edge. And a true child in a sparse part of
    /// the attractor is kept, where the absolute test threw it away
    /// for want of a sample point within 0.06 of it.
    fn near_landing(&self, ai: usize, p: [f64; 2]) -> bool {
        let (cx, cy) = self.cell_of(p);
        let idx = &self.landing[ai];
        for dx in -1..=1 {
            for dy in -1..=1 {
                let c = (cx + dx, cy + dy);
                let lo = idx.entries.partition_point(|e| e.0 < c);
                if lo < idx.entries.len() && idx.entries[lo].0 == c {
                    return true;
                }
            }
        }
        false
    }

    /// Which arm of `map` sends `x` to `p`: read off the output's
    /// angle for a root and confirmed by one forward evaluation, with
    /// a scan of every arm as the fallback.
    fn arm_of(&self, map: &IfsMap<Map2>, x: [f64; 2], p: [f64; 2]) -> Option<u32> {
        let arms = forward_arms(map);
        let scale = 1.0 + p[0].abs().max(p[1].abs());
        if arms == 1 {
            let y = forward(map, x, 0);
            let d = (y[0] - p[0]).hypot(y[1] - p[1]);
            return (d <= 1e-6 * scale).then_some(0);
        }
        if let Map2::Nonlinear(nl) = &map.forward {
            if let Kernel::Root { n, .. } = nl.kernel {
                let v = nl.post_inv.apply(p);
                let v = [v[0] / nl.w, v[1] / nl.w];
                let phi = v[1].atan2(v[0]);
                let nf = n as f64;
                let k = (nf * phi / std::f64::consts::TAU).round().rem_euclid(nf.abs()) as u32;
                let y = forward(map, x, k);
                if (y[0] - p[0]).hypot(y[1] - p[1]) <= 1e-6 * scale {
                    return Some(k);
                }
            }
        }
        let mut best = None;
        let mut best_d = f64::INFINITY;
        for k in 0..arms {
            let y = forward(map, x, k);
            if !finite(y) {
                continue;
            }
            let d = (y[0] - p[0]).hypot(y[1] - p[1]);
            if d < best_d {
                best_d = d;
                best = Some(k);
            }
        }
        (best_d <= 1e-6 * scale).then_some(best?)
    }

    /// The forward image of `x` along `word`, or `None` where a map
    /// sends it to infinity.
    fn forward_along(&self, word: &[u32], x: [f64; 2]) -> Option<[f64; 2]> {
        let mut p = x;
        for &sym in word {
            let t = sym_transform(sym) as usize;
            let arm = sym_arm(sym);
            let info = self.transforms.iter().find(|i| i.index == t)?;
            p = forward(&self.ifs.maps[info.map], p, arm);
            if !finite(p) {
                return None;
            }
        }
        Some(p)
    }

    /// Whether `x`'s forward image along `word` lands in the view.
    fn lands(&self, word: &[u32], x: [f64; 2], view: View) -> bool {
        match self.forward_along(word, x) {
            Some(y) => (y[0] - view.centre[0]).hypot(y[1] - view.centre[1]) <= view.radius,
            None => false,
        }
    }

    /// Replay a word forward on the verification sample: the
    /// fraction landing in the view, and where the landed points sit.
    fn replay(&self, word: &[u32], view: View) -> (f64, [f64; 2], f64) {
        let stride = (self.sample.len() / VERIFY).max(1);
        let mut hit = 0usize;
        let mut total = 0usize;
        let mut sum = [0.0f64; 2];
        let mut landed: Vec<[f64; 2]> = Vec::new();
        for p0 in self.sample.iter().step_by(stride) {
            total += 1;
            if let Some(p) = self.forward_along(word, *p0) {
                if (p[0] - view.centre[0]).hypot(p[1] - view.centre[1]) <= view.radius {
                    hit += 1;
                    sum[0] += p[0];
                    sum[1] += p[1];
                    landed.push(p);
                }
            }
        }
        if hit == 0 {
            return (0.0, view.centre, view.radius);
        }
        let c = [sum[0] / hit as f64, sum[1] / hit as f64];
        let r = landed.iter().map(|p| (p[0] - c[0]).hypot(p[1] - c[1])).fold(0.0, f64::max);
        (hit as f64 / total.max(1) as f64, c, r)
    }

    /// Up to `cap` sample indices filed under `cells` (and their
    /// neighbours, if asked) in `index`. The lists are disjoint across
    /// cells -- an index lands in exactly one cell per symbol -- so
    /// nothing needs deduplicating, and the search stops as soon as it
    /// has enough.
    fn gather(index: &Index, cells: &[Cell], neighbours: bool, _per_cell: usize, cap: usize) -> Vec<u32> {
        // **By measure, across the whole region.** Every cell's list is
        // a μ-distributed sample, so the concatenation of all of them is
        // the region's measure, and taking every k-th entry of it is a
        // fair sample that neither favours the front of the cell order
        // nor spends as much on an empty cell as on a dense one. Both
        // mistakes were made and measured: filling the cap from the
        // first cells took the region's leftmost strip (70% complete at
        // a view straddling an arm boundary), and two points per cell
        // sampled by AREA and starved the ring the measure lives on
        // (42%). Counting a cell is two binary searches, so the whole
        // region is counted before anything is taken.
        let reach: i32 = if neighbours { 1 } else { 0 };
        let mut seen: Vec<Cell> = Vec::with_capacity(cells.len() * 9);
        for &(cx, cy) in cells {
            for dx in -reach..=reach {
                for dy in -reach..=reach {
                    seen.push((cx + dx, cy + dy));
                }
            }
        }
        seen.sort_unstable();
        seen.dedup();
        let ranges: Vec<(usize, usize)> = seen.iter().map(|&c| index.bounds(c)).filter(|r| r.1 > r.0).collect();
        let total: usize = ranges.iter().map(|r| r.1 - r.0).sum();
        let step = total.div_ceil(cap.max(1)).max(1);
        let mut out: Vec<u32> = Vec::with_capacity(total.min(cap));
        let mut k = 0usize;
        for (lo, hi) in ranges {
            // Resume the global stride inside this cell.
            let first = (step - k % step) % step;
            let mut i = lo + first;
            while i < hi {
                out.push(index.entries[i].1);
                i += step;
            }
            k += hi - lo;
        }
        out
    }

    /// Expand one node: find its children, replay each, keep the ones
    /// that fit the view, carry the rest -- or force the node itself when
    /// its children do not account for it. Reads only `self`; everything
    /// it decides is returned, so nodes can be expanded in parallel.
    fn expand(&self, mut node: Node, depth: usize, view: View, floor_mass: f64, watch: Option<&[u32]>, record: bool) -> Expanded {
        let mut out = Expanded::default();
        let tr = &mut out.trace;
        let watched = |tr: &mut Trace, word: &[u32], what: &str, detail: &dyn Fn() -> String| {
            if let Some(line) = watch_line(watch, word, depth, what, detail) {
                tr.watched.push(line);
            }
        };
        let junk_r = JUNK_EXTENTS * self.extent;
        let is_junk = |q: [f64; 2]| (q[0] - self.centre[0]).hypot(q[1] - self.centre[1]) > junk_r;
        let points_of = |pts: &Pts| -> Vec<[f64; 2]> {
            match pts {
                Pts::Cloud(c) => c.clone(),
                Pts::Index(idx) => idx.iter().map(|&i| self.sample[i as usize]).collect(),
            }
        };

        // **Seed a cloud region from the index** the moment sample
        // points lie in it: the candidates are the sample points in the
        // cells the cloud occupies, verified exactly.
        let t_seed = std::time::Instant::now();
        tr.nodes_expanded += 1;
        if record {
            tr.expanded.push(node.word.clone());
        }
        if let Pts::Cloud(cloud) = &node.pts {
            let mut cells: Vec<Cell> = cloud.iter().map(|p| self.cell_of(*p)).collect();
            cells.sort_unstable();
            cells.dedup();
            let cands = Self::gather(&self.grid, &cells, true, 16, SEED_CANDIDATES);
            let hits: Vec<u32> = cands.into_iter().filter(|&i| self.lands(&node.word, self.sample[i as usize], view)).collect();
            if hits.len() >= MIN_SEED {
                let n = cloud.len();
                watched(tr, &node.word, "SEEDED", &|| format!("{} sample points replace {n} cloud points", hits.len()));
                node.pts = Pts::Index(hits);
                tr.seeded += 1;
            }
        }
        tr.t_seed += t_seed.elapsed();

        // **Children**: one per symbol the region's points came through.
        let t_gather = std::time::Instant::now();
        let mut children: Vec<(usize, Pts)> = Vec::new(); // (alphabet index, candidates)
        // Exact children from the orbit, by alphabet index.
        let mut from_orbit: Vec<Vec<u32>> = vec![Vec::new(); self.alphabet.len()];
        match &node.pts {
            Pts::Cloud(cloud) => {
                for (ai, a) in self.alphabet.iter().enumerate() {
                    let map = &self.ifs.maps[a.map];
                    let mut pts = Vec::new();
                    for &p in cloud {
                        // The point has to lie where this symbol's map
                        // sends the attractor, or no real path came
                        // through it.
                        if !self.near_landing(ai, p) {
                            tr.pruned += 1;
                            continue;
                        }
                        let q = map.inverse.apply(p);
                        if !finite(q) || q[0].abs() > 1e12 || q[1].abs() > 1e12 {
                            tr.no_preimage += 1;
                            continue;
                        }
                        let Some(arm) = self.arm_of(map, q, p) else {
                            tr.no_arm += 1;
                            continue;
                        };
                        if arm != a.arm || is_junk(q) {
                            continue;
                        }
                        pts.push(q);
                    }
                    if !pts.is_empty() {
                        children.push((ai, Pts::Cloud(pts)));
                    }
                }
            }
            Pts::Index(idx) => {
                for &i in idx {
                    let ai = self.made_by[i as usize];
                    if ai != u32::MAX && i > 0 {
                        from_orbit[ai as usize].push(i - 1);
                    }
                }
                let stride = (idx.len() / CELLS_FROM).max(1);
                let mut cells: Vec<Cell> = idx.iter().step_by(stride).map(|&i| self.cell_of(self.sample[i as usize])).collect();
                cells.sort_unstable();
                cells.dedup();
                let neighbours = idx.len() < NEIGHBOURS_BELOW;
                // The budget is per child: a node on one or two cells
                // still gets its full share, and a node on many gets a
                // few from each.
                let looked = cells.len() * if neighbours { 9 } else { 1 };
                let per_cell = PER_CELL.max(CAND_CAP.div_ceil(looked.max(1)));
                for (ai, _a) in self.alphabet.iter().enumerate() {
                    let cands = Self::gather(&self.landing[ai], &cells, neighbours, per_cell, CAND_CAP);
                    if cands.is_empty() && from_orbit[ai].is_empty() {
                        tr.nocand += 1;
                        continue;
                    }
                    children.push((ai, Pts::Index(cands)));
                }
            }
        }
        tr.t_gather += t_gather.elapsed();

        // Everything a node decides about its children is held here until
        // the node has been checked for completeness.
        let mut node_kept: Vec<(Cylinder, f64)> = Vec::new();
        let mut node_next: Vec<Node> = Vec::new();
        let mut node_lost = 0.0f64;
        // Which children survived -- carried or kept -- by alphabet
        // index, for the point count below.
        let mut survived = vec![false; self.alphabet.len()];
        for (ai, pts) in children {
            let orbit_hits = std::mem::take(&mut from_orbit[ai]);
            let a = &self.alphabet[ai];
            let prob = node.prob * a.prob;
            let mut word = Vec::with_capacity(node.word.len() + 1);
            word.push(a.sym);
            word.extend_from_slice(&node.word);

            // **Replay first.** Below the floor, or at the depth cap, the
            // walk stops -- and the word is FORCED if any of it lands,
            // never dropped. See `MEASURE_FLOOR`.
            let last = depth == MAX_DEPTH || prob < MEASURE_FLOOR * floor_mass;
            let t_replay = std::time::Instant::now();
            tr.n_replay += VERIFY;
            let (eff, cc, r) = self.replay(&word, view);
            tr.t_replay += t_replay.elapsed();
            // A child the walk stops at is FORCED even when its replay
            // landed nothing: zero hits in `VERIFY` samples means under
            // one part in `VERIFY`, not none. Dropping those at the floor
            // cost 1% of a view in scattered specks, while forcing them
            // costs at most their probability each -- below the floor by
            // definition, about 1% of the draws for a thousand of them.
            if !(eff > 0.0) && last {
                watched(tr, &word, "ZERO", &|| format!("prob {prob:.2e}"));
                tr.floor += 1;
            }
            let _ = &mut node_lost;
            if eff >= CUT_EFFICIENCY || last {
                // **Kept: it needs no points.** Checking a kept child's
                // candidates exactly was 65% of a plan's time, and ~90%
                // of the children checked were kept rather than carried.
                watched(tr, &word, "CUT", &|| format!("eff {eff:.2} prob {prob:.2e}"));
                survived[ai] = true;
                node_kept.push((Cylinder { word, prob, centre: cc, radius: r }, eff));
                continue;
            }

            // **Carried: now it needs its region's points**, checked
            // exactly on a capped set of candidates.
            let pts = match pts {
                Pts::Index(cands) => {
                    let n_cands = cands.len();
                    let t_verify = std::time::Instant::now();
                    tr.n_verify += cands.len();
                    let mut hits: Vec<u32> =
                        cands.into_iter().filter(|&i| self.lands(&word, self.sample[i as usize], view)).collect();
                    // Thin: search again, wider. See `TOPUP_BELOW`.
                    if hits.len() + orbit_hits.len() < TOPUP_BELOW {
                        if let Pts::Index(parent) = &node.pts {
                            let mut cells: Vec<Cell> = parent.iter().map(|&i| self.cell_of(self.sample[i as usize])).collect();
                            cells.sort_unstable();
                            cells.dedup();
                            let wide = Self::gather(&self.landing[ai], &cells, true, PER_CELL, TOPUP_CAP);
                            tr.n_verify += wide.len();
                            tr.topped_up += 1;
                            hits.extend(wide.into_iter().filter(|&i| self.lands(&word, self.sample[i as usize], view)));
                        }
                    }
                    tr.t_verify += t_verify.elapsed();
                    // The orbit's points need no check: they are in the
                    // child's region by construction.
                    hits.extend_from_slice(&orbit_hits);
                    hits.sort_unstable();
                    hits.dedup();
                    if hits.is_empty() {
                        tr.empty += 1;
                        if eff > 0.0 {
                            // It lands -- the replay says so -- but no
                            // point of its region was found to expand it
                            // from. Forced as it stands.
                            watched(tr, &word, "FORCEDCH", &|| format!("{n_cands} candidates, none land; eff {eff:.2}"));
                            survived[ai] = true;
                            node_kept.push((Cylinder { word, prob, centre: cc, radius: r }, eff));
                        } else {
                            watched(tr, &word, "EMPTY", &|| format!("{n_cands} candidates, none land"));
                        }
                        continue;
                    }
                    Pts::Index(hits)
                }
                other => other,
            };
            survived[ai] = true;
            let n = match &pts { Pts::Cloud(c) => c.len(), Pts::Index(i) => i.len() };
            let indexed = matches!(pts, Pts::Index(_));
            watched(tr, &word, "carried", &|| {
                format!("{n} pts spread {:.2e} eff {eff:.2} prob {prob:.2e} indexed {indexed}", spread_of(&points_of(&pts)))
            });
            node_next.push(Node { word, pts, prob, eff });
        }

        // **Complete, or forced.** See `COMPLETE_ENOUGH`. The share of
        // the node's own points whose producing child survived.
        let covered = match &node.pts {
            Pts::Index(idx) => {
                let mut valid = 0usize;
                let mut ok = 0usize;
                for &i in idx {
                    let ai = self.made_by[i as usize];
                    if ai == u32::MAX || i == 0 {
                        continue;
                    }
                    valid += 1;
                    if survived[ai as usize] {
                        ok += 1;
                    }
                }
                (valid >= CHECK_POINTS).then(|| ok as f64 / valid as f64)
            }
            Pts::Cloud(_) => None,
        };
        if !node.word.is_empty() && node.eff > 0.0 && covered.is_some_and(|c| c < COMPLETE_ENOUGH) {
            watched(tr, &node.word, "FORCED", &|| format!("children cover {:.3} of its points", covered.unwrap_or(0.0)));
            let (_, cc, r) = self.replay(&node.word, view);
            out.kept.push((Cylinder { word: node.word, prob: node.prob, centre: cc, radius: r }, node.eff));
            out.trace.forced += 1;
            return out;
        }
        out.trace.cut += node_kept.len();
        out.trace.not_yet += node_next.len();
        out.kept = node_kept;
        out.next = node_next;
        out.lost = node_lost;
        out
    }

    /// Plan the antichain for `view`.
    pub fn plan(&self, view: View) -> Result<Cylinders, NoCylinders> {
        self.plan_traced(view).0
    }

    /// The same, with a budget and a cancel flag.
    pub fn plan_opts(&self, view: View, opts: PlanOptions) -> Result<Cylinders, NoCylinders> {
        let mut tr = Trace::default();
        self.plan_with_opts(view, &mut tr, opts)
    }

    /// The same, with an account of every child that left the walk.
    pub fn plan_traced(&self, view: View) -> (Result<Cylinders, NoCylinders>, Trace) {
        let mut tr = Trace::default();
        let r = self.plan_with(view, &mut tr);
        (r, tr)
    }

    fn plan_with(&self, view: View, tr: &mut Trace) -> Result<Cylinders, NoCylinders> {
        self.plan_with_opts(view, tr, PlanOptions::default())
    }

    fn plan_with_opts(&self, view: View, tr: &mut Trace, opts: PlanOptions) -> Result<Cylinders, NoCylinders> {
        let cancelled = || opts.cancel.is_some_and(|c| c.load(Ordering::Relaxed));
        let watch = tr.watch.clone();
        let record = tr.record_expanded;

        let mut root_pts = vec![view.centre];
        for (ri, count) in [12usize, 20, 31].iter().enumerate() {
            let r = [0.35f64, 0.7, 1.0][ri] * view.radius;
            for k in 0..*count {
                let angle = std::f64::consts::TAU * (k as f64 + 0.37 * (ri + 1) as f64) / *count as f64;
                root_pts.push([view.centre[0] + r * angle.cos(), view.centre[1] + r * angle.sin()]);
            }
        }
        let mut frontier = vec![Node { word: Vec::new(), pts: Pts::Cloud(root_pts), prob: 1.0, eff: 0.0 }];
        let mut kept: Vec<(Cylinder, f64)> = Vec::new();
        let mut kept_mass = 0.0f64;
        let mut lost = 0.0f64;
        let started = std::time::Instant::now();

        for depth in 1..=MAX_DEPTH {
            if frontier.is_empty() {
                break;
            }
            // Cancelled: the caller has moved on and will discard this.
            if cancelled() {
                return Err(NoCylinders::ViewIsEmpty);
            }
            // Out of time: what is still on the frontier is FORCED as it
            // stands. Dropping it was a hole; forcing it is waste.
            if started.elapsed() > opts.budget {
                for n in frontier.drain(..) {
                    // Zero hits is not zero measure; forced all the same.
                    if n.word.is_empty() {
                        continue;
                    }
                    let (_, cc, r) = self.replay(&n.word, view);
                    kept.push((Cylinder { word: n.word, prob: n.prob, centre: cc, radius: r }, n.eff));
                    tr.forced += 1;
                }
                break;
            }

            // **Every node of a level is expanded in parallel.** Nodes are
            // independent -- each reads the flame's analysis and writes
            // only its own results -- and a plan was 97% node expansion
            // on one core of twelve. The floor is read as it stood at the
            // start of the level, and the results are merged in frontier
            // order, so a plan is the same however the threads ran.
            let floor_mass = kept_mass;
            let expand = |node: Node| {
                if cancelled() {
                    return Expanded::default();
                }
                self.expand(node, depth, view, floor_mass, watch.as_deref(), record)
            };
            #[cfg(not(target_arch = "wasm32"))]
            let results: Vec<Expanded> = {
                use rayon::prelude::*;
                std::mem::take(&mut frontier).into_par_iter().map(expand).collect()
            };
            #[cfg(target_arch = "wasm32")]
            let results: Vec<Expanded> = std::mem::take(&mut frontier).into_iter().map(expand).collect();
            if cancelled() {
                return Err(NoCylinders::ViewIsEmpty);
            }

            let mut next: Vec<Node> = Vec::new();
            for e in results {
                for (c, eff) in e.kept {
                    kept_mass += c.prob;
                    kept.push((c, eff));
                }
                next.extend(e.next);
                lost += e.lost;
                tr.absorb(e.trace);
            }

            if next.len() > 1 {
                // **Ranked by what a node holds of the VIEW**, not by
                // its probability. A cylinder's probability is its
                // whole measure; two nodes with the same probability
                // can hold none of the view or all of it, and ranking
                // by probability dropped, at depth seven of a 1e6
                // plan, the one node whose subtree held the view --
                // `lost` was 2.15e-8 against a kept mass of 3e-14.
                // The replay gives every node its efficiency, so the
                // budget is exact where it matters; where nothing
                // lands yet (a shallow cloud region) every node ties
                // at zero and nothing is dropped.
                let holds = |n: &Node| n.prob * n.eff;
                next.sort_by(|a, b| holds(b).partial_cmp(&holds(a)).unwrap_or(std::cmp::Ordering::Equal));
                let total: f64 = next.iter().map(holds).sum();
                let mut keep = next.len();
                let mut tail = 0.0;
                while keep > 1 && total > 0.0 {
                    let p = holds(&next[keep - 1]);
                    if tail + p > BEAM_LOSS * total {
                        break;
                    }
                    tail += p;
                    keep -= 1;
                }
                let keep = keep.min(BEAM);
                // A node whose share the replay could not resolve (zero
                // hits) has not been measured, not measured as nothing:
                // at shallow depth that is most of them. The beam ranks
                // measured nodes; unmeasured ones are always carried.
                // Ranked last and forced off the beam, they became
                // depth-1 words holding a third of the attractor.
                let mut unmeasured: Vec<Node> = Vec::new();
                let mut rest: Vec<Node> = Vec::new();
                for n in next.drain(keep..) {
                    if n.eff > 0.0 { rest.push(n) } else { unmeasured.push(n) }
                }
                next.extend(unmeasured);
                for n in rest {
                    // Off the beam: forced as it stands, not dropped.
                    if let Some(line) = watch_line(watch.as_deref(), &n.word, depth, "BEAM", || format!("prob {:.2e} eff {:.2}", n.prob, n.eff)) {
                        tr.watched.push(line);
                    }
                    let (_, cc, r) = self.replay(&n.word, view);
                    kept_mass += n.prob;
                    kept.push((Cylinder { word: n.word, prob: n.prob, centre: cc, radius: r }, n.eff));
                    tr.beam += 1;
                }
            }
            frontier = next;
        }

        // More words than the kernel's table holds: keep the ones
        // carrying the most measure and charge the rest.
        // No truncation: the kernel's word table is sized to fit and
        // binary-searched, and dropping words was a hole.
        let _ = MAX_WORDS;
        // Several preimage branches of one transform share a word; the
        // draw wants each word once.
        kept.sort_by(|a, b| a.0.word.cmp(&b.0.word));
        let mut merged: Vec<(Cylinder, f64)> = Vec::new();
        for (c, e) in kept {
            match merged.last_mut() {
                Some((m, me)) if m.word == c.word => {
                    *me = (*me + e).min(1.0);
                    m.radius = m.radius.max(c.radius);
                }
                _ => merged.push((c, e)),
            }
        }
        if merged.is_empty() {
            return Err(NoCylinders::ViewIsEmpty);
        }
        let mass: f64 = merged.iter().map(|(c, _)| c.prob).sum();
        let delivered: f64 = merged.iter().map(|(c, e)| c.prob * e).sum();
        let depth = merged.iter().map(|(c, _)| c.word.len()).max().unwrap_or(0);
        Ok(Cylinders {
            words: merged.into_iter().map(|(c, _)| c).collect(),
            mass,
            lost,
            sampling_leak: 0.0,
            efficiency: if mass > 0.0 { delivered / mass } else { 0.0 },
            depth,
            composable: false,
            view_centre: view.centre,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Does `arm_of` recover the arm the chaos game drew?**
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn arm_of_recovers_the_drawn_arm() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("output/flame-zoom/grand-julian.fflame");
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("a config");
        let b = Backward::read(&cfg.flame, reg).expect("armed");
        let total: f64 = b.transforms.iter().map(|t| t.weight).sum();
        let mut st = 0xC0FFEE_u64;
        let mut lcg = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut x = [0.31f64, 0.17];
        let mut tally: std::collections::BTreeMap<(usize, u32), [usize; 3]> = std::collections::BTreeMap::new();
        for k in 0..60_000 {
            let mut u = lcg() * total;
            let mut t = &b.transforms[b.transforms.len() - 1];
            for c in &b.transforms {
                if u < c.weight {
                    t = c;
                    break;
                }
                u -= c.weight;
            }
            let arm = ((lcg() * t.arms as f64) as u32).min(t.arms - 1);
            let map = &b.ifs.maps[t.map];
            let y = forward(map, x, arm);
            if !finite(y) || y[0].abs() > 1e12 {
                x = [0.31, 0.17];
                continue;
            }
            if k > 1000 {
                let e = tally.entry((t.index, arm)).or_insert([0; 3]);
                let q = map.inverse.apply(y);
                match b.arm_of(map, q, y) {
                    Some(a) if a == arm => e[0] += 1,
                    Some(_) => e[1] += 1,
                    None => e[2] += 1,
                }
            }
            x = y;
        }
        let mut bad = 0;
        for ((t, a), c) in &tally {
            println!("   t{t}  a{a:<2}  agree {:>6}  disagree {:>6}  none {:>6}", c[0], c[1], c[2]);
            bad += c[1] + c[2];
        }
        assert_eq!(bad, 0, "arm_of must recover every drawn arm");
    }

    /// **Is the plan COMPLETE?** `lost` counts what the beam and the
    /// floor dropped; it cannot see a branch that was never formed.
    /// This can: run the chaos game with each sample's symbol history,
    /// keep the samples that land in the view, and ask what fraction
    /// have some plan word as their last `k` symbols.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn is_the_plan_complete() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("output/flame-zoom/grand-julian.fflame");
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("a config");
        let b = Backward::read(&cfg.flame, reg).expect("armed");
        let q = b.sample_point(0.75);
        let total: f64 = b.transforms.iter().map(|t| t.weight).sum();

        for zoom in [1e2f64, 1e3, 1e4, 1e6, 1e8] {
            let view = View::of(zoom, q, 96, 96);
            let t0 = std::time::Instant::now();
            let mut tr = Trace::default();
            let t0s = |n: usize| std::iter::repeat(sym_of(0, 0)).take(n);
            tr.watch = Some(if zoom == 1e2 {
                std::iter::once(sym_of(2, 1)).chain(t0s(4)).collect()
            } else if zoom == 1e3 {
                std::iter::once(sym_of(1, 4)).chain(t0s(4)).collect()
            } else {
                Vec::new()
            });
            let plan = b.plan_with(view, &mut tr);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            println!(
                "zoom {zoom:.0e} ({ms:.0} ms) trace: no_preimage {} no_arm {} pruned {} floor {} beam {} cut {} carried {} seeded {} empty {} nocand {}",
                tr.no_preimage, tr.no_arm, tr.pruned, tr.floor, tr.beam, tr.cut, tr.not_yet, tr.seeded, tr.empty, tr.nocand
            );
            for line in tr.watched.iter().take(0) {
                println!("   watch: {line}");
            }
            let plan = match plan {
                Ok(p) => p,
                Err(e) => {
                    println!("zoom {zoom:.0e}: {e:?}");
                    continue;
                }
            };
            let mut st = 0xC0FFEE_u64;
            let mut lcg = move || {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((st >> 33) as f64) / ((1u64 << 31) as f64)
            };
            let mut x = [0.31f64, 0.17];
            let mut hist: Vec<u32> = Vec::new();
            let mut in_view = 0usize;
            let mut covered = 0usize;
            let mut missed: std::collections::HashMap<Vec<u32>, usize> = std::collections::HashMap::new();
            let longest = plan.words.iter().map(|w| w.word.len()).max().unwrap_or(1);
            let shortest = plan.words.iter().map(|w| w.word.len()).min().unwrap_or(1);
            for k in 0..40_000_000usize {
                let mut u = lcg() * total;
                let mut t = &b.transforms[b.transforms.len() - 1];
                for c in &b.transforms {
                    if u < c.weight {
                        t = c;
                        break;
                    }
                    u -= c.weight;
                }
                let arm = ((lcg() * t.arms as f64) as u32).min(t.arms - 1);
                let y = forward(&b.ifs.maps[t.map], x, arm);
                if !finite(y) || y[0].abs() > 1e12 {
                    x = [0.31, 0.17];
                    hist.clear();
                    continue;
                }
                x = y;
                hist.push(sym_of(t.index as u32, arm));
                if hist.len() > longest + 4 {
                    hist.remove(0);
                }
                if k < 1000 || (x[0] - view.centre[0]).hypot(x[1] - view.centre[1]) > view.radius {
                    continue;
                }
                in_view += 1;
                let hit = plan.words.iter().any(|w| {
                    let k = w.word.len();
                    hist.len() >= k && hist[hist.len() - k..] == w.word[..]
                });
                if hit {
                    covered += 1;
                } else {
                    let key = hist[hist.len().saturating_sub(shortest)..].to_vec();
                    *missed.entry(key).or_default() += 1;
                }
                if in_view >= 2000 {
                    break;
                }
            }
            println!(
                "zoom {zoom:.0e}: plan {} words depth {}..{}, eff {:.2}, lost {:.2e}; {in_view} CPU samples in view, {covered} in a planned cylinder = {:.3}",
                plan.words.len(), shortest, plan.depth, plan.efficiency, plan.lost, covered as f64 / in_view.max(1) as f64
            );
            let mut m: Vec<(Vec<u32>, usize)> = missed.into_iter().collect();
            m.sort_by(|a, b| b.1.cmp(&a.1));
            for (w, n) in m.iter().take(5) {
                let syms: Vec<String> = w.iter().map(|s| format!("t{}a{}", sym_transform(*s), sym_arm(*s))).collect();
                println!("   missed {n:>5}: ...{}", syms.join(" "));
            }
            if let Some(w) = &tr.watch {
                if !w.is_empty() {
                    let under: Vec<String> = plan
                        .words
                        .iter()
                        .filter(|c| c.word.len() >= w.len() && c.word[c.word.len() - w.len()..] == w[..])
                        .take(12)
                        .map(|c| {
                            let syms: Vec<String> = c.word.iter().map(|s| format!("t{}a{}", sym_transform(*s), sym_arm(*s))).collect();
                            format!("[{} p={:.1e}]", syms.join(" "), c.prob)
                        })
                        .collect();
                    println!("   planned under the watch: {}", under.join(" "));
                }
            }
        }
    }

    /// **Where a plan's time goes**, at the saved view and at a test
    /// view. `FFLAME=path` adds the saved one.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn where_a_plan_spends_its_time() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut cases: Vec<(String, crate::config::FractalConfig, f64)> = Vec::new();
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("grand-julian");
        let gj: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        for z in [1e3f64, 1e6] {
            cases.push((format!("grand-julian x{z:.0e}"), gj.clone(), z));
        }
        if let Ok(path) = std::env::var("FFLAME") {
            let c: crate::config::FractalConfig =
                serde_json::from_str(&std::fs::read_to_string(&path).expect("file")).expect("config");
            let z = c.zoom as f64;
            cases.push((path, c, z));
        }
        println!("cores: {}", std::thread::available_parallelism().map_or(0, |n| n.get()));
        for (name, cfg, zoom) in cases {
            let t0 = std::time::Instant::now();
            let b = Backward::read(&cfg.flame, reg).expect("armed");
            let read = t0.elapsed();
            let centre = if name.starts_with("grand-julian x") {
                b.sample_point(0.75)
            } else {
                [cfg.pan_x as f64, cfg.pan_y as f64]
            };
            let view = View::of(zoom, centre, 1280, 720);
            let mut tr = Trace::default();
            let t0 = std::time::Instant::now();
            let plan = b.plan_with(view, &mut tr);
            let total = t0.elapsed();
            let words = plan.as_ref().map_or(0, |p| p.words.len());
            let eff = plan.as_ref().map_or(0.0, |p| p.efficiency);
            let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3;
            let other = total.saturating_sub(tr.t_seed + tr.t_gather + tr.t_verify + tr.t_replay);
            println!(
                "== {name}: read {:.0} ms; plan {:.0} ms, {} nodes expanded, {words} words, eff {eff:.2}\n   \
                 seed {:.0} ms | gather {:.0} ms | verify {:.0} ms ({} candidate replays) | replay {:.0} ms ({} sample replays) | other {:.0} ms",
                ms(read), ms(total), tr.nodes_expanded, ms(tr.t_seed), ms(tr.t_gather), ms(tr.t_verify), tr.n_verify,
                ms(tr.t_replay), tr.n_replay, ms(other)
            );
        }
    }

    /// **What could a cache reuse?** Two measurements for the question
    /// "can planning be precomputed or cached":
    ///
    /// 1. between a view and a nearby one -- a pan, a zoom -- how many of
    ///    the second plan's expanded nodes the first had already
    ///    expanded. That is the most any cache keyed by WORD could save,
    ///    whether it is built lazily or pre-generated.
    /// 2. the attractor's box-counting dimension `D`, from the sample. A
    ///    view-independent map complete down to pieces of size `r` holds
    ///    about `(extent / r)^D` pieces at that size, which is what a
    ///    "whole fractal, every zoom" map would have to store.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_could_a_cache_reuse() {
        use std::collections::HashSet;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("grand-julian");
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        let b = Backward::read(&cfg.flame, reg).expect("armed");

        // 2. Box-counting dimension of the sample.
        println!("box counting over {} sample points, extent {:.3e}:", b.sample.len(), b.extent);
        let mut prev: Option<(f64, f64)> = None;
        for k in [16usize, 32, 64, 128, 256, 512] {
            let cell = 2.0 * b.extent / k as f64;
            let occ: HashSet<(i64, i64)> = b
                .sample
                .iter()
                .map(|p| ((p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64))
                .collect();
            let (lk, ln) = ((k as f64).ln(), (occ.len() as f64).ln());
            let slope = prev.map(|(a, bb)| (ln - bb) / (lk - a));
            println!("   {k:>4} cells across: {:>6} occupied{}", occ.len(),
                slope.map_or(String::new(), |s| format!(", local D {s:.2}")));
            prev = Some((lk, ln));
        }

        // 1. Reuse between nearby views.
        for zoom in [1e3f64, 1e6] {
            let q = b.sample_point(0.75);
            let base = View::of(zoom, q, 1280, 720);
            let run = |v: View| {
                let mut tr = Trace { record_expanded: true, ..Default::default() };
                let t0 = std::time::Instant::now();
                let p = b.plan_with(v, &mut tr).expect("a plan");
                let set: HashSet<Vec<u32>> = tr.expanded.into_iter().collect();
                (set, p.words.len(), t0.elapsed().as_secs_f64() * 1e3)
            };
            let (e0, w0, ms0) = run(base);
            println!("== zoom {zoom:.0e}: base plan {w0} words, {} nodes expanded, {ms0:.0} ms", e0.len());
            let r = base.radius;
            for (label, v) in [
                ("pan 1/4 view", View { centre: [q[0] + 0.25 * r, q[1]], radius: r }),
                ("pan 1 view", View { centre: [q[0] + 1.0 * r, q[1]], radius: r }),
                ("zoom in 1.5x", View { centre: q, radius: r / 1.5 }),
                ("zoom in 4x", View { centre: q, radius: r / 4.0 }),
                ("zoom out 2x", View { centre: q, radius: r * 2.0 }),
            ] {
                let (e1, w1, ms1) = run(v);
                let shared = e1.intersection(&e0).count();
                println!(
                    "   {label:<13} {w1:>5} words, {:>5} nodes, {ms1:>4.0} ms; {:>5.1}% of its nodes already expanded by the base plan",
                    e1.len(),
                    100.0 * shared as f64 / e1.len().max(1) as f64
                );
            }
        }
    }

    /// **The completeness check at a saved view**: `FFLAME=path`.
    #[test]
    #[ignore = "reads $FFLAME"]
    fn is_this_view_complete() {
        let Ok(path) = std::env::var("FFLAME") else { return };
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let cfg: crate::config::FractalConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("file")).expect("config");
        let b = Backward::read(&cfg.flame, reg).expect("armed");
        let total: f64 = b.transforms.iter().map(|t| t.weight).sum();
        let view = View::of(cfg.zoom as f64, [cfg.pan_x as f64, cfg.pan_y as f64], 1280, 720);
        let t0 = std::time::Instant::now();
        let mut tr = Trace::default();
        tr.watch = std::env::var("WATCH").ok().map(|w| {
            w.split(',').map(|t| { let (a, b) = t.split_once('.').unwrap(); sym_of(a.parse().unwrap(), b.parse().unwrap()) }).collect()
        });
        let plan = b.plan_with(view, &mut tr);
        for line in tr.watched.iter().take(4000) { println!("   watch: {line}"); }
        println!("view r {:.3e} at [{:.5},{:.5}], {:.0} ms, trace {:?}", view.radius, view.centre[0], view.centre[1], t0.elapsed().as_secs_f64()*1e3,
            (tr.pruned, tr.floor, tr.beam, tr.cut, tr.not_yet, tr.seeded, tr.empty, tr.nocand));
        let plan = match plan { Ok(p) => p, Err(e) => { println!("plan: {e:?}"); return; } };
        let mut st = 0xC0FFEE_u64;
        let mut lcg = move || { st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((st >> 33) as f64) / ((1u64 << 31) as f64) };
        let mut x = [0.31f64, 0.17];
        let mut hist: Vec<u32> = Vec::new();
        let longest = plan.words.iter().map(|w| w.word.len()).max().unwrap_or(1);
        let shortest = plan.words.iter().map(|w| w.word.len()).min().unwrap_or(1);
        let (mut in_view, mut covered) = (0usize, 0usize);
        let mut missed: std::collections::HashMap<Vec<u32>, usize> = Default::default();
        for k in 0..400_000_000usize {
            let mut u = lcg() * total;
            let mut t = &b.transforms[b.transforms.len() - 1];
            for c in &b.transforms { if u < c.weight { t = c; break; } u -= c.weight; }
            let arm = ((lcg() * t.arms as f64) as u32).min(t.arms - 1);
            let y = forward(&b.ifs.maps[t.map], x, arm);
            if !finite(y) || y[0].abs() > 1e12 { x = [0.31, 0.17]; hist.clear(); continue; }
            x = y;
            hist.push(sym_of(t.index as u32, arm));
            if hist.len() > longest + 4 { hist.remove(0); }
            if k < 1000 || (x[0] - view.centre[0]).hypot(x[1] - view.centre[1]) > view.radius { continue; }
            in_view += 1;
            if plan.words.iter().any(|w| { let k = w.word.len(); hist.len() >= k && hist[hist.len() - k..] == w.word[..] }) {
                covered += 1;
            } else {
                *missed.entry(hist[hist.len().saturating_sub(12)..].to_vec()).or_default() += 1;
            }
            if in_view >= 3000 { break; }
        }
        println!("plan {} words depth {}..{}, eff {:.2}, mass {:.2e}, lost {:.2e}; {in_view} in view, covered {:.3}",
            plan.words.len(), shortest, plan.depth, plan.efficiency, plan.mass, plan.lost, covered as f64 / in_view.max(1) as f64);
        let mut m: Vec<_> = missed.into_iter().collect();
        m.sort_by(|a, b| b.1.cmp(&a.1));
        for (w, n) in m.iter().take(8) {
            let syms: Vec<String> = w.iter().map(|s| format!("t{}a{}", sym_transform(*s), sym_arm(*s))).collect();
            println!("   missed {n:>5}: ...{}", syms.join(" "));
        }
    }

    /// **What the inverse walk plans for the family-J flames**, at
    /// every zoom the forward walk could not reach.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_the_inverse_walk_plans() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["grand-julian", "julian-disc", "random1"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")) else {
                continue;
            };
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("a config");
            let t0 = std::time::Instant::now();
            let b = match Backward::read(&cfg.flame, reg) {
                Ok(b) => b,
                Err(why) => {
                    println!("== {name}: refused — {why}");
                    continue;
                }
            };
            let read_ms = t0.elapsed().as_secs_f64() * 1e3;
            let q = b.sample_point(0.75);
            println!("== {name}: read in {read_ms:.0} ms, {} maps, extent {:.3e}, view at [{:.4}, {:.4}]", b.ifs.maps.len(), b.extent, q[0], q[1]);
            for zoom in [1e2f64, 1e3, 1e4, 1e6, 1e8] {
                let view = View::of(zoom, q, 512, 512);
                let t0 = std::time::Instant::now();
                let r = b.plan(view);
                let ms = t0.elapsed().as_secs_f64() * 1e3;
                match r {
                    Ok(c) => {
                        let shortest = c.words.iter().map(|w| w.word.len()).min().unwrap_or(0);
                        println!(
                            "   zoom {zoom:>6.0e} {ms:>7.1} ms  {:>4} words, depth {:>2}..{:>2}, mass {:.2e}, efficiency {:.2}, speedup {:.2e}, lost {:.2e}",
                            c.words.len(), shortest, c.depth, c.mass, c.efficiency, c.speedup(), c.lost
                        );
                    }
                    Err(e) => println!("   zoom {zoom:>6.0e} {ms:>7.1} ms  {e:?}"),
                }
            }
        }
    }
}
