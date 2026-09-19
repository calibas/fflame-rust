# Generalised IFS support: the math lives with the variation (plan, 2026-09-17)

The second of three plans written together; the first is
[ifs-perturbation-delta.md](ifs-perturbation-delta.md), whose §9
orders the work of all three, and the third is
[ifs-measure-by-inverse-walk.md](ifs-measure-by-inverse-walk.md).

**What is being asked for.** "A generalised system for constructing
any IFS fractal, re-using flame transforms and variations." A flame
already IS an IFS definition -- transforms, each a weighted sum of
variations around a pre- and post-affine, with xaos as the graph and
finals as a plot-time map -- and the chaos game renders every one of
them. What is not general is the ANALYSIS that turns a flame into
the maps the inverse walks need: today it accepts affine transforms
and six hand-coded kernels alone in their transform, and refuses the
rest by name. This plan makes the analysis reach the catalogue,
moves what each map has to supply into the variation that owns it,
and says precisely where the reach stops and why.

**What it buys.** Every flame the chaos game renders is either an
IFS the walks accept or a flame whose panel says which transform
blocks it and what would unblock it. The census
([variation-reachability-census.md](variation-reachability-census.md)
and [ifs-distance-rendering.md](ifs-distance-rendering.md) §5) is a
progress meter rather than a verdict.

**What it does not buy.** Not the folds and the blurs: a map with no
preimage or no point to invert has no place in an inverse walk, and
[ifs-distance-rendering.md](ifs-distance-rendering.md) §8.2 sorts
those out for good. Not a new editor: the flame editor is the
builder, and this plan adds no UI beyond the panel's reasons.

---

## 1. What is there

In [ifs_analysis.rs](../../src/scene/ifs_analysis.rs):

- `Kernel`, a closed enum of six planar kernels (`Root{n,d}`,
  `Spherical`, `Bubble`, `Hemisphere`, `Disc`, `Blob`) and
  `Kernel3` of three, each with a hand-derived inverse, Jacobian,
  local σ, singular distance, gap and hole rules, and for Bubble a
  cancellation-free radial scale written after the first form
  measured 102% wrong.
- `transform_map_2d_ordered`, a central `match` from variation NAME
  to kernel, reading the variation's parameters by name.
- `AFFINE_VARIATIONS` and the per-space role function of §8.7: the
  list of variations that are affine in the plane or in space.
- `Map2::hessian`, central differences of the Jacobian with a step
  clamped to `[1e-12, 1e-3]`, least accurate near the singularities
  where it matters most.
- The rules a flame fails by, in `Disqualification`: `NotAffine`
  (which includes `MixedSum`: a kernel summed with anything, plan
  §8.4's rule J4), `Singular`, `NotContractive` (σ_max ≥ 1),
  `Xaos`, `FinalNotAffine`, `MultipleFinals`, `NoBall`.
- The variation's own definition, `VariationDef`, carries forward
  WGSL, parameters and features, and nothing about an inverse.

Three of this month's bugs were in hand derivations: Bubble's σ_min
was 21× too large (a tangential derivative alone), Bubble's inverse
cancelled to 102% error, and `big_kernel_inverse` conjugated on
`n·d < 0` where the rule is `n < 0`. All three were in code that
exists three times -- f64, `BigFloat`, WGSL -- and the third was
found by a gate that compared two of the copies.

## 2. The idea

**One implementation per variation, generic over the number.** A
variation that supports the walks supplies its forward map, its
inverse (per branch), and later its difference form
([ifs-perturbation-delta.md](ifs-perturbation-delta.md) §3),
written ONCE over a scalar trait:

```rust
pub trait Real: Clone {
    fn from_f64(v: f64) -> Self;
    fn to_f64(&self) -> f64;
    fn add(&self, o: &Self) -> Self;  fn sub(..);  fn mul(..);  fn div(..);
    fn sqrt(&self) -> Self;
    fn hypot2(&self, o: &Self) -> Self;   // |(x, y)|², the form every kernel wants
}
pub trait Transcendental: Real { fn exp(..); fn ln(..); fn sin(..); fn cos(..); fn atan2(..); }
```

implemented by `f64`, by `BigFloat` (which has `sqrt`, `ln`,
`atan2` and lacks `exp`, `sin`, `cos` -- the rung-3 item of the
delta plan), and by a dual number `Dual<T>` carrying a value and a
two-component gradient, and `Dual<Dual<T>>` for second derivatives.
Then:

- the f64 inverse, the `BigFloat` inverse and the Jacobian are the
  same function called at three types, and the Hessian is exact
  rather than a finite difference;
- `local_sigma`, `singular_distance` and the gap rule become
  properties the variation states (the singular set as a closed form
  or a list of points and curves), with the Jacobian's singular
  values computed rather than bounded where a bound was all a hand
  derivation could give;
- the WGSL inverse is the only copy still written by hand, and it is
  gated against the generic one at thousands of points as the GPU
  gates do today.

**The registry holds it.** `VariationDef` gains an optional
`inverse: Option<&'static InverseDef>`, absent by default so the
other 640 definitions do not move and the append-only registration
order is untouched. The analysis asks the registry, and the central
`match` goes. The six kernels become the first six `InverseDef`s,
living in `defs/julia.rs`, `defs/spherical.rs` and so on; the
`Kernel` enum survives only as the GPU's kind code, which is what
`IfsMapGpu.kind` already is.

**What an `InverseDef` says:**

| field | meaning |
|---|---|
| `kind` | affine / conformal / radial / general -- §8.3's ladder, which decides how σ is computed |
| `branches` | how many preimages, and the rule that enumerates them (`Root`'s `n`, `Bubble`'s inner/outer, `Disc`'s rings) |
| `forward`, `inverse` | the generic functions, over `Real` or `Transcendental` |
| `difference` | the exact difference form, if the kernel has one (the delta plan's rung) |
| `singular` | the singular set: where the inverse's derivative fails, as a distance function `q ↦ s` |
| `support` | the image of the plane under the forward map -- the disc for Bubble and Hemisphere, the plane less a hole for an inverted root -- which is the gap rule |
| `wgsl_inverse`, `wgsl_difference` | the shader bodies, name-prefixed as forward bodies are |
| `precision` | which rung the `BigFloat` walk can take: rational, algebraic, transcendental |

## 3. Decisions

- ~~**D1. The math moves into the definitions.**~~ -- done
  2026-09-17, §9b and §9c. The six kernels
  become `InverseDef`s on their variations; `transform_map_2d_ordered`
  consults the registry; `AFFINE_VARIATIONS` becomes an `InverseDef`
  of kind affine on each of the dozen variations that carry it,
  with the per-space role folded in. The gate that decides whether
  a kernel is correct runs over EVERY registered `InverseDef`
  automatically (G1), so the seventh kernel gets the six's gates for
  free.
- **D2. Derivatives by dual numbers, never by hand and never by
  finite differences.** -- the f64 and `Dual` halves are done
  (§9a); `BigFloat` implements neither trait yet, which is the delta
  plan's item 8. The generic implementation is the only
  source of `J` and `H`. `Map2::hessian`'s central difference goes.
  The `BigFloat` implementations of `Real` and `Transcendental` are
  the same code path the delta plan needs for its rungs. Cost: the
  `Real` trait, three impls, and re-expressing the six kernels'
  inverses in it -- the inverses are short; the derivations they
  replace were the long part.
- ~~**D3. Sums by Newton, with the limits said.**~~ -- done
  2026-09-18, §9e, both sides. A transform whose
  normal phase sums several variations, or one kernel with an
  affine (`linear 0.5 + spherical 0.5` is among the commonest
  transforms in the census), has no closed-form inverse. It has a
  Jacobian -- from D2 on the CPU, and from the forward WGSL by
  central differences in f32 on the GPU, which Newton tolerates
  because its accuracy comes from the residual and not from `J`.
  So:
  - the inverse of `w·Σ v_j(A p)` at `q` is Newton from a seed,
    with the seed from the transform's dominant term's own inverse
    (the kernel's branch, or the affine's inverse when the affine
    dominates), three to six iterations, converged when the
    residual is under `1e-6·|q|` in f32 and to the type's precision
    in the generic form;
  - the branch count is the dominant term's, and each of its
    branches seeds one Newton solve; a sum whose dominant term has
    no `InverseDef` is refused as today, with the reason naming it;
  - **a Newton that fails is not a gap.** A gap is a proof that no
    preimage exists; a failed solve proves nothing. The lineage
    ends with the bound it had, which is a valid lower bound on the
    distance and so sound, and loose. The measure plan reads the
    same lineage as "unknown", which it treats as the coarse
    density's own value at the parent (§D5 there);
  - on the GPU this needs the flame's forward variation bodies
    spliced into the mode-D shader through the per-flame local
    index map, exactly as `shader_builder_v2` splices them into the
    chaos game. That is the one piece of real plumbing in this
    plan, and it is what §8.6 of the design doc reserved the room
    for.
- ~~**D4. Xaos is a graph-directed IFS, and the walk takes the
  graph.**~~ -- done 2026-09-18, §9d. A child at level k+1 by map `j` is admissible after a
  parent by map `i` only if `xaos[i][j] > 0`; the beam expands
  admissible children only; the address is the same address. The
  invariant ball becomes one ball per node in principle, and stays
  one ball for all in practice until a flame shows the difference.
  The measure plan needs the row-normalised transition
  probabilities beside the admissibility, and `xaos.rs` has them.
  Gate: a flame with an all-ones xaos matrix renders pixel-identical
  to the same flame without one.
- **D5. Finals, nonlinear and several.** A final is applied once at
  plot time and is not part of the dynamics, so its inverse is
  applied once at level 0: the pixel through `F⁻¹` before the walk,
  and in the delta walk the centre through `F⁻¹` in `BigFloat` with
  the delta through `J_{F⁻¹}` -- a level with a different map, and
  no more. Several finals: the chaos game picks one per plot, so
  the picture is the union of the attractor's images under each,
  and the distance is the minimum over finals of one walk each.
  `FinalNotAffine` and `MultipleFinals` go; a final with no
  `InverseDef` stays a reason.
- **D6. The ball is a bound, not an invariant.** `NotContractive`
  refuses any transform with σ_max ≥ 1, and `NoBall` refuses a set
  with no ball every map sends into itself. Neither is what the walk
  needs. The escape test needs only `B ⊇ A`: if `S_a⁻¹(x) ∉ B` then
  `x ∉ S_a(B) ⊇ S_a(A)`, whatever `S_a` does to `B`. The bound
  `σ·(r − R)` needs only `σ_min > 0` along the address, which
  invertibility gives. A flame that contracts on average -- which is
  what the chaos game's own convergence requires and what most
  artistic flames do, with one transform an isometry or an
  expansion -- has a bounded attractor and a bounding ball, and the
  sampled extent with a margin (the `Extent` machinery of §8.15)
  is a certificate for it where the invariant construction fails.
  So: the invariant ball where it exists, the sampled one with a
  stated margin where it does not, and `NotContractive` becomes a
  WARNING about the beam's ranking (an expanding map's inverse
  contracts, so its children rank near the ball's centre whether
  they are on the set or not), measured before it is trusted (G4).
- **D7. The census is the meter.** After each rung lands, the count
  of the 159 census flames and the shipped presets that qualify, by
  reason for those that do not, in the census doc's table. No rung
  is done until its row is measured.
- **D8. 3D is the same trait at `[T; 3]`.** -- half done
  2026-09-18, the delta plan's §3h. `kernel3_inverse_gen` is the
  three solid kernels over `Real`/`Transcendental`, bit-identical to
  the transcription it replaced, and `Kernel3::inverse` is it at f64
  -- so `Map3::jacobian` is a `Dual3` pushed through the same body
  and the solid prefix walks at `BigFloat`. What is NOT done is the
  registry half: `Kernel3` is still a taxonomy here rather than
  three `InverseDef`s with a solid body, and `Space` still does not
  pick which body a variation supplies.

## 4. What changes, and what does not

| piece | today | here |
|---|---|---|
| `VariationDef` | forward WGSL, params, features | plus `inverse: Option<&InverseDef>` |
| `Kernel` / `Kernel3` | the taxonomy | the GPU kind code only; the math in the defs |
| `transform_map_2d_ordered` | a match on names | a registry lookup; a sum becomes `Map2::Sum` with Newton |
| `Map2` | `Affine`, `Nonlinear`, `NonlinearInverse` | plus `Sum { terms, dominant, pre, post }` |
| `Map2::hessian` | finite differences | dual numbers |
| `big_kernel_inverse` | a `BigFloat` transcription per kernel | the generic inverse at `BigFloat` |
| `Disqualification` | seven reasons | `Xaos`, `FinalNotAffine`, `MultipleFinals`, `NotContractive` go; `NoInverse { variation }`, `NewtonSeed { variation }` arrive |
| mode-D shader | six kernel kinds in `IFS_TEMPLATE` | plus a spliced Newton step over the flame's own forward bodies, for `Sum` rows |
| the census | a verdict | a table with a row per rung |

The walk, the handover, the colourings and the presets do not
change; G0 of the delta plan (every shipped mode-D preset
pixel-identical) applies to every step here.

## 5. Gates

- **G1. Every `InverseDef` passes the kernel gates.** A registry
  test iterating all definitions with an inverse: forward∘inverse is
  the identity on every branch at thousands of points in the
  support; the dual-number Jacobian against central differences of
  the generic forward; the σ tie `σ_max(J_inverse)·σ_min(forward)
  = 1` at the kernel; the singular distance is real (half of it
  keeps the branch); the WGSL inverse against the f64 one on the
  GPU. These are the six kernels' gates today, run over the
  registry instead of a list.
- **G2. The moved kernels are the old kernels.** Every existing
  gate in `ifs_analysis.rs` and `ifs_estimate.rs` unchanged and
  green through D1 and D2, and the presets pixel-identical. The
  finite-difference Hessian against the dual one at the points the
  old gate used: agreement to the old gate's tolerance, and better
  near the singularities, measured.
- **G3. Newton is an inverse where it converges, and honest where
  it does not.** On `linear + spherical` at ten mixes, and on three
  census transforms chosen for being common: convergence rate and
  iteration count at random points of the support; the residual on
  convergence; and a walk on a two-transform set built from them
  against a chaos sample at 2^4: no pixel on the set reads as
  exterior. On the GPU, the same at 1e-5 relative.
- **G4. The bounding ball is sound where the invariant one does not
  exist.** A flame with one isometric transform: a chaos sample of
  200,000 points against the walk's distance at 2^4; every sampled
  point within a pixel of zero. And the beam's ranking measured on
  it at beams 1, 4, 8, 16 for the pruning artefacts D6 warns of.
- **G5. Xaos.** All-ones is identity (D4). A block-diagonal xaos on
  a four-transform set: the walk's address colouring shows only
  admissible addresses, checked against a chaos sample.
- **G6. Finals.** A flame with a spherical final against the same
  flame with the final folded into a chaos sample; two finals
  against the union of two samples.
- **G7. The census row.** D7, after every rung, as a number in the
  census doc.

## 6. Cost and risk

| risk | consequence | what bounds it |
|---|---|---|
| Newton finds a preimage on the wrong branch, or one seed finds the same preimage twice | a piece counted twice or missed; the distance too small or too large | dedupe by result within `1e-6·\|q\|`; the seed rule per dominant term; G3's chaos-sample check is the arbiter |
| a common transform's dominant term has no clean seed (two kernels of equal weight) | refused, with the reason | D3 says so; the census row says how often |
| the spliced Newton step makes the mode-D shader compile per flame instead of per kind | compile time on every flame change | the chaos game already pays this; the sticky-shader machinery applies |
| D6's sampled ball is smaller than the set | pixels on the set past the ball read as exterior | the margin is stated and the Extent label shows the radius; G4 measures a sample against it |
| the `Real` trait costs the f64 path speed | a slower prefix | measured against today's before D2 lands; the prefix is 3 ms at depth and has room |

## 7. Deliberately not here

- **A builder UI.** The flame editor with the panel's reasons is the
  builder; anything more is a UI project after the reach is
  measured.
- **Non-invertible variations.** Folds, blurs, `NeedsAccum` and
  per-thread-state variations: §8.2 of the design doc, unchanged.
- **Non-affine 3D finals and 3D xaos.** After the planar ones
  measure.
- **A per-node ball for xaos** (D4): after a flame shows the single
  ball too loose.

## 8. Order of work

Item numbers were the delta plan's §9, which ordered all three plans:
D1 and D2 are item 3 there, D4 is item 4, D3 is item 6. **From
2026-09-18 the order is the delta plan's §10**, which redirects the
branch at [flame-deep-zoom.md](flame-deep-zoom.md) and makes this
plan's reach a lever for it. Two levers, and the meter decides
between them:

- **`InverseDef`s for the variations that block real flames.** Not
  the shipped smoke tests -- one variation each, a long tail of
  one-offs -- but the imported corpus, where 3 of 45 flames have
  every variation invertible and 57 distinct variations block the
  rest (§9f). Read off the bodies, the invertible ones near the top
  are `curl` (a rational conformal map), `polar2`, `elliptic` and
  `bipolar` (log-type conformal maps, with branches) and `eyefish`
  (radial). A day each on the generic-body machinery, and the meter
  moves with every one. Three that LOOK invertible are not:
  `juliascope` and `rays` draw a random branch per sample and
  `boarders` takes a random 25% path, so they are stochastic maps --
  refusals under the design doc's §8.2 alongside the blurs
  (`pre_blur`), the subflames and the plots, and the panel should
  name the reason rather than say "not affine".
- **Forward reach without an inverse.** A forced prefix needs
  forward maps, which every variation has, and a Lipschitz bound per
  variation over the ball, which none supplies yet -- the deep-zoom
  plan's §7 item 1. Where that bound exists the deep zoom's stage 2
  can enumerate cylinders by forward branch-and-bound with no
  `InverseDef` at all. It does not help the inverse WALKS (mode D,
  the measure), which still need the inverse; it is the reach lever
  for the chaos-game deep zoom, which is the destination.

D5 (finals) and D6 (the ball as a bound) stay as written and are
picked up when the corpus meter says a final or a non-contractive
transform is what blocks the next flame. D8's registry half is not
on the path.

Extend the corpus meter (`how_often_a_real_flame_sums_a_kernel_with_an_affine`,
ignored, over `output/*.flame`) to print the blocking variations
before starting either lever, so each `InverseDef` is chosen by a
number.

## 9. Record

### 9a. The scalar trait, and the derivatives that come with it, 2026-09-17

D2's arithmetic half. [`src/scene/ifs_real.rs`](../../src/scene/ifs_real.rs)
holds `Real` -- the four operations, a square root, and `lit`, which
takes `&self` because a `BigFloat` carries its own limb count and
there is no "the constant 2" without a precision to build it at --
and `Transcendental` on top of it. `f64` implements both. `Dual<T>`
implements both when `T` does, so it nests, and `Dual<Dual<f64>>` is
a second derivative.

**The six kernels are now one body each.** `kernel_forward_gen` and
`kernel_inverse_gen` were written operation for operation against
the f64 bodies, and `the_generic_kernel_is_the_f64_kernel` compares
them BIT FOR BIT -- not to a tolerance -- at every fixture and probe
point. It passed on the first run, so the f64 bodies are now those
functions, and the second copy is gone.

**The Jacobian is differentiated, not derived.** Six closed forms,
each with its own frame change and its own chance of a sign, against
one rule applied by the compiler: they agree to a relative 1e-9 at
13,731 points spread over every branch and both sides of every guard.
The closed forms are deleted. What survives of them is
`kernel_inverse_domain`, the guards, which are still needed: the
generic inverse returns a SENTINEL where there is no preimage, and
the derivative of a sentinel is a finite number meaning nothing.

**The Hessian's central difference is gone, and here is what it was
costing.** The dual is exact to f64 rounding, so the gap between the
two IS the difference's error. Sampled uniformly it never showed:
1e-8 relative everywhere, which is why the old comment's estimate of
1e-10 was in the right neighbourhood. A uniform sample almost never
lands near a singularity. Walking downhill in clearance does, and
the worst relative gap then reads:

| clearance < | 1e-8 | 1e-6 | 1e-4 | 1e-2 | more |
|---|---|---|---|---|---|
| spherical | **58** | 2.8e-8 | 2.8e-8 | 2.8e-8 | 2.8e-8 |
| bubble | **4.7** | 1.5e-4 | 1.1e-6 | 1.6e-8 | 1.6e-8 |
| hemisphere | **23** | 5.7e-5 | 6.4e-7 | 1.8e-8 | 2.4e-8 |
| disc | 6.5e-5 | 3.2e-5 | 2.7e-7 | 1.9e-9 | 1.1e-8 |
| julian | **240** | 6.3e-8 | 6.3e-8 | 6.3e-8 | 7.1e-8 |

Eight digits in the open plane and NO digits within 1e-8 of a pole.
A relative error of 58 is not a worse answer, it is a different
tensor, and a second-order correction built on it adds noise where
it was meant to subtract curvature. That band is where the
perturbation handover lives, which is the reason this mattered
enough to measure rather than assume.

**G2 holds.** Every existing gate green, 1221 unit tests, all 52 GPU
IFS gates, and all eleven shipped mode-D presets byte-identical.

**Still open in D2**: `BigFloat` implements neither trait yet, so
`big_kernel_inverse` is still a transcription. It has `sqrt`, `ln`
and `atan2` and lacks `exp`, `sin` and `cos`, which is the delta
plan's item 8 -- the algebraic three (spherical, bubble,
hemisphere) could take `Real` today and the other three could not,
and splitting the walk by rung before item 8 lands would buy one
copy of three kernels at the price of two code paths. D1 -- the move
into the registry -- has not begun.

### 9b. The kernels moved into the registry, 2026-09-17

D1's first half. Two `match`es on the variation NAME are gone: the
one in `variation_stage` that noticed a kernel, and the one in
`transform_map_2d_ordered` (with its solid twin) that built it from
the transform's parameters. In their place
[`src/variations/inverse.rs`](../../src/variations/inverse.rs) holds
`InverseDef`, `INVERSES` -- append-only, like the variation
registration list -- and `VariationRegistry::inverse(name)`, which
resolves through `get()` so an alias finds the canonical variation's
inverse and a name the registry does not hold has none.

Each kernel now lives beside its own forward WGSL: `julia`,
`julian`, `bubble`, `disc` and `blob` in `defs/advanced.rs`,
`spherical` in `defs/basic.rs`, `hemisphere` in `defs/full3d.rs`,
`julia3D` and `julia3Dz` in `defs/extended.rs`,
`quaternion_julia` in its own file. Each reads its OWN parameters
through a lookup closure, so `ifs_analysis.rs` no longer knows that
`julian` has a `dist` or that `blob` has three parameters. A
variation that has an inverse but cannot supply one at these
settings returns a `Refusal`, and the caller turns that into the
transform's own `Degenerate` or `Mode` naming the variation, which
is what the flame panel already shows.

**A side list, not a field on `VariationDef`.** §2 asked for
`inverse: Option<&'static InverseDef>` "absent by default so the
other 640 definitions do not move". Rust has no default for a field
of a plain struct literal, so that field would mean adding
`inverse: None,` to all 647 of them: 647 lines of noise to reach
seven. The list is keyed by `VariationDef::name` and the lookup goes
through the registry, which is what the field was for.

**G1 now runs over the registry.** `every_registered_inverse_is_
reachable_and_inverts` iterates `INVERSES` and checks three things
per entry: that a transform carrying only that variation analyses to
a map with that kernel -- the whole wiring, definition through
registry to `transform_map_2d_ordered`; that every forward branch is
undone by SOME inverse branch, which is the property Bubble's
inverse failed by 102% before it was rewritten; and that the dual
Jacobian exists wherever `kernel_inverse_domain` says it does. A
seventh kernel gets all three by being appended to the list.

Two things the gate had to be told, both real:

- **`julian` at its defaults IS `julia`.** Power 2 and distance 1
  give the same `Root { n: 2, d: 1 }`, so at the defaults the first
  check cannot tell which definition produced it. The gate sets
  power 3.
- **The round trip runs in an annulus, 0.2 to 2.** Below it,
  `spherical`'s forward carries the flame's `1e-6` guard --
  `z/(|z|² + 1e-6)` -- which its inverse deliberately does not undo,
  so the round trip is off by `1e-6/|z|²`: a millionth at radius one
  and everything at the origin. Above it, `disc`'s forward is
  periodic in the radius and the ring passes the four branches
  tried. The tolerance, 3e-5, is that guard's own size at the inner
  edge.

Presets byte-identical, every existing gate green.

### 9c. The affine roles moved too, 2026-09-17

D1's other half, and with it D1 is done. `AFFINE_VARIATIONS` -- a
name list -- and `affine_role`'s twelve-arm match are gone.
`InverseKernel` gained an `Affine` arm carrying
`fn(f64, ParamFn, Space) -> Option<AffineRole>`, and each of the
twelve is now beside its own forward WGSL: `linear` and `linear3D`
in `defs/basic.rs`, `zscale`, `flatten` and `zcone` in
`defs/depth3d.rs`, `ztranslate` in `defs/extended.rs`, `affine3D`
(with its fifteen-parameter map, moved wholesale) in
`defs/affine3d_misc.rs`, `zblur` in `defs/blur.rs`, and the four
axis rotations in `defs/rotation3d.rs`. The weight is passed
separately from the parameters because for several of them it IS
the parameter: a rotation's angle, a z scale's factor.

`affine_role` is now five lines: ask the registry, take the `Affine`
arm, call it. A variation whose entry is a KERNEL is not affine,
which is what `variation_stage` wants -- it reads a `None` here as
"ask whether it is a kernel instead".

G1 grew an affine arm to match, and it checks the one thing the
name list could not. An affine entry is analysed alongside a
`linear`, since several of these contribute nothing in the plane and
a transform with no contribution at all is refused for having no
variations, which would say nothing about the role. Then: the plane
must give an affine map, and where the SOLID role is absent the
transform must be REFUSED naming this variation. That last clause is
what `zcone` and `zblur` exercise -- both are nothing in the plane
and neither is affine in space, and before this the only evidence
they were handled right was that a match arm existed.

Twenty-two registered inverses now: twelve affine roles, seven
planar kernels, three solid. 1223 unit tests, every gate green,
presets byte-identical.

### 9d. Xaos, and a colour-speed repair found on the way, 2026-09-18

D4. `Disqualification::Xaos` is gone: a xaos flame is an IFS the
walks handle, on the CPU and in both shaders.

**The direction is the whole thing.** An address is `[a_1, a_2, ...]`
in DISCOVERY order and the forward chain runs it backwards, so
appending a child `i` to a path whose last map is `l` puts `i`
immediately BEFORE `l` in the chaos game's own order. The transition
to admit is `i -> l`, not `l -> i`. `XaosGraph` in
[`ifs_analysis.rs`](../../src/scene/ifs_analysis.rs) stores it that
way round, built from `weight_j · xaos[i][j]` row-normalised, which
is what `select_transform_xaos` in `utilities.wgsl` draws from.

**The measure is a Markov chain now, not a product of draws.** A
cylinder's weight under a graph-directed IFS is
`π_{a_k} · Π p_{a_{m+1} a_m}`, whose incremental form is
`step[i][l] = π_i · p_{i l} / π_l` per appended map. Without xaos
`p_{i l} = w_i` for every `l`, so the factor is `w_i` and the whole
thing collapses to the product of weights it always was -- which is
why `MeasureMaps::step` is an `Option` and the ordinary path did not
move a bit. `π` comes from power iteration, and a state whose
stationary probability falls below 1e-12 is treated as unreachable:
the chain leaves it and never returns, so no part of the attractor is
there.

**One array serves both questions on the GPU.** A transition is
admissible exactly when its step probability is positive, so
`ifs_xaos` at group 1 binding 4 holds the count, the `n*n` matrix and
the stationary row, and the distance walk reads a sign where the
measure walk reads a value. A candidate's last map rides in the flags
word -- bits 8 and up, plus one, so zero means none -- because all
six vec4s a seed packs into were full and a u32 bitcast through an
f32 has 24 spare bits.

**G5, both halves.** All-ones would be a vacuous test: `has_xaos`
reads it as no xaos and the graph is never built. All-HALVES is the
real one -- the graph is built, every row normalises to the plain
weight draw, and the distance, the address and the measure have to
come out bit-identical, which they do at 144 grid points, and the
render pixel-identical, which it is. The teeth are in the second
half: a four-cycle matrix, where after map `i` only `i+1` may follow.
Irreducible, so there is no question of which class the chaos game
happened to start in, and the attractor is a strict subset of the
filled square the same four maps make without it.

| | |
|---|---|
| restricted-chaos points reading as ON the set | 2000 of 2000 |
| free-chaos points the restricted walk rejects | 1994 of 2000 |
| pixels the four-cycle moves | 4096 of 9216 |
| sample points where CPU and GPU disagree | 0 of 256 |

Either direction alone is easy to pass. A walk that ignored the graph
would pass the first row and fail the second; one that admitted
nothing would pass the second and fail the first.

**A repair found on the way.** The shader's measure walk carried
`let sp = 0.0; // EXPERIMENT: was ifs_maps[bi].measure.y` -- a
leftover committed on 2026-09-17 with §5l, which made the GPU's
colour fold ignore colour speed entirely. The gate that compares the
shader to the CPU did not catch it because every fixture had
`color_speed` zero, where both sides agree on a fold neither is
exercising. A gasket at speed 0.6 now runs beside them: it reads
0.024 off with the leftover in place and 0.00009 with the map row's
own value, which is the repair and the evidence for it in one.

**D7's row, and it is not the one the order of work expected.**
Zero of the 170 shipped flames carry a xaos matrix at all, so the
census is unmoved and says nothing about D4's reach -- those configs
are our own. What does say something: Apophysis and JWildfire write
the matrix into EVERY export, so of 45 imported `.flame` files to
hand, **27 carry one and 8 are non-trivial**. The check D4 removed
was `flame.xaos.is_some()`, not `has_xaos()`, so all 27 were refused
-- three in five imported flames turned away by a matrix that in
nineteen of those cases was all ones and meant nothing. That is the
reach this rung actually bought, and it is larger than the census
could have shown.

**Open.** The BALL is still one ball for every node, which is
stricter than a graph-directed IFS needs and therefore sound; D6 is
where that is revisited. And `NotContractive` still refuses any map
with `σ_max ≥ 1`, where a graph only needs its CYCLES to contract --
also D6.

### 9e. Sums, by Newton, on both sides, 2026-09-18

D3, whole. A transform whose normal phase sums a kernel with an
affine is a `Map2::Sum`, inverted by Newton from the dominant term's
seed; the branches are the kernel's and each seeds its own solve;
the shader inverts the same map over the same kernel's forward body,
with the Jacobian by central differences in f32.

**What the measurement changed, three times.**

*The seed is decided at the point, not at the map.* D3 says "the
seed from the transform's dominant term's own inverse", and a
`kernel_leads` flag answered that from the weights -- once, for the
whole map. That is not where the question is asked. On `spherical
0.1 + linear 0.9` the weights say the affine leads, which is right
over most of the plane and wrong near the origin where `z/|z|²` is
unbounded: the affine seed there ran the full step budget and stopped
at a residual of 2.3e-10, four orders past the tolerance. Both seeds
are formed now and the smaller residual starts.

*The Jacobian needs the branch.* A root's second preimage is the
negative of its first, so a Jacobian taken on branch 0 against a
residual taken on branch 1 has the wrong sign and Newton walks away
from the answer. Every branch-1 solve failed until
`forward_kernel_jacobian` took a branch argument.

*A root's FORWARD map has a cut its inverse does not.* The forward
divides the angle, so `atan2`'s jump across the negative x axis lands
on another branch and the map restricted to one branch is
discontinuous there -- not merely non-smooth. Seven of four hundred
solves on `julia 0.6 + linear 0.4` fail, and all seven have their
preimage within 0.028 of that ray. `Kernel::forward_singular_distance`
reports the ray; `Kernel::singular_distance`, which is about the
inverse, correctly does not.

**The step cap is twelve, and the fold is why.** Away from a fold
Newton takes three to six steps, which is what D3 expected. But
`0.5z − √z` folds at `|z| = 1`, and at a point 0.985 out the
dominant term's seed lands on the wrong side of it: the residual
RISES at the fourth step and the solve wanders six before finding the
basin, converging in three more. A halving safeguard was measured
against that and bought one step of the nine and nothing at all at
the other five sample points, so it is not there -- a wide cap is
paid only where the loop wanders, since it returns the moment the
residual is met, while a safeguard's extra forward evaluation is paid
at every step of every solve.

**G3.** Thirteen mixes -- ten of `linear + spherical`, plus
`spherical`, `bubble` and `julia` at the census's own 0.6/0.4 -- at
400 points of the support each:

| | |
|---|---|
| mean steps to 1e-12 | 3.2 to 6.0 |
| worst residual away from a fold | ≤ 1.0e-12 |
| worst steps away from a fold | 10 |
| solves that failed | 7 of 5200, all within 0.028 of a root's cut |
| chaos-sample points reading as exterior at 2^4 | 0 of 1000 |
| the walk's far field against the sample, at 2R / 4R / 8R | 1.007x / 1.004x / 1.002x |

On the GPU, against the CPU point by point over five fixtures:

| | |
|---|---|
| residual the shader converged to | ≤ 1.0e-6, which is D3's f32 figure |
| preimage error × the local contraction | ≤ 2.8e-6 |
| σ, relative | ≤ 7.9e-4 |
| solves the GPU declined that the CPU made | 0 of 1255 |
| pixel agreement, rendered, on a set of two sums | 99.56% |

**The fold is where the two sides stop being comparable, and that is
not a defect.** A sum `lw·z + kw·z/|z|²` folds on the circle `ρ =
√(kw/lw)`: two preimages meet there and the map is not invertible AT
it. Newton converges linearly rather than quadratically that close,
and a residual of 1e-6 divided by a vanishing σ is a point error of
anything -- measured, 3.0 relative in σ on `spherical 0.2 + linear
0.8`, whose sample grid crosses its circle. Both halves of G3 bucket
by conditioning for that reason and compare the residual, which is
what both sides stop on, everywhere.

**D7's row, and the plan overstated the prevalence.** Of the 170
shipped flames, not one is refused for `MixedSum` -- the reason does
not appear in the census table before this rung or after, and the
count that qualify is the same twenty either way. Measured instead on
the imported `.flame` corpus, where D4's row was also measured:

```text
  files 45 | flames 45 | transforms 109
  transforms that SUM a kernel with an affine: 4 in 4 flames
  flames unlocked by the sum rung alone: 0
  still refused: 0 sum TWO kernels; 87 name a variation with no inverse
  the sums, by kernel: spherical 3, bubble 1
```

Four in a hundred and nine is not "among the commonest". Four of the
TWENTY-TWO that get past the catalogue is -- close to one in five --
and that is the honest reading, because eighty-seven of those
transforms name a variation with no `InverseDef` at all and never
reach the sum. Zero flames are unlocked, because each of the four
sits in a flame that also carries one of the eighty-seven. The
catalogue is the wall; the sum was a second wall behind it, and
taking it down stops the sum from being the NEXT refusal every time
D1's registry gains an entry. Not one of the four sums two kernels,
so the case D3 leaves refused did not occur at all.

**Nothing that is not a sum changed.** The row grew from 112 bytes
to 144 -- a sum needs a fourth affine and there was nowhere to put it
-- but `IFS_NEWTON`, the seven forward kernels and the three dispatch
lines are spliced only when a packed row says kind 7, through markers
that are DROPPED otherwise. Every shipped mode-D preset is
byte-identical, and a gate asserts that no marker and no solve
reaches a shader without a sum in it.

**Open.** The shader solves twice per step where it could solve once:
`ifs_inv_point` and `ifs_inv_sigma` each run their own Newton, and
the walk calls both. That is paid only by a flame with a sum in it,
of which there are none shipped, so it is a cost waiting for a user
rather than a cost. Two kernels summed are still refused, per D3.
And the CPU's `Map2::hessian` for a sum is central differences of a
Newton Jacobian -- the one place D2's dual numbers do not reach,
because nesting duals through a solve would differentiate the
iteration rather than the map.

### 9f. The corpus meter, and what actually blocks it, 2026-09-18

Measured for the review that redirected the branch (delta plan §10),
by a temporary probe over the 45 imported `.flame` files in
`output/`, counting a flame under every variation it uses that has no
`InverseDef`:

```text
  corpus: 45 flames, 3 with every variation invertible
  flames blocked by:
     7  hypertile1      7  poincare3D      7  rays
     6  arctruchet      6  polar2          6  pre_blur
     5  curl            5  elliptic        4  juliascope
     3  combimirror     3  spirograph3D    3  subflame_wf
     3  yplot3d_wf      2  bipolar         2  boarders
     2  crown_js        2  dc_carpet       2  dc_hexes_wf
     2  eJulia          2  eyefish         2  iconattractor_js
     2  lorenz_js       2  parplot2d_wf    2  polarplot2d_wf
     2  post_mirror_wf
  57 distinct blocking variations
```

The corpus is JWildfire's randomiser output and is variation-heavy,
so the numbers overstate how bad a hand-made flame fares; but it is
the only corpus of real flames to hand, and the shipped census (170
flames, mostly one-variation smoke tests) is worse as a meter, not
better. Three things the table says:

- **No single variation unlocks much.** Seven flames is the top row.
  The reach is a long tail and has to be walked as one.
- **The invertible ones are the cheap ones, and they were checked
  against the bodies rather than the names.** `curl` is
  `p / (1 + c1·p + c2·p²)` in complex form, a rational conformal map;
  `polar2` is `(arg p / π, ln|p| / 2π)`, a log map with the branch of
  the argument; `elliptic` and `bipolar` are log-type conformal maps
  of the same family; `eyefish` is `2p / (|p| + 1)`, radial. All fit
  the `InverseDef` shape D1 built, with the generic body giving the
  Jacobian for free.
- **Three that look invertible draw a random number.** `juliascope`
  picks one of `|power|` branches per sample with a mirror on the
  odd ones, `rays` draws a random angle, and `boarders` takes a
  random 25% path. Those are stochastic maps -- each is several maps
  chosen by the sample, like a root's forward branches -- and an
  inverse walk treats them as a refusal today. (A forced prefix CAN
  take them: the branch is one more choice in the prefix, weighted
  by its probability. That is the forward direction's advantage
  again.)
- **The rest should be said, not left as "not affine".** `pre_blur`
  is a blur, `subflame_wf` iterates a whole other flame, the
  `*plot*_wf` family draws a graph, `dc_*` writes colour, `lorenz_js`
  and `iconattractor_js` are attractors of their own. The design
  doc's §8.2 already sorts these out; the panel should name the
  reason.

And the thing the meter cannot say, which is the point of the second
lever in §8: every one of these variations has a FORWARD body, so a
forced-prefix deep zoom with a Lipschitz bound reaches every one of
them without an inverse being written.
