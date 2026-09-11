//! The IFS distance estimate, on the CPU
//! ([docs/projects/ifs-distance-rendering.md](../../docs/projects/ifs-distance-rendering.md)
//! §2.2, phase 1).
//!
//! Hart's inverse iteration: apply inverse maps to the query point,
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

use crate::scene::ifs_analysis::{Affine2, Affine3, Ifs, Ifs2, Ifs3, IfsMap};

/// What the walk needs of one map. Implemented for the affine cases
/// now; §8's ladder adds the nonlinear ones by making the inverse and
/// the scale functions of the point rather than constants.
pub trait IfsSpace: Copy {
    type Point: Copy + PartialEq + std::fmt::Debug;

    fn apply(&self, p: Self::Point) -> Self::Point;
    fn distance(a: Self::Point, b: Self::Point) -> f64;
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

impl IfsSpace for Affine3 {
    type Point = [f64; 3];

    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        Affine3::apply(self, p)
    }

    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
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
    /// position — **the ranking key**, and the whole of it.
    ///
    /// Ranking by `bound` instead is degenerate: it is a running
    /// MAXIMUM, so once a path has grazed the ball's edge its bound
    /// sits at ~0⁻ and every descendant inherits it unchanged — all
    /// the children tie and the choice becomes whichever the sort
    /// happened to leave first. Measured on the gasket, that is loose
    /// at 568 of 576 grid points against 10 this way. Inside the ball
    /// it is the distance to the centre that says which piece the
    /// point is in.
    ///
    /// Adding `bound` as a tiebreak changes no measurement at all, so
    /// it is not here: one f32 is what the shader has to shift around
    /// its keep-list, and that is its inner loop.
    r: f64,
    address: Vec<u32>,
    escape: Option<(f64, Vec<u32>, P)>,
    /// Past [`FAR`]: converged, and no longer expanded.
    done: bool,
}

/// Ascending by position: deepest inside the ball first.
fn by_rank<P>(a: &Cand<P>, b: &Cand<P>) -> std::cmp::Ordering {
    a.r.partial_cmp(&b.r).unwrap_or(std::cmp::Ordering::Equal)
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

    let mut live = vec![Cand {
        q: q0,
        sigma: sigma0,
        bound: f64::NEG_INFINITY,
        r: A::distance(q0, centre),
        address: Vec::new(),
        escape: None,
        done: false,
    }];

    for k in 0..max_levels {
        let mut all_done = true;
        for c in live.iter_mut() {
            if c.done {
                continue;
            }
            let r = A::distance(c.q, centre);
            c.r = r;
            // Every level's value is a lower bound; the walk keeps the
            // largest. Stopping at the first escape is what draws the
            // bounding ball as though it were the set — see the module
            // docs.
            c.bound = c.bound.max(c.sigma * (r - radius));

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
                next.push(c.clone());
                continue;
            }
            for (i, m) in ifs.maps.iter().enumerate() {
                let q = m.inverse.apply(c.q);
                let sigma = c.sigma * m.sigma_min;
                let r = A::distance(q, centre);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = c.bound.max(sigma * (r - radius));
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
            }
        }
        next.sort_by(by_rank);
        next.truncate(beam);
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
        .fold(f64::NEG_INFINITY, f64::max);
    let best = live
        .into_iter()
        .min_by(|a, b| a.bound.partial_cmp(&b.bound).unwrap_or(std::cmp::Ordering::Equal))
        .expect("the beam is never empty");

    let distance = if best.bound.is_finite() { best.bound.max(0.0) } else { 0.0 };
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
pub trait SeedPoint: Clone {
    /// Apply an affine map whose coefficients are f64.
    fn apply_affine(&self, a: &Affine2) -> Self;
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
}

/// How large a delta may grow before the CPU stops and hands over.
///
/// The handover is sound only while every pixel in the view still
/// follows the centre — while no pixel's branch choice can differ from
/// the reference's. A quarter of the ball's radius is well inside
/// that, and close enough to O(1) for f32 to take over.
pub const HANDOVER_FRACTION: f64 = 0.25;

/// Walk the beam from the view centre and stop while the whole view
/// still behaves as one point.
///
/// `view_basis` maps a normalised pixel offset — the screen spanning
/// [-½, ½] on each axis — to a world offset from the centre. `px` is
/// the world width of one pixel.
pub fn seed_beam<P: SeedPoint>(
    ifs: &Ifs2,
    centre: P,
    view_basis: [[f64; 2]; 2],
    px: f64,
    max_levels: u32,
    beam: u32,
) -> Seeds {
    let ball = ifs.ball.centre;
    let radius = ifs.ball.radius;
    let beam = beam.max(1) as usize;
    let cap = radius * HANDOVER_FRACTION;
    let scale = if px > 0.0 { 1.0 / px } else { 1.0 };

    let (q0, sigma0, basis0) = match &ifs.final_map {
        Some(f) => (
            centre.apply_affine(&f.inverse),
            f.sigma_min,
            compose_basis(&f.inverse, view_basis),
        ),
        None => (centre, 1.0, view_basis),
    };

    let r0 = q0.distance_to(ball);
    let mut live = vec![Cand {
        q: q0,
        sigma: sigma0,
        bound: f64::NEG_INFINITY,
        r: r0,
        address: Vec::new(),
        escape: None,
        done: false,
    }];
    let mut bases = vec![basis0];
    let mut level = 0u32;

    let mean = mean_sigma_min(&ifs.maps);
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
        if all_done {
            break;
        }

        // Stop once a delta could start separating pixels onto
        // different branches. A candidate that has ALREADY left the
        // ball is not a reason to stop -- it is state, and it is
        // carried; stopping on it ends the prefix at level 1, because
        // a beam wider than the branching factor prunes nothing and so
        // keeps every escapee from the first level onward.
        if bases.iter().any(|b| basis_reach(*b) >= cap) {
            break;
        }

        let mut next: Vec<Cand<P>> = Vec::with_capacity(live.len() * ifs.maps.len());
        let mut next_bases: Vec<[[f64; 2]; 2]> = Vec::with_capacity(next.capacity());
        for (c, basis) in live.iter().zip(&bases) {
            if c.done {
                next.push(c.clone());
                next_bases.push(*basis);
                continue;
            }
            for (i, m) in ifs.maps.iter().enumerate() {
                let q = c.q.apply_affine(&m.inverse);
                let sigma = c.sigma * m.sigma_min;
                let r = q.distance_to(ball);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = c.bound.max(sigma * (r - radius));
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
                next_bases.push(compose_basis(&m.inverse, *basis));
            }
        }
        // The same ranking the walk uses — and the bases have to
        // follow their candidates through the sort, or every delta
        // ends up attached to the wrong path.
        let mut order: Vec<usize> = (0..next.len()).collect();
        order.sort_by(|&a, &b| by_rank(&next[a], &next[b]));
        order.truncate(beam);
        live = order.iter().map(|&i| next[i].clone()).collect();
        bases = order.iter().map(|&i| next_bases[i]).collect();
        level += 1;
    }

    Seeds {
        level,
        cands: live
            .into_iter()
            .zip(bases)
            .map(|(c, basis)| Seed {
                position: c.q.to_f64(),
                basis,
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
                escape: c.escape.map(|(lvl, _, p)| (lvl, p.to_f64())),
                done: c.done,
                address: c.address,
            })
            .collect(),
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

    let mut live: Vec<Cand<[f64; 2]>> = seeds
        .cands
        .iter()
        .map(|s| {
            let d = apply_basis(s.basis, uv);
            let q = [s.position[0] + d[0], s.position[1] + d[1]];
            Cand {
                q,
                // σ per pixel, so the bound comes out in pixels too.
                sigma: s.sigma_per_px,
                bound: s.bound_per_px,
                r: Affine2::distance(q, centre),
                address: s.address.clone(),
                escape: s.escape.map(|(lvl, p)| (lvl, s.address.clone(), p)),
                done: s.done,
            }
        })
        .collect();

    let mut best_escape: Option<(f64, Vec<u32>, [f64; 2])> = None;
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
            c.bound = c.bound.max(c.sigma * (r - radius));
            if r > radius && c.escape.is_none() {
                let level = (seeds.level + k) as f64
                    + escape_residual(r, radius, sigma_of(c));
                c.escape = Some((level, c.address.clone(), c.q));
            }
            if !r.is_finite() || r > far {
                c.done = true;
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
                next.push(c.clone());
                continue;
            }
            for (i, m) in ifs.maps.iter().enumerate() {
                let q = m.inverse.apply(c.q);
                let sigma = c.sigma * m.sigma_min;
                let r = Affine2::distance(q, centre);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = c.bound.max(sigma * (r - radius));
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
            }
        }
        next.sort_by(by_rank);
        next.truncate(beam);
        live = next;
    }

    let deepest_level = live
        .iter()
        .map(|c| {
            c.escape
                .as_ref()
                .map_or((seeds.level + max_levels) as f64, |(lvl, _, _)| *lvl)
        })
        .fold(f64::NEG_INFINITY, f64::max);
    let best = live
        .into_iter()
        .min_by(|a, b| a.bound.partial_cmp(&b.bound).unwrap_or(std::cmp::Ordering::Equal))
        .expect("the beam is never empty");
    let _ = &mut best_escape;
    let distance = if best.bound.is_finite() { best.bound.max(0.0) } else { 0.0 };
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
        Some(f) => (target.apply_affine3(&f.inverse), f.sigma_min, f.inverse.m),
        None => (target, 1.0, Affine3::IDENTITY.m),
    };

    let r0 = q0.distance_to(ball);
    let mut live = vec![Cand {
        q: q0,
        sigma: sigma0,
        bound: f64::NEG_INFINITY,
        r: r0,
        address: Vec::new(),
        escape: None,
        done: false,
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
        // deeper link.
        if mats.iter().any(|m| frobenius3(*m) * finest >= cap) {
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
            for (i, map) in ifs.maps.iter().enumerate() {
                let q = c.q.apply_affine3(&map.inverse);
                let sigma = c.sigma * map.sigma_min;
                let r = q.distance_to(ball);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = c.bound.max(sigma * (r - radius));
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
                next_mats.push(compose3(&map.inverse, *m));
            }
        }
        // The same ranking the walk uses — and the matrices have to
        // follow their candidates through the sort, or every delta
        // ends up attached to the wrong path.
        let mut order: Vec<usize> = (0..next.len()).collect();
        order.sort_by(|&a, &b| by_rank(&next[a], &next[b]));
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
            }
        })
        .collect();

    let sigma_of = |c: &Cand<[f64; 3]>| {
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
            let r = Affine3::distance(c.q, centre);
            c.r = r;
            c.bound = c.bound.max(c.sigma * (r - radius));
            if r > radius && c.escape.is_none() {
                let level = (j as u32 + k) as f64 + escape_residual(r, radius, sigma_of(c));
                c.escape = Some((level, c.address.clone(), c.q));
            }
            if !r.is_finite() || r > far {
                c.done = true;
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
                next.push(c.clone());
                continue;
            }
            for (i, m) in ifs.maps.iter().enumerate() {
                let q = m.inverse.apply(c.q);
                let sigma = c.sigma * m.sigma_min;
                let r = Affine3::distance(q, centre);
                let mut child = c.clone();
                child.q = q;
                child.sigma = sigma;
                child.bound = c.bound.max(sigma * (r - radius));
                child.r = r;
                child.address.push(i as u32);
                next.push(child);
            }
        }
        next.sort_by(by_rank);
        next.truncate(beam);
        live = next;
    }

    let total = j as u32 + max_levels;
    let deepest_level = live
        .iter()
        .map(|c| c.escape.as_ref().map_or(total as f64, |(lvl, _, _)| *lvl))
        .fold(f64::NEG_INFINITY, f64::max);
    let best = live
        .into_iter()
        .min_by(|a, b| a.bound.partial_cmp(&b.bound).unwrap_or(std::cmp::Ordering::Equal))
        .expect("the beam is never empty");
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

fn apply_basis(b: [[f64; 2]; 2], uv: [f64; 2]) -> [f64; 2] {
    [b[0][0] * uv[0] + b[0][1] * uv[1], b[1][0] * uv[0] + b[1][1] * uv[1]]
}

/// Compose an inverse map's LINEAR part onto a delta basis. The
/// translation is carried entirely by the reference, which is what
/// makes the split exact for affine maps.
fn compose_basis(inv: &Affine2, b: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    [
        [
            inv.m[0][0] * b[0][0] + inv.m[0][1] * b[1][0],
            inv.m[0][0] * b[0][1] + inv.m[0][1] * b[1][1],
        ],
        [
            inv.m[1][0] * b[0][0] + inv.m[1][1] * b[1][0],
            inv.m[1][0] * b[0][1] + inv.m[1][1] * b[1][1],
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
        let mut target = ifs.ball.centre;
        for k in 0..30u32 {
            target = ifs.maps[(k as usize) % 3].forward.apply(target);
        }

        let level_at = |zoom: f64| {
            let span = 4.0 / 2f64.powf(zoom);
            seed_beam(&ifs, target, [[span, 0.0], [0.0, -span]], span / 96.0, 400, 8).level
        };

        // At sigma = 1/2 each level doubles the delta, so the handover
        // should sit about one level deeper per bit of zoom.
        let shallow = level_at(0.0);
        let deep = level_at(60.0);
        println!("  handover level: zoom 0 -> {shallow}, zoom 60 -> {deep}");
        assert!(shallow <= 3, "a home view should hand over almost at once, got {shallow}");
        assert!(
            (deep as i64 - shallow as i64 - 60).abs() <= 4,
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
