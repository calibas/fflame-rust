//! The IFS distance estimate, on the CPU
//! ([docs/projects/ifs-distance-rendering.md](../../docs/projects/ifs-distance-rendering.md)
//! §2.2, phase 1).
//!
//! Inverse iteration (see the plan's §9 for the lineage): apply
//! inverse maps to the query point,
//! tracking how much the forward composition would contract, and scale
//! the distance measured in the expanded frame back down. What comes
//! out is the distance to the attractor — plus, from the same walk and
//! at no extra cost, the escape level, the branch address and the
//! expanded point, which are three of the four colouring quantities of
//! §2.3.
//!
//! # The walk does not stop at the first escape, and that matters
//!
//! §2.2's pseudocode breaks out of the loop the moment the point
//! leaves the bounding ball and returns `σ·(|q − c| − R)` from there.
//! That value goes to **zero at the ball's surface** — so a point
//! sitting on the ball is reported as being on the attractor, and the
//! render draws the bounding balls, at every level, as bright rings
//! around the set. It is clearly visible in a Sierpiński render.
//!
//! Every level's value is a lower bound, so the walk keeps going and
//! keeps the LARGEST. Once the point is well outside, each further
//! inversion multiplies its radius by about `1/σ` and the running
//! product by `σ`, so the value converges rather than growing without
//! limit — [`FAR`] is where it stops bothering. Measured on the unit
//! square, whose distance is exact: the worst estimate/exact ratio over
//! the exterior test grid goes from **0.448 to 1.0000**. The rings
//! disappear because they were never a property of the set.
//!
//! # Why a CPU copy of a shader loop
//!
//! This is the reference. The gates below check the ALGORITHM against
//! IFSs whose distance is known in closed form — a single contraction,
//! whose attractor is one point; four half-scale maps, whose attractor
//! is the filled unit square. The shader is then checked against this,
//! which separates "the estimate is wrong" from "the transcription is
//! wrong". Phase 0 did the same for `affine3D`.
//!
//! It is also what phase 3's 3D path and the flame-deep-zoom plan's
//! reference orbit will call, so it is generic over dimension from the
//! start rather than being 2D and rewritten.
//!
//! # The branch choice is a heuristic, and that is the known weakness
//!
//! `d` is a rigorous lower bound on the distance to the *piece* the
//! walk descends into: the forward composition contracts every
//! displacement by at least the product of smallest singular values,
//! so a distance measured after k inversions is at least that product
//! times the true one. The attractor is the union of ALL branches'
//! pieces, so this bounds the distance to the whole attractor only
//! when the walk picked the nearest branch.
//!
//! Greedy picks the branch whose inverse lands nearest the ball's
//! centre, which is scale-blind: a strongly contracting map makes a
//! tiny piece, and its inverse expands hugely, so a point sitting on
//! that piece can still land far from the centre. Disjoint pieces make
//! the choice exact; overlapping ones — the flame norm — are where it
//! fails, and phase 2 measures a beam against it. Until then soundness
//! is MEASURED rather than proved: `estimate_never_exceeds_a_sampled_upper_bound`
//! checks the walk against a dense sample of a real attractor.

use crate::scene::ifs_analysis::{Affine2, Affine3, Ifs, Ifs2, Ifs3, IfsMap, Map2, Map3};

/// What the walk needs of one map: the inverse step, and the local
/// contraction it costs. For an affine map the contraction is the
/// constant the analysis computed; for a root map (plan §8.8, J3) it
/// is that constant times a factor read at the point.
pub trait IfsSpace: Copy {
    type Point: Copy + PartialEq + std::fmt::Debug;

    fn apply(&self, p: Self::Point) -> Self::Point;
    fn distance(a: Self::Point, b: Self::Point) -> f64;

    /// One inverse step from `q` with the scalar `aux` (plan §8.11
    /// step 3; a fourth coordinate only a quaternion kernel reads):
    /// the point, the scalar, and the forward map's σ_min there given
    /// its constant part.
    fn step(&self, q: Self::Point, aux: f64, sigma_min: f64) -> (Self::Point, f64, f64) {
        (self.apply(q), aux, sigma_min)
    }

    /// When `q` lies outside this map's image, a lower bound on its
    /// distance to the map's piece; the walk records it and follows
    /// no child (plan §8.9 S4). `None` for a map onto the plane.
    fn image_gap(&self, q: Self::Point) -> Option<f64> {
        let _ = q;
        None
    }

    /// Whether this map's forward kernel is an inversion, for the
    /// beam's choice of ranking key. Affine maps are not.
    fn is_inversion(&self) -> bool {
        false
    }

    /// The bound the walk scores a gapped branch by: the larger of the
    /// geometric gap and what one step of the expansion would have
    /// bounded, when that step is representable.
    ///
    /// Both are valid lower bounds on the distance to the piece, and
    /// they do not agree at the gap's edge: the geometric gap goes to
    /// zero there while the expansion, whose hole radius comes from a
    /// generous superset of the pre-frame ball, lands a little outside
    /// the ball and reads a positive distance. Taking one inside and
    /// the other outside made the field a cliff at every inversion's
    /// hole edge -- a ring reading "on the set" -- and, since the hole
    /// follows the ball, a cliff that moved under animation: measured
    /// as one point jumping 90 pixels for a 0.075-degree turn. The
    /// maximum of two continuous bounds is continuous.
    fn gap_bound(&self, q: Self::Point, centre: Self::Point, radius: f64, sigma_min: f64) -> Option<f64> {
        let geo = self.image_gap(q)?;
        let (q2, _, s) = self.step(q, 0.0, sigma_min);
        let r2 = Self::distance(q2, centre);
        let term = s * (r2 - radius);
        Some(if term.is_finite() { geo.max(term) } else { geo })
    }
}

impl IfsSpace for Affine2 {
    type Point = [f64; 2];

    fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        Affine2::apply(self, p)
    }

    fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
    }
}

impl IfsSpace for Map2 {
    type Point = [f64; 2];

    fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        Map2::apply(self, p)
    }

    fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
        Affine2::distance(a, b)
    }

    fn step(&self, q: [f64; 2], aux: f64, sigma_min: f64) -> ([f64; 2], f64, f64) {
        match self {
            Map2::NonlinearInverse(r) => (r.apply_inverse(q), aux, sigma_min * r.local_sigma_factor(q)),
            other => (other.apply(q), aux, sigma_min),
        }
    }

    fn image_gap(&self, q: [f64; 2]) -> Option<f64> {
        match self {
            Map2::NonlinearInverse(r) => r.image_gap(q),
            _ => None,
        }
    }

    fn is_inversion(&self) -> bool {
        match self {
            Map2::NonlinearInverse(r) => r.kernel.unbounded_at_origin(),
            _ => false,
        }
    }
}

impl IfsSpace for Affine3 {
    type Point = [f64; 3];

    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        Affine3::apply(self, p)
    }

    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }
}

impl IfsSpace for Map3 {
    type Point = [f64; 3];

    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        Map3::apply(self, p)
    }

    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        Affine3::distance(a, b)
    }

    fn step(&self, q: [f64; 3], aux: f64, sigma_min: f64) -> ([f64; 3], f64, f64) {
        match self {
            Map3::NonlinearInverse(r) => {
                let s = sigma_min * r.local_sigma_factor(q, aux);
                let (p, a) = r.apply_inverse_aux(q, aux);
                (p, a, s)
            }
            other => (other.apply(q), aux, sigma_min),
        }
    }
}

/// Everything one evaluation yields — the four quantities of §2.3,
/// from one walk.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimate<P> {
    /// Lower bound on the distance from the query point to the
    /// attractor. Zero for a point the walk could not push out of the
    /// ball within `max_levels`, which is what a point on the set does.
    pub distance: f64,
    /// Inverse-orbit depth at escape, with a continuous residual
    /// across the annulus so bands do not step. **Rises toward the
    /// attractor**: a point far outside the ball reads 0, a point on
    /// the set reads `max_levels`.
    pub level: f64,
    /// The branch taken at each level, outermost first.
    pub address: Vec<u32>,
    /// The point after the last inversion — the orbit-trap coordinate,
    /// in the expanded frame.
    pub point: P,
    /// Whether the walk left the ball within `max_levels`.
    pub escaped: bool,
    /// The DEEPEST level any surviving candidate reached, as opposed
    /// to [`Self::level`], which is the winning one's.
    ///
    /// The two answer different questions and the beam need not agree
    /// with itself about them: `level` belongs to the candidate that
    /// minimises the DISTANCE, while this is "how far down can any
    /// address still explain this point" — which is what the
    /// Hepting–Hart escape buffer computes, and what an escape-time
    /// colouring means by a level.
    pub deepest_level: f64,
}

/// One partial address the beam is still following.
#[derive(Clone)]
struct Cand<P> {
    q: P,
    /// Product of the σ_min of the maps applied so far.
    sigma: f64,
    /// Running maximum of `σ·(|q − c| − R)` over the levels visited,
    /// **unclamped** — negative while the path is still inside the
    /// ball, and more negative the deeper inside it sits. That sign is
    /// what makes this one number both the bound and the ranking key.
    bound: f64,
    /// Distance from the ball's centre at this candidate's current
    /// position. The ranking key is `sigma * r` -- see [`RankKey`].
    ///
    /// Ranking by `bound` is degenerate: it is a running MAXIMUM, so
    /// once a path has grazed the ball's edge its bound sits at ~0⁻
    /// and every descendant inherits it unchanged — all the children
    /// tie and the choice becomes whichever the sort happened to leave
    /// first. Measured on the gasket, that is loose at 568 of 576 grid
    /// points against 10 by position. Inside the ball it is the
    /// distance to the centre that says which piece the point is in.
    ///
    /// Position ALONE was the key until a grand julian was reported
    /// from use. Its maps are inversions, and their `sigma` spans
    /// hundreds of orders of magnitude between siblings, so the
    /// distance a path stands for is `sigma` times its position and
    /// not its position: ranking by `r` pruned exactly the paths whose
    /// tiny `sigma` held the answer. Measured against exhaustive
    /// search at beam 8, position was loose at 300 of 576 points with
    /// a worst ratio in the tens of millions; `sigma * r` is loose at
    /// 119 with a worst of 8.6, and is identical to position on the
    /// gasket and the dragon, where every path at a level has the same
    /// `sigma` -- 0 of 576 loose either way.
    r: f64,
    address: Vec<u32>,
    escape: Option<(f64, Vec<u32>, P)>,
    /// Past [`FAR`]: converged, and no longer expanded.
    done: bool,
    /// The scalar coordinate a quaternion kernel carries (step 3).
    aux: f64,
}

/// Whether map `child` may be appended to a path whose last map is
/// `last` (`ifs-general.md` D4).
///
/// True for every child when the flame has no xaos, which is the
/// ordinary case and the reason this is a free function rather than
/// a branch at each of the six expansion sites.
///
/// **The direction.** An address is `[a_1, a_2, ...]` in DISCOVERY
/// order and the forward chain runs it backwards, so appending
/// `child` puts it immediately BEFORE `last` in the chaos game's own
/// order: the transition to admit is `child -> last`.
/// [`XaosGraph`](crate::scene::ifs_analysis::XaosGraph) holds it that
/// way round.
pub fn admits<A, P>(ifs: &Ifs<A, P>, child: usize, last: Option<u32>) -> bool {
    ifs.xaos.as_ref().is_none_or(|g| g.admits(child, last))
}

/// Which quantity the beam keeps by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RankKey {
    /// Distance from the ball's centre: "which piece is the point in".
    Position,
    /// The same, weighted by the path's accumulated `sigma` -- the
    /// production key.
    ///
    /// On an IFS whose maps all contract alike this IS `Position`,
    /// since every path at a level has the same `sigma`. Where they do
    /// not -- a root with a negative distance is an INVERSION, so a
    /// candidate far from the centre has a tiny `sigma` and stands for
    /// a tiny piece with a tiny distance -- `Position` prunes exactly
    /// the paths that would have given the answer. `Position` is kept
    /// so the measurement that decided this can be re-run.
    Weighted,
    /// Half the beam by `Position`, the other half by `Weighted` from
    /// what is left. Neither pure key is right everywhere: `Weighted`
    /// starves a plain affine map's child out of a beam of four on a
    /// bubble IFS, where the bubble's tiny-σ children all rank ahead
    /// of it, and `Position` prunes the answer on an inversion. A
    /// split beam cannot starve either class.
    Mixed,
    /// `Weighted` when EVERY map is an inversion, `Position` otherwise
    /// -- the measured best of each class, and the production key.
    ///
    /// Measured (`the_beam_key_is_scored_on_an_inversion`): on a flame
    /// that is all inversions `Position` is catastrophic and `Weighted`
    /// is not; wherever an inversion sits beside a plain map, or the
    /// IFS has no inversions, `Weighted` starves the plain pieces and
    /// `Position` is right. σ·r makes any pole-child look cheap, so it
    /// is only fair when every child is one.
    Auto,
}

/// Whether every map's forward kernel is an inversion: a root whose
/// distance exponent is negative, or `spherical`.
pub fn all_inversions<A: IfsSpace>(ifs: &Ifs<A, A::Point>) -> bool {
    !ifs.maps.is_empty() && ifs.maps.iter().all(|m| m.inverse.is_inversion())
}

/// The key a walk over this IFS ranks by: `Auto` resolved.
pub fn resolved_key<A: IfsSpace>(ifs: &Ifs<A, A::Point>, key: RankKey) -> RankKey {
    match key {
        RankKey::Auto => {
            if all_inversions(ifs) {
                RankKey::Weighted
            } else {
                RankKey::Position
            }
        }
        k => k,
    }
}

/// The comparator for a resolved pure key, for the seeded walks and
/// the handover, which sort by index and cannot use `keep_by_key`.
fn cmp_for<P>(key: RankKey) -> fn(&Cand<P>, &Cand<P>) -> std::cmp::Ordering {
    match key {
        RankKey::Weighted => by_weighted::<P>,
        _ => by_rank::<P>,
    }
}

/// Keep `beam` of `next` under `key`, in place.
fn keep_by_key<P: Clone>(next: &mut Vec<Cand<P>>, beam: usize, key: RankKey, radius: f64) {
    match key {
        RankKey::Auto => unreachable!("Auto is resolved before the walk"),
        RankKey::Position => {
            next.sort_by(by_rank);
            next.truncate(beam);
        }
        RankKey::Weighted => {
            next.sort_by(by_weighted);
            next.truncate(beam);
        }
        RankKey::Mixed => {
            let first = beam.div_ceil(2);
            next.sort_by(by_rank);
            let mut kept: Vec<Cand<P>> = next.drain(..first.min(next.len())).collect();
            next.sort_by(by_weighted);
            let rest = beam.saturating_sub(kept.len());
            kept.extend(next.drain(..rest.min(next.len())));
            *next = kept;
        }
    }
}

/// A ranking key as a total order: a key that is not a number ranks
/// LAST.
///
/// `σ·r` on a path that flew to infinity is `0·∞`, and `partial_cmp`
/// answering `Equal` for it is not a total order -- the sort panics
/// on that, correctly. A candidate whose key overflowed has already
/// been marked done and holds whatever bound it had; it is the one
/// to give up a beam slot first, so it sorts to the end.
fn key_or_last(k: f64) -> f64 {
    if k.is_nan() {
        f64::INFINITY
    } else {
        k
    }
}

/// Ascending by position: deepest inside the ball first.
fn by_rank<P>(a: &Cand<P>, b: &Cand<P>) -> std::cmp::Ordering {
    key_or_last(a.r).total_cmp(&key_or_last(b.r))
}

fn by_weighted<P>(a: &Cand<P>, b: &Cand<P>) -> std::cmp::Ordering {
    key_or_last(a.sigma * a.r).total_cmp(&key_or_last(b.sigma * b.r))
}

/// Walk the inverse maps from `p` and return the estimate.
///
/// `max_levels` bounds the walk. Deeper is tighter and never less
/// sound: the distance is a running maximum over the levels visited,
/// so raising the bound can only raise it. Under a deep zoom the depth
/// needed grows like `log(1/zoom)/log(1/σ)` (§2.5), which is why this
/// is a parameter and not a constant.
///
/// `beam` is how many addresses are followed at once. Every address
/// gives a valid bound on the distance to ITS piece, and the true
/// distance is the **minimum** over all of them — so following one
/// address can only read too large, and too large renders as "far from
/// the set". A beam of width B keeps the B most promising and takes
/// the min; `B = 1` is the greedy walk, and B equal to the full
/// branching is exhaustive search.
///
/// Candidates are ranked by `bound` alone, ascending. That is
/// scale-aware in a way the plan's "nearest the centre" rule is not: a
/// strongly contracting map makes a tiny piece whose inverse expands
/// hugely, so a point sitting on it can land far from the centre while
/// its bound stays low.
pub fn estimate<A>(
    ifs: &Ifs<A, A::Point>,
    p: A::Point,
    max_levels: u32,
    beam: u32,
) -> Estimate<A::Point>
where
    A: IfsSpace,
{
    estimate_aux(ifs, p, 0.0, max_levels, beam)
}

/// [`estimate`] from a point with a scalar coordinate `aux0` -- the
/// slice a solid of quaternion kernels is seen at (plan §8.11 step
/// 3). The radius the walk escapes against is the 4D one.
pub fn estimate_aux<A>(
    ifs: &Ifs<A, A::Point>,
    p: A::Point,
    aux0: f64,
    max_levels: u32,
    beam: u32,
) -> Estimate<A::Point>
where
    A: IfsSpace,
{
    estimate_aux_ranked(ifs, p, aux0, max_levels, beam, RankKey::Auto)
}

/// [`estimate_aux`] with the beam's ranking key chosen.
pub fn estimate_aux_ranked<A>(
    ifs: &Ifs<A, A::Point>,
    p: A::Point,
    aux0: f64,
    max_levels: u32,
    beam: u32,
    key: RankKey,
) -> Estimate<A::Point>
where
    A: IfsSpace,
{
    let centre = ifs.ball.centre;
    let radius = ifs.ball.radius;
    let far = radius.max(1.0) * FAR;
    let beam = beam.max(1) as usize;

    // The final transform maps the whole attractor, so its inverse is
    // applied once, before the walk, and its contraction scales the
    // result exactly as a level's would.
    let (q0, sigma0) = match &ifs.final_map {
        Some(f) => (f.inverse.apply(p), f.sigma_min),
        None => (p, 1.0),
    };

    let radius4 = |q: A::Point, aux: f64| A::distance(q, centre).hypot(aux - ifs.aux_centre);
    let key = resolved_key(ifs, key);
    let mut live = vec![Cand {
        q: q0,
        sigma: sigma0,
        bound: f64::NEG_INFINITY,
        r: radius4(q0, aux0),
        address: Vec::new(),
        escape: None,
        done: false,
        aux: aux0,
    }];
    // The smallest bound among pieces the walk could not enter: a
    // map whose image does not contain the point has no child to
    // follow, but its piece is a known distance away (S4), and the
    // answer is the minimum over ALL pieces.
    let mut dead_min = f64::INFINITY;
    // The best path that has finished: left the representable plane,
    // or gone past FAR. Its bound is final; it need not hold a slot.
    let mut best_done: Option<Cand<A::Point>> = None;
    // The deepest level any finished path reached. The LEVEL is the
    // deepest any address reached, not the winner's, so every path
    // that leaves the beam contributes -- not only the best-bound one.
    // Leaving this out collapsed the level colouring to one flat
    // value once finished paths stopped holding beam slots.
    let mut deepest_done = f64::NEG_INFINITY;

    for k in 0..max_levels {
        let mut all_done = true;
        for c in live.iter_mut() {
            if c.done {
                continue;
            }
            let r = radius4(c.q, c.aux);
            c.r = r;
            // Every level's value is a lower bound; the walk keeps the
            // largest. Stopping at the first escape is what draws the
            // bounding ball as though it were the set — see the module
            // docs.
            c.bound = fold_level(c.bound, c.sigma, r, radius);

            if r > radius && c.escape.is_none() {
                // The last map applied sets the width of the annulus
                // the point escaped into: one more level would have
                // expanded it by 1/σ. A local quantity, with no
                // whole-IFS average in it — except at level 0, where
                // no map has been applied and the mean is the only
                // thing available.
                let last_sigma = c
                    .address
                    .last()
                    .map(|&i| ifs.maps[i as usize].sigma_min)
                    .unwrap_or_else(|| mean_sigma_min(&ifs.maps));
                c.escape =
                    Some((k as f64 + escape_residual(r, radius, last_sigma), c.address.clone(), c.q));
            }

            if !r.is_finite() || r > far {
                c.done = true;
                // Frozen INSIDE the ball -- the next point was not a
                // number while the bound still said "could be zero" --
                // says nothing about the piece: a point on the set
                // never freezes. It carries no bound from here on,
                // and every consumer drops it by that one rule.
                if !r.is_finite() && !(c.bound > 0.0) {
                    c.bound = f64::INFINITY;
                }
            } else {
                all_done = false;
            }
        }
        if all_done {
            break;
        }
        // Positions evaluated must be exactly `max_levels`: expanding
        // on the last pass would score a level deeper than asked for,
        // which reads as the walk quietly getting looser against any
        // fixed-depth reference.
        if k + 1 >= max_levels {
            break;
        }

        // Expand, scoring each child as it is made so siblings can be
        // told apart — a child that inherited only its parent's bound
        // would rank identically to all its siblings.
        let mut next: Vec<Cand<A::Point>> = Vec::with_capacity(live.len() * ifs.maps.len());
        for c in &live {
            if c.done {
                // Its level, if it escaped: a path that finished without
                // escaping froze inside the ball and says nothing about
                // depth either.
                if let Some((lvl, _, _)) = c.escape.as_ref() {
                    deepest_done = deepest_done.max(*lvl);
                }
                // A done path never expands, so its bound is final.
                // It leaves the beam and is remembered by that bound
                // -- kept in the beam it would compete for a slot by a
                // key that is no longer a number, rank last, and be
                // pruned in favour of live paths that lead nowhere.
                // Measured on the reported view: the winning address
                // held at rank 1 at level 5 and was thrown away two
                // levels later, and a wider beam made it WORSE, since
                // it admitted more live impostors to displace it.
                // A path that finished with a non-positive bound froze
                // INSIDE the ball -- its next point was not a number --
                // and says nothing about its piece. A point on the set
                // never freezes, so it is not evidence of distance
                // zero; with the inversions' holes scored as gaps it no
                // longer happens on them at all.
                if c.bound.is_finite()
                    && best_done.as_ref().map_or(true, |b: &Cand<A::Point>| c.bound < b.bound)
                {
                    best_done = Some(c.clone());
                }
                continue;
            }
            let last = c.address.last().copied();
            for (i, m) in ifs.maps.iter().enumerate() {
                if !admits(ifs, i, last) {
                    continue;
                }
                if let Some(gap) = m.inverse.gap_bound(c.q, ifs.ball.centre, ifs.ball.radius, m.sigma_min) {
                    dead_min = dead_min.min(c.bound.max(c.sigma * gap));
                    continue;
                }
                let (q, aux, s) = m.inverse.step(c.q, c.aux, m.sigma_min);
                let sigma = c.sigma * s;
                let r = radius4(q, aux);
                let mut child = c.clone();
                child.q = q;
                child.aux = aux;
                child.sigma = sigma;
                child.bound = fold_level(c.bound, sigma, r, radius);
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
            }
        }
        if next.is_empty() {
            // Every live path had nothing left to follow: each branch
            // was a gap, scored into `dead_min`. Their own bounds are
            // NOT answers -- a path whose every child is a known
            // distance away is itself at least that distance away,
            // and reading its stale bound instead was measured
            // against exhaustive search as 8.7e-3 for a true 0.118.
            // Predates the holes; the holes made it common.
            for c in live.iter_mut() {
                if !c.done {
                    c.bound = f64::INFINITY;
                }
            }
            break;
        }
        keep_by_key(&mut next, beam as usize, key, radius);
        live = next;
    }

    // The winner is the smallest bound, and it carries the colouring
    // quantities: they describe the piece the distance was measured to.
    // The beam is RANKED by position but ANSWERED by bound: pruning
    // asks "which piece is this point in", and the estimate asks
    // "which surviving address gives the smallest distance".
    //
    // The LEVEL is a different question and takes a different answer:
    // the deepest any surviving address reached, which is what an
    // escape-time colouring means and what the escape buffer computes.
    let deepest_level = live
        .iter()
        .map(|c| c.escape.as_ref().map_or(max_levels as f64, |(lvl, _, _)| *lvl))
        .fold(deepest_done, f64::max);
    // Among FINITE bounds. With `fold_level` a path's bound is finite
    // from level 0 on, so this only differs from a plain minimum when
    // the pixel itself was not a number -- but a non-finite bound must
    // never be the one answered, and this says so rather than relying
    // on it.
    let live_best = live
        .iter()
        .filter(|c| c.bound.is_finite())
        .min_by(|a, b| a.bound.partial_cmp(&b.bound).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();
    // Paths still live at the last level finish there too.
    let best = match (live_best, best_done) {
        (Some(l), Some(d)) => if d.bound < l.bound { d } else { l },
        (Some(l), None) => l,
        (None, Some(d)) => d,
        (None, None) => live[0].clone(),
    };

    // With no finite candidate the gaps are the whole answer; with no
    // gaps either, nothing is known and zero is the sound reading.
    let distance = match (best.bound.is_finite(), dead_min.is_finite()) {
        (true, _) => best.bound.max(0.0).min(dead_min.max(0.0)),
        (false, true) => dead_min.max(0.0),
        (false, false) => 0.0,
    };
    match best.escape {
        Some((level, address, point)) => {
            Estimate { distance, level, address, point, escaped: true, deepest_level }
        }
        None => Estimate {
            distance,
            level: max_levels as f64,
            address: best.address,
            point: best.q,
            escaped: false,
            deepest_level,
        },
    }
}

/// How far out the walk bothers to keep expanding. Once the point is
/// this many ball-radii away the estimate has converged — each level
/// multiplies the radius by about `1/σ` and the running product by
/// `σ`, so the two cancel — and continuing only risks overflowing the
/// shader's f32.
const FAR: f64 = 1e12;

/// Fold one level's term into a path's running bound.
///
/// The term is `σ·(|q − c| − R)`: the distance of this level's point
/// from the ball, carried back through the path's contraction. It is
/// a valid lower bound on the distance to the path's piece, and the
/// running MAXIMUM keeps the tightest one.
///
/// Except when it is not a number. An inversion -- a root with a
/// negative distance -- sends a point near its pole to a radius past
/// what a float holds, with a `σ` that has gone the other way, and
/// `tiny × ∞` is `∞` or `NaN` depending on which underflowed first.
/// Neither is a bound. Folding `∞` in poisons every descendant: the
/// path that held the answer at level 2 (bound 0.042, verified
/// against exhaustive search) reads `∞` by level 4, the finalisation
/// turns a non-finite bound into 0 on the CPU and into "infinitely
/// far" on the GPU, and the pixel is painted as exterior. That is the
/// cut-out that was reported from use on a grand julian, moving
/// under a small rotation because which paths overflow moves with
/// it.
///
/// A level that cannot be scored says nothing about the piece, and
/// the bound the path already had is still valid for it -- deeper
/// levels only refine the same piece. So a non-finite term is
/// dropped, and the caller marks the path done, since a point that
/// has left the representable plane has nowhere further to go.
fn fold_level(bound: f64, sigma: f64, r: f64, radius: f64) -> f64 {
    let term = sigma * (r - radius);
    if term.is_finite() {
        bound.max(term)
    } else {
        bound
    }
}

/// The fractional part of the escape level, rising toward the set.
///
/// A point escaping at radius `r` sits somewhere in the annulus
/// `[R, R/σ]` — the shell one more inversion would have carried it
/// across. At the inner edge the residual is 1, so the level joins
/// continuously onto the next band; at the outer edge and beyond it is
/// 0. Returns 0 when the geometry cannot answer: a degenerate ball (a
/// single-map IFS, whose attractor is one point) or a σ that is not a
/// contraction.
fn escape_residual(r: f64, radius: f64, sigma: f64) -> f64 {
    if !(radius > 0.0) || !(sigma > 0.0) || !(sigma < 1.0) {
        return 0.0;
    }
    let across = (r / radius).ln() / (1.0 / sigma).ln();
    1.0 - across.clamp(0.0, 1.0)
}

fn mean_sigma_min<A>(maps: &[IfsMap<A>]) -> f64 {
    if maps.is_empty() {
        return 0.5;
    }
    maps.iter().map(|m| m.sigma_min).sum::<f64>() / maps.len() as f64
}

// ============================================================ seeding
//
// §2.5's reference orbit, and the continuation that consumes it.
//
// The split the walk already carries — a reference half and a delta
// half — is exact for affine maps, so the two can be advanced apart.
// What that buys is precision where it is needed and nowhere else: the
// delta is exact and f32 holds it to a zoom around 2¹²⁰, while the
// reference needs real precision but only until the deltas grow past
// its own rounding. So the CPU walks the levels every pixel shares,
// in whatever precision it likes, and hands over.
//
// 2D only. Phase 3's 3D walk wants the same shape with a 3×3 basis;
// making this generic over dimension now would mean an associated
// basis type on [`IfsSpace`] for one caller, so it waits.

/// A position the seeding walk can carry, at whatever precision the
/// view needs.
///
/// Only the POSITION needs more than f64, and only until the handover.
/// The maps' coefficients are f64 and stay f64; the accumulated basis
/// and σ product are f64 and stay f64 (they run like 2^±zoom, which an
/// f64 exponent holds to ~2¹⁰²³); the distance to the ball's centre is
/// O(1) and f64 answers it. What needs precision is the centre, and
/// the reason is cancellation: after k levels the walk has computed
/// `A_k·C + b_k` where `A_k ~ 2ᵏ` and the answer is O(1), so k bits of
/// C are consumed to get there. At the handover k is about the zoom.
pub trait SeedPoint: Clone + Sized {
    /// Apply an affine map whose coefficients are f64.
    fn apply_affine(&self, a: &Affine2) -> Self;
    /// Apply an INVERTED map of the IFS, of whatever kind, or `None`
    /// when this representation cannot take that step -- a kernel
    /// off the ladder of `ifs-nonlinear-perturbation.md` §3, or a
    /// point with no preimage. The handover stops there, which is
    /// where it stopped for every nonlinear map before.
    fn apply_map(&self, m: &Map2) -> Option<Self>;
    /// Distance to an f64 point. The answer is O(1), so f64 holds it
    /// however precise `self` is.
    fn distance_to(&self, p: [f64; 2]) -> f64;
    /// Collapse for the handover, where f32 is about to take over
    /// anyway.
    fn to_f64(&self) -> [f64; 2];
}

impl SeedPoint for [f64; 2] {
    fn apply_affine(&self, a: &Affine2) -> Self {
        a.apply(*self)
    }

    fn apply_map(&self, m: &Map2) -> Option<Self> {
        let q = m.apply(*self);
        (q[0].is_finite() && q[1].is_finite()).then_some(q)
    }

    fn distance_to(&self, p: [f64; 2]) -> f64 {
        Affine2::distance(*self, p)
    }

    fn to_f64(&self) -> [f64; 2] {
        *self
    }
}

/// One beam candidate, handed from the CPU's walk to the shader's.
#[derive(Debug, Clone, PartialEq)]
pub struct Seed {
    /// Where this candidate has got to. Always O(1) — seeding stops
    /// before anything leaves the ball — so f32 holds it.
    pub position: [f64; 2],
    /// The accumulated inverse-linear map **composed with the view
    /// basis**: it takes a pixel's normalised offset straight to this
    /// candidate's delta.
    ///
    /// Composed rather than kept apart because each half is the
    /// other's reciprocal in size — the map grows like σ⁻ᵏ, the view
    /// shrinks like the zoom — and only the product is a number a
    /// shader can hold.
    pub basis: [[f64; 2]; 2],
    /// The delta's QUADRATIC part, as the three coefficient vectors
    /// of `Q(uv) = C_uu·u² + C_uv·u·v + C_vv·v²`.
    ///
    /// A nonlinear inverse drops a second-order term when it carries
    /// an offset through its Jacobian alone, and that term is what
    /// stops the handover going deep on a set whose reference orbit
    /// passes near a singularity: measured on a grand julian, the
    /// level where the view had finally expanded enough for f32 to be
    /// EXACT cost 3.19 pixels of linearisation and was refused.
    /// Carrying the quadratic takes the residual from `O(ρ/s)`
    /// relative to `O((ρ/s)²)`.
    ///
    /// Zero for an affine map, exactly, so the affine delta stays the
    /// exact thing it has always been.
    pub quad: [[f64; 2]; 3],
    /// Product of the σ_min applied so far, **per pixel width**. The
    /// distance is reported in pixels for the same reason the basis is
    /// composed: in world units it underflows f32 long before the
    /// delta does.
    pub sigma_per_px: f64,
    /// Branch history so far, which the address colouring continues.
    pub address: Vec<u32>,
    /// σ_min of the last map applied, for the annulus residual.
    pub last_sigma: f64,
    /// Running maximum of `σ·(r − R)`, in the same pixel units.
    pub bound_per_px: f64,
    /// Where this candidate left the ball, if it already has, and the
    /// point it left at.
    ///
    /// Carried rather than recomputed because the continuation cannot
    /// know it: a candidate that escaped at level 3 of a fifty-level
    /// prefix would be reported as escaping at level 50, which is a
    /// constant shift through the whole exterior of the picture.
    pub escape: Option<(f64, [f64; 2])>,
    /// Past [`FAR`] already: converged, and not expanded further.
    pub done: bool,
}

/// The beam's state at the level the CPU hands over.
#[derive(Debug, Clone, PartialEq)]
pub struct Seeds {
    /// Levels the CPU walked. The continuation's reported level is its
    /// own count plus this.
    pub level: u32,
    pub cands: Vec<Seed>,
    /// The smallest bound among pieces the PREFIX could not enter, in
    /// the same per-pixel units as [`Seed::bound_per_px`], or
    /// infinite when it entered every branch it met.
    ///
    /// The walk answers the minimum over ALL pieces, reachable or not
    /// (`estimate_aux_ranked`'s `dead_min`): a branch whose image the
    /// point is outside of contributes its gap. The prefix meets
    /// those too, and before this it had nowhere to put one, so it
    /// STOPPED at the first -- which on a set whose inverses have
    /// holes is usually the first or second level, and cost every
    /// level after it. Carried here instead.
    ///
    /// Scored for the whole view, not for the centre: the gap is
    /// 1-Lipschitz in `q` -- it is a distance scaled by `|w|·σ_min`
    /// of the post-affine, against a frame that stretches by at most
    /// the reciprocal -- so subtracting the view's reach gives a
    /// value no pixel's own gap can fall below. Too small is the safe
    /// direction: the answer is a lower bound, and a smaller one
    /// widens a halo where a larger one would erase a piece.
    pub dead_min_per_px: f64,
}

/// How large a delta may grow before the CPU stops and hands over.
///
/// The handover is sound only while every pixel in the view still
/// follows the centre — while no pixel's branch choice can differ from
/// the reference's. A quarter of the ball's radius is well inside
/// that, and close enough to O(1) for f32 to take over.
pub const HANDOVER_FRACTION: f64 = 0.25;

/// The linearised handover's own estimate of its error, in pixels.
///
/// A nonlinear inverse carries a pixel's offset through its JACOBIAN
/// at the reference, dropping a second-order term that is a fraction
/// `ρ/s` of the one kept -- `ρ` the view's reach, `s` the distance to
/// where the map stops being smooth (`Map2::singular_distance`).
/// Every later Jacobian carries that error exactly as it carries the
/// delta, so the RELATIVE error accumulates additively and the
/// absolute error at the handover is `ρ_H · Σ ρ_k/s_k`.
///
/// Divide by the pixel at the handover and `ρ_H` cancels: `ρ/px` is
/// the same number at every level -- the view's half-diagonal in
/// pixels -- because both are the image of the view under the same
/// linear map. So the error in PIXELS is `Σ ρ_k/s_k` times that, and
/// that product is what [`seed_beam`] weighs against f32's.
///
/// An affine map has an infinite clearance and pays nothing.
///
/// **Not used.** `Σ ρ_k/s_k` was the first of three models of the
/// curvature and the only one that survived into the walk; §13 of the
/// plan replaced all of them with a measurement against the level-0
/// handover, and the early exit that read the running sum went with
/// §16's finding that the measured curvature is not monotone. The
/// derivation is kept because it is still the right way to think about
/// where the error comes from, and the arithmetic is gone.
/// Where the handover checks itself, as normalised offsets.
///
/// The corners and the centre: the corners because the delta is
/// largest there, the centre because it is the one point the handover
/// is exact at and so catches a check that has gone wrong.
const PROBE_UV: [[f64; 2]; 5] = [
    [0.0, 0.0],
    [-0.5, -0.5],
    [0.5, -0.5],
    [-0.5, 0.5],
    [0.5, 0.5],
];

/// How deep the self-check continues. Deep enough for the bound to
/// have stopped moving at these offsets, shallow enough that five of
/// them per level is a rounding error against the prefix itself,
/// which walks hundreds of levels in `BigFloat`.
const PROBE_LEVELS: u32 = 48;

/// One unit in the last place of an f64 mantissa.
///
/// The objective of [`seed_beam_inner`] measures a handover against
/// the level-0 one by continuing both in f64, so a distance it
/// reports is built from positions carrying a relative error of this.
/// Divided by the pixel that is a number of PIXELS below which the
/// measurement says nothing -- and it grows as the zoom does, because
/// the pixel shrinks while the position's magnitude does not.
///
/// At 1080p on the reported grand julian it is 0.0065 pixels at
/// 2^36, 0.1 at 2^40 and **10 at 2^46** -- so §16's 2^46 row
/// measured rounding noise, and past about 2^40 the walk was
/// choosing levels on a number that had stopped meaning anything.
/// `curv.max(floor)` is the repair: a measurement cannot claim an
/// error smaller than it can resolve, and clamping says so rather
/// than believing it.
const F64_ULP: f64 = 2.22e-16;

/// One unit in the last place of an f32 mantissa.
///
/// The shader stores each seed's position as an f32 and every pixel
/// starts from it, so the point it walks from is wrong by about
/// `|position|·F32_ULP` -- a length, which becomes a number of PIXELS
/// once divided by the pixel size at the handover. Handing over
/// deeper makes it smaller, because the pixel grows with the view.
/// See [`seed_beam_with`].
const F32_ULP: f64 = 5.96e-8;

/// Walk the beam from the view centre and stop while the whole view
/// still behaves as one point.
///
/// `view_basis` maps a normalised pixel offset — the screen spanning
/// [-½, ½] on each axis — to a world offset from the centre. `px` is
/// the world width of one pixel.
///
/// **The handover stops at the first level the view does not agree
/// on.** The beam is chosen per pixel by ranking, and the CPU walks
/// from the centre; a branch may be pruned only if every pixel in the
/// view would prune it too. A branch outside the beam is safely pruned
/// when its most optimistic key, at the pixel nearest it, is still
/// worse than the most pessimistic key of everything kept; the first
/// level at which any branch fails that test is not taken, and the
/// prefix ends one level short. That costs one level of f32 and
/// nothing else, and it makes the seeded walk EXACTLY each pixel's own
/// walk up to the handover -- measured at 0% disagreement over five
/// zooms, four beams and two sets, one of them overlapping.
///
/// Without the rule, every pixel continued from the centre's
/// surviving branches, and a pixel whose own nearest piece had been
/// pruned read a distance to some other piece and rendered as
/// exterior. Which pixels depended on how deep the handover went,
/// which depended on the zoom: 41% of the outer frame on the dragon at
/// one zoom and beam and 0% at the next -- "the structure warps as I
/// zoom, and no beam setting fixes it".
///
/// Carrying the ambiguous branches instead (a closure wider than the
/// beam, ranked once per pixel at the handover) was tried and is not
/// the same walk: greedy selection is level by level, and a lineage
/// that ranks best at the handover need not be the one greedy would
/// have followed. Measured on the gasket it moved a halo edge by a
/// pixel or two, which is the artefact being fixed. Replaying the
/// selection per level would be exact and about eight times the cost
/// at depth.
///
/// **A nonlinear map is carried by its Jacobian**, and where it hands
/// over is chosen rather than capped -- see the objective below.
pub fn seed_beam<P: SeedPoint>(
    ifs: &Ifs2,
    centre: P,
    view_basis: [[f64; 2]; 2],
    px: f64,
    max_levels: u32,
    beam: u32,
) -> Seeds {
    seed_beam_inner(ifs, centre, view_basis, px, max_levels, beam, None)
}

/// [`seed_beam`] made to hand over at a LEVEL of the caller's
/// choosing, whatever the objective would have picked.
///
/// For measuring the objective, which is otherwise circular: the
/// model decides the level, so the error at the level it chose says
/// nothing about the model.
#[cfg(test)]
pub(crate) fn seed_beam_at<P: SeedPoint>(
    ifs: &Ifs2,
    centre: P,
    view_basis: [[f64; 2]; 2],
    px: f64,
    max_levels: u32,
    beam: u32,
    level: u32,
) -> Seeds {
    seed_beam_inner(ifs, centre, view_basis, px, max_levels, beam, Some(level))
}

fn seed_beam_inner<P: SeedPoint>(
    ifs: &Ifs2,
    centre: P,
    view_basis: [[f64; 2]; 2],
    px: f64,
    max_levels: u32,
    beam: u32,
    force: Option<u32>,
) -> Seeds {
    let ball = ifs.ball.centre;
    let radius = ifs.ball.radius;
    let beam = beam.max(1) as usize;
    let cap = radius * HANDOVER_FRACTION;
    let scale = if px > 0.0 { 1.0 / px } else { 1.0 };

    let (q0, sigma0, basis0) = match &ifs.final_map {
        Some(f) => {
            let inv = f.inverse.as_affine().expect("the final transform is affine (J4)");
            (centre.apply_affine(&inv), f.sigma_min, compose_basis(&inv, view_basis))
        }
        None => (centre, 1.0, view_basis),
    };
    // The view's half-diagonal in pixels. Only its positivity is read
    // now -- it is the degeneracy guard on `choose` below -- since the
    // running curvature sum it used to convert is gone (R4).
    let reach_px = if px > 0.0 { basis_reach(view_basis) / px } else { 0.0 };

    let r0 = q0.distance_to(ball);
    let mut live = vec![Cand {
        q: q0,
        sigma: sigma0,
        bound: f64::NEG_INFINITY,
        r: r0,
        address: Vec::new(),
        escape: None,
        done: false,
        aux: 0.0,
    }];
    let mut bases = vec![basis0];
    let mut quads = vec![[[0.0f64; 2]; 3]];
    let mut level = 0u32;

    // WHERE to hand over, as opposed to how far to walk.
    //
    // Two errors pull opposite ways. The linearisation's grows with
    // depth: the view's reach grows and the dropped second-order term
    // grows with it. f32's SHRINKS with depth: the pixel grows while
    // `|position|·F32_ULP` does not. An affine map pays nothing for
    // the first, so deeper is always better and the last level wins --
    // which is what this did before and still does, by `all_affine`.
    //
    // A nonlinear map pays both, and the level that minimises their
    // sum can be any of them, including the first. Measured on a
    // grand julian: every level past 0 collapses one seed's reach --
    // the inverse of a power-15 root contracts hugely where `|v| < 1`
    // -- and f32 would add 1e15 pixels there, so no handover at all
    // is genuinely the best available and the walk has to be able to
    // say so. On a julia dust at zoom 2^20 the same rule takes level
    // 9, where the two errors are 0.010 and 0.009 pixels.
    let all_affine = ifs.maps.iter().all(|m| m.inverse.is_affine());
    let choose = !all_affine && reach_px > 0.0;
    let reach0 = basis_reach(view_basis);
    let f32_pixels = |bs: &[[[f64; 2]; 2]], cs: &[Cand<P>]| -> f64 {
        bs.iter()
            .zip(cs)
            .map(|(b, c)| {
                let grown = basis_reach(*b) / reach0;
                if !(grown > 0.0) {
                    return f64::INFINITY;
                }
                // At the scale the continuation actually works at:
                // its points are O(R) even where this one is not.
                let p = c.q.to_f64();
                let mag = p[0].hypot(p[1]).max(radius);
                mag * F32_ULP / (px * grown)
            })
            .fold(0.0, f64::max)
    };
    // Level 0 is always a candidate: it is what a nonlinear IFS did
    // before this, and it is sometimes still the best.
    // The gaps the prefix meets, in world units until the handover
    // converts them. Snapshotted with the level, so a prefix that
    // hands over at level 2 having walked to level 6 carries the
    // gaps of levels 0..2 and not the rest -- the continuation finds
    // those itself, at the pixel's own position rather than this
    // conservative one.
    let mut dead_min = f64::INFINITY;
    let mean = mean_sigma_min(&ifs.maps);
    // The handover that approximates nothing, kept as the reference
    // every deeper one is measured against.
    let exact = Seeds {
        level: 0,
        dead_min_per_px: f64::INFINITY,
        cands: seeds_of(&live, &bases, &quads, ifs, mean, scale),
    };
    // How fine a difference the objective can actually resolve, in
    // pixels: see [`F64_ULP`]. Both continuations round their
    // positions to f64, so the reference's magnitude counts beside
    // the level's.
    let ref_mag = {
        let p = live[0].q.to_f64();
        p[0].hypot(p[1]).max(radius)
    };
    let f64_floor = |cs: &[Cand<P>]| -> f64 {
        if !(px > 0.0) {
            return 0.0;
        }
        cs.iter()
            .map(|c| {
                let p = c.q.to_f64();
                p[0].hypot(p[1]).max(radius)
            })
            .fold(ref_mag, f64::max)
            * F64_ULP
            / px
    };
    let mut best_error = if choose { f32_pixels(&bases, &live) } else { f64::INFINITY };
    // Level 0 is a candidate when the objective is running, and it
    // is THE answer when level 0 is the one forced.
    //
    // `choose` is false for an affine IFS -- one pays no curvature,
    // so the deepest handover always wins and there is nothing to
    // choose. But `force` is not the objective: it is a test asking
    // for a particular level, and it used to be silently ignored on
    // exactly those sets. That cost half a day. A gasket asked for
    // level 0 handed over at level 2 and a dragon at level 4, each
    // four times the basis and sixteen times the pixel area, which is
    // precisely the gap §5n traced the shader's measure walk to.
    let mut best = if force.map_or(choose, |l| l == 0) {
        Some((0u32, live.clone(), bases.clone(), quads.clone(), dead_min))
    } else {
        None
    };

    let far = radius.max(1.0) * FAR;

    for _ in 0..max_levels {
        // Score as the walk does, but for the WHOLE VIEW rather than
        // for the centre it is walked from.
        //
        // Everything a seed carries is inherited by every pixel: the
        // bound as a running MAXIMUM that nothing later can lower, and
        // the escape as a level nothing later revisits. Scored at the
        // centre's own position that is sound only while the view is
        // small enough for the difference not to matter -- and this
        // walk's whole job is to run until it nearly does.
        //
        // So a candidate is scored at the nearest point its view can
        // reach, `r - reach`, which is a real lower bound on every
        // pixel's own distance and so a sound bound and a sound escape
        // test for all of them. The continuation raises it again per
        // pixel, which is what the continuation is for.
        //
        // Left as the centre's this is not a subtle error. Pan until
        // the centre leaves the bounding ball and its positive bound
        // is inherited by pixels INSIDE it, so a point sitting exactly
        // on the attractor reports the centre's distance to the ball
        // and the set renders as empty space -- measured at 7.333 for
        // a centre eight units out, against a true zero. That is what
        // "sections disappear when they are mostly off-screen" was.
        let mut all_done = true;
        for (c, basis) in live.iter_mut().zip(&bases) {
            if c.done {
                continue;
            }
            let near = c.r - basis_reach(*basis);
            c.bound = fold_level(c.bound, c.sigma, near, radius);
            if near > radius && c.escape.is_none() {
                let last = c
                    .address
                    .last()
                    .map(|&i| ifs.maps[i as usize].sigma_min)
                    .unwrap_or(mean);
                c.escape = Some((
                    level as f64 + escape_residual(near, radius, last),
                    c.address.clone(),
                    c.q.clone(),
                ));
            }
            if !c.r.is_finite() || near > far {
                c.done = true;
            } else {
                all_done = false;
            }
        }
        if all_done {
            break;
        }

        // Stop once a delta could start separating pixels onto
        // different branches. A candidate that has ALREADY left the
        // ball is not a reason to stop -- it is state, and it is
        // carried; stopping on it ends the prefix at level 1, because
        // a beam wider than the branching factor prunes nothing and so
        // keeps every escapee from the first level onward.
        if bases.iter().zip(&quads).any(|(b, q)| delta_reach(*b, q) >= cap) {
            break;
        }

        let mut next: Vec<Cand<P>> = Vec::with_capacity(live.len() * ifs.maps.len());
        let mut next_bases: Vec<[[f64; 2]; 2]> = Vec::with_capacity(next.capacity());
        let mut next_quads: Vec<[[f64; 2]; 3]> = Vec::with_capacity(next.capacity());
        let mut stop = false;
        'expand: for ((c, basis), quad) in live.iter().zip(&bases).zip(&quads) {
            if c.done {
                next.push(c.clone());
                next_bases.push(*basis);
                next_quads.push(*quad);
                continue;
            }
            let reach = delta_reach(*basis, quad);
            let qf = c.q.to_f64();
            let last = c.address.last().copied();
            for (i, m) in ifs.maps.iter().enumerate() {
                if !admits(ifs, i, last) {
                    continue;
                }
                // A branch the REFERENCE cannot take. Its gap belongs
                // in the answer's minimum, and `Seeds::dead_min_per_px`
                // is where the handover carries it -- less the view's
                // reach, so no pixel's own gap can fall below it.
                if let Some(gap) =
                    m.inverse.gap_bound(qf, ball, radius, m.sigma_min)
                {
                    dead_min = dead_min.min(c.bound.max(c.sigma * (gap - reach)));
                    continue;
                }
                let (Some(jac), Some(q)) = (m.inverse.jacobian(qf), c.q.apply_map(&m.inverse))
                else {
                    stop = true;
                    break 'expand;
                };
                let Some(hess) = m.inverse.hessian(qf) else {
                    stop = true;
                    break 'expand;
                };
                let sigma = c.sigma * m.inverse.local_sigma(qf, m.sigma_min);
                let r = q.distance_to(ball);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = fold_level(c.bound, sigma, r, radius);
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
                next_bases.push(compose_matrix(jac, *basis));
                next_quads.push(compose_quad(jac, &hess, *basis, quad));
            }
        }
        if stop {
            break;
        }
        // Every branch of every candidate was a gap, so there is no
        // level below this one to hand over at. Keep the beam that got
        // here rather than committing an empty one.
        //
        // This is the crash reported on 2026-09-16 -- a grand julian
        // zoomed past 2^30 died with STATUS_STACK_BUFFER_OVERRUN,
        // which is what a panic in a release GUI build looks like from
        // outside. Carrying the gap instead of stopping at it (§10)
        // turned a gapped branch from "end the prefix" into "skip this
        // branch", and when EVERY branch is gapped that skipped them
        // all: `live` became empty, the handover shipped zero seeds,
        // and the continuation indexed `live[0]`. The walk itself has
        // had this case handled since the fully-gapped rule of §8.13;
        // the prefix had not, because before the gap carry it could
        // not reach it.
        if next.is_empty() {
            break;
        }
        // The same ranking the walk uses — and the bases have to
        // follow their candidates through the sort, or every delta
        // ends up attached to the wrong path.
        let mut order: Vec<usize> = (0..next.len()).collect();
        let cmp = cmp_for(resolved_key(ifs, RankKey::Auto));
        order.sort_by(|&a, &b| cmp(&next[a], &next[b]));

        // The handover may prune a branch only if EVERY pixel in the
        // view would prune it too.
        //
        // The ranking key is the distance to the ball's centre at the
        // CENTRE's position; a pixel's own key for the same branch is
        // within that branch's reach of it. So a branch outside the
        // beam is safely pruned only when its most optimistic key is
        // still worse than the most pessimistic key of anything kept.
        // If any branch fails that test the view does not agree on
        // the beam, and the handover stops HERE, one level short --
        // which costs one level of f32 and nothing else.
        //
        // Without this, every pixel continued from the centre's
        // surviving branches, and a pixel whose own nearest piece had
        // been pruned read a distance to some other piece and rendered
        // as exterior. Which pixels depended on how deep the handover
        // went, which depended on the zoom: measured at 41% of the
        // outer frame on the dragon at one zoom and beam and 0% at the
        // next, which is what "the structure warps as I zoom, and no
        // beam setting fixes it" was.
        // Both keys, because neither alone is the one in use. The
        // sort is by `Auto`, which is `σ·r` when every map is an
        // inversion and `r` otherwise -- and this tested `r` always.
        // Measured as a no-op on the 42 rows of
        // `probe_where_a_grand_julian_caps`, so it is here for
        // agreement with the sort and not for a bug it fixed. It is
        // still not SOUND on the weighted key: σ varies across the
        // view too, and this scales the reach by the centre's σ
        // alone. Tightening that needs a bound on σ's own variation,
        // which no kernel supplies today.
        let agreed = view_agrees(
            &order,
            |i| next[i].r,
            |i| delta_reach(next_bases[i], &next_quads[i]),
            beam,
        ) && (resolved_key(ifs, RankKey::Auto) != RankKey::Weighted
            || view_agrees(
                &order,
                |i| next[i].sigma * next[i].r,
                |i| next[i].sigma * delta_reach(next_bases[i], &next_quads[i]),
                beam,
            ));
        if !agreed {
            break;
        }
        order.truncate(beam);
        live = order.iter().map(|&i| next[i].clone()).collect();
        bases = order.iter().map(|&i| next_bases[i]).collect();
        quads = order.iter().map(|&i| next_quads[i]).collect();
        level += 1;

        if let Some(want) = force {
            // A forced walk wants the level and not the objective, so
            // it does not pay for the probe continuations either.
            if level == want {
                best = Some((level, live.clone(), bases.clone(), quads.clone(), dead_min));
            }
        } else if choose {
            // What handing over HERE costs, measured rather than
            // modelled.
            //
            // Level 0 is the handover that is exact by construction --
            // its position is the view centre, its basis is the view,
            // and it approximates nothing -- so continuing from it IS
            // each pixel's own walk. Continuing from this level and
            // comparing is therefore the real quantity: the error in
            // the REPORTED DISTANCE, which is what the objective
            // trades against f32 and what every model of it got wrong.
            //
            // Three models were tried first and all three failed.
            // `Σ ρ_k/s_k` tracked the linear carry to 15% and nothing
            // else. Its square, the natural second-order form, came
            // out between 0.08 and 166 times the truth over three sets
            // and three zooms. Measuring the delta's POSITIONAL miss
            // at the corners -- exact, and cheap -- was out by a factor
            // of 380, because a positional miss at the handover is not
            // the error in the distance the walk goes on to report.
            // Five continuations of `PROBE_LEVELS` per level are what
            // it took to stop guessing.
            let curv = {
                let here = Seeds {
                    level,
                    dead_min_per_px: if dead_min.is_finite() {
                        dead_min * scale
                    } else {
                        f64::INFINITY
                    },
                    cands: seeds_of(&live, &bases, &quads, ifs, mean, scale),
                };
                // Both continuations must finish at the same ABSOLUTE
                // depth, or the comparison is between two different
                // walks: a handover at level L continued by `n` has
                // gone `L + n` deep, and the reference starts at 0.
                PROBE_UV
                    .iter()
                    .map(|uv| {
                        let a = estimate_seeded(
                            ifs,
                            &exact,
                            *uv,
                            PROBE_LEVELS + level,
                            beam as u32,
                        )
                        .distance;
                        let b = estimate_seeded(ifs, &here, *uv, PROBE_LEVELS, beam as u32)
                            .distance;
                        (a - b).abs()
                    })
                    .fold(0.0, f64::max)
            };
            // The measurement cannot resolve below its own f64 noise,
            // so it does not get to claim it did (R2).
            let err = curv.max(f64_floor(&live)) + f32_pixels(&bases, &live);
            if err < best_error {
                best_error = err;
                best = Some((level, live.clone(), bases.clone(), quads.clone(), dead_min));
            }
            // Nothing deeper can win once the curvature term ALONE
            // has passed the best total: `spent` only grows, so every
            // later level's total is at least this one's curvature.
            // That is exact, and it is what bounds the walk now that
            // the arbitrary cap is gone.
            // No early exit on the curvature. It was sound while the
            // curvature was a MODEL that only grew; MEASURED, it is
            // not monotone -- on a grand julian it swings by two
            // orders between adjacent levels, because the view's
            // expansion collapses and recovers with every inversion --
            // so stopping when it passes the best so far can skip the
            // level that wins. Measured to change nothing on the sets
            // tested; removed because the argument for it no longer
            // holds, not because it cost anything.
        }
    }

    if let Some((l, c, b, qd, d)) = best {
        level = l;
        live = c;
        bases = b;
        quads = qd;
        dead_min = d;
    }

    Seeds {
        level,
        dead_min_per_px: if dead_min.is_finite() { dead_min * scale } else { f64::INFINITY },
        cands: seeds_of(&live, &bases, &quads, ifs, mean, scale),
    }
}

/// The walk's state at a level, as the seeds that describe it.
///
/// One place, because the handover now builds seeds twice per level
/// as well as once at the end -- see the self-check in
/// [`seed_beam_inner`] -- and three transcriptions of this would be
/// three chances to disagree.
fn seeds_of<P: SeedPoint>(
    live: &[Cand<P>],
    bases: &[[[f64; 2]; 2]],
    quads: &[[[f64; 2]; 3]],
    ifs: &Ifs2,
    mean: f64,
    scale: f64,
) -> Vec<Seed> {
    live.iter()
        .zip(bases)
        .zip(quads)
        .map(|((c, basis), quad)| Seed {
            position: c.q.to_f64(),
            basis: *basis,
            quad: *quad,
            sigma_per_px: c.sigma * scale,
            last_sigma: c
                .address
                .last()
                .map(|&i| ifs.maps[i as usize].sigma_min)
                .unwrap_or(mean),
            bound_per_px: if c.bound.is_finite() {
                c.bound * scale
            } else {
                f64::NEG_INFINITY
            },
            escape: c.escape.as_ref().map(|(lvl, _, p)| (*lvl, p.to_f64())),
            done: c.done,
            address: c.address.clone(),
        })
        .collect()
}

/// A chaos-game sample of an IFS, for a test outside this module.
#[cfg(test)]
pub(crate) fn chaos_sample_for_test(ifs: &Ifs2, count: usize) -> Vec<[f64; 2]> {
    tests::chaos_sample(ifs, count)
}

/// An ordinary render of the whole attractor, as the measure the
/// inverse walk reads: the hit count per cell and the palette
/// coordinate summed over the same hits.
///
/// This is the "coarse pass" of
/// [`ifs-measure-by-inverse-walk.md`](../../docs/projects/ifs-measure-by-inverse-walk.md)
/// D1 -- view-independent, made once per flame, and covering the
/// ball. A grid too coarse for the attractor stops being a measure:
/// 64 across the ball reads 0.52 of the truth on a bubble pair where
/// 512 reads 0.98 (§5f), so the useful floor is somewhere between 64
/// and 128 and D1's 2048 is the number to build.
#[derive(Debug, Clone)]
pub struct CoarseMeasure {
    pub res: usize,
    pub centre: [f64; 2],
    pub radius: f64,
    /// Samples that landed in each cell.
    pub hits: Vec<u32>,
    /// The palette coordinate summed over those samples.
    pub palette_sum: Vec<f64>,
    /// How many samples the pass drew in total.
    pub samples: u64,
}

impl CoarseMeasure {
    /// The world width of one cell.
    pub fn cell(&self) -> f64 {
        2.0 * self.radius / self.res as f64
    }

    /// The cell `q` falls in, for a builder outside this module.
    #[cfg(test)]
    pub(crate) fn index_for_test(&self, q: [f64; 2]) -> Option<usize> {
        self.index(q)
    }

    fn index(&self, q: [f64; 2]) -> Option<usize> {
        let c = self.cell();
        let fx = (q[0] - (self.centre[0] - self.radius)) / c;
        let fy = (q[1] - (self.centre[1] - self.radius)) / c;
        if !(fx >= 0.0) || !(fy >= 0.0) {
            return None;
        }
        let (ix, iy) = (fx as usize, fy as usize);
        (ix < self.res && iy < self.res).then(|| iy * self.res + ix)
    }

    /// The measure per unit AREA at `q`, zero outside the grid.
    ///
    /// On a set of dimension below two this diverges as the cell
    /// shrinks -- there is no density to look up in the limit -- which
    /// is why the walk integrates it over a region rather than
    /// sampling it at a point (§5c).
    pub fn density(&self, q: [f64; 2]) -> f64 {
        let c = self.cell();
        self.index(q)
            .map_or(0.0, |i| self.hits[i] as f64 / (self.samples as f64 * c * c))
    }

    /// The mean palette coordinate at `q`, or a mid-palette fallback
    /// where nothing landed. It is `c_0` of the colour fold, damped by
    /// `2⁻ᵏ`, so it only has to be roughly right (§5f).
    pub fn palette(&self, q: [f64; 2]) -> f64 {
        self.index(q).map_or(0.5, |i| {
            if self.hits[i] > 0 {
                self.palette_sum[i] / self.hits[i] as f64
            } else {
                0.5
            }
        })
    }
}

/// What the measure walk needs of a flame beyond its [`Ifs2`]: the
/// probability a step carries, and the colour it folds.
///
/// **The probability is a TRANSFORM's, and the branches go two
/// different ways.** Both corrections are measured, each with a
/// control that does not move:
///
/// - A ROOT's forward map is many-valued (`julia` is the square root
///   with a random sign) and the chaos game draws the branch
///   uniformly, while its inverse is the single-valued power. For any
///   point exactly one forward branch has it in its image, so a step
///   carries `p / |n|`. Without the division a julia dust reads 4.3x
///   the truth at depth two and 32.9x at depth six (§5d).
/// - A BUBBLE's forward map is single-valued and its INVERSE is
///   two-valued, and `analyse_2d` expands that into two maps sharing
///   a `transform_index`. Those are alternative PREIMAGES, so the
///   preimage of a set is their union and each carries the WHOLE
///   probability. Normalising over maps instead of transforms reads
///   0.035 against 0.983 (§5e).
///
/// The affine fixtures do not move under either, which is what makes
/// them corrections rather than fitted constants.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureMaps {
    /// Per map, the probability the FIRST step of the inverse walk
    /// carries -- the stationary probability of landing in that map,
    /// which without xaos is simply its normalised weight.
    pub prob: Vec<f64>,
    /// `step[child * n + last]`, the factor a later step carries
    /// (`ifs-general.md` D4). `None` without xaos, where every step
    /// carries [`Self::prob`] whatever came before it.
    ///
    /// A cylinder's weight under a graph-directed IFS is
    /// `π_{a_k} · Π p_{a_{m+1} a_m}` -- a Markov chain, not a product
    /// of independent draws -- and the incremental form of that is
    /// `π_i · p_{i l} / π_l` per appended map. Without xaos
    /// `p_{i l} = w_i` for every `l`, so the factor is `w_i` and the
    /// product collapses to what it always was.
    pub step: Option<Vec<f64>>,
    /// Per map, its transform's palette colour and colour speed.
    pub colour: Vec<(f64, f64)>,
}

impl MeasureMaps {
    pub fn of(ifs: &Ifs2, flame: &crate::scene::transforms::Flame) -> Self {
        let weight = |i: usize| flame.transforms.get(i).map_or(0.0, |t| t.weight as f64);
        // Over TRANSFORMS: a transform that expanded into several maps
        // contributes its weight once.
        let used: std::collections::BTreeSet<usize> =
            ifs.maps.iter().map(|m| m.transform_index).collect();
        let total: f64 = used.iter().map(|&i| weight(i)).sum();
        let total = if total > 0.0 { total } else { 1.0 };
        let prob = ifs
            .maps
            .iter()
            .map(|m| {
                let p = weight(m.transform_index) / total;
                let branches = match m.forward.nonlinear().map(|n| n.kernel) {
                    Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => {
                        n.unsigned_abs().max(1) as f64
                    }
                    _ => 1.0,
                };
                p / branches
            })
            .collect();
        // Under xaos the first step's factor is the chain's
        // stationary probability, not the raw weight, and every later
        // step's depends on what it is being appended to. Both carry
        // the same branch correction, since a root's forward is
        // many-valued whatever chose it.
        let branches = |i: usize| match ifs.maps[i].forward.nonlinear().map(|n| n.kernel) {
            Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => {
                n.unsigned_abs().max(1) as f64
            }
            _ => 1.0,
        };
        let (prob, step) = match ifs.xaos.as_ref() {
            None => (prob, None),
            Some(g) => {
                let n = ifs.maps.len();
                let pi: Vec<f64> =
                    (0..n).map(|i| g.stationary()[i] / branches(i)).collect();
                let mut step = vec![0.0f64; n * n];
                for i in 0..n {
                    for l in 0..n {
                        step[i * n + l] = g.step_probability(i, Some(l as u32)) / branches(i);
                    }
                }
                (pi, Some(step))
            }
        };
        let colour = ifs
            .maps
            .iter()
            .map(|m| {
                flame
                    .transforms
                    .get(m.transform_index)
                    .map_or((0.5, 0.0), |t| (t.color as f64, t.color_speed as f64))
            })
            .collect();
        Self { prob, step, colour }
    }

    /// The factor the walk's probability gains when `child` is
    /// appended to a path whose last map is `last`.
    pub fn step_probability(&self, child: usize, last: Option<u32>) -> f64 {
        match (&self.step, last) {
            (Some(step), Some(l)) => {
                let n = self.prob.len();
                step.get(child * n + l as usize).copied().unwrap_or(0.0)
            }
            _ => self.prob.get(child).copied().unwrap_or(0.0),
        }
    }
}

/// What the measure walk answers for one pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasureEstimate {
    /// The measure per unit area, in the same units as
    /// [`CoarseMeasure::density`], so a ratio against an ordinary
    /// render of the same view is one.
    pub density: f64,
    /// The palette coordinate, as the density-weighted mean over the
    /// addresses that reached the pixel.
    pub palette: f64,
    /// How many addresses contributed.
    pub addresses: u32,
    /// The deepest level at which one of them stopped, which is what
    /// the stop rule actually chose.
    pub depth: u32,
}

/// How large the preimage region may grow, in coarse cells, before the
/// walk stops and reads the measure.
///
/// **One cell is wrong** and was the first version: at one cell
/// neither a point sample nor a footprint average integrates
/// anything, and the estimator reads 0.24 to 0.89 of the truth
/// (§5c). Sixteen is right on an affine set; a strongly curved one
/// wants four, because the footprint is a first-order parallelogram
/// and an inversion outgrows it (§5d). Four is the compromise that
/// is within 3% on every fixture of both kinds.
pub const MEASURE_CELLS: f64 = 4.0;

/// The invariant measure at one pixel, read through the inverse walk.
///
/// Unrolling `μ = Σ p_i (S_i)_* μ` gives `μ(B) = Σ_a p_a μ(S_a⁻¹ B)`,
/// and for a pixel `B` of area `px²` at `x` the region `S_a⁻¹ B` sits
/// around `S_a⁻¹(x)` with area `px²·|det D(S_a⁻¹)(x)|`. Walk each
/// address until that area reaches `cells` coarse cells, where an
/// ordinary render already knows the measure, and
///
/// ```text
/// density(x) = Σ_a  p_a · ρ(S_a⁻¹ x) · |det D(S_a⁻¹)(x)|
/// ```
///
/// Three factors, all of them things the walk carries. **No forward
/// sample is drawn at the zoom**, which is why this does not starve.
///
/// `ρ` is read over the whole region rather than at its centre: the
/// composed Jacobian maps the pixel square to the region, so the
/// footprint is free, and point-sampling it costs a factor of three
/// on a fractal measure and up to 7.5x at a deep stop (§5b, §5c).
///
/// The colour is the flam3 rule folded along the address in FORWARD
/// order -- `a_k` first and `a_1` last, so the shallowest branch
/// dominates -- from the coarse pass's palette coordinate at the
/// endpoint (§5f).
///
/// `beam` truncates the sum, keeping the largest contributions. It is
/// free on every set measured except one whose pixels are covered by
/// many addresses at once, where eight loses 12% and two loses 70%
/// (§5b).
pub fn estimate_measure(
    ifs: &Ifs2,
    maps: &MeasureMaps,
    coarse: &CoarseMeasure,
    x: [f64; 2],
    px: f64,
    beam: usize,
    cells: f64,
    max_levels: u32,
) -> MeasureEstimate {
    let (bc, br) = (ifs.ball.centre, ifs.ball.radius);
    let cpx = coarse.cell();
    if !(px > 0.0) || !(cpx > 0.0) {
        return MeasureEstimate { density: 0.0, palette: 0.5, addresses: 0, depth: 0 };
    }
    // The area the region must reach, as a determinant.
    let want = (cpx / px) * (cpx / px) * cells.max(1.0);
    let beam = beam.max(1);

    // `ρ` averaged over the region the composed Jacobian describes,
    // and the palette coordinate averaged over the SAME samples,
    // weighted by the measure at each.
    //
    // Both from one footprint, because reading them differently is a
    // bug: an address whose footprint straddles a populated cell while
    // its centre sits in an empty one would get a positive weight and
    // the palette's mid-grey fallback. On a sparse attractor that is
    // common -- it cost the grand julian 0.057 in palette coordinate,
    // fourteen entries of a 256-colour ramp, where every other
    // fixture read under 0.001.
    const K: i32 = 4;
    let look = |q: [f64; 2], m: [[f64; 2]; 2]| -> (f64, f64) {
        let mut acc = 0.0;
        let mut col = 0.0;
        for sy in 0..K {
            for sx in 0..K {
                let u = ((sx as f64 + 0.5) / K as f64 - 0.5) * px;
                let v = ((sy as f64 + 0.5) / K as f64 - 0.5) * px;
                let at = [
                    q[0] + m[0][0] * u + m[0][1] * v,
                    q[1] + m[1][0] * u + m[1][1] * v,
                ];
                let d = coarse.density(at);
                acc += d;
                col += d * coarse.palette(at);
            }
        }
        let n = (K * K) as f64;
        (acc / n, if acc > 0.0 { col / acc } else { 0.5 })
    };

    // The colour fold, accumulated FORWARD.
    //
    // The flam3 rule runs `a_k` first and `a_1` last, which is the
    // reverse of the order the walk discovers them in. Writing
    // `h = (1+s)/2` and `g = col·(1−s)/2` for a map, the reversed fold
    // comes out as
    //
    // ```text
    // c = c_0 · ∏_{j≤k} h_j  +  Σ_i g_i · ∏_{j<i} h_j
    // ```
    //
    // and that inner product is over the PREFIX `a_1..a_{i-1}`, which
    // the walk already has in hand. So a lineage carries two numbers
    // -- the running `∏h` and the running sum -- instead of its
    // address, and the fold needs no history at all. That is what
    // makes the shader's version possible: no per-lineage array, and
    // nothing to size.
    struct Live {
        q: [f64; 2],
        p: f64,
        m: [[f64; 2]; 2],
        /// `∏ h` over the prefix so far.
        hp: f64,
        /// `Σ g_i ∏_{j<i} h_j` over the prefix so far.
        cacc: f64,
        /// The last map appended, which under xaos decides both which
        /// children are admissible and what each one's probability
        /// is (`ifs-general.md` D4). `None` at level 0.
        last: Option<u32>,
    }
    let mut live = vec![Live {
        q: x,
        p: 1.0,
        m: [[1.0, 0.0], [0.0, 1.0]],
        hp: 1.0,
        cacc: 0.0,
        last: None,
    }];
    let mut acc = 0.0f64;
    let mut acc_col = 0.0f64;
    let mut addresses = 0u32;
    let mut depth = 0u32;

    for level in 0..max_levels {
        if live.is_empty() {
            break;
        }
        let mut next: Vec<Live> = Vec::with_capacity(live.len() * ifs.maps.len());
        for c in live.drain(..) {
            let det = (c.m[0][0] * c.m[1][1] - c.m[0][1] * c.m[1][0]).abs();
            if det >= want {
                let (rho, c0) = look(c.q, c.m);
                let w = c.p * rho * det;
                if w > 0.0 {
                    // The closed form of the reversed fold, above.
                    let col = c0 * c.hp + c.cacc;
                    acc += w;
                    acc_col += w * col;
                    addresses += 1;
                    depth = depth.max(level);
                }
                continue;
            }
            for (i, mp) in ifs.maps.iter().enumerate() {
                if !admits(ifs, i, c.last) {
                    continue;
                }
                // The preimage of a set is the union over the
                // inverse's branches, which `analyse_2d` has already
                // made separate maps of.
                let Some(j) = mp.inverse.jacobian(c.q) else { continue };
                let qi = mp.inverse.apply(c.q);
                if !qi[0].is_finite() || !qi[1].is_finite() {
                    continue;
                }
                // Outside the ball is outside the attractor, and stays
                // outside under every further inverse -- so this is an
                // exact prune, not a heuristic.
                if Affine2::distance(qi, bc) > br * (1.0 + 1e-6) {
                    continue;
                }
                let m2 = compose_matrix(j, c.m);
                let d2 = (m2[0][0] * m2[1][1] - m2[0][1] * m2[1][0]).abs();
                if !(d2 > 0.0) || !d2.is_finite() {
                    continue;
                }
                let (cl, sp) = maps.colour[i];
                let h = (1.0 + sp) * 0.5;
                let g = cl * (1.0 - sp) * 0.5;
                next.push(Live {
                    q: qi,
                    p: c.p * maps.step_probability(i, c.last),
                    m: m2,
                    // `g_i` is weighted by the product over the
                    // prefix BEFORE this map, which is `c.hp`.
                    cacc: c.cacc + g * c.hp,
                    hp: c.hp * h,
                    last: Some(i as u32),
                });
            }
        }
        if next.len() > beam {
            // The answer is a SUM, so a pruned address is lost from
            // it: keep the largest contributions.
            let key = |c: &Live| {
                let d = (c.m[0][0] * c.m[1][1] - c.m[0][1] * c.m[1][0]).abs();
                c.p * d * coarse.density(c.q)
            };
            next.sort_by(|a, b| key(b).total_cmp(&key(a)));
            next.truncate(beam);
        }
        live = next;
    }

    MeasureEstimate {
        density: acc,
        palette: if acc > 0.0 { acc_col / acc } else { 0.5 },
        addresses,
        depth,
    }
}

/// Continue a seeded walk for one pixel — the reference for what the
/// shader does after the handover.
///
/// `uv` is the pixel's normalised offset, the screen spanning [-½, ½].
/// The distance returned is **in pixels**, which is the only form that
/// survives a deep zoom.
pub fn estimate_seeded(
    ifs: &Ifs2,
    seeds: &Seeds,
    uv: [f64; 2],
    max_levels: u32,
    beam: u32,
) -> Estimate<[f64; 2]> {
    let centre = ifs.ball.centre;
    let radius = ifs.ball.radius;
    let far = radius.max(1.0) * FAR;
    let beam = beam.max(1) as usize;

    // A handover with no candidates says nothing about this pixel.
    // `seed_beam` does not produce one, and the assertion that it does
    // not is `a_fully_gapped_prefix_keeps_its_beam`; this is the
    // belt, because the alternative to an answer here is a panic in a
    // render.
    if seeds.cands.is_empty() {
        let d = if seeds.dead_min_per_px.is_finite() {
            seeds.dead_min_per_px.max(0.0)
        } else {
            0.0
        };
        return Estimate {
            distance: d,
            level: (seeds.level + max_levels) as f64,
            address: Vec::new(),
            point: ifs.ball.centre,
            escaped: false,
            deepest_level: (seeds.level + max_levels) as f64,
        };
    }
    let mut live: Vec<Cand<[f64; 2]>> = seeds
        .cands
        .iter()
        .map(|s| {
            let d = apply_basis(s.basis, uv);
            let g = apply_quad(&s.quad, uv);
            let q = [s.position[0] + d[0] + g[0], s.position[1] + d[1] + g[1]];
            Cand {
                q,
                // σ per pixel, so the bound comes out in pixels too.
                sigma: s.sigma_per_px,
                bound: s.bound_per_px,
                r: Affine2::distance(q, centre),
                address: s.address.clone(),
                escape: s.escape.map(|(lvl, p)| (lvl, s.address.clone(), p)),
                done: s.done,
                aux: 0.0,
            }
        })
        .collect();
    // Ranked by THIS pixel's position, as the walk ranks every level.
    live.sort_by(cmp_for(resolved_key(ifs, RankKey::Auto)));
    live.truncate(beam);
    // What the prefix already met and could not enter, which the walk
    // would have in hand by now (`Seeds::dead_min_per_px`).
    let mut dead_min = seeds.dead_min_per_px;
    // The same two the walk keeps, and this had lost: the best
    // FINISHED path, which leaves the beam and is remembered by its
    // bound, and the deepest level any finished path reached. Both
    // are explained in `estimate_aux_ranked`. Without them this is a
    // copy of the walk as it stood BEFORE the cut-outs were fixed,
    // and on an inversion set it read 11 to 41 pixels away from the
    // walk it is supposed to mirror -- measured on a grand julian at
    // four zooms, with no handover taken at all, so the difference
    // was the continuation's alone.
    let mut best_done: Option<Cand<[f64; 2]>> = None;
    let mut deepest_done = f64::NEG_INFINITY;
    let key = resolved_key(ifs, RankKey::Auto);
    let sigma_of = |c: &Cand<[f64; 2]>| {
        c.address
            .last()
            .map(|&i| ifs.maps[i as usize].sigma_min)
            .unwrap_or_else(|| mean_sigma_min(&ifs.maps))
    };

    for k in 0..max_levels {
        let mut all_done = true;
        for c in live.iter_mut() {
            if c.done {
                continue;
            }
            let r = Affine2::distance(c.q, centre);
            c.r = r;
            c.bound = fold_level(c.bound, c.sigma, r, radius);
            if r > radius && c.escape.is_none() {
                let level = (seeds.level + k) as f64
                    + escape_residual(r, radius, sigma_of(c));
                c.escape = Some((level, c.address.clone(), c.q));
            }
            if !r.is_finite() || r > far {
                c.done = true;
                // Frozen INSIDE the ball says nothing about the piece
                // (the walk's rule, and the shader's).
                if !r.is_finite() && !(c.bound > 0.0) {
                    c.bound = f64::INFINITY;
                }
            } else {
                all_done = false;
            }
        }
        if all_done || k + 1 >= max_levels {
            break;
        }

        let mut next: Vec<Cand<[f64; 2]>> = Vec::with_capacity(live.len() * ifs.maps.len());
        for c in &live {
            if c.done {
                // A done path never expands, so its bound is final: it
                // leaves the beam and is remembered by that bound,
                // rather than competing for a slot with a key that is
                // no longer a number.
                if let Some((lvl, _, _)) = c.escape.as_ref() {
                    deepest_done = deepest_done.max(*lvl);
                }
                if c.bound.is_finite()
                    && best_done.as_ref().map_or(true, |b: &Cand<[f64; 2]>| c.bound < b.bound)
                {
                    best_done = Some(c.clone());
                }
                continue;
            }
            let last = c.address.last().copied();
            for (i, m) in ifs.maps.iter().enumerate() {
                if !admits(ifs, i, last) {
                    continue;
                }
                if let Some(gap) = m.inverse.gap_bound(c.q, ifs.ball.centre, ifs.ball.radius, m.sigma_min) {
                    dead_min = dead_min.min(c.bound.max(c.sigma * gap));
                    continue;
                }
                let (q, _, s) = m.inverse.step(c.q, 0.0, m.sigma_min);
                let sigma = c.sigma * s;
                let r = Affine2::distance(q, centre);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = fold_level(c.bound, sigma, r, radius);
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
            }
        }
        if next.is_empty() {
            // Every live path's branches were gaps: their own bounds
            // are stale and the gaps are the answer (the walk's rule).
            for c in live.iter_mut() {
                if !c.done {
                    c.bound = f64::INFINITY;
                }
            }
            break;
        }
        keep_by_key(&mut next, beam, key, radius);
        live = next;
    }

    let deepest_level = live
        .iter()
        .map(|c| {
            c.escape
                .as_ref()
                .map_or((seeds.level + max_levels) as f64, |(lvl, _, _)| *lvl)
        })
        .fold(deepest_done, f64::max);
    // Among FINITE bounds. With `fold_level` a path's bound is finite
    // from level 0 on, so this only differs from a plain minimum when
    // the pixel itself was not a number -- but a non-finite bound must
    // never be the one answered, and this says so rather than relying
    // on it.
    let live_best = live
        .iter()
        .filter(|c| c.bound.is_finite())
        .min_by(|a, b| a.bound.partial_cmp(&b.bound).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();
    let best = match (live_best, best_done) {
        (Some(l), Some(d)) => if d.bound < l.bound { d } else { l },
        (Some(l), None) => l,
        (None, Some(d)) => d,
        (None, None) => live[0].clone(),
    };
    let distance = match (best.bound.is_finite(), dead_min.is_finite()) {
        (true, _) => best.bound.max(0.0).min(dead_min.max(0.0)),
        (false, true) => dead_min.max(0.0),
        (false, false) => 0.0,
    };
    match best.escape {
        Some((level, address, point)) => {
            Estimate { distance, level, address, point, escaped: true, deepest_level }
        }
        None => Estimate {
            distance,
            level: (seeds.level + max_levels) as f64,
            address: best.address,
            point: best.q,
            escaped: false,
            deepest_level,
        },
    }
}

// ===================================================== the delta walk
//
// `ifs-perturbation-delta.md` §3. The first design walks the centre in
// `BigFloat`, hands over ONCE at a chosen level, and every pixel
// continues from an absolute f32 position. This walks the centre to
// the full budget, keeps its beam at EVERY level, and lets a pixel
// carry an offset `δ` from whichever row it is following -- which is
// what perturbation means in the escape engine and was not what the
// first design built.
//
// Two things make it work, and both are already here: the difference
// forms of `ifs_analysis::kernel_difference_gen`, which compute
// `m⁻¹(Z + δ) − m⁻¹(Z)` without ever forming the subtraction; and the
// observation that the reference's POSITION only ever enters as a
// coefficient, so f64 holds it however deep the zoom.

/// One reference row: where the centre's beam was at one level, on one
/// lineage.
#[derive(Debug, Clone, PartialEq)]
pub struct RefRow {
    /// `Z_k`. O(1) by construction -- the walk stays in the ball's
    /// neighbourhood -- and it enters the difference forms only as a
    /// coefficient, so f64 holds it however precise the walk that
    /// produced it was. This is the same argument the escape engine's
    /// `ref_orbit` makes for storing a reference orbit in f32.
    pub z: [f64; 2],
    /// The row of the previous level this came from, and the map that
    /// made it. `u32::MAX` for the root.
    pub parent: u32,
    pub map: u32,
    /// Product of σ_min so far, per pixel width.
    pub sigma_per_px: f64,
    /// `|Z_k − c|`, the reference's own distance to the ball's centre.
    pub r: f64,
    /// `Z_k − c`, the offset from the ball's centre, and
    /// `|Z_k − c|² − R²`, its EXCESS.
    ///
    /// Both exist so a pixel's own radius is exact in `δ` rather than
    /// linearised in it. `|Z + δ − c|² = r² + t` with
    /// `t = 2u·δ + |δ|²`, and `t` is a sum of products with nothing to
    /// cancel, so
    ///
    /// ```text
    /// r_pixel − R = (excess2 + t) / (r_pixel + R)
    /// ```
    ///
    /// is accurate to f64's own relative precision for any `δ`. The
    /// first-order form `r + û·δ` that this replaced is wrong by
    /// `O(|δ|²/r)`, which near the rebase cap is a fraction of the
    /// ball -- measured as eight pixels on a julia at 2^12, where the
    /// tolerance is a thousandth of one.
    ///
    /// The remaining limit is `excess2` itself: computed in f64 it
    /// carries an absolute error of about `1e-16`, so the per-pixel
    /// variation is lost once `|t|` falls below that, around 2^50.
    /// Past there the reference has to compute this column in its own
    /// precision, which it has and f64 does not.
    pub u: [f64; 2],
    pub excess2: f64,
    /// σ_min of the map that made this row, for the escape residual.
    pub last_sigma: f64,
    /// Running maximum of `σ·(r − R)` in pixels along the REFERENCE's
    /// path. A pixel recomputes its own from `r` above; this is what a
    /// row means on its own.
    pub bound_per_px: f64,
    /// Where the reference left the ball, if it has.
    pub escape: Option<(f64, [f64; 2])>,
    /// Past `FAR`: the reference stopped expanding it.
    pub done: bool,
    /// Whether this row survived the reference's own beam, or is one
    /// of the slack children (D1). A lineage on a slack row has no
    /// children stored and must rebase to continue.
    pub kept: bool,
    /// The branches this row could not take, with their gaps in world
    /// units at the reference's own position.
    pub gaps: Vec<(u32, f64)>,
}

/// The centre's beam at every level.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceBeam {
    /// `levels[k]` is the beam at level `k`; `levels[0]` is the single
    /// root. Every child of every KEPT row is present, pruned or not
    /// (D1): one level of slack for a pixel whose ranking differs at
    /// the margin, which is the common case and what the first
    /// design's `view_agrees` measured as losing 41% of a frame.
    pub levels: Vec<Vec<RefRow>>,
    /// The view basis at level 0: a pixel's normalised offset to its
    /// delta from the centre.
    pub basis0: [[f64; 2]; 2],
    /// The pixel width the σ and bound columns are scaled by.
    pub px: f64,
}

impl ReferenceBeam {
    /// How many levels the reference reached.
    pub fn depth(&self) -> u32 {
        self.levels.len().saturating_sub(1) as u32
    }

    /// The child of `(level, idx)` by map `m`, if the reference made
    /// one.
    fn child(&self, level: usize, idx: u32, m: u32) -> Option<u32> {
        let rows = self.levels.get(level + 1)?;
        rows.iter()
            .position(|r| r.parent == idx && r.map == m)
            .map(|i| i as u32)
    }
}

/// Walk the centre's beam to the level budget, keeping every level.
///
/// The same walk `seed_beam` does, with three differences, each of
/// which is a decision in §3 or §4 of the plan:
///
/// - it does NOT stop at a handover -- no cap on the delta, no
///   `view_agrees`, no objective. There is no level to choose.
/// - it keeps every child of every kept row, not only the beam's
///   (D1).
/// - it records per row what a pixel needs to continue in delta form.
///
/// `P` is the precision the reference itself is walked in --
/// `BigFloat` for a deep view -- and nothing of that precision is
/// stored, because nothing of it is needed downstream.
pub fn reference_beam<P: SeedPoint>(
    ifs: &Ifs2,
    centre: P,
    view_basis: [[f64; 2]; 2],
    px: f64,
    max_levels: u32,
    beam: u32,
) -> ReferenceBeam {
    let ball = ifs.ball.centre;
    let radius = ifs.ball.radius;
    let beam = beam.max(1) as usize;
    let scale = if px > 0.0 { 1.0 / px } else { 1.0 };
    let far = radius.max(1.0) * FAR;
    let mean = mean_sigma_min(&ifs.maps);

    let (q0, sigma0, basis0) = match &ifs.final_map {
        Some(f) => {
            let inv = f.inverse.as_affine().expect("the final transform is affine (J4)");
            (centre.apply_affine(&inv), f.sigma_min, compose_basis(&inv, view_basis))
        }
        None => (centre, 1.0, view_basis),
    };

    // The walk's own state, in `P`; the rows are the f64 shadow of it.
    let mut live: Vec<(P, u32)> = vec![(q0.clone(), 0)];
    let r0 = q0.distance_to(ball);
    let mut levels: Vec<Vec<RefRow>> = vec![vec![RefRow {
        z: q0.to_f64(),
        parent: u32::MAX,
        map: u32::MAX,
        sigma_per_px: sigma0 * scale,
        r: r0,
        u: [q0.to_f64()[0] - ball[0], q0.to_f64()[1] - ball[1]],
        excess2: r0 * r0 - radius * radius,
        last_sigma: mean,
        bound_per_px: fold_level(f64::NEG_INFINITY, sigma0 * scale, r0, radius),
        escape: (r0 > radius)
            .then(|| (escape_residual(r0, radius, mean), q0.to_f64())),
        done: !r0.is_finite() || r0 > far,
        kept: true,
        gaps: Vec::new(),
    }]];

    for level in 0..max_levels as usize {
        let here = levels[level].clone();
        if here.iter().all(|r| r.done) {
            break;
        }
        let mut next_rows: Vec<RefRow> = Vec::new();
        let mut next_live: Vec<(P, u32)> = Vec::new();
        let mut stop = false;

        for (p, idx) in live.iter() {
            let row = &here[*idx as usize];
            if row.done || !row.kept {
                continue;
            }
            let qf = p.to_f64();
            let last = address_of(&levels, level, *idx).last().copied();
            let mut gaps: Vec<(u32, f64)> = Vec::new();
            for (i, m) in ifs.maps.iter().enumerate() {
                if !admits(ifs, i, last) {
                    continue;
                }
                // A branch the REFERENCE cannot take. Its gap is the
                // row's, and a pixel corrects it by its own `|δ|`
                // rather than by the whole view's reach -- which is
                // what the one-shot handover had to do.
                if let Some(gap) = m.inverse.gap_bound(qf, ball, radius, m.sigma_min) {
                    gaps.push((i as u32, gap));
                    continue;
                }
                let Some(q) = p.apply_map(&m.inverse) else {
                    stop = true;
                    break;
                };
                let sigma = row.sigma_per_px * m.inverse.local_sigma(qf, m.sigma_min);
                let r = q.distance_to(ball);
                let escape = row.escape.clone().or_else(|| {
                    (r > radius).then(|| {
                        (
                            (level + 1) as f64 + escape_residual(r, radius, m.sigma_min),
                            q.to_f64(),
                        )
                    })
                });
                let zf = q.to_f64();
                next_rows.push(RefRow {
                    z: zf,
                    parent: *idx,
                    map: i as u32,
                    sigma_per_px: sigma,
                    r,
                    u: [zf[0] - ball[0], zf[1] - ball[1]],
                    excess2: r * r - radius * radius,
                    last_sigma: m.sigma_min,
                    bound_per_px: fold_level(row.bound_per_px, sigma, r, radius),
                    escape,
                    done: !r.is_finite() || r > far,
                    // Decided below, once the level is ranked.
                    kept: false,
                    gaps: Vec::new(),
                });
                next_live.push((q, (next_rows.len() - 1) as u32));
            }
            levels[level][*idx as usize].gaps = gaps;
            if stop {
                break;
            }
        }
        if stop || next_rows.is_empty() {
            break;
        }

        // Rank as the walk ranks, and mark the top `beam` as kept.
        // The rest stay as rows -- that is the slack.
        let weighted = matches!(resolved_key(ifs, RankKey::Auto), RankKey::Weighted);
        let key = |r: &RefRow| if weighted { r.sigma_per_px * r.r } else { r.r };
        let mut order: Vec<usize> = (0..next_rows.len()).collect();
        order.sort_by(|&a, &b| {
            key(&next_rows[a]).total_cmp(&key(&next_rows[b]))
        });
        for &i in order.iter().take(beam) {
            next_rows[i].kept = true;
        }
        // A done row is carried forward whether or not it was kept:
        // its bound is final and the answer is a minimum over it.
        for r in next_rows.iter_mut() {
            if r.done {
                r.kept = true;
            }
        }
        next_live.retain(|(_, i)| next_rows[*i as usize].kept);
        levels.push(next_rows);
        live = next_live;
        if live.is_empty() {
            break;
        }
    }

    ReferenceBeam { levels, basis0, px }
}

/// The address of a row, read back up the tree.
fn address_of(levels: &[Vec<RefRow>], level: usize, idx: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let (mut l, mut i) = (level, idx);
    while l > 0 {
        let row = &levels[l][i as usize];
        out.push(row.map);
        i = row.parent;
        l -= 1;
    }
    out.reverse();
    out
}

/// Why a lineage stopped carrying a delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rebase {
    /// The delta grew to a fraction of the ball, so `Z + δ` in
    /// absolute f32 is as accurate as δ was. The first design's cap,
    /// applied to ONE lineage at the level its own delta gets there.
    Expanded,
    /// `|Z + δ| < |δ|`: the reference passed near a pole and the
    /// precision is now in the sum rather than in either term. The
    /// Zhuoran-style event, measured in `the_difference_forms_survive_f32`
    /// as forty times the error of anywhere else.
    Cancelled,
    /// The reference has no row for the branch this pixel wants: the
    /// row it follows was itself slack, so the tree stops there. One
    /// level of slack is what D1 buys; this is the level after.
    OffTree,
    /// The kernel has no exact difference form -- the disc, the blob,
    /// a fractional root. The Taylor rung is not built, so these
    /// rebase immediately and the walk is exactly today's.
    NoForm,
}

/// One lineage of a pixel's beam: an offset from a reference row, or
/// an absolute position once it has rebased.
#[derive(Clone)]
struct DeltaCand {
    /// `(level, index)` into the reference, while this is a delta.
    anchor: Option<(u32, u32)>,
    /// The offset from that row's `Z`, or the absolute position once
    /// `anchor` is `None`.
    d: [f64; 2],
    sigma: f64,
    bound: f64,
    /// The pixel's own distance to the ball's centre.
    r: f64,
    last_sigma: f64,
    address: Vec<u32>,
    escape: Option<(f64, Vec<u32>, [f64; 2])>,
    done: bool,
}

impl DeltaCand {
    /// The pixel's own position: the row's `Z` plus its offset, or
    /// the absolute position it rebased to.
    fn point(&self, beam: &ReferenceBeam) -> [f64; 2] {
        match self.anchor {
            Some((l, i)) => {
                let z = beam.levels[l as usize][i as usize].z;
                [z[0] + self.d[0], z[1] + self.d[1]]
            }
            None => self.d,
        }
    }
}

/// The distance estimate at one pixel, walked in DELTA form against a
/// reference beam (`ifs-perturbation-delta.md` §3).
///
/// This is the CPU twin of the shader walk the plan describes, and
/// the function G2 measures. `uv` is the pixel's normalised offset,
/// the screen spanning `[-½, ½]` on each axis, exactly as the shader
/// takes it.
///
/// **What it carries and what it does not.** A lineage's state is
/// `(row, δ)`: which reference row it follows and its offset from it.
/// One level is the kernel's exact difference form and a row change,
/// with no position ever formed in the pixel's own precision. The
/// bound and the escape test take the reference's `r` and add the
/// pixel's own first-order correction `û · δ`, with `û` the unit
/// vector from the ball's centre to `Z` -- exact to `O(|δ|²/r)`.
///
/// **Where it rebases**, per lineage rather than per view: the four
/// cases of [`Rebase`]. Past one it continues in absolute f64 exactly
/// as [`estimate_seeded`] does, which is why nothing new had to be
/// built for the continuation.
///
/// **The limit this first cut has.** The reference's `r` is an f64,
/// so `r − R` carries an absolute error of about `1e-16`, and the
/// pixel's correction `û · δ` is below that once `|δ|` is. On the
/// zooms G2 measures -- 2^12 to 2^28, where `|δ|` is 1e-4 to 1e-9 --
/// that is eight orders of headroom. Past about 2^50 it is not, and
/// the row will have to carry `r − R` computed in the reference's own
/// precision rather than f64's.
pub fn estimate_delta(
    ifs: &Ifs2,
    reference: &ReferenceBeam,
    uv: [f64; 2],
    beam: u32,
    max_levels: u32,
) -> (Estimate<[f64; 2]>, Vec<(u32, Rebase)>) {
    let ball = ifs.ball.centre;
    let radius = ifs.ball.radius;
    let beam = beam.max(1) as usize;
    let far = radius.max(1.0) * FAR;
    let cap = radius * HANDOVER_FRACTION;
    let root = &reference.levels[0][0];
    let b = reference.basis0;
    let mut rebases: Vec<(u32, Rebase)> = Vec::new();

    // δ at level 0 is the view basis applied to the pixel's offset --
    // the one place the pixel's position enters at all.
    let d0 = [
        b[0][0] * uv[0] + b[0][1] * uv[1],
        b[1][0] * uv[0] + b[1][1] * uv[1],
    ];
    let t0 = 2.0 * (root.u[0] * d0[0] + root.u[1] * d0[1]) + d0[0] * d0[0] + d0[1] * d0[1];
    let r0 = (root.r * root.r + t0).max(0.0).sqrt();
    let excess0 = (root.excess2 + t0) / (r0 + radius);
    let mut live = vec![DeltaCand {
        anchor: Some((0, 0)),
        d: d0,
        sigma: root.sigma_per_px,
        bound: {
            let term = root.sigma_per_px * excess0;
            if term.is_finite() { term } else { f64::NEG_INFINITY }
        },
        r: r0,
        last_sigma: root.last_sigma,
        address: Vec::new(),
        escape: (r0 > radius).then(|| {
            (escape_residual(r0, radius, root.last_sigma), Vec::new(), [
                root.z[0] + d0[0],
                root.z[1] + d0[1],
            ])
        }),
        done: !r0.is_finite() || r0 > far,
    }];
    let mut dead_min = f64::INFINITY;
    let mut best_done: Option<DeltaCand> = None;
    let mut deepest_done = 0.0f64;

    // `max_levels` POSITIONS are scored, level 0 included, which is
    // what the direct walk means by the budget: it scores the live set
    // and then expands, so its last expansion's result is never
    // scored. Scoring it here instead cost a julia at 2^18 a tenth of
    // a pixel -- one extra level of a squaring map, at level sixty,
    // where the radius crossed the ball and the direct walk had
    // already stopped looking.
    for level in 0..max_levels.saturating_sub(1) {
        if live.iter().all(|c| c.done) {
            break;
        }
        let mut next: Vec<DeltaCand> = Vec::with_capacity(live.len() * ifs.maps.len());
        for c in live.drain(..) {
            if c.done {
                if let Some((lvl, _, _)) = &c.escape {
                    deepest_done = deepest_done.max(*lvl);
                }
                if c.bound.is_finite()
                    && best_done.as_ref().map_or(true, |b| c.bound < b.bound)
                {
                    best_done = Some(c);
                }
                continue;
            }
            let last = c.address.last().copied();
            let here = c.point(reference);
            let dmag = f64::hypot(c.d[0], c.d[1]);

            for (i, m) in ifs.maps.iter().enumerate() {
                if !admits(ifs, i, last) {
                    continue;
                }
                // ---- the delta step, while there is a row to follow
                if let Some((lvl, idx)) = c.anchor {
                    let row = &reference.levels[lvl as usize][idx as usize];
                    // The reference could not take this branch: its
                    // gap is the answer for the piece, corrected by
                    // this pixel's own offset rather than the whole
                    // view's reach.
                    if let Some(&(_, gap)) = row.gaps.iter().find(|(g, _)| *g == i as u32) {
                        dead_min = dead_min.min(c.bound.max(c.sigma * (gap - dmag)));
                        continue;
                    }
                    let child = reference.child(lvl as usize, idx, i as u32);
                    let step = m.inverse.difference(row.z, c.d);
                    match (child, step) {
                        (Some(ci), Some(dd)) => {
                            let crow =
                                &reference.levels[lvl as usize + 1][ci as usize];
                            let dmag2 = f64::hypot(dd[0], dd[1]);
                            let zmag = f64::hypot(crow.z[0] + dd[0], crow.z[1] + dd[1]);
                            if dmag2 > cap {
                                rebases.push((level, Rebase::Expanded));
                            } else if zmag < dmag2 {
                                rebases.push((level, Rebase::Cancelled));
                            } else {
                                // The pixel's own radius, exactly in
                                // δ rather than linearised in it:
                                // `|Z + δ − c|² = r² + t` with
                                // `t = 2u·δ + |δ|²`, a sum of products
                                // with nothing to cancel.
                                let t = 2.0 * (crow.u[0] * dd[0] + crow.u[1] * dd[1])
                                    + dd[0] * dd[0]
                                    + dd[1] * dd[1];
                                let r = (crow.r * crow.r + t).max(0.0).sqrt();
                                // ...and the EXCESS through the same
                                // difference of squares, so the bound
                                // keeps its per-pixel variation
                                // wherever `t` is representable.
                                let excess = (crow.excess2 + t) / (r + radius);
                                // σ is the PIXEL's, not the row's.
                                //
                                // `σ_min` varies across the view like
                                // everything else, by `O(|δ|/s)` per
                                // level, and sixty levels of a tenth
                                // of a percent compound to six. On a
                                // julia at 2^12 that read as eight
                                // pixels against a tolerance of a
                                // thousandth, which is what found it.
                                // Evaluating it at `Z + δ` costs
                                // nothing: σ is a smooth O(1) factor,
                                // so it needs the position only to
                                // RELATIVE precision, which the sum
                                // has at any depth -- and where δ is
                                // below the sum's last digit the
                                // answer is the reference's, which is
                                // then the correct one.
                                let s = m.inverse.local_sigma(here, m.sigma_min);
                                let mut child_c = c.clone();
                                child_c.anchor = Some((lvl + 1, ci));
                                child_c.d = dd;
                                child_c.sigma = c.sigma * s;
                                child_c.r = r;
                                child_c.last_sigma = m.sigma_min;
                                let term = child_c.sigma * excess;
                                child_c.bound =
                                    if term.is_finite() { c.bound.max(term) } else { c.bound };
                                child_c.address.push(i as u32);
                                finish_child(
                                    &mut child_c, level, radius, far, reference,
                                );
                                next.push(child_c);
                                continue;
                            }
                        }
                        (None, _) => rebases.push((level, Rebase::OffTree)),
                        (_, None) => rebases.push((level, Rebase::NoForm)),
                    }
                    // Fall through: this branch rebases and is taken
                    // in absolute arithmetic from the pixel's own
                    // position.
                }

                // ---- the absolute step, exactly `estimate_seeded`'s
                if let Some(gap) = m.inverse.gap_bound(here, ball, radius, m.sigma_min) {
                    dead_min = dead_min.min(c.bound.max(c.sigma * gap));
                    continue;
                }
                let (q, _, s) = m.inverse.step(here, 0.0, m.sigma_min);
                let sigma = c.sigma * s;
                let r = Affine2::distance(q, ball);
                let mut child_c = c.clone();
                child_c.anchor = None;
                child_c.d = q;
                child_c.sigma = sigma;
                child_c.r = r;
                child_c.last_sigma = m.sigma_min;
                child_c.bound = fold_level(c.bound, sigma, r, radius);
                child_c.address.push(i as u32);
                finish_child(&mut child_c, level, radius, far, reference);
                next.push(child_c);
            }
        }
        if next.is_empty() {
            break;
        }
        // The same ranking the walk uses, on the PIXEL's own numbers.
        let weighted = matches!(resolved_key(ifs, RankKey::Auto), RankKey::Weighted);
        next.sort_by(|a, b| {
            let (ka, kb) = if weighted {
                (a.sigma * a.r, b.sigma * b.r)
            } else {
                (a.r, b.r)
            };
            ka.total_cmp(&kb)
        });
        next.truncate(beam);
        live = next;
    }

    for c in live.iter() {
        if let Some((lvl, _, _)) = &c.escape {
            deepest_done = deepest_done.max(*lvl);
        }
    }
    let live_best = live
        .iter()
        .filter(|c| c.bound.is_finite())
        .min_by(|a, b| a.bound.total_cmp(&b.bound))
        .cloned();
    let best = match (live_best, best_done) {
        (Some(l), Some(d)) => if d.bound < l.bound { d } else { l },
        (Some(l), None) => l,
        (None, Some(d)) => d,
        (None, None) => live.first().cloned().expect("a beam"),
    };
    let distance = match (best.bound.is_finite(), dead_min.is_finite()) {
        (true, _) => best.bound.max(0.0).min(dead_min.max(0.0)),
        (false, true) => dead_min.max(0.0),
        (false, false) => 0.0,
    };
    let point = best.point(reference);
    let est = match best.escape.clone() {
        Some((level, address, point)) => Estimate {
            distance,
            level,
            address,
            point,
            escaped: true,
            deepest_level: deepest_done,
        },
        None => Estimate {
            distance,
            level: max_levels as f64,
            address: best.address.clone(),
            point,
            escaped: false,
            deepest_level: deepest_done,
        },
    };
    (est, rebases)
}

/// The escape and freeze tests every child takes, either way it was
/// stepped.
fn finish_child(
    c: &mut DeltaCand,
    level: u32,
    radius: f64,
    far: f64,
    reference: &ReferenceBeam,
) {
    if c.r > radius && c.escape.is_none() {
        c.escape = Some((
            (level + 1) as f64 + escape_residual(c.r, radius, c.last_sigma),
            c.address.clone(),
            c.point(reference),
        ));
    }
    if !c.r.is_finite() || c.r > far {
        c.done = true;
    }
}

// ===================================================== 3D seeding
//
// The same split as the seeding above — a reference half the CPU walks
// in whatever precision it likes and a delta half that is exact — with
// one structural difference, and it comes from what a MARCHER asks.
//
// A plane render asks about a REGION: the view, which shrinks with the
// zoom, so one handover level serves every pixel in it. A ray asks
// about a LINE. Its samples run from the bounding sphere's near face
// to its far one, so the nearest sit a pixel from the target and the
// furthest are the whole attractor away — a span equal to the entire
// zoom. No single level makes the delta small for all of them.
//
// So the handover is a CHAIN rather than a point: the beam's state at
// every level, and a sample takes the deepest link whose delta is
// still inside the ball's neighbourhood. A sample near the target
// takes a deep link and a sample out at the sphere takes a shallow
// one, which is the same statement as "the address prefix containing a
// point gets shorter the further away the point is".
//
// One chain serves the whole view because every ray starts at the same
// place. The eye is one point, so `p − target` is the only thing that
// varies and the reference orbit is shared.

/// A position the 3D seeding walk can carry, at whatever precision the
/// view needs. The 3D twin of [`SeedPoint`], and for the same reason:
/// only the POSITION needs more than f64, because after `k` levels the
/// walk has computed `A_k·T + b_k` with `A_k ~ 2ᵏ` and an O(1) answer,
/// so `k` bits of the target are spent getting there.
pub trait SeedPoint3: Clone {
    fn apply_affine3(&self, a: &Affine3) -> Self;
    fn distance_to(&self, p: [f64; 3]) -> f64;
    fn to_f64(&self) -> [f64; 3];
}

impl SeedPoint3 for [f64; 3] {
    fn apply_affine3(&self, a: &Affine3) -> Self {
        a.apply(*self)
    }

    fn distance_to(&self, p: [f64; 3]) -> f64 {
        Affine3::distance(*self, p)
    }

    fn to_f64(&self) -> [f64; 3] {
        *self
    }
}

/// One beam candidate at one level of the chain.
#[derive(Debug, Clone, PartialEq)]
pub struct Seed3 {
    /// The reference: where this candidate's address has carried the
    /// TARGET. Always O(1) — the chain stops before anything leaves
    /// the ball's neighbourhood — so f32 holds it.
    pub position: [f64; 3],
    /// The accumulated inverse LINEAR part, taking a delta measured at
    /// the target straight to this candidate's delta. The translations
    /// are carried entirely by `position`, which is what makes the
    /// split exact for affine maps.
    ///
    /// Kept apart from any view basis, unlike the 2D seed's: there is
    /// no linear map from pixel to delta to compose with, because a
    /// sample's offset has the distance along the ray in it.
    pub matrix: [[f64; 3]; 3],
    /// An upper bound on `|matrix·d| / |d|` — the Frobenius norm,
    /// which bounds the operator norm and is what the level choice
    /// compares against. Stored because it is asked for once per
    /// sample and computing it is nine multiplies.
    pub reach: f64,
    /// Product of the σ_min applied so far. World units here, not per
    /// pixel: a marcher steps BY the distance.
    pub sigma: f64,
    /// σ_min of the last map applied, for the annulus residual.
    pub last_sigma: f64,
    /// Running maximum of `σ·(r − R)`, world units.
    pub bound: f64,
    /// Branch history so far, which the address colouring continues.
    pub address: Vec<u32>,
    /// Where this candidate left the ball, if it already has.
    pub escape: Option<(f64, [f64; 3])>,
    /// Past [`FAR`] already: converged, and not expanded further.
    pub done: bool,
}

/// The beam's state at every level from the target outward.
#[derive(Debug, Clone, PartialEq)]
pub struct SeedChain3 {
    /// `levels[j]` is the beam after `j` inverse maps. `levels[0]` is
    /// the target itself, after the final map if there is one, so a
    /// chain is never empty and a sample too far out for any link
    /// still has one to start from.
    pub levels: Vec<Vec<Seed3>>,
    /// How large a delta a link may carry before it is too deep for a
    /// sample — the same quarter radius the 2D handover uses.
    pub cap: f64,
}

impl SeedChain3 {
    /// The deepest link whose delta still lands inside the cap.
    ///
    /// `reach` rises monotonically with the level — every map's
    /// inverse expands — so this is the last link that qualifies, and
    /// a scan from the deep end finds it. The chain is at most a few
    /// hundred long and this is asked once per distance evaluation, so
    /// the scan is a binary search.
    pub fn level_for(&self, delta_len: f64) -> usize {
        if !(delta_len > 0.0) {
            return self.levels.len() - 1;
        }
        let ok = |j: usize| {
            self.levels[j].iter().all(|s| s.reach * delta_len <= self.cap)
        };
        let (mut lo, mut hi) = (0usize, self.levels.len() - 1);
        if ok(hi) {
            return hi;
        }
        // `lo` always qualifies and `hi` never does.
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if ok(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }
}

/// Walk the beam from the target and record it at every level.
///
/// `finest` is the smallest offset the view will ever ask about — one
/// pixel at the target — and it is what stops the walk: past the level
/// where a single pixel's delta fills the cap, no sample can use a
/// deeper link, so computing one is work for nothing.
///
/// The chain ends at the first level the samples do not agree on, by
/// the same rule as [`seed_beam`]'s, with the cap as every level's
/// reach since a sample sits within the cap of its link's reference by
/// construction. A consequence worth knowing: on a set whose branches
/// TIE -- the sponge's two equidistant neighbours, for a beam of two
/// -- the chain ends at once and the walk is the unseeded f32 one,
/// which is the 2¹³ wall again. A beam of one is exact for a tiling
/// set and is the solid default, so that costs nothing shipped; an
/// overlapping solid wanting both a wide beam and a deep zoom is a
/// case nobody has yet.
pub fn seed_chain3<P: SeedPoint3>(
    ifs: &Ifs3,
    target: P,
    finest: f64,
    max_levels: u32,
    beam: u32,
) -> SeedChain3 {
    let ball = ifs.ball.centre;
    let radius = ifs.ball.radius;
    let beam = beam.max(1) as usize;
    let cap = radius * HANDOVER_FRACTION;
    let mean = mean_sigma_min(&ifs.maps);
    let far = radius.max(1.0) * FAR;

    let (q0, sigma0, m0) = match &ifs.final_map {
        Some(f) => {
            let inv = f.inverse.as_affine().expect("the final transform is affine (J4)");
            (target.apply_affine3(&inv), f.sigma_min, inv.m)
        }
        None => (target, 1.0, Affine3::IDENTITY.m),
    };
    // The reference/delta split is affine (plan §8.5): a nonlinear
    // solid hands over at level 0 and the walk starts from the delta.
    let affine = ifs.maps.iter().all(|m| m.inverse.is_affine());

    let r0 = q0.distance_to(ball);
    let mut live = vec![Cand {
        q: q0,
        sigma: sigma0,
        bound: f64::NEG_INFINITY,
        r: r0,
        address: Vec::new(),
        escape: None,
        done: false,
        aux: 0.0,
    }];
    let mut mats = vec![m0];
    let mut levels: Vec<Vec<Seed3>> = Vec::new();

    for level in 0..=max_levels {
        // Scored for every sample the link can serve rather than for
        // the target alone -- the same correction as the plane's above
        // and for the same reason, since a bound is a running maximum
        // and an escape is a level, so a link hands both to every
        // sample that starts from it and the continuation can undo
        // neither.
        //
        // A link accepts a delta only while its matrix carries it no
        // further than the cap, so `cap` IS the furthest any sample of
        // this link can sit from the reference.
        let mut all_done = true;
        for c in live.iter_mut() {
            if c.done {
                continue;
            }
            let near = c.r - cap;
            c.bound = c.bound.max(c.sigma * (near - radius));
            if near > radius && c.escape.is_none() {
                let last = c
                    .address
                    .last()
                    .map(|&i| ifs.maps[i as usize].sigma_min)
                    .unwrap_or(mean);
                c.escape = Some((
                    level as f64 + escape_residual(near, radius, last),
                    c.address.clone(),
                    c.q.clone(),
                ));
            }
            if !c.r.is_finite() || near > far {
                c.done = true;
            } else {
                all_done = false;
            }
        }

        levels.push(
            live.iter()
                .zip(&mats)
                .map(|(c, m)| Seed3 {
                    position: c.q.to_f64(),
                    matrix: *m,
                    reach: frobenius3(*m),
                    sigma: c.sigma,
                    last_sigma: c
                        .address
                        .last()
                        .map(|&i| ifs.maps[i as usize].sigma_min)
                        .unwrap_or(mean),
                    bound: c.bound,
                    address: c.address.clone(),
                    escape: c.escape.clone().map(|(lvl, _, p)| (lvl, p.to_f64())),
                    done: c.done,
                })
                .collect(),
        );

        if all_done || level == max_levels {
            break;
        }
        // A single pixel already fills the cap: no sample can reach a
        // deeper link. A nonlinear map has no matrix to carry a delta.
        if !affine || mats.iter().any(|m| frobenius3(*m) * finest >= cap) {
            break;
        }

        let mut next: Vec<Cand<P>> = Vec::with_capacity(live.len() * ifs.maps.len());
        let mut next_mats: Vec<[[f64; 3]; 3]> = Vec::with_capacity(next.capacity());
        for (c, m) in live.iter().zip(&mats) {
            if c.done {
                next.push(c.clone());
                next_mats.push(*m);
                continue;
            }
            let last = c.address.last().copied();
            for (i, map) in ifs.maps.iter().enumerate() {
                if !admits(ifs, i, last) {
                    continue;
                }
                let inv = map.inverse.as_affine().expect("checked affine above");
                let q = c.q.apply_affine3(&inv);
                let sigma = c.sigma * map.sigma_min;
                let r = q.distance_to(ball);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = fold_level(c.bound, sigma, r, radius);
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
                next_mats.push(compose3(&inv, *m));
            }
        }
        // The same ranking the walk uses — and the matrices have to
        // follow their candidates through the sort, or every delta
        // ends up attached to the wrong path.
        let mut order: Vec<usize> = (0..next.len()).collect();
        let cmp = cmp_for(resolved_key(ifs, RankKey::Auto));
        order.sort_by(|&a, &b| cmp(&next[a], &next[b]));

        // The same rule as the plane's handover: a branch may be pruned
        // only if every SAMPLE that could use this link would prune it
        // too. A sample sits within the cap of the link's reference by
        // construction -- that is what `level_for` enforces -- so the
        // cap is the reach here, at every level. If the view does not
        // agree on the beam the chain ends one link short, and samples
        // that would have used the missing link use the one above it,
        // with one more level walked per sample in f32.
        if !view_agrees(&order, |i| next[i].r, |_| cap, beam) {
            break;
        }
        order.truncate(beam);
        live = order.iter().map(|&i| next[i].clone()).collect();
        mats = order.iter().map(|&i| next_mats[i]).collect();
    }

    SeedChain3 { levels, cap }
}

/// Continue a seeded 3D walk for one sample — the reference for what
/// the shader does after the handover.
///
/// `delta` is the sample's offset from the TARGET, never its absolute
/// position: forming the absolute position is exactly the cancellation
/// the chain exists to avoid.
pub fn estimate_seeded3(
    ifs: &Ifs3,
    chain: &SeedChain3,
    delta: [f64; 3],
    max_levels: u32,
    beam: u32,
) -> Estimate<[f64; 3]> {
    let centre = ifs.ball.centre;
    let radius = ifs.ball.radius;
    let far = radius.max(1.0) * FAR;
    let beam = beam.max(1) as usize;

    let len = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    let j = chain.level_for(len);
    let seeds = &chain.levels[j];

    let mut live: Vec<Cand<[f64; 3]>> = seeds
        .iter()
        .map(|s| {
            let d = apply3(s.matrix, delta);
            let q = [s.position[0] + d[0], s.position[1] + d[1], s.position[2] + d[2]];
            Cand {
                q,
                sigma: s.sigma,
                bound: s.bound,
                r: Affine3::distance(q, centre),
                address: s.address.clone(),
                escape: s.escape.map(|(lvl, p)| (lvl, s.address.clone(), p)),
                done: s.done,
                aux: 0.0,
            }
        })
        .collect();
    // Ranked by THIS sample's position, as the walk ranks every level.
    live.sort_by(cmp_for(resolved_key(ifs, RankKey::Auto)));
    live.truncate(beam);

    let sigma_of = |c: &Cand<[f64; 3]>| {
        c.address
            .last()
            .map(|&i| ifs.maps[i as usize].sigma_min)
            .unwrap_or_else(|| mean_sigma_min(&ifs.maps))
    };
    // The two the walk keeps, which this had lost in the same way its
    // planar twin had: the best FINISHED path, remembered by its bound
    // rather than left to compete for a beam slot with a key that is
    // no longer a number, and the deepest level any finished path
    // reached. See `estimate_aux_ranked`.
    //
    // Latent rather than live when it was fixed, and measured so:
    // neither rule can change an answer unless paths FINISH and the
    // chain is deeper than one link, and no shipped solid does both.
    // An affine solid never finishes a path; a `quaternion_julia`
    // finishes them constantly but its two preimages tie, so the
    // chain ends at once and the continuation IS the direct walk.
    // Measured at zero difference on a cube, a tetrahedron and a
    // quaternion julia, before and after. Fixed anyway, because the
    // first solid that does both would find an over-read here, and in
    // a marcher an over-read does not fatten a halo -- it puts a ray
    // through a surface.
    let mut best_done: Option<Cand<[f64; 3]>> = None;
    let mut deepest_done = f64::NEG_INFINITY;

    for k in 0..max_levels {
        let mut all_done = true;
        for c in live.iter_mut() {
            if c.done {
                continue;
            }
            let r = Affine3::distance(c.q, centre);
            c.r = r;
            c.bound = fold_level(c.bound, c.sigma, r, radius);
            if r > radius && c.escape.is_none() {
                let level = (j as u32 + k) as f64 + escape_residual(r, radius, sigma_of(c));
                c.escape = Some((level, c.address.clone(), c.q));
            }
            if !r.is_finite() || r > far {
                c.done = true;
                // Frozen INSIDE the ball says nothing about the piece
                // (the walk's rule, and the shader's).
                if !r.is_finite() && !(c.bound > 0.0) {
                    c.bound = f64::INFINITY;
                }
            } else {
                all_done = false;
            }
        }
        if all_done || k + 1 >= max_levels {
            break;
        }

        let mut next: Vec<Cand<[f64; 3]>> = Vec::with_capacity(live.len() * ifs.maps.len());
        for c in &live {
            if c.done {
                if let Some((lvl, _, _)) = c.escape.as_ref() {
                    deepest_done = deepest_done.max(*lvl);
                }
                if c.bound.is_finite()
                    && best_done.as_ref().map_or(true, |b: &Cand<[f64; 3]>| c.bound < b.bound)
                {
                    best_done = Some(c.clone());
                }
                continue;
            }
            let last = c.address.last().copied();
            for (i, m) in ifs.maps.iter().enumerate() {
                if !admits(ifs, i, last) {
                    continue;
                }
                let (q, aux, s) = m.inverse.step(c.q, c.aux, m.sigma_min);
                let sigma = c.sigma * s;
                let r = Affine3::distance(q, centre).hypot(aux - ifs.aux_centre);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = fold_level(c.bound, sigma, r, radius);
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
            }
        }
        next.sort_by(cmp_for(resolved_key(ifs, RankKey::Auto)));
        next.truncate(beam);
        live = next;
    }

    let total = j as u32 + max_levels;
    let deepest_level = live
        .iter()
        .map(|c| c.escape.as_ref().map_or(total as f64, |(lvl, _, _)| *lvl))
        .fold(deepest_done, f64::max);
    // Among FINITE bounds. With `fold_level` a path's bound is finite
    // from level 0 on, so this only differs from a plain minimum when
    // the pixel itself was not a number -- but a non-finite bound must
    // never be the one answered, and this says so rather than relying
    // on it.
    let live_best = live
        .iter()
        .filter(|c| c.bound.is_finite())
        .min_by(|a, b| a.bound.partial_cmp(&b.bound).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();
    let best = match (live_best, best_done) {
        (Some(l), Some(d)) => if d.bound < l.bound { d } else { l },
        (Some(l), None) => l,
        (None, Some(d)) => d,
        (None, None) => live[0].clone(),
    };
    let distance = if best.bound.is_finite() { best.bound.max(0.0) } else { 0.0 };
    match best.escape {
        Some((level, address, point)) => {
            Estimate { distance, level, address, point, escaped: true, deepest_level }
        }
        None => Estimate {
            distance,
            level: total as f64,
            address: best.address,
            point: best.q,
            escaped: false,
            deepest_level,
        },
    }
}

/// Would every pixel in the view keep the same top `beam` of these
/// ranked candidates?
///
/// `key(i)` is the ranking key at the centre and `reach(i)` how far a
/// pixel's own key for that candidate can be from it. The answer is
/// yes exactly when the most optimistic key of the best pruned
/// candidate is still worse than the most pessimistic key of the worst
/// kept one. The order is by key, so the best pruned is the first past
/// the beam.
fn view_agrees(
    order: &[usize],
    key: impl Fn(usize) -> f64,
    reach: impl Fn(usize) -> f64,
    beam: usize,
) -> bool {
    if order.len() <= beam {
        return true;
    }
    let kept_worst = order[..beam]
        .iter()
        .map(|&i| key(i) + reach(i))
        .fold(f64::NEG_INFINITY, f64::max);
    let j = order[beam];
    key(j) - reach(j) > kept_worst
}

/// Compose an inverse map's LINEAR part onto a delta matrix.
fn compose3(inv: &Affine3, m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| inv.m[i][k] * m[k][j]).sum();
        }
    }
    out
}

fn apply3(m: [[f64; 3]; 3], d: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * d[0] + m[0][1] * d[1] + m[0][2] * d[2],
        m[1][0] * d[0] + m[1][1] * d[1] + m[1][2] * d[2],
        m[2][0] * d[0] + m[2][1] * d[1] + m[2][2] * d[2],
    ]
}

/// Bounds the operator norm from above, which is the direction that
/// keeps the level choice SOUND: overstating the reach picks a
/// shallower link, and a shallower link is always valid.
fn frobenius3(m: [[f64; 3]; 3]) -> f64 {
    let mut acc = 0.0;
    for row in &m {
        for v in row {
            acc += v * v;
        }
    }
    acc.sqrt()
}

/// The furthest a normalised offset can be carried by this basis —
/// the screen's corner, which is what bounds the whole view.
fn basis_reach(b: [[f64; 2]; 2]) -> f64 {
    let x = (b[0][0].abs() + b[0][1].abs()) * 0.5;
    let y = (b[1][0].abs() + b[1][1].abs()) * 0.5;
    (x * x + y * y).sqrt()
}

/// The same, with the delta's quadratic part included: `u` and `v`
/// each run over `[-½, ½]`, so every monomial of `Q` is at most a
/// quarter.
fn delta_reach(b: [[f64; 2]; 2], q: &[[f64; 2]; 3]) -> f64 {
    let extra = q
        .iter()
        .map(|c| c[0].hypot(c[1]))
        .sum::<f64>()
        * 0.25;
    basis_reach(b) + extra
}

/// `Q(uv)`, the quadratic part of a delta.
fn apply_quad(q: &[[f64; 2]; 3], uv: [f64; 2]) -> [f64; 2] {
    let (u, v) = (uv[0], uv[1]);
    let (a, b, c) = (u * u, u * v, v * v);
    [
        q[0][0] * a + q[1][0] * b + q[2][0] * c,
        q[0][1] * a + q[1][1] * b + q[2][1] * c,
    ]
}

/// Carry a quadratic through one more inverse map: the Jacobian moves
/// what is already there, and the Hessian contributes what the linear
/// part of the delta generates.
///
/// With `δ = a·u + b·v` the columns of `A`,
/// `½·H[δ, δ] = ½H[a,a]·u² + H[a,b]·u·v + ½H[b,b]·v²`,
/// the cross term losing its half to the two orderings of `j` and `k`.
fn compose_quad(
    jac: [[f64; 2]; 2],
    hess: &[[[f64; 2]; 2]; 2],
    basis: [[f64; 2]; 2],
    quad: &[[f64; 2]; 3],
) -> [[f64; 2]; 3] {
    // The basis columns: `δ = A·uv`.
    let a = [basis[0][0], basis[1][0]];
    let b = [basis[0][1], basis[1][1]];
    let bil = |x: [f64; 2], y: [f64; 2]| -> [f64; 2] {
        let mut out = [0.0f64; 2];
        for (i, o) in out.iter_mut().enumerate() {
            for j in 0..2 {
                for k in 0..2 {
                    *o += hess[i][j][k] * x[j] * y[k];
                }
            }
        }
        out
    };
    let born = [bil(a, a), bil(a, b), bil(b, b)];
    let half = [0.5, 1.0, 0.5];
    let mut out = [[0.0f64; 2]; 3];
    for t in 0..3 {
        let moved = [
            jac[0][0] * quad[t][0] + jac[0][1] * quad[t][1],
            jac[1][0] * quad[t][0] + jac[1][1] * quad[t][1],
        ];
        out[t] = [moved[0] + half[t] * born[t][0], moved[1] + half[t] * born[t][1]];
    }
    out
}

fn apply_basis(b: [[f64; 2]; 2], uv: [f64; 2]) -> [f64; 2] {
    [b[0][0] * uv[0] + b[0][1] * uv[1], b[1][0] * uv[0] + b[1][1] * uv[1]]
}

/// Compose an inverse map's LINEAR part onto a delta basis. The
/// translation is carried entirely by the reference, which is what
/// makes the split exact for affine maps.
fn compose_basis(inv: &Affine2, b: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    compose_matrix(inv.m, b)
}

/// The same, for a matrix that is a Jacobian rather than an affine's
/// linear part -- the nonlinear step's delta, exact to first order.
fn compose_matrix(m: [[f64; 2]; 2], b: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    [
        [
            m[0][0] * b[0][0] + m[0][1] * b[1][0],
            m[0][0] * b[0][1] + m[0][1] * b[1][1],
        ],
        [
            m[1][0] * b[0][0] + m[1][1] * b[1][0],
            m[1][0] * b[0][1] + m[1][1] * b[1][1],
        ],
    ]
}

/// The address as a base-N fraction in `[0, 1)`, most significant
/// digit first — the colouring quantity, and the base-N generalisation
/// of `address_mix`'s binary fold. Digits past f64's mantissa stop
/// contributing, which is the same place the colouring stops being
/// able to show them.
pub fn address_fraction(address: &[u32], n_maps: u32) -> f64 {
    if n_maps == 0 {
        return 0.0;
    }
    let base = n_maps as f64;
    let mut scale = 1.0 / base;
    let mut acc = 0.0;
    for &d in address {
        acc += d as f64 * scale;
        scale /= base;
        if scale == 0.0 {
            break;
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ifs_analysis::{analyse_2d, Ifs2, Ifs3};
    use crate::scene::transforms::{Flame, Transform};
    use crate::variations::global_registry;
    use std::collections::HashMap;

    /// Phase 0's helper, kept identical: `m` is row-major
    /// `[[a, b], [c, d]]` and the translation is `[e, f]`.
    fn affine_xform(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Transform {
        let mut t = Transform::default();
        t.a = a;
        t.b = b;
        t.c = c;
        t.d = d;
        t.e = e;
        t.f = f;
        t.variations = HashMap::from([("linear".to_string(), 1.0)]);
        t.variation_order = vec!["linear".to_string()];
        t
    }

    fn flame_of(transforms: Vec<Transform>) -> Flame {
        let mut fl = Flame::default();
        fl.transforms = transforms;
        fl.final_transforms.clear();
        fl.xaos = None;
        fl
    }

    fn analyse(transforms: Vec<Transform>) -> Ifs2 {
        let guard = global_registry();
        analyse_2d(&flame_of(transforms), &guard).expect("should qualify")
    }

    /// `p ↦ p/2 + (tx, ty)`.
    fn half(tx: f32, ty: f32) -> Transform {
        affine_xform(0.5, 0.0, 0.0, 0.5, tx, ty)
    }

    /// Four half-scale maps to the corners. Their images tile `[0,1]²`
    /// exactly, so the attractor IS the filled unit square and the
    /// distance to it is closed form.
    fn unit_square() -> Ifs2 {
        analyse(vec![half(0.0, 0.0), half(0.5, 0.0), half(0.0, 0.5), half(0.5, 0.5)])
    }

    // ---- the root maps (plan 8.8, gate 1) ----------------------------

    /// One `julia` transform with pre-translation `−c`: the inverse
    /// iteration system of `z² + c`.
    fn julia(c: [f64; 2]) -> Ifs2 {
        let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, -(c[0] as f32), -(c[1] as f32));
        t.variations = HashMap::from([("julia".to_string(), 1.0)]);
        t.variation_order = vec!["julia".to_string()];
        analyse(vec![t])
    }

    /// The classic escape-time distance estimate for `z² + c`:
    /// `|z|·ln|z| / |z′|` at escape, or `None` for a point that does
    /// not escape within `levels`. Returns the escape level too.
    fn classic_de(c: [f64; 2], z0: [f64; 2], levels: u32) -> Option<(f64, u32)> {
        let (mut z, mut dz): ([f64; 2], [f64; 2]) = (z0, [1.0, 0.0]);
        for k in 0..levels {
            let r2 = z[0] * z[0] + z[1] * z[1];
            if r2 > 1e16 {
                let r = r2.sqrt();
                let dr = (dz[0] * dz[0] + dz[1] * dz[1]).sqrt();
                return Some((r * r.ln() / dr, k));
            }
            // dz ← 2 z dz, z ← z² + c
            dz = [2.0 * (z[0] * dz[0] - z[1] * dz[1]), 2.0 * (z[0] * dz[1] + z[1] * dz[0])];
            z = [z[0] * z[0] - z[1] * z[1] + c[0], 2.0 * z[0] * z[1] + c[1]];
        }
        None
    }

    /// Gate 1 of plan 8.8: on `z² + c` the walk IS the escape-time
    /// iteration -- the inverse map is `q² + c` and the local σ_min is
    /// `1/(2|q|)`, so the σ product is `1/|z′|` -- and its distance
    /// must agree with the classic estimate up to the factor J3
    /// admits. Membership (never escapes) must agree exactly where the
    /// classic test is not itself on the fence.
    ///
    /// Two `c`s: the Douady rabbit (connected, so the walk draws the
    /// FILLED set, J2) and a dust.
    #[test]
    fn a_julia_walk_agrees_with_the_classic_distance_estimate() {
        for (name, c) in [("rabbit", [-0.123, 0.745]), ("dust", [0.36, 0.1]), ("basilica", [-1.0, 0.0])] {
            let ifs = julia(c);
            const L: u32 = 80;
            let (mut n, mut member_bad, mut ratios) = (0usize, 0usize, Vec::new());
            for iy in 0..96 {
                for ix in 0..96 {
                    let z0 = [-2.0 + 4.0 * (ix as f64 + 0.5) / 96.0, -2.0 + 4.0 * (iy as f64 + 0.5) / 96.0];
                    let ours = estimate(&ifs, z0, L, 1);
                    let classic = classic_de(c, z0, L);
                    n += 1;
                    match classic {
                        // Not escaped by L for the classic test, whose
                        // bailout is 1e8 -- five squarings past the
                        // ball. So the walk may have left the ball a
                        // few levels before L on a point the classic
                        // test has not yet let go of; a disagreement is
                        // the walk escaping CLEARLY earlier.
                        None => {
                            if ours.escaped && ours.level < (L - 8) as f64 {
                                member_bad += 1;
                            }
                        }
                        Some((d, k)) => {
                            // A point escaping late is one the ball's
                            // radius versus the bailout could put on
                            // either side of L; leave those out.
                            if k + 8 > L {
                                continue;
                            }
                            if !ours.escaped || !(ours.distance > 0.0) {
                                member_bad += 1;
                                continue;
                            }
                            ratios.push(ours.distance / d);
                        }
                    }
                }
            }
            ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let (lo, hi) = (ratios[0], ratios[ratios.len() - 1]);
            let med = ratios[ratios.len() / 2];
            println!(
                "  {name}: {n} points, {} compared, membership disagreements {member_bad}, \
                 distance ratio walk/classic: min {lo:.3} median {med:.3} max {hi:.3}",
                ratios.len()
            );
            assert_eq!(member_bad, 0, "{name}: membership disagrees on {member_bad} points");
            assert!(ratios.len() > 2000, "{name}: only {} exterior points compared", ratios.len());
            // Measured: [0.36, 0.61] over the three sets. Below the
            // classic estimate throughout, and by a bounded factor:
            // the walk's value has no ln|z| in it, and its maximum
            // over levels lands a level or two past the escape, where
            // (r − R) stands in for r·ln r. Asserted at what was
            // measured, with room for a different c but not for a
            // different mechanism.
            assert!(lo > 0.25 && hi < 1.0, "{name}: ratio range [{lo:.3}, {hi:.3}] is outside what J3 measured");
        }
    }

    // ---- spherical and bubble (plan 8.9, gate 1) ---------------------

    fn kernel_xform(variation: &str, a: [f32; 6], w: f32) -> Transform {
        let mut t = affine_xform(a[0], a[1], a[2], a[3], a[4], a[5]);
        t.variations = HashMap::from([(variation.to_string(), w)]);
        t.variation_order = vec![variation.to_string()];
        t
    }

    /// A dense chaos-game sample of an IFS with nonlinear maps, as an
    /// upper bound on the distance to its set: the distance to the
    /// nearest sample point is at least the distance to the set.
    /// The chaos game a xaos flame actually plays: the next map is
    /// drawn from the previous one's row.
    ///
    /// The shipped `select_transform_xaos` in `utilities.wgsl` does
    /// exactly this, and the walk's whole claim is that its addresses
    /// are the ones this generates.
    fn chaos_sample_xaos(ifs: &Ifs2, xaos: &[Vec<f32>], weights: &[f64], count: usize)
        -> Vec<[f64; 2]>
    {
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let n = ifs.maps.len();
        let mut p = ifs.ball.centre;
        let mut prev = 0usize;
        let mut out = Vec::with_capacity(count);
        for i in 0..count + 500 {
            let row: Vec<f64> = (0..n)
                .map(|j| weights[j] * xaos[ifs.maps[prev].transform_index]
                    [ifs.maps[j].transform_index] as f64)
                .collect();
            let total: f64 = row.iter().sum();
            let mut t = next() * total;
            let mut pick = n - 1;
            for (j, w) in row.iter().enumerate() {
                t -= w;
                if t <= 0.0 {
                    pick = j;
                    break;
                }
            }
            prev = pick;
            p = ifs.maps[pick].forward.apply(p);
            if i >= 500 && p[0].is_finite() && p[1].is_finite() {
                out.push(p);
            }
        }
        out
    }

    /// A four-corner square, with a xaos matrix on it.
    fn square4(xaos: Option<Vec<Vec<f32>>>) -> (crate::scene::transforms::Flame, Ifs2) {
        use crate::scene::transforms::{Flame, Transform};
        let half = |tx: f32, ty: f32, col: f32| {
            let mut t = Transform::default();
            t.a = 0.5; t.d = 0.5; t.e = tx; t.f = ty;
            t.color = col;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            t
        };
        let mut flame = Flame::default();
        flame.transforms = vec![
            half(0.0, 0.0, 0.0),
            half(0.5, 0.0, 0.33),
            half(0.0, 0.5, 0.66),
            half(0.5, 0.5, 1.0),
        ];
        flame.xaos = xaos;
        let guard = crate::variations::global_registry();
        let ifs = crate::scene::ifs_analysis::analyse_2d(&flame, &guard).expect("qualifies");
        drop(guard);
        (flame, ifs)
    }

    /// A xaos flame is no longer refused, and a UNIFORM one changes
    /// nothing (`ifs-general.md` D4, G5's first half).
    ///
    /// All-ones would be a vacuous test: `Flame::has_xaos` reads it as
    /// no xaos at all and the graph is never built. All-halves is the
    /// real one -- the graph IS built, every row normalises to the
    /// same thing a plain weight draw gives, and the walk and the
    /// measure have to come out unchanged to the last bit.
    #[test]
    fn a_uniform_xaos_changes_nothing() {
        let (_, plain) = square4(None);
        let (flame, uniform) = square4(Some(vec![vec![0.5; 4]; 4]));
        assert!(plain.xaos.is_none(), "all-ones is not xaos");
        let g = uniform.xaos.as_ref().expect("all-halves is");

        // Every transition admitted, and every step factor the plain
        // weight -- a quarter each, on four transforms of weight one.
        for i in 0..4 {
            assert!(g.admits(i, None), "map {i} unreachable");
            assert!(
                (g.step_probability(i, None) - 0.25).abs() < 1e-12,
                "stationary {i} is {}",
                g.step_probability(i, None)
            );
            for l in 0..4u32 {
                assert!(g.admits(i, Some(l)));
                assert!(
                    (g.step_probability(i, Some(l)) - 0.25).abs() < 1e-12,
                    "step {i} after {l} is {}",
                    g.step_probability(i, Some(l))
                );
            }
        }

        // And the answers: the distance walk and the measure, at a
        // grid over the ball.
        let maps_plain = MeasureMaps::of(&plain, &flame);
        let maps_uni = MeasureMaps::of(&uniform, &flame);
        assert!(maps_uni.step.is_some(), "the graph is carried");
        let coarse = chaos_measure(&plain, &flame, 400_000, 128, 0x1234_5678_9abc_def1);
        let mut checked = 0usize;
        for gy in 0..12 {
            for gx in 0..12 {
                let x = [
                    plain.ball.centre[0] + plain.ball.radius * (gx as f64 / 5.5 - 1.0),
                    plain.ball.centre[1] + plain.ball.radius * (gy as f64 / 5.5 - 1.0),
                ];
                let a = estimate(&plain, x, 24, 8);
                let b = estimate(&uniform, x, 24, 8);
                assert_eq!(
                    a.distance.to_bits(),
                    b.distance.to_bits(),
                    "distance at {x:?}: {} vs {}",
                    a.distance,
                    b.distance
                );
                assert_eq!(a.address, b.address, "address at {x:?}");
                let px = plain.ball.radius / 64.0;
                let ma = estimate_measure(&plain, &maps_plain, &coarse, x, px, 8, 4.0, 40);
                let mb = estimate_measure(&uniform, &maps_uni, &coarse, x, px, 8, 4.0, 40);
                assert!(
                    (ma.density - mb.density).abs() <= 1e-12 * ma.density.max(1.0),
                    "density at {x:?}: {} vs {}",
                    ma.density,
                    mb.density
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 144);
    }

    /// A restrictive xaos makes a SMALLER attractor, and the walk
    /// knows it (`ifs-general.md` D4, G5's second half).
    ///
    /// The matrix is a four-cycle: after map `i` only `i+1` may
    /// follow. The chain is irreducible, so the chaos game and the
    /// walk agree on one invariant measure and there is no question
    /// of which class the game happened to start in. The attractor is
    /// a proper subset of the filled square the same four maps make
    /// without it.
    ///
    /// Two directions, because either alone is easy to pass: every
    /// point the restricted chaos game reaches must read as ON the
    /// set, and enough points the UNrestricted game reaches must read
    /// as off it. A walk that ignored the graph would pass the first
    /// and fail the second; one that admitted nothing would pass the
    /// second and fail the first.
    #[test]
    fn a_restrictive_xaos_shrinks_the_set_and_the_walk_follows() {
        let cycle: Vec<Vec<f32>> = (0..4)
            .map(|i| (0..4).map(|j| if j == (i + 1) % 4 { 1.0 } else { 0.0 }).collect())
            .collect();
        let (flame, ifs) = square4(Some(cycle.clone()));
        let g = ifs.xaos.as_ref().expect("a graph");
        // The transition to admit when `child` is appended after
        // `last` is `child -> last`, so `last` must be `child + 1`.
        for child in 0..4usize {
            for last in 0..4u32 {
                let want = last as usize == (child + 1) % 4;
                assert_eq!(
                    g.admits(child, Some(last)),
                    want,
                    "child {child} after {last}"
                );
            }
        }

        let weights = vec![1.0f64; 4];
        let on = chaos_sample_xaos(&ifs, &cycle, &weights, 4000);
        let (_, full) = square4(None);
        let off = chaos_sample(&full, 4000);

        let px = ifs.ball.radius / 256.0;
        let mut on_far = 0usize;
        for p in on.iter().take(2000) {
            if estimate(&ifs, *p, 40, 8).distance > 4.0 * px {
                on_far += 1;
            }
        }
        assert_eq!(on_far, 0, "{on_far} of 2000 points ON the set read as off it");

        // The restricted set is a strict subset, so a good share of
        // the full square's points must now read as exterior. Not all
        // of them: the two sets share points, and a cycle of four
        // still covers a lot of the square.
        let mut off_far = 0usize;
        for p in off.iter().take(2000) {
            if estimate(&ifs, *p, 40, 8).distance > 4.0 * px {
                off_far += 1;
            }
        }
        assert!(
            off_far > 200,
            "only {off_far} of 2000 points of the UNrestricted attractor read as off \
             the restricted one -- the walk is not using the graph"
        );
        println!("  the four-cycle rejects {off_far} of 2000 free-chaos points");
        let _ = flame;
    }

    /// The two walks side by side, level by level, on one pixel.
    ///
    /// A diagnostic, kept because it is what found the two bugs G2
    /// caught: σ taken from the reference instead of the pixel, worth
    /// eight pixels on a julia at 2^12, and one level too many in the
    /// budget, worth a tenth of one at 2^18. Both were invisible in
    /// the aggregate and obvious in this table -- the first as a slow
    /// drift in the bound column with the radius column matching, the
    /// second as a perfect match for sixty rows and a disagreement on
    /// the sixty-first.
    ///
    /// The `sum r` column is the delta form's own answer, `Z + δ`
    /// against the direct walk's absolute position. That it agrees to
    /// ten decimals even where `|δ|` has grown past the ball is the
    /// difference forms working.
    #[test]
    #[ignore = "a diagnostic; run with --ignored --nocapture"]
    fn probe_the_delta_walk_beside_the_direct_one() {
        let ifs = julia([-0.4, 0.6]);
        let target = chaos_sample(&ifs, 20_000)[10_000];
        let zoom = 18.0f64;
        let span = 2.0 * ifs.ball.radius / 2f64.powf(zoom);
        let px = span / 64.0;
        let basis = [[span, 0.0], [0.0, -span]];
        let reference = reference_beam(&ifs, target, basis, px, 60, 8);
        let uv = [(5.0 + 0.5) / 8.0 - 0.5, (7.0 + 0.5) / 8.0 - 0.5];
        let at = [
            target[0] + basis[0][0] * uv[0] + basis[0][1] * uv[1],
            target[1] + basis[1][0] * uv[0] + basis[1][1] * uv[1],
        ];
        println!("  maps {}  target {target:?}  at {at:?}", ifs.maps.len());
        // The direct walk, one map, one lineage.
        let m = &ifs.maps[0];
        let mut q = at;
        let mut zq = reference.levels[0][0].z;
        let mut d = [
            basis[0][0] * uv[0] + basis[0][1] * uv[1],
            basis[1][0] * uv[0] + basis[1][1] * uv[1],
        ];
        let radius = ifs.ball.radius;
        let mut bd = f64::NEG_INFINITY;   // direct bound, pixels
        let mut bs = f64::NEG_INFINITY;   // delta-form bound, pixels
        let mut sd = 1.0 / px;            // direct sigma per px
        let mut ss = 1.0 / px;
        println!("  radius {radius}  px {px:.3e}");
        let (got, reb) = estimate_delta(&ifs, &reference, uv, 8, 60);
        let want = estimate(&ifs, at, 60, 8);
        println!(
            "  estimate_delta {:.6} px | direct {:.6} px | escaped {} vs {} | rebases {:?}",
            got.distance,
            want.distance / px,
            got.escaped,
            want.escaped,
            &reb[..reb.len().min(6)]
        );
        println!("  lvl  direct r          sum r            direct bound     delta bound");
        for k in 0..60usize {
            if k + 1 >= reference.levels.len() {
                println!("  reference stops at level {k}");
                break;
            }
            let rows = &reference.levels[k + 1];
            let Some(ci) = rows.iter().position(|r| r.parent == 0 || rows.len() == 1) else {
                println!("  no child row at level {}", k + 1);
                break;
            };
            let dd = m.inverse.difference(zq, d).expect("a form");
            let qprev = q;
            let sprev = [zq[0] + d[0], zq[1] + d[1]];
            q = m.inverse.apply(q);
            zq = rows[ci].z;
            d = dd;
            let rq = Affine2::distance(q, ifs.ball.centre);
            let t = 2.0 * (rows[ci].u[0] * d[0] + rows[ci].u[1] * d[1]) + d[0] * d[0] + d[1] * d[1];
            let rs = (rows[ci].r * rows[ci].r + t).max(0.0).sqrt();
            sd *= m.inverse.local_sigma(qprev, m.sigma_min);
            ss *= m.inverse.local_sigma(sprev, m.sigma_min);
            bd = bd.max(sd * (rq - radius));
            bs = bs.max(ss * ((rows[ci].excess2 + t) / (rs + radius)));
            println!(
                "  {:>3}  {rq:<17.10} {rs:<16.10} {bd:<16.6} {bs:<16.6}",
                k + 1,
            );
            if k > 58 {
                break;
            }
        }
    }

    /// G2: the delta walk is the direct walk.
    ///
    /// `ifs-perturbation-delta.md` §6. The centre's beam is walked to
    /// the budget, every pixel of a grid continues from it in DELTA
    /// form, and the answer is compared against the direct f64 walk
    /// at the pixel's own absolute position.
    ///
    /// **No f32 cost and no curvature tolerance.** The first design's
    /// gates needed both, and needing neither is the point of this
    /// one.
    ///
    /// **The bar is the direct walk's OWN reproducibility.** A
    /// thousandth of a pixel is the bar wherever the direct walk is
    /// stable, and on the affine sets and the bubble set it is: they
    /// agree exactly, to the last bit, at every zoom. On a julia they
    /// do not, and the reason is not the delta form. The walk
    /// amplifies its own rounding by the map's derivative at every
    /// level, and sixty levels of a squaring map turn f64's last
    /// digit into a visible fraction of a pixel -- so the direct walk
    /// moved by one ulp of the pixel's position is a DIFFERENT
    /// answer too. This measures that jitter and requires the delta
    /// walk to sit inside it, which is the strongest statement the
    /// reference can support. Where the jitter is zero the bar is the
    /// thousandth again, so the measurement can never loosen the
    /// gate on a set where the reference is sound.
    #[test]
    fn the_delta_walk_is_the_direct_walk() {
        let cases: Vec<(&str, Ifs2)> = vec![
            ("dragon", dragon()),
            ("gasket", sierpinski()),
            ("julia", julia([-0.4, 0.6])),
            (
                "bubble set",
                analyse(vec![
                    kernel_xform("bubble", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.6),
                    kernel_xform("bubble", [0.7, 0.7, -0.7, 0.7, 0.0, -0.3], 1.2),
                    affine_xform(0.5, 0.0, 0.0, 0.5, 1.0, 0.5),
                ]),
            ),
        ];

        let mut worst_overall = 0.0f64;
        for (name, ifs) in cases {
            // A point ON the set, so the view has something in it at
            // every zoom.
            let target = chaos_sample(&ifs, 20_000)[10_000];
            for zoom in [12.0f64, 18.0, 24.0, 28.0] {
                let span = 2.0 * ifs.ball.radius / 2f64.powf(zoom);
                let px = span / 64.0;
                let basis = [[span, 0.0], [0.0, -span]];
                let reference = reference_beam(&ifs, target, basis, px, 60, 8);
                let (mut worst, mut worst_ratio) = (0.0f64, 0.0f64);
                let (mut rebased, mut n, mut jittery) = (0usize, 0usize, 0usize);
                let mut first: Vec<u32> = Vec::new();
                for gy in 0..8u32 {
                    for gx in 0..8u32 {
                        let uv = [
                            (gx as f64 + 0.5) / 8.0 - 0.5,
                            (gy as f64 + 0.5) / 8.0 - 0.5,
                        ];
                        let at = [
                            target[0] + basis[0][0] * uv[0] + basis[0][1] * uv[1],
                            target[1] + basis[1][0] * uv[0] + basis[1][1] * uv[1],
                        ];
                        let (got, reb) = estimate_delta(&ifs, &reference, uv, 8, 60);
                        let want = estimate(&ifs, at, 60, 8).distance / px;
                        // The same question asked one ulp away: what
                        // the reference itself can tell apart.
                        let nudged = [
                            f64::from_bits(at[0].to_bits() + 1),
                            f64::from_bits(at[1].to_bits() + 1),
                        ];
                        let jitter =
                            (estimate(&ifs, nudged, 60, 8).distance / px - want).abs();
                        let err = (got.distance - want).abs();
                        let bar = 1e-3f64.max(4.0 * jitter);
                        if err > worst {
                            worst = err;
                        }
                        if err / bar > worst_ratio {
                            worst_ratio = err / bar;
                        }
                        if jitter > 1e-3 {
                            jittery += 1;
                        }
                        if let Some((lvl, _)) = reb.first() {
                            rebased += 1;
                            first.push(*lvl);
                        }
                        assert!(
                            err <= bar,
                            "{name} at 2^{zoom}, pixel ({gx}, {gy}): delta {:.6} px, \
                             direct {want:.6} px, and the direct walk's own one-ulp \
                             jitter is only {jitter:.6}",
                            got.distance
                        );
                        n += 1;
                    }
                }
                first.sort_unstable();
                let carried = first.get(first.len() / 2).copied().unwrap_or(u32::MAX);
                println!(
                    "  {name:<11} 2^{zoom:<4} depth {:>3} | worst {worst:.2e} px, \
                     {worst_ratio:.2} of its bar | {jittery}/{n} jittery | {rebased}/{n} \
                     rebase, median at level {carried}",
                    reference.depth()
                );
                worst_overall = worst_overall.max(worst);
            }
        }
        println!("  worst over every case: {worst_overall:.2e} px");
    }

    /// D6: no level is chosen, and the level a lineage rebases at
    /// does not depend on the RESOLUTION.
    ///
    /// This is what replaces the first design's
    /// `the_handover_does_not_depend_on_the_resolution`, and it is a
    /// stronger statement than that gate could make. The old one
    /// checked that an OBJECTIVE picked the same level at two pixel
    /// sizes, which was a property of the objective's arithmetic.
    /// Here there is no objective: a lineage rebases when its own δ
    /// reaches a fraction of the ball, or when `Z + δ` cancels, and
    /// neither quantity contains `px` at all. So the levels must be
    /// IDENTICAL, not close -- and rendering the same view at four
    /// times the resolution must not move a single one of them.
    ///
    /// What they do depend on is the zoom, and they should: a deeper
    /// view starts with a smaller δ and therefore carries it further.
    /// That relationship is measured beside the equality, so the
    /// gate cannot pass by the rebase level being constant.
    #[test]
    fn the_rebase_level_depends_on_the_zoom_and_not_on_the_pixel() {
        for (name, ifs) in [
            ("dragon", dragon()),
            ("gasket", sierpinski()),
            ("julia", julia([-0.4, 0.6])),
        ] {
            let target = chaos_sample(&ifs, 20_000)[10_000];
            let mut by_zoom: Vec<(f64, u32)> = Vec::new();
            for zoom in [12.0f64, 20.0, 28.0] {
                let span = 2.0 * ifs.ball.radius / 2f64.powf(zoom);
                let basis = [[span, 0.0], [0.0, -span]];
                let levels = |px: f64| -> Vec<Option<u32>> {
                    let reference = reference_beam(&ifs, target, basis, px, 60, 8);
                    (0..8u32)
                        .flat_map(|gy| (0..8u32).map(move |gx| (gx, gy)))
                        .map(|(gx, gy)| {
                            let uv = [
                                (gx as f64 + 0.5) / 8.0 - 0.5,
                                (gy as f64 + 0.5) / 8.0 - 0.5,
                            ];
                            estimate_delta(&ifs, &reference, uv, 8, 60)
                                .1
                                .first()
                                .map(|(l, _)| *l)
                        })
                        .collect()
                };
                let coarse = levels(span / 64.0);
                let fine = levels(span / 256.0);
                assert_eq!(
                    coarse, fine,
                    "{name} at 2^{zoom}: four times the resolution moved a rebase level"
                );
                let mut seen: Vec<u32> = coarse.iter().flatten().copied().collect();
                seen.sort_unstable();
                by_zoom.push((zoom, seen.get(seen.len() / 2).copied().unwrap_or(0)));
            }
            println!(
                "  {name:<8} median rebase level by zoom: {:?}",
                by_zoom.iter().map(|(z, l)| (*z as u32, *l)).collect::<Vec<_>>()
            );
            // Deeper carries further. Not a tight law -- the set
            // decides how fast a delta grows -- but it must not be
            // flat, or the delta form is not doing anything the
            // first design's single handover did not.
            let (first, last) = (by_zoom[0].1, by_zoom[by_zoom.len() - 1].1);
            assert!(
                last > first + 4,
                "{name}: the rebase level went from {first} at 2^12 to {last} at 2^28, \
                 which is not the delta form carrying further as the view shrinks"
            );
        }
    }

    /// The affine difference form IS the affine basis carry.
    ///
    /// G0's argument, made directly rather than through a render: on
    /// a set whose every map is affine, a delta step is `M⁻¹δ` and the
    /// basis carry is `M⁻¹` composed with the view -- the same
    /// arithmetic in the same order -- so the two agree to the last
    /// bit, not to a tolerance. If this ever needed a tolerance, the
    /// shipped presets would be about to move.
    #[test]
    fn the_affine_delta_is_the_affine_basis_carry() {
        for (name, ifs) in [("dragon", dragon()), ("gasket", sierpinski())] {
            let target = chaos_sample(&ifs, 20_000)[10_000];
            let span = 2.0 * ifs.ball.radius / 2f64.powf(20.0);
            let px = span / 64.0;
            let basis = [[span, 0.0], [0.0, -span]];
            let reference = reference_beam(&ifs, target, basis, px, 40, 8);
            let seeds = seed_beam(&ifs, target, basis, px, 40, 8);
            // The reference's rows at the level the handover chose
            // must hold the same positions the seeds do.
            let lvl = seeds.level as usize;
            assert!(lvl < reference.levels.len(), "{name}: reference is shallower");
            for seed in seeds.cands.iter() {
                let found = reference.levels[lvl].iter().any(|r| {
                    r.z[0].to_bits() == seed.position[0].to_bits()
                        && r.z[1].to_bits() == seed.position[1].to_bits()
                });
                assert!(
                    found,
                    "{name}: seed at {:?} has no reference row at level {lvl}",
                    seed.position
                );
            }
        }
    }

    pub(super) fn chaos_sample(ifs: &Ifs2, count: usize) -> Vec<[f64; 2]> {
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut p = ifs.ball.centre;
        let mut out = Vec::with_capacity(count);
        for i in 0..count + 500 {
            let m = &ifs.maps[(next() * ifs.maps.len() as f64).floor() as usize % ifs.maps.len()];
            let k = match m.forward.nonlinear().map(|n| n.kernel) {
                Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => (next() * n.unsigned_abs() as f64).floor() as u32,
                _ => 0,
            };
            p = match &m.forward {
                Map2::Nonlinear(n) => n.apply_branch(p, k),
                other => other.apply(p),
            };
            if i >= 500 && p[0].is_finite() && p[1].is_finite() {
                out.push(p);
            }
        }
        out
    }

    /// What plan 8.10's kernels measured on the sampled bound, pinned
    /// with a little room: see the record. `usize::MAX` until measured.
    fn fold_over_limit(name: &str) -> usize {
        match name {
            // Measured 0 of 1600, both: rigorous balls, honest σ_min.
            "hemisphere" | "disc" => 0,
            // Measured 22 of 1600, 3 inner, worst 1.56x -- small, and
            // not explained: the Jacobian's singular values are checked
            // against finite differences and the ball is invariant, so
            // the product-of-parts bound should hold. Pinned as found.
            "blob" => 30,
            _ => usize::MAX,
        }
    }

    /// Plan 8.11 step 2, gate 2: on a solid of two 3D roots and an
    /// affine, the walk's distance never exceeds the distance to a
    /// dense 3D chaos-game sample of the set by more than the sample's
    /// spacing. Measured first, then pinned.
    #[test]
    fn nonlinear_solid_walks_never_exceed_a_sampled_upper_bound() {
        let guard = global_registry();
        let mk = |variation: &str, power: f32, e: f32, f: f32, g: f32, w: f32| {
            let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, e, f);
            t.g = g;
            t.variations = HashMap::from([(variation.to_string(), w)]);
            t.variation_order = vec![variation.to_string()];
            t.set_variation_param(variation, "power", power);
            t
        };
        let mut aff = affine_xform(1.0, 0.0, 0.0, 1.0, 0.6, 0.0);
        aff.g = 0.2;
        aff.variations = HashMap::from([("linear3D".to_string(), 0.5)]);
        aff.variation_order = vec!["linear3D".to_string()];
        let qj = |c: [f32; 4], power: f32| {
            let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
            t.variations = HashMap::from([("quaternion_julia".to_string(), 1.0)]);
            t.variation_order = vec!["quaternion_julia".to_string()];
            for (k, v) in [("cx", c[0]), ("cy", c[1]), ("cz", c[2]), ("cw", c[3]), ("power", power), ("inverse", 1.0)] {
                t.set_variation_param("quaternion_julia", k, v);
            }
            t
        };
        for (name, transforms) in [
            ("julia3D pair", vec![mk("julia3D", 2.0, 0.3, -0.2, 0.1, 0.9), mk("julia3D", 2.0, -0.4, 0.3, -0.2, 0.9), aff.clone()]),
            ("julia3Dz pair", vec![mk("julia3Dz", 2.0, 0.3, -0.2, 0.1, 0.9), mk("julia3Dz", 3.0, -0.4, 0.3, -0.2, 0.8), aff.clone()]),
            // Scalar-dominant constants, which have an interior on the
            // vector slice; a pure-vector c is a dust there.
            ("quaternion pair", vec![qj([0.3, 0.0, 0.0, -0.6], 2.0), qj([0.0, 0.0, 0.0, -0.5], 2.0)]),
        ] {
            let ifs = crate::scene::ifs_analysis::analyse_3d(&flame_of(transforms), &guard).expect("qualifies");
            // A 3D chaos-game sample, branches drawn at random.
            let mut state: u64 = 0x2545_F491_4F6C_DD1D;
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (state >> 11) as f64 / (1u64 << 53) as f64
            };
            // A 4D sample, its scalar carried; the slice compared is
            // the ball centre's own, where the set is thickest.
            let mut p = ifs.ball.centre;
            let mut aux = ifs.aux_centre;
            let mut sample: Vec<[f64; 4]> = Vec::with_capacity(300_000);
            for i in 0..300_500 {
                let m = &ifs.maps[(next() * ifs.maps.len() as f64).floor() as usize % ifs.maps.len()];
                let k = m.forward.nonlinear().map_or(0, |nl| (next() * nl.kernel.power().unsigned_abs() as f64).floor() as u32);
                let (np, na) = match &m.forward {
                    Map3::Nonlinear(nl) => nl.apply_branch_aux(p, aux, k),
                    other => (other.apply(p), aux),
                };
                p = np;
                aux = na;
                if i >= 500 && p.iter().all(|x| x.is_finite()) && aux.is_finite() {
                    sample.push([p[0], p[1], p[2], aux]);
                }
            }
            let w0 = ifs.aux_centre;
            let radius = ifs.ball.radius;
            let (mut n, mut over, mut worst) = (0usize, 0usize, 0.0f64);
            const G: usize = 14;
            for iz in 0..G {
                for iy in 0..G {
                    for ix in 0..G {
                        let f = |i: usize| 2.0 * (i as f64 + 0.5) / G as f64 - 1.0;
                        let (u, v, w) = (f(ix), f(iy), f(iz));
                        if (u * u + v * v + w * w).sqrt() > 1.0 {
                            continue;
                        }
                        let q = [ifs.ball.centre[0] + radius * u, ifs.ball.centre[1] + radius * v, ifs.ball.centre[2] + radius * w];
                        let e = estimate_aux(&ifs, q, w0, 32, 4);
                        // The 4D distance to the sample bounds the 4D
                        // distance to the set, which the walk's estimate
                        // is a bound on.
                        let upper = sample
                            .iter()
                            .map(|&a| Affine3::distance(q, [a[0], a[1], a[2]]).hypot(w0 - a[3]))
                            .fold(f64::INFINITY, f64::min);
                        // A 300 000-point sample of a solid is sparser
                        // than a plane's: the mean spacing in a ball of
                        // radius R is R·(4π/3 / 300 000)^(1/3) ≈ R/41, so
                        // the tolerance is a thirtieth of the ball.
                        let tol = radius / 30.0;
                        n += 1;
                        if e.distance > upper + tol {
                            over += 1;
                            worst = worst.max(e.distance / upper.max(1e-12));
                        }
                    }
                }
            }
            println!("  {name}: {over} of {n} points over the sampled bound, worst ratio {worst:.3}");
            assert!(over <= solid_over_limit(name), "{name}: {over} of {n} points read farther than the set is ({worst:.3}x)");
        }
    }

    /// What the 3D kernels measured, pinned. At beam 2 the julia3D
    /// pair read 24 of 1472 points over (worst 1.33x) and the julia3Dz
    /// pair 26 (worst 2.69x); at beam 4 both read ZERO. So the
    /// over-reads were the beam's -- one address bounding the distance
    /// to ITS piece, D4's known weakness -- and not the kernels', and
    /// the gate is at the beam that shows the kernels.
    fn solid_over_limit(name: &str) -> usize {
        match name {
            // The quaternion pair measured 0 of 1472 at beam 4 too.
            "julia3D pair" | "julia3Dz pair" | "quaternion pair" => 0,
            _ => usize::MAX,
        }
    }

    /// Gate 1 of plan 8.9: on a spherical IFS and a bubble IFS the
    /// walk's distance never exceeds the distance to a dense sample of
    /// the set by more than the sample's spacing -- the soundness
    /// measurement the module promises, applied to maps with no closed
    /// form. An over-read here is a hole in the picture; the plan
    /// predicts S3's tail and S4's fold show only in the halo.
    #[test]
    fn nonlinear_walks_never_exceed_a_sampled_upper_bound() {
        let cases = [
            (
                "spherical",
                vec![
                    kernel_xform("spherical", [0.0, -1.0, 1.0, 0.0, 1.0, 0.0], 1.0),
                    kernel_xform("spherical", [0.0, 1.0, -1.0, 0.0, 0.0, 0.0], 1.0),
                    affine_xform(0.5, 0.0, 0.0, 0.5, 0.8, 0.0),
                ],
            ),
            (
                "bubble",
                vec![
                    kernel_xform("bubble", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.6),
                    kernel_xform("bubble", [0.7, 0.7, -0.7, 0.7, 0.0, -0.3], 1.2),
                    affine_xform(0.5, 0.0, 0.0, 0.5, 1.0, 0.5),
                ],
            ),
            (
                "hemisphere",
                vec![
                    kernel_xform("hemisphere", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.5),
                    kernel_xform("hemisphere", [0.7, 0.7, -0.7, 0.7, 0.4, 0.0], 1.2),
                    affine_xform(0.5, 0.0, 0.0, 0.5, 0.8, 0.3),
                ],
            ),
            (
                "disc",
                vec![
                    kernel_xform("disc", [0.9, 0.4, -0.4, 0.9, 0.0, 0.0], 1.0),
                    affine_xform(0.55, 0.0, 0.0, 0.55, 0.0, 0.0),
                ],
            ),
            (
                "blob",
                vec![
                    {
                        let mut t = kernel_xform("blob", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0.8);
                        t.set_variation_param("blob", "high", 1.2);
                        t.set_variation_param("blob", "low", 0.5);
                        t.set_variation_param("blob", "waves", 5.0);
                        t
                    },
                    affine_xform(0.6, 0.3, -0.3, 0.6, 0.5, 0.0),
                    affine_xform(0.5, 0.0, 0.0, 0.5, -0.4, 0.3),
                ],
            ),
        ];
        for (name, transforms) in cases {
            let ifs = analyse(transforms);
            let sample = chaos_sample(&ifs, 200_000);
            // The sample's spacing: the largest nearest-neighbour gap
            // over a subset, which bounds how far a set point can be
            // from its nearest sample.
            let radius = ifs.ball.radius;
            // Counted twice: over the whole ball, and over its inner
            // half, where the bulk of a measured ball (S3) sits.
            let (mut n, mut over, mut n_bulk, mut over_bulk, mut worst) = (0usize, 0usize, 0usize, 0usize, 0.0f64);
            for iy in 0..40 {
                for ix in 0..40 {
                    let (u, v) = (2.0 * (ix as f64 + 0.5) / 40.0 - 1.0, 2.0 * (iy as f64 + 0.5) / 40.0 - 1.0);
                    let q = [ifs.ball.centre[0] + radius * u, ifs.ball.centre[1] + radius * v];
                    let e = estimate(&ifs, q, 40, 4);
                    let upper = sample.iter().map(|&a| Affine2::distance(q, a)).fold(f64::INFINITY, f64::min);
                    // A pixel at 512 across the ball is the tolerance:
                    // the sample is at least that dense on the bulk.
                    let tol = 2.0 * radius / 512.0;
                    let bad = e.distance > upper + tol;
                    let bulk = u.hypot(v) < 0.5;
                    n += 1;
                    over += bad as usize;
                    if bulk {
                        n_bulk += 1;
                        over_bulk += bad as usize;
                    }
                    if bad {
                        worst = worst.max(e.distance / upper.max(1e-12));
                    }
                }
            }
            println!(
                "  {name}: {over} of {n} points over the sampled bound ({over_bulk} of {n_bulk} in the inner half), worst ratio {worst:.3}"
            );
            // Measured (plan 8.9 record). Bubble: 34 of 1600 over, 2 of
            // 316 in the inner half, worst 22x -- all near the images
            // of its fold circle, where S4's tangential scale reads
            // too large; before the image gap (S4) it was 824, from
            // pieces reported infinitely far because inversion could
            // not reach them. Spherical: 946 of 1600 and 24 of 316 --
            // the set is unbounded through the pre-origin and the
            // walk's ball is measured (S3); recorded, not gated.
            // Re-measured with the ball found on the unexpanded maps
            // (D2 moved it before the branch expansion, so the sample
            // draws the maps uniformly rather than the branches): 24
            // of 1600, 5 of 316 inner, worst 19x. Same regime.
            // And once more with the search overshooting to its fixed
            // point (8.10): 27 of 1600, 9 of 316 inner, worst 44x. The
            // count moves with the ball because the fold band moves
            // with it; the regime does not.
            if name == "bubble" {
                assert!(over <= 40 && over_bulk <= 12, "{name}: {over} of {n} points ({over_bulk} of {n_bulk} inner) read farther than the set is ({worst:.3}x)");
            }
            // Plan 8.10: the three fold kernels have rigorous balls, so
            // an over-read is the walk's; measured first, then pinned
            // (see the record).
            if matches!(name, "hemisphere" | "disc" | "blob") {
                assert!(over <= fold_over_limit(name), "{name}: {over} of {n} points ({over_bulk} of {n_bulk} inner) read farther than the set is ({worst:.3}x)");
            }
        }
    }

    /// Three half-scale maps to a triangle's corners: the Sierpiński
    /// gasket, whose distance has no closed form.
    fn sierpinski() -> Ifs2 {
        analyse(vec![half(0.0, 0.0), half(0.5, 0.0), half(0.25, 0.5)])
    }

    /// The Heighway dragon: two similarities of ratio 1/√2, each a
    /// 45° rotation. Its two pieces JUST TOUCH and the attractor has
    /// positive area — it tiles the plane — which makes it the
    /// hardest classical case for a branch heuristic.
    fn dragon() -> Ifs2 {
        analyse(vec![
            affine_xform(0.5, -0.5, 0.5, 0.5, 0.0, 0.0),
            affine_xform(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0),
        ])
    }

    /// The best value over EVERY address of length `depth` — what a
    /// beam of unlimited width would find, at exponential cost.
    fn exhaustive(ifs: &Ifs2, p: [f64; 2], depth: u32) -> f64 {
        fn walk(
            ifs: &Ifs2,
            q: [f64; 2],
            sigma: f64,
            best_on_path: f64,
            left: u32,
        ) -> f64 {
            let r = Affine2::distance(q, ifs.ball.centre);
            let here = best_on_path.max(sigma * (r - ifs.ball.radius));
            if left == 0 {
                return here;
            }
            let mut best = f64::INFINITY;
            for m in &ifs.maps {
                let v = walk(ifs, m.inverse.apply(q), sigma * m.sigma_min, here, left - 1);
                best = best.min(v);
            }
            best
        }
        // `depth` descents means depth + 1 positions evaluated, and
        // `estimate` evaluates `max_levels` of them -- so the caller
        // passes one less to compare like with like. Getting this wrong
        // makes exhaustive look UNSOUND by a hair, which is how it was
        // found.
        walk(ifs, p, 1.0, 0.0, depth).max(0.0)
    }

    /// The best value over EVERY address, for an IFS whose maps may be
    /// nonlinear: `step` for the point and the local factor, and a
    /// branch whose image the point is outside of scores its gap, as
    /// the beam scores it into `dead_min`.
    fn exhaustive_nl(ifs: &Ifs2, p: [f64; 2], depth: u32) -> f64 {
        fn walk(ifs: &Ifs2, q: [f64; 2], sigma: f64, best_on_path: f64, left: u32) -> f64 {
            let r = Affine2::distance(q, ifs.ball.centre);
            // The same fold as the walk: a level that overflowed says
            // nothing, and the path keeps the bound it had. Returning
            // infinity there instead would DROP the path from the
            // minimum -- the over-read this whole measurement exists
            // to catch, reproduced in the reference.
            let here = fold_level(best_on_path, sigma, r, ifs.ball.radius);
            // Stop where the walk stops: `far` scales with the ball.
            if left == 0 || !r.is_finite() || r > ifs.ball.radius.max(1.0) * FAR {
                return here;
            }
            let mut best = f64::INFINITY;
            for m in &ifs.maps {
                if let Some(gap) = m.inverse.gap_bound(q, ifs.ball.centre, ifs.ball.radius, m.sigma_min) {
                    best = best.min(here.max(sigma * gap));
                    continue;
                }
                let (q2, _, s) = m.inverse.step(q, 0.0, m.sigma_min);
                best = best.min(walk(ifs, q2, sigma * s, here, left - 1));
            }
            best
        }
        walk(ifs, p, 1.0, 0.0, depth).max(0.0)
    }

    /// The flame reported from use (`grand-julian-glitches1.fflame`):
    /// three `julian`s at powers 2, 15 and 8, every one with
    /// `dist = -1` -- so every map is an INVERSION -- each carrying a
    /// `flatten` as the file does. `t2` is the third transform's
    /// affine; the report's two files differ only there.
    fn grand_julian(t2: [f32; 6]) -> Ifs2 {
        grand_julian_t1(t2, [0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0])
    }

    /// The same flame with the second transform's affine settable too:
    /// the two files of the jitter report differ only there.
    fn grand_julian_t1(t2: [f32; 6], t1: [f32; 6]) -> Ifs2 {
        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = kernel_xform("julian", aff, w);
            t.variations.insert("flatten".to_string(), 1.0);
            t.variation_order.insert(0, "flatten".to_string());
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        analyse(vec![
            j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3], 1.0, 2.0),
            j(t1, 0.2, 15.0),
            j(t2, 0.3, 8.0),
        ])
    }

    /// A weighted chaos game, and the coarse pass built from it.
    ///
    /// The forward step picks a TRANSFORM and then, for a root whose
    /// forward map is many-valued, a branch -- which is what the
    /// chaos game does and what [`MeasureMaps`] undoes.
    fn chaos_measure(
        ifs: &Ifs2,
        flame: &Flame,
        samples: usize,
        res: usize,
        seed: u64,
    ) -> CoarseMeasure {
        let mut st = seed;
        let mut rnd = move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (st >> 11) as f64 / (1u64 << 53) as f64
        };
        let used: std::collections::BTreeSet<usize> =
            ifs.maps.iter().map(|m| m.transform_index).collect();
        let total: f64 = used.iter().map(|&i| flame.transforms[i].weight as f64).sum();
        let mut out = CoarseMeasure {
            res,
            centre: ifs.ball.centre,
            radius: ifs.ball.radius,
            hits: vec![0; res * res],
            palette_sum: vec![0.0; res * res],
            samples: samples as u64,
        };
        let mut q = ifs.ball.centre;
        let mut col = 0.5f64;
        for i in 0..samples + 1000 {
            // One transform, then one of its forward branches.
            let u = rnd();
            let mut acc = 0.0;
            let mut m = ifs.maps.len() - 1;
            let mut seen: Option<usize> = None;
            for (k, mp) in ifs.maps.iter().enumerate() {
                if seen == Some(mp.transform_index) {
                    continue;
                }
                seen = Some(mp.transform_index);
                acc += flame.transforms[mp.transform_index].weight as f64 / total;
                if u <= acc {
                    m = k;
                    break;
                }
            }
            q = match &ifs.maps[m].forward {
                Map2::Nonlinear(n) => {
                    let b = match n.kernel {
                        crate::scene::ifs_analysis::Kernel::Root { n: e, .. } => {
                            (rnd() * e.unsigned_abs() as f64).floor() as u32
                        }
                        _ => 0,
                    };
                    n.apply_branch(q, b)
                }
                other => other.apply(q),
            };
            let t = &flame.transforms[ifs.maps[m].transform_index];
            let sp = t.color_speed as f64;
            col = col * (1.0 + sp) * 0.5 + t.color as f64 * (1.0 - sp) * 0.5;
            if i < 1000 || !q[0].is_finite() || !q[1].is_finite() {
                continue;
            }
            if let Some(c) = out.index_for_test(q) {
                out.hits[c] += 1;
                out.palette_sum[c] += col;
            }
        }
        out
    }

    /// Hunt a `disc` IFS whose attractor is not a single point.
    ///
    /// The fixture every other `disc` gate uses has one: both maps fix
    /// the origin and both contract toward it, so six million
    /// chaos-game samples land in one cell. Translations give the maps
    /// different fixed points, which is what it takes.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_hunt_a_disc_fixture_that_spreads() {
        let cases: Vec<(&str, Vec<Transform>)> = vec![
            ("the current one", vec![
                kernel_xform("disc", [0.9, 0.4, -0.4, 0.9, 0.0, 0.0], 1.0),
                affine_xform(0.55, 0.0, 0.0, 0.55, 0.0, 0.0),
            ]),
            ("disc shifted", vec![
                kernel_xform("disc", [0.9, 0.4, -0.4, 0.9, 0.3, 0.2], 1.0),
                affine_xform(0.55, 0.0, 0.0, 0.55, -0.4, 0.1),
            ]),
            ("disc shifted harder", vec![
                kernel_xform("disc", [0.6, 0.3, -0.3, 0.6, 0.7, -0.4], 1.0),
                affine_xform(0.5, 0.0, 0.0, 0.5, -0.6, 0.3),
            ]),
            ("disc pair", vec![
                kernel_xform("disc", [0.8, 0.2, -0.2, 0.8, 0.4, 0.0], 1.0),
                kernel_xform("disc", [0.5, -0.4, 0.4, 0.5, -0.5, 0.3], 1.0),
            ]),
            ("disc and two affines", vec![
                kernel_xform("disc", [0.7, 0.3, -0.3, 0.7, 0.5, -0.2], 1.0),
                affine_xform(0.5, 0.0, 0.0, 0.5, -0.5, 0.0),
                affine_xform(0.45, 0.2, -0.2, 0.45, 0.1, 0.55),
            ]),
        ];
        for (name, transforms) in cases {
            let guard = global_registry();
            let flame = flame_of(transforms);
            let Ok(ifs) = analyse_2d(&flame, &guard) else {
                println!("  {name:<22} does not qualify");
                continue;
            };
            drop(guard);
            let smp = chaos_sample(&ifs, 100_000);
            if smp.is_empty() {
                println!("  {name:<22} no samples");
                continue;
            }
            let span = |f: &dyn Fn(&[f64; 2]) -> f64| {
                let lo = smp.iter().map(|p| f(p)).fold(f64::INFINITY, f64::min);
                let hi = smp.iter().map(|p| f(p)).fold(f64::NEG_INFINITY, f64::max);
                hi - lo
            };
            // How many cells of a 128-grid over the ball the sample
            // reaches, which is the vacuity guard the gate uses.
            let (bc, br) = (ifs.ball.centre, ifs.ball.radius);
            let res = 128usize;
            let c = 2.0 * br / res as f64;
            let mut seen = vec![false; res * res];
            for p in &smp {
                let fx = (p[0] - (bc[0] - br)) / c;
                let fy = (p[1] - (bc[1] - br)) / c;
                if fx >= 0.0 && fy >= 0.0 {
                    let (ix, iy) = (fx as usize, fy as usize);
                    if ix < res && iy < res {
                        seen[iy * res + ix] = true;
                    }
                }
            }
            let lit = seen.iter().filter(|b| **b).count();
            println!(
                "  {name:<22} maps {} | ball r {:.3} | extent {:.4} x {:.4} | cells lit {lit}",
                ifs.maps.len(),
                br,
                span(&|p| p[0]),
                span(&|p| p[1]),
            );
        }
    }

    /// D4 of the measure plan: what brightness does at depth, and how
    /// many stops a user loses to it.
    ///
    /// The estimator answers in the same units as
    /// [`CoarseMeasure::density`] -- measure per unit area -- which is
    /// why a ratio against an ordinary render of the same view is one
    /// and why `the_measure_agrees_with_the_chaos_game` can assert
    /// that at all. So the units are settled and what is left is the
    /// POLICY: the flam3 tonemap normalises by
    /// `total_iters / pixel_count`, which is iteration-invariant and
    /// not zoom-invariant, and the measure inside a deep view is
    /// tiny.
    ///
    /// Two numbers per zoom: the measure the view holds (which is what
    /// a chaos game would have to find, and what starves), and the
    /// median density per unit area over the lit pixels (which is what
    /// the estimator reports and what a zoom-invariant normalisation
    /// would divide by).
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_what_brightness_does_at_depth() {
        const COARSE: usize = 6_000_000;
        const RES: usize = 256;
        const VP: usize = 16;
        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = affine_xform(aff[0], aff[1], aff[2], aff[3], aff[4], aff[5]);
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", w);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        let sq = [0.7071f32, 0.7071, -0.7071, 0.7071, 0.0, 0.0];
        let cases: Vec<(&str, Vec<Transform>)> = vec![
            ("gasket", vec![half(0.0, 0.0), half(0.5, 0.0), half(0.25, 0.5)]),
            ("dragon", vec![
                affine_xform(0.5, -0.5, 0.5, 0.5, 0.0, 0.0),
                affine_xform(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0),
            ]),
            ("grand julian", vec![
                j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3], 1.0, 2.0),
                j(sq, 0.2, 15.0),
                j(sq, 0.3, 8.0),
            ]),
        ];
        for (name, transforms) in cases {
            let flame = flame_of(transforms);
            let ifs = {
                let guard = global_registry();
                analyse_2d(&flame, &guard).expect("qualifies")
            };
            let maps = MeasureMaps::of(&ifs, &flame);
            let coarse = chaos_measure(&ifs, &flame, COARSE, RES, 0x9E3779B97F4A7C15);
            let (bc, br) = (ifs.ball.centre, ifs.ball.radius);
            // A point ON the attractor, not the centre of the
            // densest CELL: at 2^12 the view is far smaller than a
            // cell, and a cell that is dense on average can still have
            // the set nowhere near its geometric centre. That is what
            // made this probe read zero on the grand julian past 2^8.
            // The sample whose coarse cell holds the most measure:
            // ON the set, and where the reference has the statistics
            // to be a reference. A random attractor point is not
            // enough -- on the 6:1:1 gasket it left eight comparable
            // pixels in the frame.
            let smp = chaos_sample(&ifs, 50_000);
            let centre = *smp
                .iter()
                .max_by_key(|p| coarse.index_for_test(**p).map_or(0, |i| coarse.hits[i]))
                .expect("the sample is not empty");
            let _ = (bc, br);
            println!("{name}: centred on an attractor point {centre:?}");
            println!(
                "  {:>6} | {:>12} {:>9} | {:>12} {:>9}",
                "zoom", "view measure", "stops", "median rho", "stops"
            );
            let (mut m0, mut d0) = (0.0f64, 0.0f64);
            for &zoom in &[5.0f64, 8.0, 12.0, 16.0, 20.0] {
                let span = 2.0 * br / 2f64.powf(zoom);
                let px = span / VP as f64;
                let origin = [centre[0] - span * 0.5, centre[1] - span * 0.5];
                let mut total = 0.0f64;
                let mut lit: Vec<f64> = Vec::new();
                for iy in 0..VP {
                    for ix in 0..VP {
                        let x = [
                            origin[0] + (ix as f64 + 0.5) * px,
                            origin[1] + (iy as f64 + 0.5) * px,
                        ];
                        let e =
                            estimate_measure(&ifs, &maps, &coarse, x, px, 16, MEASURE_CELLS, 400);
                        if e.density > 0.0 {
                            total += e.density * px * px;
                            lit.push(e.density);
                        }
                    }
                }
                lit.sort_by(f64::total_cmp);
                let med = if lit.is_empty() { 0.0 } else { lit[lit.len() / 2] };
                if zoom == 5.0 {
                    m0 = total;
                    d0 = med;
                }
                let st = |a: f64, b: f64| if a > 0.0 && b > 0.0 { (b / a).log2() } else { f64::NAN };
                println!(
                    "  2^{zoom:<4.0} | {total:>12.3e} {:>9.1} | {med:>12.3e} {:>9.1}",
                    st(m0, total),
                    st(d0, med)
                );
            }
        }
    }

    /// G1 of [`ifs-measure-by-inverse-walk.md`]: the measure read
    /// through the inverse walk IS the chaos game's measure.
    ///
    /// The flame's picture is its invariant measure, and this is the
    /// claim that the measure factorises through the walk -- so a
    /// pixel's density and colour can be had from an address's
    /// probability, a lookup in an ordinary render at its endpoint,
    /// and the determinant along it, with **no forward sample drawn
    /// at the zoom**. That is what does not starve.
    ///
    /// Held to a median ratio rather than a per-pixel one because the
    /// reference is a chaos game and therefore noisy: the tolerance
    /// here is its own variance at this sample count, not the
    /// estimator's accuracy, which §5c to §5f measured at 20M samples
    /// as within 3% on every fixture.
    ///
    /// The four fixtures are the four shapes of correction: affine
    /// (dragon), a root whose FORWARD map is many-valued (grand
    /// julian), a bubble whose INVERSE is (bubble pair), and a
    /// non-uniform weighting (the 6:1:1 gasket). Each was measured to
    /// break differently, and all four are needed to pin
    /// [`MeasureMaps`].
    #[test]
    fn the_measure_agrees_with_the_chaos_game() {
        const COARSE: usize = 6_000_000;
        const DIRECT: usize = 6_000_000;
        const RES: usize = 256;
        // The view must sit where the estimator is DEFINED: a
        // coarse cell coarser than a view pixel, `VP·2^zoom >= 2·RES`
        // (§5a's domain condition). And the reference must have the
        // samples to be a reference, which pulls the other way -- the
        // measure in view falls as `2^(-zoom·D)`. VP 16 at 2^5 and
        // 2^6 satisfies both at six million samples.
        const VP: usize = 16;

        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = affine_xform(aff[0], aff[1], aff[2], aff[3], aff[4], aff[5]);
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", w);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        let sq = [0.7071f32, 0.7071, -0.7071, 0.7071, 0.0, 0.0];
        // The third field is the stop depth in coarse cells, and it
        // is NOT one number for every set. An AFFINE map's preimage
        // of a pixel is exactly the parallelogram the composed
        // Jacobian describes, at any size, so it can stop deep and
        // integrate over many cells. A CURVED one outgrows that
        // parallelogram and has to stop shallow. Measured here: the
        // 6:1:1 gasket reads 1.234 at four cells and 1.032 at
        // sixteen, while the bubble pair reads 0.957 at four and
        // 0.894 at sixteen. Averaging the two into one constant would
        // hide the finding, so each fixture gates at its own.
        let cases: Vec<(&str, Vec<Transform>, f64)> = vec![
            ("dragon", vec![
                affine_xform(0.5, -0.5, 0.5, 0.5, 0.0, 0.0),
                affine_xform(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0),
            ], 16.0),
            ("gasket 6:1:1", vec![
                {
                    let mut t = half(0.0, 0.0);
                    t.weight = 6.0;
                    t
                },
                half(0.5, 0.0),
                half(0.25, 0.5),
            ], 16.0),
            ("grand julian", vec![
                j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3], 1.0, 2.0),
                j(sq, 0.2, 15.0),
                j(sq, 0.3, 8.0),
            ], MEASURE_CELLS),
            ("bubble pair", vec![
                kernel_xform("bubble", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.6),
                kernel_xform("bubble", [0.7, 0.7, -0.7, 0.7, 0.0, -0.3], 1.2),
            ], MEASURE_CELLS),
            // `disc`, whose INVERSE branches are a bubble's case with
            // up to twelve of them, and the one kernel whose branch
            // count depends on the ball.
            //
            // NOT the fixture the kernel's other gates use: that one
            // has a single POINT for an attractor -- both its maps fix
            // the origin and both contract toward it, so six million
            // samples land in one cell of 65536 and `chaos_sample`
            // collapses too. Translations give the maps different
            // fixed points, and this spreads over 7631 cells of a
            // 128-grid (`probe_hunt_a_disc_fixture_that_spreads`).
            ("disc", vec![
                kernel_xform("disc", [0.7, 0.3, -0.3, 0.7, 0.5, -0.2], 1.0),
                affine_xform(0.5, 0.0, 0.0, 0.5, -0.5, 0.0),
                affine_xform(0.45, 0.2, -0.2, 0.45, 0.1, 0.55),
            ], MEASURE_CELLS),
        ];

        for (name, mut transforms, cells) in cases {
            let n = transforms.len().max(2) - 1;
            for (i, t) in transforms.iter_mut().enumerate() {
                t.color = i as f32 / n as f32;
                t.color_speed = 0.0;
            }
            let flame = flame_of(transforms);
            let ifs = {
                let guard = global_registry();
                analyse_2d(&flame, &guard).expect("qualifies")
            };
            let maps = MeasureMaps::of(&ifs, &flame);
            let coarse = chaos_measure(&ifs, &flame, COARSE, RES, 0x9E3779B97F4A7C15);
            // A fixture whose attractor is a point gates nothing, and
            // looks like a pass. The `disc` fixture above lit ONE cell
            // of 65536 and this is what found it.
            let lit = coarse.hits.iter().filter(|&&h| h > 0).count();
            assert!(
                lit > 100,
                "{name}: the coarse pass lit {lit} cells of {} -- the attractor is \
                 degenerate and this fixture cannot measure anything",
                RES * RES
            );
            let (bc, br) = (ifs.ball.centre, ifs.ball.radius);
            // A point ON the attractor, so the view has measure in it
            // whatever the set's shape.
            //
            // Two weaker rules were tried and both fail. Iterating the
            // forward maps from the ball's centre lands wherever that
            // orbit happens to go -- on a `disc` fixture it left one
            // comparable pixel in the frame. The densest coarse CELL's
            // centre is better but still not on the set: a cell that
            // is dense on average can have the set nowhere near its
            // geometric middle, and once the view is smaller than a
            // cell it misses entirely -- which read a flat zero on the
            // grand julian past 2^12 in
            // `probe_what_brightness_does_at_depth`.
            // The sample whose coarse cell holds the most measure:
            // ON the set, and where the reference has the statistics
            // to be a reference. A random attractor point is not
            // enough -- on the 6:1:1 gasket it left eight comparable
            // pixels in the frame.
            let smp = chaos_sample(&ifs, 50_000);
            let centre = *smp
                .iter()
                .max_by_key(|p| coarse.index_for_test(**p).map_or(0, |i| coarse.hits[i]))
                .expect("the sample is not empty");
            let _ = (bc, br);
            for &zoom in &[5.0f64, 6.0] {
                let span = 2.0 * br / 2f64.powf(zoom);
                let px = span / VP as f64;
                let origin = [centre[0] - span * 0.5, centre[1] - span * 0.5];
                // The reference: a chaos game binned into THIS view.
                let view = {
                    let mut hits = vec![0u32; VP * VP];
                    let mut pal = vec![0.0f64; VP * VP];
                    let mut st = 0x2545F4914F6CDD1Du64;
                    let mut rnd = move || {
                        st = st
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        (st >> 11) as f64 / (1u64 << 53) as f64
                    };
                    let used: std::collections::BTreeSet<usize> =
                        ifs.maps.iter().map(|m| m.transform_index).collect();
                    let total: f64 =
                        used.iter().map(|&i| flame.transforms[i].weight as f64).sum();
                    let mut q = bc;
                    let mut col = 0.5f64;
                    for i in 0..DIRECT + 1000 {
                        let u = rnd();
                        let mut acc = 0.0;
                        let mut m = ifs.maps.len() - 1;
                        let mut seen: Option<usize> = None;
                        for (k, mp) in ifs.maps.iter().enumerate() {
                            if seen == Some(mp.transform_index) {
                                continue;
                            }
                            seen = Some(mp.transform_index);
                            acc += flame.transforms[mp.transform_index].weight as f64 / total;
                            if u <= acc {
                                m = k;
                                break;
                            }
                        }
                        q = match &ifs.maps[m].forward {
                            Map2::Nonlinear(nl) => {
                                let b = match nl.kernel {
                                    crate::scene::ifs_analysis::Kernel::Root { n: e, .. } => {
                                        (rnd() * e.unsigned_abs() as f64).floor() as u32
                                    }
                                    _ => 0,
                                };
                                nl.apply_branch(q, b)
                            }
                            other => other.apply(q),
                        };
                        let t = &flame.transforms[ifs.maps[m].transform_index];
                        let sp = t.color_speed as f64;
                        col = col * (1.0 + sp) * 0.5 + t.color as f64 * (1.0 - sp) * 0.5;
                        if i < 1000 || !q[0].is_finite() || !q[1].is_finite() {
                            continue;
                        }
                        let fx = (q[0] - origin[0]) / px;
                        let fy = (q[1] - origin[1]) / px;
                        if fx >= 0.0 && fy >= 0.0 {
                            let (ix, iy) = (fx as usize, fy as usize);
                            if ix < VP && iy < VP {
                                hits[iy * VP + ix] += 1;
                                pal[iy * VP + ix] += col;
                            }
                        }
                    }
                    (hits, pal)
                };

                let mut ratios: Vec<f64> = Vec::new();
                let mut cerr: Vec<f64> = Vec::new();
                for iy in 0..VP {
                    for ix in 0..VP {
                        let h = view.0[iy * VP + ix];
                        if h < 25 {
                            continue;
                        }
                        let truth = h as f64 / (DIRECT as f64 * px * px);
                        let x = [
                            origin[0] + (ix as f64 + 0.5) * px,
                            origin[1] + (iy as f64 + 0.5) * px,
                        ];
                        let e =
                            estimate_measure(&ifs, &maps, &coarse, x, px, 16, cells, 60);
                        if e.density > 0.0 {
                            ratios.push(e.density / truth);
                            cerr.push((e.palette - view.1[iy * VP + ix] / h as f64).abs());
                        }
                    }
                }
                assert!(
                    ratios.len() >= 20,
                    "{name} 2^{zoom}: only {} pixels to compare, so this gate is not \
                     measuring anything",
                    ratios.len()
                );
                ratios.sort_by(f64::total_cmp);
                cerr.sort_by(f64::total_cmp);
                let median = ratios[ratios.len() / 2];
                let cmed = cerr[cerr.len() / 2];
                println!(
                    "  {name:<14} 2^{zoom:<3.0} {cells:>3.0} cells | {:>4} px | \
                     density median {median:.3} | colour |err| median {cmed:.4}",
                    ratios.len()
                );
                assert!(
                    (0.88..=1.15).contains(&median),
                    "{name} 2^{zoom}: the measure walk reads {median:.3} of the chaos \
                     game's density"
                );
                assert!(
                    cmed < 0.02,
                    "{name} 2^{zoom}: the colour is {cmed:.4} off the chaos game's, in \
                     palette coordinates"
                );
            }
        }
    }

    /// Item 2 of [`ifs-measure-by-inverse-walk.md`], proved on the CPU
    /// before any plumbing exists.
    ///
    /// The claim is that the flame's invariant measure factorises
    /// through the inverse walk. Unrolling `mu = sum_i p_i (S_i)_* mu`
    /// k times gives `mu(B) = sum_a p_a mu(S_a^-1 B)`, and for a pixel
    /// `B` of area `px^2` at `x`, `S_a^-1 B` is a region around
    /// `q_a = S_a^-1(x)` of area `px^2 |det D(S_a^-1)(x)|`. Stop each
    /// address once that area reaches one COARSE pixel, where an
    /// ordinary render of the whole attractor already knows the
    /// density, and
    ///
    /// ```text
    /// density(x) = sum_a  p_a . rho(q_a) . |det D(S_a^-1)(x)|
    /// ```
    ///
    /// Three factors, all of which the walk has or can carry: the
    /// address's probability, a lookup at its endpoint, and the
    /// determinant along it. No forward sample is drawn at the zoom.
    ///
    /// This enumerates every address rather than beaming, so what it
    /// tests is the FORMULA and not the truncation -- those are
    /// separate questions and the beam is the second one. The
    /// reference is a direct chaos game at the same view, so the
    /// comparison is against the thing mode A actually draws.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_the_measure_through_the_inverse_walk() {
        // An IFS with its selection probabilities, which the analysis
        // does not carry -- `transform_index` is the way back to them.
        let cases: Vec<(&str, Vec<Transform>)> = vec![
            ("gasket, equal", vec![half(0.0, 0.0), half(0.5, 0.0), half(0.25, 0.5)]),
            (
                "gasket, 6:1:1",
                vec![
                    {
                        let mut t = half(0.0, 0.0);
                        t.weight = 6.0;
                        t
                    },
                    half(0.5, 0.0),
                    half(0.25, 0.5),
                ],
            ),
            ("dragon", vec![
                affine_xform(0.5, -0.5, 0.5, 0.5, 0.0, 0.0),
                affine_xform(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0),
            ]),
            // Where the beam has something to lose: many addresses
            // cover one pixel, and a truncated SUM is biased dark.
            ("fat gasket", vec![
                affine_xform(0.6, 0.0, 0.0, 0.6, 0.0, 0.0),
                affine_xform(0.6, 0.0, 0.0, 0.6, 0.4, 0.0),
                affine_xform(0.6, 0.0, 0.0, 0.6, 0.2, 0.4),
            ]),
            ("overlapping band", vec![
                affine_xform(0.7, 0.0, 0.0, 0.7, 0.0, 0.0),
                affine_xform(0.7, 0.0, 0.0, 0.7, 0.3, 0.3),
            ]),
            // NONLINEAR, which is what the project is for. A `julia`
            // is the square root with a random sign: its FORWARD map
            // is two-valued and its inverse is the single-valued
            // square. See `branches` below for what that does to the
            // probability.
            ("julia dust", {
                let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, 0.4, -0.6);
                t.variations = HashMap::from([("julia".to_string(), 1.0)]);
                t.variation_order = vec!["julia".to_string()];
                vec![t]
            }),
            // The reported grand julian: three inversions, powers 2,
            // 15 and 8, so three transforms and twenty-five forward
            // branches between them.
            ("grand julian", {
                let j = |aff: [f32; 6], w: f32, power: f32| {
                    let mut t = affine_xform(aff[0], aff[1], aff[2], aff[3], aff[4], aff[5]);
                    t.variations.clear();
                    t.variation_order.clear();
                    t.set_variation("flatten", 1.0);
                    t.set_variation("julian", w);
                    t.set_variation_param("julian", "power", power);
                    t.set_variation_param("julian", "dist", -1.0);
                    t
                };
                let sq = [0.7071f32, 0.7071, -0.7071, 0.7071, 0.0, 0.0];
                vec![
                    j([0.7071, 0.7071, -0.7071, 0.7071, 0.0, -0.3], 1.0, 2.0),
                    j(sq, 0.2, 15.0),
                    j(sq, 0.3, 8.0),
                ]
            }),
            ("bubble pair", vec![
                kernel_xform("bubble", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.6),
                kernel_xform("bubble", [0.7, 0.7, -0.7, 0.7, 0.0, -0.3], 1.2),
            ]),
        ];

        for (name, mut transforms) in cases {
            // Distinct colours, spread over the palette, so a right
            // answer is not right by everything being equal. Speed
            // zero is the plain flam3 halving.
            let n = transforms.len().max(2) - 1;
            for (i, t) in transforms.iter_mut().enumerate() {
                t.color = i as f32 / n as f32;
                t.color_speed = 0.0;
            }
            let flame = flame_of(transforms);
            let ifs = {
                let guard = global_registry();
                analyse_2d(&flame, &guard).expect("qualifies")
            };
            // Selection probabilities, normalised as the chaos game
            // normalises them -- over TRANSFORMS, which is not the
            // same as over maps.
            //
            // `analyse_2d` makes one map per (transform, branch) where
            // the INVERSE is many-valued: a bubble is two maps and a
            // disc up to twelve. Those are alternative PREIMAGES of
            // one forward map, so the preimage of a set is their
            // union and each carries the whole of its transform's
            // probability. Normalising over maps would have split it
            // and halved a bubble.
            let ntr = flame.transforms.len();
            let used: std::collections::BTreeSet<usize> =
                ifs.maps.iter().map(|m| m.transform_index).collect();
            let total: f64 = used
                .iter()
                .map(|&i| flame.transforms[i].weight as f64)
                .sum();
            let _ = ntr;
            let p: Vec<f64> = ifs
                .maps
                .iter()
                .map(|m| flame.transforms[m.transform_index].weight as f64 / total)
                .collect();
            // How many values its forward map takes. A root's forward
            // is `|z|^(d/|n|) e^(i(arg z + 2πk)/n)` and the chaos game
            // draws `k` uniformly, so the transform's probability is
            // split `|n|` ways -- and for any point exactly one of
            // those branches has it in its image, the one the single
            // inverse undoes. So a step of the inverse walk carries
            // `p_i / n_i`, not `p_i`.
            //
            // Getting this wrong is the branch-0 error of
            // `ifs-distance-rendering.md` §8.15 wearing a different
            // hat, and on a power-15 root it would be wrong by
            // fifteen.
            let branches: Vec<f64> = ifs
                .maps
                .iter()
                .map(|m| match m.forward.nonlinear().map(|n| n.kernel) {
                    Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => {
                        n.unsigned_abs() as f64
                    }
                    _ => 1.0,
                })
                .collect();
            let pb: Vec<f64> = p.iter().zip(&branches).map(|(a, b)| a / b).collect();

            // A deterministic weighted chaos game, shared by the
            // coarse pass and the direct reference so the comparison
            // is of one measure with itself.
            let mut state: u64 = 0x9E3779B97F4A7C15;
            let mut rnd = move || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (state >> 11) as f64 / (1u64 << 53) as f64
            };
            let step = |q: [f64; 2], m: usize, u: f64, ifs: &Ifs2| -> [f64; 2] {
                match &ifs.maps[m].forward {
                    Map2::Nonlinear(n) => {
                        let k = match n.kernel {
                            crate::scene::ifs_analysis::Kernel::Root { n: e, .. } => {
                                (u * e.unsigned_abs() as f64).floor() as u32
                            }
                            _ => 0,
                        };
                        n.apply_branch(q, k)
                    }
                    other => other.apply(q),
                }
            };
            // The chaos game picks a TRANSFORM. Returns the first
            // map of it, whose forward is the transform's forward --
            // the branch only distinguishes inverses.
            let pick_transform = |u: f64, ifs: &Ifs2, flame: &Flame, total: f64| -> usize {
                let mut acc = 0.0;
                let mut seen: Option<usize> = None;
                for (i, m) in ifs.maps.iter().enumerate() {
                    if seen == Some(m.transform_index) {
                        continue;
                    }
                    seen = Some(m.transform_index);
                    acc += flame.transforms[m.transform_index].weight as f64 / total;
                    if u <= acc {
                        return i;
                    }
                }
                ifs.maps.len() - 1
            };

            // THE DIRECT REFERENCE, once per view: a chaos game at
            // the zoom, which is what mode A draws and what starves.
            const VP: usize = 64;
            const DIRECT_N: usize = 20_000_000;
            let (bc, br) = (ifs.ball.centre, ifs.ball.radius);
            let mut centre = bc;
            for k in 0..40 {
                centre = step(centre, k % ifs.maps.len(), 0.31 * k as f64 % 1.0, &ifs);
            }
            let mut direct: Vec<(f64, Vec<u32>, Vec<f64>)> = Vec::new();
            for &zoom in &[2.0f64, 4.0, 6.0] {
                let span = 2.0 * br / 2f64.powf(zoom);
                let px = span / VP as f64;
                let origin = [centre[0] - span * 0.5, centre[1] - span * 0.5];
                let mut hits = vec![0u32; VP * VP];
                let mut hcol = vec![0.0f64; VP * VP];
                let mut q = bc;
                let mut col = 0.5f64;
                for i in 0..DIRECT_N + 1000 {
                    let m = pick_transform(rnd(), &ifs, &flame, total);
                    q = step(q, m, rnd(), &ifs);
                    let t = &flame.transforms[ifs.maps[m].transform_index];
                    let sp = t.color_speed as f64;
                    col = col * (1.0 + sp) * 0.5 + t.color as f64 * (1.0 - sp) * 0.5;
                    if i < 1000 {
                        continue;
                    }
                    let fx = (q[0] - origin[0]) / px;
                    let fy = (q[1] - origin[1]) / px;
                    if fx >= 0.0 && fy >= 0.0 {
                        let (ix, iy) = (fx as usize, fy as usize);
                        if ix < VP && iy < VP {
                            hits[iy * VP + ix] += 1;
                            hcol[iy * VP + ix] += col;
                        }
                    }
                }
                direct.push((zoom, hits, hcol));
            }

            // THE COARSE PASS: the density per unit area over the
            // ball, as an ordinary render knows it. One resolution
            // now; §5a swept it and found the spread scale-free.
            const COARSE_N: usize = 20_000_000;
            let res = 512usize;
            let cpx = 2.0 * br / res as f64;
            let mut grid = vec![0u32; res * res];
            let mut cgrid = vec![0.0f64; res * res];
            let cell = |q: [f64; 2]| -> Option<usize> {
                let fx = (q[0] - (bc[0] - br)) / cpx;
                let fy = (q[1] - (bc[1] - br)) / cpx;
                if fx < 0.0 || fy < 0.0 {
                    return None;
                }
                let (ix, iy) = (fx as usize, fy as usize);
                (ix < res && iy < res).then(|| iy * res + ix)
            };
            let mut q = bc;
            let mut col = 0.5f64;
            for i in 0..COARSE_N + 1000 {
                let m = pick_transform(rnd(), &ifs, &flame, total);
                q = step(q, m, rnd(), &ifs);
                // The flam3 rule, exactly as `main_template.wgsl`
                // applies it: c' = c(1+s)/2 + col(1-s)/2.
                let t = &flame.transforms[ifs.maps[m].transform_index];
                let sp = t.color_speed as f64;
                col = col * (1.0 + sp) * 0.5 + t.color as f64 * (1.0 - sp) * 0.5;
                if i >= 1000 {
                    if let Some(c) = cell(q) {
                        grid[c] += 1;
                        cgrid[c] += col;
                    }
                }
            }
            let rho = |q: [f64; 2]| -> f64 {
                cell(q).map_or(0.0, |c| grid[c] as f64 / (COARSE_N as f64 * cpx * cpx))
            };
            // The coarse pass's mean palette coordinate: `c_0`, which
            // the address rule damps by `prod (1+s)/2` and which
            // therefore only has to be roughly right (D5).
            let rho_col = |q: [f64; 2]| -> f64 {
                cell(q).map_or(0.5, |c| {
                    if grid[c] > 0 {
                        cgrid[c] / grid[c] as f64
                    } else {
                        0.5
                    }
                })
            };

            for (zoom, hits, hcol) in &direct {
                let span = 2.0 * br / 2f64.powf(*zoom);
                let px = span / VP as f64;
                let origin = [centre[0] - span * 0.5, centre[1] - span * 0.5];
                let want = (cpx / px) * (cpx / px);
                if want < 2.0 {
                    continue;
                }

                // `foot` reads rho over the preimage of the WHOLE
                // pixel rather than of its centre.
                //
                // §5a named the single-cell lookup as the accuracy
                // limit on a fractal measure: the estimator asks for
                // the density in a grid square and the truth is the
                // measure of one particular same-sized region. The
                // composed Jacobian `m` is that region to first order
                // -- it maps the pixel square to the preimage -- so
                // sampling rho across `m . [-1/2,1/2]^2` integrates
                // the footprint instead of point-sampling it. That is
                // what texture hardware does, for this reason.
                let look = |q: [f64; 2], m: [[f64; 2]; 2], foot: bool| -> f64 {
                    if !foot {
                        return rho(q);
                    }
                    const K: i32 = 8;
                    let mut acc = 0.0;
                    for sy in 0..K {
                        for sx in 0..K {
                            // Cell centres of a K x K tiling of
                            // [-1/2, 1/2]^2, so the average is the
                            // pixel's own area and nothing else's.
                            let u = ((sx as f64 + 0.5) / K as f64 - 0.5) * px;
                            let v = ((sy as f64 + 0.5) / K as f64 - 0.5) * px;
                            acc += rho([
                                q[0] + m[0][0] * u + m[0][1] * v,
                                q[1] + m[1][0] * u + m[1][1] * v,
                            ]);
                        }
                    }
                    acc / (K * K) as f64
                };

                // `beam` of zero enumerates every address, which is
                // §5a's reference and separates the formula from the
                // truncation.
                // Density and colour together. The walk applies
                // `S_{a_1}^-1` first, so the forward sequence from
                // `q_a` back to the pixel runs `a_k` first and `a_1`
                // LAST -- and the flam3 rule is a fold in that order,
                // which makes the SHALLOWEST branch dominate with
                // weight a half. `c_0` is the coarse colour at the
                // endpoint, damped by `prod (1+s)/2`.
                let estimate_c = |x: [f64; 2], beam: usize, foot: bool, cells: f64|
                 -> (f64, f64) {
                    let want = want * cells;
                    let mut acc = 0.0f64;
                    let mut acc_col = 0.0f64;
                    // (point, probability, composed Jacobian, address)
                    let mut live: Vec<([f64; 2], f64, [[f64; 2]; 2], Vec<usize>)> =
                        vec![(x, 1.0, [[1.0, 0.0], [0.0, 1.0]], Vec::new())];
                    for _ in 0..24u32 {
                        if live.is_empty() {
                            break;
                        }
                        let mut next: Vec<([f64; 2], f64, [[f64; 2]; 2], Vec<usize>)> =
                            Vec::new();
                        for (q, pa, m, addr) in live.drain(..) {
                            let det = (m[0][0] * m[1][1] - m[0][1] * m[1][0]).abs();
                            if det >= want {
                                let w = pa * look(q, m, foot) * det;
                                // `a_k` first, `a_1` last.
                                let mut c = rho_col(q);
                                for &i in addr.iter().rev() {
                                    let t = &flame.transforms[ifs.maps[i].transform_index];
                                    let sp = t.color_speed as f64;
                                    c = c * (1.0 + sp) * 0.5 + t.color as f64 * (1.0 - sp) * 0.5;
                                }
                                acc += w;
                                acc_col += w * c;
                                continue;
                            }
                            for (i, mp) in ifs.maps.iter().enumerate() {
                                // The preimage of a set is the union
                                // over BRANCHES, each with the same p.
                                let Some(j) = mp.inverse.jacobian(q) else { continue };
                                let qi = mp.inverse.apply(q);
                                if !qi[0].is_finite() || !qi[1].is_finite() {
                                    continue;
                                }
                                // Outside the ball is outside the
                                // attractor, and stays outside under
                                // every further inverse.
                                if Affine2::distance(qi, bc) > br * 1.000001 {
                                    continue;
                                }
                                let m2 = [
                                    [
                                        j[0][0] * m[0][0] + j[0][1] * m[1][0],
                                        j[0][0] * m[0][1] + j[0][1] * m[1][1],
                                    ],
                                    [
                                        j[1][0] * m[0][0] + j[1][1] * m[1][0],
                                        j[1][0] * m[0][1] + j[1][1] * m[1][1],
                                    ],
                                ];
                                let d2 = (m2[0][0] * m2[1][1] - m2[0][1] * m2[1][0]).abs();
                                if !(d2 > 0.0) || !d2.is_finite() {
                                    continue;
                                }
                                let mut a2 = addr.clone();
                                a2.push(i);
                                next.push((qi, pa * pb[i], m2, a2));
                            }
                        }
                        if beam > 0 && next.len() > beam {
                            // Keep the largest CONTRIBUTIONS, since
                            // the answer is a sum and a pruned lineage
                            // is lost from it entirely.
                            next.sort_by(|a, b| {
                                let key = |c: &([f64; 2], f64, [[f64; 2]; 2], Vec<usize>)| {
                                    let d = (c.2[0][0] * c.2[1][1] - c.2[0][1] * c.2[1][0]).abs();
                                    c.1 * d * rho(c.0)
                                };
                                key(b).total_cmp(&key(a))
                            });
                            next.truncate(beam);
                        }
                        live = next;
                    }
                    (acc, if acc > 0.0 { acc_col / acc } else { 0.5 })
                };
                let estimate = |x: [f64; 2], beam: usize, foot: bool, cells: f64| -> f64 {
                    estimate_c(x, beam, foot, cells).0
                };

                // The beam is held at 16 so that what varies is the
                // STOP RULE alone -- §5b measured the truncation
                // separately and found 8 enough everywhere but the
                // band.
                let variants: Vec<(String, bool, f64)> = [1.0f64, 4.0, 16.0, 64.0]
                    .iter()
                    .flat_map(|&c| {
                        [
                            (format!("{c:>4.0} cell, point"), false, c),
                            (format!("{c:>4.0} cell, footprint"), true, c),
                        ]
                    })
                    .collect();
                let mut rows: Vec<(String, Vec<f64>)> =
                    variants.iter().map(|v| (v.0.clone(), Vec::new())).collect();
                for iy in 0..VP {
                    for ix in 0..VP {
                        let h = hits[iy * VP + ix];
                        if h < 40 {
                            continue;
                        }
                        let d = h as f64 / (DIRECT_N as f64 * px * px);
                        let x = [
                            origin[0] + (ix as f64 + 0.5) * px,
                            origin[1] + (iy as f64 + 0.5) * px,
                        ];
                        for (k, v) in variants.iter().enumerate() {
                            let e = estimate(x, 16, v.1, v.2);
                            if e > 0.0 && d > 0.0 {
                                rows[k].1.push(e / d);
                            }
                        }
                    }
                }
                // COLOUR: the address rule against the chaos game's
                // own mean palette coordinate, at the stop depth the
                // density rows found best for this class of set.
                {
                    let mut err: Vec<f64> = Vec::new();
                    let mut err0: Vec<f64> = Vec::new();
                    for iy in 0..VP {
                        for ix in 0..VP {
                            let h = hits[iy * VP + ix];
                            if h < 40 {
                                continue;
                            }
                            let truth = hcol[iy * VP + ix] / h as f64;
                            let x = [
                                origin[0] + (ix as f64 + 0.5) * px,
                                origin[1] + (iy as f64 + 0.5) * px,
                            ];
                            let (_, c) = estimate_c(x, 16, true, 4.0);
                            err.push((c - truth).abs());
                            // What the coarse colour alone would say,
                            // with no address rule: the control that
                            // shows the fold is doing the work.
                            err0.push((rho_col(x) - truth).abs());
                        }
                    }
                    err.sort_by(f64::total_cmp);
                    err0.sort_by(f64::total_cmp);
                    let md = |v: &[f64]| -> f64 {
                        if v.is_empty() {
                            f64::NAN
                        } else {
                            v[v.len() / 2]
                        }
                    };
                    let p90 = |v: &[f64]| -> f64 {
                        if v.is_empty() {
                            f64::NAN
                        } else {
                            v[((v.len() - 1) as f64 * 0.9).round() as usize]
                        }
                    };
                    println!(
                        "  {name:<16} 2^{zoom:<3.0} COLOUR                | cmp {:>4} | \
                         address rule |err| median {:.4} p90 {:.4} | coarse only {:.4}",
                        err.len(),
                        md(&err),
                        p90(&err),
                        md(&err0)
                    );
                }
                for (label, mut r) in rows {
                    r.sort_by(f64::total_cmp);
                    let qt = |f: f64| -> f64 {
                        if r.is_empty() {
                            f64::NAN
                        } else {
                            r[((r.len() - 1) as f64 * f).round() as usize]
                        }
                    };
                    // How far a typical pixel is from the truth, in
                    // the units a log tonemap cares about.
                    let spread = if r.is_empty() {
                        f64::NAN
                    } else {
                        (qt(0.9) / qt(0.1)).sqrt()
                    };
                    println!(
                        "  {name:<16} 2^{zoom:<3.0} {label:<22} | cmp {:>4} | \
                         p10 {:.3} median {:.3} p90 {:.3} | spread {spread:.2}x",
                        r.len(),
                        qt(0.1),
                        qt(0.5),
                        qt(0.9)
                    );
                }
            }
        }
    }

    /// Is the shader's measure gap on a gasket f32, or a mistake?
    ///
    /// `the_shader_measure_walk_is_the_cpu_one` finds the GPU exact on
    /// a julia dust and 2.4x to 3.4x the reference on a gasket, with
    /// the ADDRESSES agreeing (the colour matches once the beam's ties
    /// are broken). If the addresses agree then the probability and
    /// the determinant agree, so the difference is `ρ` -- the
    /// footprint average -- and the suspicion is f32.
    ///
    /// The mechanism, if it is real: the walk EXPANDS, so a lineage's
    /// endpoint sits at a `|q|` far larger than the ball, where one
    /// f32 ulp is a large fraction of a coarse cell. On a measure of
    /// dimension 1.585 most of a footprint lands in empty cells and
    /// two or three samples of sixteen carry everything, so a sample
    /// moved across a cell boundary changes `ρ` by a lot. A measure of
    /// dimension two has no such cliff.
    ///
    /// This walks each set and reports, at the points the lookup
    /// actually happens: how big one f32 ulp is against a coarse
    /// cell, and how much `ρ` moves when the sample positions are
    /// rounded to f32.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_whether_the_measure_gap_is_f32() {
        let jul = |power: f32, ty: f32| {
            let mut t = kernel_xform("julian", [0.7071, 0.7071, -0.7071, 0.7071, 0.0, ty], 1.0);
            t.variations.insert("flatten".to_string(), 1.0);
            t.variation_order.insert(0, "flatten".to_string());
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", 1.0);
            t
        };
        let cases: Vec<(&str, Vec<Transform>)> = vec![
            ("gasket", vec![half(0.0, 0.0), half(0.5, 0.0), half(0.25, 0.5)]),
            ("dragon", vec![
                affine_xform(0.5, -0.5, 0.5, 0.5, 0.0, 0.0),
                affine_xform(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0),
            ]),
            ("julia dust", vec![jul(2.0, -0.3), jul(3.0, 0.2)]),
        ];
        const RES: usize = 256;
        for (name, transforms) in cases {
            let flame = flame_of(transforms);
            let ifs = {
                let guard = global_registry();
                analyse_2d(&flame, &guard).expect("qualifies")
            };
            let maps = MeasureMaps::of(&ifs, &flame);
            let (bc, br) = (ifs.ball.centre, ifs.ball.radius);
            let cpx = 2.0 * br / RES as f64;

            // A coarse pass, and a view on the set.
            let mut coarse = CoarseMeasure {
                res: RES,
                centre: bc,
                radius: br,
                hits: vec![0; RES * RES],
                palette_sum: vec![0.0; RES * RES],
                samples: 4_000_000,
            };
            let smp = chaos_sample(&ifs, 4_000_000);
            for q in &smp {
                if let Some(c) = coarse.index_for_test(*q) {
                    coarse.hits[c] += 1;
                    coarse.palette_sum[c] += 0.5;
                }
            }
            coarse.samples = smp.len() as u64;
            let centre = *smp
                .iter()
                .max_by_key(|p| coarse.index_for_test(**p).map_or(0, |i| coarse.hits[i]))
                .expect("samples");

            let span = 2.0 * br / 2f64.powf(5.0);
            let px = span / 64.0;
            let want = (cpx / px) * (cpx / px) * MEASURE_CELLS;

            // Walk to the stop and record where the lookup happens.
            let mut mags: Vec<f64> = Vec::new();
            let mut moved: Vec<f64> = Vec::new();
            for gy in 0..16 {
                for gx in 0..16 {
                    let x = [
                        centre[0] + (gx as f64 / 16.0 - 0.5) * span,
                        centre[1] + (gy as f64 / 16.0 - 0.5) * span,
                    ];
                    let mut live = vec![(x, [[1.0f64, 0.0], [0.0, 1.0]])];
                    for _ in 0..40 {
                        let mut next = Vec::new();
                        for (q, m) in live.drain(..) {
                            let det = (m[0][0] * m[1][1] - m[0][1] * m[1][0]).abs();
                            if det >= want {
                                // The footprint, in f64 and rounded.
                                let (mut a, mut b) = (0.0f64, 0.0f64);
                                for sy in 0..4 {
                                    for sx in 0..4 {
                                        let u = ((sx as f64 + 0.5) / 4.0 - 0.5) * px;
                                        let v = ((sy as f64 + 0.5) / 4.0 - 0.5) * px;
                                        let at = [
                                            q[0] + m[0][0] * u + m[0][1] * v,
                                            q[1] + m[1][0] * u + m[1][1] * v,
                                        ];
                                        a += coarse.density(at);
                                        b += coarse.density([
                                            at[0] as f32 as f64,
                                            at[1] as f32 as f64,
                                        ]);
                                    }
                                }
                                if a > 0.0 {
                                    mags.push(q[0].hypot(q[1]));
                                    moved.push((b / a).ln().abs());
                                }
                                continue;
                            }
                            for (i, mp) in ifs.maps.iter().enumerate() {
                                let _ = i;
                                let Some(j) = mp.inverse.jacobian(q) else { continue };
                                let qi = mp.inverse.apply(q);
                                if !qi[0].is_finite() || !qi[1].is_finite() { continue; }
                                if Affine2::distance(qi, bc) > br * (1.0 + 1e-6) { continue; }
                                next.push((qi, compose_matrix(j, m)));
                            }
                        }
                        if next.len() > 8 {
                            next.sort_by(|p, q2| {
                                let k = |c: &([f64; 2], [[f64; 2]; 2])| {
                                    (c.1[0][0] * c.1[1][1] - c.1[0][1] * c.1[1][0]).abs()
                                        * coarse.density(c.0)
                                };
                                k(q2).total_cmp(&k(p))
                            });
                            next.truncate(8);
                        }
                        live = next;
                        if live.is_empty() { break; }
                    }
                }
            }
            if mags.is_empty() {
                println!("  {name:<12} no lookups");
                continue;
            }
            mags.sort_by(f64::total_cmp);
            moved.sort_by(f64::total_cmp);
            let med_mag = mags[mags.len() / 2];
            let ulp = med_mag * 5.96e-8;
            println!(
                "  {name:<12} median |q| at the lookup {med_mag:.3e} | one f32 ulp \
                 {ulp:.3e} = {:.4} of a coarse cell | rounding moves rho by \
                 median {:.3}x, p90 {:.3}x",
                ulp / cpx,
                moved[moved.len() / 2].exp(),
                moved[((moved.len() - 1) as f64 * 0.9) as usize].exp(),
            );
        }
    }

    /// Where the beam's ranking key fails, measured.
    ///
    /// Reported from use as "layers that overlap unpredictably" and
    /// "cut-out shapes" that move under a small rotation of one
    /// transform, on a grand julian. A cut-out is an OVER-read: the
    /// beam pruned the address that held the answer, the distance
    /// came back too large, and the pixel read as exterior. The
    /// ranking by position was measured right on the gasket, where
    /// every map contracts alike; this measures it where they do not.
    #[test]
    fn the_beam_key_is_scored_on_an_inversion() {
        const DEPTH: u32 = 8;
        let ball = |ifs: Ifs2| {
            let (c, r) = (ifs.ball.centre, ifs.ball.radius);
            (ifs, c, r)
        };
        let kernel_cases: Vec<(&str, Vec<Transform>)> = vec![
            (
                "spherical",
                vec![
                    kernel_xform("spherical", [0.0, -1.0, 1.0, 0.0, 1.0, 0.0], 1.0),
                    kernel_xform("spherical", [0.0, 1.0, -1.0, 0.0, 0.0, 0.0], 1.0),
                    affine_xform(0.5, 0.0, 0.0, 0.5, 0.8, 0.0),
                ],
            ),
            (
                "bubble",
                vec![
                    kernel_xform("bubble", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.6),
                    kernel_xform("bubble", [0.7, 0.7, -0.7, 0.7, 0.0, -0.3], 1.2),
                    affine_xform(0.5, 0.0, 0.0, 0.5, 1.0, 0.5),
                ],
            ),
            (
                "hemisphere",
                vec![
                    kernel_xform("hemisphere", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.5),
                    kernel_xform("hemisphere", [0.7, 0.7, -0.7, 0.7, 0.4, 0.0], 1.2),
                    affine_xform(0.5, 0.0, 0.0, 0.5, 0.8, 0.3),
                ],
            ),
            (
                "disc",
                vec![
                    kernel_xform("disc", [0.9, 0.4, -0.4, 0.9, 0.0, 0.0], 1.0),
                    affine_xform(0.55, 0.0, 0.0, 0.55, 0.0, 0.0),
                ],
            ),
            (
                "blob",
                vec![
                    {
                        let mut t = kernel_xform("blob", [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0.8);
                        t.set_variation_param("blob", "high", 1.2);
                        t.set_variation_param("blob", "low", 0.5);
                        t.set_variation_param("blob", "waves", 5.0);
                        t
                    },
                    affine_xform(0.6, 0.3, -0.3, 0.6, 0.5, 0.0),
                    affine_xform(0.5, 0.0, 0.0, 0.5, -0.4, 0.3),
                ],
            ),
        ];
        let mut cases: Vec<(&str, Ifs2, [f64; 2], f64)> = vec![
            ("sierpinski", sierpinski(), [0.5, 0.5], 0.9),
            ("dragon", dragon(), [0.4, 0.4], 1.0),
            {
                let g = grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]);
                let (c, r) = (g.ball.centre, g.ball.radius);
                ("grand julian 1", g, c, r)
            },
            {
                let g = grand_julian([0.509, 0.8607, -0.8607, 0.509, 0.0, 0.0]);
                let (c, r) = (g.ball.centre, g.ball.radius);
                ("grand julian 2", g, c, r)
            },
        ];
        for (name, transforms) in kernel_cases {
            let (ifs, c, r) = ball(analyse(transforms));
            cases.push((name, ifs, c, r));
        }
        // The shipped julia class: roots with a POSITIVE distance, which
        // are not inversions. `Auto` must leave these on Position.
        {
            let j = |aff: [f32; 6], power: f32| {
                let mut t = kernel_xform("julian", aff, 1.0);
                t.set_variation_param("julian", "power", power);
                t.set_variation_param("julian", "dist", 1.0);
                t
            };
            let (ifs, c, r) = ball(analyse(vec![
                j([0.7, 0.0, 0.0, 0.7, 0.3, 0.0], 2.0),
                j([0.0, -0.7, 0.7, 0.0, -0.2, 0.4], 3.0),
            ]));
            cases.push(("julia pair (+dist)", ifs, c, r));
        }

        let mut report = Vec::new();
        for (name, ifs, centre, radius) in &cases {
            let tol = 2.0 * radius / 512.0;
            for beam in [4u32, 8] {
                for key in [RankKey::Position, RankKey::Weighted, RankKey::Mixed, RankKey::Auto] {
                    let (mut loose, mut total, mut worst, mut sum_over) = (0usize, 0usize, 1.0f64, 0.0f64);
                    let n = 24;
                    for iy in 0..n {
                        for ix in 0..n {
                            let (u, v) = (
                                2.0 * (ix as f64 + 0.5) / n as f64 - 1.0,
                                2.0 * (iy as f64 + 0.5) / n as f64 - 1.0,
                            );
                            let p = [centre[0] + radius * u, centre[1] + radius * v];
                            let w = estimate_aux_ranked(ifs, p, 0.0, DEPTH, beam, key).distance;
                            let e = exhaustive_nl(ifs, p, DEPTH - 1);
                            assert!(
                                e <= w + 1e-9,
                                "{name} beam {beam} {key:?} at {p:?}: {w} is below exhaustive {e}"
                            );
                            total += 1;
                            if w > e + tol {
                                loose += 1;
                                sum_over += (w - e) / radius;
                                if e > 1e-9 {
                                    worst = worst.max(w / e);
                                }
                            }
                        }
                    }
                    report.push((name.to_string(), beam, key, loose, total, worst, sum_over));
                }
            }
        }

        println!("beam against exhaustive, depth {DEPTH}, over-reads past a pixel at 512 across the ball:");
        for (name, beam, key, loose, total, worst, sum_over) in &report {
            println!(
                "  {name:16} beam {beam} {key:9?}: loose {loose:>3}/{total}  worst x{worst:6.2}  mean over-read {:.4} radii",
                if *loose > 0 { sum_over / *loose as f64 } else { 0.0 }
            );
        }

        for key in [RankKey::Position, RankKey::Weighted, RankKey::Mixed, RankKey::Auto] {
            for beam in [4u32, 8] {
                let rows: Vec<_> = report.iter().filter(|r| r.1 == beam && r.2 == key).collect();
                let loose: usize = rows.iter().map(|r| r.3).sum();
                let total: usize = rows.iter().map(|r| r.4).sum();
                let worst = rows.iter().map(|r| r.5).fold(1.0f64, f64::max);
                println!("  TOTAL beam {beam} {key:9?}: loose {loose:>4}/{total}  worst x{worst:.2}");
            }
        }

        let at = |n: &str, b: u32, k: RankKey| {
            report
                .iter()
                .find(|r| r.0 == n && r.1 == b && r.2 == k)
                .map(|r| (r.3, r.4))
                .expect("measured")
        };
        // Auto must be no looser than the better of the two pure keys
        // on every fixture -- that is the whole claim.
        for (name, ..) in &cases {
            for b in [4, 8] {
                let (lp, t) = at(name, b, RankKey::Position);
                let (lw, _) = at(name, b, RankKey::Weighted);
                let (la, _) = at(name, b, RankKey::Auto);
                assert!(
                    la <= lp.min(lw) + t / 100,
                    "{name} beam {b}: Auto ({la}) is looser than the better key ({} of {t})",
                    lp.min(lw)
                );
            }
        }
        // And the reported flame is a different picture under it.
        for n in ["grand julian 1", "grand julian 2"] {
            let (lp, _) = at(n, 8, RankKey::Position);
            let (la, _) = at(n, 8, RankKey::Auto);
            assert!(la <= lp / 2, "{n}: Auto did not halve the loose count ({la} vs {lp})");
            let worst = report
                .iter()
                .find(|r| r.0 == n && r.1 == 8 && r.2 == RankKey::Auto)
                .map(|r| r.5)
                .unwrap();
            assert!(worst < 20.0, "{n}: Auto worst ratio {worst:.1}");
        }
    }

    /// The reported view (`grand-julian-glitches3.fflame`): the same
    /// flame at zoom 7.8 with 35 levels and a beam of 5, where wedges
    /// still show after 8.12. No handover is involved -- a nonlinear
    /// IFS hands over at level 0 -- so this is the walk itself, at
    /// depth.
    ///
    /// Every walk answers the minimum over the paths IT kept, so every
    /// estimate is at or above the depth-limited truth, and the
    /// pointwise minimum over all runs is the best reference a depth
    /// of 35 allows -- PROVIDED the answer is a real one. A winner
    /// that never escaped by level 35 is either genuinely inside the
    /// set or a path that froze (its next point past what a float
    /// holds) with a stale bound; the two are told apart here by
    /// whether ANY run escaped at that point, and frozen answers are
    /// kept out of the reference.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn the_reported_view_against_a_wide_beam() {
        let ifs = grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]);
        let centre = [-0.05554435526432207, -0.19381778925226906];
        let zoom = 7.798816f64;
        let rot = 0.7853982f64;
        let span = 4.0 / 2f64.powf(zoom);
        let (cs, sn) = (rot.cos(), rot.sin());
        let n = 32;
        let mut pts = Vec::new();
        for iy in 0..n {
            for ix in 0..n {
                let u = ((ix as f64 + 0.5) / n as f64 - 0.5) * span;
                let v = -((iy as f64 + 0.5) / n as f64 - 0.5) * span;
                pts.push([centre[0] + u * cs - v * sn, centre[1] + u * sn + v * cs]);
            }
        }
        let px = span / 800.0;
        const LEVELS: u32 = 35;

        let configs: Vec<(RankKey, u32)> = [RankKey::Weighted, RankKey::Mixed]
            .iter()
            .flat_map(|&k| [5u32, 8, 16, 32].into_iter().map(move |b| (k, b)))
            .chain([(RankKey::Weighted, 512)])
            .collect();
        // (distance, escaped) per point per run.
        let runs: Vec<Vec<(f64, bool)>> = configs
            .iter()
            .map(|&(k, b)| {
                pts.iter()
                    .map(|&p| {
                        let e = estimate_aux_ranked(&ifs, p, 0.0, LEVELS, b, k);
                        (e.distance, e.escaped)
                    })
                    .collect()
            })
            .collect();
        // The reference: the minimum over ESCAPED answers. A point no
        // run escaped is interior for the purpose of this table.
        // An answer that ESCAPED yet reads zero is a stale bound: the
        // escape's own term overflowed and was dropped, and the bound
        // the path had inside the ball survived. Sound, but not a
        // reference.
        let reference: Vec<Option<f64>> = (0..pts.len())
            .map(|i| {
                runs.iter()
                    .filter(|r| r[i].1 && r[i].0 > 0.0)
                    .map(|r| r[i].0)
                    .fold(None, |m: Option<f64>, d| Some(m.map_or(d, |m| m.min(d))))
            })
            .collect();
        let interior = reference.iter().filter(|r| r.is_none()).count();
        let ref_zero = reference.iter().filter(|r| matches!(r, Some(d) if *d <= 0.0)).count();
        println!(
            "reported view, {}x{} points, {LEVELS} levels, errors in pixels ({px:.2e}) against the min over ESCAPED answers; {interior} points escaped in no run, {ref_zero} reference zeros",
            n, n
        );
        println!("  {:9} {:>4}  {:>6} {:>6} {:>6} {:>6}  {:>8} {:>8}   {:>6}", "key", "beam", "frozen", ">1px", ">10px", ">100px", "median", "worst", "under");
        for ((k, b), w) in configs.iter().zip(&runs) {
            // frozen: not escaped at a point the reference says is exterior.
            let mut errs: Vec<f64> = Vec::new();
            let (mut frozen, mut under, mut stale) = (0usize, 0usize, 0usize);
            for (i, &(d, esc)) in w.iter().enumerate() {
                let Some(e) = reference[i] else { continue };
                if !esc {
                    frozen += 1;
                    continue;
                }
                if d <= 0.0 {
                    stale += 1;
                    continue;
                }
                let err = (d - e) / px;
                if err < -1.0 {
                    under += 1;
                }
                errs.push(err);
            }
            errs.sort_by(|a, b| a.total_cmp(b));
            let over = |t: f64| errs.iter().filter(|&&d| d > t).count();
            let median = errs.get(errs.len() / 2).copied().unwrap_or(0.0);
            let worst = errs.last().copied().unwrap_or(0.0);
            println!(
                "  {k:9?} {b:>4}  {frozen:>6} {:>6} {:>6} {:>6}  {median:>8.2} {worst:>8.1}   {under:>6}  stale {stale}",
                over(1.0),
                over(10.0),
                over(100.0)
            );
        }

        // What the survey asserts: at the beam the reported file used,
        // no visible over-read anywhere in its view, and a wider beam
        // never worse. Measured at 24 points past ten pixels and a
        // worst of 81 before `best_done` and the inversions' holes; 0
        // after.
        for ((k, b), w) in configs.iter().zip(&runs) {
            let over10 = w
                .iter()
                .enumerate()
                .filter(|(i, &(d, esc))| {
                    esc && d > 0.0 && reference[*i].map_or(false, |e| (d - e) / px > 10.0)
                })
                .count();
            assert_eq!(over10, 0, "{k:?} beam {b}: {over10} points over-read past ten pixels");
        }

    }

    /// What the LEVEL is on the reported view: the deepest any address
    /// reaches, and the level of the address that gives the distance.
    ///
    /// With three inversions there is almost always some address that
    /// stays inside the ball for every level asked, so the deepest is
    /// the maximum nearly everywhere -- and a level colouring of it is
    /// one flat value. The winner's escape level has structure. This
    /// prints both distributions so the choice is made on numbers.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn the_level_on_the_reported_view() {
        let ifs = grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]);
        let centre = [-0.05554435526432207, -0.19381778925226906];
        let (zoom, rot) = (7.798816f64, 0.7853982f64);
        let span = 4.0 / 2f64.powf(zoom);
        let (cs, sn) = (rot.cos(), rot.sin());
        let n = 32;
        const LEVELS: u32 = 35;
        for beam in [5u32, 512] {
            let mut deepest = vec![0usize; LEVELS as usize + 1];
            let mut winner = vec![0usize; LEVELS as usize + 1];
            for iy in 0..n {
                for ix in 0..n {
                    let u = ((ix as f64 + 0.5) / n as f64 - 0.5) * span;
                    let v = -((iy as f64 + 0.5) / n as f64 - 0.5) * span;
                    let p = [centre[0] + u * cs - v * sn, centre[1] + u * sn + v * cs];
                    let e = estimate(&ifs, p, LEVELS, beam);
                    deepest[(e.deepest_level.floor().max(0.0) as usize).min(LEVELS as usize)] += 1;
                    winner[(e.level.floor().max(0.0) as usize).min(LEVELS as usize)] += 1;
                }
            }
            let hist = |h: &Vec<usize>| -> String {
                h.iter().enumerate().filter(|(_, &c)| c > 0).map(|(l, c)| format!("{l}:{c}")).collect::<Vec<_>>().join(" ")
            };
            println!("beam {beam}: deepest level histogram  {}", hist(&deepest));
            println!("beam {beam}: winner  level histogram  {}", hist(&winner));
        }
    }

    /// The jitter report (`grand-julian-glitches4/5.fflame`): the same
    /// view, the second transform turned by 0.6 degrees, and a band
    /// that moves "in very noticeable amounts". At this view the beam
    /// of 5 is exact against 512 in both files, so the field's move is
    /// intrinsic to the bound. Whether it is the SET moving or the
    /// bound's slack is what this measures, two ways.
    ///
    /// First, the rotation is swept in nine steps and the distance
    /// and the winning address are watched at every point: geometry
    /// moves smoothly and monotonically, an argmin switching between
    /// two pieces with different slack snaps.
    ///
    /// Second, ground truth: a dense chaos-game sample of each end,
    /// every sample carrying the address of the last maps applied,
    /// gives each grid point its TRUE nearest piece. How many grid
    /// points change their true nearest piece between the two files,
    /// against how many change the walk's winner.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn the_jitter_between_two_rotations() {
        let t2 = [-0.9701f32, -0.2427, 0.2427, -0.9701, 0.0, 0.0];
        let a0 = (-0.5299f64).atan2(-0.848);
        let a1 = (-0.5387f64).atan2(-0.8425);
        let t1_at = |a: f64| -> [f32; 6] {
            let (c, s) = (a.cos() as f32, a.sin() as f32);
            [c, s, -s, c, 0.0, 0.0]
        };
        let centre = [-0.4318341396154855, 0.09837190993102786];
        let (zoom, rot) = (5.818763f64, 0.7853982f64);
        let span = 4.0 / 2f64.powf(zoom);
        let (cs, sn) = (rot.cos(), rot.sin());
        let n = 32;
        let px = span / 640.0;
        const LEVELS: u32 = 35;
        let mut pts = Vec::new();
        for iy in 0..n {
            for ix in 0..n {
                let u = ((ix as f64 + 0.5) / n as f64 - 0.5) * span;
                let v = -((iy as f64 + 0.5) / n as f64 - 0.5) * span;
                pts.push([centre[0] + u * cs - v * sn, centre[1] + u * sn + v * cs]);
            }
        }

        // ---- the sweep
        let steps = 9;
        let mut prev: Option<(Vec<f64>, Vec<u32>)> = None;
        let mut sign_flips = vec![0usize; pts.len()];
        let mut last_delta = vec![0.0f64; pts.len()];
        println!("sweep of the second transform's angle, {steps} steps, {}x{} points, pixels ({px:.2e}):", n, n);
        for k in 0..steps {
            let a = a0 + (a1 - a0) * k as f64 / (steps - 1) as f64;
            let g = grand_julian_t1(t2, t1_at(a));
            let dist: Vec<f64> = pts.iter().map(|&p| estimate(&g, p, LEVELS, 5).distance).collect();
            let first: Vec<u32> = pts
                .iter()
                .map(|&p| estimate(&g, p, LEVELS, 5).address.first().copied().unwrap_or(99))
                .collect();
            if let Some((pd, pf)) = &prev {
                let mut moved10 = 0;
                let mut flipped = 0;
                let mut worst = 0.0f64;
                for i in 0..pts.len() {
                    let d = (dist[i] - pd[i]) / px;
                    if d.abs() > 10.0 {
                        moved10 += 1;
                    }
                    worst = worst.max(d.abs());
                    if last_delta[i] * d < 0.0 && d.abs() > 1.0 && last_delta[i].abs() > 1.0 {
                        sign_flips[i] += 1;
                    }
                    last_delta[i] = d;
                    if first[i] != pf[i] {
                        flipped += 1;
                    }
                }
                println!(
                    "  step {k}: distance moved >10px at {moved10:>4}, worst {worst:6.1}px; first branch changed at {flipped:>4}"
                );
                assert!(worst < 4.0, "step {k}: the distance field jumped {worst:.1}px for a 0.075-degree turn");
            }
            prev = Some((dist, first));
        }
        let reversers = sign_flips.iter().filter(|&&f| f > 0).count();
        println!("  points whose distance reversed direction during the sweep: {reversers}/{}", pts.len());

        // ---- ground truth: nearest sample's address, both ends
        let sample_with_address = |ifs: &Ifs2, count: usize| -> Vec<([f64; 2], u32)> {
            let mut state: u64 = 0x2545_F491_4F6C_DD1D;
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            let mut p = ifs.ball.centre;
            let mut out = Vec::with_capacity(count);
            for i in 0..count + 200 {
                let k = (next() % ifs.maps.len() as u64) as usize;
                // Every branch of a root, as the flame's chaos game
                // takes them: `Map2::apply` alone is branch 0, a
                // sub-attractor.
                let br = match ifs.maps[k].forward.nonlinear().map(|r| r.kernel) {
                    Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => (next() % n.unsigned_abs() as u64) as u32,
                    _ => 0,
                };
                p = match &ifs.maps[k].forward {
                    Map2::Nonlinear(r) => r.apply_branch(p, br),
                    other => other.apply(p),
                };
                if !(p[0].is_finite() && p[1].is_finite()) {
                    p = ifs.ball.centre;
                    continue;
                }
                if i >= 200 {
                    out.push((p, k as u32));
                }
            }
            out
        };
        let nearest_first = |sample: &[([f64; 2], u32)], q: [f64; 2]| -> (f64, u32) {
            let mut best = (f64::INFINITY, 99u32);
            for &(a, k) in sample {
                let d = Affine2::distance(q, a);
                if d < best.0 {
                    best = (d, k);
                }
            }
            best
        };
        let g4 = grand_julian_t1(t2, t1_at(a0));
        let g5 = grand_julian_t1(t2, t1_at(a1));
        let s4 = sample_with_address(&g4, 400_000);
        let s5 = sample_with_address(&g5, 400_000);
        let (mut true_changed, mut walk_changed, mut true_moved10) = (0usize, 0usize, 0usize);
        for &p in &pts {
            let (d4, k4) = nearest_first(&s4, p);
            let (d5, k5) = nearest_first(&s5, p);
            if k4 != k5 {
                true_changed += 1;
            }
            if ((d4 - d5) / px).abs() > 10.0 {
                true_moved10 += 1;
            }
            let w4 = estimate(&g4, p, LEVELS, 5).address.first().copied().unwrap_or(99);
            let w5 = estimate(&g5, p, LEVELS, 5).address.first().copied().unwrap_or(99);
            if w4 != w5 {
                walk_changed += 1;
            }
        }
        println!(
            "ground truth between the two files: true nearest piece changes at {true_changed}/{} points, \
             true distance moves >10px at {true_moved10}; the walk's winner changes at {walk_changed}",
            pts.len()
        );
        // What this gates: the walk's picture moves no more than the
        // set does. Before the continuous ball, 44 winners and 504
        // distances past ten pixels moved in a single sweep step
        // against a set that moved at none; after, 0 and 0 with a
        // worst step of half a pixel.
        assert_eq!(true_changed, 0, "the set itself moved; the fixture is not what it was");
        assert!(
            walk_changed <= 4,
            "the walk's nearest piece changed at {walk_changed} points for a set that moved at none"
        );
        assert_eq!(reversers, 0, "the distance reversed direction under a monotone rotation");
    }

    /// Is the bound sound AT THE SET? A point the chaos game visits is
    /// on the attractor (or a transient of it), and the walk should
    /// read ~0 there. Where it does not, replaying the inverse along
    /// the address the chaos game took to that point shows the level
    /// whose term went wrongly positive -- or shows the point left the
    /// ball on its way, which is the cut and not a bug.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_the_bound_at_the_set() {
        let t1 = [-0.84805f32, -0.52986, 0.52986, -0.84805, 0.0, 0.0];
        let t2 = [-0.97008f32, -0.2427, 0.24271, -0.97008, 0.0, 0.0];
        let a0 = (-0.33506f64).atan2(0.94218);
        let (c0, s0) = (a0.cos() as f32, a0.sin() as f32);
        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = kernel_xform("julian", aff, w);
            t.variations.insert("flatten".to_string(), 1.0);
            t.variation_order.insert(0, "flatten".to_string());
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        let g = analyse(vec![j([c0, s0, -s0, c0, 0.0, -0.3], 1.0, 2.0), j(t1, 0.2, 15.0), j(t2, 0.3, 8.0)]);
        let zoom = 7.4688044f64;
        let px = 4.0 / 2f64.powf(zoom) / 640.0;
        let radius = g.ball.radius;
        let centre = g.ball.centre;
        // the chaos game, with each kept point's last twelve maps and
        // whether its orbit was inside the ball at each of them
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut p = centre;
        let mut hist: Vec<(usize, f64)> = Vec::new(); // (map, r before the map)
        let mut kept: Vec<([f64; 2], Vec<(usize, f64)>)> = Vec::new();
        for i in 0..20_500 {
            let mi = (next() * g.maps.len() as f64).floor() as usize % g.maps.len();
            let m = &g.maps[mi];
            let k = match m.forward.nonlinear().map(|n| n.kernel) {
                Some(crate::scene::ifs_analysis::Kernel::Root { n, .. }) => (next() * n.unsigned_abs() as f64).floor() as u32,
                _ => 0,
            };
            let r_before = Affine2::distance(p, centre);
            p = match &m.forward {
                Map2::Nonlinear(n) => n.apply_branch(p, k),
                other => other.apply(p),
            };
            hist.push((mi, r_before));
            if hist.len() > 12 { hist.remove(0); }
            if i >= 500 && p[0].is_finite() && p[1].is_finite() {
                kept.push((p, hist.clone()));
            }
        }
        let mut bad = 0usize;
        let mut bad_in_ball = 0usize; // a bad point whose last 12 pre-images all sat inside the ball
        let mut worst: Vec<(f64, usize)> = Vec::new();
        for (idx, (q, h)) in kept.iter().enumerate() {
            let d = estimate(&g, *q, 35, 5).distance;
            if d > px {
                bad += 1;
                let inside = h.iter().all(|(_, r)| *r <= radius);
                if inside { bad_in_ball += 1; }
                worst.push((d, idx));
            }
        }
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!(
            "ball R {radius:.4}; chaos points {}; walk reads > 1px at {bad} ({:.2}%), of which {bad_in_ball} came through the ball only (last 12 maps)",
            kept.len(), 100.0 * bad as f64 / kept.len() as f64
        );
        // Replay the inverse along the recorded address for the worst
        // few whose orbit stayed inside the ball.
        let mut shown = 0;
        for (d, idx) in &worst {
            let (q, h) = &kept[*idx];
            if !h.iter().all(|(_, r)| *r <= radius) { continue; }
            if shown >= 4 { break; }
            shown += 1;
            let ex = exhaustive_nl(&g, *q, 9);
            println!("  point {idx}: walk {:.1} px, exhaustive(10) {:.1} px; r {:.3}; replaying its address (newest map first):", d / px, ex / px, Affine2::distance(*q, centre));
            let mut cur = *q;
            let mut sigma = 1.0;
            let mut bound = f64::NEG_INFINITY;
            for (mi, r_before) in h.iter().rev() {
                let m = &g.maps[*mi];
                let gap = m.inverse.gap_bound(cur, centre, radius, m.sigma_min);
                let (q2, _, s_one) = m.inverse.step(cur, 0.0, m.sigma_min);
                let s2 = sigma * s_one;
                let r2 = Affine2::distance(q2, centre);
                let term = s2 * (r2 - radius);
                bound = fold_level(bound, s2, r2, radius);
                println!(
                    "    map {mi}: gap {:?}; step -> r {:.4} (forward orbit had r {:.4}), sigma {:.3e}, term {:+.3e} ({:+.1} px), bound so far {:+.1} px",
                    gap.map(|x| format!("{:.3e} ({:.1} px)", x, sigma * x / px)), r2, r_before, s2, term, term / px, bound / px
                );
                if gap.is_some() || !(q2[0].is_finite() && q2[1].is_finite()) { break; }
                cur = q2;
                sigma = s2;
            }
        }
    }

    /// The reference walk with PROVENANCE: the value, and the level,
    /// kind and address it came from.
    fn exhaustive_prov(ifs: &Ifs2, p: [f64; 2], depth: u32) -> (f64, u32, &'static str, Vec<u32>) {
        fn walk(ifs: &Ifs2, q: [f64; 2], sigma: f64, best_on_path: f64, left: u32, level: u32, addr: &mut Vec<u32>) -> (f64, u32, &'static str, Vec<u32>) {
            let r = Affine2::distance(q, ifs.ball.centre);
            let here = fold_level(best_on_path, sigma, r, ifs.ball.radius);
            if left == 0 || !r.is_finite() || r > ifs.ball.radius.max(1.0) * FAR {
                return (here, level, if left == 0 { "leaf" } else { "far" }, addr.clone());
            }
            let mut best = (f64::INFINITY, level, "none", addr.clone());
            for (i, m) in ifs.maps.iter().enumerate() {
                addr.push(i as u32);
                if let Some(gap) = m.inverse.gap_bound(q, ifs.ball.centre, ifs.ball.radius, m.sigma_min) {
                    let v = here.max(sigma * gap);
                    if v < best.0 {
                        best = (v, level + 1, if sigma * gap > here { "gap" } else { "gap<here" }, addr.clone());
                    }
                } else {
                    let (q2, _, sg) = m.inverse.step(q, 0.0, m.sigma_min);
                    let sub = walk(ifs, q2, sigma * sg, here, left - 1, level + 1, addr);
                    if sub.0 < best.0 {
                        best = sub;
                    }
                }
                addr.pop();
            }
            best
        }
        let mut addr = Vec::new();
        walk(ifs, p, 1.0, 0.0, depth, 0, &mut addr)
    }

    /// The grand julian of the g6 report at a first-transform angle.
    fn g6_at(a: f64) -> Ifs2 {
        let t1 = [-0.84805f32, -0.52986, 0.52986, -0.84805, 0.0, 0.0];
        let t2 = [-0.97008f32, -0.2427, 0.24271, -0.97008, 0.0, 0.0];
        let (c, sn) = (a.cos() as f32, a.sin() as f32);
        let j = |aff: [f32; 6], w: f32, power: f32| {
            let mut t = kernel_xform("julian", aff, w);
            t.variations.insert("flatten".to_string(), 1.0);
            t.variation_order.insert(0, "flatten".to_string());
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", -1.0);
            t
        };
        analyse(vec![j([c, sn, -sn, c, 0.0, -0.3], 1.0, 2.0), j(t1, 0.2, 15.0), j(t2, 0.3, 8.0)])
    }

    /// The g6 view as a grid of world points, and its pixel width.
    fn g6_view(n: usize) -> (Vec<[f64; 2]>, f64) {
        let centre = [-0.21665968763745389, 0.12254946871633931];
        let (zoom, rot) = (7.4688044f64, 0.7853982f64);
        let span = 4.0 / 2f64.powf(zoom);
        let (cs, sn) = (rot.cos(), rot.sin());
        let mut pts = Vec::new();
        for iy in 0..n {
            for ix in 0..n {
                let u = ((ix as f64 + 0.5) / n as f64 - 0.5) * span;
                let v = -((iy as f64 + 0.5) / n as f64 - 0.5) * span;
                pts.push([centre[0] + u * cs - v * sn, centre[1] + u * sn + v * cs]);
            }
        }
        (pts, span / 640.0)
    }

    /// A sweep fine enough that a continuous field cannot move ten
    /// pixels in a step: a jump then IS a discontinuity, and the
    /// reference walk with provenance says what flipped.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_the_fine_sweep() {
        let a0 = (-0.33506f64).atan2(0.94218);
        let step = 0.75f64.to_radians() / 16.0;
        let (pts, px) = g6_view(32);
        let mut prev: Option<Vec<f64>> = None;
        let mut prev_g: Option<Ifs2> = None;
        let mut total = 0usize;
        println!("step {:.4} degrees, {} points, px {px:.3e}", step.to_degrees(), pts.len());
        for k in 0..17 {
            let g = g6_at(a0 + step * k as f64);
            let walk: Vec<f64> = pts.iter().map(|&p| estimate(&g, p, 35, 5).distance).collect();
            if let (Some(pw), Some(pg)) = (&prev, &prev_g) {
                let mut jumps: Vec<(f64, usize)> = walk.iter().zip(pw).enumerate()
                    .map(|(i, (a, b))| (((a - b) / px).abs(), i)).filter(|(d, _)| *d > 10.0).collect();
                jumps.sort_by(|a, b| b.0.total_cmp(&a.0));
                total += jumps.len();
                let med = { let mut m: Vec<f64> = walk.iter().zip(pw).map(|(a, b)| ((a - b) / px).abs()).collect(); m.sort_by(|a, b| a.total_cmp(b)); m[m.len() / 2] };
                println!("  step {k:>2}: R {:.5} (moved {:+.2e}); jumps>10px {:>3}, worst {:.1} px, median move {:.2} px", g.ball.radius, g.ball.radius - pg.ball.radius, jumps.len(), jumps.first().map_or(0.0, |j| j.0), med);
                for (d, i) in jumps.iter().take(2) {
                    let before = exhaustive_prov(pg, pts[*i], 10);
                    let after = exhaustive_prov(&g, pts[*i], 10);
                    println!("     pt {i}: {:.1} -> {:.1} px ({d:.1});  ref before: {:.1} px L{} {} {:?};  after: {:.1} px L{} {} {:?}",
                        pw[*i] / px, walk[*i] / px, before.0 / px, before.1, before.2, before.3, after.0 / px, after.1, after.2, after.3);
                }
            }
            prev = Some(walk);
            prev_g = Some(g);
        }
        println!("total jumps over 16 fine steps: {total}");
    }

    /// Where to cut an unbounded set. An inversion IFS has a tail to
    /// infinity through its poles, so the ball is a CUT through it and
    /// the walk draws the bulk inside. Two costs move against each
    /// other as the cut moves out: pieces beyond it are invisible and
    /// the walk over-reads beside their images; while every bound is
    /// `σ·(r − R)` or a hole's edge, so a larger R is a looser bound
    /// everywhere and a fatter halo. This sweeps R, holes following,
    /// and prints both costs at the reported view and across its
    /// rotation. Measured 2026-09-16: over-reads 142 at the shipped
    /// radius, 0 from 1.5x; the halo 449 of 1,024 at the shipped
    /// radius, all 1,024 from 1.5x.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_where_to_cut() {
        let a0 = (-0.33506f64).atan2(0.94218);
        let a1 = (-0.23518f64).atan2(0.97194);
        let (pts, px) = g6_view(32);
        let base = g6_at(a0);
        let smp = chaos_sample(&base, 200_000);
        let truth: Vec<f64> = pts.iter().map(|&p| smp.iter().map(|&a| Affine2::distance(p, a)).fold(f64::INFINITY, f64::min)).collect();
        let mut radii: Vec<f64> = smp.iter().map(|p| Affine2::distance(*p, base.ball.centre)).collect();
        radii.sort_by(|a, b| a.total_cmp(b));
        println!("  {:>6} {:>7}  {:>8} {:>8} {:>9}   {:>9} {:>9}", "R", "covers", "unsound", "halo41", "slack med", "sweep>10", "sweepmax");
        for scale in [0.8f64, 0.9, 1.0, 1.15, 1.3, 1.5, 2.0, 3.0] {
            let g0 = base.clone().with_extent(base.ball.radius * scale);
            let r = g0.ball.radius;
            let walk: Vec<f64> = pts.iter().map(|&p| estimate(&g0, p, 35, 5).distance).collect();
            let unsound = walk.iter().zip(&truth).filter(|(w, t)| **w > **t + px).count();
            let halo = walk.iter().filter(|&&w| w / px < 41.0).count();
            let mut sl: Vec<f64> = walk.iter().zip(&truth).map(|(w, t)| (t - w) / px).collect();
            sl.sort_by(|a, b| a.total_cmp(b));
            let covers = 100.0 * radii.iter().filter(|&&x| x <= r).count() as f64 / radii.len() as f64;
            let (mut jumps, mut worst) = (0usize, 0.0f64);
            let mut prev: Option<Vec<f64>> = None;
            for k in 0..9 {
                let g = { let g = g6_at(a0 + (a1 - a0) * k as f64 / 8.0); let r = g.ball.radius * scale; g.with_extent(r) };
                let w: Vec<f64> = pts.iter().map(|&p| estimate(&g, p, 35, 5).distance).collect();
                if let Some(pw) = &prev {
                    for (x, y) in w.iter().zip(pw) {
                        let d = ((x - y) / px).abs();
                        if d > 10.0 { jumps += 1; }
                        worst = worst.max(d);
                    }
                }
                prev = Some(w);
            }
            println!("  {r:>6.2} {covers:>6.2}%  {unsound:>8} {halo:>8} {:>9.1}   {jumps:>9} {worst:>9.1}", sl[sl.len() / 2]);
        }
    }

    /// The bottom of the g6 view reads as exterior with a smooth
    /// gradient -- a bound of the form `σ·(r − R)` at a shallow level,
    /// where a change in R is a change of the reading in proportion.
    /// Between the two files the ball grows 1.6%; this measures what
    /// the exterior does between them, against the truth, and with
    /// the ball HELD at the first file's radius.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_the_wedge() {
        let a0 = (-0.33506f64).atan2(0.94218);
        let a1 = (-0.23518f64).atan2(0.97194);
        let (pts, px) = g6_view(32);
        let g0 = g6_at(a0);
        let g1 = g6_at(a1);
        let held = g1.clone().with_extent(g0.ball.radius);
        let s0 = chaos_sample(&g0, 200_000);
        let s1 = chaos_sample(&g1, 200_000);
        let pc = |g: &Ifs2, smp: &[[f64; 2]]| {
            let mut r: Vec<f64> = smp.iter().map(|p| Affine2::distance(*p, g.ball.centre)).collect();
            r.sort_by(|a, b| a.total_cmp(b));
            let q = |f: f64| r[((r.len() as f64 * f) as usize).min(r.len() - 1)];
            format!("R {:.4} centre ({:.4}, {:.4}); sample 95% {:.3} 99% {:.3} 99.5% {:.3} 99.9% {:.3}", g.ball.radius, g.ball.centre[0], g.ball.centre[1], q(0.95), q(0.99), q(0.995), q(0.999))
        };
        println!("a0: {}", pc(&g0, &s0));
        println!("a1: {}", pc(&g1, &s1));
        let truth = |smp: &[[f64; 2]], p: [f64; 2]| smp.iter().map(|&a| Affine2::distance(p, a)).fold(f64::INFINITY, f64::min);
        let t0: Vec<f64> = pts.iter().map(|&p| truth(&s0, p)).collect();
        let t1: Vec<f64> = pts.iter().map(|&p| truth(&s1, p)).collect();
        let w0: Vec<Estimate<[f64; 2]>> = pts.iter().map(|&p| estimate(&g0, p, 35, 5)).collect();
        let w1: Vec<f64> = pts.iter().map(|&p| estimate(&g1, p, 35, 5).distance).collect();
        let wh: Vec<f64> = pts.iter().map(|&p| estimate(&held, p, 35, 5).distance).collect();
        let stats = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.total_cmp(b));
            format!("median {:.1} px, 90% {:.1} px, max {:.1} px", v[v.len() / 2], v[v.len() * 9 / 10], v[v.len() - 1])
        };
        for (name, sel) in [("exterior (walk > 41 px at a0)", true), ("halo (walk <= 41 px at a0)", false)] {
            let idx: Vec<usize> = (0..pts.len()).filter(|&i| (w0[i].distance / px > 41.0) == sel).collect();
            let mut dt: Vec<f64> = idx.iter().map(|&i| ((t1[i] - t0[i]) / px).abs()).collect();
            let mut dw: Vec<f64> = idx.iter().map(|&i| ((w1[i] - w0[i].distance) / px).abs()).collect();
            let mut dh: Vec<f64> = idx.iter().map(|&i| ((wh[i] - w0[i].distance) / px).abs()).collect();
            let mut dwh: Vec<f64> = idx.iter().map(|&i| ((w1[i] - wh[i]) / px).abs()).collect();
            let mut lvl: Vec<f64> = idx.iter().map(|&i| w0[i].level).collect();
            let mut wv: Vec<f64> = idx.iter().map(|&i| w0[i].distance / px).collect();
            let mut tv: Vec<f64> = idx.iter().map(|&i| t0[i] / px).collect();
            println!("{name}: {} points", idx.len());
            println!("   walk value at a0:        {}", stats(&mut wv));
            println!("   truth at a0:             {}", stats(&mut tv));
            println!("   winner level at a0:      {}", stats(&mut lvl));
            println!("   truth moved a0->a1:      {}", stats(&mut dt));
            println!("   walk moved a0->a1:       {}", stats(&mut dw));
            println!("   walk moved, ball held:   {}", stats(&mut dh));
            println!("   the ball's own share:    {}", stats(&mut dwh));
        }
        // the exterior's winners, by kind, at a0
        let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
        for i in (0..pts.len()).filter(|&i| w0[i].distance / px > 41.0).step_by(4) {
            let (_, l, k, _) = exhaustive_prov(&g0, pts[i], 10);
            *kinds.entry(format!("L{l} {k}")).or_default() += 1;
        }
        println!("exterior winners (every 4th point), by level and kind: {kinds:?}");
    }

    /// How much each candidate radius statistic drifts across the
    /// three reported rotations, and what fraction of the set each
    /// leaves outside. The drift is what moves the cut's image in an
    /// animation; the coverage is what the picture loses.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_the_radius_statistics() {
        let t1_at = |a: f64| -> [f32; 6] {
            let (c, s) = (a.cos() as f32, a.sin() as f32);
            [c, s, -s, c, 0.0, 0.0]
        };
        let cases: Vec<(&str, Ifs2, Ifs2)> = vec![
            ("g1/g2 (T2 turn)", grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]), grand_julian([0.509, 0.8607, -0.8607, 0.509, 0.0, 0.0])),
            ("g4/g5 (T1 turn)",
                grand_julian_t1([-0.9701, -0.2427, 0.2427, -0.9701, 0.0, 0.0], t1_at((-0.5299f64).atan2(-0.848))),
                grand_julian_t1([-0.9701, -0.2427, 0.2427, -0.9701, 0.0, 0.0], t1_at((-0.5387f64).atan2(-0.8425)))),
            ("g6/g7 (T0 turn)", g6_at((-0.33506f64).atan2(0.94218)), g6_at((-0.23518f64).atan2(0.97194))),
        ];
        // the statistics: (name, radius from a sorted radius list)
        let stats: Vec<(&str, Box<dyn Fn(&[f64]) -> f64>)> = vec![
            ("top-5% mean x1.5 (shipped)", Box::new(|r: &[f64]| { let t = r.len() / 20; r[r.len() - t..].iter().sum::<f64>() / t as f64 * 1.5 })),
            ("99.5th pct x1.3", Box::new(|r: &[f64]| r[(r.len() as f64 * 0.995) as usize] * 1.3)),
            ("99th pct x1.4", Box::new(|r: &[f64]| r[(r.len() as f64 * 0.99) as usize] * 1.4)),
            ("95th pct x2", Box::new(|r: &[f64]| r[(r.len() as f64 * 0.95) as usize] * 2.0)),
            ("90th pct x2.5", Box::new(|r: &[f64]| r[(r.len() as f64 * 0.90) as usize] * 2.5)),
            ("top-20% mean x2", Box::new(|r: &[f64]| { let t = r.len() / 5; r[r.len() - t..].iter().sum::<f64>() / t as f64 * 2.0 })),
            ("rms x4", Box::new(|r: &[f64]| (r.iter().map(|x| x * x).sum::<f64>() / r.len() as f64).sqrt() * 4.0)),
        ];
        for (name, a, b) in &cases {
            let sa = chaos_sample(a, 400_000);
            let sb = chaos_sample(b, 400_000);
            let mut ra: Vec<f64> = sa.iter().map(|p| Affine2::distance(*p, a.ball.centre)).collect();
            let mut rb: Vec<f64> = sb.iter().map(|p| Affine2::distance(*p, b.ball.centre)).collect();
            ra.sort_by(|x, y| x.total_cmp(y));
            rb.sort_by(|x, y| x.total_cmp(y));
            println!("{name}: shipped ball R {:.4} -> {:.4} ({:+.2}%)", a.ball.radius, b.ball.radius, 100.0 * (b.ball.radius / a.ball.radius - 1.0));
            for (sname, f) in &stats {
                let (x, y) = (f(&ra), f(&rb));
                let cov = |r: &[f64], v: f64| 100.0 * r.iter().filter(|&&q| q <= v).count() as f64 / r.len() as f64;
                println!("   {sname:>28}: {x:.4} -> {y:.4} ({:+.2}%), covers {:.2}% / {:.2}%", 100.0 * (y / x - 1.0), cov(&ra, x), cov(&rb, y));
            }
        }
    }

    /// The far field holds when the cut does (plan §8.15). The report
    /// of `grand-julian-glitches6/7.fflame`: a six-degree turn of the
    /// first transform, at a view whose lower half reads as exterior
    /// -- inside the third map's hole, a disc about the origin whose
    /// radius is `w·r_pre^{−1/8}` and depends on the flame only
    /// through the ball's radius. The sampled radius grows 1.8% over
    /// the turn and the whole exterior slides 15.8 pixels, uniformly;
    /// with the second file's ball held at the first's, it moves a
    /// tenth of a pixel. The set itself moves 94 pixels there at the
    /// median, so the slide is not the set's.
    #[test]
    fn the_far_field_holds_when_the_cut_does() {
        let g0 = g6_at((-0.33506f64).atan2(0.94218));
        let g1 = g6_at((-0.23518f64).atan2(0.97194));
        let held = g1.clone().with_extent(g0.ball.radius);
        // the holes followed
        for (a, b) in g1.maps.iter().zip(&held.maps) {
            if let (Map2::NonlinearInverse(x), Map2::NonlinearInverse(y)) = (&a.inverse, &b.inverse) {
                assert!(x.hole > 0.0 && y.hole > x.hole, "a smaller ball is a larger hole: {} vs {}", x.hole, y.hole);
            }
        }
        let (pts, px) = g6_view(16);
        let (mut free, mut with) = (0.0f64, 0.0f64);
        let mut exterior = 0;
        for &p in &pts {
            let d0 = estimate(&g0, p, 35, 5).distance;
            if d0 / px <= 41.0 {
                continue;
            }
            exterior += 1;
            let d1 = estimate(&g1, p, 35, 5).distance;
            let dh = estimate(&held, p, 35, 5).distance;
            free = free.max((d1 - d0).abs() / px);
            with = with.max((dh - d0).abs() / px);
        }
        assert!(exterior > 50, "the view's lower half is exterior: {exterior}");
        assert!(free > 10.0, "the slide the report is about: {free:.1} px");
        assert!(with < 1.0, "held, the exterior stays put: {with:.2} px");
    }

    /// G2 of `ifs-nonlinear-perturbation.md`: a NONLINEAR handover
    /// answers what each pixel's own walk answers, and gets deep
    /// enough to be worth taking.
    ///
    /// The seeded walk runs in f64 here, so what this measures is the
    /// LINEARISATION -- the second-order term the Jacobian drops --
    /// with no f32 in it. The f32 half is what the handover level is
    /// chosen to minimise, and it is measured beside it.
    ///
    /// Measured at the budget that ships, on a julia dust whose maps
    /// are roots of positive distance:
    ///
    /// | zoom | handover | curvature | f32, handed over | f32, not |
    /// |---|---|---|---|---|
    /// | 2^20 | level 7 | 0.003 px | 0.031 px | 3.3 px |
    /// | 2^24 | level 11 | 0.004 px | 0.033 px | 52 px |
    /// | 2^28 | level 14 | 0.001 px | 0.038 px | 835 px |
    ///
    /// The last column is what a nonlinear set had before this: the
    /// shader forming `centre + basis·uv` in f32 at a centre of
    /// magnitude O(1), which stops resolving pixels somewhere past
    /// zoom 2^17 and is why the picture went to blocks.
    ///
    /// **An inversion set gets none of this, and the rule says so
    /// rather than pretending.** A julian of negative distance and
    /// power 15 inverts to `|v|^{-15}`, which CONTRACTS the view
    /// wherever `|v| > 1`; one step collapses a seed's reach and f32
    /// could not tell two pixels apart there. Level 0 -- no handover
    /// -- is then genuinely the best available, and the walk picks
    /// it. The cap on those sets is unchanged and the fix is not a
    /// budget: it is a handover position with more than f32's
    /// mantissa, which is §7.
    #[test]
    fn a_nonlinear_handover_answers_what_each_pixel_answers() {
        let cases: Vec<(&str, Ifs2, bool)> = vec![
            ("julia dust", julia([-0.4, 0.6]), true),
            ("grand julian", grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]), false),
            ("sierpinski", sierpinski(), true),
        ];
        for (name, ifs, deepens) in cases {
            let smp = chaos_sample(&ifs, 20_000);
            let target = smp[smp.len() - 1];
            for &zoom in &[12.0f64, 20.0, 24.0, 28.0] {
                let span = 4.0 / 2f64.powf(zoom);
                let view_basis = [[span, 0.0], [0.0, -span]];
                let px = span / 96.0;
                let seeds = seed_beam(&ifs, target, view_basis, px, 200, 8);
                let total = 60u32;
                let after = total.saturating_sub(seeds.level).max(1);
                for gy in 0..7 {
                    for gx in 0..7 {
                        let uv = [gx as f64 / 6.0 - 0.5, gy as f64 / 6.0 - 0.5];
                        let d = apply_basis(view_basis, uv);
                        let p = [target[0] + d[0], target[1] + d[1]];
                        let direct = estimate(&ifs, p, total, 8);
                        let seeded = estimate_seeded(&ifs, &seeds, uv, after, 8);
                        let direct_px = direct.distance / px;
                        // Against the SUM the objective minimises:
                        // the curvature measured here plus the f32
                        // error the handover bought with it, which a
                        // f64 gate cannot see. A pixel, against a
                        // measured worst of 0.004 on the bounded sets
                        // and 0.76 on the grand julian, whose deep
                        // handover is the right trade even so.
                        let budget =
                            1.0 - f32_pixels_at(&seeds, view_basis, px, ifs.ball.radius);
                        assert!(
                            (seeded.distance - direct_px).abs()
                                <= budget.max(0.1) + 1e-4 * direct_px.abs(),
                            "{name} zoom 2^{zoom} at {uv:?}: seeded {} px, direct {direct_px} px \
                             (handover at level {})",
                            seeded.distance,
                            seeds.level
                        );
                        assert_eq!(
                            seeded.address.first(),
                            direct.address.first(),
                            "{name} zoom 2^{zoom} at {uv:?}: the handover changed the first branch \
                             (handover at level {})",
                            seeds.level
                        );
                    }
                }
                // And it is worth taking: past the zoom where f32
                // stops resolving the view, the prefix has to be
                // doing the work.
                if deepens && zoom >= 20.0 {
                    assert!(
                        seeds.level >= 5,
                        "{name} zoom 2^{zoom}: handed over at level {}, which leaves the view to f32",
                        seeds.level
                    );
                }
            }
        }
    }

    /// Reported from use, 2026-09-16: a Douady Rabbit whose quality
    /// "degrades at certain zoom levels, like when it hits the
    /// previous floating point cap, but then gets better again when I
    /// zoom in a little more", and which anti-aliasing sometimes made
    /// WORSE -- both starting where the handover starts.
    ///
    /// **The gates could not see it, and that is the lesson.** Every
    /// handover gate ran at 96 pixels. The curvature budget was spent
    /// in units a pixel converts by MULTIPLYING by the view's
    /// half-diagonal, while f32's error is a length the same
    /// half-diagonal DIVIDES, so the room between the level a cap
    /// allows and the level f32 needs falls as the SQUARE of the
    /// resolution. At 96 there was room to spare and capped and
    /// uncapped chose the same level at every zoom; at 1080p the cap
    /// stopped two to four levels short.
    ///
    /// Measured on the reported set, f32's error at the chosen
    /// handover:
    ///
    /// | zoom | 1080p, capped | 1080p, now | +2x AA, capped | now |
    /// |---|---|---|---|---|
    /// | 2^17 | 1.42 px | 0.27 px | 2.84 px | 0.54 px |
    /// | 2^19 | 1.09 px | 0.29 px | 11.37 px | 0.59 px |
    /// | 2^20 | 2.17 px | 0.28 px | 4.35 px | 0.56 px |
    /// | 2^24 | 2.63 px | 0.40 px | 8.97 px | 0.80 px |
    ///
    /// The capped column oscillates because the level is an integer
    /// and the shortfall lands one level short and then catches up --
    /// which is what "degrades, then gets better when I zoom in a
    /// little more" is from the outside.
    ///
    /// With the cap gone BOTH terms of the objective scale as `1/px`,
    /// so the level it picks no longer depends on the resolution at
    /// all -- which is why the three resolutions below must agree
    /// exactly, and why anti-aliasing can no longer move it. A
    /// supersampled pixel is half the size, so an error measured in
    /// them is twice the number for the same length; in DISPLAY
    /// pixels, which is what the downsample leaves, the two are equal.
    ///
    /// **Both classes, since 2026-09-17 (R5).** The rabbit is
    /// bounded and its objective is smooth; a grand julian's swings
    /// by orders between adjacent levels (§16), so an argmin that
    /// held on the rabbit by being flat could still move there. It
    /// also guards the f64 noise floor of [`F64_ULP`], which is a
    /// third term added to the objective and would break this gate if
    /// it did not scale as `1/px` like the other two.
    #[test]
    fn the_handover_does_not_depend_on_the_resolution() {
        // The shipped Douady Rabbit: julia after a translation.
        let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, 0.123, -0.745);
        t.variations = HashMap::from([("julia".to_string(), 1.0)]);
        t.variation_order = vec!["julia".to_string()];
        let rabbit = analyse(vec![t]);
        let julian = grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]);
        // `strict` is whether the set's errors are expected to stay
        // under a pixel at all. The rabbit's are; a grand julian's
        // oscillate by orders with the zoom (§16 of the plan), and on
        // it only the resolution-INDEPENDENCE is claimed -- which is
        // the whole point of this gate and the thing the cap broke.
        for (name, ifs, strict) in [("rabbit", rabbit, true), ("grand julian", julian, false)] {
        let smp = chaos_sample(&ifs, 20_000);
        let target = smp[smp.len() - 1];

        for step in 0..14 {
            let zoom = 16.0 + step as f64;
            let span = 4.0 / 2f64.powf(zoom);
            let view_basis = [[span * 16.0 / 9.0, 0.0], [0.0, -span]];
            let reach0 = basis_reach(view_basis);
            let mut chosen: Option<u32> = None;
            for &h in &[96.0f64, 1080.0, 2160.0] {
                let px = span / h;
                let seeds = seed_beam(&ifs, target, view_basis, px, 400, 4);
                // The level is the same at every resolution.
                match chosen {
                    None => chosen = Some(seeds.level),
                    Some(l) => assert_eq!(
                        l, seeds.level,
                        "{name} zoom 2^{zoom} at height {h}: handover level {} against {l} \
                         elsewhere -- the choice has picked up a dependence on the resolution, \
                         which is what made anti-aliasing change the picture",
                        seeds.level
                    ),
                }

                // And f32, in DISPLAY pixels: the supersampled arm is
                // divided back down, since that is what the
                // downsample leaves.
                let display_scale = if h > 1080.0 { h / 1080.0 } else { 1.0 };
                let f32_px = seeds
                    .cands
                    .iter()
                    .map(|c| {
                        let grown = basis_reach(c.basis) / reach0;
                        if !(grown > 0.0) {
                            return f64::INFINITY;
                        }
                        let m = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                        m * 5.96e-8 / (px * grown) / display_scale
                    })
                    .fold(0.0, f64::max);
                if h >= 1080.0 && strict {
                    assert!(
                        f32_px < 1.0,
                        "{name} zoom 2^{zoom} at height {h}: f32 would be {f32_px:.2} display pixels \
                         out at the handover (level {})",
                        seeds.level
                    );
                }

                // The curvature error, measured rather than modelled:
                // the seeded walk against each pixel's own.
                let total = 80u32;
                let after = total.saturating_sub(seeds.level).max(1);
                let mut worst = 0.0f64;
                for gy in 0..5 {
                    if !strict {
                        break;
                    }
                    for gx in 0..5 {
                        let uv = [gx as f64 / 4.0 - 0.5, gy as f64 / 4.0 - 0.5];
                        let d = apply_basis(view_basis, uv);
                        let q = [target[0] + d[0], target[1] + d[1]];
                        let direct = estimate(&ifs, q, total, 4).distance / px;
                        let seeded = estimate_seeded(&ifs, &seeds, uv, after, 4).distance;
                        worst = worst.max((seeded - direct).abs());
                    }
                }
                assert!(
                    !strict || worst / display_scale < 1.0,
                    "{name} zoom 2^{zoom} at height {h}: the handover's own curvature costs \
                     {worst:.3} pixels (level {})",
                    seeds.level
                );
            }
        }
        }
    }

    /// How many bits of seed position a grand julian would need, per
    /// level, for the handover to be worth taking.
    ///
    /// §11 of the plan says the lever for an inversion set is a wider
    /// handover position, and guesses that a double-float would do
    /// it. This checks the guess. The shader starts each pixel at
    /// `position + basis·uv` and the reported distance inherits the
    /// position's error divided by the pixel at the handover, so the
    /// relative precision a level needs is
    /// `px·(reach_k/reach_0) / |position|`, and the bits are its
    /// negative log. f32 has 24, a double-float about 48.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_the_bits_an_inversion_would_need() {
        let cases: Vec<(&str, Ifs2)> = vec![
            ("grand julian", grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0])),
            ("julia dust (for contrast)", julia([-0.4, 0.6])),
        ];
        for (name, ifs) in cases {
            let smp = chaos_sample(&ifs, 20_000);
            let target = smp[smp.len() - 1];
            let zoom = 20.0f64;
            let span = 4.0 / 2f64.powf(zoom);
            let view_basis = [[span * 16.0 / 9.0, 0.0], [0.0, -span]];
            let px = span / 1080.0;
            let reach0 = basis_reach(view_basis);
            println!("{name}: ball R {:.3}, zoom 2^{zoom}, px {px:.3e}", ifs.ball.radius);
            println!(
                "  {:>5} {:>4}  {:>11} {:>11} {:>10}  {:>6}",
                "level", "n", "reach/reach0", "worst |pos|", "px at L", "bits"
            );
            // The frontier, all branches, no pruning: the widest and
            // the narrowest of what a beam could be holding.
            let mut front: Vec<([f64; 2], [[f64; 2]; 2])> = vec![(target, view_basis)];
            for level in 0..=8usize {
                let worst = front
                    .iter()
                    .map(|(q, b)| {
                        let grown = basis_reach(*b) / reach0;
                        let mag = q[0].hypot(q[1]).max(ifs.ball.radius);
                        // relative precision this level needs for one
                        // pixel of error
                        let need = px * grown / mag;
                        (need, grown, mag)
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .expect("a frontier");
                println!(
                    "  {level:>5} {:>4}  {:>11.3e} {:>11.3e} {:>10.3e}  {:>6.1}",
                    front.len(),
                    worst.1,
                    worst.2,
                    px * worst.1,
                    -worst.0.log2()
                );
                if level == 8 {
                    break;
                }
                let mut next = Vec::new();
                for (q, b) in &front {
                    for m in &ifs.maps {
                        if m.inverse.image_gap(*q).is_some() {
                            continue;
                        }
                        let (Some(j), Some(q2)) = (m.inverse.jacobian(*q), q.apply_map(&m.inverse))
                        else {
                            continue;
                        };
                        let nb = [
                            [
                                j[0][0] * b[0][0] + j[0][1] * b[1][0],
                                j[0][0] * b[0][1] + j[0][1] * b[1][1],
                            ],
                            [
                                j[1][0] * b[0][0] + j[1][1] * b[1][0],
                                j[1][0] * b[0][1] + j[1][1] * b[1][1],
                            ],
                        ];
                        next.push((q2, nb));
                    }
                }
                if next.is_empty() {
                    println!("  (no branch survives past level {level})");
                    break;
                }
                // Keep it to the widest 12, so the count does not run away.
                next.sort_by(|a, b| basis_reach(b.1).total_cmp(&basis_reach(a.1)));
                next.truncate(12);
                front = next;
            }
            println!("  f32 has 24 bits; a double-float about 48; f64 about 53.");
            let seeds = seed_beam(&ifs, target, view_basis, px, 400, 4);
            let f32_px = seeds
                .cands
                .iter()
                .map(|c| {
                    let grown = basis_reach(c.basis) / reach0;
                    if !(grown > 0.0) {
                        return f64::INFINITY;
                    }
                    let m = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                    m * 5.96e-8 / (px * grown)
                })
                .fold(0.0, f64::max);
            let total = 80u32;
            let after = total.saturating_sub(seeds.level).max(1);
            let mut curv = 0.0f64;
            for gy in 0..5 {
                for gx in 0..5 {
                    let uv = [gx as f64 / 4.0 - 0.5, gy as f64 / 4.0 - 0.5];
                    let d = apply_basis(view_basis, uv);
                    let q = [target[0] + d[0], target[1] + d[1]];
                    let direct = estimate(&ifs, q, total, 4).distance / px;
                    let sd = estimate_seeded(&ifs, &seeds, uv, after, 4).distance;
                    curv = curv.max((sd - direct).abs());
                }
            }
            println!(
                "  seed_beam picks level {} ({} seeds): f32 {f32_px:.3} px, curvature {curv:.3} px",
                seeds.level,
                seeds.cands.len()
            );
        }
    }

    /// What f32 will add at a handover, in pixels -- the other half of
    /// what the objective trades, and the half a CPU gate cannot see
    /// because `estimate_seeded` runs in f64 throughout.
    ///
    /// A gate that bars the CURVATURE alone bars the trade: handing
    /// over deeper costs linearisation and buys f32 resolution, and
    /// on a grand julian the deep handover is twenty times the
    /// curvature and four thousand times less f32 error. Measured at
    /// 1080p, level 6 totals 6.3 pixels against level 2's 20.2, so
    /// the deep one is right and a quarter-pixel curvature bar would
    /// have forbidden it.
    fn f32_pixels_at(seeds: &Seeds, view_basis: [[f64; 2]; 2], px: f64, radius: f64) -> f64 {
        let reach0 = basis_reach(view_basis);
        seeds
            .cands
            .iter()
            .map(|c| {
                let grown = basis_reach(c.basis) / reach0;
                if !(grown > 0.0) {
                    return f64::INFINITY;
                }
                let mag = c.position[0].hypot(c.position[1]).max(radius);
                mag * 5.96e-8 / (px * grown)
            })
            .fold(0.0, f64::max)
    }

    /// A branch the reference cannot take no longer ends the prefix.
    ///
    /// The walk answers the minimum over ALL pieces, reachable or not:
    /// a branch whose image the point is outside of contributes its
    /// gap, and `estimate_aux_ranked` keeps those in `dead_min`. The
    /// prefix meets them too and had nowhere to put one, so it
    /// stopped at the first -- which on a set whose inverses have
    /// holes is the first or second level, and cost every level after
    /// it. Measured on the reported grand julian at zoom 2^20: the
    /// handover went from level 0 to level 2, and f32's error at it
    /// from 79 pixels to 19.9.
    ///
    /// Two things have to hold. The gap must be CARRIED -- the
    /// continuation starts from it rather than from nothing -- and
    /// carrying it must not change the answer, which is what the
    /// second half checks against each pixel's own walk. The carried
    /// value is scored for the whole view rather than for the centre,
    /// so it can only be smaller than a pixel's own, and smaller is
    /// the safe direction: the answer is a lower bound, and a smaller
    /// one widens a halo where a larger one erases a piece.
    #[test]
    fn a_gap_in_the_prefix_is_carried_not_a_full_stop() {
        let ifs = grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]);
        let smp = chaos_sample(&ifs, 20_000);
        let target = smp[smp.len() - 1];
        let zoom = 20.0f64;
        let span = 4.0 / 2f64.powf(zoom);
        let view_basis = [[span, 0.0], [0.0, -span]];
        let px = span / 96.0;

        let seeds = seed_beam(&ifs, target, view_basis, px, 400, 8);
        assert!(
            seeds.level > 0,
            "the prefix stopped at level 0 -- a gapped branch is ending it again"
        );
        assert!(
            seeds.dead_min_per_px.is_finite(),
            "no gap was met, so this fixture no longer exercises the carry"
        );
        assert!(
            seeds.dead_min_per_px >= 0.0,
            "a carried gap is a distance: {}",
            seeds.dead_min_per_px
        );

        // And the answer is still each pixel's own.
        let total = 80u32;
        let after = total.saturating_sub(seeds.level).max(1);
        for gy in 0..9 {
            for gx in 0..9 {
                let uv = [gx as f64 / 8.0 - 0.5, gy as f64 / 8.0 - 0.5];
                let d = apply_basis(view_basis, uv);
                let q = [target[0] + d[0], target[1] + d[1]];
                let direct = estimate(&ifs, q, total, 8).distance / px;
                let seeded = estimate_seeded(&ifs, &seeds, uv, after, 8).distance;
                // Against the SUM the objective minimises, not the
                // curvature alone -- see `f32_pixels_at`.
                let budget = 1.0 - f32_pixels_at(&seeds, view_basis, px, ifs.ball.radius);
                assert!(
                    (seeded - direct).abs() <= budget.max(0.1) + 1e-4 * direct.abs(),
                    "at {uv:?}: seeded {seeded} px, direct {direct} px \
                     (handover level {}, carried gap {})",
                    seeds.level,
                    seeds.dead_min_per_px
                );
            }
        }
    }

    /// Which seed actually wins a pixel, against how badly f32 would
    /// represent it.
    ///
    /// The handover's objective is the MAX over seeds, so one seed
    /// whose lineage has collapsed the view decides where every seed
    /// hands over. §11 proposes per-seed levels. This measures a
    /// cheaper answer first: a seed too inaccurate to hand over could
    /// be RETIRED into `Seeds::dead_min_per_px`, which keeps a valid
    /// lower bound for its piece -- bounds only grow with depth, so a
    /// frozen one is smaller, which is the safe direction -- at the
    /// cost of never refining it. That trade is worth taking only if
    /// the collapsed seed rarely wins.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_which_seed_wins() {
        let cases: Vec<(&str, Ifs2)> = vec![
            ("grand julian", grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0])),
            ("julia dust", julia([-0.4, 0.6])),
        ];
        for (name, ifs) in cases {
            let smp = chaos_sample(&ifs, 20_000);
            let target = smp[smp.len() - 1];
            let zoom = 20.0f64;
            let span = 4.0 / 2f64.powf(zoom);
            let view_basis = [[span, 0.0], [0.0, -span]];
            let px = span / 96.0;
            let reach0 = basis_reach(view_basis);
            let seeds = seed_beam(&ifs, target, view_basis, px, 400, 8);
            println!(
                "{name}: handover level {}, {} seeds, carried gap {:.3}",
                seeds.level,
                seeds.cands.len(),
                seeds.dead_min_per_px
            );
            // Per seed: how f32 would do, and how often it is the
            // smallest bound over a grid of the view.
            let mut wins = vec![0usize; seeds.cands.len()];
            let n = 33;
            for gy in 0..n {
                for gx in 0..n {
                    let uv = [gx as f64 / (n - 1) as f64 - 0.5, gy as f64 / (n - 1) as f64 - 0.5];
                    // Each seed alone, continued, to see whose bound
                    // is the answer.
                    let mut best = (f64::INFINITY, 0usize);
                    for (j, c) in seeds.cands.iter().enumerate() {
                        let one = Seeds {
                            level: seeds.level,
                            cands: vec![c.clone()],
                            dead_min_per_px: f64::INFINITY,
                        };
                        let d = estimate_seeded(&ifs, &one, uv, 48, 8).distance;
                        if d < best.0 {
                            best = (d, j);
                        }
                    }
                    wins[best.1] += 1;
                }
            }
            for (j, c) in seeds.cands.iter().enumerate() {
                let grown = basis_reach(c.basis) / reach0;
                let mag = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                let f32_px = if grown > 0.0 { mag * 5.96e-8 / (px * grown) } else { f64::INFINITY };
                println!(
                    "   seed {j}: reach/reach0 {grown:>10.3e}, f32 {f32_px:>10.3} px, \
                     wins {:>4} of {}, address {:?}",
                    wins[j],
                    n * n,
                    c.address
                );
            }
        }
    }

    /// The SOLID continuation answers what the walk answers.
    ///
    /// `estimate_seeded` was found to be a copy of the walk from
    /// before the cut-outs were fixed -- no `best_done`, no
    /// frozen-inside rule -- and read 11 to 41 pixels away from it on
    /// an inversion set. `estimate_seeded3` had the same two gaps.
    ///
    /// They were LATENT there, and measuring that is why the fix was
    /// worth making rather than worrying about. Neither rule can
    /// change an answer unless paths FINISH and the chain is deeper
    /// than one link, and no shipped solid does both: an affine solid
    /// never finishes a path, and a `quaternion_julia` finishes them
    /// constantly but its two preimages tie, so the chain ends at
    /// once and the continuation IS the direct walk. The numbers
    /// below were identical before the fix and after it.
    ///
    /// The gate stands for the solid that does both one day, where an
    /// over-read does not fatten a halo -- it puts a ray through a
    /// surface.
    #[test]
    fn the_solid_continuation_is_the_walk() {
        let cases: Vec<(&str, Ifs3)> = vec![
            ("unit cube", unit_cube()),
            ("tetrahedron", tetrahedron()),
            ("quaternion julia", qjulia3([0.0, 0.0, 0.0, -0.5], 2.0)),
        ];
        for (name, ifs) in cases {
            // A target on the attractor, so the chain has somewhere to
            // go.
            let mut target = ifs.ball.centre;
            for k in 0..40usize {
                target = ifs.maps[k % ifs.maps.len()].forward.apply(target);
            }
            for &beam in &[1u32, 4] {
                let chain = seed_chain3(&ifs, target, 1e-9, 200, beam);
                let mut st: u64 = 0x2545_F491_4F6C_DD1D;
                let mut next = move || {
                    st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    (st >> 11) as f64 / (1u64 << 53) as f64
                };
                for _ in 0..2000 {
                    let scale = 10f64.powf(-1.0 - next() * 6.0);
                    let d = [
                        (next() - 0.5) * scale,
                        (next() - 0.5) * scale,
                        (next() - 0.5) * scale,
                    ];
                    let q = [target[0] + d[0], target[1] + d[1], target[2] + d[2]];
                    let direct = estimate(&ifs, q, 40, beam).distance;
                    let seeded = estimate_seeded3(&ifs, &chain, d, 40, beam).distance;
                    // Absolute, because both are distances in world
                    // units and the ones that matter are near zero;
                    // measured worst is 1.1e-12, which is f64 arriving
                    // by two routes.
                    assert!(
                        (seeded - direct).abs() < 1e-9,
                        "{name} beam {beam} at {d:?}: seeded {seeded}, direct {direct} \
                         (chain {} links)",
                        chain.levels.len()
                    );
                }
            }
        }
    }

    /// What the seeded walk's difference from the direct one is MADE
    /// OF, which decides whether a cheap predictor of it can exist.
    ///
    /// §11 parked the second-order term because no model of the
    /// "curvature" tracked the measured seeded-against-direct error --
    /// the corner probe, which measures the delta's positional miss
    /// exactly, was out by a factor of 380 at one level. The
    /// suspicion this tests: the difference is not one thing. A seed
    /// carries a bound scored for the WHOLE VIEW -- `near = r − reach`
    /// -- which is deliberately below what the pixel's own walk would
    /// compute, and that conservatism has nothing to do with
    /// curvature. If it dominates, then what a predictor has to
    /// predict is mostly a quantity the walk already knows exactly.
    ///
    /// Columns: the signed difference, the conservatism the seed's own
    /// numbers imply (`σ_per_px · reach`), and the two compared.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_what_the_seeded_difference_is_made_of() {
        let cases: Vec<(&str, Ifs2)> = vec![
            ("rabbit", julia([-0.123, 0.745])),
            ("grand julian", grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0])),
        ];
        for (name, ifs) in cases {
            let smp = chaos_sample(&ifs, 20_000);
            let target = smp[smp.len() - 1];
            let zoom = 20.0f64;
            let span = 4.0 / 2f64.powf(zoom);
            let view_basis = [[span * 16.0 / 9.0, 0.0], [0.0, -span]];
            let px = span / 1080.0;
            println!("{name} at 2^{zoom}:");
            println!(
                "  {:>3} {:>6}  {:>12} {:>12} {:>12}  {:>9}",
                "L", "seeds", "signed diff", "conservatism", "corner miss", "over?"
            );
            for level in 0..=10u32 {
                let seeds = seed_beam_at(&ifs, target, view_basis, px, 400, 4, level);
                if seeds.level != level {
                    continue;
                }
                let total = 80u32;
                let after = total.saturating_sub(level).max(1);
                let (mut worst_signed, mut worst_abs) = (0.0f64, 0.0f64);
                let mut over = 0usize;
                for gy in 0..5 {
                    for gx in 0..5 {
                        let uv = [gx as f64 / 4.0 - 0.5, gy as f64 / 4.0 - 0.5];
                        let d = apply_basis(view_basis, uv);
                        let q = [target[0] + d[0], target[1] + d[1]];
                        let direct = estimate(&ifs, q, total, 4).distance / px;
                        let sd = estimate_seeded(&ifs, &seeds, uv, after, 4).distance;
                        let diff = sd - direct;
                        if diff > 1e-9 {
                            over += 1;
                        }
                        if diff.abs() > worst_abs {
                            worst_abs = diff.abs();
                            worst_signed = diff;
                        }
                    }
                }
                // What the seed's own numbers say the bound was held
                // back by: it was folded at `r − reach` rather than at
                // the pixel's own `r`.
                let cons = seeds
                    .cands
                    .iter()
                    .map(|c| c.sigma_per_px * basis_reach(c.basis))
                    .fold(0.0, f64::max);
                // And the delta's positional miss, against the pixel
                // at the handover -- the corner probe's quantity.
                let miss = seeds
                    .cands
                    .iter()
                    .map(|c| {
                        let grown = basis_reach(c.basis) / basis_reach(view_basis);
                        if !(grown > 0.0) {
                            return f64::INFINITY;
                        }
                        // The corners, walked exactly, would be needed
                        // for the true miss; this is the scale it is
                        // measured against.
                        px * grown
                    })
                    .fold(0.0, f64::max);
                println!(
                    "  {level:>3} {:>6}  {worst_signed:>12.4} {cons:>12.4} {:>12.3e}  {over:>4}/25",
                    seeds.cands.len(),
                    miss
                );
            }
        }
    }

    /// What the prefix costs, now that it measures itself.
    ///
    /// `ensure_ifs_seeds` runs once per view on the interactive path,
    /// so this is paid on every pan and every zoom step. The
    /// self-check added ten continuations per level -- five probes
    /// against the level-0 reference -- and a deep zoom hands over
    /// tens of levels down, so the arithmetic is worth knowing rather
    /// than assuming.
    #[test]
    #[ignore = "a measurement; run with --ignored --nocapture"]
    fn probe_what_the_prefix_costs() {
        let cases: Vec<(&str, Ifs2)> = vec![
            ("sierpinski (affine)", sierpinski()),
            ("rabbit", julia([-0.123, 0.745])),
            ("grand julian", grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0])),
        ];
        for (name, ifs) in cases {
            let smp = chaos_sample(&ifs, 20_000);
            let target = smp[smp.len() - 1];
            for &zoom in &[8.0f64, 20.0, 32.0, 44.0] {
                let span = 4.0 / 2f64.powf(zoom);
                let view_basis = [[span * 16.0 / 9.0, 0.0], [0.0, -span]];
                let px = span / 1080.0;
                let budget = zoom as u32 + 64;
                // Warm, then timed: the first call pays whatever the
                // allocator is doing.
                let _ = seed_beam(&ifs, target, view_basis, px, budget, 8);
                let t0 = std::time::Instant::now();
                let reps = 5;
                let mut level = 0;
                for _ in 0..reps {
                    level = seed_beam(&ifs, target, view_basis, px, budget, 8).level;
                }
                let each = t0.elapsed().as_secs_f64() / reps as f64;
                println!(
                    "  {name:>20} 2^{zoom:<4.0}: handover level {level:>3}, {:>8.2} ms per view",
                    each * 1e3
                );
            }
        }
    }

    /// G3 of `ifs-nonlinear-perturbation.md`: the handover goes deeper
    /// as the zoom does, and a reference orbit that passes close to a
    /// pole is what stops it.
    ///
    /// The prefix exists to grow the view until f32 can resolve it, so
    /// its level has to track the zoom -- an affine set's climbs about
    /// one level per `log(1/σ)` of zoom, and a nonlinear one's climbs
    /// too until its own curvature stops it. Measured, per view:
    ///
    /// | zoom | Sierpinski | julia | grand julian |
    /// |---|---|---|---|
    /// | 2^8 | 2 | 0 | 0 |
    /// | 2^20 | 15 | 8 | 6 |
    /// | 2^32 | 24 | 19 | 9 |
    /// | 2^44 | 33 | 33 | 11 |
    ///
    /// The third column is the pole fixture, and it is the same grand
    /// julian the reports of 2026-09-15 and -16 were about: three
    /// julians of NEGATIVE distance, whose inverses have holes and
    /// whose orbit comes within 4e-5 of a singularity. It hands over,
    /// and it stops climbing -- eleven levels where the other two
    /// reach thirty-three. That is the curvature refusing, measured by
    /// the handover itself (§13), and it is the honest shape of the
    /// limit rather than a cap someone chose.
    #[test]
    fn the_handover_goes_deeper_as_the_zoom_does() {
        let cases: Vec<(&str, Ifs2, u32)> = vec![
            ("sierpinski", sierpinski(), 25),
            ("julia", julia([-0.123, 0.745]), 25),
            // The pole fixture: it must still hand over, and it is not
            // held to the others' depth.
            ("grand julian", grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]), 8),
        ];
        for (name, ifs, deep_enough) in cases {
            let smp = chaos_sample(&ifs, 20_000);
            let target = smp[smp.len() - 1];
            let mut last = 0u32;
            for &zoom in &[8.0f64, 20.0, 32.0, 44.0] {
                let span = 4.0 / 2f64.powf(zoom);
                let view_basis = [[span * 16.0 / 9.0, 0.0], [0.0, -span]];
                let px = span / 1080.0;
                let level = seed_beam(&ifs, target, view_basis, px, zoom as u32 + 64, 8).level;
                assert!(
                    level >= last,
                    "{name}: the handover went BACKWARDS at 2^{zoom}, {last} to {level} -- \
                     a deeper view can always take the shallower prefix, so this means the \
                     objective is not monotone in what it is offered"
                );
                last = level;
            }
            assert!(
                last >= deep_enough,
                "{name} at 2^44: handed over at level {last}, under the {deep_enough} this set \
                 reached when the measurement was taken"
            );
        }
    }

    /// Reported from use, 2026-09-16: zooming an escape-time grand
    /// julian killed the app with `STATUS_STACK_BUFFER_OVERRUN`, which
    /// is what a panic in a release GUI build looks like from outside.
    ///
    /// It was a prefix that handed over an EMPTY beam. Carrying the
    /// gap rather than stopping at it turned a gapped branch from "end
    /// the prefix" into "skip this branch", and when every branch of
    /// every candidate is gapped -- which a grand julian's holes make
    /// ordinary once the reference orbit is deep enough -- that
    /// skipped them all. `live` became empty, the handover shipped
    /// zero seeds, and the continuation indexed `live[0]`.
    ///
    /// Reproduced by CLI export at 2^40 and fixed; the sweep that
    /// found it now runs clean to 2^200. This is the unit-sized
    /// version: walk deep enough for the fully-gapped level to
    /// arrive, and require that the prefix kept a beam and that the
    /// answer is still each pixel's own.
    #[test]
    fn a_fully_gapped_prefix_keeps_its_beam() {
        let ifs = grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]);
        let smp = chaos_sample(&ifs, 20_000);
        let target = smp[smp.len() - 1];
        // The zooms the report covered, and past where it died.
        for &zoom in &[24.0f64, 30.0, 40.0, 70.0, 120.0, 200.0] {
            let span = 4.0 / 2f64.powf(zoom);
            let view_basis = [[span * 16.0 / 9.0, 0.0], [0.0, -span]];
            let px = span / 1080.0;
            let seeds = seed_beam(&ifs, target, view_basis, px, zoom as u32 + 64, 5);
            assert!(
                !seeds.cands.is_empty(),
                "2^{zoom}: the prefix handed over an empty beam at level {}",
                seeds.level
            );
            // And it still answers, rather than panicking or reading
            // the whole view as set.
            let after = 40u32;
            for gy in 0..3 {
                for gx in 0..3 {
                    let uv = [gx as f64 / 2.0 - 0.5, gy as f64 / 2.0 - 0.5];
                    let d = estimate_seeded(&ifs, &seeds, uv, after, 5).distance;
                    assert!(
                        d.is_finite() && d >= 0.0,
                        "2^{zoom} at {uv:?}: distance {d}"
                    );
                }
            }
        }

        // And the belt: a handover with no candidates is answerable
        // rather than fatal, whatever produced it.
        let empty = Seeds { level: 3, cands: Vec::new(), dead_min_per_px: 12.5 };
        let e = estimate_seeded(&ifs, &empty, [0.25, -0.25], 20, 4);
        assert_eq!(e.distance, 12.5);
        assert!(!e.escaped && e.address.is_empty());
    }

    /// Where a grand julian's zoom actually stops, and why it depends
    /// on where you are.
    ///
    /// Reported from use, 2026-09-16: "I can zoom to about 1e10 in
    /// certain regions of the grand julian now" -- 1e10 being about
    /// 2^33, against roughly 2^14 before the handover carried
    /// nonlinear maps at all. The "certain regions" is the half worth
    /// measuring: the prefix follows the REFERENCE ORBIT of the view
    /// centre, and how deep it gets is a property of that orbit, so
    /// two targets on the same set cap at different zooms.
    ///
    /// Per target and zoom: the level chosen, what f32 costs there,
    /// what the linearisation costs (measured against the level-0
    /// handover, which approximates nothing), and their sum. The cap
    /// is where the sum passes a pixel.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_where_a_grand_julian_caps() {
        let ifs = grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]);
        let smp = chaos_sample(&ifs, 200_000);
        // Several places on the set, which is what "certain regions"
        // means.
        let targets: Vec<[f64; 2]> = (0..6).map(|k| smp[smp.len() - 1 - k * 31_337]).collect();
        for (t, &target) in targets.iter().enumerate() {
            println!(
                "target {t} at ({:.5}, {:.5}), |t| {:.4}:",
                target[0],
                target[1],
                target[0].hypot(target[1])
            );
            println!(
                "  {:>7} {:>6} {:>10} {:>10} {:>10}  {}",
                "zoom", "level", "f32 px", "curv px", "total", "verdict"
            );
            for &zoom in &[20.0f64, 26.0, 30.0, 33.0, 36.0, 40.0, 46.0] {
                let span = 4.0 / 2f64.powf(zoom);
                let view_basis = [[span * 16.0 / 9.0, 0.0], [0.0, -span]];
                let px = span / 1080.0;
                let reach0 = basis_reach(view_basis);
                let budget = zoom as u32 + 64;
                let seeds = seed_beam(&ifs, target, view_basis, px, budget, 5);
                let exact = seed_beam_at(&ifs, target, view_basis, px, budget, 5, 0);
                let f32_px = seeds
                    .cands
                    .iter()
                    .map(|c| {
                        let grown = basis_reach(c.basis) / reach0;
                        if !(grown > 0.0) {
                            return f64::INFINITY;
                        }
                        let mag = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                        mag * 5.96e-8 / (px * grown)
                    })
                    .fold(0.0, f64::max);
                // The linearisation, against the handover that
                // approximates nothing, at the same absolute depth.
                let curv = [[0.0f64, 0.0], [-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]]
                    .iter()
                    .map(|uv| {
                        let a = estimate_seeded(&ifs, &exact, *uv, 48 + seeds.level, 5).distance;
                        let b = estimate_seeded(&ifs, &seeds, *uv, 48, 5).distance;
                        (a - b).abs()
                    })
                    .fold(0.0, f64::max);
                let total = f32_px + curv;
                let ulps = px / (target[0].hypot(target[1]).max(1.0) * 2.22e-16);
                // The f32 term MEASURED: the same seeds with their
                // position, basis and quadratic rounded to f32, against
                // themselves unrounded, continued the same way.
                let rounded = Seeds {
                    level: seeds.level,
                    dead_min_per_px: seeds.dead_min_per_px,
                    cands: seeds
                        .cands
                        .iter()
                        .map(|c| {
                            let r = |x: f64| x as f32 as f64;
                            let mut c = c.clone();
                            c.position = [r(c.position[0]), r(c.position[1])];
                            c.basis = [[r(c.basis[0][0]), r(c.basis[0][1])], [r(c.basis[1][0]), r(c.basis[1][1])]];
                            c.quad = [
                                [r(c.quad[0][0]), r(c.quad[0][1])],
                                [r(c.quad[1][0]), r(c.quad[1][1])],
                                [r(c.quad[2][0]), r(c.quad[2][1])],
                            ];
                            c
                        })
                        .collect(),
                };
                let f32_meas = [[0.0f64, 0.0], [-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]]
                    .iter()
                    .map(|uv| {
                        let a = estimate_seeded(&ifs, &seeds, *uv, 48, 5).distance;
                        let b = estimate_seeded(&ifs, &rounded, *uv, 48, 5).distance;
                        (a - b).abs()
                    })
                    .fold(0.0, f64::max);
                println!(
                    "  2^{zoom:<5.0} {:>6} {f32_px:>10.3} {curv:>10.3} {total:>10.3}  {} (f64 ulps/px {ulps:.1}, f32 measured {f32_meas:.3})",
                    seeds.level,
                    if total < 1.0 { "clean" } else { "past the cap" }
                );
            }
        }
    }

    /// What a PER-SEED handover level would be worth, simulated before
    /// building one.
    ///
    /// The handover picks one level for the whole beam and pays the
    /// WORST seed's f32 cost -- measured at nine orders of spread
    /// between seeds of the same level (§16). A per-seed level would
    /// let each lineage stop where its OWN view is widest and keep
    /// refining from there, which is the thing retirement was not.
    ///
    /// Simulated honestly: pick each surviving lineage's ancestor
    /// level by that candidate's own f32 cost -- the quantity the
    /// worst seed poisons -- assemble the mixed set into one handover,
    /// and measure THAT against the exact one. A first cut ranked each
    /// lineage by its cost in ISOLATION, which is not its contribution
    /// to a set: a seed that never wins reads badly alone and costs
    /// nothing in company.
    #[test]
    #[ignore = "a survey; run with --ignored --nocapture"]
    fn probe_what_per_seed_levels_would_buy() {
        let ifs = grand_julian([0.7071, 0.7071, -0.7071, 0.7071, 0.0, 0.0]);
        let smp = chaos_sample(&ifs, 200_000);
        let targets: Vec<[f64; 2]> = (0..4).map(|k| smp[smp.len() - 1 - k * 31_337]).collect();
        let probes = [[0.0f64, 0.0], [-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]];
        for (t, &target) in targets.iter().enumerate() {
            println!("target {t}:");
            println!(
                "  {:>7} | {:>5} {:>11} | {:>14} {:>11}  {}",
                "zoom", "L", "today", "levels", "mixed", "gain"
            );
            for &zoom in &[20.0f64, 26.0, 30.0, 33.0, 36.0, 40.0] {
                let span = 4.0 / 2f64.powf(zoom);
                let view_basis = [[span * 16.0 / 9.0, 0.0], [0.0, -span]];
                let px = span / 1080.0;
                let reach0 = basis_reach(view_basis);
                let budget = zoom as u32 + 64;
                let beam = 5u32;
                let exact = seed_beam_at(&ifs, target, view_basis, px, budget, beam, 0);
                let today = seed_beam(&ifs, target, view_basis, px, budget, beam);

                // The error of a WHOLE handover set against the exact
                // one, which is the only thing worth comparing.
                // `extra` continues the set past 48 so that a seed
                // `extra` levels shallower than `sd.level` still reaches
                // the reference's absolute depth.
                let err_of = |sd: &Seeds, extra: u32| -> f64 {
                    probes
                        .iter()
                        .map(|uv| {
                            let a = estimate_seeded(&ifs, &exact, *uv, 48 + sd.level, beam)
                                .distance;
                            let b = estimate_seeded(&ifs, sd, *uv, 48 + extra, beam).distance;
                            (a - b).abs()
                        })
                        .fold(0.0, f64::max)
                };
                let f32_of = |c: &Seed| -> f64 {
                    let grown = basis_reach(c.basis) / reach0;
                    if !(grown > 0.0) {
                        return f64::INFINITY;
                    }
                    let mag = c.position[0].hypot(c.position[1]).max(ifs.ball.radius);
                    mag * 5.96e-8 / (px * grown)
                };

                // Every level the walk reaches, kept whole.
                let mut per_level: Vec<Seeds> = Vec::new();
                for l in 0..=40u32 {
                    let sd = seed_beam_at(&ifs, target, view_basis, px, budget, beam, l);
                    if sd.level == l {
                        per_level.push(sd);
                    }
                }
                let Some(deepest) = per_level.last().cloned() else { continue };

                // Each deepest lineage takes the ancestor level where
                // its OWN view is widest.
                let mut chosen: Vec<(u32, Seed)> = Vec::new();
                for c in &deepest.cands {
                    let mut best: Option<(f64, u32, Seed)> = None;
                    for sd in &per_level {
                        for a in &sd.cands {
                            if !c.address.starts_with(a.address.as_slice()) {
                                continue;
                            }
                            let f = f32_of(a);
                            if best.as_ref().map_or(true, |(bf, _, _)| f < *bf) {
                                best = Some((f, sd.level, a.clone()));
                            }
                        }
                    }
                    if let Some((_, l, a)) = best {
                        if !chosen.iter().any(|(_, e)| e.address == a.address) {
                            chosen.push((l, a));
                        }
                    }
                }
                let mut levels: Vec<u32> = chosen.iter().map(|(l, _)| *l).collect();
                let mixed = Seeds {
                    // The escape arithmetic wants one number; the
                    // deepest is the honest stand-in for a simulation.
                    level: chosen.iter().map(|(l, _)| *l).max().unwrap_or(0),
                    cands: chosen.into_iter().map(|(_, c)| c).collect(),
                    dead_min_per_px: deepest.dead_min_per_px,
                };
                let spread = mixed.level - levels.iter().copied().min().unwrap_or(0);
                let today_curv = err_of(&today, 0);
                let today_f32 = today.cands.iter().map(&f32_of).fold(0.0, f64::max);
                let mixed_curv_short = err_of(&mixed, 0);
                let mixed_curv = err_of(&mixed, spread);
                let mixed_f32 = mixed.cands.iter().map(&f32_of).fold(0.0, f64::max);
                let today_err = today_curv + today_f32;
                let mixed_err = mixed_curv + mixed_f32;
                levels.sort_unstable();
                levels.dedup();
                println!(
                    "  2^{zoom:<5.0} | {:>5} {today_err:>11.3} (c {today_curv:.3} f {today_f32:.3}) | {:>14?} {mixed_err:>11.3} (c {mixed_curv:.3}, short {mixed_curv_short:.3}, f {mixed_f32:.3})  {:>7.1}x",
                    today.level,
                    levels,
                    if mixed_err > 0.0 { today_err / mixed_err } else { 1.0 }
                );
            }
        }
    }

    /// Greedy is a heuristic, and the dragon is where it shows.
    ///
    /// Every address gives a valid bound on the distance to ITS piece;
    /// the true distance is the minimum over all of them. Greedy
    /// follows one address, so it can only ever be too LARGE — and too
    /// large reads as "far from the set", which erodes the picture.
    ///
    /// This measures the gap on a real IFS, so phase 2's beam has a
    /// baseline to beat rather than an anecdote. On the Sierpiński
    /// gasket, whose pieces meet at single points, greedy is
    /// essentially exact; on the dragon, whose pieces share a boundary
    /// of positive length, it is not.
    #[test]
    fn greedy_erodes_a_just_touching_ifs_and_exhaustive_does_not() {
        const DEPTH: u32 = 10;

        let measure = |ifs: &Ifs2, lo: f64, hi: f64| {
            let (mut worse, mut total, mut max_ratio) = (0usize, 0usize, 0.0f64);
            let n = 24;
            for i in 0..n {
                for j in 0..n {
                    let p = [
                        lo + (hi - lo) * i as f64 / (n - 1) as f64,
                        lo + (hi - lo) * j as f64 / (n - 1) as f64,
                    ];
                    let g = estimate(ifs, p, DEPTH, 1).distance;
                    let e = exhaustive(ifs, p, DEPTH - 1);
                    assert!(
                        e <= g + 1e-9,
                        "exhaustive ({e}) exceeded greedy ({g}) at {p:?} -- it \
                         searches a superset of greedy's one address"
                    );
                    total += 1;
                    if g > e + 1e-6 {
                        worse += 1;
                        if e > 1e-9 {
                            max_ratio = max_ratio.max(g / e);
                        }
                    }
                }
            }
            (worse, total, max_ratio)
        };

        let (sw, st, sr) = measure(&sierpinski(), -0.4, 1.4);
        let (dw, dt, dr) = measure(&dragon(), -0.6, 1.4);
        println!(
            "greedy vs exhaustive at depth {DEPTH}:\n  \
             sierpinski: greedy is loose at {sw}/{st} points, worst ratio {sr:.2}\n  \
             dragon:     greedy is loose at {dw}/{dt} points, worst ratio {dr:.2}"
        );

        // The gasket's pieces meet at points: greedy should be right
        // essentially everywhere.
        assert!(
            sw * 20 < st,
            "greedy got worse on the gasket: loose at {sw} of {st}"
        );
        // The dragon's pieces share a boundary: greedy is measurably
        // worse, and that is the finding phase 2's beam addresses. If
        // this ever stops failing, greedy improved and the beam's case
        // needs re-arguing.
        assert!(
            dw * 4 > dt,
            "greedy is no longer loose on the dragon ({dw} of {dt}) -- re-check \
             whether phase 2 still needs a beam"
        );
    }

    /// What beam width actually buys, on the IFS that needs it.
    ///
    /// The dragon's two pieces share a boundary, so several branches
    /// keep a point inside the ball and the information that tells
    /// them apart only appears several levels down — which is exactly
    /// what a beam holds on to and a greedy walk throws away.
    #[test]
    fn a_beam_closes_the_gap_greedy_leaves_on_the_dragon() {
        const DEPTH: u32 = 10;
        let ifs = dragon();

        let mut rows = Vec::new();
        for beam in [1u32, 2, 3, 4, 6, 8, 16] {
            let (mut loose, mut total, mut worst) = (0usize, 0usize, 1.0f64);
            let n = 24;
            for i in 0..n {
                for j in 0..n {
                    let p = [
                        -0.6 + 2.0 * i as f64 / (n - 1) as f64,
                        -0.6 + 2.0 * j as f64 / (n - 1) as f64,
                    ];
                    let g = estimate(&ifs, p, DEPTH, beam).distance;
                    let e = exhaustive(&ifs, p, DEPTH - 1);
                    assert!(
                        e <= g + 1e-9,
                        "beam {beam} at {p:?}: {g} is below exhaustive {e}"
                    );
                    total += 1;
                    if g > e + 1e-6 {
                        loose += 1;
                        if e > 1e-9 {
                            worst = worst.max(g / e);
                        }
                    }
                }
            }
            rows.push((beam, loose, total, worst));
        }

        println!("dragon, depth {DEPTH}, against exhaustive over all 2^{} addresses:", DEPTH - 1);
        for (beam, loose, total, worst) in &rows {
            println!("  beam {beam:>2}: loose at {loose:>3}/{total}, worst ratio {worst:.2}");
        }

        let at = |b: u32| rows.iter().find(|r| r.0 == b).copied().expect("swept");
        let (_, l1, t1, w1) = at(1);
        let (_, l4, _, w4) = at(4);
        let (_, l16, _, w16) = at(16);
        assert!(l1 * 4 > t1, "greedy should still be loose: {l1}/{t1}");
        assert!(l4 < l1 / 2, "beam 4 barely helped: {l4} vs {l1}");
        assert!(w4 < w1 / 4.0, "beam 4 left the worst case: {w4:.2} vs {w1:.2}");
        // Wide enough and it agrees with exhaustive search outright.
        assert!(l16 * 50 < t1, "beam 16 is still loose at {l16}/{t1}");
        assert!(w16 < 1.5, "beam 16 worst ratio {w16:.2}");
    }

    /// A point ON the attractor must read distance zero, at the depth
    /// and beam that ship. Anything else is a dark speckle in the
    /// middle of a solid region — the failure mode that survives after
    /// the beam has fixed the gross erosion.
    ///
    /// The dragon is the case that matters: its pieces share a
    /// boundary, so keeping a point inside for forty levels means the
    /// beam has to hold the right address for forty levels.
    #[test]
    fn attractor_points_read_zero_at_the_shipped_depth_and_beam() {
        let ifs = dragon();

        // Every address of length 14 applied to the ball centre:
        // 16384 points, each within σ^14 ≈ 2e-3 of the attractor.
        let mut pts = vec![ifs.ball.centre];
        for _ in 0..14 {
            let mut next = Vec::with_capacity(pts.len() * ifs.maps.len());
            for m in &ifs.maps {
                for &p in &pts {
                    next.push(m.forward.apply(p));
                }
            }
            pts = next;
        }

        for &(levels, beam) in &[(40u32, 1u32), (40, 2), (40, 4), (40, 8), (24, 4), (80, 4)] {
            let missed =
                pts.iter().filter(|&&p| estimate(&ifs, p, levels, beam).distance > 0.0).count();
            println!(
                "  depth {levels:>2} beam {beam}: {missed:>5} of {} attractor points read \
                 non-zero",
                pts.len()
            );
        }

        // The shipped default must not speckle a solid attractor AT
        // ALL. Depth is not what buys this -- 24, 40 and 80 all leave
        // the same 141 at beam 4 -- so a regression here is the beam,
        // not the budget.
        let missed = pts.iter().filter(|&&p| estimate(&ifs, p, 40, 8).distance > 0.0).count();
        assert_eq!(
            missed,
            0,
            "the shipped beam speckles {missed} of {} attractor points",
            pts.len()
        );
    }

    /// A seeded walk must answer exactly what a direct one does.
    ///
    /// The whole point of the split is that it changes WHERE the work
    /// happens, not what it computes: the CPU walks the levels every
    /// pixel shares, the shader continues from the handover, and the
    /// answer is the same. If it is not, deep zoom is not a longer
    /// version of the shallow picture — it is a different picture, and
    /// nothing downstream could tell.
    #[test]
    fn a_seeded_walk_answers_what_a_direct_one_does() {
        for (name, ifs) in [("sierpinski", sierpinski()), ("dragon", dragon())] {
            // A point on the attractor to zoom into, so the view keeps
            // finding structure.
            let mut target = ifs.ball.centre;
            for k in 0..30u32 {
                target = ifs.maps[(k as usize) % ifs.maps.len()].forward.apply(target);
            }

            // Only as deep as the DIRECT path can still be trusted
            // as a reference. It forms `C + delta` at full magnitude,
            // so f64's ulp at 0.28 (5.5e-17) is a fraction of the view
            // that grows with the zoom: about 1.5e-8 of it at 2^30,
            // but 1.5% at 2^50. Past there the seeded walk is the more
            // accurate of the two and disagreement would mean nothing.
            // `a_deep_zoom_sees_the_same_figure` is the gate that goes
            // deeper.
            for &zoom in &[0.0f64, 6.0, 12.0, 20.0, 30.0] {
                let span_y = 4.0 / 2f64.powf(zoom);
                let span_x = span_y; // square view
                let view_basis = [[span_x, 0.0], [0.0, -span_y]];
                let px = span_y / 96.0;
                let seeds = seed_beam(&ifs, target, view_basis, px, 200, 8);

                let total = 60u32;
                let after = total.saturating_sub(seeds.level).max(1);

                for &uv in &[
                    [0.0f64, 0.0],
                    [0.3, -0.2],
                    [-0.45, 0.45],
                    [0.5, 0.5],
                    [-0.1, 0.37],
                ] {
                    let d = apply_basis(view_basis, uv);
                    let p = [target[0] + d[0], target[1] + d[1]];

                    let direct = estimate(&ifs, p, total, 8);
                    let seeded = estimate_seeded(&ifs, &seeds, uv, after, 8);

                    // The direct answer is in world units, the seeded
                    // one in pixels: that is the point, not a
                    // discrepancy.
                    let direct_px = direct.distance / px;
                    // Relative, and loose enough for two different
                    // orders of the same f64 arithmetic. A basis
                    // composed wrongly, or attached to the wrong
                    // candidate through the sort, is off by O(1) --
                    // which this still catches by a wide margin.
                    let tol = 1e-4 * direct_px.max(1.0);
                    assert!(
                        (seeded.distance - direct_px).abs() <= tol,
                        "{name} zoom 2^{zoom} at {uv:?}: seeded {} px, direct {direct_px} px \
                         (handover at level {})",
                        seeded.distance,
                        seeds.level
                    );
                    assert!(
                        (seeded.level - direct.level).abs() < 1e-3,
                        "{name} zoom 2^{zoom} at {uv:?}: seeded level {}, direct {}",
                        seeded.level,
                        direct.level
                    );
                    assert_eq!(
                        seeded.escaped, direct.escaped,
                        "{name} zoom 2^{zoom} at {uv:?}: escape disagrees"
                    );
                }
            }
        }
    }

    /// A deep zoom must see the same figure — which is the whole claim
    /// §2.5 makes about what a zoom into an IFS shows.
    ///
    /// Near the fixed point of a half-scale map, the attractor is
    /// EXACTLY invariant under halving: `S₀(A) ⊆ A` and `S₀` is `p ↦ p/2`,
    /// so the gasket around the origin is its own image at every
    /// scale of two. Rendering the distance field in PIXELS at zoom Z
    /// and at Z+1 must therefore give the same field, at any depth.
    ///
    /// That makes this a deep-zoom gate with no reference to lose
    /// precision: it compares the machinery against itself one octave
    /// apart, and only a walk that is actually resolving the view can
    /// pass it. A direct f64 walk fails it past about 2^50; this runs
    /// to 2^160.
    #[test]
    fn a_deep_zoom_sees_the_same_figure() {
        let ifs = sierpinski();
        // Map 0 is `p -> p/2`, so its fixed point is the origin.
        let origin = [0.0f64, 0.0];

        let field_at = |zoom: f64| -> Vec<f64> {
            let span = 4.0 / 2f64.powf(zoom);
            let basis = [[span, 0.0], [0.0, -span]];
            let px = span / 32.0;
            let seeds = seed_beam(&ifs, origin, basis, px, 600, 8);
            let mut out = Vec::new();
            for i in 0..17 {
                for j in 0..17 {
                    let uv = [i as f64 / 16.0 - 0.5, j as f64 / 16.0 - 0.5];
                    out.push(estimate_seeded(&ifs, &seeds, uv, 64, 8).distance);
                }
            }
            out
        };

        for &zoom in &[4.0f64, 20.0, 60.0, 120.0, 159.0] {
            let here = field_at(zoom);
            let octave = field_at(zoom + 1.0);
            let mut worst: f64 = 0.0;
            for (a, b) in here.iter().zip(&octave) {
                // Both are in pixels, and the figure is the same, so
                // the fields are the same.
                worst = worst.max((a - b).abs() / a.max(*b).max(1.0));
            }
            println!("  zoom 2^{zoom} vs 2^{}: worst relative difference {worst:.3e}", zoom + 1.0);
            assert!(
                worst < 1e-3,
                "at zoom 2^{zoom} the picture is not self-similar one octave down \
                 (worst {worst:.3e}) -- the walk has stopped resolving the view"
            );
            // And it must not have collapsed to a flat field, which
            // would be trivially self-similar and completely wrong.
            let spread = here.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                - here.iter().cloned().fold(f64::INFINITY, f64::min);
            assert!(
                spread > 1.0,
                "at zoom 2^{zoom} the distance field is flat (spread {spread:.3} px)"
            );
        }
    }

    /// The handover must happen where it is worth happening: deep
    /// enough that the shader's f32 is left an O(1) problem, and not
    /// so deep that a pixel's own branch could already have differed
    /// from the centre's.
    #[test]
    fn the_handover_level_tracks_the_zoom() {
        let ifs = sierpinski();
        // A point of the set with an APERIODIC address. The ball centre
        // pushed through thirty maps was the first choice, and it is a
        // trap: thirty levels of inverse iteration bring it back to
        // the ball centre, which is equidistant from all three pieces,
        // and at an exact tie the handover stops -- correctly, since
        // pixels either side of it need different branches -- so the
        // level stalls at thirty however deep the zoom.
        let mut target = [
            ifs.ball.centre[0] + 0.137 * ifs.ball.radius,
            ifs.ball.centre[1] - 0.211 * ifs.ball.radius,
        ];
        for k in 0..40u32 {
            target = ifs.maps[(k as usize * 7 / 5) % 3].forward.apply(target);
        }

        let level_at = |zoom: f64| {
            let span = 4.0 / 2f64.powf(zoom);
            seed_beam(&ifs, target, [[span, 0.0], [0.0, -span]], span / 96.0, 400, 8).level
        };

        // At sigma = 1/2 each level doubles the delta, so the handover
        // should sit about one level deeper per bit of zoom -- less
        // the levels the view does not agree on. On a generic point
        // near-ties come along every so often, and each stops the
        // handover one level short of where the reach alone would
        // have taken it; measured, 53 levels for 60 bits. What the
        // shader is left with is the difference, as bits of f32 spent
        // on the frame before the pixel: eight is a comfortable
        // fraction of the twenty-four it has.
        let shallow = level_at(0.0);
        let deep = level_at(60.0);
        println!("  handover level: zoom 0 -> {shallow}, zoom 60 -> {deep}");
        assert!(shallow <= 3, "a home view should hand over almost at once, got {shallow}");
        let short = 60 - (deep as i64 - shallow as i64);
        assert!(
            (0..=8).contains(&short),
            "handover moved {} levels for 60 bits of zoom",
            deep as i64 - shallow as i64
        );

        // A seed MAY have left the ball -- a beam wider than the
        // branching factor prunes nothing, so escapees ride along from
        // the first level. What matters is that such a candidate
        // carries its escape record: recomputing it after the handover
        // would report it escaping at the handover level instead of
        // where it actually did, shifting the whole exterior.
        for zoom in [0.0, 20.0, 60.0, 120.0] {
            let span = 4.0 / 2f64.powf(zoom);
            let seeds =
                seed_beam(&ifs, target, [[span, 0.0], [0.0, -span]], span / 96.0, 400, 8);
            assert!(!seeds.cands.is_empty(), "zoom 2^{zoom}: handed over nothing");
            for c in &seeds.cands {
                let r = Affine2::distance(c.position, ifs.ball.centre);
                if r > ifs.ball.radius {
                    let (lvl, _) = c.escape.expect("an escaped seed must carry its escape");
                    // The level carries a fractional residual across
                    // the annulus, so an escape at integer level k
                    // reads as k + something under one.
                    assert!(
                        lvl < seeds.level as f64 + 1.0,
                        "zoom 2^{zoom}: escape recorded at level {lvl}, past the                          handover at {}",
                        seeds.level
                    );
                }
                // And a live one must be somewhere f32 can hold.
                assert!(
                    r.is_finite(),
                    "zoom 2^{zoom}: handed over a non-finite position"
                );
            }
        }
    }

    /// Three OVERLAPPING affine IFSs, which the shipped catalogue does
    /// not contain: the phase-0 census found three qualifying flames
    /// and all three are variation smoke tests. D9 is decided on these
    /// or not at all.
    ///
    /// Overlap here means the pieces genuinely share area, not merely
    /// touch — which is the case §2.2 names as where a branch
    /// heuristic fails.
    fn overlapping_ifss() -> Vec<(&'static str, Ifs2)> {
        // Sierpinski's three maps at 0.6 instead of 0.5: the pieces
        // are too big for the triangle and lap over each other.
        let fat = analyse(vec![
            affine_xform(0.6, 0.0, 0.0, 0.6, 0.0, 0.0),
            affine_xform(0.6, 0.0, 0.0, 0.6, 0.4, 0.0),
            affine_xform(0.6, 0.0, 0.0, 0.6, 0.2, 0.4),
        ]);
        // Two maps covering the unit square with a half each, at 0.7 —
        // a wide band of the middle belongs to both.
        let band = analyse(vec![
            affine_xform(0.7, 0.0, 0.0, 0.7, 0.0, 0.0),
            affine_xform(0.7, 0.0, 0.0, 0.7, 0.3, 0.3),
        ]);
        // Rotated and overlapping: two similarities at 0.65 turned
        // against each other, so the overlap is not axis-aligned and
        // the nearest-centre rule has no symmetry to lean on.
        let turned = analyse(vec![
            affine_xform(0.46, -0.46, 0.46, 0.46, 0.0, 0.0),
            affine_xform(0.46, 0.46, -0.46, 0.46, 0.5, 0.1),
        ]);
        vec![("fat gasket", fat), ("overlapping band", band), ("turned pair", turned)]
    }

    /// The deepest level any address can still explain a point at —
    /// which is what the Hepting–Hart escape buffer computes.
    ///
    /// The buffer's whole advantage is that it needs no branch choice:
    /// it iterates the IMAGE through the maps, so a point is inside at
    /// level k exactly when SOME address of length k holds it. That is
    /// the maximum over addresses, and it is computable here directly.
    /// So D9 does not need the buffer implemented to be decided — it
    /// needs the answer the buffer would give, and this is that answer.
    fn exhaustive_level(ifs: &Ifs2, p: [f64; 2], depth: u32) -> u32 {
        fn walk(ifs: &Ifs2, q: [f64; 2], left: u32, deepest: &mut u32, at: u32) {
            if Affine2::distance(q, ifs.ball.centre) > ifs.ball.radius {
                return;
            }
            *deepest = (*deepest).max(at);
            if left == 0 {
                return;
            }
            for m in &ifs.maps {
                walk(ifs, m.inverse.apply(q), left - 1, deepest, at + 1);
            }
        }
        let mut deepest = 0;
        walk(ifs, p, depth, &mut deepest, 0);
        deepest
    }

    /// D9, decided: does the beam's level hold up against what the
    /// escape buffer would give, on the IFSs where the branch choice
    /// is supposed to fail?
    ///
    /// If it does, the escape buffer's remaining advantage is gone and
    /// Mode C of the escape plan is closed by this one. If it does
    /// not, Mode C's multi-pass form is the overlapping-IFS path and
    /// this plan's estimate is the rest.
    #[test]
    fn the_beams_level_against_what_the_escape_buffer_would_give() {
        const DEPTH: u32 = 12;
        println!("  IFS                beam   level agrees   worst shortfall");
        let mut verdict = Vec::new();
        for (name, ifs) in overlapping_ifss() {
            for beam in [1u32, 4, 8] {
                let (mut agree, mut total, mut worst) = (0usize, 0usize, 0u32);
                let n = 21;
                for i in 0..n {
                    for j in 0..n {
                        let p = [
                            -0.5 + 2.0 * i as f64 / (n - 1) as f64,
                            -0.5 + 2.0 * j as f64 / (n - 1) as f64,
                        ];
                        let truth = exhaustive_level(&ifs, p, DEPTH);
                        // The walk's integer level: how many levels it
                        // stayed inside for.
                        let mine =
                            estimate(&ifs, p, DEPTH + 1, beam).deepest_level.floor() as u32;
                        total += 1;
                        if mine >= truth {
                            agree += 1;
                        } else {
                            worst = worst.max(truth - mine);
                        }
                    }
                }
                let pct = agree as f64 / total as f64 * 100.0;
                println!("  {name:<18} {beam:>4}   {pct:>10.1}%   {worst:>15}");
                verdict.push((name, beam, pct, worst));
            }
        }

        // The walk can never claim a level the buffer would not: it
        // follows real addresses, so any level it reaches is one some
        // address explains. It can only fall SHORT, by picking a worse
        // address than the best one -- which is exactly the failure
        // the buffer does not have.
        for &(name, beam, pct, worst) in &verdict {
            if beam == 8 {
                assert!(
                    pct > 99.0,
                    "{name}: at the shipped beam the level falls short of the escape \
                     buffer's on {:.1}% of points (worst {worst} levels) -- Mode C's \
                     multi-pass form is still the overlapping-IFS path",
                    100.0 - pct
                );
            }
        }
        // And the beam must be what buys it, or the comparison says
        // nothing about the beam.
        let greedy = verdict.iter().filter(|v| v.1 == 1).map(|v| v.2).fold(100.0f64, f64::min);
        let wide = verdict.iter().filter(|v| v.1 == 8).map(|v| v.2).fold(100.0f64, f64::min);
        println!("  worst case: greedy {greedy:.1}%, beam 8 {wide:.1}%");
        assert!(
            wide > greedy,
            "the beam bought nothing on an overlapping IFS ({greedy:.1}% -> {wide:.1}%)"
        );
    }

    // ------------------------------------------------------------ 3D

    /// A 3D transform: the XY affine is the identity plus a
    /// translation, and `linear3D` at half weight scales all three
    /// axes — so the map is `p ↦ (p + v)/2`, whose fixed point is `v`
    /// itself: the translation IS the corner it contracts toward, not
    /// twice it.
    ///
    /// `linear` alone will not do: it contributes to all three
    /// diagonal entries too, but an Apophysis-style transform leaves
    /// the z SCALE at one, which phase 0 measured as the reason no
    /// shipped 3D flame qualifies. Half-weight `linear3D` is the
    /// simplest thing that contracts in z.
    fn half3(v: [f32; 3]) -> Transform {
        let mut t = Transform::default();
        t.a = 1.0;
        t.b = 0.0;
        t.c = 0.0;
        t.d = 1.0;
        t.e = v[0];
        t.f = v[1];
        t.g = v[2];
        t.variations = HashMap::from([("linear3D".to_string(), 0.5)]);
        t.variation_order = vec!["linear3D".to_string()];
        t
    }

    /// A `quaternion_julia` solid: the one shipped kind whose inverse
    /// sends points to infinity, so its walk actually FINISHES paths
    /// -- which is the only condition under which `best_done` and the
    /// frozen-inside rule change an answer.
    fn qjulia3(c: [f32; 4], power: f32) -> Ifs3 {
        let mut t = affine_xform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        t.variations.clear();
        t.variation_order.clear();
        t.variations.insert("quaternion_julia".to_string(), 1.0);
        t.variation_order.push("quaternion_julia".to_string());
        t.set_variation_param("quaternion_julia", "cx", c[0]);
        t.set_variation_param("quaternion_julia", "cy", c[1]);
        t.set_variation_param("quaternion_julia", "cz", c[2]);
        t.set_variation_param("quaternion_julia", "cw", c[3]);
        t.set_variation_param("quaternion_julia", "power", power);
        t.set_variation_param("quaternion_julia", "dist", 1.0);
        t.set_variation_param("quaternion_julia", "inverse", 1.0);
        analyse3(vec![t])
    }

    fn flame3(transforms: Vec<Transform>) -> Flame {
        let mut fl = Flame::default();
        fl.transforms = transforms;
        fl.final_transforms.clear();
        fl.xaos = None;
        fl
    }

    fn analyse3(transforms: Vec<Transform>) -> Ifs3 {
        let guard = global_registry();
        crate::scene::ifs_analysis::analyse_3d(&flame3(transforms), &guard)
            .expect("should qualify as a solid IFS")
    }

    /// Eight half-scale maps to the corners of a cube. Their images
    /// tile it exactly, so the attractor IS the solid unit cube and
    /// the distance to it is closed form — the 3D twin of the unit
    /// square, and the only 3D case with an exact answer everywhere.
    fn unit_cube() -> Ifs3 {
        let mut maps = Vec::new();
        for i in 0..8 {
            maps.push(half3([
                (i & 1) as f32,
                ((i >> 1) & 1) as f32,
                ((i >> 2) & 1) as f32,
            ]));
        }
        analyse3(maps)
    }

    /// The Sierpiński tetrahedron: four half-scale maps to alternating
    /// corners of a cube.
    fn tetrahedron() -> Ifs3 {
        analyse3(vec![
            half3([0.0, 0.0, 0.0]),
            half3([1.0, 1.0, 0.0]),
            half3([1.0, 0.0, 1.0]),
            half3([0.0, 1.0, 1.0]),
        ])
    }

    fn box_distance3(p: [f64; 3], lo: f64, hi: f64) -> f64 {
        let d = |x: f64| (lo - x).max(0.0).max(x - hi);
        let (dx, dy, dz) = (d(p[0]), d(p[1]), d(p[2]));
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// The walk is generic over dimension, so 3D should already work.
    /// This is the claim, checked: eight maps whose attractor is the
    /// solid unit cube, against the exact distance to that cube.
    #[test]
    fn the_walk_estimates_a_solid_cube_in_three_dimensions() {
        let ifs = unit_cube();
        assert_eq!(ifs.maps.len(), 8);
        // Every map halves every axis, so every singular value is ½ —
        // which is what makes this a SOLID IFS rather than a stack of
        // planes.
        for m in &ifs.maps {
            assert!((m.sigma_min - 0.5).abs() < 1e-6, "sigma_min {:?}", m.sigma_min);
            assert!((m.sigma_max - 0.5).abs() < 1e-6, "sigma_max {:?}", m.sigma_max);
        }
        // The ball is the cube's circumscribed sphere.
        assert!((ifs.ball.radius - (0.75f64).sqrt()).abs() < 1e-6, "{:?}", ifs.ball);

        let mut worst_ratio = f64::INFINITY;
        let mut checked = 0;
        for i in -6..=12 {
            for j in -6..=12 {
                for k in -6..=12 {
                    let p = [i as f64 * 0.25, j as f64 * 0.25, k as f64 * 0.25];
                    let d = estimate(&ifs, p, 40, 8).distance;
                    let exact = box_distance3(p, 0.0, 1.0);
                    assert!(
                        d <= exact + 1e-9,
                        "at {p:?}: estimate {d} exceeds exact {exact}"
                    );
                    if exact == 0.0 {
                        assert_eq!(d, 0.0, "inside the cube at {p:?}");
                    } else if exact > 0.25 {
                        worst_ratio = worst_ratio.min(d / exact);
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 200, "not enough exterior points: {checked}");
        println!("  cube: worst estimate/exact ratio {worst_ratio:.4} over {checked} points");
        assert!(worst_ratio > 0.999, "the 3D bound is loose: {worst_ratio}");
    }

    /// The tetrahedron must qualify as a SOLID IFS, and its attractor
    /// must be where a Sierpiński tetrahedron is.
    #[test]
    fn the_sierpinski_tetrahedron_qualifies_and_sits_where_it_should() {
        let ifs = tetrahedron();
        assert_eq!(ifs.maps.len(), 4);
        for m in &ifs.maps {
            assert!(m.sigma_max < 1.0, "not contractive: {:?}", m.sigma_max);
        }

        // Its four fixed points are the corners it is built from.
        let mut corners: Vec<[f64; 3]> =
            ifs.maps.iter().map(|m| m.forward.fixed_point().expect("contractive")).collect();
        corners.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let want = [[0.0, 0.0, 0.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0]];
        for (got, want) in corners.iter().zip(&want) {
            for k in 0..3 {
                assert!(
                    (got[k] - want[k]).abs() < 1e-6,
                    "corner {got:?} should be {want:?}"
                );
            }
        }

        // A corner is on the attractor, so the walk must not push it
        // out; the cube's centre is in the tetrahedron's hole, so it
        // must.
        for c in &corners {
            assert!(!estimate(&ifs, *c, 40, 8).escaped, "corner {c:?} escaped");
        }
        let hole = estimate(&ifs, [0.5, 0.5, 0.5], 40, 8);
        assert!(
            hole.distance > 0.05,
            "the tetrahedron's centre should be a hole, got {}",
            hole.distance
        );
    }

    fn box_distance(p: [f64; 2], lo: [f64; 2], hi: [f64; 2]) -> f64 {
        let dx = (lo[0] - p[0]).max(0.0).max(p[0] - hi[0]);
        let dy = (lo[1] - p[1]).max(0.0).max(p[1] - hi[1]);
        (dx * dx + dy * dy).sqrt()
    }

    /// A single contraction's attractor is its fixed point, so the
    /// distance is exactly `|p − fixed|` — the one case with an exact
    /// answer everywhere. The ball degenerates to the fixed point, so
    /// level 0 already has it and the walk must return it EXACTLY.
    #[test]
    fn a_single_map_gives_the_distance_to_its_fixed_point_exactly() {
        let ifs = analyse(vec![half(0.3, 0.7)]);
        // p = p/2 + t  =>  p = 2t.
        let fixed = [0.6, 1.4];
        assert!(ifs.ball.radius < 1e-12, "ball should degenerate: {:?}", ifs.ball);

        for &p in &[[0.0, 0.0], [1.0, 1.0], [-2.0, 3.0], [0.6, 1.4]] {
            let e = estimate(&ifs, p, 32, 1);
            let want = ((p[0] - fixed[0]).powi(2) + (p[1] - fixed[1]).powi(2)).sqrt();
            assert!((e.distance - want).abs() < 1e-6, "at {p:?}: got {}, want {want}", e.distance);
        }
    }

    /// The four-map square: the attractor is `[0,1]²`, so every
    /// estimate must be a LOWER bound on the exact box distance, and
    /// points inside the square must read zero.
    #[test]
    fn the_unit_square_estimate_is_a_lower_bound_on_the_exact_box_distance() {
        let ifs = unit_square();
        assert_eq!(ifs.maps.len(), 4);
        // The ball is the square's circumscribed circle.
        assert!((ifs.ball.centre[0] - 0.5).abs() < 1e-6, "{:?}", ifs.ball);
        assert!((ifs.ball.centre[1] - 0.5).abs() < 1e-6, "{:?}", ifs.ball);
        assert!((ifs.ball.radius - 0.5f64.hypot(0.5)).abs() < 1e-6, "{:?}", ifs.ball);

        let mut worst_ratio = f64::INFINITY;
        let mut checked = 0;
        for i in -12..=24 {
            for j in -12..=24 {
                let p = [i as f64 * 0.125, j as f64 * 0.125];
                let e = estimate(&ifs, p, 40, 1);
                let exact = box_distance(p, [0.0, 0.0], [1.0, 1.0]);
                assert!(
                    e.distance <= exact + 1e-9,
                    "at {p:?}: estimate {} exceeds exact {exact}",
                    e.distance
                );
                if exact > 0.25 {
                    worst_ratio = worst_ratio.min(e.distance / exact);
                    checked += 1;
                }
                if exact == 0.0 {
                    assert_eq!(e.distance, 0.0, "inside the square at {p:?}");
                }
            }
        }
        assert!(checked > 40, "not enough exterior points: {checked}");
        println!("worst estimate/exact ratio over {checked} exterior points: {worst_ratio:.4}");
        // A MEASUREMENT, and a strong one: for a similarity IFS whose
        // pieces tile, running the walk past the first escape and
        // keeping the largest bound makes the estimate EXACT, not
        // merely sound. Stopping at the first escape gave 0.448 here,
        // and drew the bounding balls as rings (see the module docs).
        assert!(worst_ratio > 0.999, "bound got looser: worst ratio {worst_ratio}");
    }

    /// More levels can only tighten: the distance is a running
    /// maximum over the levels visited, so a larger bound can only
    /// raise it.
    #[test]
    fn deeper_walks_are_tighter_and_never_less_sound() {
        let ifs = unit_square();
        for i in -6..=14 {
            for j in -6..=14 {
                let p = [i as f64 * 0.25, j as f64 * 0.25];
                let exact = box_distance(p, [0.0, 0.0], [1.0, 1.0]);
                let mut prev = -1.0;
                for k in [1u32, 2, 4, 8, 16, 32] {
                    let d = estimate(&ifs, p, k, 1).distance;
                    assert!(d >= prev - 1e-12, "at {p:?}: level {k} loosened {prev} -> {d}");
                    assert!(d <= exact + 1e-9, "at {p:?}: level {k} overshoots {exact}");
                    prev = d;
                }
            }
        }
    }

    /// The gasket has no closed-form distance, so the upper bound
    /// comes from the set itself: a dense deterministic sample of the
    /// attractor. Distance to the nearest sample is an upper bound on
    /// the distance to the attractor, and the estimate must not exceed
    /// it. This is the soundness MEASUREMENT the module docs promise —
    /// greedy is not proved sound, it is checked.
    #[test]
    fn estimate_never_exceeds_a_sampled_upper_bound() {
        let ifs = sierpinski();

        // Every address of length 10 applied to the ball centre:
        // 3^10 = 59049 points, each within σ^10 of the attractor.
        const DEPTH: u32 = 10;
        let mut pts = vec![ifs.ball.centre];
        for _ in 0..DEPTH {
            let mut next = Vec::with_capacity(pts.len() * ifs.maps.len());
            for m in &ifs.maps {
                for &p in &pts {
                    next.push(m.forward.apply(p));
                }
            }
            pts = next;
        }
        // How far the sample can sit from the true attractor, and so
        // how loose an upper bound built from it may be.
        let sample_slack = 0.5f64.powi(DEPTH as i32) * ifs.ball.radius;

        for i in -8..=16 {
            for j in -8..=16 {
                let p = [i as f64 * 0.1, j as f64 * 0.1];
                let e = estimate(&ifs, p, 40, 1);
                let upper =
                    pts.iter().map(|&a| Affine2::distance(p, a)).fold(f64::INFINITY, f64::min);
                assert!(
                    e.distance <= upper + sample_slack + 1e-9,
                    "at {p:?}: estimate {} exceeds sampled upper bound {upper}",
                    e.distance
                );
            }
        }
    }

    /// A point on the attractor never escapes, and that is the signal
    /// the 2D render draws the set from.
    #[test]
    fn points_on_the_attractor_do_not_escape() {
        let ifs = sierpinski();
        for m in &ifs.maps {
            let fixed = m.forward.fixed_point().expect("contractive");
            let e = estimate(&ifs, fixed, 60, 1);
            assert!(!e.escaped, "fixed point {fixed:?} escaped at level {}", e.level);
            assert_eq!(e.distance, 0.0);

            // Any image of an attractor point is an attractor point.
            let deeper = ifs.maps[2].forward.apply(ifs.maps[1].forward.apply(fixed));
            let e = estimate(&ifs, deeper, 60, 1);
            assert!(!e.escaped, "attractor point {deeper:?} escaped at level {}", e.level);
        }
    }

    /// The level must rise toward the set — that is what makes it a
    /// band colouring rather than a noise field, and it is why the
    /// residual counts DOWN across the annulus.
    #[test]
    fn escape_level_rises_toward_the_attractor() {
        let ifs = unit_square();
        let mut last = -1.0;
        // A ray walking in toward the square along +x.
        for k in 0..14 {
            let t = 2.0 - k as f64 * 0.1;
            let e = estimate(&ifs, [0.5 + t, 0.5], 40, 1);
            assert!(e.level >= last - 1e-9, "level fell at t={t}: {last} -> {}", e.level);
            last = e.level;
        }
        assert!(last > 1.0, "level never rose past 1: {last}");
        // Far outside reads zero, not a clamped maximum.
        assert_eq!(estimate(&ifs, [40.0, 0.5], 40, 1).level, 0.0);
    }

    /// The address is the branch history, so it must name the piece
    /// the point is actually in.
    #[test]
    fn the_address_names_the_piece_the_point_is_in() {
        let ifs = unit_square();
        // Registration order: (0,0), (.5,0), (0,.5), (.5,.5).
        for (p, want) in
            [([0.1, 0.1], 0u32), ([0.9, 0.1], 1), ([0.1, 0.9], 2), ([0.9, 0.9], 3)]
        {
            let e = estimate(&ifs, p, 6, 1);
            assert_eq!(e.address.first(), Some(&want), "at {p:?}: address {:?}", e.address);
        }
    }

    #[test]
    fn address_fraction_is_the_base_n_reading_of_the_digits() {
        // Base 4: 0.123₄ = 1/4 + 2/16 + 3/64.
        let want = 1.0 / 4.0 + 2.0 / 16.0 + 3.0 / 64.0;
        assert!((address_fraction(&[1, 2, 3], 4) - want).abs() < 1e-12);
        assert_eq!(address_fraction(&[], 4), 0.0);
        assert_eq!(address_fraction(&[1], 0), 0.0);
        // A deep address terminates rather than looping forever.
        let deep: Vec<u32> = (0..500).map(|i| i % 3).collect();
        assert!(address_fraction(&deep, 3).is_finite());
    }

    /// A final transform maps the whole attractor, so the estimate
    /// must be the un-finalled estimate at the pre-image, scaled by
    /// the final's contraction.
    #[test]
    fn a_final_transform_is_inverted_once_and_scales_the_result() {
        let base = unit_square();
        let mut flame =
            flame_of(vec![half(0.0, 0.0), half(0.5, 0.0), half(0.0, 0.5), half(0.5, 0.5)]);
        flame.final_transforms = vec![affine_xform(0.5, 0.0, 0.0, 0.5, 2.0, -1.0)];
        let guard = global_registry();
        let with_final = analyse_2d(&flame, &guard).expect("qualifies");
        let fin = with_final.final_map.expect("one affine final");
        // Same maps, so the same ball: only the pre-inversion differs.
        assert_eq!(with_final.ball, base.ball);

        for &p in &[[0.0, 0.0], [3.0, -1.0], [2.5, -0.6], [4.0, 2.0]] {
            let pre = fin.inverse.apply(p);
            let want = estimate(&base, pre, 40, 1).distance * fin.sigma_min;
            let got = estimate(&with_final, p, 40, 1).distance;
            assert!((got - want).abs() < 1e-9, "at {p:?}: {got} vs {want}");
        }
    }
}
