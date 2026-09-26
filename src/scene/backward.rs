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
use super::final_map::FinalMap;
use super::ifs_analysis::{analyse_2d_maps_blurred_sliced, Blur, Ifs2, IfsMap, Kernel, Map2, PreBlur};
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

/// A child that lands but whose region yielded no sample point is forced
/// as it stands only if that wastes at most this fraction of the mass
/// already kept -- its probability times the share of it that misses the
/// view. Above it, it is carried on from the points its replay landed.
/// See `close`.
pub const FORCE_WASTE: f64 = 0.01;

/// The most answer words one speculative batch asks for (a child's
/// replays and, for an index child, its gather's slots). A level wider
/// than this is asked in several, each read back on its own.
pub const FUSED_WORDS: usize = 1 << 20;

/// The most points a pulled-back cloud keeps. The view's own cloud is 64;
/// following every branch of an inverse can multiply it.
pub const CLOUD_CAP: usize = 256;

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

/// A rescue (`Backward::rescue`) looks near this many of a child's
/// candidates, nearest first, at each of these cylinder depths, with
/// `RESCUE_POINTS` points at each.
pub const RESCUE_CANDIDATES: usize = 8;
pub const RESCUE_DEPTHS: std::ops::RangeInclusive<usize> = 4..=16;
pub const RESCUE_POINTS: usize = 128;
/// How many levels a rescued cloud's descendants are pulled back without
/// the grid's test: each level widens the region by the maps' expansion,
/// until the sample holds points of it and seeds it.
pub const RESCUE_LEVELS: u8 = 6;
/// How far past the node's own cells an unseen child's near misses are
/// gathered, in cells, and from how many of them.
pub const UNSEEN_REACH: i32 = 4;
pub const WIDEN_FROM: usize = 64;
/// An indexed node's points pulled back for the pieces its sample does
/// not show (`Node::alt`): this many of them, and this many kept.
pub const ALT_FROM: usize = 16;
pub const ALT_CAP: usize = 32;

/// Map applications a plan may spend replaying its costliest blur words
/// (see the walk's end): about a quarter of a second on one core.
pub const RENEWAL_REPLAYS: usize = 4 << 20;

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
    /// Pieces of the picture removed (`docs/projects/word-editing.md`
    /// §5): patterns of symbols matched against a word's last-applied
    /// maps (`scene::word_tree::removed`). The walk makes no word ending
    /// with one, and refines a word that holds one rather than keeping it
    /// whole.
    pub removals: &'a [Vec<u32>],
    /// Pieces opened in the Pieces panel to see inside
    /// (`docs/projects/word-editing.md` §6): the walk splits each, and
    /// every piece holding one, rather than keeping it whole. The picture
    /// is the same; only how finely the plan divides it changes.
    pub refine: &'a [Vec<u32>],
    /// Split every word shorter than this (PathMap's level, docs/projects/
    /// word-editing.md §10): its colour reads that many maps of a path, and
    /// zoomed out the plan's paths are one map long.
    pub min_len: usize,
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
            removals: &[],
            refine: &[],
            min_len: 0,
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

/// A boxed future the walk awaits. Not `Send`: a plan runs on one thread
/// -- a worker's on the desktop, the page's own on the web.
pub type Ask<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;

/// **The walk's evaluator, as the walk awaits it** (phase 3 of
/// `gpu-cylinder-planning.md`). On the desktop every answer is ready at
/// once ([`Blocking`]); on the web the GPU's come back on a later frame,
/// and the walk, being a future, simply resumes there.
pub trait AskEval {
    fn speculative(&self) -> bool;
    fn lands<'a>(&'a mut self, b: &'a Backward, view: View, jobs: &'a [EvalJob<'a>]) -> Ask<'a, Vec<u8>>;
    fn gather_lands<'a>(
        &'a mut self,
        b: &'a Backward,
        view: View,
        jobs: &'a [EvalJob<'a>],
        gathers: &'a [GatherJob<'a>],
    ) -> Ask<'a, (Vec<u8>, Vec<Gathered>)>;
}

/// A synchronous [`Evaluate`], answered at once.
pub struct Blocking<'e>(pub &'e mut dyn Evaluate);

impl AskEval for Blocking<'_> {
    fn speculative(&self) -> bool {
        self.0.speculative()
    }

    fn lands<'a>(&'a mut self, b: &'a Backward, view: View, jobs: &'a [EvalJob<'a>]) -> Ask<'a, Vec<u8>> {
        let r = self.0.lands(b, view, jobs);
        Box::pin(std::future::ready(r))
    }

    fn gather_lands<'a>(
        &'a mut self,
        b: &'a Backward,
        view: View,
        jobs: &'a [EvalJob<'a>],
        gathers: &'a [GatherJob<'a>],
    ) -> Ask<'a, (Vec<u8>, Vec<Gathered>)> {
        let r = self.0.gather_lands(b, view, jobs, gathers);
        Box::pin(std::future::ready(r))
    }
}

pub use super::slice::{drive, Slicer, Tick};

/// [`map_all`], sliced: in order with ticks between items where the
/// slicer slices, in parallel where it does not.
async fn map_sliced<T: Sync, U: Send>(items: &[T], f: impl Fn(&T) -> U + Sync + Send, slicer: &Slicer) -> Vec<U> {
    if !slicer.slices() {
        return map_all(items, f);
    }
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        out.push(f(it));
        slicer.tick().await;
    }
    out
}

/// [`each_mut`], sliced. See [`map_sliced`].
async fn each_sliced<T: Send>(items: &mut [T], f: impl Fn(&mut T) + Sync + Send, slicer: &Slicer) {
    if !slicer.slices() {
        each_mut(items, f);
        return;
    }
    for it in items.iter_mut() {
        f(it);
        slicer.tick().await;
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
    /// Words kept whole although they hold a removed piece (see
    /// `PlanOptions::removals`): at the depth cap, a renewal, or with no
    /// points to refine them from. The removed piece stays in these.
    pub unrefined: usize,
    /// Children never made because their piece was removed.
    pub removed: usize,
    /// Blur words dropped as negligible: no landing in a long replay,
    /// and a bound under 1% of the rest of the view.
    pub renewal_dropped: usize,
    /// Children with near misses found by looking closer (`rescue`).
    pub rescued: usize,
    /// Children carried from points of a piece the sample does not show
    /// (`Node::alt`).
    pub hidden: usize,
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
    /// Sorted by cell, then sample index: `(Cell, u32)`'s own order.
    ///
    /// **By radix.** The entries arrive in sample order, so a STABLE sort
    /// by cell alone gives exactly that order, and a least-significant-
    /// digit radix sort of the cell's 64 bits, sixteen at a time -- with
    /// a digit every entry shares skipped -- is at most four passes.
    /// Comparison-sorting 100k tuples was the largest single piece of
    /// building the walk: 14-20 ms each, one per alphabet symbol, which a
    /// web frame cannot hold.
    #[cfg(test)]
    fn build(entries: Vec<(Cell, u32)>) -> Self {
        drive(Self::build_sliced(entries, &Slicer::never()))
    }

    /// [`Self::build`], ticking between passes: one index is 5-9 ms of
    /// the analysis natively, too much of a web frame in one piece.
    async fn build_sliced(mut entries: Vec<(Cell, u32)>, slicer: &Slicer) -> Self {
        if !entries.windows(2).all(|w| w[0].1 < w[1].1) {
            entries.sort_unstable();
            return Self { entries };
        }
        let key = |c: Cell| (((c.0 as u32) ^ 0x8000_0000) as u64) << 32 | ((c.1 as u32) ^ 0x8000_0000) as u64;
        let keys: Vec<u64> = entries.iter().map(|e| key(e.0)).collect();
        let n = entries.len();
        let mut perm: Vec<u32> = (0..n as u32).collect();
        let mut next: Vec<u32> = vec![0; n];
        let mut count = vec![0usize; 1 << 16];
        for pass in 0..4 {
            slicer.tick().await;
            let shift = 16 * pass;
            let digit = |i: u32| ((keys[i as usize] >> shift) & 0xFFFF) as usize;
            let first = keys.first().map_or(0, |k| (k >> shift) & 0xFFFF);
            if keys.iter().all(|k| (k >> shift) & 0xFFFF == first) {
                continue;
            }
            count.iter_mut().for_each(|c| *c = 0);
            for &i in &perm {
                count[digit(i)] += 1;
            }
            let mut at = 0usize;
            for c in count.iter_mut() {
                let k = *c;
                *c = at;
                at += k;
            }
            for &i in &perm {
                let d = digit(i);
                next[count[d]] = i;
                count[d] += 1;
            }
            std::mem::swap(&mut perm, &mut next);
        }
        slicer.tick().await;
        let entries = perm.iter().map(|&i| entries[i as usize]).collect();
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
    /// Every map the analysis made of this symbol's transform: one per
    /// branch of its inverse. See `children_of`'s cloud.
    branches: Vec<usize>,
    arm: u32,
    /// The probability the chaos game draws this transform AND this
    /// arm.
    prob: f64,
    /// Set for a blurred transform: see [`Renewal`].
    renewal: Option<Renewal>,
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
    /// Levels left in which this cloud's points are pulled back without
    /// `near_landing`'s test (`Backward::rescue`): they are points of the
    /// attractor in a part too sparse for the sample's grid to vouch for.
    rescued: u8,
    /// **Points of its region the sample does not show** (tracker C3). An
    /// indexed node's points are the sample's, and a piece of its region
    /// the sample never visited has none: another branch of a map's
    /// inverse -- `disc` past radius one -- can hold a share of the view
    /// at a measure no sample point reaches. The node's points pulled
    /// back along every branch, and a seeded cloud's points away from the
    /// sample's, are kept here, so the walk still finds the children
    /// through that piece. Empty for a cloud, which is pulled back along
    /// every branch itself.
    alt: Vec<[f64; 2]>,
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
    /// Its piece holds a removed one, or was opened to see inside
    /// (`PlanOptions::removals`, `refine`): carried on, never cut, so the
    /// walk splits it.
    refine: bool,
    /// Inherited from a rescued cloud (`Node::rescued`), one level less.
    rescued: u8,
    /// Its region's points in pieces the sample does not show
    /// (`Node::alt`): an indexed node's pulled back through this symbol.
    alt: Vec<[f64; 2]>,
    /// Candidates from the index, or the pulled-back cloud.
    pts: Pts,
    /// The node's points this symbol produced: in the child's region by
    /// construction, so never checked.
    orbit_hits: Vec<u32>,
    /// The replay: points landed of points tried.
    hit: usize,
    total: usize,
    /// The verification points its replay landed: sample points of its
    /// region, found for free. See `close`.
    replay_hits: Vec<u32>,
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

/// At most `cap` of `pts`, evenly through them.
fn thin(pts: Vec<[f64; 2]>, cap: usize) -> Vec<[f64; 2]> {
    if pts.len() <= cap {
        return pts;
    }
    let n = pts.len();
    (0..cap).map(|k| pts[k * n / cap]).collect()
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
        self.unrefined += o.unrefined;
        self.removed += o.removed;
        self.renewal_dropped += o.renewal_dropped;
        self.rescued += o.rescued;
        self.hidden += o.hidden;
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
    /// The final transforms every point is plotted through, if any: the
    /// walk plans for the view pulled back through them
    /// (`FinalMap::pull_back`).
    finals: Option<FinalMap>,
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
    /// Its `pre_blur`, or its one variation that ignores its input,
    /// taken out of the maps by the analysis and drawn here -- only ever
    /// on a [`Renewal`].
    blur: Option<Blur>,
    /// Set when it is blurred: see [`Renewal`].
    renewal: Option<Renewal>,
    /// All of the transform's maps, `map` first.
    branches: Vec<usize>,
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

/// **The replay in offsets** (`docs/projects/deep-zoom-precision.md` §2):
/// a forced sample runs its word in absolute f32 up to step `m`, then as
/// an offset from a reference orbit the CPU ran in f64.
///
/// **What one absolute step costs on a GPU**, as a share of the point's
/// distance from the origin: not f32's rounding (6e-8) but the GPU's
/// transcendental functions, which are accurate to an ABSOLUTE error
/// near 1e-7 over a turn. Measured on the plain replay
/// (`how_far_the_plain_replay_is_from_f64`, GTX 1660 SUPER): about 4e-7
/// world units at every depth on grand-julian and julian-disc, a
/// systematic displacement, not noise. Taken with margin.
pub const GPU_STEP_ERROR: f64 = 4e-6;

/// What the replay may be off by at the end, as a share of the view's
/// radius: a twentieth of a pixel on a 4K frame (half-diagonal 2200 px).
pub const PLOT_TOLERANCE: f64 = 0.05 / 2200.0;

/// `m` is the last step at which an absolute step's error, carried
/// through the rest of the word -- the largest singular value of the
/// remaining steps' Jacobian, along the reference -- stays within
/// [`PLOT_TOLERANCE`]. Taking the largest matters: disc stretches its
/// radius and shrinks its angle by very different factors, and an
/// error in the stretched direction is what reaches the picture.
///
/// A plan needs offsets at all once the plain replay's own last step
/// misses the tolerance: under this share of the view centre's distance
/// from the origin (or the attractor's scale).
pub const SWITCH_SHARE: f64 = 0.2;

/// A word's reference chains at most: one per separate cluster of its
/// landed points at `m`. A word reached from several regions -- the
/// walk merges the preimage branches of one symbol into one word --
/// needs one in each, since an offset from the wrong one is as large as
/// the distance between them, and f32 of that is nothing at depth.
pub const MAX_CHAINS: usize = 4;

/// Points of its region a kept word carries for its references.
const REF_SEEDS: usize = 8;

/// **Where the GPU planner stops** (`deep-zoom-precision.md` §8): its
/// replays and landing checks are absolute, and as displaced as the plain
/// replay -- about 4e-7 world units near unit scale. A view of radius
/// under this (times the view centre's distance from the origin, where
/// that is over one) is planned on the CPU, in f64. The GPU planner was
/// measured complete at 1e6 on grand-julian (radius 4.1e-6, the error a
/// tenth of it); here the error would be a fifth. Offsets for the planner
/// are a later item.
pub const GPU_PLAN_RADIUS: f64 = 2e-6;

/// Tests only: plan without offsets, to measure what they are for.
#[cfg(test)]
pub static OFFSETS_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One word's references. See [`SWITCH_SHARE`].
#[derive(Debug, Clone, PartialEq)]
pub struct WordRefs {
    /// The first symbol replayed as an offset; those before run in
    /// absolute f32, as a replay did before.
    pub m: usize,
    pub chains: Vec<RefChain>,
}

/// One reference orbit from step `m` on.
#[derive(Debug, Clone, PartialEq)]
pub struct RefChain {
    /// The reference before each step taken as an offset: `z_m ... z_{n-1}`.
    pub bases: Vec<[f64; 2]>,
    /// Where it ends, less the view centre, in f64: `z_n − c`.
    pub end: [f64; 2],
}

/// **A blurred transform, planned as a renewal** (tracker items C2,
/// C2c). A `pre_blur` that reaches across the attractor's whole image in
/// the frame its kernel reads, and past the kernel's preimage of every
/// point it can output, forgets its input: every point of the attractor
/// has a positive chance of landing anywhere in its output, and the
/// region of a word it starts is the whole attractor. Its child is kept,
/// never carried, and kept by whether the node's region can reach `out`
/// at all -- not by replays, which at depth miss a smooth part that lands
/// one time in ten million.
///
/// **A smaller blur is planned the same way.** Keeping by geometry is
/// complete whatever the blur's reach: `out` holds everything the
/// transform can output, so a word dropped because the view's pull-back
/// misses it cannot land. The blurred symbol is only ever a word's first,
/// and every word after it is unblurred and walked as any other. What a
/// partial blur's word loses by not being carried is a refinement: at a
/// view smaller than its smear there is nothing to localize, and above it
/// the replays measure what it lands.
///
/// v1 knows one kernel, bubble, whose image is the unit disc. A
/// transform whose one variation ignores its input -- `blur`,
/// `gaussian_blur`, `pie`, `pie3D`, `starblur` (`FreeBlur`) -- is a
/// renewal by construction, its output the disc its draw lies in.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Renewal {
    /// A disc holding everything the transform can output.
    out_centre: [f64; 2],
    out_radius: f64,
}

/// Where bubble's radial profile `4r/(r²+4)` reaches the unit circle:
/// it sends the disc of radius 2 onto the unit disc.
const BUBBLE_PREIMAGE: f64 = 2.0;

/// The blur's draw, as JWF's `pre_blur` makes it: `weight·(six uniforms
/// − 3)` at a uniform angle.
fn blur_draw(b: PreBlur, u: &mut impl FnMut() -> f64) -> [f64; 2] {
    let g = b.weight * ((0..6).map(|_| u()).sum::<f64>() - 3.0);
    let a = u() * std::f64::consts::TAU;
    [g * a.cos(), g * a.sin()]
}

/// The forward map along arm `k`, with a blur drawn from `u` where the
/// transform has one. See [`Renewal`].
fn forward_blurred(map: &IfsMap<Map2>, x: [f64; 2], k: u32, blur: Option<Blur>, u: &mut impl FnMut() -> f64) -> [f64; 2] {
    let Some(b) = blur else { return forward(map, x, k) };
    match (b, &map.forward) {
        (Blur::Pre(b), Map2::Nonlinear(n)) => {
            let d = blur_draw(b, u);
            let v = n.pre.apply(x);
            let z = n.kernel.forward([v[0] + d[0], v[1] + d[1]], k);
            n.post.apply([n.w * z[0], n.w * z[1]])
        }
        // The analysis's stand-in holds the transform's post; the draw
        // is the variation's own.
        (Blur::Free(f), Map2::Nonlinear(n)) => n.post.apply(f.draw(u)),
        (_, other) => other.apply(x),
    }
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
        drive(Self::cached_sliced(flame, registry, &Slicer::never()))
    }

    /// Forget the cached analysis, so the next plan builds it again. For the
    /// tests that time the build.
    pub fn forget_cached() {
        if let Ok(mut c) = CACHE.lock() {
            *c = None;
        }
    }

    /// [`Self::cached`], yielding at `slicer`'s ticks while it builds: the
    /// build is a quarter of a second on the desktop, which the web cannot
    /// spend in one frame.
    pub async fn cached_sliced(
        flame: &Flame,
        registry: &VariationRegistry,
        slicer: &Slicer,
    ) -> Result<Arc<Self>, String> {
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
        let b = Arc::new(Self::read_sliced(flame, registry, slicer).await?);
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
        // The finals pull the view back, and a linked transform refuses
        // the flame: both are part of what the walk read.
        let json = serde_json::to_string(&(&live, &flame.final_transforms, &flame.linked_transforms)).unwrap_or_default();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        json.hash(&mut h);
        h.finish()
    }

    /// Analyse the flame, sample its attractor and index it. `Err`
    /// names what the inverse walk cannot do for this flame.
    pub fn read(flame: &Flame, registry: &VariationRegistry) -> Result<Self, String> {
        drive(Self::read_sliced(flame, registry, &Slicer::never()))
    }

    /// [`Self::read`], yielding at `slicer`'s ticks.
    pub async fn read_sliced(flame: &Flame, registry: &VariationRegistry, slicer: &Slicer) -> Result<Self, String> {
        slicer.tick().await;
        // **The finals are the plot's, not the orbit's** (C2c): the walk
        // follows them to pull the view back, and analyses the orbit's
        // maps without them.
        let finals = FinalMap::of(flame, registry)?;
        let mut body = flame.clone();
        body.final_transforms.clear();
        for t in &mut body.transforms {
            t.final_attachments.clear();
        }
        // The maps alone: the escape engine's bounds are never read here.
        // A `pre_blur` comes back beside them (tracker item C2).
        let (ifs, blurs) = analyse_2d_maps_blurred_sliced(&body, registry, slicer).await.map_err(|errs| {
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
        })?;
        slicer.tick().await;
        if ifs.maps.is_empty() {
            return Err("no maps".into());
        }
        let mut transforms: Vec<TransformInfo> = Vec::new();
        for (mi, m) in ifs.maps.iter().enumerate() {
            if let Some(t) = transforms.iter_mut().find(|t| t.index == m.transform_index) {
                t.branches.push(mi);
                continue;
            }
            let weight = flame
                .transforms
                .get(m.transform_index)
                .map_or(0.0, |t| (t.weight as f64).max(0.0));
            let blur = blurs.get(m.transform_index).copied().flatten();
            if let Some(b) = blur {
                Self::blur_planned(m, b).map_err(|why| format!("transform {}: {why}", m.transform_index))?;
            }
            transforms.push(TransformInfo {
                index: m.transform_index,
                weight,
                arms: forward_arms(m),
                map: mi,
                blur,
                // Its output disc wants the attractor: set below.
                renewal: None,
                branches: vec![mi],
            });
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
                    branches: t.branches.clone(),
                    arm,
                    prob: t.weight / total / t.arms as f64,
                    renewal: t.renewal,
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
            let y = forward_blurred(&ifs.maps[t.map], x, arm, t.blur, &mut lcg);
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
            if k % 4096 == 0 {
                slicer.tick().await;
            }
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
        // **The blurred transforms' output discs**, from the attractor
        // the blur itself draws: the stripped maps' ball need not hold
        // it, and an output disc drawn too small drops words that land.
        for t in &mut transforms {
            if let Some(b) = t.blur {
                t.renewal = Some(Self::renewal(&ifs.maps[t.map], b, centre, extent));
            }
        }
        for a in &mut alphabet {
            a.renewal = transforms.iter().find(|t| t.map == a.map).and_then(|t| t.renewal);
        }
        let cell = 2.0 * extent / GRID_CELLS as f64;
        let key = |p: [f64; 2]| -> Cell { ((p[0] / cell).floor() as i32, (p[1] / cell).floor() as i32) };
        slicer.tick().await;
        let grid = Index::build_sliced(sample.iter().enumerate().map(|(i, p)| (key(*p), i as u32)).collect(), slicer).await;
        slicer.tick().await;
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
                if i % 4096 == 0 {
                    slicer.tick().await;
                }
            }
            slicer.tick().await;
            // One index's sort: 100k entries, a few milliseconds.
            landing.push(Index::build_sliced(entries, slicer).await);
            slicer.tick().await;
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
            finals,
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
    /// Up to `REF_SEEDS` points of a kept word's region, spread across
    /// what the walk found -- the checked lists first, then its points
    /// or candidates, which [`Self::reference_chains`] checks anyway.
    fn seeds_from(&self, lists: &[&[u32]], pts: Option<&Pts>) -> Vec<[f64; 2]> {
        let mut out: Vec<[f64; 2]> = Vec::new();
        let spread = |n: usize, room: usize| n.div_ceil(room.max(1)).max(1);
        for idx in lists {
            let room = REF_SEEDS - out.len();
            if room == 0 {
                return out;
            }
            out.extend(idx.iter().step_by(spread(idx.len(), room)).take(room).map(|&i| self.sample[i as usize]));
        }
        let room = REF_SEEDS - out.len();
        match pts {
            Some(Pts::Index(idx)) if room > 0 => {
                out.extend(idx.iter().step_by(spread(idx.len(), room)).take(room).map(|&i| self.sample[i as usize]));
            }
            Some(Pts::Cloud(c)) if room > 0 => out.extend(c.iter().step_by(spread(c.len(), room)).take(room).copied()),
            _ => {}
        }
        out
    }

    /// Whether a blurred transform can be planned, or why not.
    fn blur_planned(m: &IfsMap<Map2>, b: Blur) -> Result<(), String> {
        let Map2::Nonlinear(n) = &m.forward else {
            return Err("pre_blur is planned beside one kernel alone (not an affine, not a sum) so far".into());
        };
        match b {
            Blur::Pre(_) if !matches!(n.kernel, Kernel::Bubble) => Err(format!("pre_blur is planned beside bubble so far, not {:?}", n.kernel)),
            _ => Ok(()),
        }
    }

    /// A blurred transform's [`Renewal`]: the disc holding everything it
    /// can output, given an attractor within `extent` of `centre`.
    fn renewal(m: &IfsMap<Map2>, b: Blur, centre: [f64; 2], extent: f64) -> Renewal {
        let Map2::Nonlinear(n) = &m.forward else { unreachable!("checked by blur_planned") };
        let (_, pmax) = crate::scene::ifs_analysis::singular_values_of(n.post.m);
        let out_centre = n.post.apply([0.0, 0.0]);
        match b {
            // A variation that ignores its input: its draw's disc (the
            // analysis's stand-in is `bubble` scaled to it).
            Blur::Free(f) => Renewal { out_centre, out_radius: pmax * f.weight.abs() * f.radius() },
            // Bubble's radial profile `4r/(r² + 4)` rises to 1 at r = 2:
            // an input that reaches no further than `r` comes out within
            // it. The input reaches the attractor's image in the kernel's
            // frame, and the blur's reach past that -- the image from
            // the sample, with a margin for the points it did not draw.
            Blur::Pre(b) => {
                let c = n.pre.apply(centre);
                let (_, smax) = crate::scene::ifs_analysis::singular_values_of(n.pre.m);
                let r = (c[0].hypot(c[1]) + 1.02 * smax * extent + b.reach()).min(BUBBLE_PREIMAGE);
                Renewal { out_centre, out_radius: pmax * n.w.abs() * 4.0 * r / (r * r + 4.0) }
            }
        }
    }

    /// The flame's forward maps as the shader's rows, one per transform
    /// index up to the highest walked; an index the walk does not use
    /// (weight zero) is the identity, which no word names.
    pub fn forward_rows(&self) -> Vec<f32> {
        use crate::scene::forward_delta::{forward_row, ROW_FLOATS};
        let top = self.transforms.iter().map(|t| t.index).max().unwrap_or(0);
        let mut rows = vec![0.0f32; (top + 1) * ROW_FLOATS];
        for i in 0..=top {
            rows[i * ROW_FLOATS + 16] = 1.0;
            rows[i * ROW_FLOATS + 19] = 1.0;
        }
        for t in &self.transforms {
            rows[t.index * ROW_FLOATS..(t.index + 1) * ROW_FLOATS].copy_from_slice(&forward_row(&self.ifs.maps[t.map].forward));
        }
        rows
    }

    /// **Every piece of a word's region at step `m`**: the preimages of
    /// `end` (where the word's reference lands) back through the word's
    /// symbols `m..n`, along EVERY branch of each inverse, confirmed
    /// forward with the symbol's arm, and kept only where the symbol
    /// before could have put a point (`near_landing`) -- a piece no path
    /// reaches receives no sample.
    ///
    /// Bubble and disc are many-to-one going forward, so a word's region
    /// can be several separate pieces, and the render's samples come from
    /// all of them. A sample carried as an offset from another piece's
    /// reference has an offset as large as the gap between them, and f32
    /// of that is nothing at depth. `None` past `cap` pieces.
    fn pieces(&self, word: &[u32], end: [f64; 2], m: usize, cap: usize) -> Option<Vec<[f64; 2]>> {
        let ais: Vec<usize> = word.iter().map(|&s| self.alphabet.iter().position(|a| a.sym == s)).collect::<Option<_>>()?;
        let junk_r = JUNK_EXTENTS * self.extent;
        let mut level = vec![end];
        for k in (m..word.len()).rev() {
            let a = &self.alphabet[ais[k]];
            let map = &self.ifs.maps[a.map];
            let mut next: Vec<[f64; 2]> = Vec::new();
            for &q in &level {
                for &mb in &a.branches {
                    let p = self.ifs.maps[mb].inverse.apply(q);
                    if !finite(p) || (p[0] - self.centre[0]).hypot(p[1] - self.centre[1]) > junk_r {
                        continue;
                    }
                    if self.arm_of(map, p, q) != Some(a.arm) {
                        continue;
                    }
                    if k > 0 && !self.near_landing(ais[k - 1], p) {
                        continue;
                    }
                    let scale = 1e-12 * (1.0 + p[0].abs().max(p[1].abs()));
                    if next.iter().any(|r| (r[0] - p[0]).hypot(r[1] - p[1]) <= scale) {
                        continue;
                    }
                    next.push(p);
                }
            }
            if next.len() > cap {
                return None;
            }
            level = next;
        }
        Some(level)
    }

    /// Whether a child's candidates are gathered: an index child that is
    /// not a renewal's. The gathers asked and the answers read back both
    /// go by this, so they cannot disagree.
    fn gathers(&self, c: &Child) -> bool {
        matches!(c.pts, Pts::Index(_)) && self.alphabet[c.ai].renewal.is_none()
    }

    /// Whether a blur starts `word`: the renewal is its first map.
    fn renewal_first(&self, word: &[u32]) -> bool {
        word.first().is_some_and(|s| self.alphabet.iter().any(|a| a.sym == *s && a.renewal.is_some()))
    }

    /// **Whether a renewal can land in `view` through `word`**: the view
    /// pulled back through the word, last symbol first, along every
    /// branch and arm, each piece's radius grown by the map's smallest
    /// stretch there (twice, for the linearization), and any piece
    /// reaching the renewal's output disc. Where the pull-back cannot be
    /// followed -- no preimage on any branch, or too many pieces -- it
    /// answers yes: a renewal kept in doubt costs efficiency, one
    /// dropped in doubt is a hole.
    fn reaches(&self, word: &[u32], view: View, ren: Renewal) -> bool {
        use crate::scene::forward_delta::map_forward_difference;
        const PIECES: usize = 64;
        let mut level: Vec<([f64; 2], f64)> = vec![(view.centre, view.radius)];
        for &sym in word.iter().rev() {
            let Some(a) = self.alphabet.iter().find(|a| a.sym == sym) else { return true };
            let map = &self.ifs.maps[a.map];
            let mut next: Vec<([f64; 2], f64)> = Vec::new();
            for &(q, r) in &level {
                for &mb in &a.branches {
                    let p = self.ifs.maps[mb].inverse.apply(q);
                    if !finite(p) || self.arm_of(map, p, q) != Some(a.arm) {
                        continue;
                    }
                    let h = 1e-6 * p[0].hypot(p[1]).max(1e-6);
                    let col = |d: [f64; 2]| map_forward_difference(&map.forward, p, d, a.arm).map(|v| [v[0] / h, v[1] / h]);
                    let smin = match (col([h, 0.0]), col([0.0, h])) {
                        (Some(x), Some(y)) => crate::scene::ifs_analysis::singular_values_of([[x[0], y[0]], [x[1], y[1]]]).0,
                        _ => 0.0,
                    };
                    next.push((p, if smin > 0.0 { 2.0 * r / smin } else { f64::INFINITY }));
                }
            }
            if next.is_empty() || next.len() > PIECES {
                return true;
            }
            level = next;
        }
        level.iter().any(|&(p, r)| (p[0] - ren.out_centre[0]).hypot(p[1] - ren.out_centre[1]) <= ren.out_radius + r)
    }

    /// A symbol's forward map and arm.
    fn sym_map(&self, sym: u32) -> Option<(&IfsMap<Map2>, u32)> {
        let ti = crate::scene::cylinder::sym_transform(sym) as usize;
        let t = self.transforms.iter().find(|t| t.index == ti)?;
        Some((&self.ifs.maps[t.map], sym >> 8))
    }

    /// Whether any word of a plan for `view` could need offsets: the view
    /// is under [`SWITCH_SHARE`] of the attractor's scale, or of its own
    /// distance from the origin -- where the plain replay's last step
    /// alone misses [`PLOT_TOLERANCE`].
    pub fn needs_offsets(&self, view: View) -> bool {
        #[cfg(test)]
        if OFFSETS_OFF.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        view.radius < SWITCH_SHARE * self.extent.max(view.centre[0].hypot(view.centre[1]))
    }

    /// A disc holding the attractor: its sample's farthest point, with a
    /// margin for those it did not draw.
    fn whole(&self) -> View {
        View { centre: self.centre, radius: 1.1 * self.extent }
    }

    /// Where the orbit's point `x` is plotted: through the finals, if the
    /// flame has any.
    pub fn plotted(&self, x: [f64; 2]) -> [f64; 2] {
        self.finals.as_ref().map_or(x, |f| f.forward(x))
    }

    /// Whether the GPU planner resolves `view`. See [`GPU_PLAN_RADIUS`].
    /// Judged where the walk plans it: through the finals, if any.
    pub fn gpu_resolves(&self, view: View) -> bool {
        let view = self.finals.as_ref().and_then(|f| f.pull_back(view, self.whole())).unwrap_or(view);
        view.radius >= GPU_PLAN_RADIUS * view.centre[0].hypot(view.centre[1]).max(1.0)
    }

    /// **Each kept word's reference orbits**, for the replay in offsets.
    ///
    /// Per word: its seeds run through it in f64, and those landing in
    /// the view (within two radii of the centre) are its candidate
    /// references. The first sets `m`: the view pulled back step by step
    /// through the smallest singular value of each map at the reference
    /// (from the exact forward forms, so without cancellation), and `m`
    /// the last step still at [`SWITCH_SHARE`] of the reference's
    /// distance from the origin. Then up to [`MAX_CHAINS`] references, one
    /// per cluster of the candidates at `m` more than four regions apart.
    ///
    /// `None` for a word that needs no offsets (`m` is its length), and
    /// for one with no seed that lands -- both replay in absolute f32.
    /// The attractor's centre and radius, from the analysis's sample: the
    /// frame PathMap's Origin styles read a point in. Robust, not the
    /// walk's `extent`: that is the farthest sample point, and julian's
    /// and bubble's rare far points made it hundreds of times the body's
    /// size, which read every point as at the centre -- one flat colour.
    /// The median in each axis, and the distance nineteen points in
    /// twenty lie within.
    pub fn frame(&self) -> ([f64; 2], f64) {
        if self.sample.is_empty() {
            return (self.centre, self.extent);
        }
        let median = |axis: usize| {
            let mut v: Vec<f64> = self.sample.iter().map(|p| p[axis]).collect();
            let k = v.len() / 2;
            *v.select_nth_unstable_by(k, f64::total_cmp).1
        };
        let c = [median(0), median(1)];
        let mut d: Vec<f64> = self.sample.iter().map(|p| (p[0] - c[0]).hypot(p[1] - c[1])).collect();
        let k = (d.len() * 95 / 100).min(d.len() - 1);
        let r = *d.select_nth_unstable_by(k, f64::total_cmp).1;
        (c, if r > 0.0 { r } else { self.extent })
    }

    pub async fn reference_chains(&self, cyl: &Cylinders, view: View, slicer: &Slicer) -> Vec<Option<WordRefs>> {
        use crate::scene::forward_delta::map_forward_difference;
        let c = view.centre;
        let r = view.radius;
        let one = |w: &Cylinder| -> Option<WordRefs> {
            let syms: Vec<(&IfsMap<Map2>, u32)> = w.word.iter().map(|&s| self.sym_map(s)).collect::<Option<_>>()?;
            let n = syms.len();
            // A word a renewal starts is followed from AFTER it: its seeds
            // are points of the rest's region (`close`), the blur is the
            // shader's alone, and `m` is never before it.
            let start = usize::from(self.alphabet.iter().any(|a| a.sym == w.word[0] && a.renewal.is_some()));
            // A seed's orbit through the word, if it lands in the view.
            let orbit = |x0: [f64; 2]| -> Option<Vec<[f64; 2]>> {
                let mut z = Vec::with_capacity(n + 1);
                for _ in 0..=start {
                    z.push(x0);
                }
                for (m, arm) in &syms[start..] {
                    let y = forward(m, *z.last().expect("seeded"), *arm);
                    if !finite(y) {
                        return None;
                    }
                    z.push(y);
                }
                let end = z[n];
                ((end[0] - c[0]).hypot(end[1] - c[1]) <= 2.0 * r).then_some(z)
            };
            // The first seed that lands is the reference; the others are
            // run only where they could find another piece (below).
            let (first, primary_orbit) = w.seeds.iter().enumerate().find_map(|(i, &x0)| orbit(x0).map(|z| (i, z)))?;
            let mut orbits = vec![primary_orbit];
            let primary = &orbits[0];
            // Back along the reference: the Jacobian of the steps from k
            // to the end, `P_k = J_{n-1}···J_k`, from the exact forward
            // forms (so without cancellation). Its largest singular value
            // carries an error at k to the end; its smallest, the view
            // back to k, which is how big the region is there.
            let mut amp = vec![f64::INFINITY; n + 1];
            let mut size = vec![f64::INFINITY; n + 1];
            amp[n] = 1.0;
            size[n] = r;
            let mut prod = [[1.0f64, 0.0], [0.0, 1.0]];
            for k in (start..n).rev() {
                let (map, arm) = syms[k];
                let z = primary[k];
                let h = 1e-6 * z[0].hypot(z[1]).max(1e-6);
                let col = |d: [f64; 2]| map_forward_difference(&map.forward, z, d, arm).map(|v| [v[0] / h, v[1] / h]);
                let (Some(a), Some(b)) = (col([h, 0.0]), col([0.0, h])) else { break };
                let j = [[a[0], b[0]], [a[1], b[1]]];
                prod = [
                    [prod[0][0] * j[0][0] + prod[0][1] * j[1][0], prod[0][0] * j[0][1] + prod[0][1] * j[1][1]],
                    [prod[1][0] * j[0][0] + prod[1][1] * j[1][0], prod[1][0] * j[0][1] + prod[1][1] * j[1][1]],
                ];
                let (smin, smax) = crate::scene::ifs_analysis::singular_values_of(prod);
                if !(smax.is_finite() && smin > 0.0) {
                    break;
                }
                amp[k] = smax;
                size[k] = r / smin;
            }
            let fits = |k: usize| GPU_STEP_ERROR * primary[k][0].hypot(primary[k][1]) * amp[k] <= PLOT_TOLERANCE * r;
            // Step 0 always fits: the free orbit's own error only picks a
            // slightly different point of the attractor.
            let m = (start.max(1)..=n).rev().find(|&k| fits(k)).unwrap_or(start);
            if m == n {
                return None;
            }
            // Another piece at `m` needs a map that is many-to-one going
            // forward among the offset steps -- bubble or disc. Measured, no
            // word of the corpus had one (`what_the_references_hold`), and
            // running every seed through every word doubled a web plan; so
            // the other seeds are run only where a piece could exist.
            let merges = syms[m..].iter().any(|(map, _)| match &map.forward {
                Map2::Nonlinear(k) => matches!(k.kernel, Kernel::Bubble | Kernel::Disc),
                Map2::Sum(k) => matches!(k.kernel, Kernel::Bubble | Kernel::Disc),
                _ => false,
            });
            if merges {
                orbits.extend(w.seeds[first + 1..].iter().filter_map(|&x0| orbit(x0)));
            }
            // One reference per cluster at `m`, farthest first.
            let apart = 4.0 * size[m];
            let mut chosen = vec![0usize];
            while chosen.len() < MAX_CHAINS {
                let far = (0..orbits.len())
                    .map(|j| {
                        let d = chosen
                            .iter()
                            .map(|&i| (orbits[j][m][0] - orbits[i][m][0]).hypot(orbits[j][m][1] - orbits[i][m][1]))
                            .fold(f64::INFINITY, f64::min);
                        (j, d)
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1));
                match far {
                    Some((j, d)) if d > apart => chosen.push(j),
                    _ => break,
                }
            }
            let chains = chosen
                .into_iter()
                .map(|j| RefChain { bases: orbits[j][m..n].to_vec(), end: [orbits[j][n][0] - c[0], orbits[j][n][1] - c[1]] })
                .collect();
            Some(WordRefs { m, chains })
        };
        map_sliced(&cyl.words, one, slicer).await
    }

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
        self.forward_along_salted(word, x, 0)
    }

    /// [`Self::forward_along`], its blur drawn from a stream also seeded
    /// by `salt`: independent draws from one point. Salt 0 is
    /// `forward_along` exactly.
    fn forward_along_salted(&self, word: &[u32], x: [f64; 2], salt: u64) -> Option<[f64; 2]> {
        // A blurred transform draws its blur from a stream seeded by the
        // word and the point: a replay is random, as the chaos game is,
        // and the same every time, as a plan must be.
        let mut st = word.iter().fold(x[0].to_bits() ^ x[1].to_bits().rotate_left(17) ^ salt, |h, &s| {
            (h ^ s as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29)
        });
        let mut u = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut p = x;
        for &sym in word {
            let t = sym_transform(sym) as usize;
            let arm = sym_arm(sym);
            let info = self.transforms.iter().find(|i| i.index == t)?;
            p = forward_blurred(&self.ifs.maps[info.map], p, arm, info.blur, &mut u);
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

    /// How many of `k` independent replays of `word` land in the view:
    /// sample points in turn, each with its own blur draw. For a blur's
    /// word, whose landings the walk's few hundred replays cannot resolve.
    fn landings(&self, word: &[u32], view: View, k: usize) -> usize {
        let n = self.sample.len().max(1);
        (0..k)
            .filter(|&i| {
                self.forward_along_salted(word, self.sample[(i * 7919) % n], i as u64 + 1)
                    .is_some_and(|y| (y[0] - view.centre[0]).hypot(y[1] - view.centre[1]) <= view.radius)
            })
            .count()
    }

    /// Whether `x`'s forward image along `word` lands in the view.
    fn lands(&self, word: &[u32], x: [f64; 2], view: View) -> bool {
        match self.forward_along(word, x) {
            Some(y) => (y[0] - view.centre[0]).hypot(y[1] - view.centre[1]) <= view.radius,
            None => false,
        }
    }


    /// `pts` pulled back through symbol `ai`: **along every branch of
    /// the inverse.** A map that is not one-to-one -- a sum of a root and
    /// an affine, `disc`, `bubble` -- has several preimages of a point,
    /// and the analysis makes one map per branch. Pulled back along the
    /// first alone, a sum's second arm never received a cloud point, and
    /// random1's views past its sample's resolution planned nothing. Each
    /// preimage is confirmed forward, with its arm.
    ///
    /// A point has to lie where the symbol's map sends the attractor
    /// (`near_landing`), or no real path came through it -- unless
    /// `exempt`.
    fn pull_back(&self, ai: usize, pts: &[[f64; 2]], exempt: bool, tr: &mut Trace) -> Vec<[f64; 2]> {
        let a = &self.alphabet[ai];
        let map = &self.ifs.maps[a.map];
        let junk_r = JUNK_EXTENTS * self.extent;
        let mut out = Vec::new();
        for &p in pts {
            if !exempt && !self.near_landing(ai, p) {
                tr.pruned += 1;
                continue;
            }
            for &mb in &a.branches {
                let q = self.ifs.maps[mb].inverse.apply(p);
                if !finite(q) || q[0].abs() > 1e12 || q[1].abs() > 1e12 {
                    tr.no_preimage += 1;
                    continue;
                }
                let Some(arm) = self.arm_of(map, q, p) else {
                    tr.no_arm += 1;
                    continue;
                };
                if arm != a.arm || (q[0] - self.centre[0]).hypot(q[1] - self.centre[1]) > junk_r {
                    continue;
                }
                out.push(q);
            }
        }
        out
    }

    /// The points of `pts` more than a cell from every sample point in
    /// `idx`, at most `ALT_CAP` of them: the pieces of a region its
    /// sample points do not stand for (`Node::alt`).
    fn away_from(&self, idx: &[u32], pts: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
        if pts.is_empty() {
            return pts;
        }
        let mut cells: Vec<Cell> = idx.iter().map(|&i| self.cell_of(self.sample[i as usize])).collect();
        cells.sort_unstable();
        cells.dedup();
        let near = Self::expand_cells(&cells, true);
        let away: Vec<[f64; 2]> = pts.into_iter().filter(|p| near.binary_search(&self.cell_of(*p)).is_err()).collect();
        thin(away, ALT_CAP)
    }

    /// `cells` and every cell within `reach` of one, sorted and
    /// deduplicated: a wider gather for an unseen child's near misses.
    fn widen_cells(cells: &[Cell], reach: i32) -> Vec<Cell> {
        let mut out: Vec<Cell> = Vec::with_capacity(cells.len() * ((2 * reach + 1) * (2 * reach + 1)) as usize);
        for &(cx, cy) in cells {
            for dx in -reach..=reach {
                for dy in -reach..=reach {
                    out.push((cx + dx, cy + dy));
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
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
        drive(self.index_tables_sliced(&Slicer::never()))
    }

    /// [`Self::index_tables`], yielding between indexes: the whole is a few
    /// million entries.
    pub async fn index_tables_sliced(&self, slicer: &Slicer) -> IndexTables {
        let n: usize = self.landing.iter().map(|i| i.entries.len()).sum::<usize>() + self.grid.entries.len();
        let mut t = IndexTables { cells: Vec::with_capacity(n), idx: Vec::with_capacity(n), offsets: vec![0] };
        for index in self.landing.iter().chain(std::iter::once(&self.grid)) {
            for &((x, y), i) in &index.entries {
                t.cells.push([x, y]);
                t.idx.push(i);
            }
            t.offsets.push(t.cells.len() as u32);
            slicer.tick().await;
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
    #[allow(clippy::too_many_arguments)]
    fn children_of(&self, o: &mut Open, depth: usize, floor_mass: f64, gather_now: bool, removals: &[Vec<u32>], refine: &[Vec<u32>], min_len: usize) {
        let tr = &mut o.trace;
        let node = &o.node;
        let mut seen: Vec<Cell> = Vec::new();
        let mut found: Vec<(usize, Pts)> = Vec::new(); // (alphabet index, candidates)
        // Exact children from the orbit, by alphabet index.
        let mut from_orbit: Vec<Vec<u32>> = vec![Vec::new(); self.alphabet.len()];
        // Their hidden pieces (`Node::alt`), by alphabet index.
        let mut alt_of: Vec<Vec<[f64; 2]>> = vec![Vec::new(); self.alphabet.len()];
        match &node.pts {
            Pts::Cloud(cloud) => {
                for (ai, a) in self.alphabet.iter().enumerate() {
                    // A renewal's region is the whole attractor: nothing
                    // to pull back. Its child is decided by `reaches`.
                    if a.renewal.is_some() {
                        found.push((ai, Pts::Index(Vec::new())));
                        continue;
                    }
                    // Not tested against the landings for a rescued
                    // cloud's points: they are on the attractor, where the
                    // sample is too sparse to say so (`Backward::rescue`).
                    // A preimage that is not a path's costs a replay; its
                    // word is measured all the same.
                    // Branches can multiply a cloud level by level: kept
                    // to `CLOUD_CAP`, evenly through it.
                    let pts = thin(self.pull_back(ai, cloud, node.rescued > 0, tr), CLOUD_CAP);
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
                for (ai, a) in self.alphabet.iter().enumerate() {
                    let cands = if gather_now && a.renewal.is_none() {
                        Self::gather_seen(&self.landing[ai], &seen, CAND_CAP)
                    } else {
                        Vec::new()
                    };
                    found.push((ai, Pts::Index(cands)));
                }
                // **The pieces the sample does not show** (`Node::alt`).
                // The node's points through every branch of a map with
                // several -- one branch's preimages are the candidates'
                // piece, the others' may be pieces no sample point is in
                // -- and its own hidden points through every map. Those
                // beside the sample's are dropped in `close`.
                let from: Vec<[f64; 2]> = idx.iter().step_by(idx.len().div_ceil(ALT_FROM).max(1)).map(|&i| self.sample[i as usize]).collect();
                let mut scratch = Trace::default();
                for (ai, a) in self.alphabet.iter().enumerate() {
                    if a.renewal.is_some() {
                        continue;
                    }
                    let mut alt = if a.branches.len() > 1 { self.pull_back(ai, &from, false, &mut scratch) } else { Vec::new() };
                    alt.extend(self.pull_back(ai, &node.alt, false, &mut scratch));
                    alt_of[ai] = thin(alt, ALT_CAP);
                }
            }
        }
        let mut removed = 0usize;
        let children = found
            .into_iter()
            .filter_map(|(ai, pts)| {
                let a = &self.alphabet[ai];
                let prob = node.prob * a.prob;
                let mut word = Vec::with_capacity(node.word.len() + 1);
                word.push(a.sym);
                word.extend_from_slice(&node.word);
                // A removed piece is never made (`PlanOptions::removals`).
                if crate::scene::word_tree::removed(removals, &word) {
                    removed += 1;
                    return None;
                }
                let n_cands = match &pts {
                    Pts::Index(c) => c.len(),
                    Pts::Cloud(_) => 0,
                };
                let refine = crate::scene::word_tree::must_split(removals, refine, &word) || word.len() < min_len;
                let rescued = node.rescued.saturating_sub(1);
                Some(Child {
                    ai,
                    word,
                    prob,
                    last: depth == MAX_DEPTH || prob < MEASURE_FLOOR * floor_mass,
                    refine,
                    rescued,
                    alt: std::mem::take(&mut alt_of[ai]),
                    pts,
                    orbit_hits: std::mem::take(&mut from_orbit[ai]),
                    hit: 0,
                    total: 0,
                    replay_hits: Vec::new(),
                    hits: Vec::new(),
                    topped: false,
                    n_cands,
                    spec: None,
                    fate: Fate::Undecided,
                })
            })
            .collect();
        o.trace.removed += removed;
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
    async fn expand_level(
        &self,
        frontier: Vec<Node>,
        depth: usize,
        view: View,
        floor_mass: f64,
        watch: Option<&[u32]>,
        record: bool,
        eval: &mut dyn AskEval,
        tr: &mut Trace,
        cancelled: &dyn Fn() -> bool,
        slicer: &Slicer,
        removals: &[Vec<u32>],
        refine: &[Vec<u32>],
        min_len: usize,
    ) -> Option<Vec<Expanded>> {
        use web_time::Instant;
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
        let seen: Vec<Option<Vec<Cell>>> = map_sliced(
            &opens,
            |o| match &o.node.pts {
                Pts::Cloud(cloud) => {
                    let mut cells: Vec<Cell> = cloud.iter().map(|p| self.cell_of(*p)).collect();
                    cells.sort_unstable();
                    cells.dedup();
                    Some(Self::expand_cells(&cells, true))
                }
                Pts::Index(_) => None,
            },
            slicer,
        )
        .await;
        let seeded = {
            let gathers: Vec<GatherJob> = opens
                .iter()
                .zip(&seen)
                .filter_map(|(o, s)| {
                    s.as_ref().map(|s| GatherJob { word: &o.node.word, index: IndexId::Grid, seen: s, cap: SEED_CANDIDATES })
                })
                .collect();
            eval.gather_lands(self, view, &[], &gathers).await.1
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
                // What the sample's points do not stand for is kept:
                // the cloud's points away from them (`Node::alt`).
                o.node.alt = self.away_from(&hits, cloud.clone());
                o.node.pts = Pts::Index(hits);
                o.trace.seeded += 1;
            }
        }
        drop(seen);
        tr.t_seed += t.elapsed();
        if cancelled() {
            return None;
        }
        slicer.tick().await;

        // **2. Children.** The gathers are the cost; nodes in parallel.
        let t = Instant::now();
        each_sliced(&mut opens, |o| self.children_of(o, depth, floor_mass, !speculate, removals, refine, min_len), slicer).await;
        tr.t_gather += t.elapsed();
        if cancelled() {
            return None;
        }
        slicer.tick().await;

        // **3. Replays**: the first `REPLAY_FIRST` verification points
        // for every child, then the rest for the children whose share is
        // too close to the cut to trust. See `REPLAY_FIRST`.
        let t = Instant::now();
        // An UNSEEN child whose first pass landed nothing runs the rest
        // too: it is about to be dropped, and a share of 1.5% reads zero
        // on a hundred points a fifth of the time -- measured, a
        // julian-disc branch holding 12.7% of its view was dropped so. On
        // four hundred it reads zero 0.25% of the time. Only those: for
        // every zero it cost a CPU plan 40%.
        let in_band = |c: &Child| {
            let e = c.hit as f64 / c.total.max(1) as f64;
            let unseen = matches!(c.pts, Pts::Index(_)) && c.n_cands == 0 && c.orbit_hits.is_empty();
            (e > REPLAY_EXTEND.0 && e < REPLAY_EXTEND.1) || (c.hit == 0 && unseen)
        };
        let landed = |g: &[u8]| g.iter().filter(|g| **g == 1).count();
        // The sample indices of the points that landed.
        let which = |g: &[u8], pts: &[u32]| -> Vec<u32> {
            g.iter().zip(pts).filter(|(a, _)| **a == 1).map(|(_, i)| *i).collect()
        };
        let (n1, n2) = (self.verify_first.len(), self.verify_rest.len());
        if speculate {
            // Both passes for every child, and every index child's gather
            // and check, in as few batches as `FUSED_WORDS` allows: one
            // for most levels. A level's widest batch was ten million
            // words, and reading it back took a web frame on its own.
            let order: Vec<(usize, usize)> = opens
                .iter()
                .enumerate()
                .flat_map(|(oi, o)| (0..o.children.len()).map(move |ci| (oi, ci)))
                .collect();
            let words_of = |c: &Child| n1 + n2 + if matches!(c.pts, Pts::Index(_)) { CAND_CAP } else { 0 };
            let mut start = 0usize;
            while start < order.len() {
                let mut end = start;
                let mut words = 0usize;
                while end < order.len() {
                    let (oi, ci) = order[end];
                    let w = words_of(&opens[oi].children[ci]);
                    if end > start && words + w > FUSED_WORDS {
                        break;
                    }
                    words += w;
                    end += 1;
                }
                let (answers, gathered) = {
                    let mut jobs: Vec<EvalJob> = Vec::new();
                    let mut gathers: Vec<GatherJob> = Vec::new();
                    for &(oi, ci) in &order[start..end] {
                        let o = &opens[oi];
                        let c = &o.children[ci];
                        jobs.push(EvalJob { word: &c.word, points: &self.verify_first });
                        jobs.push(EvalJob { word: &c.word, points: &self.verify_rest });
                        if self.gathers(c) {
                            gathers.push(GatherJob { word: &c.word, index: IndexId::Landing(c.ai), seen: &o.seen, cap: CAND_CAP });
                        }
                    }
                    eval.gather_lands(self, view, &jobs, &gathers).await
                };
                let mut gathered = gathered.into_iter();
                let mut at = 0usize;
                for &(oi, ci) in &order[start..end] {
                    let gathers_c = self.gathers(&opens[oi].children[ci]);
                    let c = &mut opens[oi].children[ci];
                    if gathers_c {
                        let g = gathered.next().expect("one answer per gather");
                        c.n_cands = g.cands;
                        c.spec = Some(g.hits);
                    }
                    let first = &answers[at..at + n1];
                    c.hit = landed(first);
                    c.total = n1;
                    c.replay_hits = which(first, &self.verify_first);
                    at += n1;
                    let rest = &answers[at..at + n2];
                    at += n2;
                    if in_band(c) {
                        c.hit += landed(rest);
                        c.total += n2;
                        c.replay_hits.extend(which(rest, &self.verify_rest));
                    }
                }
                start = end;
                slicer.tick().await;
            }
        } else {
            let answers = {
                let jobs: Vec<EvalJob> = opens
                    .iter()
                    .flat_map(|o| o.children.iter().map(|c| EvalJob { word: &c.word, points: &self.verify_first }))
                    .collect();
                eval.lands(self, view, &jobs).await
            };
            for (k, c) in opens.iter_mut().flat_map(|o| o.children.iter_mut()).enumerate() {
                let first = &answers[k * n1..(k + 1) * n1];
                c.hit = landed(first);
                c.total = n1;
                c.replay_hits = which(first, &self.verify_first);
            }
            let answers = {
                let jobs: Vec<EvalJob> = opens
                    .iter()
                    .flat_map(|o| o.children.iter())
                    .filter(|c| in_band(c))
                    .map(|c| EvalJob { word: &c.word, points: &self.verify_rest })
                    .collect();
                eval.lands(self, view, &jobs).await
            };
            let mut k = 0usize;
            for c in opens.iter_mut().flat_map(|o| o.children.iter_mut()) {
                if in_band(c) {
                    let rest = &answers[k * n2..(k + 1) * n2];
                    c.hit += landed(rest);
                    c.total += n2;
                    c.replay_hits.extend(which(rest, &self.verify_rest));
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
        slicer.tick().await;

        // **4. Kept or carried.** Below the floor, or at the depth cap,
        // the walk stops -- and the word is FORCED if any of it lands,
        // never dropped. See `MEASURE_FLOOR`.
        for o in &mut opens {
            // The node's cells widened for an unseen child's near misses,
            // made once, when first wanted: from at most `WIDEN_FROM` of
            // them, since a node with a large region widened whole was
            // tens of milliseconds a child.
            let mut wide: Option<Vec<Cell>> = None;
            for c in &mut o.children {
                let eff = c.eff();
                let prob = c.prob;
                // **A renewal is kept, never carried, and kept by
                // geometry** (`Renewal`): its region is the whole
                // attractor, so carrying it cannot localize it, and at
                // depth its replays can miss a smooth part that lands one
                // time in ten million.
                if let Some(ren) = self.alphabet[c.ai].renewal {
                    c.fate = if self.reaches(&c.word[1..], view, ren) { Fate::Kept(eff) } else { Fate::Dropped };
                    if c.refine && matches!(c.fate, Fate::Kept(_)) {
                        o.trace.unrefined += 1;
                    }
                    watched(&mut o.trace, &c.word, "RENEWAL", &|| format!("eff {eff:.2e} prob {prob:.2e} kept {}", matches!(c.fate, Fate::Kept(_))));
                    continue;
                }
                let unseen = matches!(c.pts, Pts::Index(_)) && c.n_cands == 0 && c.orbit_hits.is_empty();
                // **Unseen, but perhaps only just** (tracker C3): no
                // candidate in the node's cells, but a child worth its
                // probability may have them a few cells out -- near
                // misses, to look closer around (`rescue`) -- or points
                // in a piece of its region the sample does not show at
                // all (`Node::alt`): julian-disc's `t1a3 t0^15` held 1.3%
                // of its view on the second sheet of `disc`, a unit away
                // from every sample point the walk had. Carried, as a
                // cloud, from what either finds. The rescue's points are
                // the attractor's, many and spread; the hidden ones are
                // exact but few, and carried alone they covered a branch
                // worse than a rescue does.
                if unseen && !(eff > 0.0) {
                    let rescued = if prob > FORCE_WASTE * floor_mass {
                        let cells = wide.get_or_insert_with(|| {
                            let every = o.seen.len().div_ceil(WIDEN_FROM).max(1);
                            let from: Vec<Cell> = o.seen.iter().step_by(every).copied().collect();
                            Self::widen_cells(&from, UNSEEN_REACH)
                        });
                        let near = Self::gather_seen(&self.landing[c.ai], cells, 8 * RESCUE_CANDIDATES);
                        let found = self.rescue(&c.word, &Pts::Index(near), view, prob, floor_mass, slicer).await;
                        // A gather over the widened cells is a thousand
                        // binary searches, and a node can have dozens of
                        // unseen children.
                        slicer.tick().await;
                        found
                    } else {
                        None
                    };
                    let hidden = c.alt.len();
                    if rescued.is_some() || hidden > 0 {
                        let found = rescued.as_ref().map_or(0, |r| r.len());
                        watched(&mut o.trace, &c.word, if found > 0 { "RESCUED" } else { "HIDDEN" }, &|| {
                            format!("unseen; {found} points near a wider gather land, {hidden} of a piece the sample does not show")
                        });
                        if found > 0 {
                            o.trace.rescued += 1;
                        } else {
                            o.trace.hidden += 1;
                        }
                        let mut cloud = rescued.unwrap_or_default();
                        cloud.append(&mut c.alt);
                        c.pts = Pts::Cloud(thin(cloud, CLOUD_CAP));
                        c.rescued = RESCUE_LEVELS;
                        c.fate = Fate::Carried;
                        continue;
                    }
                }
                if unseen && !(eff > 0.0) {
                    watched(&mut o.trace, &c.word, "UNSEEN", &|| format!("no candidates, {} replays land nothing; prob {prob:.2e} floor {floor_mass:.2e}", c.total));
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
                // One that holds a removed piece is never cut: it is
                // refined, down to words that either are the piece or
                // are not.
                if (eff >= CUT_EFFICIENCY && !c.refine) || c.last {
                    // **Kept: it needs no points.** Checking a kept
                    // child's candidates exactly was 65% of a plan's
                    // time, and ~90% of the children checked were kept.
                    watched(&mut o.trace, &c.word, "CUT", &|| format!("eff {eff:.2} prob {prob:.2e}"));
                    if c.refine {
                        o.trace.unrefined += 1;
                    }
                    c.fate = Fate::Kept(eff);
                } else if matches!(c.pts, Pts::Cloud(_)) {
                    c.fate = Fate::Carried;
                }
            }
            slicer.tick().await;
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
                eval.lands(self, view, &jobs).await
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
        let wide: Vec<Vec<Cell>> = map_sliced(
            &opens,
            |o| {
                let Pts::Index(parent) = &o.node.pts else { return Vec::new() };
                if !o.children.iter().any(|c| c.topped) {
                    return Vec::new();
                }
                let mut cells: Vec<Cell> = parent.iter().map(|&i| self.cell_of(self.sample[i as usize])).collect();
                cells.sort_unstable();
                cells.dedup();
                Self::expand_cells(&cells, true)
            },
            slicer,
        )
        .await;
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
            eval.gather_lands(self, view, &[], &gathers).await.1
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
        slicer.tick().await;

        // **7. Each node decided**, its children in their order.
        let mut out = Vec::with_capacity(opens.len());
        for o in opens {
            out.push(self.close(o, depth, view, watch, floor_mass, removals, slicer).await);
            slicer.tick().await;
        }
        Some(out)
    }

    /// **Points of the attractor near sample point `i`**: its last `k`
    /// maps -- the chaos game's own, which made it -- applied to other
    /// sample points. They lie in `i`'s depth-`k` cylinder, so they are as
    /// near it as `k` says, and as many as asked for. `None` where the
    /// sample was reseeded within `k` steps of `i`.
    fn near_points(&self, i: usize, k: usize, m: usize) -> Option<Vec<[f64; 2]>> {
        if i < k || k == 0 {
            return None;
        }
        let mut history: Vec<u32> = Vec::with_capacity(k);
        for j in (i + 1 - k..=i).rev() {
            let ai = *self.made_by.get(j)?;
            if ai == u32::MAX {
                return None;
            }
            history.push(self.alphabet[ai as usize].sym);
        }
        // Oldest first, as a word is applied.
        history.reverse();
        let n = self.sample.len();
        Some((0..m).filter_map(|j| self.forward_along_salted(&history, self.sample[(i.wrapping_mul(2654435761) + j * 7919) % n], j as u64 + 1)).collect())
    }

    /// **A child with near misses, looked at closer** (tracker C3). Its
    /// region's candidates came near the view and none landed, and its
    /// replays read zero: at a view the sample holds ten points of, a
    /// branch holding 3% of it (random1 at 1e3, `t1a1`) looked exactly
    /// like that, and was dropped -- 2.6% of the view missing. So a child
    /// whose probability is worth it (over `FORCE_WASTE` of the mass kept)
    /// is looked for among points of the attractor near its candidates
    /// (`near_points`), from coarse to fine. What lands is its region's,
    /// on the attractor, and the walk carries it from there as a cloud.
    ///
    /// An UNSEEN child, with no candidates at all, is given some from the
    /// node's cells widened by `UNSEEN_REACH` (`expand_level`, step 4).
    async fn rescue(&self, word: &[u32], pts: &Pts, view: View, prob: f64, floor_mass: f64, slicer: &Slicer) -> Option<Vec<[f64; 2]>> {
        let Pts::Index(cands) = pts else { return None };
        if cands.is_empty() || !(prob > FORCE_WASTE * floor_mass) {
            return None;
        }
        // Nearest miss first. Every depth: too shallow and the points
        // spread past the view, too deep and they gather round a point
        // that misses; measured on random1, the depth that lands runs
        // from 8 to 12 by candidate.
        let mut near: Vec<(u32, f64, [f64; 2])> = cands
            .iter()
            .filter_map(|&c| {
                let y = self.forward_along(word, self.sample[c as usize])?;
                Some((c, (y[0] - view.centre[0]).hypot(y[1] - view.centre[1]), y))
            })
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut found: Vec<[f64; 2]> = Vec::new();
        for &(c, dist, yc) in near.iter().take(RESCUE_CANDIDATES) {
            for k in RESCUE_DEPTHS {
                // A depth is up to a millisecond.
                slicer.tick().await;
                let before = found.len();
                let mut spread = 0.0f64;
                for z in self.near_points(c as usize, k, RESCUE_POINTS).unwrap_or_default() {
                    let Some(y) = self.forward_along(word, z) else { continue };
                    if (y[0] - view.centre[0]).hypot(y[1] - view.centre[1]) <= view.radius {
                        found.push(z);
                    }
                    spread = spread.max((y[0] - yc[0]).hypot(y[1] - yc[1]));
                }
                if found.len() >= CLOUD_CAP {
                    found.truncate(CLOUD_CAP);
                    return Some(found);
                }
                // Deeper cylinders are smaller, and so are their images:
                // once this one's no longer reaches from the miss to the
                // view, no deeper one will.
                if found.len() == before && spread < dist - view.radius {
                    break;
                }
            }
        }
        (!found.is_empty()).then_some(found)
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
    #[allow(clippy::too_many_arguments)]
    async fn close(&self, o: Open, depth: usize, view: View, watch: Option<&[u32]>, floor_mass: f64, removals: &[Vec<u32>], slicer: &Slicer) -> Expanded {
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
        let disc = |word: Vec<u32>, prob: f64, seeds: Vec<[f64; 2]>| Cylinder { word, prob, centre: view.centre, radius: view.radius, seeds, eff: 0.0, draw: 1.0 };

        let mut node_kept: Vec<(Cylinder, f64)> = Vec::new();
        let mut node_next: Vec<Node> = Vec::new();
        // Which children survived -- carried or kept -- by alphabet
        // index, for the point count below.
        let mut survived = vec![false; self.alphabet.len()];
        // A removed child's points are accounted for: the piece is gone
        // on purpose, and the node is not forced whole for want of it.
        if !removals.is_empty() {
            let mut word = Vec::with_capacity(node.word.len() + 1);
            for (ai, a) in self.alphabet.iter().enumerate() {
                word.clear();
                word.push(a.sym);
                word.extend_from_slice(&node.word);
                if crate::scene::word_tree::removed(removals, &word) {
                    survived[ai] = true;
                }
            }
        }
        for c in children {
            let eff = c.eff();
            let n_cands = c.n_cands;
            let Child { ai, word, prob, pts, orbit_hits, mut replay_hits, mut hits, fate, refine, rescued, alt, .. } = c;
            let pts = match fate {
                Fate::Dropped => continue,
                Fate::Kept(eff) => {
                    survived[ai] = true;
                    // A renewal's reference starts after it: at points of
                    // the node's region (`reference_chains`).
                    let seeds = if self.alphabet[ai].renewal.is_some() {
                        self.seeds_from(&[], Some(&node.pts))
                    } else {
                        self.seeds_from(&[&orbit_hits, &hits, &replay_hits], Some(&pts))
                    };
                    node_kept.push((disc(word, prob, seeds), eff));
                    continue;
                }
                Fate::Carried => pts,
                Fate::Undecided => {
                    // The orbit's points need no check: they are in the
                    // child's region by construction.
                    hits.extend_from_slice(&orbit_hits);
                    hits.sort_unstable();
                    hits.dedup();
                    // **Pointless, but it lands.** No point of its region
                    // was found to expand it from -- except the
                    // verification points its own replay sent into the
                    // view, which ARE points of its region. Forced as it
                    // stands, a shallow child with a sliver of the view is
                    // nearly all waste: random1 1e3 fell to an efficiency
                    // of 0.013 on a few of them. Carried on from those
                    // points instead, julian-disc's 51 arms multiplied into
                    // plans of 200k words. So it is carried when forcing
                    // it would waste more than `FORCE_WASTE` of the mass
                    // already kept, and forced otherwise.
                    // A child that must be split is carried from them
                    // too, whatever forcing it would waste: kept whole,
                    // the split is undone (`PlanOptions::refine`,
                    // `min_len`).
                    if hits.is_empty() && !replay_hits.is_empty() && (refine || prob * (1.0 - eff) > FORCE_WASTE * floor_mass) {
                        watched(&mut trace, &word, "REPLAYED", &|| format!("{} replay points; eff {eff:.2} prob {prob:.2e}", replay_hits.len()));
                        hits = std::mem::take(&mut replay_hits);
                        hits.sort_unstable();
                        hits.dedup();
                    }
                    if hits.is_empty() {
                        trace.empty += 1;
                        if eff > 0.0 {
                            // It lands -- the replay says so -- but no
                            // point of its region was found to expand it
                            // from. Forced as it stands.
                            //
                            // Carrying it on by replays alone instead was
                            // measured and not kept: julian-disc's 51 arms
                            // multiplied it into plans of 200k words at
                            // efficiency 0.003.
                            watched(&mut trace, &word, "FORCEDCH", &|| format!("{n_cands} candidates, none land; eff {eff:.2}"));
                            if refine {
                                trace.unrefined += 1;
                            }
                            survived[ai] = true;
                            let seeds = self.seeds_from(&[&replay_hits], Some(&pts));
                            node_kept.push((disc(word, prob, seeds), eff));
                        } else if let Some(cloud) = match self.rescue(&word, &pts, view, prob, floor_mass, slicer).await {
                            Some(found) => Some(found),
                            None => (!alt.is_empty()).then(Vec::new),
                        } {
                            // Near misses, and a large share: looked for
                            // closer (`rescue`), and carried from what was
                            // found -- with its points in pieces the sample
                            // does not show (`Node::alt`), or from those
                            // alone.
                            let (found, hidden) = (cloud.len(), alt.len());
                            watched(&mut trace, &word, if found > 0 { "RESCUED" } else { "HIDDEN" }, &|| {
                                format!("{n_cands} candidates, none land; {found} points near them do, {hidden} of a piece the sample does not show")
                            });
                            if found > 0 {
                                trace.rescued += 1;
                            } else {
                                trace.hidden += 1;
                            }
                            survived[ai] = true;
                            let mut cloud = cloud;
                            cloud.extend(alt);
                            node_next.push(Node { word, pts: Pts::Cloud(thin(cloud, CLOUD_CAP)), prob, eff: 0.0, rescued: RESCUE_LEVELS, alt: Vec::new() });
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
            // A rescued cloud's children keep its exemption, one level
            // less, while they are clouds; seeded, they are ordinary.
            let rescued = if matches!(pts, Pts::Cloud(_)) { rescued } else { 0 };
            // Its hidden points, but those beside the sample's: those are
            // the piece its points already are.
            let alt = match &pts {
                Pts::Index(idx) => self.away_from(idx, alt),
                Pts::Cloud(_) => Vec::new(),
            };
            node_next.push(Node { word, pts, prob, eff, rescued, alt });
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
            if crate::scene::word_tree::holds_removed(removals, &node.word) {
                trace.unrefined += 1;
            }
            let seeds = self.seeds_from(&[], Some(&node.pts));
            out.kept.push((disc(node.word, node.prob, seeds), node.eff));
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
        if let Some(gpu) = opts.gpu.filter(|_| self.gpu_resolves(view)) {
            // Held for the whole plan: one plan runs at a time, and a
            // cancelled one stops at its next batch.
            let mut g = gpu.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(eval) = g.for_flame(flame, self) {
                let mut tr = Trace::default();
                eval.totals = Default::default();
                eval.batches = 0;
                let t0 = web_time::Instant::now();
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
        drive(self.walk(view, tr, opts, &mut Blocking(eval), &Slicer::never()))
    }

    /// **The walk, as a future**: the one the desktop drives straight
    /// through ([`drive`]) and the web polls once a frame, resuming where
    /// the last frame's slice ran out or a GPU answer was still coming.
    pub async fn walk(
        &self,
        view: View,
        tr: &mut Trace,
        opts: PlanOptions<'_>,
        eval: &mut dyn AskEval,
        slicer: &Slicer,
    ) -> Result<Cylinders, NoCylinders> {
        // **Through the finals** (C2c): the orbit's points plotted in the
        // view are those in its pull-back, a disc in the orbit's space,
        // and that is what is planned. The plan keeps the view's own
        // centre, which is what the renderer compares a pan against.
        let Some(finals) = &self.finals else { return self.walk_disc(view, tr, opts, eval, slicer).await };
        let disc = finals.pull_back(view, self.whole()).ok_or(NoCylinders::ViewIsEmpty)?;
        let mut plan = self.walk_disc(disc, tr, opts, eval, slicer).await?;
        plan.view_centre = view.centre;
        Ok(plan)
    }

    /// [`Self::walk`], for a disc of the orbit's own space.
    async fn walk_disc(
        &self,
        view: View,
        tr: &mut Trace,
        opts: PlanOptions<'_>,
        eval: &mut dyn AskEval,
        slicer: &Slicer,
    ) -> Result<Cylinders, NoCylinders> {
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
        let mut frontier = vec![Node { word: Vec::new(), pts: Pts::Cloud(root_pts), prob: 1.0, eff: 0.0, rescued: 0, alt: Vec::new() }];
        let mut kept: Vec<(Cylinder, f64)> = Vec::new();
        let mut kept_mass = 0.0f64;
        let mut lost = 0.0f64;
        let started = web_time::Instant::now();

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
                    if crate::scene::word_tree::holds_removed(opts.removals, &n.word) {
                        tr.unrefined += 1;
                    }
                    let seeds = self.seeds_from(&[], Some(&n.pts));
                    kept.push((Cylinder { word: n.word, prob: n.prob, centre: view.centre, radius: view.radius, seeds, eff: 0.0, draw: 1.0 }, n.eff));
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
            let Some(results) = self
                .expand_level(
                    std::mem::take(&mut frontier),
                    depth,
                    view,
                    floor_mass,
                    watch.as_deref(),
                    record,
                    eval,
                    tr,
                    &cancelled,
                    slicer,
                    opts.removals,
                    opts.refine,
                    opts.min_len,
                )
                .await
            else {
                return Err(NoCylinders::ViewIsEmpty);
            };

            let mut next: Vec<Node> = Vec::new();
            for e in results {
                for (c, eff) in e.kept {
                    // A blur's word is not the view's measure: its blob
                    // lands mostly elsewhere. Counted in the floor, one
                    // kept at depth 1 because the view grazes the blob
                    // raised the floor a hundredfold, and the rest of the
                    // plan was cut short -- 72 words at efficiency 0.001
                    // where a view 1% smaller planned 1,100 at 0.24.
                    if !self.renewal_first(&c.word) {
                        kept_mass += c.prob;
                    }
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
                // A node holding a removed piece is carried too: forced
                // here, the piece would be back.
                for n in next.drain(keep..) {
                    if n.eff > 0.0 && !crate::scene::word_tree::must_split(opts.removals, opts.refine, &n.word) && n.word.len() >= opts.min_len {
                        rest.push(n)
                    } else {
                        unmeasured.push(n)
                    }
                }
                next.extend(unmeasured);
                for n in rest {
                    // Off the beam: forced as it stands, not dropped.
                    if let Some(line) = watch_line(watch.as_deref(), &n.word, depth, "BEAM", || format!("prob {:.2e} eff {:.2}", n.prob, n.eff)) {
                        tr.watched.push(line);
                    }
                    if !self.renewal_first(&n.word) {
                        kept_mass += n.prob;
                    }
                    let seeds = self.seeds_from(&[], Some(&n.pts));
                    kept.push((Cylinder { word: n.word, prob: n.prob, centre: view.centre, radius: view.radius, seeds, eff: 0.0, draw: 1.0 }, n.eff));
                    tr.beam += 1;
                }
            }
            frontier = next;
            slicer.tick().await;
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
                    // Each branch's points: a merged word is reached
                    // from separate regions, and each wants a reference.
                    for q in c.seeds {
                        if m.seeds.len() < 2 * REF_SEEDS {
                            m.seeds.push(q);
                        }
                    }
                }
                _ => merged.push((c, e)),
            }
        }
        // **A blur's word that would take the draws for nothing** (C2b).
        // Kept by geometry, a blur's word can carry most of the plan's
        // probability while landing almost never: the true Grand Julian
        // at zoom 1559 keeps its blob because the view's DISC grazes the
        // blob's by 2e-6 -- outside the frame, which no blob point of
        // four million reached -- and that word was 99.9% of the plan.
        // Its few hundred replays read zero, which floors its draw rate
        // at 1/20 and still hands it 97% of the draws.
        //
        // So such a word -- zero landings, a large share of the draws --
        // is replayed as often as it takes to tell. None landing in `k`
        // bounds its share of the view below `prob · 3/k`; under 1% of
        // what the rest of the plan puts there, it is dropped as
        // negligible. Otherwise what landed sets its draw rate.
        //
        // The costliest first, and the shares taken again after each:
        // a word that looked cheap beside a grazing blob is costly once
        // the blob is gone, and taken in one pass the plans either side
        // of the graze kept different words.
        {
            let draw_floor = 1.0 / (self.verify_first.len() + self.verify_rest.len()).max(1) as f64;
            let rest: f64 = merged.iter().map(|(c, e)| c.prob * e).sum();
            // How often a word is drawn, relative to its probability: its
            // measured rate once it has one.
            let rate = |c: &Cylinder, e: f64| -> f64 {
                if c.draw != 1.0 {
                    c.draw
                } else if self.renewal_first(&c.word) {
                    e.max(draw_floor).sqrt()
                } else {
                    1.0
                }
            };
            let unmeasured = |c: &Cylinder, e: f64| self.renewal_first(&c.word) && !(e > 0.0) && c.draw == 1.0;
            let mut budget = RENEWAL_REPLAYS;
            loop {
                let drawn: f64 = merged.iter().map(|(c, e)| c.prob * rate(c, *e)).sum();
                let Some(i) = merged
                    .iter()
                    .enumerate()
                    .filter(|(_, (c, e))| unmeasured(c, *e) && c.prob * draw_floor.sqrt() > 0.05 * drawn)
                    // The replays that could prove it negligible, if none
                    // land -- within what is left of the budget.
                    .filter(|(_, (c, _))| {
                        let k = ((3.0 * c.prob / (0.01 * rest.max(f64::MIN_POSITIVE))).ceil() as usize).clamp(1 << 14, 1 << 20);
                        k * c.word.len() <= budget
                    })
                    .max_by(|x, y| x.1 .0.prob.total_cmp(&y.1 .0.prob))
                    .map(|(i, _)| i)
                else {
                    break;
                };
                let c = &merged[i].0;
                let k = ((3.0 * c.prob / (0.01 * rest.max(f64::MIN_POSITIVE))).ceil() as usize).clamp(1 << 14, 1 << 20);
                budget -= k * c.word.len();
                let hits = self.landings(&c.word, view, k);
                slicer.tick().await;
                if hits == 0 && c.prob * 3.0 / (k as f64) < 0.01 * rest {
                    tr.renewal_dropped += 1;
                    merged.remove(i);
                    continue;
                }
                merged[i].0.draw = (hits as f64 / k as f64).max(0.5 / k as f64).sqrt();
            }
        }
        if merged.is_empty() {
            return Err(NoCylinders::ViewIsEmpty);
        }
        let mass: f64 = merged.iter().map(|(c, _)| c.prob).sum();
        let delivered: f64 = merged.iter().map(|(c, e)| c.prob * e).sum();
        // The least efficiency a replay can tell from none.
        let draw_floor = 1.0 / (self.verify_first.len() + self.verify_rest.len()).max(1) as f64;
        let depth = merged.iter().map(|(c, _)| c.word.len()).max().unwrap_or(0);
        let mut plan = Cylinders {
            // Each word keeps the efficiency its replays measured. A
            // blur's word is drawn at the square root of it
            // (`Cylinder::draw`), floored at the replays' resolution.
            words: merged
                .into_iter()
                .map(|(mut c, e)| {
                    c.eff = e;
                    // Unless a longer replay set it (above).
                    if self.renewal_first(&c.word) && c.draw == 1.0 {
                        c.draw = e.max(draw_floor).min(1.0).sqrt();
                    }
                    c
                })
                .collect(),
            mass,
            lost,
            sampling_leak: 0.0,
            efficiency: if mass > 0.0 { delivered / mass } else { 0.0 },
            depth,
            composable: false,
            view_centre: view.centre,
            refs: Vec::new(),
            offset_rows: Vec::new(),
        };
        // The replay in offsets, where the view is deep enough for any
        // word to need it (`deep-zoom-precision.md`). Not through finals:
        // the render applies them to the absolute point, which offsets
        // leave view-relative -- so a plan with finals replays in
        // absolute f32, as the untargeted render plots.
        if self.finals.is_none() && self.needs_offsets(view) {
            plan.refs = self.reference_chains(&plan, view, slicer).await;
            if plan.refs.iter().any(|r| r.is_some()) {
                plan.offset_rows = self.forward_rows();
            } else {
                plan.refs.clear();
            }
        }
        Ok(plan)
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
            let y = forward_blurred(&b.ifs.maps[t.map], x, arm, t.blur, &mut lcg);
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
            // Plotted through the finals, as the render plots.
            let y = b.plotted(x);
            if k < 1000 || (y[0] - view.centre[0]).hypot(y[1] - view.centre[1]) > view.radius {
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

        let (a_gpu, g_gpu) = Evaluate::gather_lands(gpu, &b, view, &replay, &gathers);
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

    /// **What the replay's references hold** (`deep-zoom-precision.md` §6
    /// step 2), measured: per flame and zoom, how many words need offsets
    /// and from which step, how many references they carry, how long the
    /// references take -- and that the offset carry reproduces the
    /// absolute replay. Every seed of every word is run both ways in f64:
    /// absolute to the end, and absolute to `m` then as an offset from
    /// its nearest reference through the forward forms. They must agree
    /// to far under a pixel, and every seed must sit within its cluster
    /// of a reference at `m` -- one that does not is a region the chains
    /// missed.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_the_references_hold() {
        use crate::scene::forward_delta::map_forward_difference;
        use std::time::Instant;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut worst_px = 0.0f64;
        let mut stray = 0usize;
        for name in ["grand-julian", "random1", "julian-disc"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")) else {
                println!("  no {name}");
                continue;
            };
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
            let Ok(b) = Backward::read(&cfg.flame, reg) else {
                println!("  {name}: not walked");
                continue;
            };
            for z in [1e4f64, 1e6, 1e8] {
                let view = View::of(z, b.sample_point(0.75), 1280, 720);
                let px = view.radius / (1280f64.hypot(720.0) / 2.0);
                let t0 = Instant::now();
                let Ok(plan) = b.plan_eval(view, PlanOptions::default(), &mut CpuEval) else {
                    println!("  {name} {z:.0e}: no plan");
                    continue;
                };
                let t_plan = t0.elapsed();
                let t1 = Instant::now();
                let refs = drive(b.reference_chains(&plan, view, &Slicer::never()));
                let t_refs = t1.elapsed();
                let with: Vec<(&Cylinder, &WordRefs)> =
                    plan.words.iter().zip(&refs).filter_map(|(w, r)| r.as_ref().map(|r| (w, r))).collect();
                let mut chains = [0usize; MAX_CHAINS + 1];
                let mut steps: Vec<usize> = Vec::new();
                let mut word_err = 0.0f64;
                for (w, r) in &with {
                    chains[r.chains.len()] += 1;
                    steps.push(w.word.len() - r.m);
                    let syms: Vec<(&IfsMap<Map2>, u32)> = w.word.iter().map(|&s| b.sym_map(s).expect("symbol")).collect();
                    for &x0 in &w.seeds {
                        // Absolute, f64, to the end.
                        let mut x = x0;
                        let mut xm = x0;
                        for (k, (m, arm)) in syms.iter().enumerate() {
                            if k == r.m {
                                xm = x;
                            }
                            x = forward(m, x, *arm);
                        }
                        if (x[0] - view.centre[0]).hypot(x[1] - view.centre[1]) > 2.0 * view.radius {
                            continue;
                        }
                        // To `m`, then an offset from the nearest reference.
                        let (ci, _) = r
                            .chains
                            .iter()
                            .enumerate()
                            .map(|(i, ch)| (i, (xm[0] - ch.bases[0][0]).hypot(xm[1] - ch.bases[0][1])))
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .expect("a chain");
                        let ch = &r.chains[ci];
                        let mut d = [xm[0] - ch.bases[0][0], xm[1] - ch.bases[0][1]];
                        let d0 = d[0].hypot(d[1]);
                        let mut ok = true;
                        for (j, (m, arm)) in syms[r.m..].iter().enumerate() {
                            match map_forward_difference(&m.forward, ch.bases[j], d, *arm) {
                                Some(v) => d = v,
                                None => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        if !ok {
                            stray += 1;
                            continue;
                        }
                        let got = [ch.end[0] + d[0], ch.end[1] + d[1]];
                        let want = [x[0] - view.centre[0], x[1] - view.centre[1]];
                        let e = (got[0] - want[0]).hypot(got[1] - want[1]) / px;
                        word_err = word_err.max(e);
                        // A seed its reference is far from: the offset
                        // is as large as the gap, and in f32 that is lost.
                        if d0 > 1e-2 * ch.bases[0][0].hypot(ch.bases[0][1]).max(1e-9) {
                            stray += 1;
                        }
                    }
                }
                steps.sort_unstable();
                let q = |f: f64| steps.get(((steps.len() as f64 - 1.0) * f).round() as usize).copied().unwrap_or(0);
                println!(
                    "  {name} {z:.0e}: {} words, {} with offsets (steps min {} median {} max {}), chains {:?}, at the cap {} | plan {:.0} ms, refs {:.1} ms | offset vs absolute, worst {:.2e} px",
                    plan.words.len(),
                    with.len(),
                    q(0.0),
                    q(0.5),
                    q(1.0),
                    &chains[1..],
                    chains[MAX_CHAINS],
                    t_plan.as_secs_f64() * 1e3,
                    t_refs.as_secs_f64() * 1e3,
                    word_err
                );
                worst_px = worst_px.max(word_err);
            }
        }
        println!("  seeds far from every reference, or off a form: {stray}");

        // How many pieces a word's region has at `m`: what the chains must
        // cover (`Backward::pieces`).
        for name in ["grand-julian", "random1", "julian-disc"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")) else { continue };
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
            let Ok(b) = Backward::read(&cfg.flame, reg) else { continue };
            for z in [1e4f64, 1e6, 1e8] {
                let view = View::of(z, b.sample_point(0.75), 1280, 720);
                let Ok(plan) = b.plan_eval(view, PlanOptions::default(), &mut CpuEval) else { continue };
                let mut hist = [0usize; 8];
                let mut over = 0usize;
                let t0 = Instant::now();
                for (w, r) in plan.words.iter().zip(&plan.refs) {
                    let Some(r) = r else { continue };
                    let ch = &r.chains[0];
                    let end = [ch.end[0] + view.centre[0], ch.end[1] + view.centre[1]];
                    match b.pieces(&w.word, end, r.m, 256) {
                        Some(p) => hist[(p.len().max(1) as f64).log2().ceil().min(7.0) as usize] += 1,
                        None => over += 1,
                    }
                }
                println!(
                    "  {name} {z:.0e}: pieces per word, by power of two [1, 2, 3-4, 5-8, 9-16, 17-32, 33-64, 65+]: {hist:?}, over 256: {over} ({:.0} ms)",
                    t0.elapsed().as_secs_f64() * 1e3
                );
            }
        }
        assert!(worst_px < 1e-3, "the offset carry and the absolute replay disagree by {worst_px:.2e} px");

        // Not only seeds: a render sends EVERY attractor point through a
        // word, most of them far from the reference. Random sample points
        // through each word, absolute against offsets, in f64: where either
        // lands within a few view radii, they must agree.
        for name in ["grand-julian", "random1", "julian-disc"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")) else { continue };
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
            let Ok(b) = Backward::read(&cfg.flame, reg) else { continue };
            let view = View::of(1e4, b.sample_point(0.75), 1280, 720);
            let px = view.radius / (1280f64.hypot(720.0) / 2.0);
            let Ok(plan) = b.plan_eval(view, PlanOptions::default(), &mut CpuEval) else { continue };
            let (mut tried, mut near, mut bad, mut worst) = (0usize, 0usize, 0usize, 0.0f64);
            let mut worst_what = String::new();
            for (wi, (w, r)) in plan.words.iter().zip(&plan.refs).enumerate().step_by(7) {
                let Some(r) = r else { continue };
                let syms: Vec<(&IfsMap<Map2>, u32)> = w.word.iter().map(|&s| b.sym_map(s).expect("symbol")).collect();
                for si in (0..b.sample.len()).step_by(b.sample.len() / 300) {
                    let x0 = b.sample[si];
                    let mut x = x0;
                    let mut xm = x0;
                    for (k, (m, arm)) in syms.iter().enumerate() {
                        if k == r.m {
                            xm = x;
                        }
                        x = forward(m, x, *arm);
                    }
                    let ch = &r.chains[0];
                    let mut d = [xm[0] - ch.bases[0][0], xm[1] - ch.bases[0][1]];
                    let mut ok = true;
                    for (j, (m, arm)) in syms[r.m..].iter().enumerate() {
                        match map_forward_difference(&m.forward, ch.bases[j], d, *arm) {
                            Some(v) => d = v,
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    tried += 1;
                    let got = [ch.end[0] + d[0], ch.end[1] + d[1]];
                    let want = [x[0] - view.centre[0], x[1] - view.centre[1]];
                    let lands = |p: [f64; 2]| p[0].hypot(p[1]) < 4.0 * view.radius;
                    if !(lands(got) || lands(want)) {
                        continue;
                    }
                    near += 1;
                    let e = if ok { (got[0] - want[0]).hypot(got[1] - want[1]) / px } else { f64::INFINITY };
                    if e > 0.01 {
                        bad += 1;
                    }
                    if !(e <= worst) {
                        worst = e;
                        worst_what = format!("word {wi} (len {}, m {}), |δ_m| {:.2e}", w.word.len(), r.m, (xm[0] - ch.bases[0][0]).hypot(xm[1] - ch.bases[0][1]));
                    }
                }
            }
            println!("  {name} 1e4, random points: {tried} tried, {near} near the view, {bad} off by more than 0.01 px; worst {worst:.2e} px at {worst_what}");
        }

        // The saved view, deeper: what the plan finds there.
        if let Ok(t) = std::fs::read_to_string("output/grand-julian-missing-pieces.fflame") {
            let c: crate::config::FractalConfig = serde_json::from_str(&t).expect("config");
            let b = Backward::read(&c.flame, reg).expect("walked");
            for z in [1e6f64, 1e8, 1e9] {
                let view = View::of(z, [c.pan_x, c.pan_y], 1280, 720);
                match b.plan_eval(view, PlanOptions::default(), &mut CpuEval) {
                    Ok(p) => println!(
                        "  saved view {z:.0e}: {} words, mass {:.2e}, {} with references",
                        p.words.len(),
                        p.mass,
                        p.refs.iter().filter(|r| r.is_some()).count()
                    ),
                    Err(e) => println!("  saved view {z:.0e}: no plan: {e:?}"),
                }
            }
        }
    }

    /// **How far the plain GPU replay is from f64**, per sample, in
    /// pixels: the planner's kernel in endpoint mode runs `ct_apply_symbol`
    /// -- the render's own replay -- from sample points, and the CPU runs
    /// the same words from the same f32-rounded points in f64. The mean
    /// error is a BIAS, which a sampling average cannot remove: it moves
    /// the picture.
    #[test]
    #[ignore = "needs a GPU; reads output/flame-zoom"]
    fn how_far_the_plain_replay_is_from_f64() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame") else { return };
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        let b = Backward::read(&cfg.flame, reg).expect("walked");
        let (device, queue) = test_device();
        let mut gpu = crate::scene::plan_gpu::PlanGpu::new(&device, &queue, &cfg.flame, b.sample());
        for z in [1e4f64, 1e5] {
            let view = View::of(z, b.sample_point(0.75), 1280, 720);
            let px = view.radius / (1280f64.hypot(720.0) / 2.0);
            let plan = b.plan_eval(view, PlanOptions::default(), &mut CpuEval).expect("a plan");
            let pts: Vec<u32> = (0..b.sample.len() as u32).step_by(b.sample.len() / 64).collect();
            let jobs: Vec<EvalJob> = plan.words.iter().step_by(5).map(|w| EvalJob { word: &w.word, points: &pts }).collect();
            let got = gpu.endpoints(&jobs);
            let (mut n, mut sum, mut mean) = (0usize, 0.0f64, [0.0f64; 2]);
            let mut worst = 0.0f64;
            let mut k = 0;
            for j in &jobs {
                let syms: Vec<(&IfsMap<Map2>, u32)> = j.word.iter().map(|&s| b.sym_map(s).expect("symbol")).collect();
                for &i in j.points {
                    let g = got[k];
                    k += 1;
                    let Some(g) = g else { continue };
                    let p0 = b.sample[i as usize];
                    let mut x = [p0[0] as f32 as f64, p0[1] as f32 as f64];
                    for (m, arm) in &syms {
                        x = forward(m, x, *arm);
                    }
                    // Only where it lands near the view: elsewhere the
                    // error does not reach the picture.
                    if (x[0] - view.centre[0]).hypot(x[1] - view.centre[1]) > 2.0 * view.radius {
                        continue;
                    }
                    let e = [(g[0] as f64 - x[0]) / px, (g[1] as f64 - x[1]) / px];
                    n += 1;
                    sum += e[0].hypot(e[1]);
                    mean[0] += e[0];
                    mean[1] += e[1];
                    worst = worst.max(e[0].hypot(e[1]));
                }
            }
            let nn = n.max(1) as f64;
            println!(
                "  {z:.0e}: {n} samples in view; plain GPU replay against f64: mean |error| {:.3} px, worst {:.3} px, mean error (bias) ({:+.3}, {:+.3}) px",
                sum / nn,
                worst,
                mean[0] / nn,
                mean[1] / nn
            );
        }
    }

    /// **Gate 4, per sample: the render's replay against f64.** The
    /// words of a packed replay table run on the GPU through the render's
    /// own code (`PlanGpu::offset_endpoints`: `ct_apply_symbol` to `m`,
    /// then either the plain replay or `ct_offsets`), from sample points,
    /// and the CPU runs the same words from the same f32-rounded points in
    /// f64. Error in pixels, for samples landing in the view: the mean
    /// magnitude, the worst, and the mean VECTOR -- a bias, which moves
    /// the picture and which no amount of sampling averages away.
    #[test]
    #[ignore = "needs a GPU; reads output/flame-zoom"]
    fn the_offset_replay_holds_per_sample() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let (device, queue) = test_device();
        let mut worst_offsets = 0.0f64;
        for name in ["grand-julian", "random1", "julian-disc", "true-grand-julian"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")) else { continue };
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
            let Ok(b) = Backward::read(&cfg.flame, reg) else { continue };
            let gpu = crate::scene::plan_gpu::PlanGpu::new(&device, &queue, &cfg.flame, b.sample());
            for z in [1e4f64, 1e6, 1e8] {
                let view = View::of(z, b.sample_point(0.75), 1280, 720);
                let px = view.radius / (1280f64.hypot(720.0) / 2.0);
                let Ok(plan) = b.plan_eval(view, PlanOptions::default(), &mut CpuEval) else { continue };
                if plan.refs.is_empty() {
                    println!("  {name} {z:.0e}: no offsets at this depth");
                    continue;
                }
                let table = crate::scene::cylinder::pack_words(&plan, &cfg.flame);
                let stride = table[0] as usize;
                let blocks = table[3] as usize;
                let step = (plan.words.len() / 400).max(1);
                let pts: Vec<usize> = (0..b.sample.len()).step_by(b.sample.len() / 48).collect();
                let mut jobs: Vec<[f32; 4]> = Vec::new();
                let mut which: Vec<(usize, usize)> = Vec::new();
                for w in (0..plan.words.len()).step_by(step) {
                    // A word a renewal starts draws its blur on its first
                    // step, which f64 cannot follow; its offset steps are
                    // the same machinery as every other word's.
                    if plan.refs[w].is_none() || b.alphabet.iter().any(|a| a.sym == plan.words[w].word[0] && a.renewal.is_some()) {
                        continue;
                    }
                    let rec = (crate::scene::cylinder::HEADER_FLOATS + w * stride) as f32;
                    let blk = table[blocks + w];
                    for &i in &pts {
                        let p = b.sample[i];
                        jobs.push([rec, blk, p[0] as f32, p[1] as f32]);
                        which.push((w, i));
                    }
                }
                let got = gpu.offset_endpoints(&cfg.flame, &table, &jobs);
                let (mut n, mut plain_sum, mut off_sum) = (0usize, 0.0f64, 0.0f64);
                let (mut plain_bias, mut off_bias) = ([0.0f64; 2], [0.0f64; 2]);
                let (mut plain_worst, mut off_worst) = (0.0f64, 0.0f64);
                let mut off_errs: Vec<[f64; 2]> = Vec::new();
                for ((w, i), g) in which.iter().zip(&got) {
                    let syms: Vec<(&IfsMap<Map2>, u32)> = plan.words[*w].word.iter().map(|&s| b.sym_map(s).expect("symbol")).collect();
                    let p0 = b.sample[*i];
                    let mut x = [p0[0] as f32 as f64, p0[1] as f32 as f64];
                    for (m, arm) in &syms {
                        x = forward(m, x, *arm);
                    }
                    let want = [x[0] - view.centre[0], x[1] - view.centre[1]];
                    if want[0].hypot(want[1]) > 2.0 * view.radius {
                        continue;
                    }
                    n += 1;
                    let ep = [(g[0] as f64 - view.centre[0] - want[0]) / px, (g[1] as f64 - view.centre[1] - want[1]) / px];
                    let eo = [(g[2] as f64 - want[0]) / px, (g[3] as f64 - want[1]) / px];
                    plain_sum += ep[0].hypot(ep[1]);
                    off_sum += eo[0].hypot(eo[1]);
                    plain_worst = plain_worst.max(ep[0].hypot(ep[1]));
                    off_worst = off_worst.max(eo[0].hypot(eo[1]));
                    off_errs.push(eo);
                    for k in 0..2 {
                        plain_bias[k] += ep[k];
                        off_bias[k] += eo[k];
                    }
                }
                let nn = n.max(1) as f64;
                println!(
                    "  {name} {z:.0e}: {n} in view | plain: mean {:.3} worst {:.3} bias ({:+.3}, {:+.3}) px | offsets: mean {:.4} worst {:.4} bias ({:+.4}, {:+.4}) px",
                    plain_sum / nn,
                    plain_worst,
                    plain_bias[0] / nn,
                    plain_bias[1] / nn,
                    off_sum / nn,
                    off_worst,
                    off_bias[0] / nn,
                    off_bias[1] / nn
                );
                // Robust to the odd sample within an ulp of a cut, which
                // both replays send the other way round it (so does the
                // free chaos game): the 99th percentile, the bias of the
                // samples within it, and how many are off by a pixel.
                off_errs.sort_by(|a, b| a[0].hypot(a[1]).total_cmp(&b[0].hypot(b[1])));
                let keep = &off_errs[..(off_errs.len() * 99 / 100).max(1)];
                let p99 = keep.last().map_or(0.0, |e| e[0].hypot(e[1]));
                let tb = [
                    keep.iter().map(|e| e[0]).sum::<f64>() / keep.len() as f64,
                    keep.iter().map(|e| e[1]).sum::<f64>() / keep.len() as f64,
                ];
                let wild = off_errs.iter().filter(|e| e[0].hypot(e[1]) > 1.0).count();
                println!("      offsets: 99th percentile {p99:.4} px, its bias ({:+.4}, {:+.4}) px, {wild} of {n} off by more than a pixel", tb[0], tb[1]);
                assert!(p99 < 0.05, "{name} {z:.0e}: the offset replay's 99th percentile is {p99:.4} px");
                assert!(tb[0].hypot(tb[1]) < 0.01, "{name} {z:.0e}: the offset replay is biased by ({:.4}, {:.4}) px", tb[0], tb[1]);
                assert!(wild * 1000 <= n, "{name} {z:.0e}: {wild} of {n} samples off by more than a pixel");
                worst_offsets = worst_offsets.max(p99);
            }
        }
        println!("  worst 99th percentile anywhere: {worst_offsets:.4} px");
    }

    /// What the analysis and the walk say of the true Grand Julian
    /// (`assets/presets.fflame`'s first flame, extracted to
    /// `output/flame-zoom/true-grand-julian.fflame`): tracker item C2.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_the_true_grand_julian_is() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/true-grand-julian.fflame") else { return };
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        for (i, t) in cfg.flame.transforms.iter().enumerate() {
            println!("  transform {i}: weight {} vars {:?}", t.weight, t.variations);
        }
        match crate::scene::ifs_analysis::analyse_2d_maps(&cfg.flame, reg) {
            Ok(ifs) => println!("  analysis: {} maps", ifs.maps.len()),
            Err(errs) => {
                for e in errs {
                    println!("  analysis refuses: {e}");
                }
            }
        }
        match Backward::read(&cfg.flame, reg) {
            Ok(b) => println!("  walk: read, extent {:.3}", b.extent),
            Err(e) => println!("  walk refuses: {e}"),
        }
        let view = View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], 1280, 720);
        match crate::scene::cylinder::Cylinders::plan(&cfg.flame, reg, view) {
            Ok(p) => println!("  plan: {} words", p.words.len()),
            Err(e) => println!("  plan refuses: {e:?}"),
        }
        let Ok(b) = Backward::read(&cfg.flame, reg) else { return };
        let renewals: Vec<u32> = b.alphabet.iter().filter(|a| a.renewal.is_some()).map(|a| a.sym).collect();
        for (label, view) in [
            ("the preset's view", View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], 1280, 720)),
            ("1e2 on the attractor", View::of(1e2, b.sample_point(0.75), 1280, 720)),
            ("1e3 on the attractor", View::of(1e3, b.sample_point(0.75), 1280, 720)),
            ("1e3 inside the blob", View::of(1e3, [0.0, 0.0], 1280, 720)),
            ("1e5 on the attractor", View::of(1e5, b.sample_point(0.3), 1280, 720)),
        ] {
            let t0 = std::time::Instant::now();
            match b.plan_eval(view, PlanOptions::default(), &mut CpuEval) {
                Ok(p) => {
                    let ms = t0.elapsed().as_secs_f64() * 1e3;
                    let first = p.words.iter().filter(|w| renewals.contains(&w.word[0])).count();
                    let cov = coverage(&b, &p, view, 20_000);
                    // Mass and efficiency, renewal words against the rest.
                    let (mut rm, mut rd, mut om, mut od) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                    for w in &p.words {
                        let e = b.replay_full(&w.word, view);
                        if renewals.contains(&w.word[0]) {
                            rm += w.prob;
                            rd += w.prob * e;
                        } else {
                            om += w.prob;
                            od += w.prob * e;
                        }
                    }
                    println!(
                        "      renewal words: {:.1}% of the mass at efficiency {:.4}; the rest: {:.1}% at {:.4}",
                        100.0 * rm / (rm + om),
                        rd / rm.max(1e-300),
                        100.0 * om / (rm + om),
                        od / om.max(1e-300)
                    );
                    println!(
                        "  {label}: {} words ({first} start with the renewal), mass {:.2e}, efficiency {:.3}, {ms:.0} ms; coverage {}",
                        p.words.len(),
                        p.mass,
                        p.efficiency,
                        cov.map_or("n/a (unreachable)".to_string(), |c| format!("{c:.4}"))
                    );
                }
                Err(e) => println!("  {label}: no plan: {e:?}"),
            }
        }
    }

    /// **What changes between two animation frames' plans**
    /// (`output/flame-zoom/grand-julian-zoom{1,2}.fflame`: transform 1
    /// rotated 1.8 degrees). The words both plans hold, and the ones only
    /// one does; for those, how far from the view they branch off the
    /// shared words (the longest suffix -- last-applied symbols -- they
    /// share with any shared word); and each group drawn on the CPU, its
    /// words' samples binned where they land, to
    /// `output/deep-offsets/frames-*.png`.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_changes_between_frames() {
        use std::collections::HashSet;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut plans = Vec::new();
        for name in ["grand-julian-zoom1", "grand-julian-zoom2"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")) else { return };
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
            let b = Backward::read(&cfg.flame, reg).expect("walked");
            let view = View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], 360, 360);
            let plan = b.plan_eval(view, PlanOptions::default(), &mut CpuEval).expect("a plan");
            plans.push((name, b, view, plan));
        }
        let sets: Vec<HashSet<Vec<u32>>> = plans.iter().map(|p| p.3.words.iter().map(|w| w.word.clone()).collect()).collect();
        let shared: HashSet<&Vec<u32>> = sets[0].intersection(&sets[1]).collect();
        // The longest suffix a word shares with any shared word.
        let suffixes: HashSet<Vec<u32>> =
            shared.iter().flat_map(|w| (0..=w.len()).map(move |k| w[w.len() - k..].to_vec())).collect();
        for (i, (name, b, view, plan)) in plans.iter().enumerate() {
            let total: f64 = plan.words.iter().map(|w| w.prob).sum();
            let mut depth_hist = [0usize; 12];
            let (mut own_mass, mut own) = (0.0f64, 0usize);
            for w in &plan.words {
                if shared.contains(&w.word) {
                    continue;
                }
                own += 1;
                own_mass += w.prob;
                let k = (0..=w.word.len()).rev().find(|&k| suffixes.contains(&w.word[w.word.len() - k..])).unwrap_or(0);
                depth_hist[k.min(11)] += 1;
            }
            // The last symbols (transform, arm) of the words that branch off
            // at depth 0, against the shared words'.
            let last = |w: &[u32]| format!("t{}a{}", w[w.len() - 1] & 255, w[w.len() - 1] >> 8);
            let mut fam: std::collections::BTreeMap<String, (usize, f64, usize)> = Default::default();
            for w in &plan.words {
                let e = fam.entry(last(&w.word)).or_default();
                if shared.contains(&w.word) {
                    e.2 += 1;
                } else if !suffixes.contains(&w.word[w.word.len() - 1..]) {
                    e.0 += 1;
                    e.1 += w.prob;
                }
            }
            for (k, (n0, m0, ns)) in &fam {
                println!("      last symbol {k}: {ns} shared words, {n0} depth-0 own words ({:.2}% of the mass)", 100.0 * m0 / total);
            }
            // Trim (word-editing.md §4): what each setting removes.
            let total_share: f64 = plan.words.iter().map(|w| w.prob * w.eff).sum();
            for levels in [1usize, 2, 3, usize::MAX] {
                let mut row = format!("      levels {:>3}:", if levels == usize::MAX { "all".to_string() } else { levels.to_string() });
                for t in [0.001f64, 0.003, 0.01, 0.03, 0.1, 0.3] {
                    let keep = crate::scene::word_tree::kept_to(plan, t, levels);
                    let share: f64 = keep.iter().map(|&i| plan.words[i].prob * plan.words[i].eff).sum();
                    let t1a0 = keep.iter().filter(|&&i| last(&plan.words[i].word) == "t1a0").count();
                    row.push_str(&format!(" | {t}: -{:.2}% t1a0 {t1a0}", 100.0 * (1.0 - share / total_share)));
                }
                println!("{row}");
            }
            // What trim 0.05 at two levels removes, word by word.
            let keep: std::collections::HashSet<usize> = crate::scene::word_tree::kept_to(plan, 0.05, 2).into_iter().collect();
            let mut gone: std::collections::BTreeMap<String, (usize, f64, f64)> = Default::default();
            for (i, w) in plan.words.iter().enumerate() {
                if keep.contains(&i) {
                    continue;
                }
                let tail = w.word.iter().rev().take(2).map(|&s| format!("t{}a{}", s & 255, s >> 8)).collect::<Vec<_>>().join("<");
                let e = gone.entry(tail).or_default();
                e.0 += 1;
                e.1 += w.prob;
                e.2 += w.prob * w.eff;
            }
            for (k, (n, pm, sh)) in &gone {
                println!("      trim 0.05/2 removes {n} words ending {k}: {:.2}% of the probability, {:.4}% of the view", 100.0 * pm / total, 100.0 * sh / total_share);
            }
            // The t1a0 family's share of the view, against t3a0's.
            let fam_share = |k: &str| plan.words.iter().filter(|w| last(&w.word) == k).map(|w| w.prob * w.eff).sum::<f64>();
            println!("      share of the view: t1a0 {:.4}%, t3a0 {:.2}%", 100.0 * fam_share("t1a0") / total_share, 100.0 * fam_share("t3a0") / total_share);
            println!(
                "  {name}: {} words, {} shared ({:.1}% of its mass), {own} its own ({:.1}%); its own branch off the shared at depth [0..11]: {depth_hist:?}",
                plan.words.len(),
                plan.words.len() - own,
                100.0 * (total - own_mass) / total,
                100.0 * own_mass / total
            );
            // Draw: all its words, and its own alone.
            for (tag, pick) in [("all", 0u8), ("own", 1u8)] {
                const N: usize = 360;
                let mut img = vec![0.0f64; N * N];
                let px = 2.0 * view.radius / (N as f64 * std::f64::consts::SQRT_2);
                for w in &plan.words {
                    if pick == 1 && shared.contains(&w.word) {
                        continue;
                    }
                    let k = 24usize;
                    for j in 0..k {
                        let x0 = b.sample[(j * 4099 + w.word.len() * 131) % b.sample.len()];
                        let Some(p) = b.forward_along(&w.word, x0) else { continue };
                        let (u, v) = ((p[0] - view.centre[0]) / px + N as f64 / 2.0, (p[1] - view.centre[1]) / px + N as f64 / 2.0);
                        if u >= 0.0 && v >= 0.0 && (u as usize) < N && (v as usize) < N {
                            img[(N - 1 - v as usize) * N + u as usize] += w.prob / k as f64;
                        }
                    }
                }
                let top = img.iter().cloned().fold(0.0f64, f64::max).max(1e-300);
                let rgba: Vec<u8> = img
                    .iter()
                    .flat_map(|&d| {
                        let l = if d > 0.0 { ((d / top).ln() / 12.0 + 1.0).clamp(0.0, 1.0) } else { 0.0 };
                        let g = (l * 255.0) as u8;
                        [g, g, g, 255]
                    })
                    .collect();
                let _ = std::fs::create_dir_all("output/deep-offsets");
                let _ = image::save_buffer(format!("output/deep-offsets/frames-{i}-{tag}.png"), &rgba, N as u32, N as u32, image::ColorType::Rgba8);
            }
        }
    }

    /// The radix-built index is in `(Cell, u32)`'s own order, exactly --
    /// including cells far out and negative, as landings can be.
    #[test]
    fn a_radix_index_is_sorted_as_tuples_are() {
        let mut st = 0x1D_u64;
        let mut rnd = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (st >> 33) as i64
        };
        for spread in [3i64, 1000, 1 << 30] {
            let entries: Vec<(Cell, u32)> = (0..20_000u32)
                .filter(|i| i % 7 != 3)
                .map(|i| (((rnd() % spread - spread / 2) as i32, (rnd() % spread - spread / 2) as i32), i))
                .collect();
            let mut want = entries.clone();
            want.sort_unstable();
            assert_eq!(Index::build(entries).entries, want, "spread {spread}");
        }
    }

    /// **Phase 3's gate, natively**: the web's plan -- `plan_sliced`, the
    /// awaitable GPU evaluator, a slice of `WEB_SLICE` -- polled once per
    /// "frame" on this thread as the page polls it, with the rest of a
    /// 60 Hz frame slept between polls. The analysis is built cold, so its
    /// slicing is timed too. Every poll is timed, less the planner's kernel
    /// compile, which a browser does off the page's thread
    /// ([`Slicer::compiled`]); the plan must be the one the desktop's
    /// worker makes with the same evaluator.
    #[test]
    #[ignore = "needs a GPU and reads output/flame-zoom"]
    fn a_web_plan_takes_a_slice_a_frame() {
        use std::future::Future;
        use std::time::{Duration, Instant};
        const WEB_SLICE: Duration = Duration::from_millis(6);
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame").expect("grand-julian");
        let gj: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        let b = Backward::read(&gj.flame, reg).expect("armed");
        let (device, queue) = test_device();
        let mut views: Vec<(String, View)> = Vec::new();
        for z in [1e3f64, 1e6] {
            views.push((format!("x{z:.0e}"), View::of(z, b.sample_point(0.75), 1280, 720)));
        }
        if let Ok(t) = std::fs::read_to_string("output/grand-julian-missing-pieces.fflame") {
            let c: crate::config::FractalConfig = serde_json::from_str(&t).expect("config");
            views.push(("missing-pieces".into(), View::of(c.zoom as f64, [c.pan_x as f64, c.pan_y as f64], 1280, 720)));
        }
        let mut worst = 0.0f64;
        for (name, view) in views {
            // The desktop's plan: the worker's evaluator, blocking.
            let mut planner = crate::scene::plan_gpu::GpuPlanner::new(&device, &queue);
            let want = {
                let eval = planner.for_flame(&gj.flame, &b).expect("the kernel builds");
                b.plan_eval(view, PlanOptions::default(), eval).expect("a plan")
            };

            // The web's: cold, sliced, polled a frame at a time.
            Backward::forget_cached();
            let mut web_planner = crate::scene::plan_gpu::GpuPlanner::new(&device, &queue);
            let slicer = Slicer::traced(WEB_SLICE);
            let t0 = Instant::now();
            let mut polls: Vec<f64> = Vec::new();
            let mut compiled = 0.0f64;
            let got = {
                let fut = crate::scene::cylinder::Cylinders::plan_sliced(&gj.flame, reg, view, Some(&mut web_planner), &[], &[], 0, &slicer);
                let mut fut = std::pin::pin!(fut);
                let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
                loop {
                    slicer.begin();
                    let t = Instant::now();
                    let (c0, g0) = (slicer.compiled(), slicer.gaps().len());
                    let r = fut.as_mut().poll(&mut cx);
                    let ms = t.elapsed().as_secs_f64() * 1e3;
                    let c = (slicer.compiled() - c0).as_secs_f64() * 1e3;
                    compiled += c;
                    polls.push(ms - c);
                    if ms - c > 12.0 {
                        println!("   poll #{}: {ms:.1} ms ({c:.1} compiling) {:?}", polls.len() - 1, &slicer.gaps()[g0..]);
                    }
                    if let std::task::Poll::Ready(r) = r {
                        break r;
                    }
                    // The rest of the frame: the page renders, the browser
                    // runs the GPU's callbacks.
                    std::thread::sleep(Duration::from_secs_f64((16.7 - ms).max(1.0) / 1e3));
                }
            };
            let got = got.expect("a web plan");
            let total = t0.elapsed().as_secs_f64();
            let mut sorted = polls.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];
            let max = *sorted.last().unwrap();
            let over = polls.iter().filter(|p| **p > 16.0).count();
            println!(
                "== {name}: {} frames ({total:.2} s), poll p95 {p95:.1} ms, max {max:.1} ms, {over} over 16 ms | {} words | kernel compiled natively in {compiled:.1} ms, not counted",
                polls.len(),
                got.words.len()
            );
            // The longest polls, and where in the plan they fell.
            let mut idx: Vec<usize> = (0..polls.len()).collect();
            idx.sort_by(|a, c| polls[*c].partial_cmp(&polls[*a]).unwrap());
            let top: Vec<String> = idx.iter().take(5).map(|&i| format!("#{i}: {:.1} ms", polls[i])).collect();
            println!("   longest: {}", top.join(", "));
            let same = got.words.len() == want.words.len()
                && got.words.iter().zip(&want.words).all(|(a, c)| a.word == c.word && a.prob.to_bits() == c.prob.to_bits());
            assert!(same, "{name}: the web's plan is not the desktop's ({} words against {})", got.words.len(), want.words.len());
            worst = worst.max(max);
        }
        assert!(worst < 16.0, "a poll took {worst:.1} ms: past a 60 Hz frame");
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
    /// follows a word suffix through julian-disc's 1e2 walk, or through
    /// the view `WATCH_AT=name-frac-off` names.
    ///
    /// `ONLY=name` runs one flame's views; `COVER_N` counts that many
    /// in-view samples (1500), for a difference of a few misses to mean
    /// something; `MISS_DUMP=prefix` writes every missed history to a file
    /// per view; `MISS_WORD=t.a,...` says where the misses through that
    /// word were before it, against the sample and the landing index.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn why_is_this_view_empty() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        // `off` moves the view off its sample point by that many radii, so
        // it holds few or none of the sample and the cloud has to seed it.
        for (name, zoom, frac, off) in [
            ("julian-disc", 1e3f64, 0.25f64, 0.0f64),
            ("julian-disc", 1e2, 0.25, 0.0),
            ("random1", 1e4, 0.25, 0.0),
            ("random1", 1e3, 0.25, 0.0),
            ("julian-disc", 1e3, 0.25, 0.6),
            ("random1", 1e3, 0.25, 0.6),
            ("random1", 1e4, 0.25, 0.6),
            ("grand-julian", 1e3, 0.75, 0.6),
            ("julian-disc", 1e3, 0.25, 2.0),
            ("random1", 1e3, 0.25, 2.0),
            ("random1", 1e3, 0.6, 2.0),
            ("julian-disc", 1e3, 0.6, 2.0),
            ("grand-julian", 1e3, 0.75, 2.0),
            ("grand-julian", 1e3, 0.3, 2.0),
        ] {
            if std::env::var("ONLY_OFF").is_ok() && off < 1.0 || std::env::var("ONLY").is_ok_and(|n| n != name) {
                continue;
            }
            let text = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame")).expect("flame");
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
            let b = Backward::read(&cfg.flame, reg).expect("armed");
            let q0 = b.sample_point(frac);
            let r0 = View::of(zoom, q0, 1280, 720).radius;
            let q = [q0[0] + off * r0, q0[1] + 0.5 * off * r0];
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
                Ok(w) if std::env::var("WATCH_AT").map_or(name == "julian-disc" && zoom == 1e2, |v| v == format!("{name}-{frac}-{off}")) => w
                    .split(',')
                    .map(|t| {
                        let (a, b) = t.split_once('.').unwrap();
                        sym_of(a.parse().unwrap(), b.parse().unwrap())
                    })
                    .collect(),
                _ => Vec::new(),
            });
            let t_plan = web_time::Instant::now();
            let plan = b.plan_with(view, &mut tr);
            println!("   planned in {:.0} ms (CPU)", t_plan.elapsed().as_secs_f64() * 1e3);
            if plan.is_err() || std::env::var("WATCH").is_ok() && !tr.watched.is_empty() && tr.watch.as_ref().is_some_and(|w| !w.is_empty()) {
                for line in tr.watched.iter().take(120) {
                    println!("   watch: {line}");
                }
            }
            println!(
                "== {name} x{zoom:.0e} at {frac} off {off}: centre [{:.5}, {:.5}] r {:.2e}, extent {:.3}, {} alphabet, {} sample points in view by {:?}",
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
                // `MISS_WORD=t.a,...` (application order): where the missed
                // samples arriving through that word were, before it.
                let miss_word: Vec<u32> = std::env::var("MISS_WORD")
                    .map(|w| {
                        w.split(',')
                            .map(|t| {
                                let (a, b) = t.split_once('.').unwrap();
                                sym_of(a.parse().unwrap(), b.parse().unwrap())
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut xs: Vec<[f64; 2]> = Vec::new();
                let mut befores: Vec<([f64; 2], [f64; 2])> = Vec::new();
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
                    xs.push(x);
                    x = y;
                    hist.push(sym_of(t.index as u32, arm));
                    if hist.len() > longest + 4 {
                        hist.remove(0);
                        xs.remove(0);
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
                        let m = miss_word.len();
                        if m > 0 && hist.len() >= m && hist[hist.len() - m..] == miss_word[..] {
                            // xs[j] is the point hist[j] was applied to.
                            let j = hist.len() - m;
                            let after_first = if m >= 2 { xs[j + 1] } else { x };
                            befores.push((xs[j], after_first));
                        }
                    }
                    if n_in >= std::env::var("COVER_N").ok().and_then(|v| v.parse().ok()).unwrap_or(1500) {
                        break;
                    }
                }
                println!("   word lengths in the plan: {lens:?}");
                if !befores.is_empty() {
                    let ai = b.alphabet.iter().position(|a| a.sym == miss_word[0]).expect("symbol");
                    let rest = &miss_word[1..];
                    // Sample points in the region of the word's rest: the
                    // points its first symbol has to land near.
                    let region: Vec<usize> = (0..b.sample.len())
                        .filter(|&i| b.forward_along(rest, b.sample[i]).is_some_and(|z| (z[0] - view.centre[0]).hypot(z[1] - view.centre[1]) <= view.radius))
                        .collect();
                    let ring = |idx: &Index, p: [f64; 2]| -> Option<i32> {
                        let (cx, cy) = b.cell_of(p);
                        (0..=256i32).find(|&r| {
                            (-r..=r).any(|dx| (-r..=r).any(|dy| {
                                dx.abs().max(dy.abs()) == r && {
                                    let (lo, hi) = idx.bounds((cx + dx, cy + dy));
                                    hi > lo
                                }
                            }))
                        })
                    };
                    println!("   MISS_WORD: {} missed samples through it; cell {:.2e}; {} sample points in its rest's region, cells {:?}",
                        befores.len(), b.cell, region.len(),
                        region.iter().take(8).map(|&i| b.cell_of(b.sample[i])).collect::<Vec<_>>());
                    for (x0, y0) in befores.iter().take(10) {
                        println!("     before {:?} cell {:?} (sample ring {:?}); after the first symbol {:?} cell {:?} (landing ring {:?})",
                            x0, b.cell_of(*x0), ring(&b.grid, *x0), y0, b.cell_of(*y0), ring(&b.landing[ai], *y0));
                    }
                }
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
                if let Ok(path) = std::env::var("MISS_DUMP") {
                    let text: String = m.iter().map(|(w, n)| format!("{n} {}
", w.iter().map(|s| format!("{}.{}", sym_transform(*s), sym_arm(*s))).collect::<Vec<_>>().join(","))).collect();
                    std::fs::write(format!("{path}-{name}-{zoom:.0e}-{frac}-{off}.txt"), text).ok();
                }
                for (w, n) in m.iter().take(10) {
                    println!("     {n:>4}: {}", show(w));
                }
            }
            println!(
                "   plan: {} | nodes {} seeded {} pruned {} no_preimage {} no_arm {} floor {} cut {} carried {} empty {} nocand {} forced {} rescued {} hidden {} beam {}",
                match &plan { Ok(p) => format!("{} words", p.words.len()), Err(e) => format!("{e:?}") },
                tr.nodes_expanded, tr.seeded, tr.pruned, tr.no_preimage, tr.no_arm, tr.floor, tr.cut, tr.not_yet, tr.empty, tr.nocand, tr.forced, tr.rescued, tr.hidden, tr.beam
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

    /// **A removal holds at every zoom** (`docs/projects/word-editing.md`
    /// §5's gate), on the first animation frame. Two removals: the
    /// flicker's (`t1a0`), and one a map longer than a word the plain plan
    /// cuts whole, which the walk must refine to take out. At the frame's
    /// view and at shallower and deeper ones: no word ends with a removed
    /// pattern; and, against the plan without removals, how much went,
    /// which of its other words are missing, which are new, and how many
    /// pieces were kept whole holding a removed one.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn a_removal_holds_at_every_zoom() {
        use crate::scene::word_tree::{parse_pattern, pattern_text, removed};
        use std::collections::HashSet;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/grand-julian-zoom1.fflame") else { return };
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
        let b = Backward::read(&cfg.flame, reg).expect("walked");
        let base = View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], 360, 360);
        let plain = b.plan_eval(base, PlanOptions::default(), &mut CpuEval).expect("a plan");
        // The cut word holding the most of the view, and the symbol most
        // likely to be applied before it.
        let cut = plain
            .words
            .iter()
            .filter(|w| w.eff >= CUT_EFFICIENCY && !b.alphabet.iter().any(|a| a.sym == w.word[0] && a.renewal.is_some()))
            .max_by(|x, y| (x.prob * x.eff).total_cmp(&(y.prob * y.eff)))
            .expect("a cut word");
        let before = b.alphabet.iter().filter(|a| a.renewal.is_none()).max_by(|x, y| x.prob.total_cmp(&y.prob)).expect("a symbol").sym;
        let mut longer = vec![before];
        longer.extend_from_slice(&cut.word);
        let removals = vec![parse_pattern("t1a0").expect("parses"), longer.clone()];
        println!(
            "  removing t1a0, and {} (the cut word {} at {:.3}% of the view, one map longer)",
            pattern_text(&longer),
            pattern_text(&cut.word),
            100.0 * cut.prob * cut.eff / plain.words.iter().map(|w| w.prob * w.eff).sum::<f64>()
        );
        // The flicker alone, at the frame's view: what trim 0.05 takes
        // there, so the two should agree.
        let flicker = vec![parse_pattern("t1a0").expect("parses")];
        let alone = b.plan_eval(base, PlanOptions { removals: &flicker, ..Default::default() }, &mut CpuEval).expect("a plan");
        let trimmed = crate::scene::word_tree::trim_to(&plain, 0.05, 2);
        let same = alone.words.len() == trimmed.words.len() && alone.words.iter().zip(&trimmed.words).all(|(x, y)| x.word == y.word && x.prob == y.prob);
        println!(
            "  t1a0 alone at zoom {}: {} words, mass {:.6e}; trim 0.05/2: {} words, mass {:.6e}; the same words: {same}",
            cfg.zoom,
            alone.words.len(),
            alone.mass,
            trimmed.words.len(),
            trimmed.mass
        );
        println!("     zoom   words  plain words  view removed  view kept  plain words missing / new  unrefined  children not made");
        for f in [1.0 / 16.0, 0.25, 1.0, 4.0, 16.0] {
            let view = View::of(cfg.zoom as f64 * f, [cfg.pan_x, cfg.pan_y], 360, 360);
            let plain = b.plan_eval(view, PlanOptions::default(), &mut CpuEval).expect("a plan");
            let mut tr = Trace::default();
            let cut_off = b
                .plan_with_eval(view, &mut tr, PlanOptions { removals: &removals, ..Default::default() }, &mut CpuEval)
                .expect("a plan");
            for w in &cut_off.words {
                assert!(!removed(&removals, &w.word), "{} survived at zoom {}", pattern_text(&w.word), cfg.zoom as f64 * f);
            }
            let share = |p: &Cylinders| p.words.iter().map(|w| w.prob * w.eff).sum::<f64>();
            let went: f64 = plain.words.iter().filter(|w| removed(&removals, &w.word)).map(|w| w.prob * w.eff).sum();
            let have: HashSet<&Vec<u32>> = cut_off.words.iter().map(|w| &w.word).collect();
            let had: HashSet<&Vec<u32>> = plain.words.iter().map(|w| &w.word).collect();
            let missing = plain.words.iter().filter(|w| !removed(&removals, &w.word) && !have.contains(&w.word)).count();
            let new = cut_off.words.iter().filter(|w| !had.contains(&w.word)).count();
            println!(
                "  {:>7.1}  {:>6}  {:>11}  {:>11.3}%  {:>8.3}%  {:>12} / {:<10}  {:>9}  {:>6}",
                cfg.zoom as f64 * f,
                cut_off.words.len(),
                plain.words.len(),
                100.0 * went / share(&plain).max(1e-300),
                100.0 * share(&cut_off) / share(&plain).max(1e-300),
                missing,
                new,
                tr.unrefined,
                tr.removed
            );
            if f == 1.0 {
                assert!(
                    !have.contains(&cut.word) || tr.unrefined > 0,
                    "the cut word holding a removed piece was kept whole and not counted"
                );
            }
        }
    }

    /// **What the walk refuses of the Grand JuliaN generator's flames**
    /// (tracker C2c): the app's own generator puts a blob on its first
    /// transform -- `blur`, `bubble` with a `pre_blur`, `pie3D` or
    /// `starblur` -- and the corpus holds none of the kinds the walk
    /// refuses. `SEEDS` flames (400), refusals tallied by blob.
    #[test]
    #[ignore = "a measurement"]
    fn what_the_walk_refuses_of_generated_grand_julians() {
        use std::collections::BTreeMap;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let host = crate::script::ScriptHost::new();
        let text = include_str!("../../assets/scripts/generators/grand_julian.rhai");
        let n: u64 = std::env::var("SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
        let mut tally: BTreeMap<(String, String), usize> = BTreeMap::new();
        for seed in 1..=n {
            let out = host.run(text, &crate::config::FractalConfig::default(), seed, Default::default()).expect("the script runs");
            let t = &out.config.flame.transforms[0];
            let blob = ["blur", "bubble", "pie3D", "starblur"].iter().find(|v| t.variations.get(**v).is_some_and(|w| *w != 0.0)).copied().unwrap_or("?");
            let blob = match (blob, t.variations.get("pre_blur")) {
                ("bubble", Some(w)) => format!("bubble + pre_blur {:.1}", (w * 2.0).round() / 2.0),
                (b, _) => b.to_string(),
            };
            let why = match Backward::read(&out.config.flame, reg) {
                Ok(_) => "read".to_string(),
                Err(e) => e.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect::<String>().split(" short of").next().unwrap_or("").to_string(),
            };
            *tally.entry((blob, why)).or_default() += 1;
        }
        for ((blob, why), k) in &tally {
            println!("  {k:>4}  {blob:<24} {why}");
        }
    }

    /// **A variation that ignores its input is planned as a renewal**
    /// (tracker C2c): Grand JuliaN generator flames with each blob kind,
    /// planned at a few views, against an independent chaos game.
    #[test]
    #[ignore = "a measurement"]
    fn free_blurs_plan_completely() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let host = crate::script::ScriptHost::new();
        let text = include_str!("../../assets/scripts/generators/grand_julian.rhai");
        let mut done: std::collections::BTreeMap<&str, usize> = Default::default();
        for seed in 1..400u64 {
            let mut cfg = host.run(text, &crate::config::FractalConfig::default(), seed, Default::default()).expect("the script runs").config;
            if !cfg.flame.final_transforms.is_empty() {
                continue;
            }
            let t0 = &cfg.flame.transforms[0];
            let pre = t0.variations.get("pre_blur").copied().unwrap_or(0.0);
            let kind = if t0.variations.contains_key("bubble") {
                // A partial blur: too small to forget its input.
                if pre < 0.6 { Some("pre_blur<0.6") } else if pre < 1.6 { Some("pre_blur<1.6") } else { None }
            } else {
                ["blur", "pie3D", "starblur"].into_iter().find(|v| t0.variations.get(*v).is_some_and(|w| *w != 0.0))
            };
            if std::env::var("KINDS").is_ok_and(|k| !kind.is_some_and(|kind| k.split(',').any(|x| x == kind))) {
                continue;
            }
            let Some(kind) = kind else { continue };
            // A gaussian_blur, from a blur flame.
            let kind = if kind == "blur" && done.get("blur").copied().unwrap_or(0) >= 2 {
                let w = cfg.flame.transforms[0].variations["blur"];
                cfg.flame.transforms[0].remove_variation("blur");
                cfg.flame.transforms[0].set_variation("gaussian_blur", w);
                "gaussian_blur"
            } else {
                kind
            };
            if done.get(kind).copied().unwrap_or(0) >= 2 {
                continue;
            }
            *done.entry(kind).or_default() += 1;
            let b = Backward::read(&cfg.flame, reg).expect("reads");
            let w = cfg.flame.transforms[0].variations.get(kind).or(cfg.flame.transforms[0].variations.get("pre_blur")).copied().unwrap_or(0.0);
            println!("== seed {seed}: {kind} {w}, extent {:.2}", b.extent);
            for (label, view) in [
                ("its own view", View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], 1280, 720)),
                ("1e2 at 0.75", View::of(1e2, b.sample_point(0.75), 1280, 720)),
                ("1e3 at 0.3", View::of(1e3, b.sample_point(0.3), 1280, 720)),
                ("1e3 in the blob", View::of(1e3, b.ifs.maps[b.transforms[0].map].forward.apply([0.0, 0.0]), 1280, 720)),
            ] {
                let t = std::time::Instant::now();
                match b.plan_eval(view, PlanOptions::default(), &mut CpuEval) {
                    Ok(p) => println!(
                        "   {label:<16} {:>6} words, efficiency {:.3}, {:.0} ms; coverage {:?}",
                        p.words.len(),
                        p.efficiency,
                        t.elapsed().as_secs_f64() * 1e3,
                        coverage(&b, &p, view, 3000)
                    ),
                    Err(e) => println!("   {label:<16} {e:?}"),
                }
            }
            if done.len() == 6 && done.values().all(|n| *n >= 2) {
                break;
            }
        }
    }

    /// **A plan through a final covers what it plots** (tracker C2c): a
    /// Grand JuliaN generator flame with a `bipolar` final, planned at
    /// its own view, at a deep one, and at one holding where the plane's
    /// far points are plotted -- whose pull-back is everything -- against
    /// a chaos game plotted through the final.
    #[test]
    fn a_plan_through_a_final_covers_what_it_plots() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = include_str!("../../assets/scripts/generators/grand_julian.rhai");
        let cfg = crate::script::ScriptHost::new().run(text, &crate::config::FractalConfig::default(), 7, Default::default()).expect("the script runs").config;
        assert!(!cfg.flame.final_transforms.is_empty(), "seed 7 has a final");
        let b = Backward::read(&cfg.flame, reg).expect("reads");
        let inf = b.finals.as_ref().and_then(|f| f.infinity()).expect("bipolar plots far points at a point");
        for (label, view) in [
            ("its own view", View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], 320, 180)),
            ("1e3", View::of(1e3, b.plotted(b.sample_point(0.3)), 320, 180)),
            ("at infinity's image", View::of(40.0, inf, 320, 180)),
        ] {
            let p = b.plan_eval(view, PlanOptions::default(), &mut CpuEval).expect("a plan");
            let c = coverage(&b, &p, view, 1500);
            assert!(c.is_some_and(|c| c >= 0.99), "{label}: coverage {c:?} with {} words", p.words.len());
        }
    }

    /// **A final transform is planned through its pull-back** (tracker
    /// C2c): Grand JuliaN generator flames with a `bipolar` final, planned
    /// at views of the plotted picture, against an independent chaos game
    /// plotted through the same final.
    #[test]
    #[ignore = "a measurement"]
    fn finals_plan_completely() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let host = crate::script::ScriptHost::new();
        let text = include_str!("../../assets/scripts/generators/grand_julian.rhai");
        let mut done = 0;
        for seed in 1..400u64 {
            let cfg = host.run(text, &crate::config::FractalConfig::default(), seed, Default::default()).expect("the script runs").config;
            if cfg.flame.final_transforms.is_empty() {
                continue;
            }
            let b = Backward::read(&cfg.flame, reg).expect("reads");
            let shift = cfg.flame.final_transforms[0].get_variation_param_or_default("bipolar", "shift", reg);
            println!("== seed {seed}: bipolar final, shift {shift:.2}, extent {:.2}", b.extent);
            for (label, view) in [
                ("its own view", View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], 1280, 720)),
                ("1e2 at 0.75", View::of(1e2, b.plotted(b.sample_point(0.75)), 1280, 720)),
                ("1e3 at 0.3", View::of(1e3, b.plotted(b.sample_point(0.3)), 1280, 720)),
                ("1e4 at 0.55", View::of(1e4, b.plotted(b.sample_point(0.55)), 1280, 720)),
            ] {
                let t = std::time::Instant::now();
                match b.plan_eval(view, PlanOptions::default(), &mut CpuEval) {
                    Ok(p) => println!(
                        "   {label:<14} {:>6} words, efficiency {:.3}, {:.0} ms; coverage {:?}",
                        p.words.len(),
                        p.efficiency,
                        t.elapsed().as_secs_f64() * 1e3,
                        coverage(&b, &p, view, 3000)
                    ),
                    Err(e) => println!("   {label:<14} {e:?}"),
                }
            }
            done += 1;
            if done == 4 {
                break;
            }
        }
    }

    /// **What the inverse walk refuses across the corpus, and why**
    /// (tracker C2c). Every flame in `output/*.flame`, `output/flame-zoom`
    /// and `assets/presets.fflame` is read by `Backward::read`, and the
    /// refusals are tallied by reason, with the blurs each flame carries.
    #[test]
    #[ignore = "reads output/*.flame"]
    fn what_the_walk_refuses_across_the_corpus() {
        use std::collections::BTreeMap;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut flames: Vec<(String, crate::scene::transforms::Flame)> = Vec::new();
        // `output/*.fflame` are escape and simulation configs, whose flame
        // is the default one: only the XML flames there.
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir("output")
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|x| x.to_str()) == Some("flame")).collect())
            .unwrap_or_default();
        paths.extend(std::fs::read_dir("output/flame-zoom").map(|rd| rd.flatten().map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default());
        paths.push("assets/presets.fflame".into());
        paths.sort();
        for p in &paths {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let Ok(text) = std::fs::read_to_string(p) else { continue };
            match p.extension().and_then(|x| x.to_str()) {
                Some("flame") => {
                    if let Ok(cfgs) = crate::flame_xml::parse_flame_xml(&text) {
                        for (k, c) in cfgs.into_iter().enumerate() {
                            flames.push((format!("{stem}#{k}"), c.flame));
                        }
                    }
                }
                Some("fflame") => {
                    if let Ok(c) = serde_json::from_str::<crate::config::FractalConfig>(&text) {
                        flames.push((stem, c.flame));
                    } else if let Ok(cs) = serde_json::from_str::<Vec<crate::config::FractalConfig>>(&text) {
                        for (k, c) in cs.into_iter().enumerate() {
                            flames.push((format!("{stem}#{k}"), c.flame));
                        }
                    }
                }
                _ => {}
            }
        }
        let blurs = ["pre_blur", "blur", "gaussian_blur", "radial_blur", "pre_gaussian_blur", "post_blur"];
        let mut why: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut ok = 0usize;
        for (name, flame) in &flames {
            let carried: Vec<String> = flame
                .transforms
                .iter()
                .enumerate()
                .flat_map(|(i, t)| {
                    blurs.iter().filter_map(move |b| t.variations.get(*b).filter(|w| **w != 0.0).map(|w| format!("t{i} {b} {w}")))
                })
                .collect();
            let t0 = std::time::Instant::now();
            let r = Backward::read(flame, reg);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            match r {
                Ok(b) => {
                    ok += 1;
                    println!("  OK      {name:<40} {} symbols, {ms:.0} ms {}", b.alphabet.len(), carried.join("; "));
                }
                Err(e) => {
                    println!("  REFUSED {name:<40} {e} | {}", carried.join("; "));
                    // The reason without its numbers, to tally.
                    let key: String = e.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
                    why.entry(key).or_default().push(name.clone());
                }
            }
        }
        println!("
  {} flames, {ok} read", flames.len());
        let mut v: Vec<_> = why.into_iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(n.len()));
        for (k, n) in &v {
            println!("  {:>3}  {k}", n.len());
        }
        // **What would free the most**, greedily: each refused flame's
        // blockers -- the variations it names, and each other reason --
        // and the fewest fixes that clear the most flames.
        let blockers_of = |e: &str| -> std::collections::BTreeSet<String> {
            e.split("; ")
                .map(|part| match (part.find("uses `"), part.find("` has")) {
                    (Some(i), _) => part[i + 6..].split('`').next().unwrap_or("").to_string(),
                    (_, Some(_)) => part.split('`').nth(1).map_or(part.to_string(), |n| format!("{n} (a parameter)")),
                    _ => part.split(" (").next().unwrap_or(part).replace(|c: char| c.is_ascii_digit(), "#"),
                })
                .collect()
        };
        let mut need: Vec<std::collections::BTreeSet<String>> = Vec::new();
        for (_, flame) in &flames {
            if let Err(e) = Backward::read(flame, reg) {
                if !e.contains("forward planner is exact") && !e.contains("no transforms") {
                    need.push(blockers_of(&e));
                }
            }
        }
        println!("
  greedy cover of {} refused flames:", need.len());
        let mut have: std::collections::BTreeSet<String> = Default::default();
        for step in 1..=12 {
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for m in need.iter().filter(|m| !m.is_subset(&have)) {
                for x in m.difference(&have) {
                    *counts.entry(x.clone()).or_default() += 1;
                }
            }
            let Some((x, c)) = counts.into_iter().max_by_key(|(_, c)| *c) else { break };
            have.insert(x.clone());
            let freed = need.iter().filter(|m| m.is_subset(&have)).count();
            println!("    +{step:<2} {x:<40} (in {c:>2}) -> {freed:>2} clear");
        }
    }

    /// **A grazing blob does not take the draws** (the quality cliff at
    /// zoom 1559/1560 on the true Grand Julian, tracker C2b). The view's
    /// disc at 1080x1055 grazes the blob's by 2e-6, outside the frame, and
    /// its blur word was kept by geometry: 99.9% of the plan, landing
    /// never, and raising the floor so the rest stopped at 72 words --
    /// efficiency 0.001, where 256x256 (1% smaller) planned 1,116 at 0.24.
    /// Now either side of the graze, at either size, the plans agree: the
    /// blob is measured and dropped as negligible, blur words no longer
    /// raise the floor, and most draws land.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn a_grazing_blob_does_not_take_the_draws() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut plans = Vec::new();
        for f in [1, 2] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/grand-julian-zoom-quality{f}.fflame")) else { return };
            let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("config");
            let b = Backward::read(&cfg.flame, reg).expect("walked");
            for (w, h) in [(1080u32, 1055u32), (256, 256)] {
                let view = View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], w, h);
                let mut tr = Trace::default();
                let p = b.plan_with_eval(view, &mut tr, PlanOptions::default(), &mut CpuEval).expect("a plan");
                let drawn: f64 = p.words.iter().map(|w| w.prob * w.draw).sum();
                let landed = p.words.iter().map(|w| w.prob * w.draw * w.eff).sum::<f64>() / drawn;
                println!(
                    "  zoom {} at {w}x{h}: {} words, mass {:.3e}, efficiency {:.3}, draws landing {:.3}, blur words dropped {}",
                    cfg.zoom,
                    p.words.len(),
                    p.mass,
                    p.efficiency,
                    landed,
                    tr.renewal_dropped
                );
                assert!(landed > 0.5, "zoom {} at {w}x{h}: only {landed:.3} of draws land", cfg.zoom);
                plans.push(p.mass);
            }
        }
        let (lo, hi) = plans.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), &m| (lo.min(m), hi.max(m)));
        assert!(hi / lo < 1.05, "the plans either side of the graze disagree: mass {lo:.3e} to {hi:.3e}");
    }
}
