//! Pushing a disc through a whole transform.
//!
//! [`bound`](crate::variations::bound) says what ONE variation does to
//! a disc. This composes those answers into the thing the enumeration
//! actually asks for: given a disc in the plane, where does this
//! transform send it?
//!
//! # The composition
//!
//! A flame transform evaluates in four stages, and each has its own
//! rule for discs:
//!
//! 1. **the affine** `a..f` — exact. The centre moves by the map and
//!    the radius scales by `σ_max`, because that is the definition of
//!    the largest singular value;
//! 2. **the pre-phase variations**, applied in order, each replacing
//!    the point. Chain the discs;
//! 3. **the normal-phase sum** `Σ w_v V_v(q)`. If each `w_v V_v` lands
//!    in `B(m_v, ρ_v)` then the sum lands in `B(Σ m_v, Σ ρ_v)` — the
//!    triangle inequality, and the reason the per-variation contract
//!    returns the CONTRIBUTION rather than the bare map;
//! 4. **the post-phase variations**, then the post-affine — chain and
//!    apply as above.
//!
//! Every step over-estimates or is exact, and an over-estimate of an
//! over-estimate is still an over-estimate, so the composition
//! inherits the per-variation contract's direction. That is what lets
//! the enumeration trust the result.
//!
//! # What it refuses
//!
//! A variation that is neither affine nor bounded, and a bound that
//! declines at these parameters. Both name the variation, because the
//! panel's job is to say which one — "not available" without a name
//! is not an answer anyone can act on.

use crate::scene::ifs_analysis::{affine_role, Affine2, AffineRole, Space};
use crate::scene::transforms::Transform;
use crate::variations::bound::{self, Ball};
use crate::variations::{VariationPhase, VariationRegistry};

/// Why a transform cannot push a disc.
#[derive(Debug, Clone, PartialEq)]
pub enum NoBall {
    /// The variation is neither affine nor bounded: nothing in the
    /// registry knows what it does to a disc.
    Unbounded(String),
    /// The variation HAS a bound, but it declines at these parameters
    /// over this disc — `julian` with a negative exponent over the
    /// origin, where the image really is unbounded.
    Declines(String),
    /// The variation is forced out of its usual phase by a priority
    /// override, which this composition does not model.
    Priority(String),
}

impl std::fmt::Display for NoBall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unbounded(n) => write!(f, "`{n}` has no forward bound"),
            Self::Declines(n) => write!(f, "`{n}` has no bound at these parameters"),
            Self::Priority(n) => write!(f, "`{n}` is moved out of its phase by a priority"),
        }
    }
}

/// A disc through an exact affine: centre by the map, radius by
/// `σ_max`.
pub fn affine_ball(a: &Affine2, b: Ball) -> Ball {
    let (_, smax) = a.singular_values();
    Ball::new(
        [
            a.m[0][0] * b.c[0] + a.m[0][1] * b.c[1] + a.t[0],
            a.m[1][0] * b.c[0] + a.m[1][1] * b.c[1] + a.t[1],
        ],
        smax * b.r,
    )
}

/// The planar 2×2 of an [`AffineRole`]'s 3D map, which is what acts
/// in a 2D flame — the z column multiplies a coordinate the chaos
/// game does not have.
fn planar(a: &crate::scene::ifs_analysis::Affine3) -> Affine2 {
    Affine2 {
        m: [[a.m[0][0], a.m[0][1]], [a.m[1][0], a.m[1][1]]],
        t: [a.t[0], a.t[1]],
    }
}

/// Where this transform sends `input`, as a disc containing the
/// image.
///
/// The result is an over-estimate by construction; see the module
/// docs for why that direction is the safe one.
pub fn transform_ball_2d(
    t: &Transform,
    registry: &VariationRegistry,
    input: Ball,
) -> Result<Ball, NoBall> {
    Bounder::new(t, registry)?.apply(input)
}

/// **One transform's bound, with the naming work done once.**
///
/// `transform_ball_2d` used to redo, on every disc it was handed, the
/// part of the job that depends only on the TRANSFORM: resolving the
/// active variation names in dispatch order and splitting them by
/// phase. `ordered_variation_names` walks all 647 registered names
/// cloning Strings, and measured at 12.2 us of a 13 us call — about
/// ninety per cent of every bound evaluation, spent rediscovering
/// something that had not changed.
///
/// That was invisible while the enumeration refused most flames
/// early. Once derived bounds let them through to the expansion loop,
/// `plan` started calling this thousands of times per pan, and a
/// deep view cost tens of milliseconds of pure name lookup.
///
/// Build one per transform, apply it to as many discs as you like.
pub struct Bounder<'a> {
    t: &'a Transform,
    registry: &'a VariationRegistry,
    affine: Affine2,
    /// `(name, weight)` per phase, in dispatch order.
    pre: Vec<(String, f64)>,
    normal: Vec<(String, f64)>,
    post: Vec<(String, f64)>,
    post_affine: Option<Affine2>,
}

impl<'a> Bounder<'a> {
    pub fn new(t: &'a Transform, registry: &'a VariationRegistry) -> Result<Self, NoBall> {
        let order = t.ordered_variation_names(registry);

        // Split the active variations by the phase the dispatcher
        // runs them in. `Any` is normal unless a priority moves it,
        // which is the same rule the shader builder and
        // `variation_stage` use.
        let mut pre = Vec::new();
        let mut normal = Vec::new();
        let mut post = Vec::new();
        for name in &order {
            let w = t.variations.get(name).copied().unwrap_or(0.0) as f64;
            if w == 0.0 {
                continue;
            }
            let phase = registry.get(name).map(|i| i.phase.clone());
            let moved = t.variation_priorities.get(name).copied().unwrap_or(0) != 0;
            match phase {
                Some(VariationPhase::Pre) => pre.push((name.clone(), w)),
                Some(VariationPhase::Post) => post.push((name.clone(), w)),
                Some(VariationPhase::Any) if moved => return Err(NoBall::Priority(name.clone())),
                _ => normal.push((name.clone(), w)),
            }
        }

        Ok(Self {
            t,
            registry,
            affine: Affine2 {
                m: [[t.a as f64, t.b as f64], [t.c as f64, t.d as f64]],
                t: [t.e as f64, t.f as f64],
            },
            pre,
            normal,
            post,
            post_affine: t.post_affine_enabled.then(|| Affine2 {
                m: [
                    [t.post_a as f64, t.post_b as f64],
                    [t.post_c as f64, t.post_d as f64],
                ],
                t: [t.post_e as f64, t.post_f as f64],
            }),
        })
    }

    /// Where this transform sends `input`, as a disc containing the
    /// image.
    pub fn apply(&self, input: Ball) -> Result<Ball, NoBall> {
        // Stage 1: the transform's own affine.
        let mut b = affine_ball(&self.affine, input);

        // Stage 2: the pre-phase chain. No weight is applied by the
        // dispatcher here; a body that wants its own reads it itself.
        for (name, w) in &self.pre {
            b = one(name, *w, self.t, self.registry, b, Chain::Replace)?;
        }

        // Stage 3: the normal-phase sum. Centres add, radii add.
        let mut sum = Ball::new([0.0, 0.0], 0.0);
        for (name, w) in &self.normal {
            let part = one(name, *w, self.t, self.registry, b, Chain::Sum)?;
            sum.c[0] += part.c[0];
            sum.c[1] += part.c[1];
            sum.r += part.r;
        }
        b = sum;

        // Stage 4: the post-phase chain, then the post-affine.
        for (name, w) in &self.post {
            b = one(name, *w, self.t, self.registry, b, Chain::Replace)?;
        }
        if let Some(pa) = &self.post_affine {
            b = affine_ball(pa, b);
        }
        Ok(b)
    }
}

/// Whether this variation's answer replaces the disc or is summed
/// into one. Only affects how an AFFINE role is read: a `Sum` role
/// carries the weight already, a `Pre`/`Post` role does not.
#[derive(Clone, Copy, PartialEq)]
enum Chain {
    Replace,
    Sum,
}

/// One variation's contribution, from whichever registry knows it:
/// the affine roles first, because they are exact, and the forward
/// bounds after.
fn one(
    name: &str,
    w: f64,
    t: &Transform,
    registry: &VariationRegistry,
    b: Ball,
    chain: Chain,
) -> Result<Ball, NoBall> {
    if let Some(role) = affine_role(name, w, t, registry, Space::Planar) {
        return Ok(match role {
            // Nothing in the plane: a z-only map or a 2D stub. In the
            // sum it contributes no disc at all; in a chain it leaves
            // the point alone.
            AffineRole::Nothing => match chain {
                Chain::Sum => Ball::new([0.0, 0.0], 0.0),
                Chain::Replace => b,
            },
            AffineRole::Sum(a) | AffineRole::Pre(a) | AffineRole::Post(a) => {
                affine_ball(&planar(&a), b)
            }
        });
    }
    let pf = |p: &str| t.get_variation_param_or_default(name, p, registry) as f64;

    // A bound written by hand first: there are five, they are tighter
    // than anything derived, and they are the reference the evaluator
    // is checked against.
    //
    // **A hand bound that DECLINES falls through rather than ending
    // the search.** Each of the five answers only where its
    // derivation holds -- `julian`'s wants a disc clear of the
    // origin, because the map's Lipschitz constant blows up there --
    // and returning `Declines` from here treated "my proof does not
    // cover this disc" as "nothing can bound this", which is a
    // different claim. Two hand-picked zoom flames were refused with
    // "`julian` has no bound at these parameters" while the evaluator
    // sitting right below would have answered for most of the discs
    // the enumeration actually asks about.
    if let Some(def) = bound::for_name(name) {
        if let Some(out) = (def.planar)(&pf, w, b) {
            return Ok(out);
        }
    }

    // Otherwise derive one from the variation's shipped WGSL
    // (`docs/projects/forward-bounds.md`).
    //
    // **The weight is applied here, not there.** A derived bound is
    // of the body alone -- `V(p)` -- because that is what the WGSL
    // computes. The dispatcher multiplies a `Normal`-phase
    // variation's result by the weight before summing it, so the
    // caller has to; a `Pre` or `Post` one is applied with no weight
    // of its own, and the few that want theirs read it inside the
    // body where the evaluator already resolves it.
    #[cfg(not(target_arch = "wasm32"))]
    {
        // **Retry once on a finer grid before giving up.**
        //
        // Interval arithmetic's error grows with the width of its
        // input, so a refusal can be the body's real behaviour or
        // just the dependency problem -- `elliptic` divides by half
        // the sum of the distances to (±1, 0), which is at least 1 by
        // the ellipse property and which intervals read as possibly
        // zero because they cannot see the two square roots are
        // linked. Splitting the input tells the two apart: `elliptic`
        // comes back at k=3, and `curl` and `rays`, whose poles are
        // real, refuse at every k.
        //
        // Only on refusal, so the common path pays nothing, and once,
        // because the measurement says a finer grid than this buys
        // almost nothing (k=8 recovered three more bodies of 647 for
        // sixty-four times the work).
        let derived = crate::variations::derive::derive(name, &pf, w, b)
            .or_else(|_| crate::variations::derive::derive_subdivided(name, &pf, w, b, 3));
        return match derived {
            Ok(out) => Ok(match chain {
                Chain::Sum => Ball::new(
                    [out.c[0] * w, out.c[1] * w],
                    out.r * w.abs(),
                ),
                Chain::Replace => out,
            }),
            Err(why) => Err(NoBall::Unbounded(format!("{name} ({why})"))),
        };
    }
    // The web build has no WGSL front end to parse with, so the hand
    // bounds are all it has -- and there a decline really is the end
    // of the search. See `variations::mod`.
    #[cfg(target_arch = "wasm32")]
    Err(if bound::for_name(name).is_some() {
        NoBall::Declines(name.to_string())
    } else {
        NoBall::Unbounded(name.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;


    fn linear_xf() -> Transform {
        let mut t = Transform::new();
        t.a = 0.5;
        t.b = 0.0;
        t.c = 0.0;
        t.d = 0.5;
        t.e = 0.25;
        t.f = 0.0;
        t.weight = 1.0;
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("linear", 1.0);
        t
    }

    /// An affine-only transform must push a disc EXACTLY — the same
    /// answer `transform_affine_2d` gives, since nothing here is an
    /// estimate.
    #[test]
    fn an_affine_transform_is_exact() {
        let r = crate::variations::global_registry();
        let t = linear_xf();
        let b = transform_ball_2d(&t, &r, Ball::new([1.0, 2.0], 0.4)).unwrap();
        // centre = 0.5·(1,2) + (0.25, 0) ; radius = 0.5·0.4
        assert!((b.c[0] - 0.75).abs() < 1e-12, "{b:?}");
        assert!((b.c[1] - 1.0).abs() < 1e-12, "{b:?}");
        assert!((b.r - 0.2).abs() < 1e-12, "{b:?}");
    }

    /// A bounded variation makes an otherwise-unreachable transform
    /// answerable — the point of the whole file.
    #[test]
    fn a_bounded_variation_is_no_longer_a_refusal() {
        let r = crate::variations::global_registry();
        let mut t = linear_xf();
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("bubble", 1.0);

        // The affine analysis cannot take it...
        assert!(crate::scene::ifs_analysis::transform_affine_2d(&t, &r).is_err());
        // ...and the ball analysis can, with a disc that is SMALLER
        // than the one it was handed -- which is what the enumeration
        // needs, and what the crude "always the unit disc" form of
        // this bound could never give.
        //
        // Note the claim is not required to sit inside the unit disc
        // even though the image does: `bubble` reports whichever of
        // its two sound derivations has the smaller RADIUS, and the
        // Lipschitz one can be tighter while reaching further out.
        let input = Ball::new([5.0, 5.0], 3.0);
        let b = transform_ball_2d(&t, &r, input).unwrap();
        assert!(b.r.is_finite() && b.r > 0.0, "{b:?}");
        assert!(b.r < input.r, "the claim must contract: {b:?} from {input:?}");
    }

    /// Radii add across the sum, so two bounded variations at weight
    /// one reach twice as far as either.
    #[test]
    fn the_sum_adds_radii() {
        let r = crate::variations::global_registry();
        let mut t = linear_xf();
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("bubble", 1.0);
        t.set_variation("blur", 1.0);
        let input = Ball::new([0.0, 0.0], 0.5);
        let b = transform_ball_2d(&t, &r, input).unwrap();

        // Asserted against the two bounds THEMSELVES rather than a
        // hard-coded number, so this stays a test of the composition
        // rule -- radii add across the sum -- and does not silently
        // become a second copy of `bubble`'s arithmetic.
        let after_affine = affine_ball(
            &Affine2 { m: [[0.5, 0.0], [0.0, 0.5]], t: [0.25, 0.0] },
            input,
        );
        let pf = |_: &str| 0.0;
        let bub = (crate::variations::bound::for_name("bubble").unwrap().planar)(
            &pf, 1.0, after_affine,
        )
        .unwrap();
        let blr = (crate::variations::bound::for_name("blur").unwrap().planar)(
            &pf, 1.0, after_affine,
        )
        .unwrap();
        assert!((b.r - (bub.r + blr.r)).abs() < 1e-12, "{b:?} vs {bub:?} + {blr:?}");
    }

    /// A pre-phase blur grows the disc BEFORE the sum sees it, so it
    /// reaches the answer through the rest of the transform rather
    /// than being added at the end.
    #[test]
    fn a_pre_blur_grows_the_disc_before_the_sum() {
        let r = crate::variations::global_registry();
        let mut t = linear_xf();
        t.set_variation("pre_blur", 0.1);
        // affine: radius 0.4 -> 0.2 ; pre_blur: +0.3 ; linear: ×1
        //
        // 1e-6 rather than 1e-12: the weight is stored as f32, so 0.1
        // arrives as 0.100000001 and three times it is off in the
        // ninth place. That is the storage, not the composition.
        let b = transform_ball_2d(&t, &r, Ball::new([0.0, 0.0], 0.4)).unwrap();
        assert!((b.r - 0.5).abs() < 1e-6, "{b:?}");
    }

    /// A variation with no bound still refuses, and the refusal
    /// names both the variation and why.
    ///
    /// `waves` reads the per-transform variation weights directly, so
    /// neither a hand bound nor the WGSL evaluator can answer for it
    /// — and the targeting panel shows this string to the user, so
    /// "waves" alone would not tell them what to change.
    #[test]
    fn an_unbounded_variation_names_itself() {
        let r = crate::variations::global_registry();
        let mut t = linear_xf();
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("waves", 1.0);
        match transform_ball_2d(&t, &r, Ball::new([0.0, 0.0], 1.0)) {
            Err(NoBall::Unbounded(n)) => {
                assert!(n.starts_with("waves"), "{n}");
                assert!(n.contains("weights"), "{n}");
            }
            other => panic!("expected an unbounded refusal, got {other:?}"),
        }
    }

    /// A transform built only from bounded parts always answers, at
    /// every scale, with a finite disc.
    ///
    /// Deliberately NOT a monotonicity claim. An earlier version
    /// asserted that a bigger input disc never yields a smaller one,
    /// which is not something the per-variation contract promises and
    /// which `bubble` breaks by reporting whichever of its two sound
    /// derivations claims less. The enumeration does not need it
    /// either — see `bound::tests::a_claim_is_finite_and_non_negative`.
    #[test]
    fn a_bounded_transform_answers_at_every_scale() {
        let r = crate::variations::global_registry();
        let mut t = linear_xf();
        t.set_variation("spherical", 0.3);
        t.set_variation("pre_blur", 0.05);
        for rad in [0.0f64, 0.01, 0.05, 0.2, 0.6, 1.5, 40.0] {
            let b = transform_ball_2d(&t, &r, Ball::new([2.0, 0.0], rad)).unwrap();
            assert!(b.r.is_finite() && b.r >= 0.0, "radius {rad} gave {b:?}");
        }
    }
}
