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
///
/// **One.** A sample point is on the attractor exactly, and the
/// indexed walk grows a thin region by its orbit split, its top-ups and
/// its replays of unseen children. The cloud cannot always grow one: it
/// needs the region to widen as it is pulled back, and where the
/// dominant map is nearly neutral it does not. At 16, julian-disc from
/// 1e3 (3 sample points in view) and random1 from 1e4 (1 point) walked
/// the cloud ten levels at the same size until it drifted off the
/// attractor, and planned nothing; at 1, they plan with coverage 0.998
/// and 0.990. Grand-julian's plans were unchanged but for two deep views.
pub const MIN_SEED: usize = 1;
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
    /// is meaningless. Checked between a level's batches.
    pub cancel: Option<&'a AtomicBool>,
    /// Answer the walk's questions on the GPU when given, and when its
    /// kernel builds for the flame; the CPU otherwise. Desktop only for
    /// now -- the web is phase 3 of `gpu-cylinder-planning.md`.
    #[cfg(not(target_arch = "wasm32"))]
    pub gpu: Option<&'a Mutex<crate::scene::plan_gpu::GpuPlanner>>,
}

/// The budget where a plan must block: the web, which has no threads and
/// plans on the UI thread. Raising [`TIME_BUDGET`] to 20 s for the
/// background thread would otherwise have let a single-core web plan
/// freeze the page that long. Hitting it forces the frontier, so the
/// plan is complete and less efficient, never incomplete.
pub const INLINE_BUDGET: std::time::Duration = std::time::Duration::from_millis(1500);

impl Default for PlanOptions<'_> {
    fn default() -> Self {
        let budget = if cfg!(target_arch = "wasm32") { INLINE_BUDGET } else { TIME_BUDGET };
        Self {
            budget,
            cancel: None,
            #[cfg(not(target_arch = "wasm32"))]
            gpu: None,
        }
    }
}

/// One question for the walk's evaluator: apply `word` to each of these
/// sample points (indices into the sample).
pub struct EvalJob<'a> {
    pub word: &'a [u32],
    pub points: &'a [u32],
}

/// **Answers the walk's one question, in batches**: apply each job's
/// word to each of its sample points -- does the result land in the
/// view? One byte per point, in job order, 1 where it lands.
///
/// A level asks everything it needs in five batches (seeds, replays,
/// the replays' second pass, checks, top-ups), so the same walk runs on
/// the CPU ([`CpuEval`]) or the GPU (`scene::plan_gpu::PlanGpu`), and
/// every decision stays in the walk.
pub trait Evaluate {
    fn lands(&mut self, b: &Backward, view: View, jobs: &[EvalJob]) -> Vec<u8>;

    /// Whether a batch's round trip costs more than extra answers do.
    /// The walk then asks, with a level's replays, everything it MIGHT
    /// ask next -- the replays' second pass and every child's checks --
    /// and uses only the answers it would have asked for, so the plan is
    /// the same either way. The GPU's batches queue behind the render's
    /// frames, so a round trip there costs up to a frame; the CPU pays
    /// for every answer and asks only what it needs.
    fn speculative(&self) -> bool {
        false
    }

    /// Plain `jobs` and `gathers`, asked together: one round trip where
    /// round trips cost. A gather is `Backward::gather_seen` -- the
    /// candidates, exactly -- and a check of each against its word. The
    /// answers to `jobs` come back as `lands` gives them; each gather
    /// comes back as its candidate count and the candidates that landed,
    /// in candidate order.
    ///
    /// This default gathers on the CPU and asks `lands` for the lot, so
    /// an evaluator that only answers `lands` plans exactly as before.
    fn gather_lands(&mut self, b: &Backward, view: View, jobs: &[EvalJob], gathers: &[GatherJob]) -> (Vec<u8>, Vec<Gathered>) {
        gather_on_cpu(self, b, view, jobs, gathers)
    }
}

/// `Evaluate::gather_lands` with the gathers made on the CPU and every
/// question put to `eval.lands`.
pub fn gather_on_cpu<E: Evaluate + ?Sized>(
    eval: &mut E,
    b: &Backward,
    view: View,
    jobs: &[EvalJob],
    gathers: &[GatherJob],
) -> (Vec<u8>, Vec<Gathered>) {
    let cands: Vec<Vec<u32>> = map_all(gathers, |g| Backward::gather_seen(b.index(g.index), g.seen, g.cap));
    let mut all: Vec<EvalJob> = jobs.iter().map(|j| EvalJob { word: j.word, points: j.points }).collect();
    all.extend(gathers.iter().zip(&cands).map(|(g, c)| EvalJob { word: g.word, points: c }));
    let mut answers = eval.lands(b, view, &all);
    let n: usize = jobs.iter().map(|j| j.points.len()).sum();
    let mut at = n;
    let gathered = cands
        .iter()
        .map(|c| {
            let got = &answers[at..at + c.len()];
            at += c.len();
            Gathered { cands: c.len(), hits: c.iter().zip(got).filter(|(_, g)| **g == 1).map(|(i, _)| *i).collect() }
        })
        .collect();
    answers.truncate(n);
    (answers, gathered)
}

/// Every index the walk gathers from, concatenated for the GPU: the
/// landing indexes in alphabet order, then the grid. Index k's entries are
/// `offsets[k]..offsets[k + 1]`, sorted by cell as `Index` keeps them.
pub struct IndexTables {
    pub cells: Vec<[i32; 2]>,
    pub idx: Vec<u32>,
    pub offsets: Vec<u32>,
}

/// Which of the walk's indexes a gather reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexId {
    /// Sample indices by the cell they land in under this alphabet entry.
    Landing(usize),
    /// Sample indices by the cell they lie in.
    Grid,
}

/// A gather and a check in one: the candidates `Backward::gather_seen`
/// takes from `index` over the cells `seen` (at most `cap`), and which of
/// them `word` sends into the view.
pub struct GatherJob<'a> {
    pub word: &'a [u32],
    pub index: IndexId,
    /// Sorted and deduplicated, neighbours already added: `expand_cells`.
    pub seen: &'a [Cell],
    pub cap: usize,
}

/// What a gather found: how many candidates it took, and the ones that
/// landed, in candidate order.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Gathered {
    pub cands: usize,
    pub hits: Vec<u32>,
}

/// The CPU's answers: `Backward::lands`, in f64, over the jobs in
/// parallel where there are threads.
pub struct CpuEval;

impl Evaluate for CpuEval {
    fn lands(&mut self, b: &Backward, view: View, jobs: &[EvalJob]) -> Vec<u8> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use rayon::prelude::*;
            jobs.par_iter()
                .flat_map_iter(|j| {
                    let (word, points) = (j.word, j.points);
                    points.iter().map(move |&i| b.lands(word, b.sample[i as usize], view) as u8)
                })
                .collect()
        }
        #[cfg(target_arch = "wasm32")]
        {
            jobs.iter()
                .flat_map(|j| {
                    let (word, points) = (j.word, j.points);
                    points.iter().map(move |&i| b.lands(word, b.sample[i as usize], view) as u8)
                })
                .collect()
        }
    }
}

/// `f` over every item, in parallel where there are threads; results in
/// order.
fn map_all<T: Sync, U: Send>(items: &[T], f: impl Fn(&T) -> U + Sync + Send) -> Vec<U> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        items.par_iter().map(f).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        items.iter().map(f).collect()
    }
}

/// `f` on every item, in parallel where there are threads.
fn each_mut<T: Send>(items: &mut [T], f: impl Fn(&mut T) + Sync + Send) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        items.par_iter_mut().for_each(f);
    }
    #[cfg(target_arch = "wasm32")]
    {
        items.iter_mut().for_each(f);
    }
}

/// How many of the `VERIFY` points a replay runs first.
///
/// **A sequential replay.** The first `REPLAY_FIRST` of the `VERIFY`
/// points -- spread evenly through them -- and the rest only when the
/// share so far is close enough to `CUT_EFFICIENCY` that noise could flip
/// the decision ([`REPLAY_EXTEND`]). The number a replay produces decides
/// only whether a child is kept or carried, and ranks the beam; neither
/// touches completeness, which is a count of points. A child landing 30%
/// in frame is carried after a hundred points as surely as after four
/// hundred, and replays were 64% of a plan's work.
pub const REPLAY_FIRST: usize = 100;

/// A first pass whose share falls strictly inside this band runs the
/// rest of the `VERIFY` points: close enough to `CUT_EFFICIENCY` (0.9)
/// that a hundred points could decide it wrongly. At 0.9 a hundred
/// points have a standard deviation of 0.03.
pub const REPLAY_EXTEND: (f64, f64) = (0.8, 0.97);

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

/// A grid cell: `floor(p / cell)` in x and y.
pub type Cell = (i32, i32);

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

/// A child being decided while its level's batches run. See
/// `Backward::expand_level`.
struct Child {
    /// Its symbol, as an alphabet index.
    ai: usize,
    word: Vec<u32>,
    prob: f64,
    /// The walk stops here: kept if any of it lands.
    last: bool,
    /// Candidates from the index, or the pulled-back cloud.
    pts: Pts,
    /// The node's points this symbol produced: in the child's region by
    /// construction, so never checked.
    orbit_hits: Vec<u32>,
    /// The replay: points landed of points tried.
    hit: usize,
    total: usize,
    /// Candidates checked and found in the region.
    hits: Vec<u32>,
    /// Thin after its check, so searched again, wider.
    topped: bool,
    /// How many candidates its gather took. An index child with none and
    /// no orbit points is UNSEEN: neither the node's points nor the index
    /// put a single sample point in its region. It is replayed like any
    /// other -- a replay that lands keeps or carries it, and only a replay
    /// that lands nothing drops it. See `Backward::children_of`.
    n_cands: usize,
    /// Its candidates' answers, asked ahead of need. See
    /// `Evaluate::speculative`.
    spec: Option<Vec<u32>>,
    fate: Fate,
}

impl Child {
    fn eff(&self) -> f64 {
        self.hit as f64 / self.total.max(1) as f64
    }

    fn cands(&self) -> &[u32] {
        match &self.pts {
            Pts::Index(c) => c,
            Pts::Cloud(_) => &[],
        }
    }
}

/// What the replays decided for a child, before its points are known.
#[derive(Clone, Copy)]
enum Fate {
    /// Carried, with an index region still to be checked.
    Undecided,
    /// Kept, at this efficiency: it needs no points.
    Kept(f64),
    /// Carried with its cloud as it stands.
    Carried,
    /// Unseen and its replay landed nothing: no evidence it holds any
    /// of the view. See `Child::unseen`.
    Dropped,
}

/// A node of the level being expanded.
struct Open {
    node: Node,
    children: Vec<Child>,
    trace: Trace,
    /// The cells its children gather over: `expand_cells` of the node's
    /// own, once per node rather than once per child.
    seen: Vec<Cell>,
}

/// What expanding one node produced. See `Backward::expand_level`.
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
    /// The `VERIFY` verification points, split as a replay runs them:
    /// every `VERIFY / REPLAY_FIRST`-th first, the others only when the
    /// first pass is too close to the cut to trust.
    verify_first: Vec<u32>,
    verify_rest: Vec<u32>,
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
        let stride = (sample.len() / VERIFY).max(1);
        let every = (VERIFY / REPLAY_FIRST).max(1);
        let (mut verify_first, mut verify_rest) = (Vec::new(), Vec::new());
        for k in 0..VERIFY {
            if k * stride >= sample.len() {
                break;
            }
            if k % every == 0 { &mut verify_first } else { &mut verify_rest }.push((k * stride) as u32);
        }
        Ok(Self {
            ifs,
            transforms,
            alphabet,
            sample,
            made_by,
            centre,
            extent,
            cell,
            grid,
            landing,
            verify_first,
            verify_rest,
        })
    }

    /// The attractor sample the walk's questions index into.
    pub fn sample(&self) -> &[[f64; 2]] {
        &self.sample
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

    /// The share of all `VERIFY` points `word` sends into the view, for
    /// measurements that want the full count.
    #[cfg(test)]
    fn replay_full(&self, word: &[u32], view: View) -> f64 {
        let n = self.verify_first.len() + self.verify_rest.len();
        let hit = self.verify_first.iter().chain(&self.verify_rest).filter(|&&i| self.lands(word, self.sample[i as usize], view)).count();
        hit as f64 / n.max(1) as f64
    }

    /// Whether `x`'s forward image along `word` lands in the view.
    fn lands(&self, word: &[u32], x: [f64; 2], view: View) -> bool {
        match self.forward_along(word, x) {
            Some(y) => (y[0] - view.centre[0]).hypot(y[1] - view.centre[1]) <= view.radius,
            None => false,
        }
    }


    /// The cells a gather reads: `cells` and, if asked, their eight
    /// neighbours, sorted and deduplicated.
    fn expand_cells(cells: &[Cell], neighbours: bool) -> Vec<Cell> {
        let reach: i32 = if neighbours { 1 } else { 0 };
        let mut seen: Vec<Cell> = Vec::with_capacity(cells.len() * if neighbours { 9 } else { 1 });
        for &(cx, cy) in cells {
            for dx in -reach..=reach {
                for dy in -reach..=reach {
                    seen.push((cx + dx, cy + dy));
                }
            }
        }
        seen.sort_unstable();
        seen.dedup();
        seen
    }

    /// Up to `cap` sample indices filed under the cells `seen` in `index`.
    /// The lists are disjoint across cells -- an index lands in exactly
    /// one cell per symbol -- so nothing needs deduplicating.
    ///
    /// **By measure, across the whole region.** Every cell's list is a
    /// μ-distributed sample, so the concatenation of all of them is the
    /// region's measure, and taking every k-th entry of it is a fair
    /// sample that neither favours the front of the cell order nor spends
    /// as much on an empty cell as on a dense one. Both mistakes were made
    /// and measured: filling the cap from the first cells took the
    /// region's leftmost strip (70% complete at a view straddling an arm
    /// boundary), and two points per cell sampled by AREA and starved the
    /// ring the measure lives on (42%). Counting a cell is two binary
    /// searches, so the whole region is counted before anything is taken.
    ///
    /// The GPU does exactly this (`plan_gather.wgsl`); a plan must not
    /// depend on which one gathered.
    fn gather_seen(index: &Index, seen: &[Cell], cap: usize) -> Vec<u32> {
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

    /// The walk's indexes, concatenated for the GPU. See [`IndexTables`].
    pub fn index_tables(&self) -> IndexTables {
        let n: usize = self.landing.iter().map(|i| i.entries.len()).sum::<usize>() + self.grid.entries.len();
        let mut t = IndexTables { cells: Vec::with_capacity(n), idx: Vec::with_capacity(n), offsets: vec![0] };
        for index in self.landing.iter().chain(std::iter::once(&self.grid)) {
            for &((x, y), i) in &index.entries {
                t.cells.push([x, y]);
                t.idx.push(i);
            }
            t.offsets.push(t.cells.len() as u32);
        }
        t
    }

    /// Where `id` sits in [`IndexTables::offsets`].
    pub fn index_slot(&self, id: IndexId) -> usize {
        match id {
            IndexId::Landing(ai) => ai,
            IndexId::Grid => self.landing.len(),
        }
    }

    /// The index a gather names.
    fn index(&self, id: IndexId) -> &Index {
        match id {
            IndexId::Landing(ai) => &self.landing[ai],
            IndexId::Grid => &self.grid,
        }
    }

    /// A node's children: one per symbol its region's points came
    /// through, each with its candidates. Reads only `self` and the node,
    /// so the nodes of a level find theirs in parallel.
    fn children_of(&self, o: &mut Open, depth: usize, floor_mass: f64, gather_now: bool) {
        let tr = &mut o.trace;
        let node = &o.node;
        let mut seen: Vec<Cell> = Vec::new();
        let junk_r = JUNK_EXTENTS * self.extent;
        let is_junk = |q: [f64; 2]| (q[0] - self.centre[0]).hypot(q[1] - self.centre[1]) > junk_r;
        let mut found: Vec<(usize, Pts)> = Vec::new(); // (alphabet index, candidates)
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
                        found.push((ai, Pts::Cloud(pts)));
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
                seen = Self::expand_cells(&cells, neighbours);
                // **A child the sample does not see is replayed, not
                // dropped.** One whose region holds none of the node's
                // points and none of the index's candidates can still hold
                // real measure: its share of the node is below one part in
                // the node's point count. Dropping those unreplayed lost
                // julian-disc 69% of a 1e2 view -- its dominant map is
                // nearly neutral there, so a branch is carried ~80 levels
                // at efficiency ~0.3 and at each level 50 julian arms,
                // 0.06% of the node apiece and mostly unseen, were
                // dropped: up to 3% a level, compounding, and invisible
                // to the completeness check, which counts points.
                //
                // Gathered here on the CPU; an evaluator that gathers
                // (`Evaluate::speculative`) does it with the replays.
                for (ai, _a) in self.alphabet.iter().enumerate() {
                    let cands =
                        if gather_now { Self::gather_seen(&self.landing[ai], &seen, CAND_CAP) } else { Vec::new() };
                    found.push((ai, Pts::Index(cands)));
                }
            }
        }
        let children = found
            .into_iter()
            .map(|(ai, pts)| {
                let a = &self.alphabet[ai];
                let prob = node.prob * a.prob;
                let mut word = Vec::with_capacity(node.word.len() + 1);
                word.push(a.sym);
                word.extend_from_slice(&node.word);
                let n_cands = match &pts {
                    Pts::Index(c) => c.len(),
                    Pts::Cloud(_) => 0,
                };
                Child {
                    ai,
                    word,
                    prob,
                    last: depth == MAX_DEPTH || prob < MEASURE_FLOOR * floor_mass,
                    pts,
                    orbit_hits: std::mem::take(&mut from_orbit[ai]),
                    hit: 0,
                    total: 0,
                    hits: Vec::new(),
                    topped: false,
                    n_cands,
                    spec: None,
                    fate: Fate::Undecided,
                }
            })
            .collect();
        o.children = children;
        o.seen = seen;
    }

    /// **Expand a whole level, in batches.** Each node's children are
    /// found, replayed, kept or carried, checked, and the node checked
    /// for completeness -- the walk's rules, unchanged -- but the one
    /// question underneath all of it, *does this word send this sample
    /// point into the view*, is asked for every node of the level at
    /// once, in five batches: seeds, replays, the replays' second pass,
    /// checks, top-ups. `eval` answers them on the CPU or the GPU.
    ///
    /// `None` when cancelled between batches.
    #[allow(clippy::too_many_arguments)]
    fn expand_level(
        &self,
        frontier: Vec<Node>,
        depth: usize,
        view: View,
        floor_mass: f64,
        watch: Option<&[u32]>,
        record: bool,
        eval: &mut dyn Evaluate,
        tr: &mut Trace,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<Vec<Expanded>> {
        use std::time::Instant;
        let watched = |t: &mut Trace, word: &[u32], what: &str, detail: &dyn Fn() -> String| {
            if let Some(line) = watch_line(watch, word, depth, what, detail) {
                t.watched.push(line);
            }
        };
        let mut opens: Vec<Open> = frontier
            .into_iter()
            .map(|node| {
                let mut trace = Trace::default();
                trace.nodes_expanded += 1;
                if record {
                    trace.expanded.push(node.word.clone());
                }
                Open { node, children: Vec::new(), trace, seen: Vec::new() }
            })
            .collect();
        let speculate = eval.speculative();

        // **1. Seeds.** A cloud region is seeded from the grid the moment
        // sample points lie in it: the candidates are the sample points
        // in the cells the cloud occupies, checked exactly.
        let t = Instant::now();
        let seen: Vec<Option<Vec<Cell>>> = map_all(&opens, |o| match &o.node.pts {
            Pts::Cloud(cloud) => {
                let mut cells: Vec<Cell> = cloud.iter().map(|p| self.cell_of(*p)).collect();
                cells.sort_unstable();
                cells.dedup();
                Some(Self::expand_cells(&cells, true))
            }
            Pts::Index(_) => None,
        });
        let seeded = {
            let gathers: Vec<GatherJob> = opens
                .iter()
                .zip(&seen)
                .filter_map(|(o, s)| {
                    s.as_ref().map(|s| GatherJob { word: &o.node.word, index: IndexId::Grid, seen: s, cap: SEED_CANDIDATES })
                })
                .collect();
            eval.gather_lands(self, view, &[], &gathers).1
        };
        let mut seeded = seeded.into_iter();
        for (o, s) in opens.iter_mut().zip(&seen) {
            if s.is_none() {
                continue;
            }
            let hits = seeded.next().expect("one answer per gather").hits;
            let Pts::Cloud(cloud) = &o.node.pts else { continue };
            if hits.len() >= MIN_SEED {
                let n = cloud.len();
                watched(&mut o.trace, &o.node.word, "SEEDED", &|| format!("{} sample points replace {n} cloud points", hits.len()));
                o.node.pts = Pts::Index(hits);
                o.trace.seeded += 1;
            }
        }
        drop(seen);
        tr.t_seed += t.elapsed();
        if cancelled() {
            return None;
        }

        // **2. Children.** The gathers are the cost; nodes in parallel.
        let t = Instant::now();
        each_mut(&mut opens, |o| self.children_of(o, depth, floor_mass, !speculate));
        tr.t_gather += t.elapsed();
        if cancelled() {
            return None;
        }

        // **3. Replays**: the first `REPLAY_FIRST` verification points
        // for every child, then the rest for the children whose share is
        // too close to the cut to trust. See `REPLAY_FIRST`.
        let t = Instant::now();
        let in_band = |c: &Child| {
            let e = c.hit as f64 / c.total.max(1) as f64;
            e > REPLAY_EXTEND.0 && e < REPLAY_EXTEND.1
        };
        let landed = |g: &[u8]| g.iter().filter(|g| **g == 1).count();
        let (n1, n2) = (self.verify_first.len(), self.verify_rest.len());
        if speculate {
            // One batch: both passes for every child, and every index
            // child's gather and check.
            let (answers, gathered) = {
                let mut jobs: Vec<EvalJob> = Vec::new();
                let mut gathers: Vec<GatherJob> = Vec::new();
                for o in &opens {
                    for c in &o.children {
                        jobs.push(EvalJob { word: &c.word, points: &self.verify_first });
                        jobs.push(EvalJob { word: &c.word, points: &self.verify_rest });
                        if matches!(c.pts, Pts::Index(_)) {
                            gathers.push(GatherJob {
                                word: &c.word,
                                index: IndexId::Landing(c.ai),
                                seen: &o.seen,
                                cap: CAND_CAP,
                            });
                        }
                    }
                }
                eval.gather_lands(self, view, &jobs, &gathers)
            };
            let mut gathered = gathered.into_iter();
            let mut at = 0usize;
            for c in opens.iter_mut().flat_map(|o| o.children.iter_mut()) {
                c.hit = landed(&answers[at..at + n1]);
                c.total = n1;
                at += n1;
                let rest = landed(&answers[at..at + n2]);
                at += n2;
                if in_band(c) {
                    c.hit += rest;
                    c.total += n2;
                }
                if matches!(c.pts, Pts::Index(_)) {
                    let g = gathered.next().expect("one answer per gather");
                    c.n_cands = g.cands;
                    c.spec = Some(g.hits);
                }
            }
        } else {
            let answers = {
                let jobs: Vec<EvalJob> = opens
                    .iter()
                    .flat_map(|o| o.children.iter().map(|c| EvalJob { word: &c.word, points: &self.verify_first }))
                    .collect();
                eval.lands(self, view, &jobs)
            };
            for (k, c) in opens.iter_mut().flat_map(|o| o.children.iter_mut()).enumerate() {
                c.hit = landed(&answers[k * n1..(k + 1) * n1]);
                c.total = n1;
            }
            let answers = {
                let jobs: Vec<EvalJob> = opens
                    .iter()
                    .flat_map(|o| o.children.iter())
                    .filter(|c| in_band(c))
                    .map(|c| EvalJob { word: &c.word, points: &self.verify_rest })
                    .collect();
                eval.lands(self, view, &jobs)
            };
            let mut k = 0usize;
            for c in opens.iter_mut().flat_map(|o| o.children.iter_mut()) {
                if in_band(c) {
                    c.hit += landed(&answers[k * n2..(k + 1) * n2]);
                    c.total += n2;
                    k += 1;
                }
            }
        }
        for o in &mut opens {
            for c in &o.children {
                o.trace.n_replay += c.total;
            }
        }
        tr.t_replay += t.elapsed();
        if cancelled() {
            return None;
        }

        // **4. Kept or carried.** Below the floor, or at the depth cap,
        // the walk stops -- and the word is FORCED if any of it lands,
        // never dropped. See `MEASURE_FLOOR`.
        for o in &mut opens {
            for c in &mut o.children {
                let eff = c.eff();
                let prob = c.prob;
                let unseen = matches!(c.pts, Pts::Index(_)) && c.n_cands == 0 && c.orbit_hits.is_empty();
                if unseen && !(eff > 0.0) {
                    c.fate = Fate::Dropped;
                    o.trace.nocand += 1;
                    continue;
                }
                // A child the walk stops at is FORCED even when its
                // replay landed nothing: zero hits in `VERIFY` samples
                // means under one part in `VERIFY`, not none. Dropping
                // those at the floor cost 1% of a view in scattered
                // specks, while forcing them costs at most their
                // probability each.
                if !(eff > 0.0) && c.last {
                    watched(&mut o.trace, &c.word, "ZERO", &|| format!("prob {prob:.2e}"));
                    o.trace.floor += 1;
                }
                if eff >= CUT_EFFICIENCY || c.last {
                    // **Kept: it needs no points.** Checking a kept
                    // child's candidates exactly was 65% of a plan's
                    // time, and ~90% of the children checked were kept.
                    watched(&mut o.trace, &c.word, "CUT", &|| format!("eff {eff:.2} prob {prob:.2e}"));
                    c.fate = Fate::Kept(eff);
                } else if matches!(c.pts, Pts::Cloud(_)) {
                    c.fate = Fate::Carried;
                }
            }
        }

        // **5. Checks.** A carried child now needs its region's points,
        // checked exactly on a capped set of candidates.
        let t = Instant::now();
        let checking = |c: &Child| matches!(c.fate, Fate::Undecided);
        if speculate {
            // Answered with the replays; a kept child's answers go unused.
            for o in &mut opens {
                for c in &mut o.children {
                    let spec = c.spec.take();
                    if checking(c) {
                        o.trace.n_verify += c.n_cands;
                        c.hits = spec.unwrap_or_default();
                    }
                }
            }
        } else {
            let answers = {
                let jobs: Vec<EvalJob> = opens
                    .iter()
                    .flat_map(|o| o.children.iter())
                    .filter(|c| checking(c))
                    .map(|c| EvalJob { word: &c.word, points: c.cands() })
                    .collect();
                eval.lands(self, view, &jobs)
            };
            let mut at = 0usize;
            for o in &mut opens {
                for c in &mut o.children {
                    if !checking(c) {
                        continue;
                    }
                    let cands = c.cands();
                    let got = &answers[at..at + cands.len()];
                    at += cands.len();
                    o.trace.n_verify += cands.len();
                    let hits: Vec<u32> = cands.iter().zip(got).filter(|(_, g)| **g == 1).map(|(i, _)| *i).collect();
                    c.hits = hits;
                }
            }
        }

        // **6. Top-ups.** A thin child is searched again, wider, over its
        // parent's cells. See `TOPUP_BELOW`.
        for o in &mut opens {
            let parent_indexed = matches!(o.node.pts, Pts::Index(_));
            for c in &mut o.children {
                c.topped = checking(c) && parent_indexed && c.hits.len() + c.orbit_hits.len() < TOPUP_BELOW;
                if c.topped {
                    o.trace.topped_up += 1;
                }
            }
        }
        // The parent's cells, all of them, and their neighbours: once per
        // node with a thin child.
        let wide: Vec<Vec<Cell>> = map_all(&opens, |o| {
            let Pts::Index(parent) = &o.node.pts else { return Vec::new() };
            if !o.children.iter().any(|c| c.topped) {
                return Vec::new();
            }
            let mut cells: Vec<Cell> = parent.iter().map(|&i| self.cell_of(self.sample[i as usize])).collect();
            cells.sort_unstable();
            cells.dedup();
            Self::expand_cells(&cells, true)
        });
        let topped = {
            let gathers: Vec<GatherJob> = opens
                .iter()
                .zip(&wide)
                .flat_map(|(o, w)| {
                    o.children.iter().filter(|c| c.topped).map(move |c| GatherJob {
                        word: &c.word,
                        index: IndexId::Landing(c.ai),
                        seen: w,
                        cap: TOPUP_CAP,
                    })
                })
                .collect();
            eval.gather_lands(self, view, &[], &gathers).1
        };
        let mut topped = topped.into_iter();
        for o in &mut opens {
            for c in o.children.iter_mut().filter(|c| c.topped) {
                let g = topped.next().expect("one answer per gather");
                o.trace.n_verify += g.cands;
                c.hits.extend(g.hits);
            }
        }
        drop(wide);
        tr.t_verify += t.elapsed();
        if cancelled() {
            return None;
        }

        // **7. Each node decided**, its children in their order.
        Some(opens.into_iter().map(|o| self.close(o, depth, view, watch)).collect())
    }

    /// Settle a node once its children's questions are answered: which
    /// children are kept, which carried with what points -- and then
    /// whether they account for the node, or the node is forced itself.
    ///
    /// A kept word's `centre` and `radius` are the view disc: what the
    /// walk knows of where its landed points sit is that they are in the
    /// view. Nothing downstream reads them for an inverse-walk plan (the
    /// kernel's table holds the symbols, the probability and the colour
    /// fold), and computing them needed positions an evaluator does not
    /// return.
    fn close(&self, o: Open, depth: usize, view: View, watch: Option<&[u32]>) -> Expanded {
        let Open { node, children, mut trace, .. } = o;
        let watched = |t: &mut Trace, word: &[u32], what: &str, detail: &dyn Fn() -> String| {
            if let Some(line) = watch_line(watch, word, depth, what, detail) {
                t.watched.push(line);
            }
        };
        let points_of = |pts: &Pts| -> Vec<[f64; 2]> {
            match pts {
                Pts::Cloud(c) => c.clone(),
                Pts::Index(idx) => idx.iter().map(|&i| self.sample[i as usize]).collect(),
            }
        };
        let disc = |word: Vec<u32>, prob: f64| Cylinder { word, prob, centre: view.centre, radius: view.radius };

        let mut node_kept: Vec<(Cylinder, f64)> = Vec::new();
        let mut node_next: Vec<Node> = Vec::new();
        // Which children survived -- carried or kept -- by alphabet
        // index, for the point count below.
        let mut survived = vec![false; self.alphabet.len()];
        for c in children {
            let eff = c.eff();
            let n_cands = c.n_cands;
            let Child { ai, word, prob, pts, orbit_hits, mut hits, fate, .. } = c;
            let pts = match fate {
                Fate::Dropped => continue,
                Fate::Kept(eff) => {
                    survived[ai] = true;
                    node_kept.push((disc(word, prob), eff));
                    continue;
                }
                Fate::Carried => pts,
                Fate::Undecided => {
                    // The orbit's points need no check: they are in the
                    // child's region by construction.
                    hits.extend_from_slice(&orbit_hits);
                    hits.sort_unstable();
                    hits.dedup();
                    if hits.is_empty() {
                        trace.empty += 1;
                        if eff > 0.0 {
                            // It lands -- the replay says so -- but no
                            // point of its region was found to expand it
                            // from. Forced as it stands.
                            watched(&mut trace, &word, "FORCEDCH", &|| format!("{n_cands} candidates, none land; eff {eff:.2}"));
                            survived[ai] = true;
                            node_kept.push((disc(word, prob), eff));
                        } else {
                            watched(&mut trace, &word, "EMPTY", &|| format!("{n_cands} candidates, none land"));
                        }
                        continue;
                    }
                    Pts::Index(hits)
                }
            };
            survived[ai] = true;
            let n = match &pts {
                Pts::Cloud(c) => c.len(),
                Pts::Index(i) => i.len(),
            };
            let indexed = matches!(pts, Pts::Index(_));
            watched(&mut trace, &word, "carried", &|| {
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
        let mut out = Expanded::default();
        if !node.word.is_empty() && node.eff > 0.0 && covered.is_some_and(|c| c < COMPLETE_ENOUGH) {
            watched(&mut trace, &node.word, "FORCED", &|| format!("children cover {:.3} of its points", covered.unwrap_or(0.0)));
            out.kept.push((disc(node.word, node.prob), node.eff));
            trace.forced += 1;
            out.trace = trace;
            return out;
        }
        trace.cut += node_kept.len();
        trace.not_yet += node_next.len();
        out.kept = node_kept;
        out.next = node_next;
        out.trace = trace;
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
        self.plan_with_eval(view, tr, opts, &mut CpuEval)
    }

    /// Plan with the evaluator `opts` offers: the GPU when it is given
    /// and its kernel builds for `flame`, the CPU otherwise. `flame` must
    /// be the flame this walk was read from.
    pub fn plan_for(&self, flame: &Flame, view: View, opts: PlanOptions) -> Result<Cylinders, NoCylinders> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(gpu) = opts.gpu {
            // Held for the whole plan: one plan runs at a time, and a
            // cancelled one stops at its next batch.
            let mut g = gpu.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(eval) = g.for_flame(flame, self) {
                let mut tr = Trace::default();
                eval.totals = Default::default();
                eval.batches = 0;
                let t0 = std::time::Instant::now();
                let r = self.plan_with_eval(view, &mut tr, opts, eval);
                {
                    log::debug!(
                        "GPU plan {:.0} ms: {} batches, pack {:.0} wait {:.0} read {:.0} ms; walls seed {:.0} gather {:.0} replay {:.0} verify {:.0}",
                        t0.elapsed().as_secs_f64() * 1e3,
                        eval.batches,
                        eval.totals.pack,
                        eval.totals.wait,
                        eval.totals.read,
                        tr.t_seed.as_secs_f64() * 1e3,
                        tr.t_gather.as_secs_f64() * 1e3,
                        tr.t_replay.as_secs_f64() * 1e3,
                        tr.t_verify.as_secs_f64() * 1e3,
                    );
                }
                return r;
            }
        }
        let _ = flame;
        self.plan_opts(view, opts)
    }

    /// Plan with a given evaluator, for a caller holding one.
    pub fn plan_eval(&self, view: View, opts: PlanOptions, eval: &mut dyn Evaluate) -> Result<Cylinders, NoCylinders> {
        let mut tr = Trace::default();
        self.plan_with_eval(view, &mut tr, opts, eval)
    }

    fn plan_with_eval(&self, view: View, tr: &mut Trace, opts: PlanOptions, eval: &mut dyn Evaluate) -> Result<Cylinders, NoCylinders> {
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
                    kept.push((Cylinder { word: n.word, prob: n.prob, centre: view.centre, radius: view.radius }, n.eff));
                    tr.forced += 1;
                }
                break;
            }

            // **Every node of a level is expanded at once**, its questions
            // asked in batches (`expand_level`). Nodes are independent --
            // each reads the flame's analysis and writes only its own
            // results -- so the CPU's work between batches runs in
            // parallel. The floor is read as it stood at the start of the
            // level, and the results are merged in frontier order, so a
            // plan is the same however the threads ran and whichever
            // evaluator answered.
            let floor_mass = kept_mass;
            let Some(results) = self.expand_level(
                std::mem::take(&mut frontier),
                depth,
                view,
                floor_mass,
                watch.as_deref(),
                record,
                eval,
                tr,
                &cancelled,
            ) else {
                return Err(NoCylinders::ViewIsEmpty);
            };

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
                    kept_mass += n.prob;
                    kept.push((Cylinder { word: n.word, prob: n.prob, centre: view.centre, radius: view.radius }, n.eff));
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
        for z in [1e3f64, 1e4, 1e6] {
            cases.push((format!("grand-julian x{z:.0e}"), gj.clone(), z));
        }
        if let Ok(path) = std::env::var("FFLAME") {
            let c: crate::config::FractalConfig =
                serde_json::from_str(&std::fs::read_to_string(&path).expect("file")).expect("config");
            let z = c.zoom as f64;
            cases.push((path, c, z));
        }
        println!("cores: {}", std::thread::available_parallelism().map_or(0, |n| n.get()));
        let (device, queue) = test_device();
        let mut planner = crate::scene::plan_gpu::GpuPlanner::new(&device, &queue);

        /// An evaluator that times another: how much of a plan is the
        /// answering, and how much the walk around it.
        struct Timed<'e> {
            inner: &'e mut dyn Evaluate,
            spent: std::time::Duration,
            batches: usize,
            points: usize,
        }
        impl Evaluate for Timed<'_> {
            fn lands(&mut self, b: &Backward, view: View, jobs: &[EvalJob]) -> Vec<u8> {
                let t = std::time::Instant::now();
                let r = self.inner.lands(b, view, jobs);
                self.spent += t.elapsed();
                self.batches += 1;
                self.points += r.len();
                r
            }
            fn speculative(&self) -> bool {
                self.inner.speculative()
            }
            fn gather_lands(
                &mut self,
                b: &Backward,
                view: View,
                jobs: &[EvalJob],
                gathers: &[GatherJob],
            ) -> (Vec<u8>, Vec<Gathered>) {
                let t = std::time::Instant::now();
                let r = self.inner.gather_lands(b, view, jobs, gathers);
                self.spent += t.elapsed();
                self.batches += 1;
                self.points += r.0.len() + r.1.iter().map(|g| g.cands).sum::<usize>();
                r
            }
        }

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
            let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3;
            println!("== {name}: read {:.0} ms", ms(read));
            let mut cpu = CpuEval;
            let gpu = planner.for_flame(&cfg.flame, &b).expect("the kernel builds");
            gpu.totals = Default::default();
            gpu.batches = 0;
            for (label, inner) in [("CPU", &mut cpu as &mut dyn Evaluate), ("GPU", gpu as &mut dyn Evaluate)] {
                let mut timed = Timed { inner, spent: Default::default(), batches: 0, points: 0 };
                let mut tr = Trace::default();
                let t0 = std::time::Instant::now();
                let plan = b.plan_with_eval(view, &mut tr, PlanOptions::default(), &mut timed);
                let total = t0.elapsed();
                let words = plan.as_ref().map_or(0, |p| p.words.len());
                let other = total.saturating_sub(tr.t_seed + tr.t_gather + tr.t_verify + tr.t_replay);
                println!(
                    "   {label}: plan {:.0} ms, {} nodes, {words} words | answering {:.0} ms in {} batches, {} points | \
                     walls: seed {:.0} gather {:.0} replay {:.0} verify {:.0} other {:.0} ms",
                    ms(total),
                    tr.nodes_expanded,
                    ms(timed.spent),
                    timed.batches,
                    timed.points,
                    ms(tr.t_seed),
                    ms(tr.t_gather),
                    ms(tr.t_replay),
                    ms(tr.t_verify),
                    ms(other)
                );
            }
            let g = planner.for_flame(&cfg.flame, &b).expect("built");
            println!(
                "   GPU batches: {} -- pack {:.0} ms, submit to mapped {:.0} ms, read {:.0} ms",
                g.batches, g.totals.pack, g.totals.wait, g.totals.read
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

    /// **What a margin costs.** Plan for a disc `m` times the view's
    /// radius, then ask of the ACTUAL view: what share of the forced
    /// sampling lands in it (the efficiency the render sees), and
    /// whether the plan still covers it. `FFLAME=path` adds a saved view.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_a_margin_costs() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("grand-julian");
        let gj: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        let b = Backward::read(&gj.flame, reg).expect("armed");
        let mut views: Vec<(String, View)> = Vec::new();
        for z in [1e2f64, 1e3, 1e6] {
            views.push((format!("grand-julian x{z:.0e}"), View::of(z, b.sample_point(0.75), 1280, 720)));
        }
        if let Ok(path) = std::env::var("FFLAME") {
            let c: crate::config::FractalConfig =
                serde_json::from_str(&std::fs::read_to_string(&path).expect("file")).expect("config");
            views.push((path, View::of(c.zoom as f64, [c.pan_x as f64, c.pan_y as f64], 1280, 720)));
        }
        let total: f64 = b.transforms.iter().map(|t| t.weight).sum();
        for (name, view) in views {
            println!("== {name}");
            for m in [1.0f64, 1.25, 1.5, 2.0, 2.25, 3.0] {
                let planned = View { centre: view.centre, radius: view.radius * m };
                let t0 = std::time::Instant::now();
                let Ok(plan) = b.plan(planned) else {
                    println!("   margin {m:.2}: no plan");
                    continue;
                };
                let ms = t0.elapsed().as_secs_f64() * 1e3;
                // In-frame efficiency against the real view.
                let (mut num, mut den) = (0.0f64, 0.0f64);
                for w in &plan.words {
                    let eff = b.replay_full(&w.word, view);
                    num += w.prob * eff;
                    den += w.prob;
                }
                // Coverage of the real view, where the CPU game reaches it.
                let mut st = 0xC0FFEE_u64;
                let mut lcg = move || {
                    st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    ((st >> 33) as f64) / ((1u64 << 31) as f64)
                };
                let mut x = [0.31f64, 0.17];
                let mut hist: Vec<u32> = Vec::new();
                let longest = plan.words.iter().map(|w| w.word.len()).max().unwrap_or(1);
                let (mut inv, mut cov) = (0usize, 0usize);
                for k in 0..40_000_000usize {
                    let mut u = lcg() * total;
                    let mut t = &b.transforms[b.transforms.len() - 1];
                    for c in &b.transforms {
                        if u < c.weight { t = c; break; }
                        u -= c.weight;
                    }
                    let arm = ((lcg() * t.arms as f64) as u32).min(t.arms - 1);
                    let y = forward(&b.ifs.maps[t.map], x, arm);
                    if !finite(y) || y[0].abs() > 1e12 { x = [0.31, 0.17]; hist.clear(); continue; }
                    x = y;
                    hist.push(sym_of(t.index as u32, arm));
                    if hist.len() > longest + 2 { hist.remove(0); }
                    if k < 1000 || (x[0] - view.centre[0]).hypot(x[1] - view.centre[1]) > view.radius { continue; }
                    inv += 1;
                    if plan.words.iter().any(|w| { let n = w.word.len(); hist.len() >= n && hist[hist.len() - n..] == w.word[..] }) {
                        cov += 1;
                    }
                    if inv >= 1500 { break; }
                }
                let coverage = if inv >= 100 { format!("{:.3}", cov as f64 / inv as f64) } else { "--".into() };
                println!(
                    "   margin {m:.2}: {:>5} words, plan {ms:>5.0} ms, in-frame efficiency {:.3}, coverage {coverage}",
                    plan.words.len(),
                    num / den.max(f64::MIN_POSITIVE)
                );
            }
        }
    }

    /// A GPU for the tests that plan on one.
    fn test_device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("adapter");
        println!("adapter: {:?}", adapter.get_info().name);
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("plan test"), ..Default::default() }))
            .expect("device")
    }

    /// The share of an independent chaos game's in-view samples whose
    /// recent past is one of the plan's words: what a targeted render
    /// draws of what the untargeted one does. `None` where the game
    /// cannot reach the view often enough to say -- deep enough, which is
    /// why targeting exists (at 1e6, 400M iterations landed none).
    fn coverage(b: &Backward, plan: &Cylinders, view: View, samples: usize) -> Option<f64> {
        let total: f64 = b.transforms.iter().map(|t| t.weight).sum();
        let mut st = 0xC0FFEE_u64;
        let mut lcg = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut x = [0.31f64, 0.17];
        let mut hist: Vec<u32> = Vec::new();
        let longest = plan.words.iter().map(|w| w.word.len()).max().unwrap_or(1);
        let (mut in_view, mut covered) = (0usize, 0usize);
        for k in 0..400_000_000usize {
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
            if plan.words.iter().any(|w| {
                let k = w.word.len();
                hist.len() >= k && hist[hist.len() - k..] == w.word[..]
            }) {
                covered += 1;
            }
            if in_view >= samples {
                break;
            }
        }
        (in_view >= samples / 10).then(|| covered as f64 / in_view as f64)
    }

    /// **Phase 2 of `gpu-cylinder-planning.md`: the GPU's plans.** The
    /// same views planned with the CPU's answers and the GPU's: time,
    /// size, efficiency, and completeness against an independent chaos
    /// game. The gate is that the GPU's plan is as complete as the CPU's:
    /// its answers differ at the rim by f32 rounding, and a plan is
    /// allowed to differ with them, but not to lose what the CPU's finds.
    /// Both are measured on the same chaos game, so equal plans cover
    /// equally to the sample; 0.003 is two standard errors at 3000
    /// samples near full coverage.
    ///
    /// The CPU's own coverage is reported, not gated here: at one view it
    /// is 0.989 (`gpu-cylinder-planning.md` §14), which is the walk's to
    /// answer for, not the evaluator's.
    #[test]
    #[ignore = "needs a GPU and reads output/flame-zoom"]
    fn the_gpu_plans_as_completely_as_the_cpu() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("grand-julian");
        let gj: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        let b = Backward::read(&gj.flame, reg).expect("armed");
        let (device, queue) = test_device();
        let planner = std::sync::Mutex::new(crate::scene::plan_gpu::GpuPlanner::new(&device, &queue));
        // Build the kernel outside the timings.
        let t0 = std::time::Instant::now();
        let _ = planner.lock().unwrap().for_flame(&gj.flame, &b).is_some();
        println!("kernel for the flame: {:.0} ms", t0.elapsed().as_secs_f64() * 1e3);

        let mut views: Vec<(String, View)> = Vec::new();
        for z in [1e2f64, 1e3, 1e4, 1e6] {
            for frac in [0.25f64, 0.75] {
                views.push((format!("x{z:.0e} at {frac}"), View::of(z, b.sample_point(frac), 1280, 720)));
            }
        }
        if let Ok(t) = std::fs::read_to_string("output/grand-julian-missing-pieces.fflame") {
            let c: crate::config::FractalConfig = serde_json::from_str(&t).expect("config");
            views.push(("missing-pieces".into(), View::of(c.zoom as f64, [c.pan_x as f64, c.pan_y as f64], 1280, 720)));
        }
        let (mut cpu_total, mut gpu_total) = (0.0f64, 0.0f64);
        for (name, view) in views {
            let t0 = std::time::Instant::now();
            let cpu = b.plan(view).expect("a CPU plan");
            let cpu_ms = t0.elapsed().as_secs_f64() * 1e3;
            let t0 = std::time::Instant::now();
            let gpu = b
                .plan_for(&gj.flame, view, PlanOptions { gpu: Some(&planner), ..Default::default() })
                .expect("a GPU plan");
            let gpu_ms = t0.elapsed().as_secs_f64() * 1e3;
            cpu_total += cpu_ms;
            gpu_total += gpu_ms;
            let same = {
                let a: std::collections::HashSet<&Vec<u32>> = cpu.words.iter().map(|w| &w.word).collect();
                gpu.words.iter().filter(|w| a.contains(&w.word)).count()
            };
            // 1e6 is past any independent chaos game: 400M iterations
            // landed none there, and each try costs a minute.
            let reachable = !name.starts_with("x1e6");
            let measure = |p: &Cylinders| reachable.then(|| coverage(&b, p, view, 3000)).flatten();
            let (cc, gc) = (measure(&cpu), measure(&gpu));
            let shown = |c: Option<f64>| c.map_or("n/a (unreachable)".to_string(), |c| format!("{c:.4}"));
            println!(
                "== {name}: CPU {cpu_ms:>5.0} ms, {:>5} words, eff {:.3}, coverage {} | \
                 GPU {gpu_ms:>5.0} ms, {:>5} words, eff {:.3}, coverage {} | {same} words in both | {:.1}x",
                cpu.words.len(),
                cpu.efficiency,
                shown(cc),
                gpu.words.len(),
                gpu.efficiency,
                shown(gc),
                cpu_ms / gpu_ms
            );
            if let (Some(cc), Some(gc)) = (cc, gc) {
                assert!(gc >= cc - 0.003, "{name}: the GPU's plan covers {gc:.4} where the CPU's covers {cc:.4}");
            }
        }
        println!("total: CPU {cpu_total:.0} ms, GPU {gpu_total:.0} ms, {:.1}x", cpu_total / gpu_total);
    }

    /// **Phase 4: the GPU gathers exactly what the CPU gathers.** Random
    /// gather jobs -- cell lists from runs of the sample with and without
    /// neighbours, every landing index and the grid, caps from 1 to past
    /// the total, and an empty cell list -- through `PlanGpu::gather_lands`
    /// (gathered and checked on the GPU) and `gather_on_cpu` over the same
    /// `PlanGpu` (gathered on the CPU, checked on the GPU). Same arithmetic
    /// for the checks, so the hits agree exactly iff the candidates do.
    #[test]
    #[ignore = "needs a GPU and reads output/flame-zoom"]
    fn the_gpu_gathers_as_the_cpu_does() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("grand-julian");
        let gj: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        let b = Backward::read(&gj.flame, reg).expect("armed");
        let (device, queue) = test_device();
        let mut planner = crate::scene::plan_gpu::GpuPlanner::new(&device, &queue);
        let gpu = planner.for_flame(&gj.flame, &b).expect("the kernel builds");
        assert!(gpu.gathers_here());

        let mut st = 0xFEED_u64;
        let mut rnd = move |n: usize| {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 33) as usize) % n.max(1)
        };
        // About half the attractor: the median distance from a point of it
        // (the extent is the farthest, and outliers make it huge). Some
        // candidates land and some do not.
        let centre = b.sample_point(0.3);
        let mut d: Vec<f64> = b.sample.iter().map(|p| (p[0] - centre[0]).hypot(p[1] - centre[1])).collect();
        d.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let view = View { centre, radius: d[d.len() / 2] };
        let mut seen_lists: Vec<Vec<Cell>> = Vec::new();
        for k in 0..40 {
            let start = rnd(b.sample.len() - 2000);
            let len = [1usize, 3, 20, 200, 1500][k % 5];
            let mut cells: Vec<Cell> = (start..start + len).map(|i| b.cell_of(b.sample[i])).collect();
            cells.sort_unstable();
            cells.dedup();
            seen_lists.push(Backward::expand_cells(&cells, k % 2 == 0));
        }
        seen_lists.push(Vec::new());
        let words: Vec<Vec<u32>> = (0..seen_lists.len())
            .map(|_| (0..1 + rnd(6)).map(|_| b.alphabet[rnd(b.alphabet.len())].sym).collect())
            .collect();
        let mut gathers: Vec<GatherJob> = Vec::new();
        for (k, (seen, word)) in seen_lists.iter().zip(&words).enumerate() {
            for index in [IndexId::Landing(rnd(b.alphabet.len())), IndexId::Landing(k % b.alphabet.len()), IndexId::Grid] {
                let cap = [1usize, 7, 64, CAND_CAP, TOPUP_CAP][rnd(5)];
                gathers.push(GatherJob { word, index, seen, cap });
            }
        }
        let replay: Vec<EvalJob> = words.iter().take(5).map(|w| EvalJob { word: w, points: &b.verify_first }).collect();

        let (a_gpu, g_gpu) = gpu.gather_lands(&b, view, &replay, &gathers);
        let (a_cpu, g_cpu) = gather_on_cpu(gpu, &b, view, &replay, &gathers);
        assert_eq!(a_gpu, a_cpu, "the plain answers differ");
        let mut total = 0usize;
        let mut hits = 0usize;
        for (k, (x, y)) in g_gpu.iter().zip(&g_cpu).enumerate() {
            assert_eq!(x.cands, y.cands, "gather {k} ({:?}, {} cells, cap {}): candidate count", gathers[k].index, gathers[k].seen.len(), gathers[k].cap);
            assert_eq!(x.hits, y.hits, "gather {k}: the hits differ");
            total += x.cands;
            hits += x.hits.len();
        }
        println!("{} gathers, {total} candidates, {hits} landed: identical", gathers.len());
        // The same, answered in f64 on the CPU: rounding may move a few.
        let (a_f64, g_f64) = gather_on_cpu(&mut CpuEval, &b, view, &replay, &gathers);
        let hits_f64: usize = g_f64.iter().map(|g| g.hits.len()).sum();
        println!(
            "f64: {hits_f64} landed; plain answers {} of {} land on the GPU, {} in f64",
            a_gpu.iter().filter(|x| **x == 1).count(),
            a_gpu.len(),
            a_f64.iter().filter(|x| **x == 1).count()
        );
        assert!(total > 10_000 && hits > 0 && hits < total, "the test gathered too little to mean anything");
    }

    /// **Phase 0 and 1 of `gpu-cylinder-planning.md`: does the GPU give
    /// the CPU's answers, and how fast?**
    ///
    /// Real words from real plans (the kept words and every expanded
    /// node's word), applied to the planner's own verification points
    /// and to random sample points, on both sides. Agreement is counted
    /// per point; time is compared at the batch sizes a plan's levels
    /// actually have. `FFLAME=path` adds a saved view.
    #[test]
    #[ignore = "needs a GPU and reads output/flame-zoom"]
    fn the_gpu_answers_as_the_cpu_does() {
        use crate::scene::plan_gpu::{EvalJob, PlanGpu};
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("grand-julian");
        let gj: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        let b = Backward::read(&gj.flame, reg).expect("armed");

        let (device, queue) = test_device();
        let t0 = std::time::Instant::now();
        let mut gpu = PlanGpu::new(&device, &queue, &gj.flame, &b.sample);
        println!("PlanGpu::new (shader + upload): {:.0} ms", t0.elapsed().as_secs_f64() * 1e3);

        let mut views: Vec<(String, View)> = Vec::new();
        for z in [1e2f64, 1e3, 1e4, 1e6] {
            views.push((format!("x{z:.0e}"), View::of(z, b.sample_point(0.75), 1280, 720)));
        }
        if let Ok(path) = std::env::var("FFLAME") {
            let c: crate::config::FractalConfig =
                serde_json::from_str(&std::fs::read_to_string(&path).expect("file")).expect("config");
            views.push(("saved".into(), View::of(c.zoom as f64, [c.pan_x as f64, c.pan_y as f64], 1280, 720)));
        }

        let stride = (b.sample.len() / VERIFY).max(1);
        let verify_pts: Vec<u32> = (0..VERIFY).map(|k| (k * stride) as u32).collect();
        let mut st = 0x1234_5678u64;
        let mut rnd = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (st >> 33) as u32
        };
        for (name, view) in views {
            let mut tr = Trace { record_expanded: true, ..Default::default() };
            let plan = b.plan_with(view, &mut tr).expect("a plan");
            let mut words: Vec<Vec<u32>> = plan.words.iter().map(|c| c.word.clone()).collect();
            words.extend(tr.expanded.into_iter().filter(|w| !w.is_empty()));
            // Checks go to random points; replays to the verification ones.
            let check_pts: Vec<Vec<u32>> = words
                .iter()
                .map(|_| (0..256).map(|_| rnd() % b.sample.len() as u32).collect())
                .collect();
            let mut jobs: Vec<EvalJob> = Vec::new();
            for (w, cp) in words.iter().zip(&check_pts) {
                jobs.push(EvalJob { word: w, points: &verify_pts });
                jobs.push(EvalJob { word: w, points: cp });
            }
            let entries: usize = jobs.iter().map(|j| j.points.len()).sum();
            let symbols: usize = jobs.iter().map(|j| j.word.len() * j.points.len()).sum();

            // Best of five: a short burst on an idle GPU runs at whatever
            // clock it was idling at, and the first runs measure the ramp.
            // Only the FIRST view's times are clean: after the CPU's runs
            // below have held twelve cores for a second, the next view's
            // GPU batches measured 2-3x slower in whichever order the
            // views ran (packing, on the CPU, slowed as much). Run with one
            // view (FFLAME and a reordered list) to time another zoom.
            let mut got = Vec::new();
            let mut gpu_ms = f64::INFINITY;
            for _ in 0..5 {
                let t0 = std::time::Instant::now();
                let g = gpu.evaluate(view, &jobs);
                gpu_ms = gpu_ms.min(t0.elapsed().as_secs_f64() * 1e3);
                assert!(got.is_empty() || got == g, "the GPU is not deterministic");
                got = g;
            }
            let mut want = Vec::new();
            let mut cpu_ms = f64::INFINITY;
            for _ in 0..3 {
                let t0 = std::time::Instant::now();
                want = {
                    use rayon::prelude::*;
                    let bb = &b;
                    jobs.par_iter()
                        .flat_map_iter(|j| j.points.iter().map(move |&i| bb.lands(j.word, bb.sample[i as usize], view) as u8))
                        .collect::<Vec<u8>>()
                };
                cpu_ms = cpu_ms.min(t0.elapsed().as_secs_f64() * 1e3);
            }

            let agree = got.iter().zip(&want).filter(|(a, b)| a == b).count();
            let (gpu_only, cpu_only) = got.iter().zip(&want).fold((0, 0), |(g, c), (a, b)| {
                (g + (*a == 1 && *b == 0) as usize, c + (*a == 0 && *b == 1) as usize)
            });
            let hits = want.iter().filter(|x| **x == 1).count();
            println!(
                "== {name}: {} words, {entries} points, {:.1}M symbol applications, {hits} CPU hits
                    per point: agreement {:.5} (GPU-only hits {gpu_only}, CPU-only hits {cpu_only})
                    GPU {gpu_ms:.1} ms (last: pack {:.1}, wait {:.1}, read {:.1}) | CPU (12 threads) {cpu_ms:.1} ms | {:.1}x",
                words.len(),
                symbols as f64 / 1e6,
                agree as f64 / entries as f64,
                gpu.last.pack,
                gpu.last.wait,
                gpu.last.read,
                cpu_ms / gpu_ms
            );

            // **Where the answers part.** The GPU's end points against the
            // CPU's, in view radii. Rounding moves a point by a hair and
            // disagrees only at the rim -- the f32 render puts that point
            // on the GPU's side of it too. Anything moved further is a
            // different point: a branch taken the other way.
            let ends = gpu.endpoints(&jobs);
            let cpu_ends: Vec<Option<[f64; 2]>> = jobs
                .iter()
                .flat_map(|j| j.points.iter().map(move |&i| (j.word, i)))
                .map(|(w, i)| b.forward_along(w, b.sample[i as usize]))
                .collect();
            let moved = |k: usize| -> f64 {
                match (ends[k], cpu_ends[k]) {
                    (Some(g), Some(c)) => (g[0] as f64 - c[0]).hypot(g[1] as f64 - c[1]) / view.radius,
                    (None, None) => 0.0,
                    _ => f64::INFINITY,
                }
            };
            let edges = [1e-4, 1e-3, 1e-2, 1e-1, 1.0, 10.0];
            let mut all = [0usize; 8];
            let mut dis = [0usize; 8];
            let (mut structural, mut rim) = (0usize, 0usize);
            for k in 0..entries {
                let d = moved(k);
                let bk = if d.is_infinite() { 7 } else { edges.iter().position(|e| d < *e).unwrap_or(6) };
                all[bk] += 1;
                structural += (d >= 0.1) as usize;
                if got[k] != want[k] {
                    dis[bk] += 1;
                    if d < 0.1 {
                        if let Some(c) = cpu_ends[k] {
                            let from_rim = ((c[0] - view.centre[0]).hypot(c[1] - view.centre[1]) - view.radius).abs();
                            rim += (from_rim <= 2.0 * d * view.radius + 1e-12) as usize;
                        }
                    }
                }
            }
            let at = (view.centre[0].abs().max(view.centre[1].abs()) as f32).to_bits();
            let ulp = (f32::from_bits(at + 1) - f32::from_bits(at)) as f64;
            // The frame's pixel: `View::of` spans 4/zoom across the short side.
            let pixel = view.radius / (1280f64.hypot(720.0) / 2.0);
            println!("   f32 step at the centre: {:.2e} r = {:.2} px", ulp / view.radius, ulp / pixel);
            println!("   |gpu - cpu| in radii:  <1e-4    <1e-3    <1e-2    <1e-1       <1      <10     >=10  one-bad");
            println!("     every point   {}", all.iter().map(|n| format!("{n:>8}")).collect::<Vec<_>>().join(" "));
            println!("     disagreeing   {}", dis.iter().map(|n| format!("{n:>8}")).collect::<Vec<_>>().join(" "));
            let small = dis[..4].iter().sum::<usize>();
            println!(
                "   disagreements moved < 0.1 r: {small}, of which {rim} sit within twice their move of the rim;                  moved >= 0.1 r (a different point): {structural} = {:.1e} of all points",
                structural as f64 / entries as f64
            );

            // **Replay shares**: each word's verification job, both sides.
            let mut off = 0usize;
            let mut worst = 0.0f64;
            let mut within = 0usize;
            let mut shown = 0usize;
            // The decision a share makes: keep at `CUT_EFFICIENCY`, or carry.
            let mut same_call = 0usize;
            for (j, job) in jobs.iter().enumerate() {
                let n = job.points.len();
                if j % 2 == 0 {
                    let g = got[off..off + n].iter().filter(|x| **x == 1).count() as f64 / n as f64;
                    let c = want[off..off + n].iter().filter(|x| **x == 1).count() as f64 / n as f64;
                    worst = worst.max((g - c).abs());
                    within += ((g - c).abs() <= 0.01) as usize;
                    same_call += ((g >= CUT_EFFICIENCY) == (c >= CUT_EFFICIENCY)) as usize;
                    if (g - c).abs() > 0.01 && shown < 3 {
                        shown += 1;
                        // How big the word's image is, and how far each point moved.
                        let pts: Vec<[f64; 2]> = cpu_ends[off..off + n].iter().flatten().copied().collect();
                        let spread = spread_of(&pts) / view.radius;
                        let mv = (off..off + n).map(&moved).filter(|d| d.is_finite()).fold(0.0, f64::max);
                        println!(
                            "     word len {:>2}: share cpu {c:.3} gpu {g:.3}; image spread {spread:.1e} r, largest move {mv:.1e} r",
                            job.word.len()
                        );
                    }
                }
                off += n;
            }
            let share_ok = within as f64 / words.len() as f64;
            let call_ok = same_call as f64 / words.len() as f64;
            println!(
                "   replay share within 1/100 of the CPU's: {share_ok:.4} of words (worst {worst:.4});                  the same keep-or-carry call: {call_ok:.4}"
            );

            // The gate, where the render can still resolve a pixel.
            if ulp < pixel {
                assert!(
                    (structural as f64) < 1e-4 * entries as f64,
                    "{name}: {structural} of {entries} points land somewhere else on the GPU"
                );
                assert_eq!(small, rim, "{name}: a disagreement that is not at the rim");
                // Not the share itself: a word whose whole image is a speck
                // on the rim, 1e-4 r across, shifts its share by the rim's
                // rounding (measured: 0.02-0.05, one 0.235), and that only
                // moves an efficiency estimate. The call the share makes is
                // what changes a plan.
                assert!(call_ok >= 0.99, "{name}: keep-or-carry differs for {:.3} of words", 1.0 - call_ok);
            } else {
                println!("   (past the render's f32 ceiling here: reported, not gated)");
            }
        }

        // Latency: one small job, as the smallest round trip a level needs.
        let w = vec![sym_of(0, 0)];
        let pts: Vec<u32> = (0..100).collect();
        let view = View::of(1e3, b.sample_point(0.75), 1280, 720);
        let mut best = f64::INFINITY;
        for _ in 0..20 {
            let t0 = std::time::Instant::now();
            let _ = gpu.evaluate(view, &[EvalJob { word: &w, points: &pts }]);
            best = best.min(t0.elapsed().as_secs_f64() * 1e3);
        }
        println!("round trip, one job of 100 points: {best:.2} ms (best of 20)");
    }

    /// **Plans, written down exactly**: `PLAN_DUMP=dir` writes the plan
    /// of every view in a fixed set to `dir/<view>.txt` -- each word's
    /// symbols and the bits of its probability, then the plan's totals
    /// -- so two builds' plans can be diffed. A word's centre and radius
    /// are left out. `PLAN_GPU=1` plans through the GPU evaluator.
    #[test]
    #[ignore = "reads output/flame-zoom; writes $PLAN_DUMP"]
    fn dump_plans() {
        use std::fmt::Write as _;
        let Ok(dir) = std::env::var("PLAN_DUMP") else { return };
        std::fs::create_dir_all(&dir).expect("the dump directory");
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut flames: Vec<(String, crate::config::FractalConfig)> = Vec::new();
        for name in ["grand-julian", "julian-disc", "random1"] {
            if let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")) {
                flames.push((name.to_string(), serde_json::from_str(&text).expect("a config")));
            }
        }
        let saved = "output/grand-julian-missing-pieces.fflame";
        let saved_cfg: Option<crate::config::FractalConfig> =
            std::fs::read_to_string(saved).ok().map(|t| serde_json::from_str(&t).expect("a config"));
        let gpu = std::env::var("PLAN_GPU").is_ok().then(|| {
            let (device, queue) = test_device();
            std::sync::Mutex::new(crate::scene::plan_gpu::GpuPlanner::new(&device, &queue))
        });
        let t0 = std::time::Instant::now();
        for (name, cfg) in &flames {
            let Ok(b) = Backward::read(&cfg.flame, reg) else {
                println!("{name}: refused");
                continue;
            };
            let mut views: Vec<(String, View)> = Vec::new();
            for z in [1e2f64, 1e3, 1e4, 1e6] {
                for frac in [0.25f64, 0.75] {
                    views.push((format!("{name}-x{z:.0e}-at{frac}"), View::of(z, b.sample_point(frac), 1280, 720)));
                }
            }
            if name == "grand-julian" {
                if let Some(c) = &saved_cfg {
                    views.push((
                        "grand-julian-missing-pieces".into(),
                        View::of(c.zoom as f64, [c.pan_x as f64, c.pan_y as f64], 1280, 720),
                    ));
                }
            }
            for (vname, view) in views {
                let plan = b.plan_for(&cfg.flame, view, PlanOptions { gpu: gpu.as_ref(), ..Default::default() });
                let mut out = String::new();
                match plan {
                    Ok(p) => {
                        for w in &p.words {
                            let _ = writeln!(out, "{:?} {:016x}", w.word, w.prob.to_bits());
                        }
                        let _ = writeln!(
                            out,
                            "words {} mass {:016x} lost {:016x} efficiency {:016x} depth {}",
                            p.words.len(),
                            p.mass.to_bits(),
                            p.lost.to_bits(),
                            p.efficiency.to_bits(),
                            p.depth
                        );
                    }
                    Err(e) => {
                        let _ = writeln!(out, "no plan: {e:?}");
                    }
                }
                std::fs::write(format!("{dir}/{vname}.txt"), out).expect("write");
            }
        }
        println!("dumped in {:.1} s", t0.elapsed().as_secs_f64());
    }

    /// **Why does the walk come back empty -- or incomplete** -- for
    /// julian-disc and random1? (`inversive-targeting.md` §31.) Per view:
    /// the sample points in view and the symbols they came through, the
    /// plan's coverage against an independent chaos game, and the recent
    /// past of the samples it misses beside the sample's own. `WATCH=t.a,...`
    /// follows a word suffix through julian-disc's 1e2 walk.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn why_is_this_view_empty() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for (name, zoom, frac) in [("julian-disc", 1e3f64, 0.25f64), ("julian-disc", 1e2, 0.25), ("random1", 1e4, 0.25), ("random1", 1e3, 0.25)] {
            let text = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")).expect("flame");
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
            let b = Backward::read(&cfg.flame, reg).expect("armed");
            let q = b.sample_point(frac);
            let view = View::of(zoom, q, 1280, 720);
            // How much of the sample is in the view at all, and through
            // which symbols it arrived.
            let inside: Vec<usize> = (0..b.sample.len())
                .filter(|&i| (b.sample[i][0] - q[0]).hypot(b.sample[i][1] - q[1]) <= view.radius)
                .collect();
            let mut by: std::collections::BTreeMap<String, usize> = Default::default();
            for &i in &inside {
                let ai = b.made_by[i];
                let k = if ai == u32::MAX { "reseed".to_string() } else {
                    let a = &b.alphabet[ai as usize];
                    format!("t{}a{}", sym_transform(a.sym), sym_arm(a.sym))
                };
                *by.entry(k).or_default() += 1;
            }
            let mut tr = Trace::default();
            // Every word, watched -- or one lost branch, where set.
            tr.watch = Some(match std::env::var("WATCH") {
                Ok(w) if name == "julian-disc" && zoom == 1e2 => w
                    .split(',')
                    .map(|t| {
                        let (a, b) = t.split_once('.').unwrap();
                        sym_of(a.parse().unwrap(), b.parse().unwrap())
                    })
                    .collect(),
                _ => Vec::new(),
            });
            let plan = b.plan_with(view, &mut tr);
            if plan.is_err() || std::env::var("WATCH").is_ok() && name == "julian-disc" && zoom == 1e2 {
                for line in tr.watched.iter().take(80) {
                    println!("   watch: {line}");
                }
            }
            println!(
                "== {name} x{zoom:.0e} at {frac}: centre [{:.5}, {:.5}] r {:.2e}, extent {:.3}, {} alphabet, {} sample points in view by {:?}",
                q[0], q[1], view.radius, b.extent, b.alphabet.len(), inside.len(), by
            );
            if let Ok(p) = &plan {
                println!("   coverage {:?}, efficiency {:.3}, depth {}", coverage(&b, p, view, 3000), p.efficiency, p.depth);
                // What the plan misses: the in-view samples' recent past.
                let total: f64 = b.transforms.iter().map(|t| t.weight).sum();
                let mut st = 0xC0FFEE_u64;
                let mut lcg = move || {
                    st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    ((st >> 33) as f64) / ((1u64 << 31) as f64)
                };
                let mut x = [0.31f64, 0.17];
                let mut hist: Vec<u32> = Vec::new();
                let longest = p.words.iter().map(|w| w.word.len()).max().unwrap_or(1);
                let mut missed: std::collections::HashMap<Vec<u32>, usize> = Default::default();
                let mut lens: std::collections::BTreeMap<usize, usize> = Default::default();
                for w in &p.words {
                    *lens.entry(w.word.len()).or_default() += 1;
                }
                let show = |w: &[u32]| -> String {
                    let mut out: Vec<String> = Vec::new();
                    let mut run = 0usize;
                    for s in w {
                        if *s == sym_of(0, 0) {
                            run += 1;
                            continue;
                        }
                        if run > 0 {
                            out.push(format!("t0^{run}"));
                            run = 0;
                        }
                        out.push(format!("t{}a{}", sym_transform(*s), sym_arm(*s)));
                    }
                    if run > 0 {
                        out.push(format!("t0^{run}"));
                    }
                    out.join(" ")
                };
                let (mut n_in, mut n_miss) = (0usize, 0usize);
                for k in 0..200_000_000usize {
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
                    n_in += 1;
                    let hit = p.words.iter().any(|w| {
                        let k = w.word.len();
                        hist.len() >= k && hist[hist.len() - k..] == w.word[..]
                    });
                    if !hit {
                        n_miss += 1;
                        // The last few symbols, most recent last, with t0
                        // runs collapsed so the branches show.
                        let tail: Vec<u32> = hist[hist.len().saturating_sub(24)..].to_vec();
                        *missed.entry(tail).or_default() += 1;
                    }
                    if n_in >= 1500 {
                        break;
                    }
                }
                println!("   word lengths in the plan: {lens:?}");
                // The planner's own sample: the histories of its points in
                // the view, back 30 steps, by the same collapsed notation.
                let mut hist_s: std::collections::HashMap<String, usize> = Default::default();
                let inside_s: Vec<usize> = (0..b.sample.len())
                    .filter(|&i| (b.sample[i][0] - view.centre[0]).hypot(b.sample[i][1] - view.centre[1]) <= view.radius)
                    .collect();
                for &i in &inside_s {
                    let mut syms: Vec<u32> = Vec::new();
                    let mut j = i;
                    while syms.len() < 24 && j > 0 && b.made_by[j] != u32::MAX {
                        syms.push(b.alphabet[b.made_by[j] as usize].sym);
                        j -= 1;
                    }
                    syms.reverse();
                    *hist_s.entry(show(&syms)).or_default() += 1;
                }
                let mut hs: Vec<_> = hist_s.into_iter().collect();
                hs.sort_by(|a, b| b.1.cmp(&a.1));
                println!("   the planner's own {} in-view sample points, last 24 symbols:", inside_s.len());
                for (w, n) in hs.iter().take(8) {
                    println!("     {n:>4}: {w}");
                }
                println!("   {n_miss} of {n_in} in-view samples missed; their last 24 symbols (most recent last):");
                let mut m: Vec<_> = missed.into_iter().collect();
                m.sort_by(|a, b| b.1.cmp(&a.1));
                for (w, n) in m.iter().take(10) {
                    println!("     {n:>4}: {}", show(w));
                }
            }
            println!(
                "   plan: {} | nodes {} seeded {} pruned {} no_preimage {} no_arm {} floor {} cut {} carried {} empty {} nocand {} forced {}",
                match &plan { Ok(p) => format!("{} words", p.words.len()), Err(e) => format!("{e:?}") },
                tr.nodes_expanded, tr.seeded, tr.pruned, tr.no_preimage, tr.no_arm, tr.floor, tr.cut, tr.not_yet, tr.empty, tr.nocand, tr.forced
            );
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
