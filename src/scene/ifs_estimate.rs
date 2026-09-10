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

use crate::scene::ifs_analysis::{Affine2, Affine3, Ifs, IfsMap};

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
    let best = live
        .into_iter()
        .min_by(|a, b| a.bound.partial_cmp(&b.bound).unwrap_or(std::cmp::Ordering::Equal))
        .expect("the beam is never empty");

    let distance = if best.bound.is_finite() { best.bound.max(0.0) } else { 0.0 };
    match best.escape {
        Some((level, address, point)) => Estimate { distance, level, address, point, escaped: true },
        None => Estimate {
            distance,
            level: max_levels as f64,
            address: best.address,
            point: best.q,
            escaped: false,
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
    use crate::scene::ifs_analysis::{analyse_2d, Ifs2};
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
