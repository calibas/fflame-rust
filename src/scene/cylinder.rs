//! Cylinder targeting — stage 2 of
//! [flame-deep-zoom.md](../../docs/projects/flame-deep-zoom.md).
//!
//! Stage 1 biases the chaos game's transform selection and corrects
//! with a likelihood ratio. Its mechanism is exact and its payoff was
//! not there ([§11](../../docs/projects/flame-deep-zoom.md)): a fixed
//! per-transform bias is still a polynomial share of the orbit, so it
//! moves a constant that the weight's own variance swamps. This is
//! the stage where the asymptotics change.
//!
//! # The identity
//!
//! The invariant measure satisfies `μ = Σ_i p_i (S_i)_* μ`. Unrolled
//! along any **complete antichain** `A` — a prefix-free set of words
//! that every infinite path passes through exactly once —
//!
//! ```text
//! μ = Σ_{a ∈ A} p_a · (S_a)_* μ,     p_a = ∏ p(symbol)
//! ```
//!
//! with `S_a = S_{a_k} ∘ … ∘ S_{a_1}` for a word stored oldest-first,
//! which is the order the chaos game applies them in.
//!
//! Restricted to a viewport `V`, every word whose image misses `V`
//! contributes nothing, so
//!
//! ```text
//! μ|_V = Σ_{a ∈ A_V} p_a · (S_a)_* μ |_V,   A_V = {a ∈ A : S_a(B) ∩ V ≠ ∅}
//! ```
//!
//! **That is exact, not an approximation.** Sampling `a` from `A_V`
//! with probability `p_a / P(A_V)`, drawing `x ~ μ` by running the
//! true chaos game, and plotting `S_a(x)` with weight `P(A_V)`
//! therefore has expectation `μ|_V` — for any `A_V` built this way,
//! however loose the bound that built it. A loose bound admits words
//! whose image does not really reach `V`; their samples land outside
//! and are simply not seen. Efficiency, never correctness.
//!
//! # Why this is different from stage 1
//!
//! The weight is `P(A_V)` for **every** sample — one number for the
//! whole view, not a product accumulated along an orbit. There is no
//! window, no epoch, and no weight variance at all. And every sample
//! lands in the viewport by construction, where the unbiased game's
//! share falls off polynomially with the zoom. The cost is the
//! prefix's length, which grows like `log(1/zoom)`.
//!
//! # What this module does not do
//!
//! **Affine transforms only, and no xaos.** Both are deliberate and
//! both are about the bound rather than the identity:
//!
//! - the image bound needs a Lipschitz constant per map. For an
//!   affine one it is `σ_max` of the 2×2, exact and free. For a
//!   variation it is the deep-zoom plan's §7 item 1, the shared piece
//!   the escape-time plan also wants, and it is not built. Without it
//!   a nonlinear map has no honest bound and an enumeration built on
//!   a guessed one would be wrong rather than loose;
//! - under xaos the first symbol's probability is conditioned on
//!   whatever transform the burn-in ended on, so the sampling table
//!   is per-predecessor rather than one table. The identity holds
//!   unchanged; the bookkeeping does not, and it can follow.
//!
//! [`plan`](Cylinders::plan) reports which of these turned a flame
//! away, so the panel can say so rather than silently rendering the
//! ordinary way.

use crate::scene::transforms::Flame;
use crate::scene::ifs_analysis::Affine2;
use crate::variations::bound::Ball;

/// How far a word may be expanded before the enumeration gives up.
///
/// A word's image shrinks geometrically, so the depth needed is
/// `log(R/V)/log(1/λ)` — about 60 at a zoom of 2^40 with a
/// contraction of a half. The cap is a guard against a flame whose
/// contraction is so weak that the enumeration would run away, not a
/// budget: a word that reaches it is kept as it is, which is sound
/// (its image is merely larger than the viewport, so its samples
/// spread wider than they need to).
pub const MAX_DEPTH: usize = 96;

/// How many words the enumeration may keep.
///
/// A viewport is covered by a handful of cylinders at its own scale,
/// so this is not normally approached. It bounds the pathological
/// case — a viewport straddling many pieces of a set with a large
/// branching factor — where the right answer is to render the
/// ordinary way rather than to enumerate ten thousand words.
pub const MAX_WORDS: usize = 4096;

/// Where the arm lives inside a word's symbol.
///
/// A symbol used to be a transform index and nothing else. For a
/// variation whose draw picks one of several images — `julian` and its
/// kin, see `bound::ARMED` — the arm has to be part of the word, or
/// the bound covers every arm at once and is an annulus whose radius
/// does not depend on the input.
///
/// Packed into one number rather than carried alongside, so a word
/// stays a `Vec<u32>` and the kernel still reads one value per step.
/// Transforms are capped at 128, so eight bits hold one; arms are
/// capped at [`crate::variations::bound::MAX_ARMS`], so the whole
/// symbol fits in sixteen and survives the trip through an `f32`
/// exactly.
///
/// **Arm zero encodes to the bare transform index**, which is what
/// every word written before arms existed already holds.
pub const ARM_SHIFT: u32 = 8;

pub fn sym_of(transform: u32, arm: u32) -> u32 {
    transform | (arm << ARM_SHIFT)
}

pub fn sym_transform(sym: u32) -> u32 {
    sym & ((1 << ARM_SHIFT) - 1)
}

pub fn sym_arm(sym: u32) -> u32 {
    sym >> ARM_SHIFT
}

/// Whether a many-valued variation's arms are in the alphabet, so a
/// word names which image the replay must take.
///
/// ON. The planner for an armed flame is the inverse walk
/// (`backward.rs`), the replay masks the transform out of the symbol
/// and sets `ct_forced_arm` before applying it, and the armed
/// variations' draws are wrapped by the shader builder to read it --
/// only under `CYLINDER_REPLAY`, so an untargeted shader is
/// byte-identical to what it was. The picture gate is
/// `a_targeted_grand_julian_render_is_the_untargeted_render`.
pub const ARMS_ENABLED: bool = true;

/// Family M: the most probability increments the map-keyed walk will
/// deliver before giving up and charging the rest to
/// [`Cylinders::lost`]. A pop is cheap unless it reaches a map for the
/// first time, so this bounds the walk without bounding the useful
/// work.
pub const MAX_POPS: usize = 2_000_000;

/// Family M: the most distinct MAPS the walk will hold. Each costs one
/// region, which is the expensive thing here.
pub const MAX_NODES: usize = 100_000;

/// Family M: measure below which an increment is charged to
/// [`Cylinders::lost`] rather than followed. A map re-reached by ever
/// longer words receives a geometrically shrinking series of these,
/// and this is where the series is cut.
pub const MEASURE_FLOOR: f64 = 1e-12;

/// Family M: how long the map-keyed walk may take before it stops and
/// charges what is left to [`Cylinders::lost`].
///
/// **A wall-clock budget, because the symptom is wall-clock.** `plan`
/// runs on the UI thread on every pan, and the node and pop caps do
/// not bound time: a flame whose cover is large pays milliseconds per
/// region, and `schottky2` spent a hundred seconds inside caps that
/// were never reached. This is the one limit that is about the thing
/// that actually goes wrong.
pub const MOBIUS_TIME_BUDGET: std::time::Duration = std::time::Duration::from_millis(1000);

/// Family J: how many discs the root cover may hold.
///
/// A flame like `grand-julian` has NO invariant disc — `julian` with
/// `dist = -1` is `|p|^(-1/2)`, unbounded at the origin, and the
/// attractor surrounds the origin, so every disc containing it
/// contains the pole. What exists instead is a COVER: many small discs
/// that between them hold the attractor and individually keep away
/// from the pole. Family M met the same wall first and this is the
/// same machinery, generalised in `mobius::Cover::push_by`.
pub const ROOT_COVER_DISCS: usize = 512;

/// Family J: each root disc starts at this fraction of the sampled
/// attractor's extent and halves until it can be pushed.
pub const ROOT_COVER_START: f64 = 0.05;

/// Family J: orbit points kept to refine a pushed cover against, out
/// of [`ROOT_COVER_SAMPLE`] sampled.
pub const ROOT_COVER_ANCHORS: usize = 400;

/// Family J: how long an orbit is run through the BOUNDS, as points,
/// to find where the attractor is.
pub const ROOT_COVER_SAMPLE: usize = 20_000;

/// Family J: the most of the sampled orbit the root cover may leave
/// outside itself before the flame is refused.
///
/// This is not a quality knob. A point outside the cover is a point
/// whose images the root images do not contain, so the antichain
/// would be missing measure it does not know about — and the render
/// would quietly under-weight a piece of the picture. Measured,
/// `grand-julian` leaks 0.0 and `julian-disc` leaks 0.34 at a
/// 256-disc cap; the second one is refused, correctly, until its
/// cover is built better.
pub const ROOT_COVER_LEAK: f64 = 0.02;

/// Family J: the most a pushed disc's radius may be, as a multiple of
/// the attractor's extent, before the disc is split instead of
/// accepted.
///
/// The true image of the attractor lies INSIDE the attractor, so an
/// image bound several times the extent is a bound that has wandered
/// off — and near a pole it wanders off long before it stops being
/// finite. One extent is the generous reading of "still describes the
/// attractor".
pub const ROOT_IMAGE_CAP: f64 = 1.0;

/// Family J: how wide the frontier beam is.
///
/// Wider than family M's, and it has to be. A Schottky flame has four
/// symbols, so a level offers `4 × beam` candidates and a beam of 96
/// barely bites; `grand-julian` has twenty-five, so a level offers
/// `25 × beam` and the beam decides almost everything. Measured, at
/// 96 the branch that actually contains a deep view is dropped before
/// it is distinguishable and the plan comes back `ViewIsEmpty`.
pub const ROOT_BEAM: usize = 96;

/// Family J: the most angular pieces one symbol's root image may be
/// cut into.
///
/// **A root image is an annular SECTOR, and how wide the sector is
/// decides whether a ball can hold it.** `julian` with `n` arms
/// divides the angle by `n`, so arm `k`'s image of the attractor is a
/// `360/n`-degree sector of an annulus around the origin — which is
/// where the map's own pole is. Measured on `grand-julian`: the
/// fifteen-arm transform gives 24° sectors whose enclosing balls sit
/// 0.08 clear of the origin and walk fine, while the two-arm
/// transform gives a 180° sector whose enclosing ball must contain
/// the origin, and no amount of refining the cover changes that — it
/// is geometry, not looseness. The fix is to stop insisting on ONE
/// ball: cut the sector by angle into pieces that each clear the
/// pole. The enumeration already carries a bag-shaped region for
/// family M, so this costs pieces-per-symbol bound evaluations and no
/// new machinery.
pub const ROOT_PIECES: usize = 128;

/// Family J: how long a word the root images are walked through to
/// see whether the ordinary Ball walk will terminate from them.
pub const ROOT_PROBE_DEPTH: usize = 12;

/// Family J: how many such words are tried.
pub const ROOT_PROBE_WORDS: usize = 32;

/// Family J: what fraction of the attractor's extent a probe word's
/// disc has to fall below to count as having contracted.
pub const ROOT_PROBE_SHRINK: f64 = 1e-2;

/// Family J: the fraction of probe words that have to contract before
/// the flame is accepted.
///
/// **This is the test that keeps the cover root from being tried on
/// flames it cannot help**, and it asks the question the enumeration
/// actually cares about. The obvious gate — demand that a root image
/// be small — is the wrong one: `julian` with power 2 maps the plane
/// two-to-one, so arm 0's image is a WEDGE of the attractor and its
/// enclosing ball is the size of the attractor by nature. Measured,
/// `grand-julian`'s first symbol gives 2.7e1 against an extent of
/// 1.9e1 and then falls to 1.5e-5 by depth six; `spherical`'s gives
/// 1.9e1 and stays there, because a disc bound through an inversion
/// never shrinks. Walking a few words is what tells those two apart,
/// and it costs a few hundred bound evaluations.
pub const ROOT_PROBE_PASS: f64 = 0.5;

/// Why a flame cannot be cylinder-targeted.
#[derive(Debug, Clone, PartialEq)]
pub enum NoCylinders {
    /// A transform contains something with no forward bound, so
    /// there is no way to push a disc through it. Names the transform
    /// and what inside it refused.
    Unbounded { index: usize, why: String },
    /// No origin-centred disc could be found that every map sends
    /// into itself, so the enumeration has no root to refine from.
    /// A flame whose attractor sits far from the origin, or whose
    /// bounds are too loose to close.
    NoInvariantBall,
    /// The flame's colour is not an affine function of the running
    /// colour coordinate -- a `WritesColor` or `WritesRgb` variation
    /// is active -- so the word's colour cannot be folded into the
    /// two coefficients the kernel applies, and forcing a prefix
    /// would plot the right point in the wrong colour.
    ColourNotAffine,
    /// The flame carries a non-trivial xaos matrix: the first
    /// symbol's probability is conditional on the burn-in's last
    /// transform, which this table does not carry.
    Xaos,
    /// A transform expands (σ_max ≥ 1), so a word's image does not
    /// shrink and the enumeration has no stopping rule.
    NotContractive(usize),
    /// The flame has no transforms, or every weight is zero.
    Empty,
    /// No word's image reaches the viewport: the view is off the
    /// attractor entirely, and there is nothing to target.
    ViewIsEmpty,
    /// The walk ran out of its time budget before it found anything.
    ///
    /// Distinct from [`Self::ViewIsEmpty`], which says there is
    /// nothing there. This says we did not look long enough, which is
    /// a different thing to tell someone — and reporting it as an
    /// empty view was exactly the sort of true-but-useless message
    /// that sent this project chasing the wrong problem before.
    TimedOut { nodes: usize },
    /// The enumeration hit [`MAX_WORDS`] — the viewport straddles too
    /// many pieces for targeting to be worth it.
    TooManyWords(usize),
}

/// The viewport, in world coordinates.
///
/// `world_to_pixel` is `(p − pan) · zoom` mapped so the SHORTER
/// screen axis spans ±2, so the view is the rectangle of half-extent
/// `2/zoom` in that axis and `2·aspect/zoom` in the other, centred on
/// the pan. Carried as a centre and a half-diagonal because every
/// test here is against a disc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub centre: [f64; 2],
    /// Half the diagonal: the radius of a disc containing the frame.
    pub radius: f64,
}

impl View {
    /// The view a config frames, from its zoom, pan and aspect.
    pub fn of(zoom: f64, pan: [f64; 2], width: u32, height: u32) -> Self {
        let short = width.min(height).max(1) as f64;
        let hx = 2.0 / zoom * (width as f64 / short);
        let hy = 2.0 / zoom * (height as f64 / short);
        Self { centre: pan, radius: (hx * hx + hy * hy).sqrt() }
    }
}

/// One enumerated word: the transforms to force, oldest first, and
/// the probability of the chaos game having chosen them.
#[derive(Debug, Clone, PartialEq)]
pub struct Cylinder {
    /// The symbols in the order the chaos game applies them, so a
    /// replay is a forward loop. The composed map is
    /// `S_{last} ∘ … ∘ S_{first}`.
    pub word: Vec<u32>,
    /// `p_a = ∏ p(symbol)`, the true probability of this word.
    pub prob: f64,
    /// Where the ball lands under this word, and how big it is — the
    /// bound the enumeration stopped on.
    pub centre: [f64; 2],
    pub radius: f64,
    /// A few points the inverse walk found landing in the view under
    /// this word: where the replay's reference orbits start
    /// (`docs/projects/deep-zoom-precision.md` §2). Empty from the
    /// other enumerations, and where the walk kept a word on its
    /// replay alone.
    pub seeds: Vec<[f64; 2]>,
}

/// The enumerated antichain, restricted to the words that reach the
/// view.
#[derive(Debug, Clone, PartialEq)]
pub struct Cylinders {
    pub words: Vec<Cylinder>,
    /// `P(A_V) = Σ p_a` over the kept words: the weight every forced
    /// sample carries, and the share of the invariant measure the
    /// viewport holds.
    pub mass: f64,
    /// **The share of the invariant measure this enumeration could
    /// not account for.**
    ///
    /// A word whose region cannot be bounded — a pole inside it, a
    /// variation with no forward bound at those parameters — is
    /// dropped, and every word extending it with it. Its `prob` is
    /// exactly the measure of that whole subtree, so adding it here
    /// once is the complete cost.
    ///
    /// Zero for every flame that targets today, and the gate
    /// `every_working_flame_loses_nothing` keeps it that way. It is
    /// non-zero only for the leaky regions the inversive family
    /// needs (`docs/projects/inversive-targeting.md`), where the
    /// alternative is refusing the flame outright.
    ///
    /// **The render stays unbiased CONDITIONAL on the kept set.** The
    /// forced sampler draws from `words` with probability
    /// `p_a / mass`, so what it draws is right; the lost measure is
    /// simply absent from the picture, and this is how much of it
    /// there is. Never hidden from the user — a render that is
    /// missing a part of the attractor has to say so.
    pub lost: f64,
    /// **Attractor the enumeration's region never covered**, for a
    /// flame whose root is a sampled COVER rather than a proven ball.
    ///
    /// Distinct from [`Self::lost`], which is measure belonging to
    /// words that were dropped. This is measure belonging to words
    /// that were KEPT, in the gaps between the sample points the
    /// cover was built from — see
    /// `docs/projects/inversive-targeting.md` §12. Zero for every
    /// flame with a real invariant ball, which is every flame that
    /// targeted before family M existed.
    pub sampling_leak: f64,
    /// The fraction of forced samples that land in the frame, weighted
    /// by the words' probabilities. One everywhere except the inverse
    /// walk, which verifies each word by replaying it forward on a
    /// sample of the attractor and reports what it measured. Below
    /// one is WASTE, not error: the kernel discards a plot outside the
    /// frame, so the picture is unchanged and the speedup is scaled.
    pub efficiency: f64,
    /// The deepest word kept, which is what the prefix costs per
    /// plotted sample.
    pub depth: usize,
    /// Whether every map in the flame is affine, so a word composes
    /// to ONE matrix on the CPU and the kernel applies a single
    /// multiply. False when any map is merely bounded, where the
    /// kernel has to walk the word's symbols instead.
    pub composable: bool,
    /// The view this was enumerated for. Kept because the packing
    /// needs it: a word's translation is expressed RELATIVE to this
    /// point, which is what keeps a deep zoom out of f32's teeth.
    pub view_centre: [f64; 2],
    /// Per word, its reference orbits for the replay in offsets, where
    /// the view is deep enough to need them (`Backward::reference_chains`).
    /// Empty when nothing was computed: every word then replays in
    /// absolute f32, as before.
    pub refs: Vec<Option<crate::scene::backward::WordRefs>>,
    /// The flame's forward maps as the shader's rows
    /// (`forward_delta::forward_row`), one per transform index, for the
    /// offset steps. Empty with `refs`.
    pub offset_rows: Vec<f32>,
}

impl Cylinders {
    /// How much work one plotted sample costs against the unbiased
    /// game's, as a ratio of useful samples per map application.
    ///
    /// The unbiased game applies one map per iteration and a
    /// `mass` fraction of its samples land in view. Forcing applies
    /// `depth` maps plus one free-orbit step and every sample lands
    /// in view, so the ratio is `1 / (mass · (depth + 1))`.
    ///
    /// This is the number the whole stage is for, and it grows
    /// without bound as the view shrinks: `mass` falls like a power
    /// of the zoom while `depth` grows like its logarithm.
    pub fn speedup(&self) -> f64 {
        if self.mass <= 0.0 {
            return f64::INFINITY;
        }
        1.0 / (self.mass * (self.depth as f64 + 1.0))
    }

    /// **The family-M enumeration: cylinders keyed by their map.**
    ///
    /// The ordinary expansion walks WORDS. For a Möbius IFS that is
    /// the wrong index: the flame's alphabet holds each generator and
    /// its inverse, so `a·a⁻¹·w` is the same map as `w`, and a
    /// word-indexed walk treats them as different cylinders. Measured
    /// on `schottky1`, a raw random word's region stalls at 0.567
    /// while a backtrack-avoiding one reaches 8.8e-8 — the same
    /// geometry, the same cover, eight orders apart.
    ///
    /// Merging by map is not an optimisation, it is the correct
    /// index. The measure decomposition
    /// `μ = Σ_w p_w (S_w)_* μ` groups by MAP: every word carrying the
    /// same `S_w` contributes the same push-forward, so their
    /// probabilities add and their regions are computed once. Measured
    /// at depth 8: 9,842 distinct maps from 65,536 words, which is the
    /// free-group reduced count — merging by map IS reduction, and it
    /// stays correct when the group has extra relations, which these
    /// flames do (their isometric circles overlap).
    ///
    /// # Why increments, and not a breadth-first sweep
    ///
    /// `w` has length `k` and `a·a⁻¹·w` has length `k+2`, so a
    /// level-synchronous walk meets them at different levels and can
    /// never merge them. This walks PROBABILITY INCREMENTS in
    /// decreasing order instead: each pop carries some measure to a
    /// map, and a map re-reached later simply receives more. A node's
    /// region is computed once, when it is first reached — which is
    /// also when its shortest word is known, and the shortest word is
    /// the cheapest prefix for the kernel to replay.
    ///
    /// Increments below `MEASURE_FLOOR` are charged to
    /// [`Cylinders::lost`] rather than followed, so the truncation is
    /// a number the panel shows.
    fn plan_mobius(
        mf: &crate::scene::mobius::MobiusFlame,
        weights: &[f64],
        total_w: f64,
        view: View,
        sampling_leak: f64,
    ) -> Result<Self, NoCylinders> {
        use crate::scene::mobius::{MapKey, Word};
        use std::collections::HashMap;

        /// What the walk decided about a map, once.
        enum Verdict {
            /// Its region fits the view: a cylinder, at this index.
            Emit(usize),
            /// Its region misses the view. Every extension misses too,
            /// because a child's image is a subset of its parent's.
            Miss,
            /// Bigger than the view and meeting it: keep going.
            Expand,
        }
        struct Node {
            word: Vec<u32>,
            map: Word,
            verdict: Verdict,
        }

        let mut nodes: Vec<Node> = Vec::new();
        // **Increments coalesce before they are delivered.**
        //
        // A map is re-reached by every word that folds to it, and the
        // series is geometric — so delivering each arrival separately
        // means popping the same node hundreds of times for ever
        // smaller amounts. Measured before this: four million pops for
        // seven thousand nodes, 543 apiece, and the walk ran out of
        // budget at depth 13 with the frontier already turning over.
        //
        // Instead each node carries what it is owed. A pop takes the
        // whole outstanding amount at once, and a node is only queued
        // when it goes from owed-nothing to owed-something. The heap
        // key can then be stale — the amount may have grown after the
        // push — which costs some ordering and no correctness.
        let mut pending: Vec<f64> = Vec::new();
        let mut queued: Vec<bool> = Vec::new();
        let mut seen: HashMap<MapKey, usize> = HashMap::new();
        let mut kept: Vec<Cylinder> = Vec::new();
        let mut lost = 0.0f64;

        // Increments to deliver, largest first. `f64` has no `Ord`, so
        // the heap carries the bits of a known-finite positive float,
        // whose ordering as an integer is the ordering as a float.
        let mut heap: std::collections::BinaryHeap<(u64, usize)> =
            std::collections::BinaryHeap::new();
        let bits = |p: f64| -> u64 { p.to_bits() };
        let unbits = |b: u64| -> f64 { f64::from_bits(b) };

        let root = Word { map: crate::scene::mobius::Moebius::IDENTITY };
        let root_key = root.map.key().ok_or(NoCylinders::NoInvariantBall)?;
        nodes.push(Node { word: Vec::new(), map: root, verdict: Verdict::Expand });
        pending.push(1.0);
        queued.push(true);
        seen.insert(root_key, 0);
        heap.push((bits(1.0), 0));

        let started = std::time::Instant::now();
        let mut regions_computed = 0usize;
        let mut timed_out = false;
        let mut pops = 0usize;
        let mut by_depth: Vec<[usize; 3]> = vec![[0; 3]; MAX_DEPTH + 2];
        while let Some((_, idx)) = heap.pop() {
            queued[idx] = false;
            let inc = std::mem::replace(&mut pending[idx], 0.0);
            if inc <= 0.0 {
                continue;
            }
            pops += 1;
            // Checked every so often rather than every pop: `Instant::now`
            // is not free, and a pop that only adds a number is cheap
            // enough that asking the clock would dominate it.
            if pops > MAX_POPS
                || (pops % 256 == 0 && started.elapsed() > MOBIUS_TIME_BUDGET)
            {
                lost += inc;
                timed_out = true;
                heap.clear();
                for p in pending.iter_mut() {
                    lost += std::mem::take(p);
                }
                break;
            }

            // A map is classified once, when it is first reached; the
            // region is the expensive part and is never recomputed.
            match nodes[idx].verdict {
                Verdict::Miss => continue,
                Verdict::Emit(ci) => {
                    kept[ci].prob += inc;
                    continue;
                }
                Verdict::Expand => {}
            }

            for (a, wa) in weights.iter().enumerate() {
                if !(*wa > 0.0) {
                    continue;
                }
                let child_inc = inc * (wa / total_w);
                if child_inc < MEASURE_FLOOR {
                    lost += child_inc;
                    continue;
                }
                let Some(cmap) = mf.extend(&nodes[idx].map, a) else {
                    lost += child_inc;
                    continue;
                };
                let Some(key) = cmap.map.key() else {
                    lost += child_inc;
                    continue;
                };
                if let Some(&existing) = seen.get(&key) {
                    // The same map by a different word: same cylinder,
                    // more measure. This is the whole point.
                    match nodes[existing].verdict {
                        Verdict::Emit(ci) => kept[ci].prob += child_inc,
                        Verdict::Miss => {}
                        Verdict::Expand => {
                            pending[existing] += child_inc;
                            if !queued[existing] {
                                queued[existing] = true;
                                heap.push((bits(pending[existing]), existing));
                            }
                        }
                    }
                    continue;
                }

                let mut word = Vec::with_capacity(nodes[idx].word.len() + 1);
                word.push(a as u32);
                word.extend_from_slice(&nodes[idx].word);
                if word.len() > MAX_DEPTH {
                    lost += child_inc;
                    continue;
                }
                regions_computed += 1;
                let Ok(cover) = mf.region(&cmap, &word) else {
                    lost += child_inc;
                    continue;
                };
                let Some(enc) = cover.enclosing() else {
                    lost += child_inc;
                    continue;
                };
                let verdict = if !cover.meets_disc(view.centre, view.radius) {
                    Verdict::Miss
                } else if enc.r <= view.radius {
                    if kept.len() >= MAX_WORDS {
                        lost += child_inc;
                        continue;
                    }
                    kept.push(Cylinder {
                        word: word.clone(),
                        prob: child_inc,
                        centre: enc.c,
                        radius: enc.r,
                        seeds: Vec::new(),
                    });
                    Verdict::Emit(kept.len() - 1)
                } else {
                    Verdict::Expand
                };
                by_depth[word.len().min(MAX_DEPTH + 1)][match verdict {
                    Verdict::Miss => 0,
                    Verdict::Emit(_) => 1,
                    Verdict::Expand => 2,
                }] += 1;
                let expand = matches!(verdict, Verdict::Expand);
                nodes.push(Node { word, map: cmap, verdict });
                pending.push(0.0);
                queued.push(false);
                let ni = nodes.len() - 1;
                seen.insert(key, ni);
                if expand {
                    pending[ni] = child_inc;
                    queued[ni] = true;
                    heap.push((bits(child_inc), ni));
                }
                if nodes.len() >= MAX_NODES {
                    lost += child_inc;
                    break;
                }
            }
        }

        if std::env::var("CYL_STATS").is_ok() {
            println!(
                "     family M: {} nodes, {} regions, {} cylinders, {} pops, lost {:.3e}",
                nodes.len(),
                regions_computed,
                kept.len(),
                pops,
                lost
            );
            println!("       depth   miss   emit   expand");
            for (d, c) in by_depth.iter().enumerate() {
                if c[0] + c[1] + c[2] > 0 {
                    println!("       {d:>5}  {:>5}  {:>5}  {:>6}", c[0], c[1], c[2]);
                }
            }
        }
        if kept.is_empty() {
            return Err(if timed_out {
                NoCylinders::TimedOut { nodes: nodes.len() }
            } else {
                NoCylinders::ViewIsEmpty
            });
        }
        let mass: f64 = kept.iter().map(|c| c.prob).sum();
        let depth = kept.iter().map(|c| c.word.len()).max().unwrap_or(0);
        Ok(Self {
            words: kept,
            mass,
            lost,
            sampling_leak,
            efficiency: 1.0,
            depth,
            composable: false,
            view_centre: view.centre,
            refs: Vec::new(),
            offset_rows: Vec::new(),
        })
    }

    /// Enumerate the words whose image reaches `view`, or say why the
    /// flame cannot be targeted.
    pub fn plan(
        flame: &Flame,
        registry: &crate::variations::VariationRegistry,
        view: View,
    ) -> Result<Self, NoCylinders> {
        Self::plan_inner(flame, registry, view, ARMS_ENABLED, Default::default())
    }

    /// The same, with the arms of many-valued variations in the
    /// alphabet whatever [`ARMS_ENABLED`] says.
    ///
    /// For measuring family J before the kernel can replay an arm.
    pub fn plan_armed(
        flame: &Flame,
        registry: &crate::variations::VariationRegistry,
        view: View,
    ) -> Result<Self, NoCylinders> {
        Self::plan_inner(flame, registry, view, true, Default::default())
    }

    /// [`Self::plan`] with a budget and a cancel flag for the inverse
    /// walk -- what the app's background planner needs. The other
    /// planners are fast or carry their own budgets and ignore both.
    pub fn plan_opts(
        flame: &Flame,
        registry: &crate::variations::VariationRegistry,
        view: View,
        opts: crate::scene::backward::PlanOptions,
    ) -> Result<Self, NoCylinders> {
        Self::plan_inner(flame, registry, view, ARMS_ENABLED, opts)
    }

    /// What refuses any plan before a planner is chosen: no transforms,
    /// xaos, or a colour that is not affine.
    fn plannable(flame: &Flame, registry: &crate::variations::VariationRegistry) -> Result<(), NoCylinders> {
        if flame.transforms.is_empty() {
            return Err(NoCylinders::Empty);
        }
        if flame.has_xaos() {
            return Err(NoCylinders::Xaos);
        }

        // Colour must stay an affine function of the running colour
        // coordinate, because that is what lets a whole word fold
        // into the two coefficients the kernel applies. A
        // `WritesColor` or `WritesRgb` variation breaks that, and the
        // forced sample would land in the right place wearing the
        // wrong colour -- a bug that looks like an artistic choice.
        let reg = registry;
        for t in &flame.transforms {
            if t.weight <= 0.0 {
                continue;
            }
            for name in t.ordered_variation_names(reg) {
                if t.variations.get(&name).copied().unwrap_or(0.0) == 0.0 {
                    continue;
                }
                let writes = reg.get(&name).is_some_and(|i| {
                    i.has_feature(crate::variations::Feature::WritesColor)
                        || i.has_feature(crate::variations::Feature::WritesRgb)
                });
                if writes {
                    return Err(NoCylinders::ColourNotAffine);
                }
            }
        }
        Ok(())
    }

    /// Whether some transform draws among several images -- a flame the
    /// inverse walk plans.
    fn armed(flame: &Flame) -> bool {
        flame.transforms.iter().any(|t| {
            t.weight > 0.0
                && t.variations.iter().any(|(n, w)| *w != 0.0 && crate::variations::bound::arms_for(n).is_some())
        })
    }

    /// Why an armed flame gets no plan: the inverse walk refused it.
    fn walk_refused(flame: &Flame, why: &str) -> NoCylinders {
        let index = flame
            .transforms
            .iter()
            .position(|t| {
                t.weight > 0.0
                    && t.variations.iter().any(|(n, w)| *w != 0.0 && crate::variations::bound::arms_for(n).is_some())
            })
            .unwrap_or(0);
        NoCylinders::Unbounded { index, why: format!("the inverse walk cannot plan this flame: {why}") }
    }

    /// **A plan as a future** -- the web's way to plan (phase 3 of
    /// `gpu-cylinder-planning.md`), polled once a frame. An armed flame's
    /// inverse walk -- building its sample and index, then walking its
    /// levels -- yields at `slicer`'s ticks and while `gpu` answers; any
    /// other flame is planned as [`Self::plan`] plans it, at once. The
    /// same plan as `plan` makes with the same evaluator.
    pub async fn plan_sliced(
        flame: &Flame,
        registry: &crate::variations::VariationRegistry,
        view: View,
        gpu: Option<&mut crate::scene::plan_gpu::GpuPlanner>,
        slicer: &crate::scene::backward::Slicer,
    ) -> Result<Self, NoCylinders> {
        use crate::scene::backward::{Backward, Blocking, CpuEval, PlanOptions, Trace, TIME_BUDGET};
        slicer.tick().await;
        if !(ARMS_ENABLED && Self::armed(flame)) {
            return Self::plan(flame, registry, view);
        }
        Self::plannable(flame, registry)?;
        let b = match Backward::cached_sliced(flame, registry, slicer).await {
            Ok(b) => b,
            Err(why) => return Err(Self::walk_refused(flame, &why)),
        };
        // Nothing blocks, so the web's plan needs no inline budget; the
        // desktop's safety net applies.
        let opts = PlanOptions { budget: TIME_BUDGET, ..Default::default() };
        let mut tr = Trace::default();
        slicer.tick().await;
        let eval = match gpu {
            // Past its f32, the CPU plans (`Backward::gpu_resolves`).
            Some(g) if b.gpu_resolves(view) => g.for_flame_sliced(flame, &b, slicer).await,
            _ => None,
        };
        match eval {
            Some(eval) => b.walk(view, &mut tr, opts, eval, slicer).await,
            None => b.walk(view, &mut tr, opts, &mut Blocking(&mut CpuEval), slicer).await,
        }
    }

    fn plan_inner(
        flame: &Flame,
        registry: &crate::variations::VariationRegistry,
        view: View,
        family_j: bool,
        opts: crate::scene::backward::PlanOptions,
    ) -> Result<Self, NoCylinders> {
        let n = flame.transforms.len();
        Self::plannable(flame, registry)?;
        let _ = n;

        // **Armed**: some transform draws among several images, so a
        // word has to say which. Everything below that is specific to
        // arms is keyed on this and not on `family_j`, so a flame
        // without arms takes exactly the path it always took.
        let armed = family_j && Self::armed(flame);

        // **The inverse walk first, for an armed flame.** A flame the
        // inverse-walk analysis accepts is planned by pulling the view
        // back through its inverses (`backward.rs`), which is exact
        // where the forward bound below is structurally loose -- see
        // `docs/projects/inversive-targeting.md` §24. A flame it
        // refuses falls through to the forward machinery.
        if armed {
            // The inverse walk is the ONLY planner for an armed flame.
            // The forward cover/bag walk it replaced (§23) is
            // structurally unable to contract here and measured 17 to
            // 60 seconds a plan -- on the UI thread, on every pan -- so
            // a flame the inverse walk refuses is refused, with its
            // reason, rather than handed to it.
            return match crate::scene::backward::Backward::cached(flame, registry) {
                Ok(b) => b.plan_for(flame, view, opts),
                Err(why) => Err(Self::walk_refused(flame, &why)),
            };
        }

        // **Family M first**: a flame built from similarities,
        // `linear` and `spherical` is a Möbius IFS, and none of the
        // machinery below applies to it. Its maps have poles, so no
        // disc is invariant and `invariant_ball` cannot succeed; they
        // are not contractions, so the check below would refuse them;
        // and a disc bound through an inversion never shrinks, so the
        // enumeration would not terminate even if it started. See
        // `docs/projects/inversive-targeting.md`.
        // Gated OFF -- see `mobius::ENABLED` for the measurements.
        // The module stays fully exercised by its own tests; this is
        // the one place that decides whether a real render may wait on
        // it, and today it may not.
        let mobius = crate::scene::mobius::ENABLED
            .then(|| {
                crate::scene::mobius::MobiusFlame::read(
                    flame,
                    registry,
                    crate::scene::mobius::ROOT_SAMPLE,
                )
            })
            .flatten();

        // Each transform's selection probability, and whether the
        // whole flame is affine -- which decides whether a word can
        // be composed into one matrix or has to be replayed symbol by
        // symbol in the kernel.
        let mut weights: Vec<f64> = Vec::with_capacity(n);
        let mut total_w = 0.0f64;
        let mut composable = true;
        let mut probe_failure: Option<NoCylinders> = None;
        for (i, t) in flame.transforms.iter().enumerate() {
            if mobius.is_none()
                && crate::scene::ifs_analysis::transform_affine_2d(t, registry).is_err()
            {
                composable = false;
                // It still has to be BOUNDED, or there is nothing to
                // push a disc with. Asked at a disc that is certainly
                // in range — which for an inversive flame is a disc
                // holding the pole, where the answer is "unbounded"
                // and the flame is refused before anything has looked
                // at where its attractor actually is. So the refusal
                // is REMEMBERED rather than returned: a root built
                // from a cover never asks this question, and only if
                // no root can be found at all does this become the
                // reason.
                if probe_failure.is_none() {
                    if let Err(why) = crate::scene::ifs_ball::transform_ball_2d(
                        t,
                        registry,
                        Ball::new([0.0, 0.0], 1.0),
                    ) {
                        probe_failure =
                            Some(NoCylinders::Unbounded { index: i, why: why.to_string() });
                    }
                }
            }
            let w = (t.weight as f64).max(0.0);
            total_w += w;
            weights.push(w);
        }
        if !(total_w > 0.0) {
            return Err(NoCylinders::Empty);
        }
        // Deferring the probe is for the cover root's benefit alone.
        // Without it the answer is what it always was, returned where
        // it always was, so no flame outside family J sees a
        // different refusal or a different order of refusals.
        if !armed {
            if let Some(why) = probe_failure.take() {
                return Err(why);
            }
        }

        // **One bounder per transform, built once.**
        //
        // Everything below pushes discs through these thousands of
        // times; resolving the variation names per disc made a deep
        // view cost tens of milliseconds of pure name lookup. See
        // `ifs_ball::Bounder`.
        let mut bounders = Vec::with_capacity(n);
        for (i, t) in flame.transforms.iter().enumerate() {
            if mobius.is_some() {
                break;
            }
            match crate::scene::ifs_ball::Bounder::new(t, registry) {
                Ok(b) => bounders.push(b),
                Err(why) => {
                    return Err(NoCylinders::Unbounded { index: i, why: why.to_string() })
                }
            }
        }

        // Family M does not use the word walk below at all: its
        // cylinders are keyed by MAP, not by word, because the flame's
        // alphabet holds each generator and its inverse. See
        // `plan_mobius`.
        if let Some(mf) = &mobius {
            return Self::plan_mobius(mf, &weights, total_w, view, mf.leak);
        }

        // **The alphabet.** One symbol per transform normally; one per
        // (transform, arm) when a variation's draw picks among several
        // images and `ARMS_ENABLED` says to enumerate them. A
        // transform's weight divides evenly among its arms, because
        // the draw is uniform over them — that is what
        // `floor(n · rng_nextf())` means.
        let mut alphabet: Vec<(u32, f64)> = Vec::new();
        for (i, w) in weights.iter().enumerate() {
            if !(*w > 0.0) {
                continue;
            }
            let arms = if armed { bounders[i].arms().max(1) } else { 1 };
            for a in 0..arms {
                alphabet.push((sym_of(i as u32, a), w / arms as f64));
            }
        }
        if alphabet.is_empty() {
            return Err(NoCylinders::Empty);
        }

        // The ball every map sends into itself: the enumeration's
        // root, and what `S_a(B)` is the image of.
        //
        // When no such ball exists — every disc holding the attractor
        // holds a pole — the root is a COVER instead, and what the
        // enumeration starts from is the cover pushed once per symbol.
        // Measured, one ball suffices from the very first step, so
        // only the root pays the cover's price and the walk below is
        // the ordinary cheap one. See
        // `docs/projects/inversive-targeting.md` §22.
        let mut root_images: Option<std::collections::HashMap<u32, Vec<Ball>>> = None;
        let (root_c, root_r, sampling_leak) = match invariant_ball(flame, registry) {
            Ok((c, r)) => (c, r, 0.0),
            Err(no_ball) => {
                let cover_root = if armed {
                    bounded_root_images(&bounders, &weights, &alphabet)
                } else {
                    Err("not enabled".to_string())
                };
                match cover_root {
                    Ok((enclosing, images, leak)) => {
                        root_images = Some(images);
                        (enclosing.c, enclosing.r, leak)
                    }
                    Err(_why) => return Err(probe_failure.unwrap_or(no_ball)),
                }
            }
        };

        // Contraction, measured on the root rather than read off a
        // matrix. For an affine map the two agree (`σ_max` exactly);
        // for a bounded one there is no matrix to read, and what the
        // enumeration actually needs is that a word's disc shrinks.
        //
        // Skipped for family M, where it is the wrong question:
        // `spherical` is not a Euclidean contraction anywhere, and a
        // disc bound through it never shrinks. What contracts is a
        // WORD, measured at −0.30 per step, and the cover is what can
        // see it.
        for (i, t) in flame.transforms.iter().enumerate() {
            if mobius.is_some() || root_images.is_some() || weights[i] <= 0.0 {
                continue;
            }
            let _ = t;
            let img = bounders[i]
                .apply(Ball::new(root_c, root_r))
                .map_err(|why| NoCylinders::Unbounded { index: i, why: why.to_string() })?;
            if !(img.r < root_r) {
                return Err(NoCylinders::NotContractive(i));
            }
        }

        // Breadth-first, because the frontier is then ordered by
        // depth and the antichain comes out sorted — which makes the
        // completeness check below a statement about levels rather
        // than about a traversal.
        // `word` is in the order the chaos game APPLIES the maps,
        // so extending it means prepending — see the comment at the
        // extension. The disc is carried alongside only to prune
        // against; it is recomputed from the root each time, because
        // there is no way to extend it incrementally once the new map
        // goes on the inside.
        struct Node {
            word: Vec<u32>,
            prob: f64,
            centre: [f64; 2],
            radius: f64,
            /// Family M only: the whole word as one Möbius map, so a
            /// child costs a complex matrix multiply rather than a
            /// walk from the root.
            mob: Option<crate::scene::mobius::Word>,
        }
        let mut frontier = vec![Node {
            word: Vec::new(),
            prob: 1.0,
            centre: root_c,
            radius: root_r,
            mob: mobius.as_ref().map(|mf| mf.empty_word()),
        }];
        let mut kept: Vec<Cylinder> = Vec::new();
        // Measure that fell out of the enumeration; see `Cylinders::lost`.
        let mut lost = 0.0f64;

        // The disc a word's image lies in: push the root through the
        // word's maps, in the order the chaos game applies them.
        //
        // Recomputed from the root for every candidate rather than
        // extended from the parent, and that is the whole point of
        // this function's shape. See `Node` below.
        let disc_of = |word: &[u32]| -> Option<Ball> {
            let mut b = Ball::new(root_c, root_r);
            for &sym in word {
                b = bounders[sym_transform(sym) as usize]
                    .apply_arm(b, sym_arm(sym))
                    .ok()?;
            }
            Some(b)
        };

        // **Family J's region is a BAG of balls**, because the root
        // image of one symbol is an annular sector that no single ball
        // holds without swallowing the pole (see [`ROOT_PIECES`]). The
        // first symbol picks the bag; the rest of the word pushes
        // every piece. A piece that cannot be pushed takes the whole
        // word with it, which the caller charges to `lost` — dropping
        // it silently would leave a hole in the region and the pruning
        // below would then be unsound.
        //
        // **And it collapses back to one ball the moment it can.** A
        // bag of a hundred pieces costs a hundred bound evaluations
        // per symbol, and a word twenty long costs two thousand — the
        // difference between a plan and a hang. The pieces exist only
        // to keep the pole out; once the word has contracted far
        // enough that one ball round the whole bag clears every pole,
        // there is nothing left for them to do, and collapsing only
        // GROWS the region so containment survives it.
        let first_arm: Vec<u32> = {
            let mut seen = Vec::new();
            let mut out = Vec::new();
            for &(sym, _) in &alphabet {
                let t = sym_transform(sym);
                if !seen.contains(&t) {
                    seen.push(t);
                    out.push(sym);
                }
            }
            out
        };
        let clears = |b: &Ball| -> bool {
            first_arm.iter().all(|&sym| {
                matches!(
                    bounders[sym_transform(sym) as usize].apply_arm(*b, sym_arm(sym)),
                    Ok(r) if r.r.is_finite() && r.c[0].is_finite()
                )
            })
        };
        let bag_of = |word: &[u32]| -> Option<Vec<Ball>> {
            let images = root_images.as_ref()?;
            let mut rest = word.iter();
            let mut bag = images.get(rest.next()?)?.clone();
            for &sym in rest {
                let b = &bounders[sym_transform(sym) as usize];
                let arm = sym_arm(sym);
                let mut next = Vec::with_capacity(bag.len());
                for piece in &bag {
                    match b.apply_arm(*piece, arm) {
                        Ok(img) if img.r.is_finite() && img.c[0].is_finite() => next.push(img),
                        // **One piece of a hundred reaching a pole
                        // must not take the word with it.** The
                        // pieces are an artifact of the cover, not of
                        // the flame: a piece that blows up holds a
                        // pole the true image does not, and refusing
                        // the whole word for it killed every word
                        // past depth seven -- measured, that is why a
                        // 1e4 view came back `ViewIsEmpty` while a
                        // 1e2 view planned. Dropping it shrinks the
                        // region, which is the same approximation the
                        // cover already makes between its samples,
                        // and it is charged to the same leak.
                        _ => {}
                    }
                }
                if next.is_empty() {
                    return None;
                }
                bag = next;
                if bag.len() > 1 {
                    if let Some(one) = enclosing_ball(&bag).filter(&clears) {
                        bag.clear();
                        bag.push(one);
                    }
                }
            }
            Some(bag)
        };

        for _depth in 0..MAX_DEPTH {
            if frontier.is_empty() {
                break;
            }
            let mut next: Vec<Node> = Vec::new();
            for node in frontier.drain(..) {
                for &(sym, w) in &alphabet {
                    let i = sym_transform(sym) as usize;
                    // **The new symbol is applied FIRST, not last.**
                    //
                    // A word is `S_{a_k} ∘ … ∘ S_{a_1}` and the chaos
                    // game applies `a_1` first, so prepending a symbol
                    // makes the new map the INNERMOST one. That is
                    // what makes the child's image a subset of the
                    // parent's: `S_a(S_j(B)) ⊆ S_a(B)` because
                    // `S_j(B) ⊆ B`, and the pruning below is sound
                    // only because of it.
                    //
                    // Appending instead — applying the new map on the
                    // OUTSIDE — was the original shape and it is
                    // wrong: the child is then the parent's disc under
                    // a different map, which lands somewhere else
                    // entirely and is not contained in the parent at
                    // all. Pruning on "the child is inside the parent"
                    // then discards branches that do reach the view,
                    // and their share of the measure never gets drawn.
                    //
                    // It is invisible at a fixed point, where the word
                    // is `S_i^k` and the two conventions agree, which
                    // is exactly why every gate here passed while a
                    // generic point of a real flame went empty.
                    let mut word = Vec::with_capacity(node.word.len() + 1);
                    word.push(sym);
                    word.extend_from_slice(&node.word);
                    // Family M: extend the composed map, then ask it
                    // for the region in one push.
                    let child_mob = match (&mobius, &node.mob) {
                        (Some(mf), Some(parent)) => match mf.extend(parent, i) {
                            Some(w) => Some(w),
                            None => {
                                lost += node.prob * (w / total_w);
                                continue;
                            }
                        },
                        _ => None,
                    };
                    // Computed before the bound is attempted, because
                    // a bound that fails still has to say how much
                    // measure went with it.
                    let prob = node.prob * (w / total_w);
                    // **The view test has to see the COVER, not the
                    // disc around it.**
                    //
                    // A family-M word's region is a scatter of small
                    // discs over the attractor, and the disc enclosing
                    // them is the attractor's own extent for the first
                    // twenty symbols — the contraction does not bite
                    // before then. Tested through that enclosing disc,
                    // every child meets the view, nothing prunes, and
                    // the frontier grows like the branching factor to
                    // the depth: measured, `TooManyWords(4096)` at
                    // every zoom after five seconds. Asking the discs
                    // themselves is the same question asked where the
                    // structure is.
                    let mut cover = None;
                    let mut bag: Option<Vec<Ball>> = None;
                    let region = match (&mobius, &child_mob) {
                        (Some(mf), Some(cw)) => match mf.region(cw, &word) {
                            Ok(c) => {
                                let e = c.enclosing().map(|d| Ball::new(d.c, d.r));
                                cover = Some(c);
                                e
                            }
                            Err(_) => None,
                        },
                        _ if root_images.is_some() => match bag_of(&word) {
                            Some(b) => {
                                let e = enclosing_ball(&b);
                                bag = Some(b);
                                e
                            }
                            None => None,
                        },
                        _ => disc_of(&word),
                    };
                    let Some(img) = region else {
                        // **Not a silent drop any more.** This word
                        // and its whole subtree leave the antichain,
                        // and `prob` is that subtree's measure.
                        lost += prob;
                        continue;
                    };
                    let centre = img.c;
                    let radius = img.r;
                    // Disjoint from the view: this word and every
                    // word extending it miss, because a child's image
                    // is a SUBSET of its parent's. Dropping it keeps
                    // the antichain complete -- the word is still a
                    // member of `A`, it simply contributes nothing to
                    // `V` and so is never sampled.
                    let hits = |c: [f64; 2], r: f64| {
                        ((c[0] - view.centre[0]).powi(2) + (c[1] - view.centre[1]).powi(2)).sqrt()
                            <= r + view.radius
                    };
                    let meets = match (&cover, &bag) {
                        (Some(c), _) => c.meets_disc(view.centre, view.radius),
                        // Same question as below, asked where the
                        // structure is: the sector's enclosing ball
                        // spans the hole it was cut to avoid, so
                        // testing through it would keep every child.
                        (None, Some(b)) => b.iter().any(|p| hits(p.c, p.r)),
                        _ => hits(centre, radius),
                    };
                    if !meets {
                        continue;
                    }
                    // Small enough: the image fits the view, so
                    // forcing this word puts a sample in the frame
                    // and subdividing further would only lengthen the
                    // prefix. This is where the antichain is cut.
                    if radius <= view.radius {
                        kept.push(Cylinder { word, prob, centre, radius, seeds: Vec::new() });
                        if kept.len() > MAX_WORDS {
                            return Err(NoCylinders::TooManyWords(kept.len()));
                        }
                    } else {
                        next.push(Node { word, prob, centre, radius, mob: child_mob });
                    }
                }
            }
            // **The frontier is a beam, and what falls off it is
            // counted.**
            //
            // With contractive maps the frontier narrows by itself:
            // the images are disjoint, a small view is inside one of
            // them, and everything else prunes at the first level.
            // A Möbius IFS is not like that. Measured on
            // `spherical.fflame`, centred on its own attractor, the
            // frontier went 4, 16, 63, 240, 887, 3113 — the branching
            // factor to the depth, with almost nothing pruned — while
            // the widest region GREW from 17 to 2.8e5.
            //
            // Both halves of that are real. Words contract on
            // AVERAGE, which is what the Lyapunov exponent says and
            // what a random weighted word does; the enumeration walks
            // ALL of them, and the expanding ones stay large, so they
            // keep meeting the view and keep branching forever.
            //
            // What saves it is that those words carry almost no
            // measure. Keeping the most probable `MAX_WORDS` and
            // charging the rest to `lost` is the measure decomposition
            // truncated where it stops mattering — and because it is
            // charged rather than discarded, the panel can say how
            // much of the picture went with it.
            //
            // The beam is much narrower for family M, and has to be:
            // every node there costs a cover push, which is hundreds
            // of microseconds, where an affine flame's node costs a
            // matrix multiply. A wide beam at full depth is
            // `beam × symbols × depth` pushes, and at 4096 that is
            // twenty minutes for one plan.
            //
            // **For the flames with no invariant ball.** Everywhere
            // else an overfull frontier is still `TooManyWords`,
            // because for a flame with a real invariant ball it means
            // the view straddles more pieces than the antichain can
            // hold, and answering that with a beam would turn a clear
            // refusal into a picture quietly missing most of itself.
            // A flame rooted in a cover has no such ball and no such
            // alternative — measured, `grand-julian`'s frontier
            // reaches 25981 without it, which is the branching factor
            // to the depth and the same symptom `spherical.fflame`
            // showed above.
            let Some(beam) = (mobius.is_some() || root_images.is_some()).then(|| {
                if root_images.is_some() {
                    ROOT_BEAM
                } else {
                    crate::scene::mobius::BEAM
                }
            }) else {
                if next.len() >= MAX_WORDS {
                    return Err(NoCylinders::TooManyWords(next.len()));
                }
                frontier = next;
                continue;
            };
            if next.len() > beam {
                next.sort_by(|a, b| {
                    b.prob.partial_cmp(&a.prob).unwrap_or(std::cmp::Ordering::Equal)
                });
                for node in next.drain(beam..) {
                    lost += node.prob;
                }
            }
            frontier = next;
        }
        // Anything still on the frontier hit the depth cap. Keeping
        // it is sound: its image is larger than the view, so its
        // samples spread wider than they need to, which costs
        // efficiency and not correctness.
        for node in frontier {
            kept.push(Cylinder {
                word: node.word,
                prob: node.prob,
                centre: node.centre,
                radius: node.radius,
                seeds: Vec::new(),
            });
        }

        if kept.is_empty() {
            return Err(NoCylinders::ViewIsEmpty);
        }
        let mass: f64 = kept.iter().map(|c| c.prob).sum();
        let depth = kept.iter().map(|c| c.word.len()).max().unwrap_or(0);
        Ok(Self {
            words: kept,
            mass,
            lost,
            sampling_leak,
            efficiency: 1.0,
            depth,
            composable,
            view_centre: view.centre,
            refs: Vec::new(),
            offset_rows: Vec::new(),
        })
    }
}

/// The disc every map sends into itself.
///
/// For an affine flame this is `analyse_2d`'s own answer, which is
/// exact and tight. For anything else there is no closed form, so
/// walk an origin-centred ladder and take the first disc the bounds
/// say is invariant: if every `T_i(B)` is inside `B`, then `B`
/// contains the attractor, which is all the enumeration needs of it.
///
/// **Origin-centred is a real limitation**, not a simplification. A
/// flame whose attractor sits far from the origin needs a disc large
/// enough to reach it, and a large root is a loose root. The honest
/// fix is to search the centre too; the reason it is not done here is
/// that the obvious way to find a good centre — run the chaos game
/// and look — needs a CPU evaluation of the variations, and there
/// isn't one: the bodies are WGSL.
/// The ball holding a bag of balls, or `None` if the bag is empty or
/// any of it is not finite.
fn enclosing_ball(bag: &[Ball]) -> Option<Ball> {
    let mut it = bag.iter();
    let first = *it.next()?;
    if !first.r.is_finite() || !first.c[0].is_finite() || !first.c[1].is_finite() {
        return None;
    }
    let mut out = crate::scene::mobius::Disc::new(first.c, first.r);
    for b in it {
        if !b.r.is_finite() || !b.c[0].is_finite() || !b.c[1].is_finite() {
            return None;
        }
        out = crate::scene::mobius::Cover::union_disc(
            &out,
            &crate::scene::mobius::Disc::new(b.c, b.r),
        );
    }
    Some(Ball::new(out.c, out.r))
}

/// Cut a pushed cover into as few balls as possible, each of which
/// every symbol can push.
///
/// **By recursive bisection, because nothing here knows where the map
/// blows up.** That is the premise of the whole pole-free path: a
/// Möbius map can name its pole, a bound cannot, and the only
/// available question is whether a given ball pushes. So the cut is
/// geometric and blind — split the discs at the median of their
/// widest axis, and recurse on each half until it pushes. Sorting by
/// angle was tried first and is worse: the shape being cut is an
/// annular sector, its centroid is not the hole's centre, and wedges
/// taken about the centroid still span the hole.
///
/// One ball is tried first, so a symbol that never needed cutting
/// pays one push per symbol and the common case is unchanged.
fn walkable_pieces(
    discs: &[crate::scene::mobius::Disc],
    max_pieces: usize,
    walkable: impl Fn(&Ball) -> bool,
) -> Option<Vec<Ball>> {
    fn recurse(
        discs: &[crate::scene::mobius::Disc],
        budget: &mut usize,
        walkable: &impl Fn(&Ball) -> bool,
        out: &mut Vec<Ball>,
    ) -> bool {
        let Some(whole) = enclosing_ball(
            &discs.iter().map(|d| Ball::new(d.c, d.r)).collect::<Vec<_>>(),
        ) else {
            return false;
        };
        if walkable(&whole) {
            if *budget == 0 {
                return false;
            }
            *budget -= 1;
            out.push(whole);
            return true;
        }
        // A single disc that cannot be pushed is one the cover should
        // never have placed, and there is nothing left to cut.
        if discs.len() < 2 {
            return false;
        }
        let xs = (
            discs.iter().map(|d| d.c[0]).fold(f64::INFINITY, f64::min),
            discs.iter().map(|d| d.c[0]).fold(f64::NEG_INFINITY, f64::max),
        );
        let ys = (
            discs.iter().map(|d| d.c[1]).fold(f64::INFINITY, f64::min),
            discs.iter().map(|d| d.c[1]).fold(f64::NEG_INFINITY, f64::max),
        );
        let axis = usize::from(ys.1 - ys.0 > xs.1 - xs.0);
        let mut sorted: Vec<crate::scene::mobius::Disc> = discs.to_vec();
        sorted.sort_by(|a, b| {
            a.c[axis].partial_cmp(&b.c[axis]).unwrap_or(std::cmp::Ordering::Equal)
        });
        let (lo, hi) = sorted.split_at(sorted.len() / 2);
        // The two halves overlap nowhere, so between them they hold
        // every disc and the union of the pieces still covers.
        recurse(lo, budget, walkable, out) && recurse(hi, budget, walkable, out)
    }

    if discs.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let mut budget = max_pieces;
    recurse(discs, &mut budget, &walkable, &mut out).then_some(out)
}

/// **A root for a flame that has no invariant disc**, built from a
/// cover and then collapsed back to one ball per symbol.
///
/// The enumeration needs to start from a region containing the
/// attractor and push it through words. When no disc qualifies, a
/// cover does — but a cover is expensive to push (measured at 5 ms a
/// step for `grand-julian`'s twenty-five arms) and `julian` is not a
/// group, so unlike family M there is no folding a word into one map.
/// A cover at every depth would cost `discs × depth` bound
/// evaluations per candidate, which is the wall family M hit.
///
/// **The cover is only needed once.** It exists to get past the pole,
/// and once a single symbol has been applied the image is a small
/// ball again — measured at radius 0.119 for `grand-julian`, against
/// an attractor extent of 22.7. So: build the cover, push it once per
/// symbol, collapse each image to its enclosing ball, and hand those
/// to the ordinary Ball walk, which costs what an affine flame's
/// does. Collapsing only GROWS a region, so containment survives it;
/// the check that each collapsed ball can still be pushed by every
/// symbol is what makes it safe to walk from.
///
/// Returns the cover's own enclosing disc (the root node's region,
/// used only for pruning the empty word), the image ball per symbol,
/// and the fraction of the sampled orbit the cover failed to hold.
fn bounded_root_images(
    bounders: &[crate::scene::ifs_ball::Bounder],
    weights: &[f64],
    alphabet: &[(u32, f64)],
) -> Result<(Ball, std::collections::HashMap<u32, Vec<Ball>>, f64), String> {
    use crate::scene::mobius::{cover_by_pushing, CoverRules, Disc};

    let total: f64 = weights.iter().sum();
    if !(total > 0.0) {
        return Err("no weight".into());
    }

    // Where the attractor is, found by running the chaos game through
    // the BOUNDS as points: a zero-radius ball's image centre is the
    // image of the point, so the same machinery that bounds discs can
    // also just iterate. Deterministic, because a plan that changed
    // shape between two identical frames would be untestable.
    let mut st = 0x9E37_79B9_7F4A_7C15u64;
    let mut lcg = move || {
        st = st
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((st >> 33) as f64) / ((1u64 << 31) as f64)
    };
    let mut p = [0.31f64, 0.17];
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(ROOT_COVER_SAMPLE);
    for k in 0..ROOT_COVER_SAMPLE {
        let mut u = lcg() * total;
        let mut j = weights.len() - 1;
        for (i, w) in weights.iter().enumerate() {
            if u < *w {
                j = i;
                break;
            }
            u -= *w;
        }
        let arms = bounders[j].arms().max(1);
        let a = ((lcg() * arms as f64) as u32).min(arms - 1);
        match bounders[j].apply_arm(Ball::new(p, 0.0), a) {
            Ok(b) if b.c[0].is_finite() && b.c[1].is_finite() => p = b.c,
            _ => {
                // The orbit walked into a pole. Reseed rather than
                // give up: that is what the kernel's own bad-value
                // recovery does, and the point of this walk is to
                // find where the attractor LIVES.
                p = [0.31, 0.17];
                continue;
            }
        }
        if k > ROOT_COVER_SAMPLE / 100 {
            pts.push(p);
        }
    }
    if pts.len() < 1000 {
        return Err(format!("the orbit did not settle ({} points)", pts.len()));
    }

    // How far the sample reaches, measured here rather than read back
    // out of the cover, because the size cap below is what the cover
    // has to be built AGAINST.
    let n = pts.len() as f64;
    let centre = [
        pts.iter().map(|p| p[0]).sum::<f64>() / n,
        pts.iter().map(|p| p[1]).sum::<f64>() / n,
    ];
    let extent = pts
        .iter()
        .map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2)).sqrt())
        .fold(0.0f64, f64::max);
    if !(extent > 0.0) || !extent.is_finite() {
        return Err("the orbit has no extent".into());
    }

    // **A disc is usable when its images can themselves be pushed.**
    //
    // Finiteness is not enough and neither is size: measured on
    // `grand-julian`, an image disc well inside a generous size cap
    // still contained the pole, and once that has happened the
    // enumeration cannot take a single further step from it. So the
    // test is one level deeper — push the disc, then check the image
    // can be pushed in turn.
    //
    // `clears` asks one arm per transform rather than all twenty-five
    // symbols, because every arm of one `julian` shares its pole and
    // the answers agree; that makes this |T| evaluations instead of
    // |A| and the difference is between a hundred milliseconds and a
    // second. It is a CONSTRUCTION heuristic — the pieces handed to
    // the walk are checked against the whole alphabet below, and a
    // flame is refused if that check fails.
    let max_image_r = ROOT_IMAGE_CAP * extent;
    let first_arm: Vec<u32> = {
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for &(sym, _) in alphabet {
            let t = sym_transform(sym);
            if !seen.contains(&t) {
                seen.push(t);
                out.push(sym);
            }
        }
        out
    };
    let clears = |c: [f64; 2], r: f64| -> bool {
        first_arm.iter().all(|&sym| {
            matches!(
                bounders[sym_transform(sym) as usize]
                    .apply_arm(Ball::new(c, r), sym_arm(sym)),
                Ok(b) if b.r.is_finite() && b.c[0].is_finite()
            )
        })
    };
    let accepts = |d: &Disc| -> bool { d.r <= max_image_r && clears(d.c, d.r) };
    let pushes = |d: &Disc| -> bool {
        alphabet.iter().all(|&(sym, _)| {
            matches!(
                bounders[sym_transform(sym) as usize]
                    .apply_arm(Ball::new(d.c, d.r), sym_arm(sym)),
                Ok(b) if accepts(&Disc::new(b.c, b.r))
            )
        })
    };
    let (cover, cover_extent, leak) = cover_by_pushing(
        &pts,
        ROOT_COVER_DISCS,
        ROOT_COVER_START,
        ROOT_COVER_ANCHORS,
        pushes,
    )
    .ok_or_else(|| "no cover could be built".to_string())?;
    debug_assert!((cover_extent - extent).abs() <= 1e-9 * extent.max(1.0));
    if !(leak <= ROOT_COVER_LEAK) {
        return Err(format!("the cover leaks {leak:.2e} of the orbit"));
    }
    let enclosing = cover
        .enclosing()
        .ok_or_else(|| "the cover has no enclosing disc".to_string())?;

    // One push per symbol, then collapse.
    let rules = CoverRules::by_pushing(ROOT_COVER_DISCS, 4.0, max_image_r);
    let mut images = std::collections::HashMap::with_capacity(alphabet.len());
    for &(sym, _) in alphabet {
        let b = &bounders[sym_transform(sym) as usize];
        let arm = sym_arm(sym);
        let pushed = cover
            .push_by(
                |q| {
                    b.apply_arm(Ball::new(q, 0.0), arm)
                        .ok()
                        .map(|r| r.c)
                        .filter(|c| c[0].is_finite() && c[1].is_finite())
                },
                |d| {
                    b.apply_arm(Ball::new(d.c, d.r), arm)
                        .ok()
                        .filter(|r| r.r.is_finite())
                        .map(|r| Disc::new(r.c, r.r))
                },
                &accepts,
                &rules,
            )
            .map_err(|e| format!("symbol {sym} could not push the cover: {e:?}"))?;
        // Walkable means every symbol can push it to something
        // finite, which is all the ordinary Ball walk needs. Size is
        // not asked here — the contraction probe below is the test
        // for that, and it asks about words rather than one step.
        let walkable = |b: &Ball| -> bool {
            b.r.is_finite()
                && b.c[0].is_finite()
                && b.c[1].is_finite()
                && alphabet.iter().all(|&(s2, _)| {
                    matches!(
                        bounders[sym_transform(s2) as usize].apply_arm(*b, sym_arm(s2)),
                        Ok(r) if r.r.is_finite() && r.c[0].is_finite()
                    )
                })
        };
        let pieces = walkable_pieces(&pushed.discs, ROOT_PIECES, walkable).ok_or_else(|| {
            format!(
                "symbol {sym}'s image cannot be cut into {ROOT_PIECES} pieces that clear the pole"
            )
        })?;
        images.insert(sym, pieces);
    }

    // **Do words shrink from here?** The root images escape the pole;
    // whether the enumeration terminates is a separate question, and
    // it is the one that separates `grand-julian` from `spherical`.
    let mut contracted = 0usize;
    let mut worst = 0.0f64;
    for _ in 0..ROOT_PROBE_WORDS {
        let first = alphabet[((lcg() * alphabet.len() as f64) as usize).min(alphabet.len() - 1)].0;
        let mut bag = images[&first].clone();
        let mut alive = true;
        for _ in 1..ROOT_PROBE_DEPTH {
            let sym =
                alphabet[((lcg() * alphabet.len() as f64) as usize).min(alphabet.len() - 1)].0;
            let b = &bounders[sym_transform(sym) as usize];
            let arm = sym_arm(sym);
            let mut next = Vec::with_capacity(bag.len());
            for piece in &bag {
                match b.apply_arm(*piece, arm) {
                    Ok(img) if img.r.is_finite() && img.c[0].is_finite() => next.push(img),
                    // A word that cannot be followed is one the
                    // enumeration prunes, not one that fails to
                    // contract.
                    _ => {
                        alive = false;
                        break;
                    }
                }
            }
            if !alive {
                break;
            }
            bag = next;
        }
        if alive {
            let r = enclosing_ball(&bag).map_or(f64::INFINITY, |b| b.r);
            worst = worst.max(r / extent);
            if r < ROOT_PROBE_SHRINK * extent {
                contracted += 1;
            }
        }
    }
    if (contracted as f64) < ROOT_PROBE_PASS * ROOT_PROBE_WORDS as f64 {
        return Err(format!(
            "only {contracted} of {ROOT_PROBE_WORDS} probe words contracted \
             (worst ended at {worst:.2e} of the extent), so the walk would not terminate"
        ));
    }

    Ok((Ball::new(enclosing.c, enclosing.r), images, leak))
}

fn invariant_ball(
    flame: &Flame,
    registry: &crate::variations::VariationRegistry,
) -> Result<([f64; 2], f64), NoCylinders> {
    let bounders: Vec<crate::scene::ifs_ball::Bounder> = flame
        .transforms
        .iter()
        .filter(|t| t.weight > 0.0)
        .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, registry).ok())
        .collect();
    if bounders.is_empty() {
        return Err(NoCylinders::NoInvariantBall);
    }

    // How far the images of `disc(c, r)` reach from `c`. The disc is
    // invariant exactly when this is `<= r`.
    //
    // Asked through the BOUNDS even of `analyse_2d`'s own answer,
    // which is the subtlety: that analysis understands the kernel
    // variations through their `InverseDef`s, so it happily returns a
    // ball for a flame whose forward bound is much cruder --
    // `spherical` near the origin, say. Its ball is then correct for
    // the walks and NOT invariant under what the enumeration will
    // actually push with, and taking it on trust produced a root that
    // the very next contraction check rejected.
    let reach = |c: [f64; 2], r: f64| -> Option<f64> {
        let mut need = 0.0f64;
        for b in &bounders {
            let img = b.apply(Ball::new(c, r)).ok()?;
            let d = ((img.c[0] - c[0]).powi(2) + (img.c[1] - c[1]).powi(2)).sqrt();
            need = need.max(d + img.r);
        }
        need.is_finite().then_some(need)
    };

    if let Ok(ifs) = crate::scene::ifs_analysis::analyse_2d(flame, registry) {
        if reach(ifs.ball.centre, ifs.ball.radius).is_some_and(|n| n <= ifs.ball.radius) {
            return Ok((ifs.ball.centre, ifs.ball.radius));
        }
    }

    // **A point on the attractor, to centre the search on.**
    //
    // The old fallback only ever tried discs around the ORIGIN,
    // doubling the radius. For a flame whose attractor sits away from
    // the origin that asks the wrong question: an origin-centred disc
    // must be large enough to span the gap as well as the set, and a
    // disc that large is far less likely to be invariant -- a
    // nonlinear body that is gentle on the attractor can be wild out
    // at the origin. Flames that were perfectly well behaved were
    // refused with "no disc around the origin contains the
    // attractor", which was true and beside the point.
    //
    // There is no CPU evaluator for a variation, but there is a
    // bound, and a bound applied to a degenerate disc is the image
    // point plus a little slop. So the chaos game runs through the
    // bounds themselves, round-robin rather than at random, and lands
    // wherever the maps are pulling.
    // **Several starts, because the origin is exactly where the
    // inversive family is undefined.** `spherical` is `p/|p|²` and
    // `julian` with a negative `dist` is `|p|^(-1/2)`; both are
    // unbounded at zero, so a walk that begins there dies on its
    // first step and the flame is refused for want of a starting
    // point rather than for anything about its attractor.
    let mut seeds: Vec<[f64; 2]> = Vec::new();
    for start in [[0.0f64, 0.0], [1.0, 0.0], [0.6, -0.8], [-0.35, 0.42], [2.5, 1.5]] {
        let mut p = start;
        let mut ok = true;
        for k in 0..64 {
            match bounders[k % bounders.len()].apply(Ball::new(p, 0.0)) {
                Ok(img) if img.c[0].is_finite() && img.c[1].is_finite() => p = img.c,
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            seeds.push(p);
        }
    }
    seeds.push([0.0, 0.0]);

    // Grow a disc at that centre until it holds its own images.
    //
    // `reach` is increasing in `r`, and for contractive maps it is
    // roughly `D + s·r`, so iterating `r <- reach(r)` walks up to the
    // least fixed point `D/(1−s)` geometrically. Extrapolating that
    // geometry gets there in a handful of steps instead of hundreds
    // when `s` is close to 1 -- and every candidate is CHECKED before
    // it is returned, so a bad extrapolation costs an iteration
    // rather than correctness.
    for centre in seeds {
        let mut r = 0.0f64;
        let mut prev_step = f64::INFINITY;
        for _ in 0..64 {
            let Some(need) = reach(centre, r) else { break };
            if need <= r {
                return Ok((centre, r));
            }
            let step = need - r;
            // The contraction factor this step implies. Below 1 the
            // series converges and its tail sums in closed form.
            let ratio = step / prev_step;
            let guess = if (0.0..1.0).contains(&ratio) {
                need + step * ratio / (1.0 - ratio)
            } else {
                need
            };
            prev_step = step;
            // A little slack, so the fixed point is crossed rather
            // than approached forever.
            let next = (guess * 1.0009375).max(need).max(1e-9);
            if !next.is_finite() || next > 1e12 {
                break;
            }
            r = next;
        }
    }
    Err(NoCylinders::NoInvariantBall)
}

/// How many floats one packed word occupies: the composed affine
/// (six), the colour fold's two coefficients, the cumulative
/// probability, and three to round the stride to a `vec4` multiple.
pub const WORD_FLOATS: usize = 12;

/// Pack the enumeration for the kernel.
///
/// **A word becomes ONE affine, not a sequence of symbols.** Every
/// map is affine here, so `S_a = S_{a_k} ∘ … ∘ S_{a_1}` composes to a
/// single 2×2 and a translation on the CPU — so forcing a prefix of
/// eighteen transforms costs the kernel one matrix multiply, not
/// eighteen. That is the deep-zoom plan's stage 3 note arriving
/// early: "compose the camera zoom with the (contracting) forced
/// prefix at f64 on the CPU into one well-conditioned map".
///
/// The COLOUR folds the same way. flam3's rule is
/// `c ← c·h + g` per transform with `h = (1+s)/2` and
/// `g = colour·(1−s)/2`, an affine map of `c`, so a whole word is
/// `c ← c·H + G` with
///
/// ```text
/// H = ∏ h_j          G = Σ_i g_i · ∏_{j>i} h_j
/// ```
///
/// the inner product running over the maps applied AFTER `i`, since
/// those are the ones that still act on `g_i`. Two numbers per word,
/// and the plotted colour is exact rather than approximated.
///
/// Layout, `WORD_FLOATS` per word:
///
/// ```text
/// 0..4   m00, m01, m10, m11      the composed 2×2
/// 4..6   t0, t1                  its translation
/// 6..8   H, G                    the colour fold
/// 8      cdf                     cumulative p_a / mass, ascending
/// 9..12  spare (stride)
/// ```
///
/// The CDF is cumulative and its last entry is exactly 1, so the
/// kernel's search cannot fall off the end however the floats
/// rounded.
pub fn pack(cyl: &Cylinders, flame: &Flame, registry: &crate::variations::VariationRegistry) -> Vec<f32> {
    let maps: Vec<Affine2> = flame
        .transforms
        .iter()
        .map(|t| {
            crate::scene::ifs_analysis::transform_affine_2d(t, registry)
                .unwrap_or(Affine2 { m: [[1.0, 0.0], [0.0, 1.0]], t: [0.0, 0.0] })
        })
        .collect();

    let mut out = vec![0.0f32; cyl.words.len() * WORD_FLOATS];
    let mut acc = 0.0f64;
    for (w, c) in cyl.words.iter().enumerate() {
        // Compose oldest-first: the word is stored in the order the
        // chaos game applies it, so each new map goes on the OUTSIDE.
        let mut m = Affine2 { m: [[1.0, 0.0], [0.0, 1.0]], t: [0.0, 0.0] };
        let mut h_prod = 1.0f64;
        let mut g_acc = 0.0f64;
        for &sym in &c.word {
            let i = sym_transform(sym) as usize;
            m = maps[i].then_after(&m);
            let t = &flame.transforms[i];
            let s = t.color_speed as f64;
            let h = (1.0 + s) * 0.5;
            let g = t.color as f64 * (1.0 - s) * 0.5;
            // `g_acc` is the sum so far; this map acts on all of it,
            // then adds its own term.
            g_acc = g_acc * h + g;
            h_prod *= h;
        }
        acc += c.prob / cyl.mass.max(f64::MIN_POSITIVE);
        let base = w * WORD_FLOATS;
        out[base] = m.m[0][0] as f32;
        out[base + 1] = m.m[0][1] as f32;
        out[base + 2] = m.m[1][0] as f32;
        out[base + 3] = m.m[1][1] as f32;
        // **Relative to the view centre, and subtracted in f64.**
        //
        // `t` is where the word sends the origin, which at depth is
        // the view centre to within the word's own tiny radius. The
        // kernel then computes `M·x + t` in f32, where `M·x` is the
        // whole picture -- 1e-5 and smaller -- and `t` is order 0.3.
        // One f32 ulp at 0.3 is 3e-8, so past about 1e5 zoom the add
        // quantises the picture onto a lattice and the fractal
        // structure is simply gone. Measured on a two-map fern: clean
        // at 86k, visibly striped at 692k, nothing but a diagonal dot
        // grid at 5.5M.
        //
        // Subtracting the centre HERE, in f64, makes every number the
        // kernel touches small, so f32's relative precision applies to
        // the detail instead of to the distance from the origin. The
        // plot then works in view-relative coordinates, which is why
        // `world_to_pixel` must skip its own pan subtraction under
        // `CYLINDER_RELATIVE` -- the two changes are one change.
        out[base + 4] = (m.t[0] - cyl.view_centre[0]) as f32;
        out[base + 5] = (m.t[1] - cyl.view_centre[1]) as f32;
        out[base + 6] = h_prod as f32;
        out[base + 7] = g_acc as f32;
        out[base + 8] = acc as f32;
    }
    // The last entry is exactly one, whatever the sum rounded to, so
    // a uniform draw always finds a word.
    if let Some(last) = cyl.words.len().checked_sub(1) {
        out[last * WORD_FLOATS + 8] = 1.0;
    }
    out
}

/// Pack the enumeration for a kernel that must REPLAY it.
///
/// Used when [`Cylinders::composable`] is false: some map in the
/// flame is only bounded, not affine, so there is no single matrix
/// for the whole word and the kernel walks the symbols instead —
/// running each transform exactly as the chaos game would.
///
/// Layout: an eight-float header `[stride, count, rows, blocks, shift x,
/// shift y, 0, 0]`, then one word per stride as
/// `[cdf, H, G, len, sym0, sym1, …]`.
///
/// **The replay in offsets** (`docs/projects/deep-zoom-precision.md`),
/// when the plan carries references: `rows` is where the flame's forward
/// maps start (`forward_delta::ROW_FLOATS` each, by transform index),
/// and `blocks` where one offset per word starts, then the words' blocks
/// -- `[m, chains, per chain: z_m … z_{n-1}, z_n − c]`, offset 0 for a
/// word with none. `shift` is the plan's centre less the view's, in f64,
/// which the renderer writes every frame so a plan still drawing after a
/// pan stays where it belongs. Zero rows and blocks: no offsets. The colour still
/// folds to the two coefficients `H` and `G`, because flam3's rule is
/// affine in the colour coordinate whatever the POSITION map does —
/// so the colour is exact here for the same reason it is in
/// [`pack`], and only the position costs a walk.
///
/// The stride is uniform and set by the deepest word. That wastes a
/// few floats on the shallow ones and buys the kernel a multiply
/// instead of an indirection; at the caps (4096 words, depth 96) the
/// whole table is under 1.6 MB.
pub fn pack_words(cyl: &Cylinders, flame: &Flame) -> Vec<f32> {
    let longest = cyl.words.iter().map(|c| c.word.len()).max().unwrap_or(0);
    // Round the stride to a vec4 multiple, as `pack` does, so a word
    // never straddles awkwardly.
    let stride = ((4 + longest) + 3) / 4 * 4;
    let mut out = vec![0.0f32; HEADER_FLOATS + cyl.words.len() * stride];
    out[0] = stride as f32;
    out[1] = cyl.words.len() as f32;

    let mut acc = 0.0f64;
    for (w, c) in cyl.words.iter().enumerate() {
        let mut h_prod = 1.0f64;
        let mut g_acc = 0.0f64;
        for &sym in &c.word {
            let t = &flame.transforms[sym_transform(sym) as usize];
            let s = t.color_speed as f64;
            let h = (1.0 + s) * 0.5;
            let g = t.color as f64 * (1.0 - s) * 0.5;
            g_acc = g_acc * h + g;
            h_prod *= h;
        }
        acc += c.prob / cyl.mass.max(f64::MIN_POSITIVE);
        let base = HEADER_FLOATS + w * stride;
        out[base] = acc as f32;
        out[base + 1] = h_prod as f32;
        out[base + 2] = g_acc as f32;
        out[base + 3] = c.word.len() as f32;
        for (k, &sym) in c.word.iter().enumerate() {
            out[base + 4 + k] = sym as f32;
        }
    }
    // The last entry is exactly one, whatever the sum rounded to, so
    // a uniform draw always finds a word.
    if let Some(last) = cyl.words.len().checked_sub(1) {
        out[HEADER_FLOATS + last * stride] = 1.0;
    }
    if cyl.refs.len() == cyl.words.len() && !cyl.offset_rows.is_empty() {
        out[2] = out.len() as f32;
        out.extend_from_slice(&cyl.offset_rows);
        let offsets_at = out.len();
        out[3] = offsets_at as f32;
        out.resize(offsets_at + cyl.words.len(), 0.0);
        for (w, r) in cyl.refs.iter().enumerate() {
            let Some(r) = r else { continue };
            out[offsets_at + w] = out.len() as f32;
            out.push(r.m as f32);
            out.push(r.chains.len() as f32);
            for ch in &r.chains {
                for z in &ch.bases {
                    out.push(z[0] as f32);
                    out.push(z[1] as f32);
                }
                out.push(ch.end[0] as f32);
                out.push(ch.end[1] as f32);
            }
        }
        // f32 offsets are exact below 2^24 floats, 64 MB of table.
        debug_assert!(out.len() < 1 << 24);
    }
    out
}

/// Floats before the first word of a replay table: see [`pack_words`].
pub const HEADER_FLOATS: usize = 8;

/// The gates that need a GPU: a targeted render is the untargeted
/// render, and it gets there with far fewer wasted samples.
#[cfg(test)]
mod gpu_tests {
    use super::*;
    use crate::config::FractalConfig;
    use crate::scene::transforms::Transform;

    fn device() -> (wgpu::Device, wgpu::Queue) {
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
        let al = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
        limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
        limits.max_buffer_size = al.max_buffer_size;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cylinder test"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            ..Default::default()
        }))
        .expect("device")
    }

    fn gasket_config() -> FractalConfig {
        let mut cfg = FractalConfig::default();
        cfg.flame.transforms.clear();
        for (i, (e, f)) in [(0.0f32, 0.0f32), (0.5, 0.0), (0.25, 0.5)].into_iter().enumerate() {
            let mut t = Transform::default();
            t.a = 0.5;
            t.d = 0.5;
            t.e = e;
            t.f = f;
            t.weight = 1.0;
            // NOT `i / 2`, which gives transform 0 the colour 0.0.
            // flam3's colour rule drives the coordinate toward the
            // colour of whatever transform ran last, so a word of many
            // consecutive S0s -- which is exactly what a deep zoom
            // toward S0's fixed point selects for -- lands at 0.0, the
            // BLACK end of the palette. The render then has full
            // density everywhere and is invisible anyway, which reads
            // exactly like starvation and is not (see
            // `auto_exposure_makes_a_deep_view_visible`).
            t.color = 0.5 + i as f32 / 4.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            cfg.flame.transforms.push(t);
        }
        cfg.deterministic_rng = true;
        // **Levels OFF for the picture gates.** These compare a forced
        // render against an unbiased one, and Levels is an alpha remap
        // relative to the mean density of the pixels IN FRAME -- a
        // quantity the two renders legitimately disagree about, since
        // the forced one puts everything in view and the reference at
        // depth puts almost none of it there. Leaving it on makes the
        // brightness comparison depend on that second mechanism
        // instead of on the deposit's weight, which is what is
        // actually being tested.
        cfg.levels_enabled = false;
        // The origin is S0's fixed point, so the view is on the set at
        // every scale.
        cfg.pan_x = 0.0;
        cfg.pan_y = 0.0;
        cfg
    }

    fn render_out(cfg: &FractalConfig, n: u32, iters: u64) -> crate::renderer::RenderOutput {
        let (device, queue) = device();
        let job = crate::renderer::RenderJob::new(cfg, n, n).with_iterations(iters);
        pollster::block_on(crate::renderer::render(
            &device,
            &queue,
            job,
            &mut crate::renderer::NoProgress,
        ))
        .expect("render")
    }

    fn render(cfg: &FractalConfig, n: u32, iters: u64) -> Vec<u8> {
        render_out(cfg, n, iters).rgba_data
    }

    /// What share of the gasket's chaos game a zoomed view holds, on
    /// the CPU: the number the GPU counters are supposed to agree
    /// with. Same three maps, same equal weights, a square frame of
    /// half-extent `2/zoom` about the origin.
    fn cpu_coverage(zoom: f64, pan: f64, n: usize) -> f64 {
        let maps = [[0.0f64, 0.0], [0.5, 0.0], [0.25, 0.5]];
        let mut st = 0x2545_F491_4F6C_DD1Du64;
        let mut rnd = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (st >> 11) as f64 / (1u64 << 53) as f64
        };
        let half = 2.0 / zoom;
        let (mut p, mut hits) = ([0.0f64, 0.0], 0usize);
        for i in 0..n + 200 {
            let m = maps[((rnd() * 3.0) as usize).min(2)];
            p = [0.5 * p[0] + m[0], 0.5 * p[1] + m[1]];
            if i >= 200 && (p[0] - pan).abs() <= half && (p[1] - pan).abs() <= half {
                hits += 1;
            }
        }
        hits as f64 / n as f64
    }

    /// Auto exposure makes a deep view visible, and the number it uses
    /// is the real one.
    ///
    /// The shipped tone map normalises by `total_iters / pixel_count`,
    /// which assumes the frame holds all the work. A zoomed view does
    /// not: most of the attractor is off-screen, so the samples that
    /// DID land get divided by a count dominated by ones that did not,
    /// and the picture fades as the zoom deepens.
    ///
    /// Two things are asserted, because the feature is worth nothing
    /// unless both hold:
    ///
    /// - the measured coverage matches an independent CPU chaos game,
    ///   so the counters mean what they claim;
    /// - a view that has gone dark without it is exposed with it.
    ///
    /// Measured on a gasket at a generic point of the set, 96x96,
    /// 64M iterations:
    ///
    /// ```text
    ///   zoom   coverage gpu / cpu      max off / on   mean off / on
    ///   2^2     7.22e-1 /  7.22e-1        234 / 252   0.0499 / 0.0539
    ///   2^4     1.12e-1 /  1.11e-1        143 / 235   0.0322 / 0.0539
    ///   2^6     8.28e-3 /  8.25e-3        101 / 255   0.0132 / 0.0394
    ///   2^8     9.02e-4 /  9.03e-4         60 / 255   0.0080 / 0.0393
    ///   2^10    1.04e-4 /  1.03e-4         37 / 255   0.0047 / 0.0374
    ///   2^12    8.28e-6 /  8.22e-6         28 / 255   0.0015 / 0.0176
    /// ```
    ///
    /// **A GENERIC point of the set, deliberately.** Aimed at `S₀`'s
    /// fixed point instead, the deep view is all-black for a reason
    /// that has nothing to do with exposure: the cylinder a corner
    /// zoom selects is the all-`S₀` word, flam3's colour rule walks
    /// the colour coordinate to transform 0's colour, and at the
    /// fixture's original `0.0` that is the black end of the palette.
    /// Full density, no light. That is what
    /// `gasket_config`'s colours are now chosen to avoid, and it is
    /// worth knowing because it mimics starvation exactly -- max
    /// channel zero, unmoved by any exposure -- while being a
    /// completely different fault.
    #[test]
    #[ignore = "needs a GPU"]
    fn auto_exposure_makes_a_deep_view_visible() {
        const N: u32 = 96;
        const ITERS: u64 = 64_000_000;
        // A generic point of the set rather than a fixed point of one
        // of the maps, so the view keeps sampling the whole attractor
        // as it descends.
        const PAN: f32 = 0.25;

        let stat = |rgba: &[u8]| -> (u8, f64) {
            let mx = rgba.chunks(4).map(|p| p[0].max(p[1]).max(p[2])).max().unwrap_or(0);
            let sum: u64 = rgba.chunks(4).map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum();
            (mx, sum as f64 / 765.0 / (N * N) as f64)
        };

        println!("  zoom   coverage gpu / cpu      max off / on   mean off / on");
        let mut rescued = 0;
        for zoom_pow in [2i32, 4, 6, 8, 10, 12] {
            let mut off = gasket_config();
            off.zoom = 2f32.powi(zoom_pow);
            off.pan_x = PAN as f64;
            off.pan_y = PAN as f64;
            // **Levels ON here, unlike the picture gates**, because
            // this is where the starvation actually shows. Measured
            // with Levels off, a deep view does NOT fade -- the log
            // mapping saturates and `max` stays at 255 whatever the
            // zoom. What fades is the ALPHA: Levels clips opacity
            // against a mean density computed as though every
            // iteration landed in frame, and at depth almost none do.
            // So auto exposure's visible effect is mostly through
            // Levels rather than through brightness, and a gate that
            // turned Levels off would be measuring a feature doing
            // nothing.
            off.levels_enabled = true;
            let mut on = off.clone();
            on.auto_exposure = true;

            let r_off = render_out(&off, N, ITERS);
            let r_on = render_out(&on, N, ITERS);
            let (mx_off, mean_off) = stat(&r_off.rgba_data);
            let (mx_on, mean_on) = stat(&r_on.rgba_data);
            let cpu = cpu_coverage(2f64.powi(zoom_pow), PAN as f64, 4_000_000);

            println!(
                "  2^{zoom_pow:<4} {:>9.2e} / {cpu:>9.2e}     {mx_off:>4} / {mx_on:<4}   {mean_off:.4} / {mean_on:.4}",
                r_on.frame_coverage
            );

            // Off must measure nothing: the counters are not compiled
            // in, so the fraction stays at its 1.0 identity.
            assert_eq!(
                r_off.frame_coverage, 1.0,
                "2^{zoom_pow}: a render with auto exposure off reported a coverage -- the \
                 counters are running when the flag says they are not"
            );
            assert!(
                r_on.frame_coverage <= 1.0,
                "2^{zoom_pow}: coverage {:.2e} exceeds 1 -- more attempts landed than were made",
                r_on.frame_coverage
            );

            // The measurement has to be the real one. Loose, because
            // the two use different RNGs and different burn-ins; this
            // is catching an off-by-a-factor, not a rounding
            // difference. Only where the CPU estimate itself has
            // enough hits to be worth comparing against.
            if cpu * 4_000_000.0 > 1000.0 {
                let ratio = r_on.frame_coverage as f64 / cpu;
                assert!(
                    (0.8..1.25).contains(&ratio),
                    "2^{zoom_pow}: GPU coverage {:.2e} against an independent CPU chaos game's \
                     {cpu:.2e} ({ratio:.2}x) -- the counters are not measuring what they claim",
                    r_on.frame_coverage
                );
            }

            // The rescue: a view the shipped normalisation has taken
            // well below full scale comes back to it.
            if mx_off < 96 && mx_on > 224 && mean_on > mean_off * 2.0 {
                rescued += 1;
            }
        }

        assert!(
            rescued >= 3,
            "auto exposure rescued {rescued} of the deep zooms -- either the views are no \
             longer being dimmed by the iteration-count normalisation, or the measured \
             coverage is not reaching the tone map"
        );
    }

    /// The picture matches at a GENERIC point too, not only at a
    /// fixed point.
    ///
    /// The render-level companion to
    /// `a_generic_point_of_the_attractor_enumerates`. Both existing
    /// picture gates aim at `S₀`'s fixed point, which is the one
    /// place the enumeration's pruning was accidentally correct, so
    /// neither of them could fail when it was wrong everywhere else.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_generic_point_renders_the_same_picture() {
        const N: u32 = 96;
        let mut p = [0.0f64, 0.0];
        {
            let f = gasket_config();
            for k in 0..60 {
                let t = &f.flame.transforms[[0usize, 1, 2, 1, 0, 2][k % 6]];
                p = [
                    t.a as f64 * p[0] + t.b as f64 * p[1] + t.e as f64,
                    t.c as f64 * p[0] + t.d as f64 * p[1] + t.f as f64,
                ];
            }
        }
        let lit_of = |rgba: &[u8]| -> Vec<bool> {
            rgba.chunks(4).map(|q| q[0] as u32 + q[1] as u32 + q[2] as u32 > 24).collect()
        };

        let reg = crate::variations::global_registry();
        println!("  zoom   speedup   lit ref / tgt   overlap");
        let mut checked = 0;
        for zoom_pow in [4i32, 6, 8] {
            let mut base = gasket_config();
            base.zoom = 2f32.powi(zoom_pow);
            base.pan_x = p[0];
            base.pan_y = p[1];
            let plan = Cylinders::plan(
                &base.flame,
                &reg,
                View::of(base.zoom as f64, [base.pan_x, base.pan_y], N, N),
            )
            .expect("a generic attractor point enumerates");

            let mut tgt = base.clone();
            tgt.cylinder_targeting = true;
            let lit_r = lit_of(&render(&base, N, 64_000_000));
            let lit_t = lit_of(&render(&tgt, N, 4_000_000));
            let nr = lit_r.iter().filter(|b| **b).count();
            let nt = lit_t.iter().filter(|b| **b).count();
            let both = lit_r.iter().zip(&lit_t).filter(|(a, b)| **a && **b).count();
            let overlap = both as f64 / nr.max(1) as f64;
            println!(
                "  2^{zoom_pow:<4} {:>7.1}   {nr:>4} / {nt:>4}   {:>6.1}%",
                plan.speedup(),
                overlap * 100.0
            );
            if plan.speedup() <= 1.0 {
                continue;
            }
            checked += 1;
            assert!(nr > 50 && nt > 50, "2^{zoom_pow}: {nr} / {nt} lit");
            assert!(
                overlap > 0.90,
                "2^{zoom_pow}: overlap {:.1}% at a generic point",
                overlap * 100.0
            );
        }
        assert!(checked > 0, "nothing was actually targeted");
    }

    /// A REPLAYED word is the untargeted render too — the same
    /// claim as the composed path, for the kernel that has to walk
    /// the symbols.
    ///
    /// The flame carries a bounded-but-not-affine map (`blur` on one
    /// transform), so `Cylinders::composable` is false, `pack_words`
    /// emits the symbol form, and the shader compiles the
    /// `CYLINDER_REPLAY` arm: a loop over the word running each
    /// transform through `apply_variations` exactly as the chaos game
    /// would.
    ///
    /// **This is the gate that makes the forward bounds worth
    /// building.** Everything before it — the per-variation contract,
    /// its GPU soundness check, the composition through a transform's
    /// phases, the enumeration — is machinery for producing a word
    /// list. If the kernel replays that list and the picture moves,
    /// none of it was worth anything.
    ///
    /// Measured on a gasket with `blur` at 0.02 on one transform:
    ///
    /// ```text
    ///   zoom   P(A_V)    speedup   lit ref / tgt   overlap   brightness
    /// ```
    #[test]
    #[ignore = "needs a GPU"]
    fn a_replayed_word_is_the_untargeted_render() {
        const N: u32 = 96;
        let stats = |rgba: &[u8]| -> (Vec<bool>, f64) {
            let lit: Vec<bool> = rgba
                .chunks(4)
                .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .collect();
            let (mut acc, mut n) = (0.0f64, 0.0f64);
            for (p, l) in rgba.chunks(4).zip(&lit) {
                if !*l {
                    continue;
                }
                acc += (p[0] as f64 + p[1] as f64 + p[2] as f64) / 765.0;
                n += 1.0;
            }
            (lit, if n > 0.0 { acc / n } else { 0.0 })
        };

        let reg = crate::variations::global_registry();
        println!("  zoom   P(A_V)    speedup   lit ref / tgt   overlap   brightness");
        let mut ran = 0;
        for zoom_pow in [2i32, 4, 6] {
            let mut base = gasket_config();
            // Bounded, not affine: this is what forces the replay.
            base.flame.transforms[1].set_variation("blur", 0.02);
            base.zoom = 2f32.powi(zoom_pow);

            let plan = Cylinders::plan(
                &base.flame,
                &reg,
                View::of(base.zoom as f64, [base.pan_x, base.pan_y], N, N),
            )
            .expect("a bounded flame enumerates");
            assert!(!plan.composable, "this fixture must exercise the REPLAY arm");

            let mut tgt = base.clone();
            tgt.cylinder_targeting = true;

            let (lit_r, bright_r) = stats(&render(&base, N, 64_000_000));
            let (lit_t, bright_t) = stats(&render(&tgt, N, 4_000_000));

            let nr = lit_r.iter().filter(|b| **b).count();
            let nt = lit_t.iter().filter(|b| **b).count();
            let both = lit_r.iter().zip(&lit_t).filter(|(a, b)| **a && **b).count();
            let overlap = both as f64 / nr.max(1) as f64;
            println!(
                "  2^{zoom_pow:<4} {:.2e}  {:>7.1}   {nr:>4} / {nt:>4}   {:>6.1}%   {bright_r:.3} / {bright_t:.3}",
                plan.mass,
                plan.speedup(),
                overlap * 100.0
            );

            if plan.speedup() <= 1.0 {
                // Below the pay line the renderer declines, so the
                // two renders differ only by iteration count.
                continue;
            }
            ran += 1;
            assert!(nr > 100 && nt > 100, "2^{zoom_pow}: {nr} / {nt} lit -- nothing to compare");
            assert!(
                overlap > 0.93,
                "2^{zoom_pow}: the replayed render covers only {:.1}% of the reference's lit \
                 pixels -- the kernel is walking the word wrongly, or walking the wrong word",
                overlap * 100.0
            );
            assert!(
                (bright_t / bright_r - 1.0).abs() < 0.25,
                "2^{zoom_pow}: the replayed render is {:.3}x the reference's brightness",
                bright_t / bright_r
            );
        }
        assert!(ran > 0, "no zoom in the sweep actually exercised the replay");
    }

    /// A deep zoom resolves STRUCTURE, not an f32 lattice.
    ///
    /// The forced prefix computes `M_a·x + t_a`, where `M_a·x` is the
    /// whole picture — 1e-5 and smaller — and `t_a` is where the word
    /// sends the origin, which at depth is the view centre. Away from
    /// the origin that centre is an ordinary number like 0.27, whose
    /// f32 ulp is 3e-8, so past about 1e5 zoom the add rounds the
    /// picture onto a lattice and the fractal structure is gone.
    /// Measured on this fixture before the fix: clean at 86k, visibly
    /// striped at 692k, a diagonal grid of isolated dots at 5.5M.
    ///
    /// `pack` subtracts the view centre in f64, so the kernel only
    /// ever touches small numbers and f32's RELATIVE precision applies
    /// to the detail instead of to the distance from the origin.
    ///
    /// **Lit-pixel counts cannot see this** — a lattice of dots has
    /// plenty of lit pixels — so what is measured is CONNECTEDNESS:
    /// the share of lit pixels with a lit neighbour. Real structure
    /// is connected; a quantisation lattice is isolated dots, and the
    /// two are miles apart on this number.
    ///
    /// The fixture is deliberately NOT the gasket at the origin, the
    /// fixture every other test here uses. f32 is finely spaced near
    /// zero, so the origin is the one place this bug cannot appear —
    /// the same blind spot that hid the enumeration's word-order bug.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_deep_zoom_resolves_structure_and_not_a_lattice() {
        const N: u32 = 300;
        // Two similitudes with an attractor well away from the origin
        // (a Barnsley-style fern), and the view on the fixed point of
        // the dominant map, which is where a deep zoom can actually
        // land.
        let mut cfg = FractalConfig::default();
        cfg.flame.transforms.clear();
        for (a, b, c, d, e, f) in [
            (0.4262196f32, 0.4407327, -0.4407333, 0.42621973, -0.20342615, 0.08565312),
            (0.7153461, -0.022982832, 0.022982799, 0.715347, 0.03961471, 0.07494647),
        ] {
            let mut t = crate::scene::transforms::Transform::default();
            t.a = a; t.b = b; t.c = c; t.d = d; t.e = e; t.f = f;
            t.weight = 1.0;
            // NOT the default 0.0: flam3's colour rule walks the
            // coordinate toward the last transform's colour, and 0.0
            // is the black end of the palette. A deep zoom selects
            // long runs of one symbol, so the whole frame would come
            // back black with a perfectly healthy histogram under it
            // -- the same trap `gasket_config` documents.
            t.color = 0.6;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            cfg.flame.transforms.push(t);
        }
        cfg.flame.transforms[1].color = 0.95;
        cfg.deterministic_rng = true;
        cfg.auto_exposure = true;
        cfg.cylinder_targeting = true;
        // **The centre is solved, not typed in.** It is the fixed
        // point of the dominant map -- the one place a deep zoom can
        // land -- and it has to be the fixed point of the map as
        // STORED, in f32, widened to f64. A centre computed from the
        // decimal coefficients instead sits 6.4e-9 away, which is
        // inside the viewport at 1e8 and outside it at 1e9, so the
        // enumeration finds nothing and the whole ladder reads as a
        // precision failure that is really a typo.
        let t = &cfg.flame.transforms[1];
        let (a, b, c, d) = (t.a as f64, t.b as f64, t.c as f64, t.d as f64);
        let (e, f) = (t.e as f64, t.f as f64);
        let det = (1.0 - a) * (1.0 - d) - b * c;
        cfg.pan_x = ((1.0 - d) * e + b * f) / det;
        cfg.pan_y = (c * e + (1.0 - a) * f) / det;

        println!("  zoom        lit   connected");
        let mut deep = 0;
        // Past 1e8 only an f64 pan can even ADDRESS the view: one f32
        // ulp near 0.27 is 3e-8, wider than the whole frame there.
        // The far end is set by f64 in turn -- measured, structure
        // survives to 1e17 and the frame is empty by 1e18.
        for zoom in [1.0e5f32, 1.0e7, 1.0e10, 1.0e13] {
            cfg.zoom = zoom;
            let rgba = render(&cfg, N, 48_000_000);
            let lit: Vec<bool> = rgba
                .chunks(4)
                .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .collect();
            let n = N as usize;
            let (mut total, mut joined) = (0usize, 0usize);
            for y in 0..n {
                for x in 0..n {
                    if !lit[y * n + x] {
                        continue;
                    }
                    total += 1;
                    let has = (x > 0 && lit[y * n + x - 1])
                        || (x + 1 < n && lit[y * n + x + 1])
                        || (y > 0 && lit[(y - 1) * n + x])
                        || (y + 1 < n && lit[(y + 1) * n + x]);
                    if has {
                        joined += 1;
                    }
                }
            }
            let frac = joined as f64 / total.max(1) as f64;
            println!("  {zoom:<10.0e}  {total:>5}  {:>8.3}", frac);
            assert!(
                total > 500,
                "zoom {zoom:e}: only {total} lit pixels -- the view found nothing to draw"
            );
            assert!(
                frac > 0.85,
                "zoom {zoom:e}: only {:.1}% of lit pixels have a lit neighbour. That is a \\
                 quantisation lattice, not fractal structure -- the forced prefix is being \\
                 computed in absolute coordinates again",
                frac * 100.0
            );
            deep += 1;
        }
        assert!(deep == 4, "the ladder did not run");
    }

    /// **Gate 4 of deep-zoom-precision.md: a replayed deep zoom holds.**
    ///
    /// Grand-julian, whose maps are roots and so is REPLAYED, down a
    /// ladder from 64k -- where f32's spacing near the view is a third of
    /// a pixel and the plain replay stripes -- to 1e10, each rung rendered
    /// with the replay in offsets and without. Three numbers per render:
    /// lit pixels; the share with a lit neighbour, which a quantisation
    /// lattice fails; and STRIPES, the spread of each column's (and row's)
    /// brightness about the mean of its six neighbours, which rises when
    /// f32's grid gives neighbouring columns unequal shares of the points.
    #[test]
    #[ignore = "needs a GPU; reads output/flame-zoom"]
    fn a_deep_replay_has_no_stripes_and_no_lattice() {
        use std::sync::atomic::Ordering;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame") else {
            println!("  no grand-julian.fflame");
            return;
        };
        const N: u32 = 320;
        let mut cfg: FractalConfig = serde_json::from_str(&text).expect("a config");
        cfg.cylinder_targeting = true;
        cfg.render_mode = crate::scene::transforms::RenderMode::TwoD;
        cfg.deterministic_rng = true;
        cfg.auto_exposure = true;
        {
            let guard = crate::variations::global_registry();
            let b = crate::scene::backward::Backward::read(&cfg.flame, &guard).expect("armed");
            let x = b.sample_point(0.75);
            cfg.pan_x = x[0];
            cfg.pan_y = x[1];
        }
        let measure = |rgba: &[u8]| -> (usize, f64, f64) {
            let n = N as usize;
            let lum: Vec<f64> = rgba.chunks(4).map(|p| p[0] as f64 + p[1] as f64 + p[2] as f64).collect();
            let lit: Vec<bool> = lum.iter().map(|&l| l > 24.0).collect();
            let (mut total, mut joined) = (0usize, 0usize);
            for y in 0..n {
                for x in 0..n {
                    if !lit[y * n + x] {
                        continue;
                    }
                    total += 1;
                    if (x > 0 && lit[y * n + x - 1])
                        || (x + 1 < n && lit[y * n + x + 1])
                        || (y > 0 && lit[(y - 1) * n + x])
                        || (y + 1 < n && lit[(y + 1) * n + x])
                    {
                        joined += 1;
                    }
                }
            }
            // Column and row sums, each against the mean of its six
            // neighbours: the high-frequency part stripes live in.
            let spread = |sums: &[f64]| -> f64 {
                let mut r = Vec::new();
                for i in 3..sums.len() - 3 {
                    let around: f64 = (i - 3..=i + 3).filter(|&j| j != i).map(|j| sums[j]).sum::<f64>() / 6.0;
                    if around > 0.0 {
                        r.push(sums[i] / around);
                    }
                }
                let m = r.iter().sum::<f64>() / r.len().max(1) as f64;
                (r.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / r.len().max(1) as f64).sqrt()
            };
            let cols: Vec<f64> = (0..n).map(|x| (0..n).map(|y| lum[y * n + x]).sum()).collect();
            let rows: Vec<f64> = (0..n).map(|y| (0..n).map(|x| lum[y * n + x]).sum()).collect();
            (total, joined as f64 / total.max(1) as f64, spread(&cols).max(spread(&rows)))
        };
        // Gate 3 first: at 1e3 the plain replay is still nearly right --
        // its GPU transcendentals put it about 0.03 px off there, ten
        // times that at 1e4 (`how_far_the_plain_replay_is_from_f64`) --
        // and the plan already carries references, so the two must draw
        // the same picture. Not
        // the same SAMPLES: the offset steps skip the draws the absolute
        // steps make, so later word picks differ, and the two renders are
        // two samplings of one picture. So the bar is the noise between two
        // samplings of the plain one -- deterministic against not --
        // per pixel and over 4x4 blocks, where noise falls and a real
        // difference would not.
        {
            cfg.zoom = 1.0e3;
            let lum = |px: &[u8]| px[0] as f64 + px[1] as f64 + px[2] as f64;
            let n = N as usize;
            let blocks = |img: &[u8]| -> Vec<f64> {
                let mut out = vec![0.0; (n / 4) * (n / 4)];
                for y in 0..n {
                    for x in 0..n {
                        out[(y / 4) * (n / 4) + x / 4] += lum(&img[(y * n + x) * 4..(y * n + x) * 4 + 4]);
                    }
                }
                out
            };
            let gap = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f64>() / b.iter().sum::<f64>();
            let pixels = |img: &[u8]| img.chunks(4).map(lum).collect::<Vec<f64>>();
            let iters: u64 = std::env::var("GATE3_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(48_000_000);
            crate::scene::backward::OFFSETS_OFF.store(false, Ordering::Relaxed);
            let on = render(&cfg, N, iters);
            crate::scene::backward::OFFSETS_OFF.store(true, Ordering::Relaxed);
            let off = render(&cfg, N, iters);
            cfg.deterministic_rng = false;
            let off2 = render(&cfg, N, iters);
            cfg.deterministic_rng = true;
            crate::scene::backward::OFFSETS_OFF.store(false, Ordering::Relaxed);
            if std::env::var_os("GATE3_SAVE").is_some() {
                let save = |name: &str, img: &[u8]| {
                    let _ = image::save_buffer(format!("output/deep-offsets/{name}.png"), img, N, N, image::ColorType::Rgba8);
                };
                save("g3-on", &on);
                save("g3-off", &off);
                save("g3-off2", &off2);
                let amp = |a: &[u8], b: &[u8]| -> Vec<u8> {
                    a.chunks(4)
                        .zip(b.chunks(4))
                        .flat_map(|(x, y)| {
                            let d = (lum(x) - lum(y)) * 2.0;
                            [(d.max(0.0)).min(255.0) as u8, 0, ((-d).max(0.0)).min(255.0) as u8, 255]
                        })
                        .collect()
                };
                save("g3-diff-on-off", &amp(&on, &off));
                save("g3-diff-off2-off", &amp(&off2, &off));
            }
            let (px_on, px_noise) = (gap(&pixels(&on), &pixels(&off)), gap(&pixels(&off2), &pixels(&off)));
            let (bl_on, bl_noise) = (gap(&blocks(&on), &blocks(&off)), gap(&blocks(&off2), &blocks(&off)));
            println!(
                "  1e3: offsets against plain {:.2}% per pixel, {:.2}% per 4x4 block; two plain samplings {:.2}% and {:.2}%",
                100.0 * px_on,
                100.0 * bl_on,
                100.0 * px_noise,
                100.0 * bl_noise
            );
            assert!(
                px_on < 1.2 * px_noise && bl_on < 1.2 * bl_noise,
                "at 1e3 the offset replay is further from the plain one than sampling noise"
            );
        }
        println!("  zoom       offsets:  lit  joined  stripes |  plain:  lit  joined  stripes");
        for zoom in [65536.0f32, 1.0e6, 1.0e8, 1.0e10] {
            cfg.zoom = zoom;
            crate::scene::backward::OFFSETS_OFF.store(false, Ordering::Relaxed);
            let on_rgba = render(&cfg, N, 48_000_000);
            let _ = std::fs::create_dir_all("output/deep-offsets");
            let _ = image::save_buffer(
                format!("output/deep-offsets/ladder-{zoom:.0e}.png"),
                &on_rgba,
                N,
                N,
                image::ColorType::Rgba8,
            );
            let on = measure(&on_rgba);
            crate::scene::backward::OFFSETS_OFF.store(true, Ordering::Relaxed);
            let off = measure(&render(&cfg, N, 48_000_000));
            crate::scene::backward::OFFSETS_OFF.store(false, Ordering::Relaxed);
            println!(
                "  {zoom:<10.0e}        {:>6}  {:>6.3}  {:>7.4} |       {:>6}  {:>6.3}  {:>7.4}",
                on.0, on.1, on.2, off.0, off.1, off.2
            );
            assert!(on.0 > 500, "zoom {zoom:e}: only {} lit pixels with offsets", on.0);
            assert!(on.1 > 0.85, "zoom {zoom:e}: a lattice with offsets ({:.3} joined)", on.1);
        }
    }

    /// Panning and zooming with targeting on never submits a
    /// destroyed buffer.
    ///
    /// The cylinder table is resized whenever the number of words
    /// reaching the view changes, and a resize DESTROYS the old
    /// buffer. The bind group has to be rebuilt to match, or the next
    /// submit fails validation with "Buffer with 'Cylinder Buffer'
    /// label has been destroyed" and the renderer stays broken until
    /// something forces a full reload.
    ///
    /// **A picture comparison cannot see this**, which is why it
    /// reached the app: every other gate here renders once from a
    /// fresh renderer, so no buffer is ever resized under a live bind
    /// group. What catches it is a wgpu VALIDATION SCOPE around the
    /// app's real sequence — load, then move the view repeatedly,
    /// submitting each time.
    #[test]
    #[ignore = "needs a GPU"]
    fn moving_the_view_never_submits_a_destroyed_buffer() {
        let (device, queue) = device();
        let mut cfg = gasket_config();
        cfg.cylinder_targeting = true;
        cfg.zoom = 64.0;

        let mut r = crate::renderer::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            96,
            96,
            &cfg.flame,
            cfg.palette_size,
        );
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("rebind gate load"),
        });
        r.load_config(&device, &mut enc, &queue, &cfg, &cfg.palette, 1, 0);
        queue.submit(Some(enc.finish()));

        // Recorded rather than panicked in the callback: it can fire
        // from a poll on another thread, where a panic would unwind
        // the wrong stack and report nothing useful.
        let seen: std::sync::Arc<std::sync::Mutex<Option<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = seen.clone();
        device.on_uncaptured_error(std::sync::Arc::new(move |e| {
            let mut g = sink.lock().unwrap();
            if g.is_none() {
                *g = Some(format!("{e}"));
            }
        }));

        // A sweep that genuinely changes how many words reach the
        // view, in both directions, so the table grows and shrinks.
        let mut words = Vec::new();
        for (zoom, pan) in [
            (64.0f32, 0.0f32),
            (256.0, 0.0),
            (4096.0, 0.0),
            (4096.0, 0.25),
            (1024.0, 0.25),
            (65536.0, 0.25),
            (65536.0, 0.0),
            (16.0, 0.0),
            (262144.0, 0.0),
        ] {
            cfg.zoom = zoom;
            cfg.pan_x = pan as f64;
            cfg.pan_y = pan as f64;
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rebind gate frame"),
            });
            if r.sync_cylinders(&device, &queue, &cfg) {
                r.load_config(&device, &mut enc, &queue, &cfg, &cfg.palette, 1, 0);
            }
            words.push(r.targeting_state().clone());
            r.compute_pass(
                &mut enc, &queue, &device, 64, 1, 0, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, cfg.speed_factor, true, false,
            );
            queue.submit(Some(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        }

        let err = seen.lock().unwrap().clone();
        assert!(
            err.is_none(),
            "moving the view with targeting on raised a wgpu error: {}",
            err.unwrap_or_default()
        );

        // The sweep has to have actually resized the table, or it
        // proved nothing. Count the distinct word counts it visited.
        let mut counts: Vec<usize> = words
            .iter()
            .filter_map(|s| match s {
                crate::renderer::TargetingState::Active { words, .. } => Some(*words),
                _ => None,
            })
            .collect();
        counts.sort_unstable();
        counts.dedup();
        assert!(
            counts.len() > 1,
            "the sweep never changed the word count, so it never resized the buffer: {counts:?}"
        );
    }

    /// **A ticked box never reports "Not running."**
    ///
    /// The panel draws its status line only when cylinder targeting
    /// is switched ON, so `TargetingState::Off` reaching it is always
    /// a lie: the user did ask. It happened for every 3D flame,
    /// because the enumeration is planar and `sync_cylinders` folded
    /// "wrong render mode" into the same state as "not asked for".
    /// The flame was fine — `linear3d-modified-zoomed` plans 8 words
    /// at depth 61 for a 117x speedup — and the app said nothing at
    /// all about why it would not use them.
    ///
    /// This pins the distinction rather than the wording: whatever
    /// `Off` comes to mean, it must not be what a ticked box gets.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_ticked_box_always_says_why_not() {
        use crate::renderer::TargetingState as TS;
        let (device, queue) = device();

        let mut cfg = gasket_config();
        cfg.cylinder_targeting = true;
        cfg.zoom = 64.0;
        cfg.render_mode = crate::scene::transforms::RenderMode::ThreeD;

        let mut r = crate::renderer::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            96,
            96,
            &cfg.flame,
            cfg.palette_size,
        );
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("not-planar gate"),
        });
        r.load_config(&device, &mut enc, &queue, &cfg, &cfg.palette, 1, 0);
        queue.submit(Some(enc.finish()));

        r.sync_cylinders(&device, &queue, &cfg);
        assert_eq!(
            *r.targeting_state(),
            TS::NotPlanar,
            "a 3D flame with targeting ON must say the enumeration is 2D-only, not go silent"
        );

        // ...and switching the box off is the one case that may.
        cfg.cylinder_targeting = false;
        r.sync_cylinders(&device, &queue, &cfg);
        assert_eq!(*r.targeting_state(), TS::Off);

        // The same flame in 2D is not refused for the render mode.
        cfg.cylinder_targeting = true;
        cfg.render_mode = crate::scene::transforms::RenderMode::TwoD;
        r.sync_cylinders(&device, &queue, &cfg);
        assert_ne!(*r.targeting_state(), TS::NotPlanar);
        assert_ne!(*r.targeting_state(), TS::Off);
    }

    /// **The root ball really does hold the attractor — asked of the
    /// shader, not of the bounds.**
    ///
    /// `invariant_ball` proves its answer with forward BOUNDS, and a
    /// bound is only as good as the reasoning behind it. Nothing has
    /// ever checked the conclusion against the thing that actually
    /// runs. The leak probe does: an ordinary untargeted render
    /// counts, in world space, every plot attempt that falls outside
    /// a claimed disc.
    ///
    /// Two directions, because a counter that always reads zero would
    /// pass the first half on its own:
    ///
    /// * at the root ball, the count must be EXACTLY zero — every
    ///   sample the real chaos game deposits is inside;
    /// * at a quarter of that radius, it must be clearly non-zero, or
    ///   the instrument is not measuring anything.
    ///
    /// This is the phase-0 validation from
    /// `docs/projects/inversive-targeting.md`: the leaky regions of
    /// phase 1 will rely on this number, so it is calibrated here
    /// against an answer already known.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_leak_probe_agrees_that_the_root_ball_holds_everything() {
        let (device, queue) = device();
        let guard = crate::variations::global_registry();
        let cfg = gasket_config();
        let (root_c, root_r) =
            invariant_ball(&cfg.flame, &guard).expect("the gasket has a root ball");
        drop(guard);

        // **Burn-in matters here and nowhere else in this file.** The
        // chaos game starts at a random point and only approaches the
        // attractor; with none, the first iterations of every thread
        // deposit points that are genuinely outside any invariant
        // region, and the probe correctly reports them (measured:
        // 6.1e-3 of attempts at burn_in = 0). That is the transient,
        // not a hole in the root ball.
        let measure = |c: [f64; 2], r: f64, burn: u32| -> f32 {
            let mut rr = crate::renderer::FlameRenderer::with_palette_size(
                &device,
                &queue,
                wgpu::TextureFormat::Rgba8Unorm,
                128,
                128,
                &cfg.flame,
                cfg.palette_size,
            );
            rr.set_leak_probe(Some((c, r)));
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("leak probe"),
            });
            rr.load_config(&device, &mut enc, &queue, &cfg, &cfg.palette, 64, burn);
            rr.compute_pass(
                &mut enc, &queue, &device, 256, 64, burn, cfg.zoom, cfg.pan_x as f32,
                cfg.pan_y as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, cfg.speed_factor,
                true, false,
            );
            queue.submit(Some(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            rr.apply_exact_density_fraction(&device, &queue);
            rr.leak_fraction().expect("the probe was set, so a fraction must come back")
        };

        let inside = measure(root_c, root_r, 30);
        println!("  at the root ball (r = {root_r:.4}): {inside:.3e} of plot attempts outside");
        assert_eq!(
            inside, 0.0,
            "the root ball is supposed to be invariant, but the real chaos game put \
             {inside:.3e} of its samples outside it"
        );

        let quarter = measure(root_c, root_r * 0.25, 30);
        println!("  at a quarter of it:            {quarter:.3e} outside");
        assert!(
            quarter > 1e-3,
            "a disc a quarter the size should obviously leak, but the probe read \
             {quarter:.3e} -- it is not measuring anything"
        );
    }

    /// The per-frame sync asks for a reload only when the SHADER
    /// changes, and never for an ordinary pan.
    ///
    /// **The picture gate for family M.**
    ///
    /// Everything else in this module measures regions and word
    /// counts. This asks the only question that decides whether any
    /// of it ships: does a TARGETED render of an inversive flame draw
    /// the same picture as an unbiased one?
    ///
    /// `schottky1` is four `mobius` transforms — two circle-pairing
    /// generators and their inverses, decomposed from the
    /// `schottky_group` variation. It is the flame family M exists
    /// for, and until the map-keyed walk it could not be enumerated
    /// at all: a word-indexed expansion treats `a·a⁻¹·w` as a
    /// different cylinder from `w`, and the regions stall.
    ///
    /// Compared the way the affine gate compares: which pixels are
    /// lit, and how bright the lit ones are on average. A forced
    /// render puts every sample in frame, so it is far less noisy
    /// than the reference at the same iteration count — the claim is
    /// that they agree about the SHAPE and the exposure, not that
    /// they are bit-identical.
    #[test]
    #[ignore = "needs a GPU and reads output/flame-zoom"]
    fn a_targeted_schottky_render_is_the_untargeted_render() {
        const N: u32 = 96;
        let stats = |rgba: &[u8]| -> (Vec<bool>, f64) {
            let lit: Vec<bool> = rgba
                .chunks(4)
                .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .collect();
            let (mut acc, mut n) = (0.0f64, 0.0f64);
            for (p, l) in rgba.chunks(4).zip(&lit) {
                if !*l {
                    continue;
                }
                acc += (p[0] as f64 + p[1] as f64 + p[2] as f64) / 765.0;
                n += 1.0;
            }
            (lit, if n > 0.0 { acc / n } else { 0.0 })
        };

        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/schottky1.fflame") else {
            println!("  no schottky1.fflame");
            return;
        };
        let mut base: crate::config::FractalConfig =
            serde_json::from_str(&text).expect("a config");
        base.deterministic_rng = true;
        base.levels_enabled = false;

        // On the set, so the view is not looking at empty space.
        let mf = crate::scene::mobius::MobiusFlame::read(
            &base.flame,
            reg,
            crate::scene::mobius::ROOT_SAMPLE,
        )
        .expect("family M");
        let x = mf.root.points[mf.root.points.len() / 2];
        base.pan_x = x[0];
        base.pan_y = x[1];

        println!("  zoom     words  depth   mass      speedup    lit ref/tgt   overlap  bright");
        let mut checked = 0usize;
        for zoom in [1e2f64, 1e3, 1e4] {
            base.zoom = zoom as f32;
            let plan = match Cylinders::plan(
                &base.flame,
                reg,
                View::of(zoom, [base.pan_x, base.pan_y], N, N),
            ) {
                Ok(p) => p,
                Err(e) => {
                    println!("  {zoom:>7.0e}  {e:?}");
                    continue;
                }
            };
            if plan.speedup() <= 1.0 {
                println!("  {zoom:>7.0e}  speedup {:.2} -- not worth forcing", plan.speedup());
                continue;
            }
            // **Matched on IN-FRAME samples, not on iterations.**
            //
            // The reference lands `iters · mass` of its samples in the
            // view; the forced one lands all of them, but each costs
            // `depth + 1` map applications. Comparing at equal
            // ITERATIONS gave the reference ten times the samples and
            // the gate read that as the targeted render drawing half
            // the picture.
            let mut refc = base.clone();
            refc.cylinder_targeting = false;
            let mut tgt = base.clone();
            tgt.cylinder_targeting = true;

            let iters_ref = 120_000_000u64;
            let iters_tgt = (((iters_ref as f64) * plan.mass * (plan.depth as f64 + 1.0))
                as u64)
                .clamp(4_000_000, 400_000_000);
            let a = render(&refc, N, iters_ref);
            let b = render(&tgt, N, iters_tgt);
            let (la, ba) = stats(&a);
            let (lb, bb) = stats(&b);
            let lit_a = la.iter().filter(|v| **v).count();
            let lit_b = lb.iter().filter(|v| **v).count();
            let both = la.iter().zip(&lb).filter(|(p, q)| **p && **q).count();
            let overlap = both as f64 / lit_a.max(1) as f64;
            println!(
                "  {zoom:>7.0e}  {:>5}  {:>5}  {:.2e}  {:.3e}   {lit_a:>4}/{lit_b:<4}   {overlap:>6.3}  {ba:.3}/{bb:.3}  (tgt iters {iters_tgt:.2e})",
                plan.words.len(),
                plan.depth,
                plan.mass,
                plan.speedup()
            );
            if lit_a < 40 {
                println!("         reference too sparse to compare");
                continue;
            }
            checked += 1;
            assert!(
                overlap > 0.6,
                "at zoom {zoom:.0e} the targeted render lit only {overlap:.3} of what the \
                 reference did -- it is drawing a different picture"
            );
            assert!(
                (ba - bb).abs() < 0.35 * ba.max(bb).max(1e-6),
                "at zoom {zoom:.0e} brightness disagrees: reference {ba:.3}, targeted {bb:.3}"
            );
        }
        assert!(checked > 0, "no zoom produced a comparable pair");
    }

    /// **The picture gate for family J.**
    ///
    /// `grand-julian` is three `julian` transforms with 2, 15 and 8
    /// arms. Its plan comes from the inverse walk, its words carry
    /// arms, and the replay forces each one: this asks whether the
    /// result is the same picture the free chaos game draws. Compared
    /// as the family-M gate compares -- which pixels are lit and how
    /// bright -- at matched in-frame samples.
    #[test]
    #[ignore = "needs a GPU and reads output/flame-zoom"]
    fn a_targeted_grand_julian_render_is_the_untargeted_render() {
        const N: u32 = 96;
        let stats = |rgba: &[u8]| -> (Vec<bool>, f64) {
            let lit: Vec<bool> = rgba
                .chunks(4)
                .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .collect();
            let (mut acc, mut n) = (0.0f64, 0.0f64);
            for (p, l) in rgba.chunks(4).zip(&lit) {
                if !*l {
                    continue;
                }
                acc += (p[0] as f64 + p[1] as f64 + p[2] as f64) / 765.0;
                n += 1.0;
            }
            (lit, if n > 0.0 { acc / n } else { 0.0 })
        };

        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame") else {
            println!("  no grand-julian.fflame");
            return;
        };
        let mut base: crate::config::FractalConfig =
            serde_json::from_str(&text).expect("a config");
        base.deterministic_rng = true;
        base.levels_enabled = false;

        // On the set: a point of the attractor the planner sampled.
        let b = crate::scene::backward::Backward::read(&base.flame, reg).expect("armed");
        let x = b.sample_point(0.75);
        base.pan_x = x[0];
        base.pan_y = x[1];

        println!("  zoom     words  depth   mass      eff   speedup    lit ref/tgt   overlap  bright");
        let mut checked = 0usize;
        let mut failures: Vec<String> = Vec::new();
        for zoom in [1e2f64, 1e4, 1e6] {
            base.zoom = zoom as f32;
            let plan = match Cylinders::plan(
                &base.flame,
                reg,
                View::of(zoom, [base.pan_x, base.pan_y], N, N),
            ) {
                Ok(p) => p,
                Err(e) => {
                    println!("  {zoom:>7.0e}  {e:?}");
                    continue;
                }
            };
            if plan.speedup() <= 1.0 {
                println!("  {zoom:>7.0e}  speedup {:.2} -- not worth forcing", plan.speedup());
                continue;
            }
            let mut refc = base.clone();
            refc.cylinder_targeting = false;
            let mut tgt = base.clone();
            tgt.cylinder_targeting = true;

            // Matched on in-frame samples: the reference lands
            // `iters · mass` of its samples in the view; the forced one
            // lands `efficiency` of them, each costing `depth + 1`.
            let iters_ref = 120_000_000u64;
            let iters_tgt = (((iters_ref as f64) * plan.mass * (plan.depth as f64 + 1.0)
                / plan.efficiency.max(0.05)) as u64)
                .clamp(4_000_000, 400_000_000);
            let ra = render_out(&refc, N, iters_ref);
            let rb = render_out(&tgt, N, iters_tgt);
            let (a, b) = (ra.rgba_data, rb.rgba_data);
            let (la, ba) = stats(&a);
            let (lb, bb) = stats(&b);
            let lit_a = la.iter().filter(|v| **v).count();
            let lit_b = lb.iter().filter(|v| **v).count();
            let both = la.iter().zip(&lb).filter(|(p, q)| **p && **q).count();
            let overlap = both as f64 / lit_a.max(1) as f64;
            println!(
                "  {zoom:>7.0e}  {:>5}  {:>5}  {:.2e}  {:.2}  {:.3e}   {lit_a:>4}/{lit_b:<4}   {overlap:>6.3}  {ba:.3}/{bb:.3}  (tgt iters {iters_tgt:.2e}; frame coverage ref {:.2e} tgt {:.2e})",
                plan.words.len(),
                plan.depth,
                plan.mass,
                plan.efficiency,
                plan.speedup(),
                ra.frame_coverage,
                rb.frame_coverage
            );
            if lit_a < 40 {
                println!("         reference too sparse to compare");
                continue;
            }
            checked += 1;
            if overlap <= 0.6 {
                failures.push(format!(
                    "at zoom {zoom:.0e} the targeted render lit only {overlap:.3} of what the \
                     reference did -- it is drawing a different picture"
                ));
            }
            // Brightness is only comparable against a DENSE reference:
            // a starved one has a sample or two per lit pixel, and its
            // mean is whatever the log map does to that. The overlap
            // stays meaningful there -- with far more in-frame samples,
            // the targeted render must light everything the reference
            // managed to.
            if lit_a >= 1000 && (ba - bb).abs() >= 0.35 * ba.max(bb).max(1e-6) {
                failures.push(format!(
                    "at zoom {zoom:.0e} brightness disagrees: reference {ba:.3}, targeted {bb:.3}"
                ));
            }
        }
        assert!(checked > 0, "no zoom produced a comparable pair");
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// **A saved view, targeted against untargeted**: `FFLAME=path`.
    /// Writes both renders next to the file as `<name>-ref.png` and
    /// `<name>-tgt.png`, and prints how they agree.
    #[test]
    #[ignore = "needs a GPU and reads $FFLAME"]
    fn a_saved_view_targeted_against_untargeted() {
        let Ok(path) = std::env::var("FFLAME") else { return };
        const N: u32 = 384;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut base: FractalConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("file")).expect("config");
        base.deterministic_rng = true;
        base.levels_enabled = false;
        let plan = Cylinders::plan(&base.flame, reg, View::of(base.zoom as f64, [base.pan_x, base.pan_y], N, N))
            .expect("a plan");
        let mut refc = base.clone();
        refc.cylinder_targeting = false;
        let mut tgt = base.clone();
        tgt.cylinder_targeting = true;
        let iters_ref: u64 = std::env::var("REF_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(4_000_000_000);
        let iters_tgt = (((iters_ref as f64) * plan.mass * (plan.depth as f64 + 1.0)
            / plan.efficiency.max(0.05)) as u64)
            .clamp(20_000_000, 2_000_000_000);
        let a = render_out(&refc, N, iters_ref).rgba_data;
        let b = render_out(&tgt, N, iters_tgt).rgba_data;
        let stem = path.trim_end_matches(".fflame");
        let save = |name: &str, data: &[u8]| {
            image::save_buffer(format!("{stem}-{name}.png"), data, N, N, image::ColorType::Rgba8).expect("png");
        };
        save("ref", &a);
        save("tgt", &b);
        let lum = |p: &[u8]| (p[0] as f64 + p[1] as f64 + p[2] as f64) / 765.0;
        let lit = |d: &[u8]| d.chunks(4).map(|p| lum(p) > 0.03).collect::<Vec<_>>();
        let (la, lb) = (lit(&a), lit(&b));
        let na = la.iter().filter(|v| **v).count();
        let nb = lb.iter().filter(|v| **v).count();
        let both = la.iter().zip(&lb).filter(|(x, y)| **x && **y).count();
        // Where the reference is lit and the target is DARK: holes.
        let holes = la.iter().zip(&lb).filter(|(x, y)| **x && !**y).count();
        // Mean brightness over pixels lit in both.
        let (mut sa, mut sb, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for (pa, pb) in a.chunks(4).zip(b.chunks(4)) {
            if lum(pa) > 0.03 && lum(pb) > 0.03 {
                sa += lum(pa);
                sb += lum(pb);
                n += 1.0;
            }
        }
        println!(
            "plan {} words depth {}, mass {:.2e}, eff {:.2}, speedup {:.2e}\n\
             ref iters {iters_ref:.2e}, tgt iters {iters_tgt:.2e}\n\
             lit ref {na} tgt {nb}; overlap {:.3} of ref; holes (ref lit, tgt dark) {:.3} of ref\n\
             brightness over shared pixels ref {:.3} tgt {:.3}\n\
             wrote {stem}-ref.png and {stem}-tgt.png",
            plan.words.len(), plan.depth, plan.mass, plan.efficiency, plan.speedup(),
            both as f64 / na.max(1) as f64, holes as f64 / na.max(1) as f64,
            sa / n.max(1.0), sb / n.max(1.0)
        );
    }

    /// Points on `flame`'s attractor, by the render's own maps: each of
    /// 256 scattered seeds carried through its own random 48-symbol word
    /// (transforms by weight, arms at random) on the planning kernel. Works
    /// for any flame the kernel builds for, analysable or not -- which is
    /// the point, since the question below is about the ones that are not.
    ///
    /// Before any final transform: the plan kernel applies normals only.
    fn attractor_points(device: &wgpu::Device, queue: &wgpu::Queue, flame: &Flame, n: usize) -> Vec<[f64; 2]> {
        use crate::scene::plan_gpu::{EvalJob, PlanGpu};
        let mut st = 0x5EED_u64;
        let mut rnd = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let seeds: Vec<[f64; 2]> = (0..256).map(|_| [rnd() * 2.0 - 1.0, rnd() * 2.0 - 1.0]).collect();
        let live: Vec<(usize, f64)> =
            flame.transforms.iter().enumerate().filter(|(_, t)| t.weight > 0.0).map(|(i, t)| (i, t.weight as f64)).collect();
        if live.is_empty() {
            return Vec::new();
        }
        let total: f64 = live.iter().map(|l| l.1).sum();
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut gpu = PlanGpu::new(device, queue, flame, &seeds);
        if pollster::block_on(scope.pop()).is_some() {
            return Vec::new();
        }
        let words: Vec<Vec<u32>> = (0..seeds.len())
            .map(|_| {
                (0..48)
                    .map(|_| {
                        let mut u = rnd() * total;
                        let mut pick = live[live.len() - 1].0;
                        for &(i, w) in &live {
                            if u < w {
                                pick = i;
                                break;
                            }
                            u -= w;
                        }
                        // An arm for a many-valued variation; below 16, so
                        // `2 pi k` stays exact in f32. Ignored by the rest.
                        sym_of(pick as u32, (rnd() * 16.0) as u32)
                    })
                    .collect()
            })
            .collect();
        let idx: Vec<[u32; 1]> = (0..seeds.len() as u32).map(|k| [k]).collect();
        let jobs: Vec<EvalJob> = words.iter().zip(&idx).map(|(w, i)| EvalJob { word: w, points: i }).collect();
        let pts: Vec<[f64; 2]> = gpu
            .endpoints(&jobs)
            .into_iter()
            .flatten()
            .map(|p| [p[0] as f64, p[1] as f64])
            .filter(|p| p[0].abs() < 1e6 && p[1].abs() < 1e6)
            .collect();
        let step = (pts.len() / n.max(1)).max(1);
        pts.into_iter().step_by(step).take(n).collect()
    }

    /// **Do the flames that convert to escape time deep-zoom, and only
    /// they?** The theory in `gpu-cylinder-planning.md` §12, decision 4.
    ///
    /// For every flame in the corpus (`output/*.flame` and
    /// `output/flame-zoom/*.fflame`): does it convert -- mode D's
    /// `pack_flame`, planar or solid, and the planar `analyse_2d` alone --
    /// and does targeting pay at depth -- a plan with a speedup above one
    /// at 1e3 and 1e5 times the flame's own zoom, centred on three points
    /// of its attractor. The table is the answer; the off-diagonal cells
    /// are named with their reasons.
    #[test]
    #[ignore = "needs a GPU; reads output/*.flame and output/flame-zoom"]
    fn do_escape_and_deep_zoom_go_together() {
        use std::collections::BTreeMap;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let (device, queue) = device();

        let mut corpus: Vec<(String, FractalConfig)> = Vec::new();
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir("output")
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|x| x.to_str()) == Some("flame")).collect())
            .unwrap_or_default();
        files.sort();
        for path in &files {
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            let Ok(configs) = crate::flame_xml::parse_flame_xml(&text) else { continue };
            let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let many = configs.len() > 1;
            for (k, cfg) in configs.into_iter().enumerate() {
                corpus.push((if many { format!("{stem}#{k}") } else { stem.clone() }, cfg));
            }
        }
        let mut zoomset: Vec<std::path::PathBuf> = std::fs::read_dir("output/flame-zoom")
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|x| x.to_str()) == Some("fflame")).collect())
            .unwrap_or_default();
        zoomset.sort();
        for path in &zoomset {
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            let Ok(cfg) = serde_json::from_str::<FractalConfig>(&text) else { continue };
            corpus.push((format!("zoom/{}", path.file_stem().unwrap_or_default().to_string_lossy()), cfg));
        }
        if corpus.is_empty() {
            println!("  no corpus -- nothing to measure");
            return;
        }

        // (escape, planar, deep) -> names, with the reason for the deep answer.
        let mut cells: BTreeMap<(bool, bool), Vec<String>> = BTreeMap::new();
        let mut planar_cells: BTreeMap<(bool, bool), usize> = BTreeMap::new();
        for (name, cfg) in &corpus {
            let flame = &cfg.flame;
            let escape = crate::escape::ifs::pack_flame(flame, reg, None).is_ok();
            let planar = crate::scene::ifs_analysis::analyse_2d(flame, reg);
            let finals = flame.has_attachments();
            let points = attractor_points(&device, &queue, flame, 3);
            let mut best: Option<f64> = None;
            let mut why = String::new();
            for p in &points {
                for mult in [1e3f64, 1e5] {
                    let view = View::of(cfg.zoom.max(1e-6) as f64 * mult, *p, 512, 512);
                    match Cylinders::plan(flame, reg, view) {
                        Ok(c) => best = Some(best.unwrap_or(0.0).max(c.speedup())),
                        Err(e) => {
                            why = format!("{e:?}");
                        }
                    }
                }
            }
            let deep = best.is_some_and(|s| s > 1.0);
            let detail = match (best, points.is_empty()) {
                (_, true) => "no attractor points (kernel did not build)".to_string(),
                (Some(s), _) => format!("speedup {s:.2e}"),
                (None, _) => {
                    let w: String = why.chars().take(90).collect();
                    format!("refused: {w}")
                }
            };
            println!(
                "  {name:<34} escape {:<5} planar {:<5} deep {:<5} {}{}",
                escape,
                planar.is_ok(),
                deep,
                detail,
                if finals { "  [has finals: centres before them]" } else { "" }
            );
            cells.entry((escape, deep)).or_default().push(format!("{name} ({detail})"));
            *planar_cells.entry((planar.is_ok(), deep)).or_default() += 1;
        }

        let n = |m: &BTreeMap<(bool, bool), Vec<String>>, k: (bool, bool)| m.get(&k).map_or(0, |v| v.len());
        println!("\n  {} flames. Escape time (mode D) against deep zoom (targeting pays at depth):", corpus.len());
        println!("                     deep     not deep");
        println!("    converts       {:>6}    {:>6}", n(&cells, (true, true)), n(&cells, (true, false)));
        println!("    does not       {:>6}    {:>6}", n(&cells, (false, true)), n(&cells, (false, false)));
        let p = |k: (bool, bool)| planar_cells.get(&k).copied().unwrap_or(0);
        println!("  The planar analysis alone (what the inverse walk starts from):");
        println!("    analysable     {:>6}    {:>6}", p((true, true)), p((true, false)));
        println!("    not            {:>6}    {:>6}", p((false, true)), p((false, false)));
        for (k, label) in [((true, false), "converts but does not deep-zoom"), ((false, true), "deep-zooms but does not convert")] {
            if let Some(v) = cells.get(&k) {
                println!("\n  {label}:");
                for w in v {
                    println!("    {w}");
                }
            }
        }
    }

    /// **Phase 2's contention gate** (`gpu-cylinder-planning.md` §14): a
    /// plan made while the render runs, with the CPU's answers and with
    /// the GPU's. The GPU planner's batches share the render's queue, so
    /// each waits behind whatever frame is running; this measures what
    /// that does to a plan, and what a plan does to the frames.
    ///
    /// Two renders. **As the app draws**: 128 workgroups a frame (the
    /// app's governor never dispatches more) paced to 60 Hz. **Heavy**:
    /// frames of ~12 ms of GPU work back to back, the worst a busy queue
    /// can do to a round trip. A plan is timed from the moment it starts
    /// -- the settle delay before it is the view's, not the planner's.
    #[test]
    #[ignore = "needs a GPU; reads output/flame-zoom"]
    fn a_plan_on_the_gpu_shares_it_with_the_render() {
        use std::time::{Duration, Instant};
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame") else {
            println!("  no grand-julian.fflame");
            return;
        };
        let (device, queue) = device();
        let mut cfg: FractalConfig = serde_json::from_str(&text).expect("a config");
        cfg.cylinder_targeting = true;
        cfg.render_mode = crate::scene::transforms::RenderMode::TwoD;
        let start = {
            let guard = crate::variations::global_registry();
            let b = crate::scene::backward::Backward::read(&cfg.flame, &guard).expect("armed");
            b.sample_point(0.75)
        };
        let (w, h) = (1280u32, 720u32);
        let mut r = crate::renderer::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            w,
            h,
            &cfg.flame,
            cfg.palette_size,
        );
        r.set_background_planning(true);
        let load = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig| {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("contention load") });
            r.load_config(&device, &mut enc, &queue, cfg, &cfg.palette, 1, 0);
            queue.submit(Some(enc.finish()));
        };
        // One frame: a compute pass of `groups` workgroups, waited for,
        // then held to `pace` if it came in under it (vsync).
        let frame = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig, groups: u32, pace: Duration| -> Duration {
            let t = Instant::now();
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("contention frame") });
            if r.sync_cylinders(&device, &queue, cfg) {
                r.load_config(&device, &mut enc, &queue, cfg, &cfg.palette, 1, 0);
            }
            r.compute_pass(
                &mut enc, &queue, &device, groups, 256, 0, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, cfg.speed_factor, false, false,
            );
            queue.submit(Some(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            let spent = t.elapsed();
            if spent < pace {
                std::thread::sleep(pace - spent);
            }
            spent
        };
        let pct = |v: &mut Vec<Duration>, q: f64| -> f64 {
            v.sort();
            v.get(((v.len() as f64 - 1.0) * q).round() as usize).map_or(0.0, |d| d.as_secs_f64() * 1e3)
        };

        for heavy in [false, true] {
            for gpu in [false, true] {
                r.set_plan_on_gpu(gpu);
                cfg.zoom = 1e3;
                cfg.pan_x = start[0];
                cfg.pan_y = start[1];
                load(&mut r, &cfg);
                // The first plan, and the shader it brings, out of the way.
                let t0 = Instant::now();
                loop {
                    frame(&mut r, &cfg, 64, Duration::ZERO);
                    if r.take_plan_arrived() {
                        break;
                    }
                    assert!(t0.elapsed() < Duration::from_secs(90), "no first plan");
                }
                let (groups, pace) = if heavy {
                    // Calibrated to ~12 ms of GPU work, back to back.
                    let mut groups = 256u32;
                    for _ in 0..6 {
                        let mut v: Vec<Duration> = (0..5).map(|_| frame(&mut r, &cfg, groups, Duration::ZERO)).collect();
                        let ms = pct(&mut v, 0.5);
                        groups = ((groups as f64 * 12.0 / ms.max(0.1)) as u32).clamp(16, 65535);
                    }
                    (groups, Duration::ZERO)
                } else {
                    (128, Duration::from_micros(16_667))
                };
                let mut idle: Vec<Duration> = (0..30).map(|_| frame(&mut r, &cfg, groups, pace)).collect();
                let idle_med = pct(&mut idle, 0.5);

                // Five moves; each plan timed with the frames it overlapped.
                let mut plans = Vec::new();
                let mut during: Vec<Duration> = Vec::new();
                for k in 0..5 {
                    cfg.zoom *= if k % 2 == 0 { 3.0 } else { 0.5 };
                    let t0 = Instant::now();
                    let mut began: Option<Instant> = None;
                    loop {
                        let f = frame(&mut r, &cfg, groups, pace);
                        if r.planning_elapsed().is_some() {
                            began.get_or_insert_with(Instant::now);
                            during.push(f);
                        }
                        if r.take_plan_arrived() {
                            break;
                        }
                        assert!(t0.elapsed() < Duration::from_secs(90), "no plan after a move");
                    }
                    // A zoom out inside the standby needs no plan at all.
                    if let Some(b) = began {
                        plans.push(b.elapsed().as_secs_f64() * 1e3);
                    }
                    // Let the standby plan finish before the next move, so
                    // each move times one plan.
                    let t1 = Instant::now();
                    while r.plans_running() && t1.elapsed() < Duration::from_secs(30) {
                        frame(&mut r, &cfg, groups, pace);
                    }
                }
                let n = during.len();
                println!(
                    "== {} frames ({groups} workgroups, {idle_med:.1} ms), planner on the {}: plans {} ms | \
                     {n} frames during them: median {:.1} ms, p95 {:.1} ms, max {:.1} ms",
                    if heavy { "heavy" } else { "app-like" },
                    if gpu { "GPU" } else { "CPU" },
                    plans.iter().map(|p| format!("{p:.0}")).collect::<Vec<_>>().join(", "),
                    pct(&mut during, 0.5),
                    pct(&mut during, 0.95),
                    pct(&mut during, 1.0),
                );
            }
        }
    }

    /// **Planning on a background thread, driven the way the app drives
    /// it.**
    ///
    /// `sync_cylinders` once a frame, with background planning on:
    ///
    /// - the first plan is generated off the caller's thread and lands
    ///   with `take_plan_arrived`, which is the app's cue to reset;
    /// - a VIEW-only move keeps the previous plan drawing while the next
    ///   is generated, and the panel sees `planning_elapsed`;
    /// - a FLAME edit drops the plan at once -- its words may name
    ///   transforms that no longer exist -- and plans again;
    /// - `load_config` after a plan lands does not plan a second time.
    #[test]
    #[ignore = "needs a GPU and reads output/flame-zoom"]
    fn a_plan_is_made_in_the_background() {
        use crate::renderer::TargetingState as TS;
        use std::time::{Duration, Instant};
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame") else {
            println!("  no grand-julian.fflame");
            return;
        };
        let (device, queue) = device();
        let mut cfg: FractalConfig = serde_json::from_str(&text).expect("a config");
        cfg.cylinder_targeting = true;
        cfg.render_mode = crate::scene::transforms::RenderMode::TwoD;
        {
            let guard = crate::variations::global_registry();
            let b = crate::scene::backward::Backward::read(&cfg.flame, &guard).expect("armed");
            let x = b.sample_point(0.75);
            cfg.pan_x = x[0];
            cfg.pan_y = x[1];
        }
        cfg.zoom = 1e3;

        let mut r = crate::renderer::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            256,
            256,
            &cfg.flame,
            cfg.palette_size,
        );
        r.set_background_planning(true);
        let load = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig| {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("background plan gate"),
            });
            r.load_config(&device, &mut enc, &queue, cfg, &cfg.palette, 1, 0);
            queue.submit(Some(enc.finish()));
        };
        // Frames until a plan lands; returns the longest single sync, so
        // the test can say whether the caller's thread ever blocked.
        let run_until_arrived = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig, saw_planning: &mut bool| -> Duration {
            let t0 = Instant::now();
            let mut longest = Duration::ZERO;
            loop {
                // The PLANNING call alone. A true return is followed by a
                // `load_config` that recompiles the shader when targeting
                // starts -- a once-per-start cost that has nothing to do
                // with planning, and that other GPU tests running
                // alongside can stretch past any fixed bound.
                let s = Instant::now();
                let reload = r.sync_cylinders(&device, &queue, cfg);
                longest = longest.max(s.elapsed());
                if reload {
                    load(r, cfg);
                }
                if r.planning_elapsed().is_some() {
                    *saw_planning = true;
                }
                if r.take_plan_arrived() {
                    return longest;
                }
                assert!(t0.elapsed() < Duration::from_secs(90), "no plan arrived in 90 s");
                std::thread::sleep(Duration::from_millis(10));
            }
        };

        load(&mut r, &cfg);
        let mut saw = false;
        let t0 = Instant::now();
        let longest = run_until_arrived(&mut r, &cfg, &mut saw);
        println!(
            "  first plan: {:.2} s, longest sync on the caller {:.1} ms, state {:?}",
            t0.elapsed().as_secs_f64(),
            longest.as_secs_f64() * 1e3,
            r.targeting_state()
        );
        assert!(saw, "the panel never saw a plan being generated");
        assert!(matches!(r.targeting_state(), TS::Active { .. }), "{:?}", r.targeting_state());
        assert!(
            longest < Duration::from_millis(250),
            "a sync blocked the caller for {longest:?} -- the plan is not in the background"
        );
        // The reload after arrival must not plan again.
        let s = Instant::now();
        load(&mut r, &cfg);
        assert!(r.planning_elapsed().is_none(), "load_config started a second plan");
        println!("  reload after arrival: {:.1} ms", s.elapsed().as_secs_f64() * 1e3);

        // A view-only move: the old plan stays on screen while the next
        // is generated.
        cfg.zoom *= 1.5;
        let mut saw = false;
        let mut kept_during = false;
        let t0 = Instant::now();
        loop {
            if r.sync_cylinders(&device, &queue, &cfg) {
                load(&mut r, &cfg);
            }
            if r.planning_elapsed().is_some() {
                saw = true;
                kept_during |= matches!(r.targeting_state(), TS::Active { .. });
            }
            if r.take_plan_arrived() {
                break;
            }
            assert!(t0.elapsed() < Duration::from_secs(90), "no plan after a zoom");
            std::thread::sleep(Duration::from_millis(10));
        }
        println!("  after a zoom: {:.2} s, state {:?}", t0.elapsed().as_secs_f64(), r.targeting_state());
        assert!(saw && kept_during, "a view-only move must keep the previous plan while planning");

        // A flame edit: the plan is dropped at once.
        cfg.flame.transforms[1].weight *= 1.25;
        load(&mut r, &cfg);
        assert!(
            matches!(r.targeting_state(), TS::Off),
            "a flame edit must drop the old plan at once, not keep drawing it: {:?}",
            r.targeting_state()
        );
        let mut saw = false;
        let t0 = Instant::now();
        run_until_arrived(&mut r, &cfg, &mut saw);
        println!("  after an edit: {:.2} s, state {:?}", t0.elapsed().as_secs_f64(), r.targeting_state());
        assert!(matches!(r.targeting_state(), TS::Active { .. }), "{:?}", r.targeting_state());
    }

    /// **The standby plan keeps a moving view complete.**
    ///
    /// Once a tight plan lands, a plan for a disc twice the view's radius
    /// is made in reserve. A pan of half a view radius must swap it in
    /// on that very frame -- no settle delay, no plan to wait for -- and
    /// signal a reset; the tight plan for the new view follows. A pan
    /// far outside the standby must not use it.
    #[test]
    #[ignore = "needs a GPU and reads output/flame-zoom"]
    fn a_standby_plan_covers_a_move() {
        standby_flow(false);
    }

    /// **The web's plan job, on the desktop**: the same flow as
    /// `a_standby_plan_covers_a_move` with plans made as the web makes
    /// them -- a task polled a slice at a time each frame
    /// (`gpu-cylinder-planning.md` phase 3) -- and no frame's planning
    /// allowed past a 60 Hz frame.
    #[test]
    #[ignore = "needs a GPU; reads output/flame-zoom"]
    fn the_web_plan_job_plans_a_slice_a_frame() {
        let [cold, warm] = standby_flow(true);
        // Until the first plan is up the frames include building the
        // planner's kernel, one piece no tick can split: a native driver
        // compiles it on this thread, 11-21 ms measured, where a browser
        // compiles it in its GPU process instead (and what that costs is
        // the browser's own gate's to measure: tests/visual/wasm/
        // test_plan.py, gpu-cylinder-planning.md §17). Reported, not gated.
        println!(
            "  longest sync_cylinders in task mode: {:.1} ms to the first plan (kernel build included), {:.1} ms after",
            cold.as_secs_f64() * 1e3,
            warm.as_secs_f64() * 1e3
        );
        assert!(warm < std::time::Duration::from_millis(16), "a frame's planning took {warm:?}");
    }

    /// The standby flow, with plans on a worker thread or in a task.
    /// Returns the longest single `sync_cylinders` up to the first plan,
    /// and after it.
    fn standby_flow(task: bool) -> [std::time::Duration; 2] {
        use crate::renderer::TargetingState as TS;
        use std::time::{Duration, Instant};
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame") else {
            println!("  no grand-julian.fflame");
            return [Duration::ZERO; 2];
        };
        let (device, queue) = device();
        let mut cfg: FractalConfig = serde_json::from_str(&text).expect("a config");
        cfg.cylinder_targeting = true;
        cfg.render_mode = crate::scene::transforms::RenderMode::TwoD;
        {
            let guard = crate::variations::global_registry();
            let b = crate::scene::backward::Backward::read(&cfg.flame, &guard).expect("armed");
            let x = b.sample_point(0.75);
            cfg.pan_x = x[0];
            cfg.pan_y = x[1];
        }
        cfg.zoom = 1e3;
        const N: u32 = 256;
        let mut r = crate::renderer::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            N,
            N,
            &cfg.flame,
            cfg.palette_size,
        );
        r.set_background_planning(true);
        r.set_plan_in_task(task);
        let longest = std::cell::Cell::new([Duration::ZERO; 2]);
        let warm = std::cell::Cell::new(false);
        let load = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig| {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("standby gate"),
            });
            r.load_config(&device, &mut enc, &queue, cfg, &cfg.palette, 1, 0);
            queue.submit(Some(enc.finish()));
        };
        let frame = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig| -> Duration {
            let s = Instant::now();
            let reload = r.sync_cylinders(&device, &queue, cfg);
            // The planning alone: a reload recompiles the shader, which
            // is not the planner's time.
            let mut l = longest.get();
            let i = warm.get() as usize;
            l[i] = l[i].max(s.elapsed());
            longest.set(l);
            if reload {
                load(r, cfg);
            }
            s.elapsed()
        };
        let wait = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig, what: &str, done: &dyn Fn(&mut crate::renderer::FlameRenderer) -> bool| {
            let t0 = Instant::now();
            loop {
                frame(r, cfg);
                if done(r) {
                    println!("  {what}: {:.2} s", t0.elapsed().as_secs_f64());
                    return;
                }
                assert!(t0.elapsed() < Duration::from_secs(60), "{what}: nothing in 60 s");
                std::thread::sleep(Duration::from_millis(10));
            }
        };

        load(&mut r, &cfg);
        wait(&mut r, &cfg, "first tight plan", &|r| r.take_plan_arrived());
        warm.set(true);
        wait(&mut r, &cfg, "standby ready", &|r| r.has_standby_plan());

        // A pan of half a view radius: inside the standby's disc.
        let radius = View::of(cfg.zoom as f64, [cfg.pan_x, cfg.pan_y], N, N).radius;
        cfg.pan_x += 0.5 * radius;
        let took = frame(&mut r, &cfg);
        assert!(r.take_plan_arrived(), "the standby was not swapped in on the frame the view moved");
        assert!(!r.has_standby_plan(), "a swapped-in standby is still held");
        assert!(matches!(r.targeting_state(), TS::Active { .. }), "{:?}", r.targeting_state());
        assert!(took < Duration::from_millis(100), "the swap blocked for {took:?}");
        println!("  swap on the first frame after a half-radius pan: {:.1} ms", took.as_secs_f64() * 1e3);
        wait(&mut r, &cfg, "tight plan for the new view", &|r| r.take_plan_arrived());
        wait(&mut r, &cfg, "standby around the new view", &|r| r.has_standby_plan());

        // A pan far outside the standby: no swap; the old plan draws
        // while a new one is made.
        cfg.pan_x += 5.0 * radius;
        frame(&mut r, &cfg);
        assert!(!r.take_plan_arrived(), "a standby was swapped in for a view it does not cover");
        wait(&mut r, &cfg, "tight plan after a long pan", &|r| r.take_plan_arrived());
        assert!(matches!(r.targeting_state(), TS::Active { .. }), "{:?}", r.targeting_state());

        // **Moving while a plan is made.** Each pan lands while the last
        // one's plan is still in flight, and starting the next plan
        // drops it -- at whatever point it had reached, a readback from
        // the GPU included. The web crashed here: a task dropped while
        // its readback was out left the planner's staging buffer mapped,
        // and the next plan's first batch panicked in `map_async`
        // ("Buffer is already mapped"). A thread, told to stop, finishes
        // its batch first.
        let mut in_flight = 0;
        for k in 0..8u32 {
            cfg.pan_x += if k % 2 == 0 { 5.0 } else { -5.0 } * radius;
            cfg.pan_y += 0.1 * radius;
            // The settle (250 ms), then a different way into the plan
            // each time.
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_millis(260 + 40 * k as u64) {
                frame(&mut r, &cfg);
                std::thread::sleep(Duration::from_millis(10));
            }
            in_flight += r.plans_running() as u32;
        }
        println!("  {in_flight} of 8 pans landed with a plan in flight");
        assert!(in_flight > 0, "no pan landed mid-plan: the drop was not tested");
        // Then somewhere no standby covers -- over two radii from every
        // view so far -- held still: after all those drops, the planner
        // still makes a plan.
        cfg.pan_y -= 3.0 * radius;
        frame(&mut r, &cfg);
        r.take_plan_arrived();
        wait(&mut r, &cfg, "a fresh plan after the pans mid-plan", &|r| r.take_plan_arrived());
        assert!(matches!(r.targeting_state(), TS::Active { .. }), "{:?}", r.targeting_state());
        longest.get()
    }

    /// **The gate for a bug the whole suite missed.** `sync_cylinders`
    /// used to rebuild the shader itself, from the raw config —
    /// while `load_config` compiles against the sticky-adopted flame
    /// and packs the variation-params buffer against that same local
    /// index map. The rebuilt shader's variation indices therefore did
    /// not match the buffer, `get_param` read the wrong slots, and
    /// every flame in the app collapsed to a single pixel at the
    /// origin: on load, and again after every pan, with any real
    /// config change appearing to fix it.
    ///
    /// Nothing here caught it because nothing here drove the APP's
    /// order — load a config, then move the view, then render. The
    /// CLI and every render gate call `load_config` once and never
    /// move, so the whole path was untested. This test is that order.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_pan_does_not_disturb_the_shader() {
        let (device, queue) = device();
        let mut cfg = gasket_config();
        // A nonlinear variation, so the sticky superset has something
        // to retain and the two index maps can actually differ.
        cfg.flame.transforms[1].set_variation("spherical", 0.3);

        let mut r = crate::renderer::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            96,
            96,
            &cfg.flame,
            cfg.palette_size,
        );
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sync gate"),
        });
        r.load_config(&device, &mut enc, &queue, &cfg, &cfg.palette, 1, 0);
        queue.submit(Some(enc.finish()));

        // The app's per-frame call, with nothing changed: must be a
        // no-op, because `load_config` already recorded the key.
        assert!(
            !r.sync_cylinders(&device, &queue, &cfg),
            "an unchanged view must not ask for a reload"
        );

        // ...and with the view moved, targeting off (the default, and
        // what every existing flame does). Still no reload: there is
        // no shader change to make, and asking for one here is what
        // broke the app.
        for (dx, dz) in [(0.01f32, 1.0f32), (-0.2, 1.0), (0.0, 8.0), (0.0, 4096.0)] {
            cfg.pan_x += dx as f64;
            cfg.zoom *= dz;
            assert!(
                !r.sync_cylinders(&device, &queue, &cfg),
                "a pan or zoom with targeting off must not ask for a reload \
                 (pan {}, zoom {})",
                cfg.pan_x,
                cfg.zoom
            );
        }

        // Switching targeting ON at a zoom where it pays DOES change
        // the shader, and must say so.
        let mut on = gasket_config();
        on.cylinder_targeting = true;
        on.zoom = 64.0;
        assert!(
            r.sync_cylinders(&device, &queue, &on),
            "starting targeting changes the shader and must ask for a reload"
        );
    }

    /// A targeted render is the untargeted render — the same picture,
    /// from a fraction of the samples.
    ///
    /// This is the estimator's claim, end to end: sampling a word
    /// with probability `p_a/P(A_V)`, carrying a point of the free
    /// orbit through it and plotting there has expectation `μ|_V`, so
    /// the picture must be the one the ordinary chaos game draws.
    ///
    /// Compared as a PICTURE rather than per pixel — the two draw
    /// from the same distribution by different routes and each has
    /// its own noise — so: which pixels are lit, and how bright they
    /// are on average. The untargeted reference gets sixteen times
    /// the iterations, because at these zooms it is starved and a
    /// fair-budget comparison would be against noise rather than
    /// against the answer.
    ///
    /// Measured on a gasket at `S₀`'s fixed point:
    ///
    /// ```text
    ///   zoom   P(A_V)    speedup   lit ref / tgt   overlap   brightness
    ///   2^2    1.00e0        0.5    594 /  603     100.0%   0.514 / 0.508
    ///   2^4    1.11e-1       3.0    595 /  603      99.8%   0.266 / 0.263
    ///   2^6    1.23e-2      16.2    600 /  594      99.0%   0.153 / 0.154
    ///   2^8    1.37e-3     104.1    596 /  594      99.7%   0.093 / 0.093
    ///   2^10   1.52e-4     729.0    588 /  594      99.5%   0.056 / 0.057
    /// ```
    ///
    /// The same picture at seven hundred times the rate.
    ///
    /// **Why it stops at 2^10: the REFERENCE gives out, not the
    /// target.** At 2^12 the unbiased render lights 252 pixels to the
    /// targeted one's 378 and the overlap falls to 78% — the targeted
    /// render is drawing structure its starved reference never
    /// reaches, which is the direction the whole stage exists to
    /// produce and is also exactly what makes it unverifiable. There
    /// is no comparison past the point where nothing else can draw
    /// the picture.
    ///
    /// **An earlier version of this gate stopped at 2^4 and blamed
    /// the tone map. That was wrong.** The renders past 2^4 came out
    /// with max channel zero, unmoved by four thousand times the
    /// exposure, which reads exactly like starvation — and it was the
    /// PALETTE. A zoom toward `S₀`'s fixed point selects the all-`S₀`
    /// word, flam3's colour rule walks the colour coordinate to
    /// transform 0's colour, and the fixture gave transform 0 the
    /// colour `0.0`: the black end of the palette. Full density, no
    /// light. `gasket_config` now starts its colours at 0.5, and six
    /// zoom levels that were thought to be out of reach were there
    /// the whole time. The iteration-count normalisation IS a real
    /// limit on deep views — that is what `auto_exposure` addresses —
    /// but it was not what made this gate dark.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_targeted_render_is_the_untargeted_render() {
        const N: u32 = 96;
        let stats = |rgba: &[u8]| -> (Vec<bool>, f64) {
            let lit: Vec<bool> = rgba
                .chunks(4)
                .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .collect();
            let (mut acc, mut n) = (0.0f64, 0.0f64);
            for (p, l) in rgba.chunks(4).zip(&lit) {
                if !*l {
                    continue;
                }
                acc += (p[0] as f64 + p[1] as f64 + p[2] as f64) / 765.0;
                n += 1.0;
            }
            (lit, if n > 0.0 { acc / n } else { 0.0 })
        };

        let reg = crate::variations::global_registry();
        println!("  zoom   P(A_V)    speedup   lit ref / tgt   overlap   brightness");
        for zoom_pow in [2i32, 4, 6, 8, 10] {
            let mut base = gasket_config();
            base.zoom = 2f32.powi(zoom_pow);
            let plan = Cylinders::plan(
                &base.flame,
                &reg,
                View::of(base.zoom as f64, [base.pan_x, base.pan_y], N, N),
            )
            .expect("a gasket is affine");
            let mut tgt = base.clone();
            tgt.cylinder_targeting = true;

            let (lit_r, bright_r) = stats(&render(&base, N, 64_000_000));
            let (lit_t, bright_t) = stats(&render(&tgt, N, 4_000_000));

            let nr = lit_r.iter().filter(|b| **b).count();
            let nt = lit_t.iter().filter(|b| **b).count();
            let both = lit_r.iter().zip(&lit_t).filter(|(a, b)| **a && **b).count();
            let overlap = both as f64 / nr.max(1) as f64;
            println!(
                "  2^{zoom_pow:<4} {:.2e}  {:>7.1}   {nr:>4} / {nt:>4}   {:>6.1}%   {bright_r:.3} / {bright_t:.3}",
                plan.mass,
                plan.speedup(),
                overlap * 100.0
            );

            assert!(nr > 100 && nt > 100, "2^{zoom_pow}: {nr} / {nt} lit -- nothing to compare");
            assert!(
                overlap > 0.95,
                "2^{zoom_pow}: the targeted render covers only {:.1}% of the reference's lit \
                 pixels -- it is drawing a different set, so the forced prefix is not the \
                 word the enumeration named",
                overlap * 100.0
            );
            assert!(
                (bright_t / bright_r - 1.0).abs() < 0.2,
                "2^{zoom_pow}: the targeted render is {:.3}x the reference's brightness -- the \
                 iteration count is not being inflated by 1/P(A_V), so the deposit's weight is \
                 wrong",
                bright_t / bright_r
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::transforms::Transform;
    use crate::variations::global_registry;

    fn affine(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, w: f32) -> Transform {
        let mut t = Transform::default();
        t.a = a;
        t.b = b;
        t.c = c;
        t.d = d;
        t.e = e;
        t.f = f;
        t.weight = w;
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("linear", 1.0);
        t
    }

    fn flame_of(ts: Vec<Transform>) -> Flame {
        let mut f = Flame::default();
        f.transforms = ts;
        f
    }

    /// A point ON the attractor: `S₀(p) = p/2` fixes the origin, so
    /// it is in the set at every scale. A generic point is not — the
    /// gasket is measure zero, and centring on (0.25, 0.25) made the
    /// view empty past 2^16, which is what found this.
    const ON_SET: [f64; 2] = [0.0, 0.0];

    fn gasket() -> Flame {
        flame_of(vec![
            affine(0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 1.0),
            affine(0.5, 0.0, 0.0, 0.5, 0.5, 0.0, 1.0),
            affine(0.5, 0.0, 0.0, 0.5, 0.25, 0.5, 1.0),
        ])
    }

    /// A chaos-game sample of the attractor, for checking that what
    /// the enumeration claims about the measure is true of the set.
    fn sample(flame: &Flame, n: usize) -> Vec<[f64; 2]> {
        let reg = global_registry();
        let maps: Vec<(Affine2, f64)> = flame
            .transforms
            .iter()
            .map(|t| {
                (
                    crate::scene::ifs_analysis::transform_affine_2d(t, &reg).expect("affine"),
                    t.weight as f64,
                )
            })
            .collect();
        let total: f64 = maps.iter().map(|m| m.1).sum();
        let mut st = 0x2545_F491_4F6C_DD1Du64;
        let mut rnd = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (st >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut p = [0.0f64, 0.0];
        let mut out = Vec::with_capacity(n);
        for i in 0..n + 200 {
            let u = rnd() * total;
            let mut acc = 0.0;
            let mut k = maps.len() - 1;
            for (j, m) in maps.iter().enumerate() {
                acc += m.1;
                if u <= acc {
                    k = j;
                    break;
                }
            }
            p = maps[k].0.apply(p);
            if i >= 200 {
                out.push(p);
            }
        }
        out
    }

    /// The identity the stage rests on: the kept words' probability
    /// is the share of the invariant measure the viewport holds.
    ///
    /// `P(A_V) = Σ p_a` over the words whose image reaches the view,
    /// and the chaos game's own share of samples landing in the view
    /// has to agree. It is an upper bound rather than an equality --
    /// a word is kept when its image OVERLAPS the view, and an
    /// overlapping image may put only part of its measure inside --
    /// so this checks that the sampled share is under it and of the
    /// right order.
    #[test]
    fn the_kept_mass_bounds_the_measure_in_view() {
        let reg = global_registry();
        let flame = gasket();
        let pts = sample(&flame, 400_000);
        println!("  zoom   words  depth   P(A_V)    sampled    ratio   speedup");
        for zoom in [2.0f64, 4.0, 8.0, 16.0, 32.0, 64.0] {
            let view = View::of(zoom, ON_SET, 96, 96);
            let cyl = Cylinders::plan(&flame, &reg, view).expect("a gasket is affine");
            // The chaos game's own share of the view, over the disc
            // the enumeration tested against.
            let inside = pts
                .iter()
                .filter(|p| {
                    (p[0] - view.centre[0]).hypot(p[1] - view.centre[1]) <= view.radius
                })
                .count();
            let sampled = inside as f64 / pts.len() as f64;
            println!(
                "  {zoom:<6} {:<6} {:<6} {:.3e}  {sampled:.3e}  {:>6.2}  {:>7.1}x",
                cyl.words.len(),
                cyl.depth,
                cyl.mass,
                cyl.mass / sampled.max(1e-12),
                cyl.speedup()
            );
            assert!(
                cyl.mass >= sampled * 0.95,
                "at 2^{zoom} the kept mass {:.3e} is under the sampled share {sampled:.3e} -- \
                 the enumeration is missing words the chaos game reaches, which would leave \
                 part of the view unrendered",
                cyl.mass
            );
            assert!(
                cyl.mass <= sampled * 12.0 + 1e-6,
                "at 2^{zoom} the kept mass {:.3e} is {:.1}x the sampled share -- the bound has \
                 gone so loose that most forced samples would miss the view",
                cyl.mass,
                cyl.mass / sampled.max(1e-12)
            );
        }
    }

    /// The antichain is PREFIX-FREE, which is what makes the unrolled
    /// identity a sum over disjoint events rather than a double
    /// count.
    ///
    /// If one kept word were a prefix of another, the orbit passing
    /// through the shorter would also pass through the longer and its
    /// measure would be counted twice.
    #[test]
    fn the_kept_words_are_prefix_free() {
        let reg = global_registry();
        for zoom in [2.0f64, 8.0, 32.0] {
            let cyl = Cylinders::plan(&gasket(), &reg, View::of(zoom, ON_SET, 96, 96))
                .expect("affine");
            for (i, a) in cyl.words.iter().enumerate() {
                for (j, b) in cyl.words.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    assert!(
                        !b.word.starts_with(&a.word),
                        "at 2^{zoom} word {:?} is a prefix of {:?}, so the measure through it \
                         is counted twice",
                        a.word,
                        b.word
                    );
                }
            }
        }
    }

    /// A word's stored image is the image its own composed map
    /// produces, and that image really does contain where the
    /// attractor's points go.
    ///
    /// The bound is `σ_max` per step, which for an affine map is
    /// exact in the worst direction — so a point of the ball has to
    /// land inside, and the check is against the chaos-game sample
    /// pushed through the same word.
    #[test]
    fn a_words_image_contains_where_its_points_land() {
        let reg = global_registry();
        let flame = gasket();
        let maps: Vec<Affine2> = flame
            .transforms
            .iter()
            .map(|t| crate::scene::ifs_analysis::transform_affine_2d(t, &reg).expect("affine"))
            .collect();
        let pts = sample(&flame, 20_000);
        let cyl = Cylinders::plan(&flame, &reg, View::of(16.0, ON_SET, 96, 96))
            .expect("affine");
        let mut checked = 0usize;
        for c in &cyl.words {
            for p in pts.iter().step_by(37) {
                // The word is stored oldest-first, which is the order
                // the chaos game applies it in.
                let mut q = *p;
                for &s in &c.word {
                    q = maps[sym_transform(s) as usize].apply(q);
                }
                let d = (q[0] - c.centre[0]).hypot(q[1] - c.centre[1]);
                assert!(
                    d <= c.radius * 1.0000001,
                    "word {:?}: a point of the attractor lands {d:.6e} from the image centre, \
                     outside the claimed radius {:.6e}",
                    c.word,
                    c.radius
                );
                checked += 1;
            }
        }
        assert!(checked > 500, "only {checked} points checked");
    }

    /// Every kept word's image really does reach the view, and its
    /// radius really is under the view's — the two stopping rules,
    /// asserted rather than assumed.
    #[test]
    fn every_kept_word_reaches_the_view_and_fits_it() {
        let reg = global_registry();
        for zoom in [4.0f64, 16.0, 64.0] {
            let view = View::of(zoom, ON_SET, 96, 96);
            let cyl = Cylinders::plan(&gasket(), &reg, view).expect("affine");
            for c in &cyl.words {
                let d = (c.centre[0] - view.centre[0]).hypot(c.centre[1] - view.centre[1]);
                assert!(
                    d <= c.radius + view.radius,
                    "at 2^{zoom} word {:?} was kept but its image is {d:.4e} away, past \
                     {:.4e} + {:.4e}",
                    c.word,
                    c.radius,
                    view.radius
                );
                assert!(
                    c.radius <= view.radius || c.word.len() >= MAX_DEPTH,
                    "at 2^{zoom} word {:?} was cut at radius {:.4e} against a view of {:.4e} \
                     without reaching the depth cap",
                    c.word,
                    c.radius,
                    view.radius
                );
            }
        }
    }

    /// What the stage is for, as a number: the deeper the view, the
    /// more the forced prefix is worth.
    ///
    /// `speedup` is `1 / (mass · (depth + 1))` — every sample lands
    /// in view at a cost of `depth + 1` map applications, against the
    /// unbiased game's one application of which a `mass` fraction is
    /// useful. The point is the SHAPE: `mass` falls like a power of
    /// the zoom while `depth` grows like its logarithm, so the ratio
    /// grows without bound.
    ///
    /// Measured on the gasket at the origin — `S₀`'s fixed point, so
    /// the view is on the set at every scale:
    ///
    /// ```text
    ///   zoom   words  depth    P(A_V)    speedup
    ///   2^1        3      1   1.00e0        0.5x
    ///   2^3        1      1   3.33e-1       1.5x
    ///   2^6        1      4   1.23e-2      16.2x
    ///   2^9        1      7   4.57e-4     273.4x
    ///   2^12       1     10   1.69e-5     5,368.1x
    ///   2^16       1     14   2.09e-7   318,864.6x
    ///   2^20       1     18   2.58e-9  20,390,552x
    /// ```
    ///
    /// Roughly doubling per octave, which is `1/mass` growing like
    /// `zoom^D` against `depth` growing like `log zoom`. **Below 2^2
    /// it is a LOSS** — the whole attractor fits the view, every
    /// sample is already useful, and the prefix is pure overhead.
    /// That is the right answer and the reason the renderer has to be
    /// able to decline: targeting is for depth, and at no depth there
    /// is nothing to target.
    ///
    /// Only ONE word survives from 2^3 on, because a view that small
    /// sits inside a single cylinder. That is the ideal case and it
    /// is the common one: the enumeration's branching is transient,
    /// dying out as soon as the view stops straddling pieces.
    #[test]
    fn the_speedup_grows_with_the_zoom() {
        let reg = global_registry();
        let flame = gasket();
        println!("  zoom   words  depth    P(A_V)    speedup");
        let mut prev = 0.0f64;
        let mut first_win = None;
        for zoom_pow in [1i32, 3, 6, 9, 12, 16, 20] {
            let zoom = 2f64.powi(zoom_pow);
            let view = View::of(zoom, ON_SET, 96, 96);
            let cyl = Cylinders::plan(&flame, &reg, view).expect("affine");
            let up = cyl.speedup();
            println!(
                "  2^{zoom_pow:<4} {:<6} {:<6} {:.2e}  {up:>10.1}x",
                cyl.words.len(),
                cyl.depth,
                cyl.mass
            );
            // The depth must track the zoom: a prefix that stopped
            // growing would mean the enumeration had stopped
            // resolving, and the speedup would be an artefact of the
            // mass alone.
            assert!(
                cyl.depth >= (zoom_pow as usize).saturating_sub(2),
                "at 2^{zoom_pow} the deepest word is only {} symbols -- the enumeration is not \
                 following the zoom",
                cyl.depth
            );
            if zoom_pow > 1 {
                assert!(
                    up > prev,
                    "at 2^{zoom_pow} the speedup {up:.2} did not beat the previous {prev:.2} -- \
                     a targeting whose value does not rise with depth is not answering \
                     starvation"
                );
            }
            if first_win.is_none() && up > 1.0 {
                first_win = Some(zoom_pow);
            }
            prev = up;
        }
        assert_eq!(
            first_win,
            Some(3),
            "targeting starts paying at a different zoom than it did -- below it the prefix is \
             pure overhead, and the renderer's decision to decline is keyed on that"
        );
        assert!(
            prev > 1e6,
            "by 2^20 the speedup is only {prev:.1e}x -- the asymptotics are not what this \
             stage exists for"
        );
    }

    /// The packed affine IS the word replayed, and the packed colour
    /// fold IS the colour folded per transform.
    ///
    /// This is the error-prone part of the stage: a composition taken
    /// in the wrong order still produces a plausible picture, of a
    /// different flame. So both are checked against a direct replay
    /// rather than against a derivation — the point, pushed through
    /// the maps one at a time, against the one matrix the kernel
    /// will use.
    ///
    /// The colour is checked the same way because it folds the same
    /// way: flam3's `c ← c·h + g` per transform is an affine map of
    /// `c`, so a word is `c ← c·H + G`, and getting `G`'s inner
    /// product over the WRONG end of the word is the same class of
    /// mistake with the same plausible-looking result.
    #[test]
    fn the_packed_word_is_the_word_replayed() {
        let reg = global_registry();
        let mut flame = gasket();
        // Distinct colours and a non-zero speed, so a fold taken the
        // wrong way round cannot come out right by symmetry.
        for (i, t) in flame.transforms.iter_mut().enumerate() {
            t.color = [0.1f32, 0.55, 0.9][i];
            t.color_speed = [0.0f32, 0.3, 0.6][i];
        }
        let maps: Vec<Affine2> = flame
            .transforms
            .iter()
            .map(|t| crate::scene::ifs_analysis::transform_affine_2d(t, &reg).expect("affine"))
            .collect();

        let probes = [[0.3f64, 0.2], [-0.4, 0.7], [0.0, 0.0], [0.62, 0.11]];
        let mut checked = 0usize;
        // Shallow, where the view straddles pieces and there are
        // several short words, through deep, where there is one word
        // of eighteen symbols -- a composition order that is wrong
        // shows up at length and a CDF that is wrong shows up at
        // breadth.
        for zoom_pow in [1i32, 2, 4, 8, 16, 20] {
        let cyl = Cylinders::plan(&flame, &reg, View::of(2f64.powi(zoom_pow), ON_SET, 96, 96))
            .expect("affine");
        let packed = pack(&cyl, &flame, &reg);
        assert_eq!(packed.len(), cyl.words.len() * WORD_FLOATS);

        for (w, c) in cyl.words.iter().enumerate() {
            let b = w * WORD_FLOATS;
            let m = Affine2 {
                m: [
                    [packed[b] as f64, packed[b + 1] as f64],
                    [packed[b + 2] as f64, packed[b + 3] as f64],
                ],
                t: [packed[b + 4] as f64, packed[b + 5] as f64],
            };
            let (big_h, big_g) = (packed[b + 6] as f64, packed[b + 7] as f64);

            for p in probes {
                // Replay: apply the word's maps one at a time, in the
                // order the chaos game would.
                let mut q = p;
                for &sym in &c.word {
                    q = maps[sym_transform(sym) as usize].apply(q);
                }
                let got = m.apply(p);
                let e = (got[0] - q[0]).hypot(got[1] - q[1]);
                assert!(
                    e < 1e-6,
                    "word {:?}: the composed affine puts {p:?} at {got:?} where the replay \
                     puts it at {q:?} ({e:.2e})",
                    c.word
                );
                checked += 1;
            }

            for c0 in [0.0f64, 0.25, 0.5, 1.0] {
                let mut col = c0;
                for &sym in &c.word {
                    let t = &flame.transforms[sym_transform(sym) as usize];
                    let s = t.color_speed as f64;
                    col = col * ((1.0 + s) * 0.5) + t.color as f64 * (1.0 - s) * 0.5;
                }
                let got = c0 * big_h + big_g;
                assert!(
                    (got - col).abs() < 1e-6,
                    "word {:?}: the folded colour from {c0} is {got} where the replay gives \
                     {col} -- H and G are over the wrong end of the word",
                    c.word
                );
                checked += 1;
            }
        }
        // The CDF is ascending and ends at exactly one, so a uniform
        // draw always finds a word however the floats rounded.
        let mut prev = 0.0f32;
        for w in 0..cyl.words.len() {
            let v = packed[w * WORD_FLOATS + 8];
            assert!(v >= prev, "the CDF went backwards at word {w}: {v} after {prev}");
            prev = v;
        }
        assert_eq!(prev, 1.0, "the CDF ends at {prev}, so a draw near one finds nothing");
        }
        // Six zooms, one to three words each, eight comparisons a word.
        assert!(checked >= 80, "only {checked} comparisons");
    }

    /// A GENERIC point of the attractor enumerates, not just a fixed
    /// point of one of the maps.
    ///
    /// **This is the gate that was missing, and its absence hid a
    /// real bug for three commits.** Every fixture here sat on `S₀`'s
    /// fixed point, where the word is `S₀^k` and a word extended on
    /// the outside happens to nest exactly as one extended on the
    /// inside does. The enumeration was extending on the OUTSIDE and
    /// pruning as though the children nested, which is false in
    /// general — so everywhere except a fixed point it discarded
    /// branches that reached the view, and the view came back empty.
    ///
    /// Reported from the app as "I can only keep zooming in on a
    /// single point the whole fractal converges on", which is
    /// precisely the shape of the bug.
    #[test]
    fn a_generic_point_of_the_attractor_enumerates() {
        let reg = global_registry();
        let f = gasket();

        // A point of the attractor that is NOT a fixed point of any
        // map: follow a repeating address long enough to land on the
        // set. Every map here is affine, so this is exact arithmetic.
        let mut p = [0.0f64, 0.0];
        for k in 0..60 {
            let t = &f.transforms[[0usize, 1, 2, 1, 0, 2][k % 6]];
            p = [
                t.a as f64 * p[0] + t.b as f64 * p[1] + t.e as f64,
                t.c as f64 * p[0] + t.d as f64 * p[1] + t.f as f64,
            ];
        }
        for zoom_pow in [4i32, 6, 8, 10] {
            let view = View::of(2f64.powi(zoom_pow), p, 96, 96);
            let plan = Cylinders::plan(&f, &reg, view).unwrap_or_else(|e| {
                panic!(
                    "2^{zoom_pow} at a generic attractor point {p:?} refused with {e:?} -- \
                     the enumeration is losing the branch that reaches the view"
                )
            });
            // 2^4 is still a shallow view of a unit-sized attractor,
            // so declining there is right; the claim is that the
            // payoff arrives, not that it is always present.
            if zoom_pow >= 6 {
                assert!(
                    plan.speedup() > 1.0,
                    "2^{zoom_pow}: speedup {:.2} at a generic point -- the enumeration                      reaches the view but finds no saving, which it should by this depth",
                    plan.speedup()
                );
            }
        }
    }

    /// Whether this one variation can be bounded at all, on a
    /// transform that carries it alone. The set cover needs a
    /// per-VARIATION answer, and `transform_ball_2d` gives a
    /// per-transform one.
    fn single_variation_bounds(
        name: &str,
        t: &crate::scene::transforms::Transform,
        reg: &crate::variations::VariationRegistry,
    ) -> bool {
        let mut solo = t.clone();
        solo.variations.clear();
        solo.variation_order.clear();
        solo.set_variation(name, t.variations.get(name).copied().unwrap_or(1.0));
        crate::scene::ifs_ball::transform_ball_2d(&solo, reg, Ball::new([0.0, 0.0], 1.0))
            .is_ok()
    }

    /// **The corpus meter: what actually blocks targeting, ranked.**
    ///
    /// "How do we support more flames" has a measured answer, and
    /// guessing at it is how the forward-bound registry ends up full
    /// of variations nobody's flame uses.
    ///
    /// Every `.flame` in `output/` is checked against each blocker
    /// INDEPENDENTLY rather than by running `plan` and taking its
    /// first refusal. That distinction is the whole point: `plan`
    /// tests colour before boundedness, so a flame with both is
    /// filed under colour and the bound it also needs never shows up
    /// in the tally. What is wanted is the marginal unlock — how many
    /// flames a given fix actually frees — and only the independent
    /// form gives that.
    ///
    /// Ignored, like `how_often_a_real_flame_sums_a_kernel_with_an_affine`
    /// which it is modelled on, because it reads a directory that is
    /// not in the repository. Run it before adding a bound; add the
    /// one at the top.
    #[test]
    #[ignore = "reads output/*.flame"]
    fn what_blocks_targeting_across_the_corpus() {
        use std::collections::{BTreeMap, BTreeSet};
        let guard = crate::variations::global_registry();
        let reg = &*guard;

        let mut files: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(rd) = std::fs::read_dir("output") {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("flame") {
                    files.push(p);
                }
            }
        }
        files.sort();
        if files.is_empty() {
            println!("  no corpus in output/ — nothing to measure");
            return;
        }

        // Per flame: the set of independent blockers.
        let mut per_flame: Vec<BTreeSet<&'static str>> = Vec::new();
        let mut blockers: BTreeMap<String, usize> = BTreeMap::new();
        let mut total = 0usize;

        for path in &files {
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            let Ok(configs) = crate::flame_xml::parse_flame_xml(&text) else { continue };
            for cfg in configs {
                total += 1;
                let mut blocks: BTreeSet<&'static str> = BTreeSet::new();
                if cfg.flame.has_xaos() {
                    blocks.insert("xaos");
                }
                let mut names: BTreeSet<String> = BTreeSet::new();
                for t in &cfg.flame.transforms {
                    if t.weight <= 0.0 {
                        continue;
                    }
                    // **The real question, asked of the real code.**
                    // Not "is there a hand-written bound" -- a bound
                    // derived from the WGSL counts exactly as much --
                    // so this pushes a disc through the transform the
                    // way the enumeration does and records what came
                    // back.
                    if let Err(why) = crate::scene::ifs_ball::transform_ball_2d(
                        t,
                        reg,
                        Ball::new([0.0, 0.0], 1.0),
                    ) {
                        blocks.insert("a variation has no forward bound");
                        names.insert(format!("{why}"));
                    }
                    for name in t.ordered_variation_names(reg) {
                        if t.variations.get(&name).copied().unwrap_or(0.0) == 0.0 {
                            continue;
                        }
                        let Some(info) = reg.get(&name) else { continue };
                        if info.has_feature(crate::variations::Feature::WritesColor)
                            || info.has_feature(crate::variations::Feature::WritesRgb)
                        {
                            blocks.insert("colour is not affine (a DC/RGB variation)");
                        }
                    }
                }
                for n in names {
                    *blockers.entry(n).or_default() += 1;
                }
                per_flame.push(blocks);
            }
        }

        let clear = per_flame.iter().filter(|b| b.is_empty()).count();
        println!("\n  {total} flames from {} files; {clear} have no blocker at all.\n", files.len());

        // How many flames each blocker touches, and how many it is
        // the ONLY thing standing in the way of.
        let mut touch: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut sole: BTreeMap<&'static str, usize> = BTreeMap::new();
        for b in &per_flame {
            for k in b {
                *touch.entry(k).or_default() += 1;
                if b.len() == 1 {
                    *sole.entry(k).or_default() += 1;
                }
            }
        }
        println!("  blocker                                 blocks  sole cause");
        let mut rows: Vec<_> = touch.iter().collect();
        rows.sort_by_key(|(k, n)| (std::cmp::Reverse(**n), *k));
        for (k, n) in rows {
            println!("    {k:<38} {n:>4}   {:>4}", sole.get(*k).copied().unwrap_or(0));
        }

        // **Greedy set cover.** A flame needs EVERY one of its
        // variations bounded, so the unlock is not additive: bounding
        // the commonest blocker frees nothing if each of its flames
        // also carries a rarer one. This is the number that decides
        // whether hand-deriving bounds is a plan or a treadmill.
        {
            let mut need: Vec<BTreeSet<String>> = Vec::new();
            for path in &files {
                let Ok(text) = std::fs::read_to_string(path) else { continue };
                let Ok(configs) = crate::flame_xml::parse_flame_xml(&text) else { continue };
                for cfg in configs {
                    let mut miss: BTreeSet<String> = BTreeSet::new();
                    for t in &cfg.flame.transforms {
                        if t.weight <= 0.0 { continue; }
                        for name in t.ordered_variation_names(reg) {
                            if t.variations.get(&name).copied().unwrap_or(0.0) == 0.0 { continue; }
                            if reg.get(&name).is_none() { continue; }
                            let affine = crate::scene::ifs_analysis::affine_role(
                                &name, 1.0, t, reg,
                                crate::scene::ifs_analysis::Space::Planar,
                            ).is_some();
                            let bounded = crate::variations::bound::for_name(&name).is_some()
                                || single_variation_bounds(&name, t, reg);
                            if !affine && !bounded {
                                miss.insert(name.clone());
                            }
                        }
                    }
                    need.push(miss);
                }
            }
            println!();
            println!("  greedy set cover -- bounds added, flames freed of the bound blocker:");
            let mut have: BTreeSet<String> = BTreeSet::new();
            for step in 1..=14 {
                let mut counts: BTreeMap<String, usize> = BTreeMap::new();
                for m in &need {
                    if m.is_subset(&have) { continue; }
                    for v in m.difference(&have) {
                        *counts.entry(v.clone()).or_default() += 1;
                    }
                }
                let mut best: Option<(String, usize)> = None;
                for (v, c) in &counts {
                    if best.as_ref().map_or(true, |(_, bc)| c > bc) {
                        best = Some((v.clone(), *c));
                    }
                }
                let Some((v, _)) = best else { break };
                have.insert(v.clone());
                let freed = need.iter().filter(|m| m.is_subset(&have)).count();
                println!("    +{step:<2} {v:<22} -> {freed:>3} of {} flames clear", need.len());
            }
        }

        println!("\n  variations with no forward bound, by flames blocked:");
        let mut by_var: Vec<_> = blockers.iter().collect();
        by_var.sort_by_key(|(name, n)| (std::cmp::Reverse(**n), (*name).clone()));
        for (name, n) in by_var.iter().take(20) {
            println!("    {n:>4}  {name}");
        }
        println!("    ({} distinct)", blockers.len());
    }

    /// **An attractor nowhere near the origin still gets a root
    /// ball, and a tight one.**
    ///
    /// The fallback used to try discs centred at the origin only, so
    /// a flame living at (40, -25) needed a radius of about 48 just
    /// to be reached — and a disc that large is much less likely to
    /// hold its own images, especially once a nonlinear body that is
    /// gentle on the attractor gets evaluated way out at the origin.
    /// Such flames were refused with `NoInvariantBall`, which read as
    /// "this fractal is unsupported" and meant "we looked in the
    /// wrong place".
    ///
    /// The radius assertion is the real content. Finding SOME ball is
    /// easy; finding one that is not mostly empty space is what keeps
    /// the enumeration from wasting its depth budget crossing the gap
    /// between the origin and the set.
    #[test]
    fn an_off_origin_attractor_gets_a_tight_root_ball() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let far = [40.0f32, -25.0f32];
        let mut flame = crate::scene::transforms::Flame::new();
        flame.transforms.clear();
        for (dx, dy) in [(0.0f32, 0.0f32), (0.5, 0.0), (0.25, 0.5)] {
            let mut t = Transform::default();
            // Half-scale maps whose fixed points sit around `far`.
            t.a = 0.5;
            t.d = 0.5;
            t.e = far[0] * 0.5 + dx;
            t.f = far[1] * 0.5 + dy;
            t.weight = 1.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            flame.transforms.push(t);
        }

        let (c, r) = invariant_ball(&flame, reg).expect("a ball around the attractor");

        // It is a ball, and it is invariant -- the property the
        // enumeration actually relies on.
        for t in &flame.transforms {
            let img = crate::scene::ifs_ball::transform_ball_2d(t, reg, Ball::new(c, r))
                .expect("bounded");
            let d = ((img.c[0] - c[0]).powi(2) + (img.c[1] - c[1]).powi(2)).sqrt();
            assert!(d + img.r <= r * (1.0 + 1e-9), "not invariant: {d} + {} > {r}", img.r);
        }

        // And it is near the set, not near the origin. The attractor
        // spans well under a unit here, so anything past a few units
        // means the search is still centred on the wrong point.
        let off = ((c[0] - far[0] as f64).powi(2) + (c[1] - far[1] as f64).powi(2)).sqrt();
        assert!(off < 2.0, "centre {c:?} is {off} from the attractor at {far:?}");
        assert!(r < 3.0, "radius {r} is far larger than the attractor");
    }

    /// **Does a word's disc keep shrinking all the way down?**
    ///
    /// The enumeration cuts a word when its disc fits the view, so
    /// everything depends on discs shrinking at roughly the map's own
    /// contraction rate. An exact affine does: `affine_ball` uses
    /// `sigma_max`, which is the truth. A DERIVED bound does not have
    /// to — interval arithmetic over-estimates, and if it
    /// over-estimated by a constant factor per level the error would
    /// compound geometrically and a deep word's disc would be
    /// hundreds of times too big. That is exactly what "the view
    /// straddles too many pieces" would look like from the outside.
    ///
    /// So: walk one word down and print the ratio each level.
    #[test]
    #[ignore = "prints a measurement"]
    fn does_a_derived_disc_keep_shrinking() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;

        let build = |extra: f32| {
            let mut f = crate::scene::transforms::Flame::new();
            f.transforms.clear();
            for (e, g) in [(0.0f32, 0.0f32), (0.5, 0.0), (0.25, 0.5)] {
                let mut t = Transform::default();
                t.a = 0.5;
                t.d = 0.5;
                t.e = e;
                t.f = g;
                t.weight = 1.0;
                t.variations.clear();
                t.variation_order.clear();
                t.set_variation("linear", 1.0);
                if extra != 0.0 {
                    t.set_variation("sinusoidal", extra);
                }
                f.transforms.push(t);
            }
            f
        };

        for (label, extra) in [("affine only", 0.0f32), ("+ sinusoidal", 0.05)] {
            let flame = build(extra);
            let bounders: Vec<_> = flame
                .transforms
                .iter()
                .map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).unwrap())
                .collect();
            let (c, r) = invariant_ball(&flame, reg).expect("root");
            println!();
            println!("  {label}: root r = {r:.6}");
            let mut b = Ball::new(c, r);
            let mut prev = b.r;
            for depth in 1..=24 {
                // The word that a deep zoom actually selects: the same
                // symbol over and over, toward one fixed point.
                let Ok(next) = bounders[0].apply(b) else {
                    println!("    depth {depth:>2}: refused");
                    break;
                };
                b = next;
                if depth <= 6 || depth % 6 == 0 {
                    println!(
                        "    depth {depth:>2}:  r = {:>12.3e}   ratio {:.4}",
                        b.r,
                        b.r / prev
                    );
                }
                prev = b.r;
            }
        }
    }

    /// **Nothing that works today loses any measure.**
    ///
    /// `Cylinders::lost` exists for the leaky regions the inversive
    /// family needs, where the alternative is refusing the flame
    /// outright. It must stay exactly zero everywhere else, because a
    /// non-zero value means the picture is missing part of its
    /// attractor — and before this field existed, a word whose bound
    /// failed was dropped with no trace at all.
    ///
    /// Also the arithmetic: `mass` and `lost` are disjoint shares of
    /// the same unit measure, so they cannot sum past 1.
    #[test]
    #[ignore = "reads output/*.flame and output/flame-zoom"]
    fn every_working_flame_loses_nothing() {
        const ARMED_LOST_CEILING: f64 = 1e-2;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut checked = 0usize;
        let mut offenders: Vec<String> = Vec::new();
        let mut reported: Vec<String> = Vec::new();

        let mut configs: Vec<(String, crate::config::FractalConfig)> = Vec::new();
        for dir in ["output", "output/flame-zoom"] {
            let Ok(rd) = std::fs::read_dir(dir) else { continue };
            let mut paths: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                match p.extension().and_then(|x| x.to_str()) {
                    Some("fflame") => {
                        if let Ok(c) = serde_json::from_str::<crate::config::FractalConfig>(&text) {
                            configs.push((stem, c));
                        }
                    }
                    Some("flame") => {
                        if let Ok(cs) = crate::flame_xml::parse_flame_xml(&text) {
                            for (i, c) in cs.into_iter().enumerate() {
                                configs.push((format!("{stem}#{i}"), c));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if configs.is_empty() {
            println!("  no corpus — nothing to check");
            return;
        }

        for (name, cfg) in &configs {
            // Its own framing and several depths, plus a view centred
            // on the attractor so the deep enumerations are exercised
            // rather than short-circuited by an empty view.
            let mut centres = vec![[cfg.pan_x, cfg.pan_y]];
            if let Ok((c, _)) = invariant_ball(&cfg.flame, reg) {
                let bs: Vec<_> = cfg
                    .flame
                    .transforms
                    .iter()
                    .filter(|t| t.weight > 0.0)
                    .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
                    .collect();
                if !bs.is_empty() {
                    let mut p = c;
                    for k in 0..96 {
                        if let Ok(img) = bs[k % bs.len()].apply(Ball::new(p, 0.0)) {
                            p = img.c;
                        }
                    }
                    centres.push(p);
                }
            }
            for centre in centres {
                for mult in [1.0f64, 1e3, 1e6, 1e9] {
                    let view =
                        View::of((cfg.zoom.max(1e-6) as f64) * mult, centre, 512, 512);
                    if let Ok(c) = Cylinders::plan(&cfg.flame, reg, view) {
                        checked += 1;
                        assert!(
                            c.mass + c.lost <= 1.0 + 1e-9,
                            "{name}: mass {} + lost {} exceeds the unit measure",
                            c.mass,
                            c.lost
                        );
                        // Family M drops measure BY DESIGN — it has
                        // no invariant ball and a narrow beam — so it
                        // is reported rather than failed. Everything
                        // else must still lose nothing, which is the
                        // regression this gate exists for.
                        let family_m = crate::scene::mobius::MobiusFlame::read(
                            &cfg.flame,
                            reg,
                            256,
                        )
                        .is_some();
                        // So does the inverse walk that plans an ARMED
                        // flame: its beam and measure floor charge
                        // what they drop to `lost` (backward.rs), so
                        // the loss is accounted, not silent. It gets a
                        // ceiling rather than a pass -- measured at
                        // 1e-4 to 3e-3 across the corpus, and a
                        // regression in the walk would show as more.
                        let armed = cfg.flame.transforms.iter().any(|t| {
                            t.weight > 0.0
                                && t.variations.iter().any(|(n, w)| {
                                    *w != 0.0
                                        && crate::variations::bound::arms_for(n).is_some()
                                })
                        });
                        if c.lost != 0.0 {
                            let line =
                                format!("{name} at x{mult:.0e}: lost {:.3e}", c.lost);
                            if family_m || (armed && c.lost < ARMED_LOST_CEILING) {
                                reported.push(line);
                            } else {
                                offenders.push(line);
                            }
                        }
                    }
                }
            }
        }

        println!("  {checked} successful enumerations across {} flames", configs.len());
        if !reported.is_empty() {
            println!("  family M and armed flames, which drop measure by design:");
            for line in &reported {
                println!("    {line}");
            }
        }
        assert!(
            offenders.is_empty(),
            "these enumerate but silently drop measure, and nothing outside family M \
             should:\n  {}",
            offenders.join("\n  ")
        );
    }

    /// **The fair test family M never had.**
    ///
    /// Every earlier attempt used `spherical`, which is inversion in a
    /// circle and therefore an INVOLUTION: `S_i ∘ S_i` is the
    /// identity, so words fold back on themselves and regions
    /// oscillate instead of shrinking. I concluded from that that no
    /// shipped variation gives a loxodromic generator. That was wrong
    /// — `mobius` has been in the registry the whole time, and its
    /// body is exactly `(Az + B)/(Cz + D)`, the map this module
    /// composes.
    ///
    /// A Schottky group needs generators that pair DISJOINT discs:
    /// `g` maps the outside of `D_a` onto the inside of `D_b`, and
    /// `g⁻¹` does the reverse. Then every point of the limit set has
    /// one address, the cylinders nest, and the measure concentrates
    /// — which is the condition cylinder targeting has always needed
    /// and `spherical.fflame` does not meet.
    ///
    /// Built from two generators and their inverses, four maps, in the
    /// classical form: `g(z) = (az + b)/(cz + d)` with `ad − bc = 1`
    /// and `|a + d| > 2`, which is loxodromic.
    fn loxodromic_flame() -> crate::scene::transforms::Flame {
        use crate::scene::transforms::{Flame, Transform};
        let mut f = Flame::new();
        f.transforms.clear();
        // **The Schottky condition is about the ISOMETRIC CIRCLES,
        // not about the traces.** For `g = (az+b)/(cz+d)` with
        // `ad − bc = 1`, `g` has its isometric circle at `−d/c` and
        // `g⁻¹` at `a/c`, both of radius `1/|c|`. The group is
        // Schottky when all four are mutually disjoint; then `g` maps
        // the outside of its circle onto the inside of its partner's,
        // the level-one pieces are disjoint, and the cylinders nest.
        //
        // A first attempt picked loxodromic matrices and checked only
        // the traces. Its circles were [−2,0], [1,3], [0,1], [−1,0] —
        // overlapping — and the regions oscillated exactly as the
        // involution flame's had.
        //
        // These put the four circles at ±2 and ±2i, all of radius 1,
        // pairwise 2.83 apart:
        //   g1 = [[2, 3], [1, 2]]      circles at −2 and 2
        //   g2 = [[2i, −5], [1, 2i]]   circles at −2i and 2i
        // both with determinant 1 and |trace| = 4, so both loxodromic.
        let gens: [[(f64, f64); 4]; 4] = [
            [(2.0, 0.0), (3.0, 0.0), (1.0, 0.0), (2.0, 0.0)],
            [(2.0, 0.0), (-3.0, 0.0), (-1.0, 0.0), (2.0, 0.0)],
            [(0.0, 2.0), (-5.0, 0.0), (1.0, 0.0), (0.0, 2.0)],
            [(0.0, 2.0), (5.0, 0.0), (-1.0, 0.0), (0.0, 2.0)],
        ];
        for [(ar, ai), (br, bi), (cr, ci), (dr, di)] in gens {
            let mut t = Transform::default();
            t.a = 1.0;
            t.d = 1.0;
            t.weight = 1.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("mobius", 1.0);
            for (k, v) in [
                ("re_a", ar),
                ("im_a", ai),
                ("re_b", br),
                ("im_b", bi),
                ("re_c", cr),
                ("im_c", ci),
                ("re_d", dr),
                ("im_d", di),
            ] {
                t.variation_params.insert(format!("mobius.{k}"), v as f32);
            }
            f.transforms.push(t);
        }
        f
    }

    #[test]
    #[ignore = "prints a measurement"]
    fn a_loxodromic_flame_is_the_fair_test() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let f = loxodromic_flame();

        // It has to READ as family M first.
        for (i, t) in f.transforms.iter().enumerate() {
            match crate::scene::mobius::detect(t, reg) {
                Some(m) => println!(
                    "  xform {i}: {:?}, pole {:?}",
                    m.kind,
                    m.pole().map(|p| [format!("{:.4}", p[0]), format!("{:.4}", p[1])])
                ),
                None => {
                    println!("  xform {i}: NOT read as family M");
                    return;
                }
            }
        }
        let Some(mf) = crate::scene::mobius::MobiusFlame::read(
            &f,
            reg,
            crate::scene::mobius::ROOT_SAMPLE,
        ) else {
            println!("  MobiusFlame::read declined");
            return;
        };
        println!(
            "  cover: {} discs, extent {:.4e}, leak {:.3e}",
            mf.root.discs.len(),
            mf.extent,
            mf.leak
        );

        // Do its regions SHRINK, where the involution flame's
        // oscillated?
        println!("  depth   region radius   discs");
        let mut cur = mf.empty_word();
        let mut syms: Vec<u32> = Vec::new();
        let mut st = 13u64;
        for k in 1..=14usize {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = ((st >> 33) as usize) % mf.maps.len();
            let Some(next) = mf.extend(&cur, j) else {
                println!("  {k:>5}   extend refused");
                break;
            };
            cur = next;
            syms.insert(0, j as u32);
            match mf.region(&cur, &syms) {
                Ok(c) => println!(
                    "  {k:>5}   {:>13.4e}   {}",
                    c.enclosing().map_or(f64::NAN, |d| d.r),
                    c.discs.len()
                ),
                Err(e) => {
                    println!("  {k:>5}   {e:?}");
                    break;
                }
            }
        }

        println!();
        let x = mf.root.points[mf.root.points.len() / 2];
        for zoom in [1e2f64, 1e4, 1e6] {
            let view = View::of(zoom, x, 512, 512);
            let t0 = std::time::Instant::now();
            let r = Cylinders::plan(&f, reg, view);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            match r {
                Ok(c) => println!(
                    "   zoom {zoom:>7.0e}  {ms:>7.1} ms  {:>4} words, depth {:>3}, \
                     speedup {:.3e}, lost {:.3e}",
                    c.words.len(),
                    c.depth,
                    c.speedup(),
                    c.lost
                ),
                Err(e) => println!("   zoom {zoom:>7.0e}  {ms:>7.1} ms  {e:?}"),
            }
        }
    }

    /// **What the map-keyed walk does on the Schottky flames.**
    ///
    /// The numbers that decide whether family M ships: how long a plan
    /// takes, how big the antichain is, how much measure is lost, and
    /// whether the speedup is worth the prefix.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn the_map_keyed_walk_on_the_schottky_flames() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["schottky1", "schottky2"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let Some(mf) = crate::scene::mobius::MobiusFlame::read(
                &cfg.flame,
                reg,
                crate::scene::mobius::ROOT_SAMPLE,
            ) else {
                println!("  {name}: not family M");
                continue;
            };
            let x = mf.root.points[mf.root.points.len() / 2];
            println!(
                "== {name}   extent {:.3e}, cover {} discs, leak {:.3e}, on-set [{:.4}, {:.4}]",
                mf.extent,
                mf.root.discs.len(),
                mf.leak,
                x[0],
                x[1]
            );
            for zoom in [1e1f64, 1e3, 1e5, 1e7, 1e9, 1e12] {
                let view = View::of(zoom, x, 512, 512);
                let t0 = std::time::Instant::now();
                let r = Cylinders::plan(&cfg.flame, reg, view);
                let ms = t0.elapsed().as_secs_f64() * 1e3;
                match r {
                    Ok(c) => println!(
                        "   zoom {zoom:>7.0e}  {ms:>8.1} ms  {:>5} words, depth {:>3}, \
                         mass {:.3e}, speedup {:.3e}, lost {:.3e}",
                        c.words.len(),
                        c.depth,
                        c.mass,
                        c.speedup(),
                        c.lost
                    ),
                    Err(e) => println!("   zoom {zoom:>7.0e}  {ms:>8.1} ms  {e:?}"),
                }
            }
            println!();
        }
    }

    /// **A cover for a flame whose maps are only BOUNDED, and what a
    /// push through it costs.**
    ///
    /// `grand-julian` has no invariant disc — `julian` with
    /// `dist = -1` is `|p|^(-1/2)`, unbounded at the origin, and the
    /// attractor spans `|p| ∈ [1.7e-1, 2.7e1]` so every disc holding
    /// it holds the pole. Family M met the same wall and answered it
    /// with a cover; `Cover::push_by` now takes any map that can push
    /// a point and a disc, and a `Bounder` can do both.
    ///
    /// The question this asks is not whether it works — it is what it
    /// COSTS. Family M could fold a word into one matrix; `julian` is
    /// not a group and there is nothing to fold, so a candidate costs
    /// `discs × depth` bound evaluations. That number decides whether
    /// family J is affordable at all, and it is better to know it now
    /// than after the enumeration is written around it.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_a_bounded_cover_costs_for_grand_julian() {
        use std::time::Instant;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["grand-julian", "julian-disc"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let bounders: Vec<crate::scene::ifs_ball::Bounder> = cfg
                .flame
                .transforms
                .iter()
                .filter(|t| t.weight > 0.0)
                .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
                .collect();
            if bounders.is_empty() {
                continue;
            }
            let arms: Vec<u32> = bounders.iter().map(|b| b.arms()).collect();
            let weights: Vec<f64> = cfg
                .flame
                .transforms
                .iter()
                .filter(|t| t.weight > 0.0)
                .map(|t| t.weight as f64)
                .collect();
            let total: f64 = weights.iter().sum();
            let alphabet: Vec<(usize, u32)> = (0..bounders.len())
                .flat_map(|i| (0..arms[i]).map(move |a| (i, a)))
                .collect();

            // A sampled orbit, run through the bounds as points.
            let mut st = 5u64;
            let mut lcg = move || {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((st >> 33) as f64) / ((1u64 << 31) as f64)
            };
            let mut p = [0.31f64, 0.17];
            let mut pts: Vec<[f64; 2]> = Vec::new();
            for k in 0..20000 {
                let mut u = lcg() * total;
                let mut j = weights.len() - 1;
                for (i, w) in weights.iter().enumerate() {
                    if u < *w {
                        j = i;
                        break;
                    }
                    u -= *w;
                }
                let a = (lcg() * arms[j] as f64) as u32 % arms[j].max(1);
                match bounders[j].apply_arm(Ball::new(p, 0.0), a) {
                    Ok(b) if b.c[0].is_finite() && b.c[1].is_finite() => p = b.c,
                    _ => {
                        p = [0.31, 0.17];
                        continue;
                    }
                }
                if k > 200 {
                    pts.push(p);
                }
            }
            if pts.len() < 1000 {
                println!("  {name}: orbit did not settle");
                continue;
            }

            // A disc is fine when every symbol can push it.
            use crate::scene::mobius::Disc;
            let pushes = |d: &Disc| -> bool {
                alphabet.iter().all(|&(i, a)| {
                    matches!(
                        bounders[i].apply_arm(Ball::new(d.c, d.r), a),
                        Ok(b) if b.r.is_finite() && b.r < 1e6
                    )
                })
            };
            let t0 = Instant::now();
            let built = crate::scene::mobius::cover_by_pushing(&pts, 256, 0.05, 400, pushes);
            let build_ms = t0.elapsed().as_secs_f64() * 1e3;
            let Some((cover, extent, leak)) = built else {
                println!("  {name}: no cover");
                continue;
            };
            println!(
                "== {name}: alphabet {}, extent {extent:.3e}, cover {} discs / {} points, \
                 leak {leak:.2e}, built in {build_ms:.0} ms",
                alphabet.len(),
                cover.discs.len(),
                cover.points.len()
            );

            // Push it along a word, and time one push.
            let rules = crate::scene::mobius::CoverRules::by_pushing(256, 4.0, f64::INFINITY);
            let mut cur = cover.clone();
            let mut collapse_at: Option<(usize, f64)> = None;
            let mut line = Vec::new();
            let t0 = Instant::now();
            let mut steps = 0usize;
            for k in 1..=24usize {
                let (i, a) = alphabet[(lcg() * alphabet.len() as f64) as usize
                    % alphabet.len()];
                let b = &bounders[i];
                let next = cur.push_by(
                    |q| b.apply_arm(Ball::new(q, 0.0), a).ok().map(|r| r.c)
                        .filter(|c| c[0].is_finite() && c[1].is_finite()),
                    |d| b.apply_arm(Ball::new(d.c, d.r), a).ok()
                        .filter(|r| r.r.is_finite())
                        .map(|r| Disc::new(r.c, r.r)),
                    |_| true,
                    &rules,
                );
                match next {
                    Ok(n) => {
                        cur = n;
                        steps += 1;
                        // **When does a single ball suffice again?**
                        // The cover exists only to get past the pole;
                        // once the whole image is somewhere every
                        // symbol can push, it can collapse to one disc
                        // and the walk costs one bound call a symbol
                        // instead of one per disc.
                        if collapse_at.is_none() {
                            if let Some(e) = cur.enclosing() {
                                if pushes(&e) {
                                    collapse_at = Some((k, e.r));
                                }
                            }
                        }
                        if k % 6 == 0 {
                            line.push(format!(
                                "{k}:{:.2e}/{}",
                                cur.enclosing().map_or(f64::NAN, |d| d.r),
                                cur.discs.len()
                            ));
                        }
                    }
                    Err(e) => {
                        line.push(format!("{k}:{e:?}"));
                        break;
                    }
                }
            }
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            println!(
                "   {} steps in {ms:.0} ms ({:.2} ms per push)  {}",
                steps,
                ms / steps.max(1) as f64,
                line.join("  ")
            );
            match collapse_at {
                Some((k, r)) => println!(
                    "   one ball suffices from step {k} (radius {r:.3e}) --                      after that a symbol costs ONE bound call, not {}",
                    cover.discs.len()
                ),
                None => println!("   the cover never collapsed to one ball"),
            }
            println!();
        }
    }

    /// **Symbol by symbol: does the root cover's image collapse to a
    /// usable ball, and if not, why not?**
    ///
    /// The whole cover-root idea rests on one claim — that a single
    /// symbol is enough to get clear of the pole, after which a ball
    /// suffices. The aggregate answer was no for `grand-julian`, and
    /// an aggregate no says nothing about whether the idea is wrong or
    /// three of twenty-five symbols are awkward. This prints the
    /// table.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn which_symbols_collapse_to_a_ball() {
        use crate::scene::mobius::{cover_by_pushing, CoverRules, Disc};
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["grand-julian", "julian-disc"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let bounders: Vec<_> = cfg
                .flame
                .transforms
                .iter()
                .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
                .collect();
            let weights: Vec<f64> =
                cfg.flame.transforms.iter().map(|t| (t.weight as f64).max(0.0)).collect();
            if bounders.len() != weights.len() {
                continue;
            }
            let mut alphabet: Vec<(u32, f64)> = Vec::new();
            for (i, w) in weights.iter().enumerate() {
                if !(*w > 0.0) {
                    continue;
                }
                let arms = bounders[i].arms().max(1);
                for a in 0..arms {
                    alphabet.push((sym_of(i as u32, a), w / arms as f64));
                }
            }

            // The same orbit the real thing samples.
            let total: f64 = weights.iter().sum();
            let mut st = 0x9E37_79B9_7F4A_7C15u64;
            let mut lcg = move || {
                st = st
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((st >> 33) as f64) / ((1u64 << 31) as f64)
            };
            let mut p = [0.31f64, 0.17];
            let mut pts: Vec<[f64; 2]> = Vec::new();
            for k in 0..ROOT_COVER_SAMPLE {
                let mut u = lcg() * total;
                let mut j = weights.len() - 1;
                for (i, w) in weights.iter().enumerate() {
                    if u < *w {
                        j = i;
                        break;
                    }
                    u -= *w;
                }
                let arms = bounders[j].arms().max(1);
                let a = ((lcg() * arms as f64) as u32).min(arms - 1);
                match bounders[j].apply_arm(Ball::new(p, 0.0), a) {
                    Ok(b) if b.c[0].is_finite() && b.c[1].is_finite() => p = b.c,
                    _ => {
                        p = [0.31, 0.17];
                        continue;
                    }
                }
                if k > ROOT_COVER_SAMPLE / 100 {
                    pts.push(p);
                }
            }
            let n = pts.len() as f64;
            let centre = [
                pts.iter().map(|q| q[0]).sum::<f64>() / n,
                pts.iter().map(|q| q[1]).sum::<f64>() / n,
            ];
            let extent = pts
                .iter()
                .map(|q| {
                    ((q[0] - centre[0]).powi(2) + (q[1] - centre[1]).powi(2)).sqrt()
                })
                .fold(0.0f64, f64::max);
            println!("== {name}: extent {extent:.3e}, alphabet {}", alphabet.len());
            let rmin = pts
                .iter()
                .map(|q| (q[0] * q[0] + q[1] * q[1]).sqrt())
                .fold(f64::INFINITY, f64::min);
            for (cap_frac, disc_cap) in [
                (1.0f64, 256usize),
                (0.3, 256),
                (0.1, 256),
                (0.03, 512),
                (0.01, 1024),
                (0.003, 2048),
            ] {
            let max_image_r = cap_frac * extent;
            let pushes = |d: &Disc| -> bool {
                alphabet.iter().all(|&(sym, _)| {
                    matches!(
                        bounders[sym_transform(sym) as usize]
                            .apply_arm(Ball::new(d.c, d.r), sym_arm(sym)),
                        Ok(b) if b.r <= max_image_r && b.c[0].is_finite()
                    )
                })
            };
            let t_build = std::time::Instant::now();
            let Some((cover, _, leak)) = cover_by_pushing(
                &pts,
                disc_cap,
                ROOT_COVER_START,
                ROOT_COVER_ANCHORS,
                pushes,
            ) else {
                println!("   cap {cap_frac:>6.3}: no cover");
                continue;
            };
            let build_ms = t_build.elapsed().as_secs_f64() * 1e3;
            let _ = rmin;

            let rules = CoverRules::by_pushing(disc_cap, 4.0, max_image_r);
            let mut good = 0usize;
            let mut lost_w = 0.0f64;
            let mut biggest = 0.0f64;
            let total_w: f64 = alphabet.iter().map(|x| x.1).sum();
            let t_push = std::time::Instant::now();
            for &(sym, _) in &alphabet {
                let b = &bounders[sym_transform(sym) as usize];
                let arm = sym_arm(sym);
                let t0 = std::time::Instant::now();
                let pushed = cover.push_by(
                    |q| {
                        b.apply_arm(Ball::new(q, 0.0), arm)
                            .ok()
                            .map(|r| r.c)
                            .filter(|c| c[0].is_finite() && c[1].is_finite())
                    },
                    |d| {
                        b.apply_arm(Ball::new(d.c, d.r), arm)
                            .ok()
                            .filter(|r| r.r.is_finite())
                            .map(|r| Disc::new(r.c, r.r))
                    },
                    |_| true,
                    &rules,
                );
                let _ = t0;
                let mut bad_w = 0.0f64;
                match pushed.ok().and_then(|c| c.enclosing()) {
                    Some(e) => {
                        let ball = Ball::new(e.c, e.r);
                        let walkable = e.r.is_finite()
                            && alphabet.iter().all(|&(s2, _)| {
                                matches!(
                                    bounders[sym_transform(s2) as usize]
                                        .apply_arm(ball, sym_arm(s2)),
                                    Ok(r) if r.r.is_finite()
                                )
                            });
                        if walkable {
                            good += 1;
                            biggest = biggest.max(e.r / extent);
                        } else {
                            bad_w = 1.0;
                        }
                    }
                    None => bad_w = 1.0,
                }
                if bad_w > 0.0 {
                    lost_w += alphabet.iter().find(|x| x.0 == sym).map_or(0.0, |x| x.1);
                }
            }
            let push_ms = t_push.elapsed().as_secs_f64() * 1e3;
            println!(
                "   cap {cap_frac:>6.3}: {:>4} discs, leak {leak:.2e}, build {build_ms:>5.0} ms, \
                 {} of {} walkable (measure lost {:.3}), biggest {:.2e} of extent, \
                 push {push_ms:>5.0} ms",
                cover.discs.len(),
                good,
                alphabet.len(),
                lost_w / total_w,
                biggest
            );
            }
            println!();
        }
    }

    /// **How loose is the family-J bound against the TRUE cylinder,
    /// per step?**
    ///
    /// The question that decides whether this can reach a deep view.
    /// A bound that over-estimates by a constant factor per step
    /// compounds: at 2x a step, a depth-20 word is a million times
    /// too big, and a view that needs depth 20 is unreachable however
    /// cleverly the walk is run. The truth is available cheaply —
    /// push the SAME sampled orbit through the same word as points
    /// and measure its spread — so the ratio can be read off directly
    /// rather than inferred from where the enumeration gives up.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn how_loose_is_the_bag_against_the_truth() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame")
            .expect("output/flame-zoom/grand-julian.fflame");
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("a config");
        let bounders: Vec<_> = cfg
            .flame
            .transforms
            .iter()
            .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
            .collect();
        let weights: Vec<f64> =
            cfg.flame.transforms.iter().map(|t| (t.weight as f64).max(0.0)).collect();
        assert_eq!(bounders.len(), weights.len());
        let mut alphabet: Vec<(u32, f64)> = Vec::new();
        for (i, w) in weights.iter().enumerate() {
            let arms = bounders[i].arms().max(1);
            for a in 0..arms {
                alphabet.push((sym_of(i as u32, a), w / arms as f64));
            }
        }
        let (_, images, _) =
            bounded_root_images(&bounders, &weights, &alphabet).expect("a root");

        // The attractor, sampled the same way the root was.
        let total: f64 = weights.iter().sum();
        let mut st = 0x9E37_79B9_7F4A_7C15u64;
        let mut lcg = move || {
            st = st
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut p = [0.31f64, 0.17];
        let mut pts: Vec<[f64; 2]> = Vec::new();
        for k in 0..6000 {
            let mut u = lcg() * total;
            let mut j = weights.len() - 1;
            for (i, w) in weights.iter().enumerate() {
                if u < *w {
                    j = i;
                    break;
                }
                u -= *w;
            }
            let arms = bounders[j].arms().max(1);
            let a = ((lcg() * arms as f64) as u32).min(arms - 1);
            match bounders[j].apply_arm(Ball::new(p, 0.0), a) {
                Ok(b) if b.c[0].is_finite() && b.c[1].is_finite() => p = b.c,
                _ => {
                    p = [0.31, 0.17];
                    continue;
                }
            }
            if k > 1000 {
                pts.push(p);
            }
        }
        let spread = |q: &[[f64; 2]]| -> f64 {
            let n = q.len() as f64;
            let c = [
                q.iter().map(|x| x[0]).sum::<f64>() / n,
                q.iter().map(|x| x[1]).sum::<f64>() / n,
            ];
            q.iter()
                .map(|x| ((x[0] - c[0]).powi(2) + (x[1] - c[1]).powi(2)).sqrt())
                .fold(0.0, f64::max)
        };
        let extent = spread(&pts);
        let first_arm: Vec<u32> = {
            let mut seen = Vec::new();
            let mut out = Vec::new();
            for &(sym, _) in &alphabet {
                let t = sym_transform(sym);
                if !seen.contains(&t) {
                    seen.push(t);
                    out.push(sym);
                }
            }
            out
        };
        let clears = |b: &Ball| -> bool {
            first_arm.iter().all(|&sym| {
                matches!(
                    bounders[sym_transform(sym) as usize].apply_arm(*b, sym_arm(sym)),
                    Ok(r) if r.r.is_finite() && r.c[0].is_finite()
                )
            })
        };

        println!("grand-julian: extent {extent:.3e}; per depth: bound radius / true radius");
        // Words are applied first-symbol-first, exactly as `bag_of`
        // and the chaos game do.
        let mut ratios: Vec<Vec<f64>> = vec![Vec::new(); 21];
        for wi in 0..12 {
            let n = alphabet.len();
            let word: Vec<u32> =
                (0..20).map(|_| alphabet[((lcg() * n as f64) as usize).min(n - 1)].0).collect();
            let mut bag = images[&word[0]].clone();
            let mut truth: Vec<[f64; 2]> = pts.iter().step_by(4).copied().collect();
            let b0 = &bounders[sym_transform(word[0]) as usize];
            truth = truth
                .iter()
                .filter_map(|q| b0.apply_arm(Ball::new(*q, 0.0), sym_arm(word[0])).ok())
                .filter(|r| r.c[0].is_finite() && r.c[1].is_finite())
                .map(|r| r.c)
                .collect();
            let mut line = Vec::new();
            for (k, &sym) in word.iter().enumerate() {
                if k > 0 {
                    let b = &bounders[sym_transform(sym) as usize];
                    let arm = sym_arm(sym);
                    let mut next = Vec::with_capacity(bag.len());
                    for piece in &bag {
                        if let Ok(img) = b.apply_arm(*piece, arm) {
                            if img.r.is_finite() && img.c[0].is_finite() {
                                next.push(img);
                            }
                        }
                    }
                    if next.is_empty() {
                        line.push(format!("{k}:dead"));
                        break;
                    }
                    bag = next;
                    if bag.len() > 1 {
                        if let Some(one) = enclosing_ball(&bag).filter(&clears) {
                            bag = vec![one];
                        }
                    }
                    truth = truth
                        .iter()
                        .filter_map(|q| b.apply_arm(Ball::new(*q, 0.0), arm).ok())
                        .filter(|r| r.c[0].is_finite() && r.c[1].is_finite())
                        .map(|r| r.c)
                        .collect();
                }
                if truth.len() < 8 {
                    line.push(format!("{k}:truth-lost"));
                    break;
                }
                let br = enclosing_ball(&bag).map_or(f64::NAN, |b| b.r);
                let tr = spread(&truth);
                let ratio = br / tr.max(1e-300);
                ratios[k].push(ratio);
                if k % 2 == 1 || k == 0 {
                    line.push(format!(
                        "{k}:{:.0e}/{:.0e}={:.0e}{}",
                        br,
                        tr,
                        ratio,
                        if bag.len() > 1 { format!("[{}]", bag.len()) } else { String::new() }
                    ));
                }
            }
            println!("  word {wi:>2}: {}", line.join(" "));
        }
        println!();
        println!("  depth  median(bound/true)  n");
        for (k, r) in ratios.iter().enumerate() {
            if r.is_empty() {
                continue;
            }
            let mut v = r.clone();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!("  {k:>5}  {:>17.2e}  {}", v[v.len() / 2], v.len());
        }
    }

    /// **Is the deep-view failure a cover gap?**
    ///
    /// `grand_julian_enumerates_with_arms` centres its view on a point
    /// reached by a fixed cyclic orbit — legitimate, but not one the
    /// root cover was built from. If that point sits in a gap between
    /// the cover's discs, a 1e2 view still meets some bag by sheer
    /// size and a 1e4 view meets none, which is exactly the pattern
    /// measured. Two checks: does any root piece contain that point,
    /// and does a view centred on a point the cover WAS built from
    /// plan at the depths the other one could not.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn is_the_deep_view_failure_a_cover_gap() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame")
            .expect("output/flame-zoom/grand-julian.fflame");
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("a config");
        let bounders: Vec<_> = cfg
            .flame
            .transforms
            .iter()
            .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
            .collect();
        let weights: Vec<f64> =
            cfg.flame.transforms.iter().map(|t| (t.weight as f64).max(0.0)).collect();
        let arms: Vec<u32> = bounders.iter().map(|b| b.arms()).collect();
        let mut alphabet: Vec<(u32, f64)> = Vec::new();
        for (i, w) in weights.iter().enumerate() {
            for a in 0..arms[i].max(1) {
                alphabet.push((sym_of(i as u32, a), w / arms[i].max(1) as f64));
            }
        }
        let (_, images, _) =
            bounded_root_images(&bounders, &weights, &alphabet).expect("a root");

        // The other test's point, and the symbol that produced it.
        let mut p = [0.31f64, 0.17];
        let mut last = 0u32;
        for k in 0..64 {
            let j = k % bounders.len();
            let a = (k as u32) % arms[j].max(1);
            match bounders[j].apply_arm(Ball::new(p, 0.0), a) {
                Ok(b) if b.c[0].is_finite() && b.c[1].is_finite() => {
                    p = b.c;
                    last = sym_of(j as u32, a);
                }
                _ => break,
            }
        }
        let inside = |q: [f64; 2]| -> Vec<u32> {
            let mut v: Vec<u32> = images
                .iter()
                .filter(|(_, bag)| {
                    bag.iter().any(|b| {
                        ((q[0] - b.c[0]).powi(2) + (q[1] - b.c[1]).powi(2)).sqrt() <= b.r
                    })
                })
                .map(|(s, _)| *s)
                .collect();
            v.sort();
            v
        };
        println!(
            "cyclic-orbit point [{:.4}, {:.4}], last symbol {last}: inside root bags {:?}",
            p[0],
            p[1],
            inside(p)
        );

        // A point the cover was built from: the same orbit the root
        // samples, taken a long way in.
        let total: f64 = weights.iter().sum();
        let mut st = 0x9E37_79B9_7F4A_7C15u64;
        let mut lcg = move || {
            st = st
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut q = [0.31f64, 0.17];
        let mut qlast = 0u32;
        for _ in 0..5000 {
            let mut u = lcg() * total;
            let mut j = weights.len() - 1;
            for (i, w) in weights.iter().enumerate() {
                if u < *w {
                    j = i;
                    break;
                }
                u -= *w;
            }
            let a = ((lcg() * arms[j] as f64) as u32).min(arms[j] - 1);
            if let Ok(b) = bounders[j].apply_arm(Ball::new(q, 0.0), a) {
                if b.c[0].is_finite() && b.c[1].is_finite() {
                    q = b.c;
                    qlast = sym_of(j as u32, a);
                }
            }
        }
        println!(
            "sampled-orbit point [{:.4}, {:.4}], last symbol {qlast}: inside root bags {:?}",
            q[0],
            q[1],
            inside(q)
        );

        for (label, centre) in [("cyclic", p), ("sampled", q)] {
            for zoom in [1e2f64, 1e3, 1e4, 1e5, 1e6] {
                let view = View::of(zoom, centre, 512, 512);
                let t0 = std::time::Instant::now();
                let r = Cylinders::plan_armed(&cfg.flame, reg, view);
                let ms = t0.elapsed().as_secs_f64() * 1e3;
                match r {
                    Ok(c) => println!(
                        "  {label:<8} zoom {zoom:>6.0e} {ms:>8.0} ms  {:>5} words, depth {:>2}, \
                         mass {:.2e}, speedup {:.2e}, lost {:.2e}",
                        c.words.len(),
                        c.depth,
                        c.mass,
                        c.speedup(),
                        c.lost
                    ),
                    Err(e) => println!("  {label:<8} zoom {zoom:>6.0e} {ms:>8.0} ms  {e:?}"),
                }
            }
        }
    }

    /// **Follow the one word known to contain the view point through
    /// the walk, and report where it goes missing.**
    ///
    /// A sampled orbit's last `k` symbols are a word whose cylinder
    /// contains the orbit's current point — not by a bound, by
    /// construction. Centre the view there, run the same expansion
    /// `plan_inner` runs with the beam removed, and at every depth
    /// ask: is that word still on the frontier? If it is dropped, the
    /// reason is one of three, and they point at completely different
    /// fixes: its bag stopped meeting the view (the bound is UNSOUND,
    /// since the truth is inside it), its bag died (every piece hit a
    /// pole), or the frontier grew past what any beam would keep.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn where_does_the_true_word_go_missing() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame")
            .expect("output/flame-zoom/grand-julian.fflame");
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("a config");
        let bounders: Vec<_> = cfg
            .flame
            .transforms
            .iter()
            .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
            .collect();
        let weights: Vec<f64> =
            cfg.flame.transforms.iter().map(|t| (t.weight as f64).max(0.0)).collect();
        let arms: Vec<u32> = bounders.iter().map(|b| b.arms()).collect();
        let mut alphabet: Vec<(u32, f64)> = Vec::new();
        for (i, w) in weights.iter().enumerate() {
            for a in 0..arms[i].max(1) {
                alphabet.push((sym_of(i as u32, a), w / arms[i].max(1) as f64));
            }
        }
        let total_w: f64 = weights.iter().sum();
        let (root, images, _) =
            bounded_root_images(&bounders, &weights, &alphabet).expect("a root");

        // The orbit, with its symbol history.
        let mut st = 0x9E37_79B9_7F4A_7C15u64;
        let mut lcg = move || {
            st = st
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut q = [0.31f64, 0.17];
        let mut history: Vec<u32> = Vec::new();
        for _ in 0..5000 {
            let mut u = lcg() * total_w;
            let mut j = weights.len() - 1;
            for (i, w) in weights.iter().enumerate() {
                if u < *w {
                    j = i;
                    break;
                }
                u -= *w;
            }
            let a = ((lcg() * arms[j] as f64) as u32).min(arms[j] - 1);
            if let Ok(b) = bounders[j].apply_arm(Ball::new(q, 0.0), a) {
                if b.c[0].is_finite() && b.c[1].is_finite() {
                    q = b.c;
                    history.push(sym_of(j as u32, a));
                }
            }
        }
        // The true word at depth k, in application order, is the last
        // k symbols of the history.
        let true_word = |k: usize| -> Vec<u32> { history[history.len() - k..].to_vec() };

        let first_arm: Vec<u32> = {
            let mut seen = Vec::new();
            let mut out = Vec::new();
            for &(sym, _) in &alphabet {
                let t = sym_transform(sym);
                if !seen.contains(&t) {
                    seen.push(t);
                    out.push(sym);
                }
            }
            out
        };
        let clears = |b: &Ball| -> bool {
            first_arm.iter().all(|&sym| {
                matches!(
                    bounders[sym_transform(sym) as usize].apply_arm(*b, sym_arm(sym)),
                    Ok(r) if r.r.is_finite() && r.c[0].is_finite()
                )
            })
        };
        let bag_of = |word: &[u32]| -> Option<Vec<Ball>> {
            let mut rest = word.iter();
            let mut bag = images.get(rest.next()?)?.clone();
            for &sym in rest {
                let b = &bounders[sym_transform(sym) as usize];
                let arm = sym_arm(sym);
                let mut next = Vec::with_capacity(bag.len());
                for piece in &bag {
                    if let Ok(img) = b.apply_arm(*piece, arm) {
                        if img.r.is_finite() && img.c[0].is_finite() {
                            next.push(img);
                        }
                    }
                }
                if next.is_empty() {
                    return None;
                }
                bag = next;
                if bag.len() > 1 {
                    if let Some(one) = enclosing_ball(&bag).filter(&clears) {
                        bag = vec![one];
                    }
                }
            }
            Some(bag)
        };

        for zoom in [1e3f64, 1e4] {
            let view = View::of(zoom, q, 512, 512);
            println!(
                "== zoom {zoom:.0e}: view radius {:.3e} at [{:.4}, {:.4}]",
                view.radius, q[0], q[1]
            );
            let hits = |c: [f64; 2], r: f64| {
                ((c[0] - view.centre[0]).powi(2) + (c[1] - view.centre[1]).powi(2)).sqrt()
                    <= r + view.radius
            };
            // (word, prob)
            let mut frontier: Vec<(Vec<u32>, f64)> = vec![(Vec::new(), 1.0)];
            let mut kept = 0usize;
            let mut kept_mass = 0.0f64;
            let mut lost_died = 0.0f64;
            let _ = root;
            for depth in 1..=14usize {
                let tw = true_word(depth);
                let mut next: Vec<(Vec<u32>, f64)> = Vec::new();
                let (mut died, mut missed, mut cut) = (0usize, 0usize, 0usize);
                let mut true_fate = "not expanded (parent gone)";
                let mut true_detail = String::new();
                for (pw, pp) in &frontier {
                    for &(sym, w) in &alphabet {
                        let mut word = Vec::with_capacity(pw.len() + 1);
                        word.push(sym);
                        word.extend_from_slice(pw);
                        let prob = pp * (w / total_w);
                        let is_true = word == tw;
                        match bag_of(&word) {
                            None => {
                                died += 1;
                                lost_died += prob;
                                if is_true {
                                    true_fate = "DIED (every piece hit a pole)";
                                }
                            }
                            Some(bag) => {
                                let e = enclosing_ball(&bag).unwrap();
                                let meets = bag.iter().any(|p| hits(p.c, p.r));
                                if is_true {
                                    let dq = bag
                                        .iter()
                                        .map(|b| {
                                            ((q[0] - b.c[0]).powi(2) + (q[1] - b.c[1]).powi(2))
                                                .sqrt()
                                                - b.r
                                        })
                                        .fold(f64::INFINITY, f64::min);
                                    true_detail = format!(
                                        "bag {} pieces, enclosing {:.2e}, nearest piece edge to q {:+.2e}",
                                        bag.len(),
                                        e.r,
                                        dq
                                    );
                                }
                                if !meets {
                                    missed += 1;
                                    if is_true {
                                        true_fate = "MISSED the view (bound unsound)";
                                    }
                                } else if e.r <= view.radius {
                                    cut += 1;
                                    kept += 1;
                                    kept_mass += prob;
                                    if is_true {
                                        true_fate = "CUT (kept, fits the view)";
                                    }
                                } else {
                                    next.push((word, prob));
                                    if is_true {
                                        true_fate = "on the frontier";
                                    }
                                }
                            }
                        }
                    }
                }
                println!(
                    "  depth {depth:>2}: frontier in {:>6}, died {died:>6}, missed {missed:>6}, \
                     cut {cut:>5}, frontier out {:>6} | kept {kept} mass {kept_mass:.2e} | \
                     true word: {true_fate}  {true_detail}",
                    frontier.len(),
                    next.len()
                );
                if next.len() > 20000 {
                    println!("  (frontier past 20000, stopping)");
                    break;
                }
                frontier = next;
                if frontier.is_empty() {
                    break;
                }
            }
            println!("  lost to dead bags {lost_died:.2e}");
        }
    }

    /// **Does the measure concentrate around a deep point of
    /// `grand-julian`? Asked exactly, by walking the inverse maps.**
    ///
    /// Forward bounds cannot answer this here — they are too loose to
    /// separate "many words reach this view" from "many bounds do".
    /// But every map in this flame is `julian ∘ rotation`, whose
    /// inverse is single-valued: given an OUTPUT point, only one arm
    /// of each transform can have produced it, because the arms'
    /// images are disjoint sectors. So the words whose cylinder
    /// contains a point form a tree of branching at most three (one
    /// per transform), not twenty-five, and it can be walked
    /// backwards from the point with exact arithmetic and no bound at
    /// all. The only approximation is "the pre-image lies on the
    /// attractor", tested against a sample at three tolerances.
    ///
    /// Reports, per depth, how many words contain the point and the
    /// measure they carry; then, cutting each path where its own
    /// derivative product says the cylinder fits the view, the
    /// antichain size and mass at each zoom. The derivative is taken
    /// at one point, so the cut depth is optimistic; the counts at a
    /// fixed depth are exact.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn does_the_measure_concentrate_backwards() {
        let text = std::fs::read_to_string("output/flame-zoom/grand-julian.fflame")
            .expect("output/flame-zoom/grand-julian.fflame");
        let cfg: crate::config::FractalConfig = serde_json::from_str(&text).expect("a config");
        struct Map {
            aff: [f64; 6],
            inv: [f64; 6],
            wj: f64,
            power: f64,
            dist: f64,
            weight: f64,
        }
        let maps: Vec<Map> = cfg
            .flame
            .transforms
            .iter()
            .map(|t| {
                let (a, b, c, d, e, f) = (
                    t.a as f64, t.b as f64, t.c as f64, t.d as f64, t.e as f64, t.f as f64,
                );
                let det = a * d - b * c;
                Map {
                    aff: [a, b, c, d, e, f],
                    inv: [
                        d / det,
                        -b / det,
                        -c / det,
                        a / det,
                        -(d * e - b * f) / det,
                        -(a * f - c * e) / det,
                    ],
                    wj: *t.variations.get("julian").unwrap_or(&0.0) as f64,
                    power: *t.variation_params.get("julian.power").unwrap_or(&1.0) as f64,
                    dist: *t.variation_params.get("julian.dist").unwrap_or(&1.0) as f64,
                    weight: t.weight as f64,
                }
            })
            .collect();
        let total_w: f64 = maps.iter().map(|m| m.weight).sum();
        let fwd = |m: &Map, x: [f64; 2], arm: u32| -> [f64; 2] {
            let w = [
                m.aff[0] * x[0] + m.aff[1] * x[1] + m.aff[4],
                m.aff[2] * x[0] + m.aff[3] * x[1] + m.aff[5],
            ];
            let r = (w[0] * w[0] + w[1] * w[1]).powf(m.dist / (2.0 * m.power));
            let t = (w[1].atan2(w[0]) + 2.0 * std::f64::consts::PI * arm as f64) / m.power;
            [m.wj * r * t.cos(), m.wj * r * t.sin()]
        };
        // Inverse: the arm is DETERMINED by the output's angle.
        let inv = |m: &Map, z: [f64; 2]| -> Option<([f64; 2], u32, f64)> {
            let rho = (z[0] * z[0] + z[1] * z[1]).sqrt() / m.wj;
            if !(rho > 0.0) || !rho.is_finite() {
                return None;
            }
            let phi = z[1].atan2(z[0]);
            let n = m.power;
            let two_pi = 2.0 * std::f64::consts::PI;
            let mut a = (n * phi / two_pi).round();
            a = a.rem_euclid(n);
            let mut theta = n * phi - two_pi * a;
            theta = (theta + std::f64::consts::PI).rem_euclid(two_pi) - std::f64::consts::PI;
            let rw = rho.powf(n / m.dist);
            let w = [rw * theta.cos(), rw * theta.sin()];
            let x = [
                m.inv[0] * w[0] + m.inv[1] * w[1] + m.inv[4],
                m.inv[2] * w[0] + m.inv[3] * w[1] + m.inv[5],
            ];
            // |S'| at x: rotation is conformal with scale 1, then
            // wj · |α| · |w|^(α−1).
            let alpha = m.dist / m.power;
            let deriv = m.wj * alpha.abs() * rw.powf(alpha - 1.0);
            Some((x, a as u32, deriv))
        };

        // The attractor, sampled; and a point on it with history.
        let mut st = 0x9E37_79B9_7F4A_7C15u64;
        let mut lcg = move || {
            st = st
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut x = [0.31f64, 0.17];
        let mut pts: Vec<[f64; 2]> = Vec::new();
        for k in 0..200_000 {
            let mut u = lcg() * total_w;
            let mut j = maps.len() - 1;
            for (i, m) in maps.iter().enumerate() {
                if u < m.weight {
                    j = i;
                    break;
                }
                u -= m.weight;
            }
            let arm = ((lcg() * maps[j].power) as u32).min(maps[j].power as u32 - 1);
            let y = fwd(&maps[j], x, arm);
            if y[0].is_finite() && y[1].is_finite() {
                x = y;
            } else {
                x = [0.31, 0.17];
                continue;
            }
            if k > 1000 {
                pts.push(x);
            }
        }
        let q = x;
        let n = pts.len() as f64;
        let centre = [
            pts.iter().map(|p| p[0]).sum::<f64>() / n,
            pts.iter().map(|p| p[1]).sum::<f64>() / n,
        ];
        let extent = pts
            .iter()
            .map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2)).sqrt())
            .fold(0.0, f64::max);
        // Sanity: the inverse really inverts the forward map.
        {
            let m = &maps[0];
            let y = fwd(m, [0.4, -0.2], 1);
            let (back, arm, _) = inv(m, y).unwrap();
            assert!((back[0] - 0.4).abs() < 1e-9 && (back[1] + 0.2).abs() < 1e-9, "{back:?}");
            assert_eq!(arm, 1);
        }
        // A grid over the sample, for the on-attractor test.
        let cell = 0.02f64;
        let mut grid: std::collections::HashMap<(i64, i64), Vec<[f64; 2]>> =
            std::collections::HashMap::new();
        for p in &pts {
            grid.entry(((p[0] / cell).floor() as i64, ((p[1] / cell).floor()) as i64))
                .or_default()
                .push(*p);
        }
        let near = |p: [f64; 2], delta: f64| -> bool {
            let reach = (delta / cell).ceil() as i64;
            let (cx, cy) = ((p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64);
            for dx in -reach..=reach {
                for dy in -reach..=reach {
                    if let Some(v) = grid.get(&(cx + dx, cy + dy)) {
                        if v.iter().any(|s| {
                            ((s[0] - p[0]).powi(2) + (s[1] - p[1]).powi(2)).sqrt() <= delta
                        }) {
                            return true;
                        }
                    }
                }
            }
            false
        };
        println!(
            "grand-julian: {} sample points, extent {extent:.3e}, q = [{:.4}, {:.4}]",
            pts.len(),
            q[0],
            q[1]
        );
        // Cross-check the hand-written map against the real bounder's
        // point push, arm by arm.
        {
            let guard = crate::variations::global_registry();
            let reg = &*guard;
            let mut worst = 0.0f64;
            for (i, t) in cfg.flame.transforms.iter().enumerate() {
                let b = crate::scene::ifs_ball::Bounder::new(t, reg).expect("bounder");
                for arm in 0..maps[i].power as u32 {
                    for x in [[0.4, -0.2], [-1.3, 0.7], [0.05, 0.9], [2.0, 2.0]] {
                        let mine = fwd(&maps[i], x, arm);
                        let theirs = b.apply_arm(Ball::new(x, 0.0), arm).expect("push").c;
                        let d = ((mine[0] - theirs[0]).powi(2) + (mine[1] - theirs[1]).powi(2))
                            .sqrt();
                        worst = worst.max(d);
                    }
                }
            }
            println!("hand-written map vs bounder point push: worst disagreement {worst:.2e}");
            assert!(worst < 1e-5, "the hand-written map is not the flame's");
        }
        // Why only one pre-image? Show all three at the first depths.
        {
            let nearest = |p: [f64; 2]| -> f64 {
                pts.iter()
                    .map(|s| ((s[0] - p[0]).powi(2) + (s[1] - p[1]).powi(2)).sqrt())
                    .fold(f64::INFINITY, f64::min)
            };
            let mut p = q;
            for depth in 1..=4 {
                let mut line = Vec::new();
                let mut chosen = None;
                for (i, m) in maps.iter().enumerate() {
                    match inv(m, p) {
                        Some((pre, arm, _)) => {
                            let d = nearest(pre);
                            line.push(format!(
                                "t{i} arm {arm}: pre [{:+.3}, {:+.3}] |pre| {:.3} nearest-sample {:.2e}",
                                pre[0],
                                pre[1],
                                (pre[0] * pre[0] + pre[1] * pre[1]).sqrt(),
                                d
                            ));
                            if d < 0.03 && chosen.is_none() {
                                chosen = Some(pre);
                            }
                        }
                        None => line.push(format!("t{i}: no inverse")),
                    }
                }
                println!("  depth {depth}: {}", line.join(" | "));
                match chosen {
                    Some(c) => p = c,
                    None => break,
                }
            }
        }

        // The same walk from several other points of the orbit, so the
        // answer is about the flame and not about one point.
        {
            let delta = 0.03f64;
            let mut st2 = 12345u64;
            let mut lcg2 = move || {
                st2 = st2
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((st2 >> 33) as f64) / ((1u64 << 31) as f64)
            };
            println!("== ten other orbit points, tolerance {delta}: max words at any depth <= 20, and depth where the derivative product first falls below 1e-6");
            for _ in 0..10 {
                let p0 = pts[(lcg2() * pts.len() as f64) as usize % pts.len()];
                let mut level: Vec<([f64; 2], f64)> = vec![(p0, 1.0)];
                let mut widest = 0usize;
                let mut cut_depth = None;
                for depth in 1..=20usize {
                    let mut next = Vec::new();
                    for &(p, d) in &level {
                        for m in &maps {
                            if let Some((pre, _, deriv)) = inv(m, p) {
                                if pre[0].is_finite() && near(pre, delta) {
                                    next.push((pre, d * deriv));
                                }
                            }
                        }
                    }
                    if next.is_empty() {
                        break;
                    }
                    widest = widest.max(next.len());
                    if cut_depth.is_none() && next.iter().all(|x| x.1 < 1e-6) {
                        cut_depth = Some(depth);
                    }
                    level = next;
                }
                println!(
                    "  [{:+.3}, {:+.3}] |p| {:.3}: widest level {widest}, all paths below 1e-6 by depth {:?}",
                    p0[0],
                    p0[1],
                    (p0[0] * p0[0] + p0[1] * p0[1]).sqrt(),
                    cut_depth
                );
            }
        }
        for delta in [0.01f64, 0.03, 0.1] {
            println!("== on-attractor tolerance {delta}");
            // (pre-image, prob, derivative product)
            let mut level: Vec<([f64; 2], f64, f64)> = vec![(q, 1.0, 1.0)];
            let mut cut: Vec<(usize, f64, f64)> = Vec::new(); // (depth, prob, D)
            let zooms = [1e3f64, 1e4, 1e6];
            let mut antichain: Vec<(usize, f64)> = vec![(0, 0.0); zooms.len()];
            let mut open: Vec<Vec<([f64; 2], f64, f64)>> = vec![level.clone(); zooms.len()];
            let _ = &mut cut;
            println!("  depth  words   mass        D range");
            for depth in 1..=24usize {
                let mut next = Vec::new();
                for &(p, prob, d) in &level {
                    for m in &maps {
                        if let Some((pre, _arm, deriv)) = inv(m, p) {
                            if pre[0].is_finite() && near(pre, delta) {
                                let pw = prob * (m.weight / total_w) / m.power;
                                next.push((pre, pw, d * deriv));
                            }
                        }
                    }
                }
                if next.is_empty() {
                    println!("  {depth:>5}  (no pre-images on the attractor)");
                    break;
                }
                let mass: f64 = next.iter().map(|x| x.1).sum();
                let dmin = next.iter().map(|x| x.2).fold(f64::INFINITY, f64::min);
                let dmax = next.iter().map(|x| x.2).fold(0.0, f64::max);
                println!(
                    "  {depth:>5}  {:>6}  {mass:.3e}  {:.1e}..{:.1e}",
                    next.len(),
                    dmin,
                    dmax
                );
                // Antichains: cut a path when its cylinder estimate fits.
                for (zi, &zoom) in zooms.iter().enumerate() {
                    let view_r = extent / zoom;
                    let mut still = Vec::new();
                    let prev = std::mem::take(&mut open[zi]);
                    for &(p, prob, d) in &prev {
                        for m in &maps {
                            if let Some((pre, _arm, deriv)) = inv(m, p) {
                                if pre[0].is_finite() && near(pre, delta) {
                                    let pw = prob * (m.weight / total_w) / m.power;
                                    let dd = d * deriv;
                                    if dd * extent <= view_r {
                                        antichain[zi].0 += 1;
                                        antichain[zi].1 += pw;
                                    } else {
                                        still.push((pre, pw, dd));
                                    }
                                }
                            }
                        }
                    }
                    open[zi] = still;
                }
                if next.len() > 200_000 {
                    println!("  (past 200000 words, stopping)");
                    break;
                }
                level = next;
            }
            for (zi, &zoom) in zooms.iter().enumerate() {
                println!(
                    "  zoom {zoom:.0e}: antichain {} words, mass {:.3e}, still open {}",
                    antichain[zi].0,
                    antichain[zi].1,
                    open[zi].len()
                );
            }
        }
    }

    /// **Does the inverse-walk analysis accept the family-J flames?**
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn does_analyse_2d_accept_family_j() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["grand-julian", "julian-disc", "spherical", "random1"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let t0 = std::time::Instant::now();
            match crate::scene::ifs_analysis::analyse_2d(&cfg.flame, reg) {
                Ok(ifs) => {
                    let mut per: Vec<(usize, u32)> = Vec::new();
                    for m in &ifs.maps {
                        let b = m.forward.nonlinear().map_or(0, |n| n.branch);
                        per.push((m.transform_index, b));
                    }
                    println!(
                        "{name}: OK in {:.1} ms — {} maps (transform, branch) {:?}, ball {:.3e} at [{:.3}, {:.3}], xaos {}",
                        t0.elapsed().as_secs_f64() * 1e3,
                        ifs.maps.len(),
                        per,
                        ifs.ball.radius,
                        ifs.ball.centre[0],
                        ifs.ball.centre[1],
                        ifs.xaos.is_some()
                    );
                }
                Err(errs) => println!("{name}: refused — {errs:?}"),
            }
        }
    }

    /// **What the root cover says about each flame that has no
    /// invariant disc.**
    ///
    /// [`bounded_root_images`] has five ways to give up and each one
    /// is a measurement: the orbit not settling, the cover leaking,
    /// a symbol unable to push the cover, an image that collapses to
    /// nothing smaller than the attractor, and an image ball that
    /// cannot itself be pushed. Which one fires tells you whether the
    /// flame is unreachable or the constants are wrong, and those are
    /// completely different problems.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn what_the_root_cover_says() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["grand-julian", "julian-disc", "spherical", "random1"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let mut bounders = Vec::new();
            let mut weights = Vec::new();
            let mut ok = true;
            for t in &cfg.flame.transforms {
                match crate::scene::ifs_ball::Bounder::new(t, reg) {
                    Ok(b) => {
                        bounders.push(b);
                        weights.push((t.weight as f64).max(0.0));
                    }
                    Err(e) => {
                        println!("== {name}: no bounder: {e}");
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            let mut alphabet: Vec<(u32, f64)> = Vec::new();
            for (i, w) in weights.iter().enumerate() {
                if !(*w > 0.0) {
                    continue;
                }
                let arms = bounders[i].arms().max(1);
                for a in 0..arms {
                    alphabet.push((sym_of(i as u32, a), w / arms as f64));
                }
            }
            let t0 = std::time::Instant::now();
            let r = bounded_root_images(&bounders, &weights, &alphabet);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            match r {
                Ok((root, images, leak)) => {
                    let mut rs: Vec<f64> = images
                        .values()
                        .filter_map(|bag| enclosing_ball(bag).map(|b| b.r))
                        .collect();
                    rs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let pieces: usize = images.values().map(|b| b.len()).sum();
                    println!(
                        "== {name}: OK in {ms:.0} ms — root {:.3e} at [{:.3}, {:.3}], \
                         {} images in {pieces} pieces, radii {:.3e}..{:.3e}, leak {leak:.2e}",
                        root.r,
                        root.c[0],
                        root.c[1],
                        images.len(),
                        rs[0],
                        rs[rs.len() - 1]
                    );
                }
                Err(why) => println!("== {name}: refused in {ms:.0} ms — {why}"),
            }
        }
    }

    /// **Does `grand-julian` enumerate once arms are in the alphabet?**
    ///
    /// The whole of family J's CPU half, asked end to end. Today the
    /// flame is refused outright — `julian` bounded over every arm is
    /// an annulus whose radius does not depend on the input. With one
    /// symbol per `(transform, arm)` the bound shrinks, so the
    /// question is whether the antichain is small enough to be worth
    /// forcing and cheap enough to compute.
    ///
    /// `grand-julian` has arms 2, 15 and 8, so its alphabet is 25
    /// symbols against 3 — the branching factor is eight times wider,
    /// and it has to pay for itself in depth.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn grand_julian_enumerates_with_arms() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["grand-julian", "julian-disc"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let arms: Vec<u32> = cfg
                .flame
                .transforms
                .iter()
                .filter(|t| t.weight > 0.0)
                .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
                .map(|b| b.arms())
                .collect();
            println!(
                "== {name}: arms {arms:?}, alphabet {}",
                arms.iter().sum::<u32>()
            );

            // On the set: walk the flame's own maps as points, arm 0,
            // which is a legitimate orbit whichever arm it picks.
            let bounders: Vec<_> = cfg
                .flame
                .transforms
                .iter()
                .filter(|t| t.weight > 0.0)
                .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
                .collect();
            let mut p = [0.31f64, 0.17];
            for k in 0..64 {
                let j = k % bounders.len();
                match bounders[j].apply_arm(Ball::new(p, 0.0), (k as u32) % arms[j].max(1)) {
                    Ok(b) if b.c[0].is_finite() && b.c[1].is_finite() => p = b.c,
                    _ => break,
                }
            }
            println!("   view on the set at [{:.4}, {:.4}]", p[0], p[1]);

            for zoom in [1e0f64, 1e2, 1e4, 1e6] {
                let view = View::of(zoom, p, 512, 512);
                for (label, armed) in [("plain", false), ("armed", true)] {
                    let t0 = std::time::Instant::now();
                    let r = if armed {
                        Cylinders::plan_armed(&cfg.flame, reg, view)
                    } else {
                        Cylinders::plan(&cfg.flame, reg, view)
                    };
                    let ms = t0.elapsed().as_secs_f64() * 1e3;
                    match r {
                        Ok(c) => println!(
                            "   zoom {zoom:>6.0e} {label:<6} {ms:>7.1} ms  {:>5} words, \
                             depth {:>2}, mass {:.2e}, speedup {:.3e}, lost {:.2e}",
                            c.words.len(),
                            c.depth,
                            c.mass,
                            c.speedup(),
                            c.lost
                        ),
                        Err(e) => {
                            println!("   zoom {zoom:>6.0e} {label:<6} {ms:>7.1} ms  {e:?}")
                        }
                    }
                }
            }
            println!();
        }
    }

    /// **Does an arm-resolved word contract for `grand-julian`?**
    ///
    /// Family J's question, asked of whole transforms rather than of
    /// one variation. `grand-julian` is three `julian` transforms with
    /// `dist = -1` and powers 2, 15 and 8 — twenty-five arms between
    /// them — and it is refused today because `julian` bounded over
    /// every arm is an annulus whose radius does not depend on the
    /// input.
    ///
    /// This walks a random word twice: once with the arms unresolved
    /// (what `plan` does today) and once with an arm chosen per
    /// symbol (what family J would do). If the second does not shrink,
    /// there is nothing to build.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn an_arm_resolved_word_contracts_for_grand_julian() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["grand-julian", "julian-disc"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let bounders: Vec<crate::scene::ifs_ball::Bounder> = cfg
                .flame
                .transforms
                .iter()
                .filter(|t| t.weight > 0.0)
                .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
                .collect();
            if bounders.is_empty() {
                println!("  {name}: no bounders");
                continue;
            }
            let arms: Vec<u32> = bounders.iter().map(|b| b.arms()).collect();
            println!(
                "== {name}: arms per transform {arms:?} (alphabet {})",
                arms.iter().sum::<u32>()
            );

            // A disc on the attractor, found by walking the flame's
            // own maps as points.
            let mut p = [0.31f64, 0.17];
            let mut st = 9u64;
            let mut lcg = move || {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (st >> 33) as usize
            };
            let start = Ball::new(p, 1e-3);
            let _ = &mut p;

            for (label, resolve) in [("unresolved", false), ("arm-resolved", true)] {
                let mut b = start;
                let mut line = Vec::new();
                let mut ok = true;
                for k in 1..=16usize {
                    let j = lcg() % bounders.len();
                    let r = if resolve {
                        let a = (lcg() as u32) % arms[j].max(1);
                        bounders[j].apply_arm(b, a)
                    } else {
                        bounders[j].apply(b)
                    };
                    match r {
                        Ok(next) => {
                            b = next;
                            if k % 4 == 0 {
                                line.push(format!("{k}:{:.2e}", b.r));
                            }
                        }
                        Err(e) => {
                            line.push(format!("{k}:{e}"));
                            ok = false;
                            break;
                        }
                    }
                }
                println!(
                    "   {label:<13} start {:.1e} -> {}{}",
                    start.r,
                    line.join("  "),
                    if ok { "" } else { "  (stopped)" }
                );
            }
            println!();
        }
    }

    /// **Does an equal map stay equal, forty symbols down?**
    ///
    /// Merging cylinders by their composed map only works if two words
    /// carrying the same map produce the same key. `w` and
    /// `a·a⁻¹·w` are the same map exactly, in arithmetic; in f64 they
    /// are two different products of forty matrices whose entries have
    /// grown by orders of magnitude. If they drift past the quantum,
    /// the merge silently stops merging and the enumeration is back
    /// where it was — with no symptom except being slow.
    ///
    /// So: measure the drift before building anything on it.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn equal_words_keep_equal_keys() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["schottky1", "schottky2"] {
            let Ok(text) = std::fs::read_to_string(format!("output/flame-zoom/{name}.fflame"))
            else {
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            let Some(mf) = crate::scene::mobius::MobiusFlame::read(
                &cfg.flame,
                reg,
                4096,
            ) else {
                println!("  {name}: not family M");
                continue;
            };
            // Inverse pairs, numerically.
            let mut inv: Vec<(usize, usize)> = Vec::new();
            for i in 0..mf.maps.len() {
                for j in 0..mf.maps.len() {
                    let ok = mf.root.points.iter().take(48).all(|p| {
                        let q = mf.maps[j].apply_point(mf.maps[i].apply_point(*p));
                        (q[0] - p[0]).hypot(q[1] - p[1]) <= 1e-6 * (1.0 + p[0].hypot(p[1]))
                    });
                    if ok {
                        inv.push((i, j));
                    }
                }
            }
            println!("== {name}   inverse pairs {inv:?}");
            println!("     depth   drift(w vs a.a'.w)   key match   distinct maps / words");

            let mut st = 4242u64;
            let mut lcg = move || {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (st >> 33) as usize
            };
            for depth in [10usize, 20, 30, 40, 60, 80] {
                // A reduced word of this length.
                let mut syms: Vec<usize> = Vec::new();
                let mut last: Option<usize> = None;
                while syms.len() < depth {
                    let mut j = lcg() % mf.maps.len();
                    let mut tries = 0;
                    while last.is_some_and(|l| inv.contains(&(l, j))) && tries < 8 {
                        j = lcg() % mf.maps.len();
                        tries += 1;
                    }
                    last = Some(j);
                    syms.push(j);
                }
                // Both words, composed the way `plan` composes them:
                // the new symbol goes on the INSIDE.
                let compose = |word: &[usize]| {
                    let mut w = mf.empty_word();
                    for &j in word.iter().rev() {
                        w = mf.extend(&w, j).expect("composition");
                    }
                    w
                };
                let plain = compose(&syms);
                // Insert a·a⁻¹ in the middle: same map, two longer.
                let (a, ai) = inv[0];
                let mut padded = syms.clone();
                let mid = padded.len() / 2;
                padded.splice(mid..mid, [ai, a]);
                let folded = compose(&padded);

                let drift = plain
                    .map
                    .projective_distance(&folded.map)
                    .unwrap_or(f64::INFINITY);
                let same = plain.map.key().is_some() && plain.map.key() == folded.map.key();

                // How much merging is on offer at this depth: distinct
                // maps among all words of length `d`, for small `d`.
                let d = depth.min(8);
                let mut maps = std::collections::HashSet::new();
                let mut count = 0usize;
                let n = mf.maps.len();
                let mut stack: Vec<(usize, crate::scene::mobius::Word)> =
                    vec![(0, mf.empty_word())];
                while let Some((k, w)) = stack.pop() {
                    if k == d {
                        count += 1;
                        if let Some(key) = w.map.key() {
                            maps.insert(key);
                        }
                        continue;
                    }
                    for j in 0..n {
                        if let Some(c) = mf.extend(&w, j) {
                            stack.push((k + 1, c));
                        }
                    }
                }
                println!(
                    "     {depth:>5}   {drift:>18.3e}   {:>9}   {:>7} / {:<7} (at depth {d})",
                    if same { "yes" } else { "NO" },
                    maps.len(),
                    count
                );
            }
            println!();
        }
    }

    /// **The real Schottky flames, decomposed from `schottky_group`.**
    ///
    /// `output/flame-zoom/schottky{1,2}.fflame` are four `mobius`
    /// transforms each: two circle-pairing generators and their
    /// inverses. The native `schottky_group` variation walks these
    /// with the Indra's Pearls backtrack-avoid — reduced words. A
    /// decomposed flame picks uniformly and walks everything,
    /// `a·a⁻¹` included, which is the fold-back case §16 suspected.
    ///
    /// Three questions, in order: does the file read as family M and
    /// are its four isometric circles disjoint (the Schottky
    /// condition); do regions contract along a raw random word; and
    /// do they along a backtrack-avoiding one.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn the_real_schottky_flames() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        for name in ["schottky1", "schottky2"] {
            let path = format!("output/flame-zoom/{name}.fflame");
            let Ok(text) = std::fs::read_to_string(&path) else {
                println!("  no {path}");
                continue;
            };
            let cfg: crate::config::FractalConfig =
                serde_json::from_str(&text).expect("a config");
            println!("== {name}");
            let Some(mf) = crate::scene::mobius::MobiusFlame::read(
                &cfg.flame,
                reg,
                crate::scene::mobius::ROOT_SAMPLE,
            ) else {
                println!("  MobiusFlame::read declined");
                continue;
            };
            println!(
                "  cover: {} discs, extent {:.4e}, leak {:.3e}",
                mf.root.discs.len(),
                mf.extent,
                mf.leak
            );
            // Isometric circles: for det-1 (az+b)/(cz+d), the map's
            // own at -d/c and its inverse's at a/c, both radius 1/|c|.
            let mut circles: Vec<([f64; 2], f64)> = Vec::new();
            for (i, m) in mf.moebius.iter().enumerate() {
                let det = m.a.mul(m.d).add(m.b.mul(m.c).scale(-1.0));
                let cabs = m.c.abs();
                if cabs <= 0.0 {
                    println!("  map {i}: c = 0 (affine), det {:.3}", det.abs());
                    continue;
                }
                let r = det.abs().sqrt() / cabs;
                let p = m.d.scale(-1.0).div(m.c).unwrap();
                println!(
                    "  map {i}: det {:.3}{}  isometric circle centre [{:.3}, {:.3}] r {:.3}",
                    det.abs(),
                    if m.conj { " (anti)" } else { "" },
                    p.re,
                    p.im,
                    r
                );
                circles.push(([p.re, p.im], r));
            }
            let mut disjoint = true;
            for i in 0..circles.len() {
                for j in (i + 1)..circles.len() {
                    let (a, b) = (circles[i], circles[j]);
                    let d = (a.0[0] - b.0[0]).hypot(a.0[1] - b.0[1]);
                    if d < a.1 + b.1 {
                        disjoint = false;
                        println!(
                            "  circles {i} and {j} OVERLAP: distance {d:.3} < {:.3}",
                            a.1 + b.1
                        );
                    }
                }
            }
            println!("  Schottky condition (all circles disjoint): {disjoint}");

            // Which pairs are inverses, numerically.
            let mut inv: Vec<(usize, usize)> = Vec::new();
            for i in 0..mf.maps.len() {
                for j in 0..mf.maps.len() {
                    let mut ok = true;
                    for p in mf.root.points.iter().take(64) {
                        let q = mf.maps[j].apply_point(mf.maps[i].apply_point(*p));
                        if (q[0] - p[0]).hypot(q[1] - p[1]) > 1e-6 * (1.0 + p[0].hypot(p[1])) {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        inv.push((i, j));
                    }
                }
            }
            println!("  inverse pairs: {inv:?}");

            // Regions along a raw random word, and along one that never
            // follows a symbol with its inverse.
            for (label, avoid) in [("raw", false), ("backtrack-avoiding", true)] {
                let mut st = 99u64;
                let mut lcg = move || {
                    st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    ((st >> 33) as usize)
                };
                let mut cur = mf.empty_word();
                let mut syms: Vec<u32> = Vec::new();
                let mut last: Option<usize> = None;
                let mut line = Vec::new();
                for k in 1..=24usize {
                    let mut j = lcg() % mf.maps.len();
                    if avoid {
                        while let Some(l) = last {
                            if inv.contains(&(l, j)) {
                                j = lcg() % mf.maps.len();
                            } else {
                                break;
                            }
                        }
                    }
                    last = Some(j);
                    let Some(next) = mf.extend(&cur, j) else { break };
                    cur = next;
                    // Inside: the new symbol is applied FIRST, but a
                    // backtrack is a backtrack in either convention.
                    syms.insert(0, j as u32);
                    if k % 4 == 0 {
                        match mf.region(&cur, &syms) {
                            Ok(c) => line.push(format!(
                                "{k}:{:.2e}",
                                c.enclosing().map_or(f64::NAN, |d| d.r)
                            )),
                            Err(e) => {
                                line.push(format!("{k}:{e:?}"));
                                break;
                            }
                        }
                    }
                }
                println!("  {label:<20} {}", line.join("  "));
            }
            println!();
        }
    }

    /// **Is it the family, or is it that flame?**
    ///
    /// `spherical.fflame`'s measure does not concentrate — the words
    /// holding 90% of a view's measure go 3, 10, 41, 146, 579, 1818
    /// as the depth grows, and cylinder targeting needs that number
    /// to settle. But that flame also has two pure TRANSLATIONS,
    /// which is why its attractor is unbounded, and nothing says
    /// every inversive flame is like it.
    ///
    /// A Schottky configuration is the opposite extreme: inversions
    /// in mutually DISJOINT circles, each mapping the outside of its
    /// own circle into the inside. The images are then disjoint, the
    /// limit set is a Cantor set, and every point has one address.
    /// That is what cylinder targeting was built for, and it is the
    /// fair test of whether family M is useful machinery or just
    /// correct machinery.
    ///
    /// Built here rather than loaded: it is a statement about the
    /// geometry, not about any file.
    fn schottky_flame(rho: f32) -> crate::scene::transforms::Flame {
        use crate::scene::transforms::{Flame, Transform};
        let mut f = Flame::new();
        f.transforms.clear();
        // Four circles of radius `rho` at the compass points, at
        // distance 1 from the origin — mutually disjoint while
        // `rho < 1/√2`.
        for (cx, cy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let mut t = Transform::default();
            // Inversion in the circle (c, rho) is
            // `p -> c + rho²·(p − c)/|p − c|²`: translate the centre to
            // the origin, invert, scale by rho², translate back.
            t.a = 1.0;
            t.d = 1.0;
            t.e = -cx;
            t.f = -cy;
            t.weight = 1.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("spherical", rho * rho);
            t.post_affine_enabled = true;
            t.post_a = 1.0;
            t.post_d = 1.0;
            t.post_e = cx;
            t.post_f = cy;
            f.transforms.push(t);
        }
        f
    }

    #[test]
    #[ignore = "prints a measurement"]
    fn a_schottky_flame_localizes_where_the_kleinian_one_does_not() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let f = schottky_flame(0.4);
        let mf = crate::scene::mobius::MobiusFlame::read(
            &f,
            reg,
            crate::scene::mobius::ROOT_SAMPLE,
        )
        .expect("family M");
        println!(
            "  cover: {} discs, extent {:.4e}, leak {:.3e}",
            mf.root.discs.len(),
            mf.extent,
            mf.leak
        );

        let x = mf.root.points[mf.root.points.len() / 2];
        let weights: Vec<f64> = f.transforms.iter().map(|t| t.weight as f64).collect();
        let total: f64 = weights.iter().sum();

        // How the measure spreads over words, the same question asked
        // of the other flame.
        const HIST: usize = 20;
        let view_r = 3e-2f64;
        let mut st: u64 = 555;
        let mut lcg = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut p = [0.3f64, 0.2];
        let mut hist: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
        let mut in_view: Vec<Vec<u32>> = Vec::new();
        for i in 0..3_000_000usize {
            let mut u = lcg() * total;
            let mut j = weights.len() - 1;
            for (k, w) in weights.iter().enumerate() {
                if u < *w {
                    j = k;
                    break;
                }
                u -= *w;
            }
            p = mf.maps[j].apply_point(p);
            if !p[0].is_finite() {
                p = [0.3, 0.2];
                hist.clear();
                continue;
            }
            hist.push_back(j as u32);
            if hist.len() > HIST {
                hist.pop_front();
            }
            if i > 2000 && hist.len() == HIST && (p[0] - x[0]).hypot(p[1] - x[1]) <= view_r {
                in_view.push(hist.iter().copied().collect());
            }
        }
        println!("  {} samples in a view of radius {view_r:.0e}", in_view.len());
        if in_view.len() < 100 {
            println!("  too few to say anything");
            return;
        }
        println!("  depth   distinct   words for 90%");
        for k in [2usize, 4, 6, 8, 10, 12, 16, 20] {
            let mut counts: std::collections::HashMap<Vec<u32>, usize> =
                std::collections::HashMap::new();
            for h in &in_view {
                *counts.entry(h[HIST - k..].to_vec()).or_default() += 1;
            }
            let mut v: Vec<usize> = counts.values().copied().collect();
            v.sort_unstable_by(|a, b| b.cmp(a));
            let tot: usize = v.iter().sum();
            let (mut acc, mut need) = (0usize, v.len());
            for (i, c) in v.iter().enumerate() {
                acc += c;
                if acc as f64 >= 0.9 * tot as f64 {
                    need = i + 1;
                    break;
                }
            }
            println!("  {k:>5}   {:>8}   {need:>13}", v.len());
        }

        println!();
        // Does a word's REGION shrink the way its cylinder does? The
        // enumeration cuts on the region, so if it does not, nothing
        // is ever kept and the beam drops everything.
        println!("  depth   region radius   (cylinder should be ~0.16^k)");
        let mut cur = mf.empty_word();
        let mut syms: Vec<u32> = Vec::new();
        for k in 1..=8usize {
            let Some(next) = mf.extend(&cur, k % mf.maps.len()) else { break };
            cur = next;
            syms.insert(0, (k % mf.maps.len()) as u32);
            match mf.region(&cur, &syms) {
                Ok(c) => println!(
                    "  {k:>5}   {:>13.4e}   ({} discs)",
                    c.enclosing().map_or(f64::NAN, |d| d.r),
                    c.discs.len()
                ),
                Err(e) => {
                    println!("  {k:>5}   {e:?}");
                    break;
                }
            }
        }

        println!();
        for zoom in [1e2f64, 1e4, 1e6, 1e8] {
            let view = View::of(zoom, x, 512, 512);
            let t0 = std::time::Instant::now();
            let r = Cylinders::plan(&f, reg, view);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            match r {
                Ok(c) => println!(
                    "   zoom {zoom:>7.0e}  {ms:>7.1} ms  {:>4} words, depth {:>3}, \
                     speedup {:.3e}, lost {:.3e}",
                    c.words.len(),
                    c.depth,
                    c.speedup(),
                    c.lost
                ),
                Err(e) => println!("   zoom {zoom:>7.0e}  {ms:>7.1} ms  {e:?}"),
            }
        }
    }

    /// **How loose is the bound, in words?**
    ///
    /// The enumeration keeps a word when its computed region meets
    /// the view. The truth is whether its image carries any measure
    /// there. This counts both, at a view big enough that the orbit
    /// gives real statistics:
    ///
    /// * `bound`  — words of length k whose region meets the view;
    /// * `truth`  — words of length k that some orbit sample in the
    ///   view actually belongs to.
    ///
    /// Their ratio is the looseness, and it decides the next move. If
    /// it is near one, the flame simply overlaps and no better bound
    /// helps. If it is orders, the cover's resolution is the dial.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn how_loose_is_the_region_in_words() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/spherical.fflame") else {
            println!("  no corpus");
            return;
        };
        let cfg: crate::config::FractalConfig =
            serde_json::from_str(&text).expect("a config");
        let mf = crate::scene::mobius::MobiusFlame::read(
            &cfg.flame,
            reg,
            crate::scene::mobius::ROOT_SAMPLE,
        )
        .expect("family M");
        let n = mf.maps.len();
        let x = mf.root.points[mf.root.points.len() / 2];
        let weights: Vec<f64> = cfg
            .flame
            .transforms
            .iter()
            .filter(|t| t.weight > 0.0)
            .map(|t| t.weight as f64)
            .collect();
        let total: f64 = weights.iter().sum();

        const HIST: usize = 8;
        for view_r in [3e-1f64, 1e-1, 3e-2] {
            // --- truth: words the orbit actually uses to reach here
            let mut st: u64 = 777;
            let mut lcg = move || {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((st >> 33) as f64) / ((1u64 << 31) as f64)
            };
            let mut p = [0.3f64, 0.2];
            let mut hist: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
            let mut seen: Vec<std::collections::HashSet<Vec<u32>>> =
                vec![std::collections::HashSet::new(); HIST + 1];
            let mut hits = 0usize;
            let mut plotted = 0usize;
            for i in 0..3_000_000usize {
                let mut u = lcg() * total;
                let mut j = weights.len() - 1;
                for (k, w) in weights.iter().enumerate() {
                    if u < *w {
                        j = k;
                        break;
                    }
                    u -= *w;
                }
                p = mf.maps[j].apply_point(p);
                if !p[0].is_finite() {
                    p = [0.3, 0.2];
                    hist.clear();
                    continue;
                }
                hist.push_back(j as u32);
                if hist.len() > HIST {
                    hist.pop_front();
                }
                if i > 2000 && hist.len() == HIST {
                    plotted += 1;
                    if (p[0] - x[0]).hypot(p[1] - x[1]) <= view_r {
                        hits += 1;
                        let h: Vec<u32> = hist.iter().copied().collect();
                        for k in 1..=HIST {
                            seen[k].insert(h[HIST - k..].to_vec());
                        }
                    }
                }
            }

            // --- bound: words whose computed region meets the view
            let view = View { centre: x, radius: view_r };
            println!(
                "  view radius {view_r:.1e}: {hits} of {plotted} samples inside ({:.2e})",
                hits as f64 / plotted.max(1) as f64
            );
            println!("    k   words    truth   bound   looseness");
            let mut level: Vec<(Vec<u32>, crate::scene::mobius::Word)> =
                vec![(Vec::new(), mf.empty_word())];
            for k in 1..=6usize {
                let mut next = Vec::new();
                for (word, parent) in &level {
                    for i in 0..n {
                        let mut w = Vec::with_capacity(word.len() + 1);
                        w.push(i as u32);
                        w.extend_from_slice(word);
                        let Some(cw) = mf.extend(parent, i) else { continue };
                        let Ok(cover) = mf.region(&cw, &w) else { continue };
                        if cover.meets_disc(view.centre, view.radius) {
                            next.push((w, cw));
                        }
                    }
                }
                let truth = seen[k].len();
                println!(
                    "   {k:>2}   {:>5}   {truth:>6}   {:>5}   {:>9.1}x",
                    n.pow(k as u32),
                    next.len(),
                    next.len() as f64 / truth.max(1) as f64
                );
                level = next;
                if level.is_empty() {
                    break;
                }
            }
            println!();
        }
    }

    /// **Do this flame's cylinders localize at all?**
    ///
    /// Targeting replaces "run the chaos game and hope a sample lands
    /// in the view" with "force a word whose whole image is in the
    /// view". That only pays if the measure reaching the view is
    /// carried by FEW words. This counts them, from the real orbit:
    /// how many distinct length-k prefixes are shared by the samples
    /// that land in the view, and what share of the view's measure
    /// the commonest few hold.
    ///
    /// For a gasket the answer is one word per depth. For an IFS
    /// whose pieces overlap it is many, and no enumeration can help.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn do_the_kleinian_cylinders_localize() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/spherical.fflame") else {
            println!("  no corpus");
            return;
        };
        let cfg: crate::config::FractalConfig =
            serde_json::from_str(&text).expect("a config");
        let mf = crate::scene::mobius::MobiusFlame::read(
            &cfg.flame,
            reg,
            crate::scene::mobius::ROOT_SAMPLE,
        )
        .expect("family M");
        let x = mf.root.points[mf.root.points.len() / 2];
        let weights: Vec<f64> = cfg
            .flame
            .transforms
            .iter()
            .filter(|t| t.weight > 0.0)
            .map(|t| t.weight as f64)
            .collect();
        let total: f64 = weights.iter().sum();

        // A long run, remembering each sample's recent symbol history.
        const HIST: usize = 24;
        let mut st: u64 = 20240920;
        let mut lcg = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 33) as f64) / ((1u64 << 31) as f64)
        };
        let mut p = [0.3f64, 0.2];
        let mut hist: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
        let n_steps = 4_000_000usize;
        let mut in_view: Vec<Vec<u32>> = Vec::new();
        let mut plotted = 0usize;
        for zoom_i in 0..1 {
            let _ = zoom_i;
        }
        // Big enough that the orbit gives real statistics: at 2.8e-3
        // only 32 of four million samples landed inside, and 32
        // samples cannot tell "few words" from "many".
        let view_r = 3e-2f64;
        for i in 0..n_steps {
            let mut u = lcg() * total;
            let mut j = weights.len() - 1;
            for (k, w) in weights.iter().enumerate() {
                if u < *w {
                    j = k;
                    break;
                }
                u -= *w;
            }
            p = mf.maps[j].apply_point(p);
            if !p[0].is_finite() || !p[1].is_finite() {
                p = [0.3, 0.2];
                hist.clear();
                continue;
            }
            hist.push_back(j as u32);
            if hist.len() > HIST {
                hist.pop_front();
            }
            if i > 1000 && hist.len() == HIST {
                plotted += 1;
                if (p[0] - x[0]).hypot(p[1] - x[1]) <= view_r {
                    in_view.push(hist.iter().copied().collect());
                }
            }
        }
        println!(
            "  {plotted} samples, {} in a view of radius {view_r:.3e} ({:.3e})",
            in_view.len(),
            in_view.len() as f64 / plotted.max(1) as f64
        );
        if in_view.is_empty() {
            return;
        }
        println!("  depth   distinct words   top word share   words for 90%");
        for k in [2usize, 4, 6, 8, 10, 12, 16, 20] {
            let mut counts: std::collections::HashMap<Vec<u32>, usize> =
                std::collections::HashMap::new();
            for h in &in_view {
                // The LAST k symbols, in application order, are the
                // word whose image this sample is in.
                let w: Vec<u32> = h[HIST - k..].to_vec();
                *counts.entry(w).or_default() += 1;
            }
            let mut v: Vec<usize> = counts.values().copied().collect();
            v.sort_unstable_by(|a, b| b.cmp(a));
            let tot: usize = v.iter().sum();
            let mut acc = 0usize;
            let mut need = v.len();
            for (i, c) in v.iter().enumerate() {
                acc += c;
                if acc as f64 >= 0.9 * tot as f64 {
                    need = i + 1;
                    break;
                }
            }
            println!(
                "  {k:>5}   {:>14}   {:>14.3}   {need:>12}",
                v.len(),
                v[0] as f64 / tot as f64
            );
        }
    }

    /// **Does the frontier settle, or does it branch forever?**
    ///
    /// The enumeration only works if most words MISS the view, so
    /// that the frontier stays narrow while the depth grows. For a
    /// gasket that happens at the first level: the three images are
    /// disjoint and a small view is inside exactly one. For a flame
    /// whose pieces overlap, every word meets everything and the
    /// frontier is the branching factor to the depth.
    ///
    /// This replicates the BFS and prints the width at each level,
    /// which is the difference between "the cover is too coarse" and
    /// "this IFS overlaps".
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn how_wide_is_the_kleinian_frontier() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let Ok(text) = std::fs::read_to_string("output/flame-zoom/spherical.fflame") else {
            println!("  no corpus");
            return;
        };
        let cfg: crate::config::FractalConfig =
            serde_json::from_str(&text).expect("a config");
        let mf = crate::scene::mobius::MobiusFlame::read(
            &cfg.flame,
            reg,
            crate::scene::mobius::ROOT_SAMPLE,
        )
        .expect("family M");
        let n = mf.maps.len();
        let x = mf.root.points[mf.root.points.len() / 2];

        for zoom in [1e3f64, 1e7, 1e11] {
            let view = View::of(zoom, x, 512, 512);
            println!(
                "  zoom {zoom:.0e}, view radius {:.3e}, centred on the set",
                view.radius
            );
            let mut frontier: Vec<(Vec<u32>, crate::scene::mobius::Word)> =
                vec![(Vec::new(), mf.empty_word())];
            let mut kept = 0usize;
            for depth in 1..=24 {
                let mut next = Vec::new();
                for (word, parent) in &frontier {
                    for i in 0..n {
                        let mut w = Vec::with_capacity(word.len() + 1);
                        w.push(i as u32);
                        w.extend_from_slice(word);
                        let Some(cw) = mf.extend(parent, i) else { continue };
                        let Ok(cover) = mf.region(&cw, &w) else { continue };
                        if !cover.meets_disc(view.centre, view.radius) {
                            continue;
                        }
                        let r = cover.enclosing().map_or(f64::INFINITY, |d| d.r);
                        if r <= view.radius {
                            kept += 1;
                        } else {
                            next.push((w, cw));
                        }
                    }
                }
                let widest = next
                    .iter()
                    .filter_map(|(w, cw)| mf.region(cw, w).ok())
                    .filter_map(|c| c.enclosing())
                    .map(|d| d.r)
                    .fold(0.0f64, f64::max);
                println!(
                    "   depth {depth:>2}: frontier {:>6}, kept {kept:>5}, widest region {widest:.3e}",
                    next.len()
                );
                frontier = next;
                if frontier.is_empty() || frontier.len() > 20000 {
                    break;
                }
            }
            println!();
        }
    }

    /// **The Kleinian flame, at the depths targeting is for.**
    ///
    /// `spherical.fflame` has no invariant disc — its maps are
    /// inversions, so every disc containing the attractor contains a
    /// pole — and before family M it was refused outright. This asks
    /// what it does now, on the set, as the view shrinks.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn the_kleinian_flame_at_depth() {
        use std::time::Instant;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let path = "output/flame-zoom/spherical.fflame";
        let Ok(text) = std::fs::read_to_string(path) else {
            println!("  no {path}");
            return;
        };
        let cfg: crate::config::FractalConfig =
            serde_json::from_str(&text).expect("a config");

        let mf = crate::scene::mobius::MobiusFlame::read(
            &cfg.flame,
            reg,
            crate::scene::mobius::ROOT_SAMPLE,
        )
        .expect("family M");
        println!(
            "  cover: {} discs, {} anchor points, extent {:.4e}, leak {:.3e}",
            mf.root.discs.len(),
            mf.root.points.len(),
            mf.extent,
            mf.leak
        );

        // A point on the attractor: one the cover's own weighted
        // orbit actually visited. A round-robin walk is not that —
        // applying each map in turn means applying the translations
        // every fourth step, which marches out into the unbounded
        // tail. Measured, it landed at |p| = 91 and every view there
        // was empty, correctly.
        let p = mf.root.points[mf.root.points.len() / 2];
        println!("  view centred on the set at [{:.4}, {:.4}]", p[0], p[1]);

        for zoom in [1e1f64, 1e3, 1e5, 1e7, 1e9, 1e12] {
            let view = View::of(zoom, p, 512, 512);
            let t0 = Instant::now();
            let r = Cylinders::plan(&cfg.flame, reg, view);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            match r {
                Ok(c) => println!(
                    "   zoom {zoom:>8.0e}  {ms:>8.1} ms  {:>5} words, depth {:>3}, \
                     speedup {:.3e}, lost {:.3e}, leak {:.3e}",
                    c.words.len(),
                    c.depth,
                    c.speedup(),
                    c.lost,
                    c.sampling_leak
                ),
                Err(e) => println!("   zoom {zoom:>8.0e}  {ms:>8.1} ms  {e:?}"),
            }
        }
    }

    /// **Why each flame in a zoom corpus can or cannot be targeted.**
    ///
    /// Reads `output/flame-zoom/*.fflame` — hand-picked flames people
    /// actually want to zoom, as opposed to randomiser output — and
    /// prints, per flame, the two numbers that decide it:
    ///
    /// * the CONTRACTION of each map, measured on the root ball, and
    ///   the depth the slowest one needs to reach a deep view. The
    ///   enumeration caps at `MAX_DEPTH`, so a map at 0.95 cannot
    ///   reach 1e-6 however patient it is.
    ///
    /// * the SIMILARITY DIMENSION `D` solving `Σ sᵢᴰ = 1`. Above 2 the
    ///   pieces must overlap — the attractor fills area instead of
    ///   being sparse — and the number of cylinders meeting a small
    ///   view GROWS as the view shrinks. That is `TooManyWords`, and
    ///   no larger cap fixes it.
    #[test]
    #[ignore = "reads output/flame-zoom"]
    fn why_each_zoom_flame_does_or_does_not_target() {
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let dir = std::path::Path::new("output/flame-zoom");
        let Ok(rd) = std::fs::read_dir(dir) else {
            println!("  no output/flame-zoom — nothing to measure");
            return;
        };
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        files.sort();

        for path in files {
            if path.extension().and_then(|x| x.to_str()) != Some("fflame") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(cfg) = serde_json::from_str::<crate::config::FractalConfig>(&text) else {
                println!("  {:?}: not a config", path.file_stem().unwrap_or_default());
                continue;
            };
            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            println!();
            println!("  == {name}   zoom {:.4e}", cfg.zoom);

            // Contraction of each map, on whatever root we can find.
            let root = invariant_ball(&cfg.flame, reg);
            let mut sigmas: Vec<f64> = Vec::new();
            match &root {
                Ok((c, r)) => {
                    println!("     root ball  c=[{:.4}, {:.4}]  r={:.5}", c[0], c[1], r);
                    for (i, t) in cfg.flame.transforms.iter().enumerate() {
                        if t.weight <= 0.0 {
                            continue;
                        }
                        match crate::scene::ifs_ball::transform_ball_2d(t, reg, Ball::new(*c, *r)) {
                            Ok(img) => {
                                let s = img.r / r;
                                sigmas.push(s);
                                let vars: Vec<&str> =
                                    t.ordered_variation_names(reg).iter().map(|_| "").collect();
                                let _ = vars;
                                println!(
                                    "     xform {i}  w={:.3}  contraction {:.4}{}",
                                    t.weight,
                                    s,
                                    if s >= 1.0 { "   NOT CONTRACTIVE" } else { "" }
                                );
                            }
                            Err(why) => println!("     xform {i}  w={:.3}  {why}", t.weight),
                        }
                    }
                }
                Err(e) => {
                    println!("     no root ball: {e:?}");
                    // How close to the origin does the orbit go? A
                    // disc holding the attractor must hold every point
                    // the orbit visits, so if this gets small then
                    // every candidate disc contains the origin -- and
                    // the origin is exactly where the inversive
                    // bodies are unbounded. That is the disc being
                    // the wrong SHAPE, not the flame being wild.
                    let bs: Vec<_> = cfg
                        .flame
                        .transforms
                        .iter()
                        .filter(|t| t.weight > 0.0)
                        .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
                        .collect();
                    if !bs.is_empty() {
                        let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
                        let mut p = [1.0f64, 0.0];
                        let mut steps = 0;
                        for k in 0..4000 {
                            match bs[k % bs.len()].apply(Ball::new(p, 0.0)) {
                                Ok(img) if img.c[0].is_finite() && img.c[1].is_finite() => {
                                    p = img.c;
                                    let m = (p[0] * p[0] + p[1] * p[1]).sqrt();
                                    if k > 32 {
                                        lo = lo.min(m);
                                        hi = hi.max(m);
                                    }
                                    steps += 1;
                                }
                                _ => break,
                            }
                        }
                        println!(
                            "     orbit over {steps} steps: |p| in [{lo:.2e}, {hi:.2e}]"
                        );
                    }
                    for (i, t) in cfg.flame.transforms.iter().enumerate() {
                        if t.weight <= 0.0 {
                            continue;
                        }
                        if let Err(why) =
                            crate::scene::ifs_ball::transform_ball_2d(t, reg, Ball::new([0.0, 0.0], 1.0))
                        {
                            println!("     xform {i}  {why}");
                        }
                    }
                }
            }

            if !sigmas.is_empty() && sigmas.iter().all(|s| *s < 1.0) {
                // Σ sᵢᴰ = 1, by bisection.
                let (mut lo, mut hi) = (0.01f64, 80.0f64);
                for _ in 0..200 {
                    let mid = 0.5 * (lo + hi);
                    let v: f64 = sigmas.iter().map(|s| s.powf(mid)).sum();
                    if v > 1.0 {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let slowest = sigmas.iter().cloned().fold(0.0f64, f64::max);
                let depth = (1e-6f64).ln() / slowest.ln();
                println!(
                    "     dimension D = {lo:.3}{}   slowest {slowest:.4} needs depth {depth:.0} for a 1e-6 view (cap {MAX_DEPTH})",
                    if lo > 2.0 { " OVERLAPS (>2)" } else { "" }
                );
            }

            // **Is a deep refusal about the flame or about the spot?**
            //
            // `ViewIsEmpty` at a deeper zoom can mean two very
            // different things: the enumeration broke, or the point
            // the view is centred on genuinely has nothing at that
            // scale -- a fractal is mostly holes, and zooming 1000x
            // further into a point that was covered before may land
            // in one. Re-ask centred on a point KNOWN to be on the
            // attractor, found by running the chaos game through the
            // bounds, and the two come apart.
            if let Ok((rc, rr)) = &root {
                let mut seed = *rc;
                let bs: Vec<_> = cfg
                    .flame
                    .transforms
                    .iter()
                    .filter(|t| t.weight > 0.0)
                    .filter_map(|t| crate::scene::ifs_ball::Bounder::new(t, reg).ok())
                    .collect();
                for k in 0..128 {
                    if let Ok(img) = bs[k % bs.len()].apply(Ball::new(seed, 0.0)) {
                        seed = img.c;
                    }
                }
                let _ = rr;
                for mult in [1e3f64, 1e9, 1e15] {
                    let view = View::of(
                        (cfg.zoom.max(1e-6) as f64) * mult,
                        seed,
                        512,
                        512,
                    );
                    match Cylinders::plan(&cfg.flame, reg, view) {
                        Ok(c) => println!(
                            "     ON-SET x{mult:<6.0e} OK   {:>5} words, depth {}, speedup {:.3e}",
                            c.words.len(),
                            c.depth,
                            c.speedup()
                        ),
                        Err(e) => println!("     ON-SET x{mult:<6.0e} {e:?}"),
                    }
                }
            }

            // And what the enumeration actually says, at the flame's
            // own framing and deeper.
            for mult in [1.0f64, 1e3, 1e6] {
                let view = View::of(
                    (cfg.zoom.max(1e-6) as f64) * mult,
                    [cfg.pan_x, cfg.pan_y],
                    512,
                    512,
                );
                match Cylinders::plan(&cfg.flame, reg, view) {
                    Ok(c) => println!(
                        "     zoom x{mult:<6.0e} OK   {:>5} words, depth {}, speedup {:.3e}, lost {:.3e}",
                        c.words.len(),
                        c.depth,
                        c.speedup(),
                        c.lost
                    ),
                    Err(e) => println!("     zoom x{mult:<6.0e} {e:?}"),
                }
            }
        }
    }

    /// **What one `plan` costs, and where the time goes.**
    ///
    /// `plan` runs on every pan and every zoom step, so its cost is
    /// felt directly as interface latency — this is not a batch job.
    #[test]
    #[ignore = "prints a measurement"]
    fn what_a_plan_costs() {
        use std::time::Instant;
        let guard = crate::variations::global_registry();
        let reg = &*guard;

        // One `transform_ball_2d` on an affine transform, and one on
        // a transform whose bound has to be DERIVED from the WGSL.
        let mut affine = Transform::default();
        affine.a = 0.5;
        affine.d = 0.5;
        affine.weight = 1.0;
        affine.variations.clear();
        affine.variation_order.clear();
        affine.set_variation("linear", 1.0);

        let mut derived = affine.clone();
        derived.variations.clear();
        derived.variation_order.clear();
        derived.set_variation("sinusoidal", 1.0);

        // One that REFUSES, which is the path that also pays for the
        // k=3 subdivision retry.
        let mut refuses = affine.clone();
        refuses.variations.clear();
        refuses.variation_order.clear();
        refuses.set_variation("curl", 1.0);

        let b = Ball::new([0.1, 0.2], 0.3);

        // **Warm the module cache first.** `modules()` parses every
        // shipped variation's WGSL through the probe's shader builder
        // on first touch, and folding that into the first timed call
        // makes an 8 us operation read as 1 ms.
        let pf0 = |_: &str| 0.0f64;
        let t0 = Instant::now();
        let _ = crate::variations::derive::derive("sinusoidal", &pf0, 1.0, b);
        println!("  first derive of the session   {:>10.1} us  (parses the corpus)",
                 t0.elapsed().as_secs_f64() * 1e6);

        for (label, t) in [("affine", &affine), ("derived", &derived), ("refuses", &refuses)] {
            let _ = crate::scene::ifs_ball::transform_ball_2d(t, reg, b);
            let n = 200;
            let t0 = Instant::now();
            for _ in 0..n {
                let _ = crate::scene::ifs_ball::transform_ball_2d(t, reg, b);
            }
            let each = t0.elapsed().as_secs_f64() / n as f64;
            println!("  transform_ball_2d({label:>8})  {:>10.1} us", each * 1e6);
        }

        // And the derive call ALONE, without the per-call scaffolding
        // `transform_ball_2d` puts around it.
        let pf = |_: &str| 0.0f64;
        for name in ["sinusoidal", "spherical", "julian", "curl"] {
            // Each name parses its own module on first touch now, so
            // warm this one before timing it.
            let _ = crate::variations::derive::derive(name, &pf, 1.0, b);
            let n = 200;
            let t0 = Instant::now();
            let mut ok = true;
            for _ in 0..n {
                ok = crate::variations::derive::derive(name, &pf, 1.0, b).is_ok();
            }
            let each = t0.elapsed().as_secs_f64() / n as f64;
            println!(
                "  derive({name:>12})            {:>10.1} us  {}",
                each * 1e6,
                if ok { "ok" } else { "refused" }
            );
        }
        // The two lookups `one()` does before it ever reaches derive.
        {
            let n = 2000;
            let t0 = Instant::now();
            for _ in 0..n {
                let _ = crate::scene::ifs_analysis::affine_role(
                    "sinusoidal",
                    1.0,
                    &derived,
                    reg,
                    crate::scene::ifs_analysis::Space::Planar,
                );
            }
            println!(
                "  affine_role(sinusoidal)       {:>10.1} us",
                t0.elapsed().as_secs_f64() / n as f64 * 1e6
            );
            let t0 = Instant::now();
            for _ in 0..n {
                let _ = crate::variations::bound::for_name("sinusoidal");
            }
            println!(
                "  bound::for_name(sinusoidal)   {:>10.1} us",
                t0.elapsed().as_secs_f64() / n as f64 * 1e6
            );
            let pf2 = |p: &str| {
                derived.get_variation_param_or_default("sinusoidal", p, reg) as f64
            };
            let t0 = Instant::now();
            for _ in 0..n {
                let _ = crate::variations::derive::derive("sinusoidal", &pf2, 1.0, b);
            }
            println!(
                "  derive w/ the real param fn   {:>10.1} us",
                t0.elapsed().as_secs_f64() / n as f64 * 1e6
            );
        }

        // The naming lookup alone, which every call redoes.
        {
            let n = 2000;
            let t0 = Instant::now();
            for _ in 0..n {
                let _ = derived.ordered_variation_names(reg);
            }
            println!(
                "  ordered_variation_names()     {:>10.1} us",
                t0.elapsed().as_secs_f64() / n as f64 * 1e6
            );
        }

        println!();
        // A whole plan, at the depths the interface actually reaches.
        let gasket = || {
            let mut f = crate::scene::transforms::Flame::new();
            f.transforms.clear();
            for (e, g) in [(0.0f32, 0.0f32), (0.5, 0.0), (0.25, 0.5)] {
                let mut t = Transform::default();
                t.a = 0.5;
                t.d = 0.5;
                t.e = e;
                t.f = g;
                t.weight = 1.0;
                t.variations.clear();
                t.variation_order.clear();
                t.set_variation("linear", 1.0);
                f.transforms.push(t);
            }
            f
        };
        let flame = gasket();
        for mult in [1.0f64, 1e3, 1e6, 1e9] {
            let view = View::of(mult, [0.0, 0.0], 512, 512);
            let t0 = Instant::now();
            let r = Cylinders::plan(&flame, reg, view);
            let dt = t0.elapsed().as_secs_f64();
            match r {
                Ok(c) => println!(
                    "  plan(gasket, zoom {mult:>8.0e})  {:>8.1} ms  {:>5} words  depth {}",
                    dt * 1e3,
                    c.words.len(),
                    c.depth
                ),
                Err(e) => println!("  plan(gasket, zoom {mult:>8.0e})  {:>8.1} ms  {e:?}", dt * 1e3),
            }
        }

        println!();
        // The same, with a transform whose bound is derived rather
        // than read off a matrix.
        let mut nonaffine = gasket();
        nonaffine.transforms[2].set_variation("sinusoidal", 0.02);
        for mult in [1.0f64, 1e3, 1e6] {
            let view = View::of(mult, [0.0, 0.0], 512, 512);
            let t0 = Instant::now();
            let r = Cylinders::plan(&nonaffine, reg, view);
            let dt = t0.elapsed().as_secs_f64();
            match r {
                Ok(c) => println!(
                    "  plan(derived, zoom {mult:>7.0e})  {:>8.1} ms  {:>5} words  depth {}",
                    dt * 1e3,
                    c.words.len(),
                    c.depth
                ),
                Err(e) => println!("  plan(derived, zoom {mult:>7.0e})  {:>8.1} ms  {e:?}", dt * 1e3),
            }
        }
    }

    /// **The number the whole forward-bounds plan exists to move:
    /// how many real flames can actually be targeted.**
    ///
    /// The blocker meter above probes one worst-case disc at the
    /// origin, which is where a radial body's denominator is most
    /// likely to vanish, so it RANKS well and under-reports. This
    /// asks the question the renderer asks: run `Cylinders::plan` on
    /// each corpus flame at its own framing, zoomed in far enough for
    /// targeting to pay, and see what comes back.
    #[test]
    #[ignore = "reads output/*.flame"]
    fn how_many_corpus_flames_enumerate() {
        use std::collections::BTreeMap;
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(rd) = std::fs::read_dir("output") {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("flame") {
                    files.push(p);
                }
            }
        }
        files.sort();
        if files.is_empty() {
            println!("  no corpus in output/ -- nothing to measure");
            return;
        }

        let mut total = 0usize;
        let mut paying = 0usize;
        let mut enumerated = 0usize;
        let mut why: BTreeMap<String, usize> = BTreeMap::new();
        let mut named: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut best: Vec<(f64, String)> = Vec::new();

        for path in &files {
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            let Ok(configs) = crate::flame_xml::parse_flame_xml(&text) else { continue };
            let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            for cfg in configs {
                total += 1;
                // Deep enough that targeting is worth having.
                let mut found: Option<f64> = None;
                let mut last = String::new();
                for mult in [64.0f64, 1024.0, 16384.0] {
                    let view = View::of(
                        (cfg.zoom.max(1e-6) as f64) * mult,
                        [cfg.pan_x, cfg.pan_y],
                        512,
                        512,
                    );
                    match Cylinders::plan(&cfg.flame, reg, view) {
                        Ok(c) => {
                            found = Some(found.unwrap_or(0.0).max(c.speedup()));
                        }
                        Err(e) => last = format!("{e:?}"),
                    }
                }
                println!(
                    "    {:<34} {:>3} xf  {}",
                    stem,
                    cfg.flame.transforms.len(),
                    match &found {
                        Some(sp) => format!("ENUMERATES, speedup {sp:.3e}"),
                        None => last.split(['{', '(']).next().unwrap_or("?").trim().to_string(),
                    }
                );
                match found {
                    Some(s) => {
                        enumerated += 1;
                        if s > 1.0 {
                            paying += 1;
                            best.push((s, stem.clone()));
                        }
                    }
                    None => {
                        let k = last.split(['{', '(']).next().unwrap_or("?").trim().to_string();
                        *why.entry(k.clone()).or_default() += 1;
                        named.entry(k).or_default().push(stem.clone());
                    }
                }
            }
        }

        println!();
        println!("  {total} corpus flames:");
        println!("    {enumerated:>4} enumerate at some depth");
        println!("    {paying:>4} of those reach a speedup above 1");
        println!();
        println!("  refusals at every depth tried:");
        let mut v: Vec<_> = why.into_iter().collect();
        v.sort_by_key(|(k, n)| (std::cmp::Reverse(*n), k.clone()));
        for (k, n) in &v {
            let who = named.get(k).map(|w| w.join(", ")).unwrap_or_default();
            println!("    {n:>4}  {k:<18}  {who}");
        }
        best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        println!();
        println!("  best speedups:");
        for (s, n) in best.iter().take(8) {
            println!("    {s:>12.3e}  {n}");
        }
    }

    /// The refusals are refusals, not silent wrong answers.
    #[test]
    fn a_flame_that_cannot_be_targeted_says_so() {
        let reg = global_registry();
        let view = View::of(8.0, ON_SET, 96, 96);

        // Nonlinear but BOUNDED is no longer a refusal -- that is
        // what the forward bounds bought. The flame enumerates; it is
        // simply not composable, and the kernel walks the word
        // instead of multiplying a matrix.
        let mut curved = gasket();
        curved.transforms[1].set_variation("blur", 0.02);
        let planned = Cylinders::plan(&curved, &reg, view)
            .expect("a bounded transform is enumerable");
        assert!(
            !planned.composable,
            "a flame with a nonlinear map cannot fold a word into one matrix"
        );

        // **Bounded is necessary and not sufficient.** `spherical`'s
        // bound is the inversion's `1/t²` away from the origin and a
        // crude global disc over it, and a transform whose input
        // reaches the origin therefore gets a claim that never
        // shrinks -- so the enumeration has no stopping rule and says
        // so, rather than running to the depth cap. A tighter
        // near-origin bound is the fix, and it is a fact about the
        // bound rather than about the map.
        let mut sph = gasket();
        sph.transforms[1].set_variation("spherical", 0.2);
        assert!(
            Cylinders::plan(&sph, &reg, view).is_err(),
            "a bound that cannot shrink must refuse, not enumerate forever"
        );

        // Nonlinear and UNBOUNDED still refuses, by index and with a
        // reason that names what inside the transform refused.
        let mut wild = gasket();
        wild.transforms[1].set_variation("waves", 1.0);
        match Cylinders::plan(&wild, &reg, view) {
            Err(NoCylinders::Unbounded { index, why }) => {
                assert_eq!(index, 1);
                assert!(why.contains("waves"), "the reason must name it: {why}");
            }
            other => panic!("expected an unbounded refusal, got {other:?}"),
        }

        // A colour-writing variation would plot the right point in
        // the wrong colour, so it is refused rather than approximated.
        let mut dc = gasket();
        dc.transforms[1].set_variation("dc_cube", 1.0);
        assert_eq!(Cylinders::plan(&dc, &reg, view), Err(NoCylinders::ColourNotAffine));

        // Xaos: the first symbol's probability is conditional.
        let mut x = gasket();
        x.xaos = Some(vec![
            vec![1.0, 0.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ]);
        assert_eq!(Cylinders::plan(&x, &reg, view), Err(NoCylinders::Xaos));

        // Expanding: a word's image does not shrink, so there is no
        // stopping rule.
        let mut big = gasket();
        big.transforms[2] = affine(1.2, 0.0, 0.0, 1.2, 0.0, 0.0, 1.0);
        assert!(matches!(
            Cylinders::plan(&big, &reg, view),
            Err(NoCylinders::NotContractive(2))
                | Err(NoCylinders::Unbounded { .. })
                | Err(NoCylinders::NoInvariantBall)
        ));

        // A view nowhere near the attractor.
        let far = View::of(8.0, [40.0, 40.0], 96, 96);
        assert_eq!(Cylinders::plan(&gasket(), &reg, far), Err(NoCylinders::ViewIsEmpty));
    }
}
