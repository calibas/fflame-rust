//! What a variation tells the FORWARD analyses about how far it can
//! stretch a ball.
//!
//! The sibling of [`inverse`](super::inverse), and the piece both
//! plans named and neither built: [flame-deep-zoom.md](../../docs/projects/flame-deep-zoom.md)
//! §7 item 1, [ifs-general.md](../../docs/projects/ifs-general.md) §8's
//! second lever. Cylinder targeting enumerates words by pushing a ball
//! through the flame's maps and asking which images still reach the
//! viewport. For an affine map that push is exact and free — the
//! centre moves by the map, the radius scales by σ_max. For anything
//! else there is nothing to push with, which is why
//! [`Cylinders::plan`](crate::scene::cylinder::Cylinders::plan) turns
//! every nonlinear flame away.
//!
//! # The contract, and which way it must err
//!
//! A bound answers: given a disc `B` in the variation's input, name a
//! disc that CONTAINS its image.
//!
//! **It must over-estimate, never under-estimate**, and the asymmetry
//! is the whole safety argument. A bound that is too large admits
//! words whose image does not really reach the view; their samples
//! land outside the frame and are simply not seen, so the estimator
//! stays exact and only the efficiency suffers. A bound that is too
//! SMALL drops a word that does reach the view, and that word's share
//! of the measure vanishes from the picture — a silent, permanent
//! error in the render, not a slow one.
//!
//! So every bound here is derived, the derivation is written down
//! beside it, and `bounds_contain_the_real_wgsl` checks the claim
//! against the shipped shader on the GPU.
//!
//! # Randomness is not an obstacle
//!
//! Three of the first five bounds are for maps that draw a random
//! number per sample, and the inverse walks refuse all of them for
//! exactly that reason ([ifs-general.md](../../docs/projects/ifs-general.md)
//! §8: `juliascope`, `rays` and `boarders` are stochastic maps and so
//! are the blurs). A FORWARD bound does not care. `julian` picks one
//! of `|power|` angular branches, but every branch has the same
//! modulus, so a disc about the origin holds all of them at once; a
//! blur adds an offset of bounded length, so the image is the input
//! disc grown by that length. The forward direction reaches a class
//! of variation the inverse direction never will.

use super::inverse::ParamFn;

/// A disc in the plane: the currency of the forward enumeration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ball {
    pub c: [f64; 2],
    pub r: f64,
}

impl Ball {
    pub fn new(c: [f64; 2], r: f64) -> Self {
        Self { c, r }
    }

    /// Distance from the origin to the nearest point of the disc, and
    /// to the farthest. The two numbers almost every radial bound is
    /// written in terms of.
    pub fn radial_span(&self) -> (f64, f64) {
        let d = (self.c[0] * self.c[0] + self.c[1] * self.c[1]).sqrt();
        ((d - self.r).max(0.0), d + self.r)
    }

    /// Whether `p` is inside, with a relative slack for the f32 the
    /// shader computes in against the f64 the bound is derived in.
    pub fn contains(&self, p: [f64; 2], slack: f64) -> bool {
        let dx = p[0] - self.c[0];
        let dy = p[1] - self.c[1];
        (dx * dx + dy * dy).sqrt() <= self.r * (1.0 + slack) + slack
    }
}

/// A variation's forward bound: the disc its contribution lands in,
/// or `None` where no sound one exists at these parameters and this
/// input.
///
/// The weight is an argument rather than applied by the caller
/// because it does not enter uniformly. A `Normal`-phase variation is
/// summed as `w · V(p)`, so the bound is of the scaled map; a `Pre` or
/// `Post` one is applied by the dispatcher with no weight of its own,
/// and several of those (`pre_blur`) read the weight INSIDE the body.
/// Returning the contribution as it actually lands keeps that
/// distinction in the one place that knows it.
pub type BoundFn = fn(ParamFn, f64, Ball) -> Option<Ball>;

/// One variation's forward bound.
pub struct BoundDef {
    /// The variation this bounds, spelled as its
    /// [`VariationDef::name`](super::definition::VariationDef::name).
    pub name: &'static str,
    /// The planar bound. Solid is not here yet: cylinder targeting is
    /// a 2D enumeration, and a 3D one needs the view frustum rather
    /// than a disc.
    pub planar: BoundFn,
}

/// Every registered forward bound. **Append-only**, like [`INVERSES`](super::inverse::INVERSES)
/// and the variation list, and for the same reason: the gates iterate
/// it, so the order is what a failure report names.
pub static BOUNDS: &[&BoundDef] = &[
    &SPHERICAL_BOUND,
    &BUBBLE_BOUND,
    &JULIAN_BOUND,
    &PRE_BLUR_BOUND,
    &BLUR_BOUND,
];

/// The bound registered for `name`.
pub fn for_name(name: &str) -> Option<&'static BoundDef> {
    BOUNDS.iter().copied().find(|d| d.name == name)
}

// ===========================================================================
// spherical
// ===========================================================================

/// `p / (|p|² + 1e-6)` — the inversion, with the shader's guard.
///
/// # Derivation
///
/// Write `s = t² + ε` for `t = |p|` and `ε = 1e-6`. The Jacobian of
/// `g(p) = p/s` is `I/s − 2ppᵀ/s²`, whose eigenvalues are `1/s`
/// across the radius and `(ε − t²)/s²` along it. For `t ≥ √ε` the
/// radial one has magnitude `(t² − ε)/s² ≤ 1/s`, because
/// `t² − ε ≤ t² + ε`. So there `‖Dg‖ = 1/s`, which is decreasing in
/// `t`, and its supremum over the disc is at the disc's NEAREST point
/// to the origin.
///
/// A disc is convex, so the mean value inequality gives
/// `|g(x) − g(c)| ≤ sup‖Dg‖ · |x − c| ≤ L·r`. Centre the image on
/// `g(c)` with that radius.
///
/// Inside `√ε` of the origin the map turns over and the derivative
/// argument stops holding. Rather than refuse, fall back on the
/// global bound: `|g(p)| = t/(t²+ε)` is maximised at `t = √ε` and
/// equals `1/(2√ε)`, so the whole plane maps inside a disc of that
/// radius about the origin. Sound, useless for targeting, and useless
/// in the right way — the enumeration sees a word that never shrinks,
/// declines, and says so.
pub static SPHERICAL_BOUND: BoundDef = BoundDef {
    name: "spherical",
    planar: |_p, w, b| {
        const EPS: f64 = 1e-6;
        let (near, _) = b.radial_span();
        if near >= EPS.sqrt() {
            let s = near * near + EPS;
            let l = 1.0 / s;
            let cs = b.c[0] * b.c[0] + b.c[1] * b.c[1] + EPS;
            let centre = [w * b.c[0] / cs, w * b.c[1] / cs];
            Some(Ball::new(centre, w.abs() * l * b.r))
        } else {
            Some(Ball::new([0.0, 0.0], w.abs() / (2.0 * EPS.sqrt())))
        }
    },
};

// ===========================================================================
// bubble
// ===========================================================================

/// `p / (|p|²/4 + 1)` — bounded everywhere, and contractive away
/// from the origin.
///
/// # Derivation
///
/// Two bounds, and the tighter one wins.
///
/// **The global one.** `|V(p)| = t/(t²/4 + 1) = 4t/(t² + 4)` is
/// maximised at `t = 2` where it equals 1, so the image of ANY input
/// disc lies in the unit disc about the origin.
///
/// **The Lipschitz one**, which is the one the enumeration needs. For
/// `s = 1 + t²/4`, `Dg = I/s − ppᵀ/(2s²)`, with eigenvalues `1/s`
/// across the radius and `(2 − t²/2)/(2s²)` along it. Both are bounded
/// above by 1 and both fall off like `1/t²`, so a disc away from the
/// origin is genuinely contracted:
///
/// - the across-radius term is decreasing in `t`, so its supremum is
///   at the disc's nearest approach to the origin;
/// - the along-radius term, written in `v = 1 + t²/4`, is
///   `(v − 2)/v²` past `t = 2`, whose derivative `(4 − v)/v³`
///   vanishes at `v = 4`. So past `t = 2` it never exceeds **1/8**,
///   and below `t = 2` it is decreasing and again largest at the
///   nearest approach.
///
/// **Why both are kept.** The global bound alone is sound but useless
/// to a deep zoom: it claims the unit disc no matter how small the
/// input, so a word ending in `bubble` never shrinks, the enumeration
/// never reaches its stopping rule, and every word survives to the
/// depth cap. A bound can be perfectly correct and still make the
/// thing that consumes it useless — which is why the contract is a
/// disc rather than a yes/no, and why this one reports whichever of
/// the two discs is smaller.
pub static BUBBLE_BOUND: BoundDef = BoundDef {
    name: "bubble",
    planar: |_p, w, b| {
        let (near, far) = b.radial_span();
        let s = 1.0 + near * near / 4.0;
        // Across the radius: decreasing, so the nearest point decides.
        let mut l = 1.0 / s;
        // Along it: decreasing below t = 2 and never past 1/8 above.
        l = l.max((2.0 - near * near / 2.0).abs() / (2.0 * s * s));
        if far * far > 4.0 {
            l = l.max(0.125);
        }
        let sc = 1.0 + (b.c[0] * b.c[0] + b.c[1] * b.c[1]) / 4.0;
        let lip = Ball::new([w * b.c[0] / sc, w * b.c[1] / sc], w.abs() * l * b.r);
        let global = Ball::new([0.0, 0.0], w.abs());
        // Both are sound, so taking either is sound; take the one
        // that claims less.
        if lip.r < global.r {
            Some(lip)
        } else {
            Some(global)
        }
    },
};

// ===========================================================================
// julian
// ===========================================================================

/// `|p|^(dist/|power|)` at one of `|power|` random angles.
///
/// # Derivation
///
/// The body computes `r = pow(dot(p,p), cpower)` with
/// `cpower = dist/(2·|power|)`, so the output modulus is
/// `t^(dist/|power|)` — call the exponent `e`. The angle is
/// `(θ + 2πk)/power` for a `k` drawn uniformly per sample, so the
/// output direction is one of `|power|` values and the bound must
/// hold for all of them at once. A disc CENTRED ON THE ORIGIN does,
/// and its radius is the largest modulus the input disc can produce:
///
/// - `e > 0`: modulus grows with `t`, so the far edge, `t_far^e`;
/// - `e < 0`: modulus grows as `t` falls, so the near edge — and if
///   the disc reaches the origin the modulus is unbounded, which is
///   the one case with no sound answer;
/// - `e = 0`: every point maps to modulus 1.
///
/// This is the bound that shows randomness is not an obstacle to the
/// forward direction: the inverse walks refuse `julian`'s cousins for
/// drawing a branch per sample, and here the branch simply does not
/// enter.
pub static JULIAN_BOUND: BoundDef = BoundDef {
    name: "julian",
    planar: |p, w, b| {
        let power = p("power");
        let dist = p("dist");
        if power == 0.0 {
            // `cpower` divides by `max(|power|, 1e-30)`; at zero the
            // shader's exponent is 1e30 and the output is not a map
            // worth bounding.
            return None;
        }
        let e = dist / power.abs();
        let (near, far) = b.radial_span();
        let m = if e > 0.0 {
            far.powf(e)
        } else if e < 0.0 {
            if near <= 0.0 {
                return None;
            }
            near.powf(e)
        } else {
            1.0
        };
        if !m.is_finite() {
            return None;
        }
        Some(Ball::new([0.0, 0.0], w.abs() * m))
    },
};

// ===========================================================================
// the blurs
// ===========================================================================

/// `p + g·(cos a, sin a)` with `g = w·(ΣR₆ − 3)` and `a` uniform.
///
/// # Derivation
///
/// Six uniforms on `[0,1)` sum to at most 6, so `g ∈ [−3w, 3w]` and
/// the offset has length at most `3|w|` in an arbitrary direction.
/// The image of a disc is therefore that disc grown by `3|w|`.
///
/// `pre_blur` is a `Pre`-phase variation: the dispatcher applies no
/// weight of its own, and the body reads its own weight (the
/// `NeedsTransform` feature), so the weight is already inside `g` and
/// must not be applied again.
///
/// The bound is exact — `g` really does reach `±3w` — and it is a
/// bound on a map with no inverse at all. A blur destroys
/// information; it does not stop you knowing where the information
/// went.
pub static PRE_BLUR_BOUND: BoundDef = BoundDef {
    name: "pre_blur",
    planar: |_p, w, b| Some(Ball::new(b.c, b.r + 3.0 * w.abs())),
};

/// `blur`: a point drawn on a disc of radius `w`, ignoring the input
/// entirely.
///
/// # Derivation
///
/// The body is `w·R·(cos a, sin a)` for `R` and `a` uniform, so the
/// image is the disc of radius `|w|` about the origin — no dependence
/// on the input at all, which makes this the most contractive map in
/// the catalogue and the easiest bound in the file.
pub static BLUR_BOUND: BoundDef = BoundDef {
    name: "blur",
    planar: |_p, w, _b| Some(Ball::new([0.0, 0.0], w.abs())),
};

/// The gate that makes the bounds worth anything: every claim is
/// checked against the SHIPPED WGSL, on the GPU, at points drawn
/// inside the disc the claim is about.
#[cfg(test)]
mod gpu_tests {
    use super::*;
    use crate::probe::batch::{Batch, Target};
    use crate::probe::lens;
    use crate::scene::transforms::{Flame, Transform};

    /// Parameter settings to test each bound at. A bound whose name is
    /// absent is tested at the registry defaults alone.
    ///
    /// `julian` is here because its bound BRANCHES on the sign of
    /// `dist/|power|` — the far edge of the disc decides when the
    /// exponent is positive and the near edge when it is negative —
    /// and a defaults-only gate would exercise one arm of that and
    /// call the file covered.
    const PARAM_SETS: &[(&str, &[&[(&str, f64)]])] = &[(
        "julian",
        &[
            &[("power", 2.0), ("dist", 1.0)],
            &[("power", 5.0), ("dist", 2.0)],
            &[("power", 3.0), ("dist", -1.0)],
            &[("power", -4.0), ("dist", 0.5)],
        ],
    )];

    /// The discs every bound is asked about: over the origin, beside
    /// it, and far from it, at three scales.
    fn test_balls() -> Vec<Ball> {
        let mut out = Vec::new();
        for c in [[0.0, 0.0], [0.35, -0.2], [1.5, 0.0], [-3.0, 2.0]] {
            for r in [0.05, 0.3, 1.0] {
                out.push(Ball::new(c, r));
            }
        }
        out
    }

    /// Points spread over the disc — not a uniform sample, which
    /// concentrates in the rim, but rings including the centre and the
    /// boundary, because the boundary is where a Lipschitz claim is
    /// tightest and most likely to be wrong.
    fn points_in(b: &Ball, rings: usize, per_ring: usize) -> Vec<[f32; 2]> {
        let mut out = vec![[b.c[0] as f32, b.c[1] as f32]];
        for i in 1..=rings {
            let rr = b.r * i as f64 / rings as f64;
            for k in 0..per_ring {
                let a = std::f64::consts::TAU * k as f64 / per_ring as f64;
                out.push([(b.c[0] + rr * a.cos()) as f32, (b.c[1] + rr * a.sin()) as f32]);
            }
        }
        out
    }

    fn target_for(name: &str) -> Target {
        let reg = crate::variations::global_registry();
        let info = reg.get(name).expect("registered variation");
        Target {
            name: info.name.clone(),
            slots: info.slot_count(),
            needs_init: info.init_param_count > 0,
            phase: info.phase.clone(),
        }
    }

    /// One transform carrying the variation at weight one, with the
    /// parameters this case is about — the lens survey's own flame
    /// cannot express those, which is why `run_batch` takes a flame.
    fn flame_for(name: &str, target: &Target, params: &[(&str, f64)]) -> Flame {
        let mut flame = Flame::new();
        flame.transforms.clear();
        let mut xf = Transform::new();
        xf.a = 1.0;
        xf.b = 0.0;
        xf.e = 0.0;
        xf.c = 0.0;
        xf.d = 1.0;
        xf.f = 0.0;
        xf.g = 0.0;
        xf.weight = 1.0;
        if target.needs_carrier() {
            // A pre/post variation alone evaluates to the empty sum;
            // the carrier makes the normal phase a pass-through so the
            // transform's output IS the variation's.
            xf.set_variation(crate::probe::flame::CARRIER, 1.0);
        }
        xf.set_variation(name, 1.0);
        for (k, v) in params {
            xf.set_variation_param(name, k, *v as f32);
        }
        flame.transforms.push(xf);
        flame
    }

    /// Every registered forward bound contains what the shader
    /// actually computes.
    ///
    /// **The direction is the point.** A bound that is too generous
    /// costs efficiency; a bound that is too tight drops a word that
    /// did reach the viewport, and that word's share of the measure
    /// disappears from the render silently and forever. So this test
    /// only ever fails one way, and when it does it prints the point
    /// that escaped.
    ///
    /// Randomness is tested, not avoided: `julian`, `blur` and
    /// `pre_blur` draw per sample, so each of the several hundred
    /// evaluation points is an independent draw, and a bound that
    /// held only for the mean would not survive.
    #[test]
    #[ignore = "needs a GPU"]
    fn bounds_contain_the_real_wgsl() {
        let (device, queue) =
            pollster::block_on(lens::open_device_for_bounds()).expect("gpu");

        let mut violations: std::collections::BTreeMap<String, (usize, String)> = Default::default();
        let mut checked = 0usize;
        println!("  variation   params                  balls  points  worst fill");
        for def in BOUNDS {
            let target = target_for(def.name);
            let sets: &[&[(&str, f64)]] = PARAM_SETS
                .iter()
                .find(|(n, _)| *n == def.name)
                .map(|(_, s)| *s)
                .unwrap_or(&[&[]]);

            for params in sets {
                let pf = |n: &str| {
                    params
                        .iter()
                        .find(|(k, _)| *k == n)
                        .map(|(_, v)| *v)
                        .unwrap_or_else(|| {
                            let reg = crate::variations::global_registry();
                            reg.get(def.name)
                                .and_then(|i| {
                                    i.parameters.iter().find(|p| p.name == n).map(|p| p.default_value as f64)
                                })
                                .unwrap_or(0.0)
                        })
                };
                let flame = flame_for(def.name, &target, params);
                let batch = Batch { slots: target.slots, targets: vec![target.clone()] };

                let mut balls = 0usize;
                let mut pts = 0usize;
                let mut worst = 0.0f64;
                for ball in test_balls() {
                    let Some(claim) = (def.planar)(&pf, 1.0, ball) else {
                        continue;
                    };
                    let points = points_in(&ball, 6, 24);
                    let maps = lens::run_batch(&device, &queue, &batch, &flame, &points, 512)
                        .expect("lens dispatch");
                    let out = &maps[0].points;
                    for (i, o) in out.iter().enumerate() {
                        let o64 = [o[0] as f64, o[1] as f64];
                        if !o64[0].is_finite() || !o64[1].is_finite() {
                            // A non-finite output is the bad-value
                            // recovery's business, not the bound's:
                            // the render never plots it.
                            continue;
                        }
                        // f32 in the shader against f64 in the bound.
                        assert!(
                            claim.contains(o64, 1e-4),
                            "{}: input {ball:?} params {params:?} -- the shader sent \
                             {:?} to {o64:?}, which is OUTSIDE the claimed disc {claim:?}. \
                             A bound that under-estimates drops words the enumeration \
                             needed, and the measure they carry vanishes from the render.",
                            def.name,
                            points[i]
                        );
                        let d = ((o64[0] - claim.c[0]).powi(2) + (o64[1] - claim.c[1]).powi(2))
                            .sqrt();
                        worst = worst.max(if claim.r > 0.0 { d / claim.r } else { 0.0 });
                        pts += 1;
                    }
                    balls += 1;
                    checked += 1;
                }
                println!(
                    "  {:<11} {:<22}  {balls:>5}  {pts:>6}  {worst:>9.3}",
                    def.name,
                    format!("{params:?}"),
                );
            }
        }
        assert!(checked > 0, "the gate checked nothing");
    }
}

/// What shape are the variation bodies, for the purpose of deriving
/// a bound from them automatically?
///
/// The corpus meter says hand-derived bounds are a treadmill -- 14 of
/// them free 42% of the corpus and there are 55 distinct blockers --
/// so the question becomes whether a bound can be COMPUTED from the
/// shipped WGSL instead of written by hand. Interval arithmetic
/// through the body would do it soundly and for every variation at
/// once, and its cost is decided by how much of the language the
/// bodies actually use.
///
/// This counts that. Ignored; it is a survey, not a gate.
#[cfg(test)]
mod shape_survey {
    #[test]
    #[ignore = "a survey, not a gate"]
    fn how_much_wgsl_would_an_interval_evaluator_need() {
        use std::collections::BTreeMap;
        let mut straight = 0usize;
        let mut branchy = 0usize;
        let mut loopy = 0usize;
        let mut stateful = 0usize;
        let mut total = 0usize;
        let mut calls: BTreeMap<String, usize> = BTreeMap::new();

        for def in crate::variations::defs::ALL_VARIATIONS {
            let body = def.wgsl_2d;
            if body.trim().is_empty() {
                continue;
            }
            total += 1;
            let has_loop = body.contains("for (") || body.contains("while (");
            let has_if = body.contains("if (") || body.contains("select(");
            let has_state = body.contains("get_state")
                || body.contains("set_state")
                || body.contains("rng_next");
            if has_loop {
                loopy += 1;
            } else if has_if {
                branchy += 1;
            } else {
                straight += 1;
            }
            if has_state {
                stateful += 1;
            }
            // Every `name(` that is not a declaration.
            let mut i = 0;
            let b = body.as_bytes();
            while i < b.len() {
                if b[i] == b'(' {
                    let mut j = i;
                    while j > 0
                        && (b[j - 1].is_ascii_alphanumeric() || b[j - 1] == b'_')
                    {
                        j -= 1;
                    }
                    if j < i {
                        let f = &body[j..i];
                        if !matches!(f, "fn" | "if" | "for" | "while" | "return" | "let" | "var") {
                            *calls.entry(f.to_string()).or_default() += 1;
                        }
                    }
                }
                i += 1;
            }
        }

        println!();
        println!("  {total} variations with a 2D body:");
        println!("    {straight:>4}  straight-line arithmetic");
        println!("    {branchy:>4}  with a branch or select");
        println!("    {loopy:>4}  with a loop");
        println!("    {stateful:>4}  read rng or per-thread state (any of the above)");
        println!();
        println!("  intrinsics and helpers used, by how many bodies call them:");
        let mut rows: Vec<_> = calls.iter().collect();
        rows.sort_by_key(|(f, n)| (std::cmp::Reverse(**n), (*f).clone()));
        for (f, n) in rows.iter().take(40) {
            println!("    {n:>5}  {f}");
        }
        println!("    ({} distinct callees in all)", calls.len());
    }
}

// Desktop only, with the evaluator it exercises.
#[cfg(not(target_arch = "wasm32"))]
/// The gate that decides whether DERIVED bounds can be trusted:
/// every one of them, against the shipped shader, on the GPU.
#[cfg(test)]
mod derived_gate {
    use super::*;
    use crate::probe::batch::{Batch, Target};
    use crate::probe::lens;
    use crate::scene::transforms::{Flame, Transform};

    fn target_for(name: &str) -> Option<Target> {
        let reg = crate::variations::global_registry();
        let info = reg.get(name)?;
        Some(Target {
            name: info.name.clone(),
            slots: info.slot_count(),
            needs_init: info.init_param_count > 0,
            phase: info.phase.clone(),
        })
    }

    fn flame_for(targets: &[Target]) -> Flame {
        let mut flame = Flame::new();
        flame.transforms.clear();
        for t in targets {
            let mut xf = Transform::new();
            xf.a = 1.0;
            xf.b = 0.0;
            xf.e = 0.0;
            xf.c = 0.0;
            xf.d = 1.0;
            xf.f = 0.0;
            xf.g = 0.0;
            xf.weight = 1.0;
            if t.needs_carrier() {
                xf.set_variation(crate::probe::flame::CARRIER, 1.0);
            }
            xf.set_variation(&t.name, 1.0);
            flame.transforms.push(xf);
        }
        flame
    }

    fn points_in(b: &Ball, rings: usize, per_ring: usize) -> Vec<[f32; 2]> {
        let mut out = vec![[b.c[0] as f32, b.c[1] as f32]];
        for i in 1..=rings {
            let rr = b.r * i as f64 / rings as f64;
            for k in 0..per_ring {
                let a = std::f64::consts::TAU * k as f64 / per_ring as f64;
                out.push([(b.c[0] + rr * a.cos()) as f32, (b.c[1] + rr * a.sin()) as f32]);
            }
        }
        out
    }

    fn default_params(name: &str) -> impl Fn(&str) -> f64 + '_ {
        move |p: &str| {
            let reg = crate::variations::global_registry();
            reg.get(name)
                .and_then(|i| {
                    i.parameters.iter().find(|q| q.name == p).map(|q| q.default_value as f64)
                })
                .unwrap_or(0.0)
        }
    }

    /// **Every derived bound contains what the shader actually
    /// computes.**
    ///
    /// The whole case for deriving bounds rests here. An interval rule
    /// that is subtly wrong produces a disc that is too SMALL, the
    /// enumeration drops a word that did reach the viewport, and the
    /// measure it carried vanishes from the render silently — the one
    /// failure the contract exists to prevent. Sampling cannot prove a
    /// bound sound, but it is what catches a transcribed rule, and it
    /// checks against the shipped WGSL rather than a second opinion
    /// about it.
    ///
    /// Batched by disc rather than by variation: the lens evaluates a
    /// batch of targets at one set of points, so a dozen discs over
    /// every derivable body is a few dozen dispatches.
    #[test]
    #[ignore = "needs a GPU"]
    fn derived_bounds_contain_the_real_wgsl() {
        let (device, queue) = lens::open_device_for_bounds_blocking();

        // Which bodies derive at all, at the discs below.
        let discs: Vec<Ball> = {
            let mut v = Vec::new();
            for c in [[0.0, 0.0], [0.35, -0.2], [1.5, 0.0], [-3.0, 2.0]] {
                for r in [0.05, 0.3, 1.0] {
                    v.push(Ball::new(c, r));
                }
            }
            v
        };

        let names: Vec<String> = {
            let reg = crate::variations::global_registry();
            reg.ordered_names.clone()
        };

        let mut checked = 0usize;
        let mut bodies = std::collections::BTreeSet::new();
        let mut worst = 0.0f64;
        let mut worst_name = String::new();

        for disc in &discs {
            // Everything that derives AT THIS DISC.
            let mut targets: Vec<Target> = Vec::new();
            let mut claims: Vec<Ball> = Vec::new();
            for name in &names {
                let pf = default_params(name);
                let Ok(claim) = crate::variations::derive::derive(name, &pf, 1.0, *disc) else {
                    continue;
                };
                if !claim.r.is_finite() || claim.r > 1.0e12 {
                    continue;
                }
                let Some(t) = target_for(name) else { continue };
                targets.push(t);
                claims.push(claim);
            }
            if targets.is_empty() {
                continue;
            }

            let points = points_in(disc, 5, 16);
            for (chunk, cl) in targets.chunks(24).zip(claims.chunks(24)) {
                let batch = Batch {
                    slots: chunk.iter().map(|t| t.slots).sum(),
                    targets: chunk.to_vec(),
                };
                let flame = flame_for(chunk);
                let maps = match lens::run_batch(&device, &queue, &batch, &flame, &points, 512) {
                    Ok(m) => m,
                    Err(e) => panic!("lens dispatch failed: {e}"),
                };
                for (map, claim) in maps.iter().zip(cl) {
                    bodies.insert(map.name.clone());
                    for (i, o) in map.points.iter().enumerate() {
                        let o64 = [o[0] as f64, o[1] as f64];
                        if !o64[0].is_finite() || !o64[1].is_finite() {
                            // Bad-value recovery discards these; a
                            // bound says nothing about them.
                            continue;
                        }
                        assert!(
                            claim.contains(o64, 1e-3),
                            "`{}` at input {disc:?}: the shader sent {:?} to {o64:?}, OUTSIDE                              the derived disc {claim:?}. A derived bound that under-estimates                              drops words the enumeration needed, and the measure they carry                              never reaches the render.",
                            map.name,
                            points[i]
                        );
                        let d = ((o64[0] - claim.c[0]).powi(2) + (o64[1] - claim.c[1]).powi(2))
                            .sqrt();
                        let fill = if claim.r > 0.0 { d / claim.r } else { 0.0 };
                        if fill > worst {
                            worst = fill;
                            worst_name = map.name.clone();
                        }
                        checked += 1;
                    }
                }
            }
        }

        println!();
        println!("  {} bodies derived a bound and were checked", bodies.len());
        println!("  {checked} shader evaluations, all inside their disc");
        println!("  tightest observed: `{worst_name}` filled {worst:.3} of its radius");
        assert!(bodies.len() > 100, "only {} bodies checked", bodies.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params<'a>(pairs: &'a [(&'a str, f64)]) -> impl Fn(&str) -> f64 + 'a {
        move |n: &str| pairs.iter().find(|(k, _)| *k == n).map(|(_, v)| *v).unwrap_or(0.0)
    }

    /// Every registered bound names a variation that exists, and no
    /// name is registered twice.
    #[test]
    fn every_bound_names_a_real_variation() {
        let reg = crate::variations::global_registry();
        let mut seen = std::collections::HashSet::new();
        for d in BOUNDS {
            assert!(
                reg.get(d.name).is_some(),
                "`{}` has a forward bound but is not a registered variation",
                d.name
            );
            assert!(seen.insert(d.name), "`{}` is registered twice in BOUNDS", d.name);
        }
    }

    /// Two sound discs need not nest, and the enumeration does not
    /// need them to.
    ///
    /// This test replaced one that asserted the opposite — that
    /// growing the input disc never shrinks the claim — which looked
    /// like the contract and was not. `bubble` reports whichever of
    /// its two derivations claims less, so a larger input can cross
    /// over from the Lipschitz disc to the global one and come back
    /// with a SMALLER reach. Both discs contain the image, so both
    /// are correct.
    ///
    /// The pruning that consumes these bounds is still sound, and for
    /// a reason that has nothing to do with nesting: a child's TRUE
    /// image is a subset of its parent's true image, which is inside
    /// the parent's claim. So if the parent's claim misses the view,
    /// the child's image misses it too, whatever disc the child's own
    /// bound would have named.
    ///
    /// What every bound must do is contain the image, and only the
    /// GPU gate can check that.
    #[test]
    fn a_claim_is_finite_and_non_negative() {
        let p = params(&[("power", 2.0), ("dist", 1.0)]);
        for d in BOUNDS {
            for centre in [[0.0, 0.0], [1.0, 0.0], [3.0, -2.0]] {
                for r in [0.0f64, 0.01, 0.2, 1.0, 50.0] {
                    let Some(out) = (d.planar)(&p, 1.0, Ball::new(centre, r)) else {
                        continue;
                    };
                    assert!(
                        out.r >= 0.0 && out.r.is_finite(),
                        "{}: claimed radius {} at input radius {r}",
                        d.name,
                        out.r
                    );
                    assert!(
                        out.c[0].is_finite() && out.c[1].is_finite(),
                        "{}: claimed centre {:?} at input radius {r}",
                        d.name,
                        out.c
                    );
                }
            }
        }
    }

    /// `bubble` never claims more than the unit disc, and on a small
    /// disc away from the origin it claims far less — which is the
    /// difference between a bound the enumeration can use and one it
    /// cannot.
    #[test]
    fn bubble_is_bounded_and_contracts_away_from_the_origin() {
        let p = params(&[]);
        for b in [
            Ball::new([0.0, 0.0], 0.1),
            Ball::new([1e6, 1e6], 1e6),
            Ball::new([0.0, 0.0], 1e30),
        ] {
            let out = (BUBBLE_BOUND.planar)(&p, 1.0, b).expect("bubble is always bounded");
            assert!(out.r <= 1.0, "bubble's image is inside the unit disc, got {}", out.r);
        }

        // The case the enumeration lives on: a small disc far from
        // the origin must come back SMALLER than it went in, or a
        // word ending in `bubble` never reaches the stopping rule.
        let small = Ball::new([3.0, 0.0], 1e-3);
        let out = (BUBBLE_BOUND.planar)(&p, 1.0, small).unwrap();
        assert!(
            out.r < small.r,
            "bubble must contract a small distant disc, got {} from {}",
            out.r,
            small.r
        );
    }

    /// `julian`'s sign cases, which is where its bound can be wrong.
    #[test]
    fn julian_follows_the_exponent() {
        // e = dist/|power| = +0.5: the modulus grows with the input,
        // so the far edge decides.
        let p = params(&[("power", 2.0), ("dist", 1.0)]);
        let out = (JULIAN_BOUND.planar)(&p, 1.0, Ball::new([3.0, 0.0], 1.0)).unwrap();
        assert!((out.r - 4.0f64.powf(0.5)).abs() < 1e-12, "far edge, got {}", out.r);

        // e = −0.5: the NEAR edge decides, and the origin is fatal.
        let n = params(&[("power", 2.0), ("dist", -1.0)]);
        let out = (JULIAN_BOUND.planar)(&n, 1.0, Ball::new([3.0, 0.0], 1.0)).unwrap();
        assert!((out.r - 2.0f64.powf(-0.5)).abs() < 1e-12, "near edge, got {}", out.r);
        assert!(
            (JULIAN_BOUND.planar)(&n, 1.0, Ball::new([0.5, 0.0], 1.0)).is_none(),
            "a negative exponent over the origin is unbounded and must refuse"
        );

        // A zero power is not a map worth bounding.
        let z = params(&[("power", 0.0), ("dist", 1.0)]);
        assert!((JULIAN_BOUND.planar)(&z, 1.0, Ball::new([1.0, 0.0], 0.1)).is_none());
    }

    /// `spherical` away from the origin is the inversion's `1/t²`,
    /// and over the origin it falls back to the global disc rather
    /// than refusing.
    #[test]
    fn spherical_is_the_inverse_square_away_from_the_origin() {
        let p = params(&[]);
        let out = (SPHERICAL_BOUND.planar)(&p, 1.0, Ball::new([2.0, 0.0], 0.5)).unwrap();
        // near = 1.5, L = 1/(1.5² + 1e-6), radius = L · 0.5
        let want = 0.5 / (2.25 + 1e-6);
        assert!((out.r - want).abs() < 1e-12, "got {}, want {want}", out.r);

        let over = (SPHERICAL_BOUND.planar)(&p, 1.0, Ball::new([0.0, 0.0], 1.0)).unwrap();
        assert!(
            over.r >= 499.0 && over.c == [0.0, 0.0],
            "over the origin the bound is the global disc, got {over:?}"
        );
    }

    /// A blur grows its input by the offset's reach and nothing more.
    #[test]
    fn a_blur_grows_the_disc_by_its_reach() {
        let p = params(&[]);
        let out = (PRE_BLUR_BOUND.planar)(&p, 0.5, Ball::new([1.0, 2.0], 0.25)).unwrap();
        assert_eq!(out.c, [1.0, 2.0]);
        assert!((out.r - (0.25 + 1.5)).abs() < 1e-12, "got {}", out.r);
    }
}
