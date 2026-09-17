# Nonlinear perturbation for the flame attractor modes (plan, 2026-09-16)

Asked for alongside the report in
[ifs-distance-rendering.md](ifs-distance-rendering.md) §8.15. This
is the plan for carrying mode D's reference/delta split -- the
affine handover of §2.5 there -- through nonlinear inverse maps, so
a julia, spherical or bubble set zooms past f32 the way an affine
set already does.

**What it buys.** Deep zoom on nonlinear sets. Today a nonlinear IFS
hands over at level 0 and the shader walks from the pixel's own f32
position, `centre + basis·uv` in f32: at a view centre of magnitude
0.25 an f32 step is 3e-8, and a 640-pixel view spanning `4/2^z` has
pixels that size at z ≈ 17.7. Past that the picture is blocks. An
affine set has no such cap: the CPU walks the centre in `BigFloat`
and hands over an O(1) delta.

**What it does not buy, said first.** Not the band that slides under
animation (§8.15: that is the cut's image, held by Extent), not the
halo's looseness (§8.15: the bound's, at any radius), not the
over-reads beside the tail's images (the cut's). Perturbation is
about representing a pixel's offset from the centre; those three are
about the bound.

## 1. What is there

The affine handover, all in
[`ifs_estimate.rs`](../../src/scene/ifs_estimate.rs) and
[`escape/ifs.rs`](../../src/escape/ifs.rs):

- `seed_beam(ifs, centre, view_basis, px, max_levels, beam)` walks the
  view CENTRE (any `SeedPoint`: f64 or `[BigFloat; 2]`) level by
  level, carrying per candidate a **basis** `B_k` -- the accumulated
  inverse-linear map composed with the view basis, so a pixel's
  normalised offset `uv` maps to its delta at level k in one 2×2
  multiply. `compose_basis(inv, B) = M⁻¹·B`; the translation is
  carried entirely by the reference, which is what makes the split
  exact for affine maps.
- It scores every candidate at the nearest point its view can reach,
  `r − basis_reach(B)`, so the inherited bound and escape level are
  sound for every pixel.
- It stops at the first of: any `basis_reach ≥ radius ·
  HANDOVER_FRACTION` (0.25); the view not agreeing on the beam
  (`view_agrees`: a pruned branch's most optimistic key must be worse
  than every kept branch's most pessimistic one, both widened by
  their reach); and -- the line this plan removes -- `!affine`, so a
  nonlinear IFS hands over at level 0.
- `Seed { position, basis, sigma_per_px, address, last_sigma,
  bound_per_px, escape, done }` and `pack_seeds` put fifteen seeds in
  the params' `fdata` block: position + basis (one vec4 and a half),
  σ and bound per pixel, address fraction, escape state. The shader's
  `ifs_evaluate` starts each pixel at `position + basis·uv`, ranks
  the seeds by its own position, and continues the walk in f32
  (`estimate_seeded` is the CPU reference of that continuation).
- `ensure_ifs_seeds` runs it once per view (keyed on centre, zoom,
  rotation, size, beam, and the packed IFS), with a level budget of
  `zoom_log2 + 64` and the centre at the zoom's precision
  (`centre_at_precision`).

The delta form is exact for affine maps because
`S⁻¹(C + δ) = S⁻¹(C) + M⁻¹δ` has no cross term (§2.5). A nonlinear
inverse `F` has one:

```
F(C + δ) = F(C) + J_F(C)·δ + ½ δᵀ·H_F(C)·δ + …
```

and that term is the whole of what follows.

## 2. The idea

Carry the basis through the Jacobian instead of the constant matrix:

```
B_{k+1} = J_F(q_k) · B_k          (q_k the reference at level k)
```

and stop the handover **before** the dropped term could matter,
rather than detecting afterwards that it did. Mandelbrot
perturbation has to detect glitches after the fact because a pixel
has nowhere else to go; here it does: the continuation already
starts each pixel from `position + B·uv` in f32 and walks on its
own, which is exactly a rebase into the expanded frame. So the only
question the handover has to answer is *at which level does the
linearisation stop being exact to well under a pixel* -- and the
answer is a stopping rule, not a repair.

**The error is relative, and it compounds forward.** For every
kernel here the inverse is homogeneous or close to it, so the
second-order term at `q_k` with offset `ρ_k = basis_reach(B_k)` is a
fraction `ρ_k / s_k` of the first-order one, where `s_k` is the
distance from `q_k` to the nearest point where `F` is not smooth (its
*singular distance*: the pole for an inversion, the unit circle for
a disc-image kernel). An error made at level k is carried by every
later Jacobian exactly as the delta is, so the relative error at the
handover is the sum

```
E = Σ_k ρ_k / s_k
```

and since `ρ_k` grows geometrically the sum is dominated by its last
terms. The rule: stop at the first level where `E` would exceed
`τ`, with `τ` such that `τ · ρ_handover` is under a hundredth of a
pixel -- `τ = 2⁻¹²` at the affine's cap of a quarter of the radius
is far below that, and the constant is measured, not chosen (G2).

**Built 2026-09-16, and the shape changed once it was measured.**
The rule is not "stop when the budget runs out" but **walk under the
hard rules and hand over at the best level**, because a second error
pulls the other way and the plan as first written had only seen one
of them.

*The hard rules*, which say how far the walk may go at all:

1. ~~*Linearisation budget*~~ -- there is none. It was a tenth of a
   pixel and it was wrong; §9 records why. The curvature error is
   now one TERM of the objective below rather than a gate, and what
   bounds the walk instead is exact: once the curvature term alone
   passes the best total found, no deeper level can win, because
   `spent` only grows. An affine map has an infinite clearance and
   pays nothing, so every affine handover is exactly where it was.
2. *A branch the reference cannot take*: if any map is GAPPED at the
   reference, the prefix ends. Its gap belongs in the answer's
   minimum -- the walk scores it into `dead_min` -- and a seed has
   nowhere to carry that, so dropping it would be an over-read.
   A hole's edge is also a branch edge, so `NonlinearMap2::
   singular_distance` folds `| |v| − hole |` in and rule 1 stops the
   walk well before the view could straddle one.
3. *View agreement*, and the cap `basis_reach ≥ radius ·
   HANDOVER_FRACTION`, both unchanged.

*And then the choice*, which is the whole rule. The shader stores
each seed's position as an f32, so every pixel starts from a point
wrong by `|position|·2⁻²⁴`.
Divided by the pixel size at the handover that is a number of pixels,
and it SHRINKS with depth, because the pixel grows with the view
while the position's magnitude does not. The linearisation's error
grows with depth. So the level to hand over at is the one minimising
their sum, and for an affine walk -- which pays nothing for
curvature -- that is always the last one, which is what it already
did.

Both terms are a length divided by the pixel, so both scale as
`1/px` and the ARGMIN does not depend on the resolution at all --
which is the property `the_handover_does_not_depend_on_the_resolution`
pins, and the property the old cap did not have.

For a nonlinear walk the best level can be any of them, **including
the first**, and that is not a degenerate case:

## 3. What each kernel has to supply

Two closed forms per kernel, beside the inverse and the local σ it
already has in [`ifs_analysis.rs`](../../src/scene/ifs_analysis.rs).
**Built 2026-09-16** as `Kernel::inverse_jacobian` and
`Kernel::singular_distance`, with `NonlinearMap2` and `Map2` twins
that carry them through the affines; §6 records what building them
found.

| kernel | inverse on `v` | `J` of the inverse | singular distance |
|---|---|---|---|
| Root `{n, d}` | `\|v\|^{m} e^{i·n·arg v}`, `m = \|n\|/d` | `R(n·arg v)·diag(m, n)·\|v\|^{m−1}·R(−arg v)`: the radial and tangential rates read out of the frame at `v` and into the one at `u` | `\|v\|` (the pole; an integer `n` has no cut) |
| Spherical | `v / \|v\|²` | `(I − 2 v vᵀ/\|v\|²) / \|v\|²` | `\|v\|` |
| Bubble (branch b) | `v·s`, `s` from `bubble_scale` | `s·I + 2 s'·v vᵀ` | `1 − \|v\|` (the disc's edge) and, for the outer branch, `\|v\|` |
| Hemisphere | `v / √(1 − \|v\|²)` | `t·I + t³ v vᵀ`, `t = (1 − \|v\|²)^{−½}` | `1 − \|v\|` |
| Disc (ring m) | polar → `(r sin θ, r cos θ)` with `r = φ/π + m`, `θ = ±π\|v\|` | chain rule through `(\|v\|, φ)` | the nearest of: the cut ray at `φ = π`, the ray `φ = −mπ` where the ring reaches zero, `1 − \|v\|`, and `\|v\|` |
| Blob | `(v_y, v_x) / s(θ)` | `J = (1/s)·P − (s'/s²)·(swap v)·∇θᵀ`, `P` the swap | `\|v\|` (θ's pole) and where `s(θ) → 0` |

`NonlinearMap2::inverse_jacobian(q)` is then
`pre_inv.m · J_K(before_kernel(q)) · post_inv.m / w`.

The tie to the existing `local_sigma_factor` is a consistency check
for free, and it is worth stating precisely because stating it
loosely is how it went unnoticed. The inverse's derivative is the
inverse of the forward's, so their singular values are reciprocal
**and swapped**:

```
σ_max(J_inverse) · σ_min(forward) = 1
```

At the kernel that is an equality and holds to machine precision. At
the MAP it is an inequality -- `NonlinearMap2::singular_values`
multiplies the parts' singular values, which bounds rather than
computes the product's -- and the sound direction is
`σ_min(reported) ≤ 1/σ_max(J)`: a σ_min above the truth is a bound
above the truth, which is an over-read.

**Precision, kernel by kernel.** The reference walk runs the centre
in `BigFloat`, which has add, mul, recip, sqrt, ln and atan2 and no
exp, sin or cos. So the rungs are:

1. *Rational kernels*: Spherical (`v/|v|²`), and Root with `d = ±1`,
   which is `v^n` (n > 0, d = 1), `v^n / |v|^{2n}` (d = −1), or the
   conjugates for n < 0 -- multiplication and one reciprocal. This is
   the grand julian of every report this month and every `julia`.
2. *Square-root kernels*: Bubble and Hemisphere.
3. *Transcendental kernels*: Root with a non-integer `|n|/d` (needs
   `exp(ln)`), Disc and Blob (need sin/cos). Not until BigFloat has
   them; those sets keep today's cap.

The Jacobian itself stays f64: only the product `B_k`'s *direction
and scale* matter, relative precision is enough, and the escape
engine's reference orbits make the same call (`reference.rs`: f32
pairs suffice because only the linear term reads them).

## 4. What changes, and what does not

- `SeedPoint` gains `apply_map(&self, m: &Map2) -> Option<Self>`,
  implemented for `[f64; 2]` by `apply_inverse` and for
  `[BigFloat; 2]` by the rung-1 rational forms (rung 2 with `sqrt`).
  A map outside the implemented rungs returns `None` and `seed_beam`
  stops there, which is level 0 for those sets, as now. **Done.**
- `seed_beam`: the `!affine` break becomes §2's rules; the child step
  uses `apply_map` and `inverse_jacobian`; `sigma` multiplies
  `Map2::local_sigma`, which is what `IfsSpace::step` computes
  alongside the point for a walk whose point IS an `[f64; 2]`.
  **Done.**
- `Seed`, `Seeds`, `pack_seeds`, `MAX_SEEDS`, the `fdata` layout and
  the shader's seeded start: unchanged **for a bounded nonlinear
  set**, which is the claim §7 checked and the part that worked.
  They are NOT enough for an inversion set: f32's mantissa at the
  handover is what declines those, and widening it is a change to
  all five. See §9.
- `estimate_seeded` needed a fix of its own, though not for this
  reason: it was a copy of the walk from before §8.12 and §8.13 of
  [ifs-distance-rendering.md](ifs-distance-rendering.md). §7.
- The 3D twin: `seed_beam3` / `compose3` take a 3×3 Jacobian the
  same way; Quaternion's inverse `qⁿ + c` is a polynomial (rung 1),
  Root3/RootZ3 are rung 1 when `n` is an integer power. Not built.

## 5. Gates

- **G1** Jacobians, **done**: for every kernel and branch, at
  thousands of random points inside the image, `J` against central
  differences (`the_kernels_jacobians_are_the_derivative`, worst
  3.5e-5 relative), the σ tie above to machine precision, and the
  clearance checked to be a real one -- half of it keeps the branch.
  A map-level twin composes the affines and asserts the sound
  direction of the inequality
  (`a_nonlinear_maps_jacobian_composes_through_its_affines`).
- **G2** The seeded walk is each pixel's own, **done**:
  `estimate_seeded` against `estimate` from the pixel's own f64
  position, on a julia dust, a grand julian and a Sierpinski at
  zooms 2^12 through 2^28 -- distance within a quarter of a pixel
  against a measured worst of 0.004, first branch identical, and the
  handover required to reach level 5 or deeper where f32 alone would
  not resolve the view
  (`a_nonlinear_handover_answers_what_each_pixel_answers`).
  It ran at 96 pixels and so could not see §9's fault;
  `the_handover_does_not_depend_on_the_resolution` is the gate that
  can, and it is the one to extend when this part changes again.
- **G3** The handover level tracks the zoom, **done**:
  `the_handover_goes_deeper_as_the_zoom_does` walks three sets at
  four zooms and asserts the level never goes backwards and reaches
  the depth measured. The pole fixture is the grand julian of the
  reports, and it is held to a lower bar on purpose -- eleven levels
  at 2^44 where the other two reach thirty-three, which is its
  curvature refusing rather than a cap anyone chose.
- **G4** Byte identity: every affine mode-D preset renders
  byte-identical (`output/ifs/preset-*.png` against the current
  set); the affine path is not touched.
- **G5** GPU, **done**: `the_gpu_agrees_on_a_nonlinear_set_at_depth`
  renders a bounded julia at zoom 2^20, where the handover reaches
  level 5, and classifies every pixel by the same seeds and the same
  continuation the shader ran: 99.9% agreement. A Sierpinski runs
  beside it at the same depth, at 100%, which exercises the affine
  path and pins the measurement -- see §8.
- **G6** The picture, **done**: the shipped Douady Rabbit preset
  centred on its own attractor, rendered by the CLI at 2^12, 2^20
  and 2^28 by THIS build and by the one before the handover, the
  same config file through both
  (`output/deepzoom/before-after.png`):

  | zoom | before | after |
  |---|---|---|
  | 2^12 | 672 distinct colours | 672 |
  | 2^20 | 269, visibly blocky | 632 |
  | 2^28 | **1** -- a flat field | 567 |

  Below the cap the two are identical, which is the other half of
  the claim. Not the grand julian: §7 measured that it declines the
  handover, so its picture is not this step's to fix.

## 6. What step 1 found, 2026-09-16

Building the Jacobians and checking them against the walk's own σ
turned up two faults in shipped code and one deliberate omission.
The gate is the reason: a derivative and a σ_min are the same fact
twice, and until now only one of them was written down.

**Bubble's σ_min was the tangential derivative alone, and that is
unsound.** The forward `4p/(|p|² + 4)` FOLDS at `|p| = 2`: its
radial derivative `4(4 − |p|²)/(|p|² + 4)²` passes through zero
there while the tangential stays at ½, so the smaller singular value
is the radial one everywhere between. `local_sigma_factor` returned
`|v|²/f`, the tangential, which overstates σ_min without bound as
the fold is approached -- **21× at `|v| = 0.9989`**, measured. An
overstated σ_min makes `σ·(r − R)` too large, which is an over-read:
the pixel reports a distance to bubble's piece larger than the truth
and reads as exterior. The fold's image is the image disc's edge
`|v| = 1`, which is where the walk spends its time on a bubble set.

In closed form the two differ by exactly the root. With `x = |v|²`,
`σ_min = x/max(f, |2f'x − f|)` and `|2f'x − f| = f/√(1 − x)` on
**both** branches, so the correct factor is the tangential times
`√(1 − |v|²)`. One term, CPU and shader. No shipped preset uses
bubble, so no picture moved; the existing round-trip test had an
explicit `if kernel != Kernel::Bubble` around its stretch assertion,
which is now gone because bubble passes it.

**Bubble's inverse could not be differentiated numerically at all
near the origin.** The inner branch's `f = 2 − 2√(1 − x)` is
`x + x²/4 + …` computed as a difference of two numbers either side
of 2, so it keeps only the digits `x` is below 1 -- eight of f64's
sixteen at `|v| = 1e-4`, three of f32's seven -- and the derivative
`(f'x − f)/x²` then cancels what is left. Measured, the finite
difference disagreed with the analytic Jacobian by **102%**. With
`x = (1 − root)(1 + root)` the root divides out and every term is a
sum of positives: `s = 2/(1 + root)`, `s' = 1/(root(1 + root)²)`
inside; `s = 2(1 + root)/x`, `s' = −(1 + root)²/(x² root)` outside.
`Kernel::bubble_scale` is the one place both live, and the shader
has the twin. The outer branch's pole at the origin is real, not a
cancellation: its preimage is at infinity.

This one matters more for what comes next than for what ships. Rung
2 puts bubble in `BigFloat`, and a subtraction that loses half its
digits loses half its limbs.

**Blob's σ_min loses digits where the map is conformal, and is left
that way.** It reads the smaller root of a discriminant, `(a −
disc)/2`, whose two roots MEET at `s' = 0` -- twice a period.
Measured against the derivative: 1.5e-13 on a blob whose scale stays
positive, **3.0e-5** on one whose scale crosses zero, against
machine precision for every other kernel. `|det|/σ_max`, with
`det = s²` exact and `(a + disc)/2` adding two positives, is the
same number without the subtraction and is two lines. It is not
taken: 3e-5 of a bound is 3e-5 of a pixel, and the change moves 3
pixels of the Blob Flower preset -- a worse trade than the
inaccuracy. The gate carries that one case at 1e-4 and says why, so
the first measurement that needs those digits finds it.

**And a note on measuring singular values at all.** `Affine2::
singular_values` reads both out of `sqrt(‖M‖⁴ − 4 det²)`, which is
exactly zero for a conformal map -- every kernel here, at some
point -- so it keeps half of f64's digits there, including in
σ_max. The gate computes σ_max its own way, as the larger eigenvalue
of `MᵀM` where the square root is a sum of squares and is added.
That took the check from 7.7e-9 to 4.4e-16 and is why the tie can be
asserted at machine precision at all.

## 7. What step 2 found, 2026-09-16

**The measurement first, because it decides the design.** A julia
dust, maps `|v|^{1/2}` of positive distance, at 96 pixels -- the
resolution these were taken at, which §9 later showed was the one
resolution where the cap did not bite:

| zoom | handover | curvature error | f32, handed over | f32, level 0 |
|---|---|---|---|---|
| 2^12 | level 0 | 0 | 0.013 px | 0.013 px |
| 2^20 | level 7 | 0.003 px | 0.031 px | 3.3 px |
| 2^24 | level 11 | 0.004 px | 0.033 px | 52 px |
| 2^28 | level 14 | 0.001 px | 0.038 px | 835 px |

The last column is the cap this exists to lift: the shader forming
`centre + basis·uv` in f32 at a centre of magnitude O(1). At zoom
2^28 it is 835 pixels wrong and the picture is blocks; with the
prefix it is four hundredths of a pixel. The curvature error -- the
second-order term the Jacobian drops, measured with no f32 in it by
running the continuation in f64 -- never reaches a hundredth of a
pixel at any of these depths.

**An inversion set gets none of it, and the rule says so.** A julian
of negative distance and power 15 inverts to `|v|^{-15}`, which
CONTRACTS the view wherever `|v| > 1`. One level collapses a seed's
reach, and f32 at the handover would then be 1e15 pixels wrong -- it
could not tell two pixels of the view apart. Level 0 is genuinely
the best available and the walk picks it, at every zoom tested. So
the grand julian of §8.12-8.15 keeps the cap it has. The fix for
those is not a budget: it is a handover position carrying more than
f32's mantissa, which is the next thing to build and is written up
in §9.

**The measurement was blocked by a stale copy of the walk.** Before
any of the above could be read, `estimate_seeded` -- documented as
"the reference for what the shader does after the handover" -- was
found to disagree with the direct walk by **11 to 41 pixels on an
inversion set with no handover taken at all**, which is the one case
where the two must be identical by construction. It was a copy of
the walk as it stood before §8.12 and §8.13: no `best_done`, so a
finished path's final bound was pruned out of the beam and lost; no
frozen-inside rule; no fully-gapped rule. The shader has all three,
and so does `estimate_aux_ranked`; only the CPU reference had been
left behind, and it passed its tests because they only exercise
AFFINE sets, where those three rules almost never fire. Ported
across, and budget 0 now reads 0.000 px, which is what made the rest
of the table trustworthy.

Its 3D twin, `estimate_seeded3`, has the same two of the three
(there are no gaps in the solid walk). Not touched here: it feeds
the marcher, where an over-read punches a ray through a surface, and
it deserves its own measurement rather than a change made in
passing.

**And a caveat on the f32 number.** It is the worst over the seeds,
and a seed whose reach has collapsed is one whose whole branch maps
into a tiny region -- where the bound it contributes barely varies
across the view, so the visible error is smaller than the metric
says. The metric is conservative in the direction that declines a
handover rather than takes a bad one, which is the right way round,
but it is an estimate and not a measurement of the picture.

## 8. What step 3 found, 2026-09-16

**The nonlinear seeded path had no GPU test at all.** Every seeded
GPU test uses a Sierpinski, whose maps are affine, so the nonlinear
half of `seed_beam` and the whole of `big_kernel_inverse` were
reached by none of the 57. A sign error in the arbitrary-precision
root -- committed in step 2 -- passed a green run of all of them.

What it was: `z/|z|²` reciprocates the MAGNITUDE and keeps the
argument, while a complex reciprocal reflects the angle as well.
Reading the first as the second makes the conjugation rule depend on
`n` and `d` together instead of on `n` alone, which is wrong in
exactly the `n > 0, d < 0` corner -- a root of negative distance and
positive power, which is most of the sets §8.12-8.15 were about.
`the_big_kernel_inverse_is_the_f64_one` checks every sign
combination against the f64 formula it has to reproduce, and
`the_gpu_agrees_on_a_nonlinear_set_at_depth` is the one that would
have caught it from outside.

**And a measurement caught by its own control.** The first run of
that GPU gate read 90.7% on the nonlinear arm, which looks like a
real disagreement. Running a Sierpinski beside it at the same depth
read **59.6%** -- worse, on the path that had not changed -- which
says the fault is in the test. It was: `view_basis` already carries
the y flip, so negating `uv` a second time mirrored the picture
against the render. Corrected, the two read 99.9% and 100%. The
control is now part of the gate, because a number that looks
plausible on its own is what a control is for.

## 9. The cap was the bug, 2026-09-16

**Reported from use**, on a Douady Rabbit: the quality "degrades at
certain zoom levels, like when it hits the previous floating point
cap, but then gets better again when I zoom in a little more", and
anti-aliasing "sometimes reduces the quality" -- both starting where
the handover starts.

**All three symptoms are one mistake.** The curvature budget was a
hard cap, spent in units of `Σ ρ_k/s_k` that become pixels by
MULTIPLYING by the view's half-diagonal; f32's error is a length
that the same half-diagonal DIVIDES. So the room between the level a
cap allows and the level f32 needs falls as the SQUARE of the
resolution. Measured on the reported set, f32's error at the chosen
handover:

| zoom | 1080p, capped | now | +2× AA, capped | now |
|---|---|---|---|---|
| 2^17 | 1.42 px | 0.27 px | 2.84 px | 0.54 px |
| 2^19 | 1.09 px | 0.29 px | 11.37 px | 0.59 px |
| 2^20 | 2.17 px | 0.28 px | 4.35 px | 0.56 px |
| 2^24 | 2.63 px | 0.40 px | 8.97 px | 0.80 px |

The capped column oscillates with the zoom because the level is an
INTEGER: the shortfall lands one level short, then catches up, then
falls short again. That is "degrades, then gets better when I zoom in
a little more" from the outside. Anti-aliasing is a resolution
increase as far as `ensure_ifs_seeds` is concerned -- it measures
`px` off the supersampled height -- so it squeezed the room by
another factor of four, which is why turning it on made things
worse.

**And the gates could not see any of it, which is the lesson.**
Every handover gate ran at 96 pixels, where there is room to spare:
capped and uncapped choose the same level at every zoom tested, so
all of G2, G5 and G6 were green on the broken build. The fix carries
its own gate at three resolutions including a supersampled one, and
that gate fails on the capped build -- checked, at zoom 2^16 on the
anti-aliased arm.

**The fix.** Delete the cap. The objective already contains the
curvature term, so capping it as well was both redundant and, in the
regime that matters, harmful: a level costing half a pixel of
curvature to save fifty of f32 is obviously worth taking, and the
cap forbade it. What bounds the walk now is exact rather than
arbitrary: once the curvature term alone exceeds the best total
found, no deeper level can win, since `spent` only grows.

Both terms scale as `1/px`, so the level chosen is now independent
of the resolution, and anti-aliasing cannot move it. In display
pixels -- what the downsample leaves -- the error is then identical
with AA and without.

Rendered at 1920×1080, this build against the capped one, same
config: at 2^19 with 2× AA the distinct-colour count goes from 5,308
to 11,913 and 30% of pixels change; at 2^20, 5,504 to 10,793; at
2^24, 4,659 to 10,212. The capped renders show a ragged boundary and
a broken interior, worse with AA than without.
`output/aa/sheet.png`.

## 10. The gap rides along, 2026-09-16

Item 1 of §12, built. The prefix used to stop at the first branch
the reference could not take: a gap belongs in the answer's minimum
-- `estimate_aux_ranked` keeps those in `dead_min` -- and a seed had
nowhere to put one, so dropping it would have been an over-read and
stopping was the safe thing. On a set whose inverses have holes that
is the first or second level, and it cost every level after it.

`Seeds::dead_min_per_px` carries it instead, scored for the whole
view rather than for the centre: the gap is 1-Lipschitz in `q` -- a
distance scaled by `|w|·σ_min` of the post-affine against a frame
that stretches by at most the reciprocal -- so subtracting the
view's reach gives a value no pixel's own gap can fall below. Too
small is the safe direction: the answer is a lower bound, and a
smaller one widens a halo where a larger one erases a piece. It
rides in `fdata[2].x`, the planar layout's one free vec4, and the
shader's `dead_min` starts from it instead of from nothing.

It is snapshotted with the level, so a prefix that hands over at
level 2 having walked to level 6 carries the gaps of levels 0..2 and
not the rest -- the continuation finds those itself, at the pixel's
own position rather than this conservative one.

Measured on the reported grand julian at zoom 2^20: the handover
went from level 0 to level 2, and f32's error at it from 79 pixels
to 19.9 at 1080p. Gate:
`a_gap_in_the_prefix_is_carried_not_a_full_stop`, which checks the
prefix gets past a gap, that one was actually met, and that the
answer still matches each pixel's own walk to a quarter pixel. It
fails with the old stop restored.

## 11. The second-order term, parked and then unparked, 2026-09-16

**Resolved by §13.** What follows is the first attempt, kept because
the three failed models are the reason the second attempt stopped
modelling at all.

### 11a. The first attempt

Item 7, attempted. `Map2::hessian` is built and gated and the carry
works; what does NOT work is the objective's ability to use it, and
the reason is worth recording so the next attempt starts past it.

**The carry itself is right.** `Map2::hessian` takes the second
derivative by central-differencing the exact Jacobian, which is
enough by a wide margin -- an error `ε` in `H` moves a correction
that is itself second order, so a relative accuracy of 1e-6 leaves a
residual a millionth of what carrying nothing leaves, and a central
difference of an exact derivative reaches about 1e-10.
`a_maps_hessian_is_its_second_derivative` checks it the long way
round, by differencing the MAP twice, on all six kernels. That gate
and the method are kept; nothing else of the attempt is.

Carrying `Q(uv) = C_uu·u² + C_uv·u·v + C_vv·v²` beside the basis, with
`Q_{k+1} = J·Q_k + ½H[A_k, A_k]`, measurably improved the delta: on
the reported grand julian the level where the view has finally
expanded went from 3.19 pixels of error to 0.56, and level 5 went
from 0.31 to under the gate's quarter-pixel bar.

**What defeated it was the objective.** The handover picks a level by
minimising `curvature + f32`, and the curvature term has to be
predicted, not measured, because measuring it needs a direct walk per
pixel. Three models were tried:

- `Σ ρ_k/s_k`, the first-order one. Well calibrated for the LINEAR
  carry: it predicted 3.75 pixels where the truth was 3.19. Useless
  for the quadratic carry, which it massively overstates.
- `Σ (ρ_k/s_k)²`, the natural second-order version. Measured against
  the truth at ratios from **0.08 to 166** across three sets and
  three zooms -- not a calibration, a coincidence. With it the
  bounded rabbit REGRESSED from 0.15 pixels to 2.03, because the
  model let the walk go too deep.
- The corner probe, which drops modelling entirely: carry the view's
  four corners exactly beside the reference and measure how far the
  basis and the quadratic miss them. That is exact and cheap, and it
  is still the wrong quantity. At level 12 of a julia dust the
  corners miss by 0.0097 pixels while the real seeded-against-direct
  error is 3.715 -- a factor of 380. The positional miss at the
  handover is not the error in the REPORTED DISTANCE, and the
  conversion between them is not the `σ ≈ 1/grown` the derivation
  assumes.

That last gap is the thing to understand before trying again. Until
it is understood, any objective is steering on a number that is not
the one that matters, and the measured consequence of getting it
wrong is a regression on the sets that currently work.

**Parked deliberately, not abandoned.** The Hessian is in the tree,
gated, and costs nothing while unused. What the next attempt needs
first is a cheap and TRUSTWORTHY predictor of the seeded walk's error
against the pixel's own -- and the way to get one is probably to
measure the reported distance at the corners, continuing each of them
a few levels, rather than to measure their positions.

*That last sentence turned out to be the answer; §13 is what came of
it.*

## 12. The solid twin, measured before it was touched, 2026-09-16

§7 found `estimate_seeded` -- the reference for what the shader does
after the handover -- to be a copy of the walk from before the
cut-outs were fixed, and left its 3D twin alone on the grounds that
the marcher deserved its own measurement rather than a change made
in passing. This is that measurement.

`estimate_seeded3` had the same two gaps: no `best_done`, so a
finished path's final bound was pruned out of the beam and lost, and
no frozen-inside rule. It has no third gap, because the solid walk
has no image gaps to carry.

**They are latent, and that is the finding.** Neither rule can change
an answer unless paths FINISH and the chain runs deeper than one
link, and no shipped solid does both:

- an affine solid -- the Sierpiński tetrahedron, the Menger sponge --
  never finishes a path, because every map contracts and nothing
  reaches `FAR`;
- a `quaternion_julia` finishes them constantly, its inverse `qⁿ + c`
  running to infinity, but its two preimages TIE, so `seed_chain3`
  ends at once by its own view-agreement rule and the continuation IS
  the direct walk. Measured: one link, at beam 1 and beam 4 both.

Measured against the direct walk at 3,000 offsets spanning six
decades, on a unit cube, a tetrahedron and a quaternion julia, before
the fix and after it: identical to 1.1e-12, which is f64 arriving by
two routes.

Fixed anyway. The divergence is real even where it is unreachable,
the template was proven in 2D, and the first solid that both
finishes paths and keeps a deep chain would find an over-read here --
which in a marcher does not fatten a halo, it puts a ray through a
surface. Gate: `the_solid_continuation_is_the_walk`, which also
pins the measurement above so the latency is a fact rather than an
assumption.

## 13. The handover measures itself, 2026-09-16

§11 failed three times to PREDICT what handing over at a level would
cost. The way out was to stop predicting.

**Level 0 is a handover that approximates nothing.** Its position is
the view centre, its basis is the view, and it carries no quadratic;
continuing from it therefore IS each pixel's own walk. So the cost of
handing over at level L is not a quantity to be modelled -- it is the
difference between continuing from L and continuing from 0, and both
are things the walk can just run. Five probe points, the corners and
the centre, continued `PROBE_LEVELS` each, and the objective has the
real number instead of a proxy.

Two details make it right rather than nearly right. Both
continuations must finish at the same ABSOLUTE depth, or the
comparison is between two different walks -- a handover at level L
continued by `n` has gone `L + n` deep, and the reference starts at
zero; getting this wrong cost 0.33 pixels at zoom 2^28 and nothing
anywhere shallower, which is exactly the shape of a bug that ships.
And the check runs only on a nonlinear walk: an affine handover
approximates nothing at any level, so it pays neither the arithmetic
nor the risk.

**With a trustworthy objective the quadratic pays.** Carrying
`Q(uv) = C_uu·u² + C_uv·u·v + C_vv·v²` beside the basis, with
`Q_{k+1} = J·Q_k + ½H[A_k, A_k]`, and letting the measurement choose:

| set | before | after |
|---|---|---|
| grand julian, 2^20, 1080p | level 2, **20.2 px** total | level 6, **6.6 px** |
| julia dust, 2^20, 1080p | level 7, 0.32 px | level 8, 0.23 px |

The grand julian's deep handover costs twenty times the curvature and
buys four thousand times less f32 error, which is the trade the whole
objective exists to make and which every model of it had refused.

**A gate that bars the curvature bars the trade.** The CPU gates run
`estimate_seeded` in f64, so they see the linearisation and not the
f32 error it was spent on. Holding them to a quarter pixel of
curvature would have forbidden exactly the choice that halves the
total. They now assert against the SUM, with the f32 half computed
the way the objective computes it.

**And a trap worth naming.** `SEED_VEC4S` went from four to six to
make room for `Q`, and the shader's `ifs_seed` kept a stride of four
written as a literal. That reads seed 1 onward out of the middle of
seed 0's words -- invisible on the one-seed nonlinear gate, which
passed at 99.9%, and 11% of the view wrong on a Sierpinski, which
keeps eight. `the_shaders_seed_stride_matches_the_packer` is the
source-scanning gate for it.

## 14. What the prefix costs, 2026-09-16

The self-check of §13 runs ten continuations per level -- five probes
against the level-0 reference -- and `ensure_ifs_seeds` is on the
interactive path, so it is paid on every pan and every zoom step.
Measured per view, at 1080p with a beam of eight:

| zoom | Sierpinski | julia | grand julian |
|---|---|---|---|
| 2^8 | 0.02 ms | 0.11 ms | 0.24 ms |
| 2^20 | 0.17 ms | 1.28 ms | 2.00 ms |
| 2^32 | 0.21 ms | 4.36 ms | 2.36 ms |
| 2^44 | 0.29 ms | 8.23 ms | 3.16 ms |

An affine walk pays none of it -- the check does not run, since an
affine handover approximates nothing at any level -- which is why the
first column is flat. The worst case is eight milliseconds against a
mode-D render measured in hundreds, so it is two to five per cent,
and the cost grows with the handover LEVEL rather than with the zoom,
which is why the grand julian's column stops climbing where its level
does. `probe_what_the_prefix_costs` keeps the measurement.

## 15. The empty beam, 2026-09-16

**Reported from use, within hours of the change that caused it:**
zooming an escape-time grand julian killed the app with
`STATUS_STACK_BUFFER_OVERRUN` (0xc0000409). That code has meant a GPU
driver reset before in this project, which is a misleading place to
start; here it was a panic in a release GUI build, seen from outside.

**The prefix handed over an empty beam.** §10 changed a gapped branch
from "end the prefix" to "carry the gap and skip this branch", which
is right for one branch and wrong for all of them. A grand julian's
inverses have holes, and deep enough into the reference orbit EVERY
branch of every candidate is inside one -- so every branch was
skipped, `live` became empty, the handover shipped zero seeds, and
the continuation indexed `live[0]`.

The walk itself has handled this since the fully-gapped rule of
`ifs-distance-rendering.md` §8.13. The prefix had not, because before
§10 it could not reach the case: the first gapped branch stopped it.
A rule removed is a rule whose job has to go somewhere.

Reproduced by CLI export at zoom 2^40, on the flame from the reports;
the sweep that found it now runs clean to 2^200. The fix is to keep
the beam that got there rather than commit an empty one, and the
continuation additionally answers an empty handover instead of
indexing into it, because the alternative in a render is a panic.
Gate: `a_fully_gapped_prefix_keeps_its_beam`, which fails with the
fix removed and the gate kept -- checked, with the same index-out-of-
bounds the report was.

**What it says about the gates.** Every handover gate ran at zoom
2^28 or shallower, because that is where an f64 direct walk is still
a trustworthy reference. The fully-gapped level arrives at 2^40 on
this set. The new gate goes to 2^200 and asserts what can still be
asserted without a reference: that the prefix kept a beam and that
the answers are finite and non-negative.

## 16. Cost and risk

The CPU pays `beam × maps` inverse evaluations per level, as now, in
`BigFloat` where today they are f64 affines; a rung-1 root costs a
handful of multiplications and one reciprocal at the zoom's limb
count, and the level budget is the same `zoom_log2 + 64`. The shader
pays nothing new.

The risk is the stopping rule's constant: too loose and G2 fails
visibly (a pixel's delta lands on the wrong side of a branch cut,
which is a wrong address, which the address colouring shows as a
seam); too tight and the handover stops shallow and the zoom cap
moves less than hoped. G2 measures it before anything ships.

The ball is a second interaction. The reference orbit is walked from
the centre and can leave the ball at any level; that is state the
seed carries already (`escape`, `done`), and the same rule applies:
the cut is the cut.

## 17. What is next

For a bounded nonlinear set, nothing: the cap is lifted and §7's
table is the evidence. The remaining work is the shader half -- the
seeded start already reads `position + basis·uv` and needs no change
in shape, so what is left is the GPU gate (G5) and a picture (G6).

**For an INVERSION set -- a grand julian -- the handover declines,
and the reason is not what this section first said.** It said the
lever was a wider seed position, a double-float in place of the f32,
on the grounds that f32's error at level 1 is 1e15 pixels. That
guess does not survive its own arithmetic: twenty-four more bits buy
a factor of 1.7e7, and 1e15 needs fifty.

Measured instead (`probe_the_bits_an_inversion_would_need`), the
precision a level needs -- `px·(reach_k/reach_0)/|position|`, whose
negative log is the bits -- on the reported grand julian at zoom
2^20:

| level | 0 | 1 | 2 | 3 | 4 | 5 | **6** | **7** | 8 |
|---|---|---|---|---|---|---|---|---|---|
| bits | 30.3 | 80.5 | 28.3 | 30.7 | 59.2 | 30.5 | **16.5** | **18.6** | 50.6 |

f32 has 24. So the set is not short of precision in principle --
levels 6 and 7 sit comfortably inside it -- and no FIXED width fixes
anything, because the requirement swings sixty bits between adjacent
levels. What is needed is the ability to hand over at a good level
rather than a bad one, and precision costs nothing to pass THROUGH:
the walk crosses levels 1 and 4 in `BigFloat` for free, since only
the level actually handed over at is ever stored in an f32.

**The walk already gets there, and the reason it declines is
CURVATURE.** This section has now guessed twice and been wrong
twice, so what follows is measured rather than reasoned.

Carrying the gap (§10, built) took the handover from level 0 to
level 2 and f32's error from 79 pixels to 19.9 at 1080p. Tracing the
beam per level at 96 pixels then gives the whole picture:

| level | reach / reach₀ | f32 | curvature (model) |
|---|---|---|---|
| 1 | 7.9e-16 | 8.6e15 px | 0.0001 px |
| **2** | 3.94 | **1.72 px** | 0.001 px |
| 3 | 0.75 | 8.98 px | 0.018 px |
| 4 | 2.0e-9 | 3.4e9 px | 0.049 px |
| 5 | 0.85 | 7.98 px | 0.246 px |
| **6** | 1.5e4 | **0.000 px** | **3.75 px** |

Level 6 is where the view has finally expanded -- f32 would be
EXACT there -- and it is refused because the linearisation costs
3.75 pixels. Forcing the handover to each level and measuring the
seeded walk against each pixel's own confirms the model rather than
the excuse: 0.31 pixels of real error at level 5, 3.19 at level 6,
against 0.25 and 3.75 predicted. The reference orbit passes within
4e-5 of a singularity at level 5, and across a view that has grown
by four orders a Jacobian is simply no longer the map.

So neither of this section's earlier answers was the obstacle. Not
precision: f32 is exact at the level that matters. Not the beam: at
96 pixels the handover holds ONE seed, so per-seed levels would
change nothing, and the lineage that expands is the one the beam
already keeps.

**What a grand julian needs is the second-order term.** Carrying
`½ δᵀ·H·δ` alongside the Jacobian takes the dropped term from
`O(ρ/s)` relative to `O((ρ/s)²)`, which at level 6's `ρ/s ≈ 0.055`
turns 3.19 pixels into about a fifth of one -- and level 6 then wins
outright, since its f32 cost is nothing. That is a Hessian per
kernel, a rank-3 tensor per map carried beside the basis, and the
same G1-shaped gate against central differences. It is the real
item, and §2's "the answer is a stopping rule, not a repair" was
written before the sets that need it were measured.

**Still worth doing, and cheap:** widen the seed position, knowing
what it buys -- not the collapse at level 1, but levels that miss by
a few bits, like level 2's 28.3 against f32's 24. It does NOT lift
the no-handover cap, because the shader's walk is f32 throughout and
a delta far below the position's own ulp is swamped by the first
step whatever it was stored in.

## 18. Order of work

1. ~~Jacobians and singular distances for the six kernels, with G1.~~
   Done 2026-09-16; §6 records what it found.
2. ~~`SeedPoint::apply_map` for rung 1 in f64 and BigFloat;
   `seed_beam` with the three rules; G2 at f64 precision.~~ Done
   2026-09-16; §7 records what it found, including that the three
   rules became two plus a choice.
3. ~~G2 at BigFloat precision past zoom 20~~ -- the GPU path seeds
   from `[BigFloat; 2]` and G5 exercises it at 2^20; §8. G3 (the
   handover level against the zoom, and a fixture whose orbit passes
   a pole) is still open.
4. ~~GPU gate G5~~; the app; G6 -- the Douady Rabbit renders with a
   sharp, self-similar boundary at 2^4, 2^12, 2^20 and 2^28
   (`output/deepzoom/sheet.png`).
5. ~~Carry `dead_min` through the handover~~ -- done, §10.
6. ~~A per-seed handover level~~ -- measured to be a non-issue: the
   grand julian's handover holds one seed (§12).
7. ~~The second-order term~~ -- done (§13). The objective stopped
   predicting and started measuring against level 0, and with a
   trustworthy number the quadratic pays: the grand julian's total
   error at 2^20 went from 20.2 pixels to 6.6.
8. ~~Widen the seed position~~ -- dropped, with the reason. It
   cannot help while the shader's continuation is f32 THROUGHOUT: a
   handover position stored to 48 bits is rounded to 24 by the first
   arithmetic the walk does, so two pixels closer together than f32's
   ulp at `|q|` stay indistinguishable however the seed was written.
   What would help is a continuation that works in delta form
   internally, which is a different and larger piece.
9. Rung 2 (sqrt kernels: bubble, hemisphere), same gates.
10. ~~The `estimate_seeded3` staleness §7 left alone~~ -- done and
    measured latent, §12. `seed_beam3` carrying a Jacobian is still
    open, and waits on §11's predictor like its planar twin.
11. ~~G3~~ -- done; the grand julian IS the pole fixture, and the
    gate holds it to its own measured depth rather than the others'.
