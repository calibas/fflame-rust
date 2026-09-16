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

**Three stopping rules, then, in the order tested per level.**

1. *Linearisation budget*: `E + ρ_k / s_k > τ` for any live
   candidate. Near a pole `s_k` is small and the handover stops early
   -- the view continues in f32 from there, which is what it does
   today from level 0, so the failure mode is "no deeper than now",
   never "wrong".
2. *A gap edge within reach*: a candidate whose pre-frame position
   is within `ρ_k` of a hole's or the unit disc's edge, since pixels
   on the two sides take different branches (one gapped, one not).
   Same shape as the view-agreement rule.
3. *View agreement* as now, with each branch's reach widened by its
   curvature allowance `ρ_k · (1 + ρ_k / s_k)`.

The affine rule `basis_reach ≥ radius · HANDOVER_FRACTION` stays as
the outer cap.

## 3. What each kernel has to supply

Two closed forms per kernel, beside the inverse and the local σ it
already has in [`ifs_analysis.rs`](../../src/scene/ifs_analysis.rs):

| kernel | inverse on `v` | `J` of the inverse | singular distance |
|---|---|---|---|
| Root `{n, d}` | `\|v\|^{\|n\|/d} e^{i·n·arg v}` | in the (radial, tangential) frame at `v`: radial `(\|n\|/d)·\|v\|^{\|n\|/d − 1}`, tangential `n·\|v\|^{\|n\|/d − 1}`, rotated by `(n − 1)·arg v` | `\|v\|` (the pole) |
| Spherical | `v / \|v\|²` | `(I − 2 v vᵀ/\|v\|²) / \|v\|²` | `\|v\|` |
| Bubble (branch b) | `v · f_b(\|v\|²)/\|v\|²`, `f = 2 ∓ 2√(1 − \|v\|²)` | `f/\|v\|² · I + v vᵀ · d(f/\|v\|²)/d(\|v\|²) · 2` | `1 − \|v\|` (the disc's edge) and, for the outer branch, `\|v\|` |
| Hemisphere | `v / √(1 − \|v\|²)` | `t·I + t³ v vᵀ`, `t = (1 − \|v\|²)^{−½}` | `1 − \|v\|` |
| Disc (ring m) | polar → `(r sin θ, r cos θ)` with `r = φ/π + m`, `θ = ±π\|v\|` | chain rule through `(ρ, φ)` | `1 − \|v\|` and the ring's cut `φ = ±π` |
| Blob | `(v_y, v_x) / s(θ)` | `J = (1/s)·P − (s'/s²)·(swap v)·∇θᵀ`, `P` the swap | `\|v\|` (θ's pole) and where `s(θ) → 0` |

`NonlinearMap2::inverse_jacobian(q)` is then
`pre_inv.m · J_K(before_kernel(q)) · post_inv.m / w`, and the
existing `local_sigma_factor` is its smallest singular value at the
same point -- a consistency check for free (G1).

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

- `SeedPoint` gains `apply_map(&self, m: &Map2) -> Self`, implemented
  for `[f64; 2]` by `apply_inverse` and for `[BigFloat; 2]` by the
  rung-1 rational forms (rung 2 with `sqrt`). A map outside the
  implemented rungs makes `seed_beam` stop at level 0, as now.
- `seed_beam`: the `!affine` break becomes the three rules of §2; the
  child step uses `apply_map` and `inverse_jacobian`; `sigma` is the
  product of local factors, which `Map2::step` already returns.
- `Seed`, `Seeds`, `pack_seeds`, `MAX_SEEDS`, the `fdata` layout, the
  shader's seeded start, `estimate_seeded`: **unchanged**. The
  nonlinear handover produces the same fifteen numbers per seed.
- The 3D twin: `seed_beam3` / `compose3` take a 3×3 Jacobian the
  same way; Quaternion's inverse `qⁿ + c` is a polynomial (rung 1),
  Root3/RootZ3 are rung 1 when `n` is an integer power.

## 5. Gates

- **G1** Jacobians: for every kernel and branch, at 1,000 random
  points inside the image, `J` against central differences at 1e-6
  relative; and its smallest singular value equals
  `local_sigma_factor` to 1e-9.
- **G2** The seeded walk is each pixel's own: `estimate_seeded`
  against `estimate` from the pixel's own f64 position, on the
  shipped nonlinear presets at zooms 0, 5, 10, 15, 20 (the last past
  today's cap), beams 1 and 4: distance within 1e-3 px, address
  identical. This is `a_seeded_walk_answers_what_a_direct_one_does`
  widened to nonlinear sets, and it is what sets `τ`: the largest
  `τ` at which it passes with margin.
- **G3** The handover level tracks the zoom on a nonlinear set, as
  `the_handover_level_tracks_the_zoom` shows for affine ones, and
  stops early on a view whose centre orbit passes a pole (a fixture
  built to do so).
- **G4** Byte identity: every affine mode-D preset renders
  byte-identical (`output/ifs/preset-*.png` against the current
  set); the affine path is not touched.
- **G5** GPU: `the_seeded_walk_is_each_pixels_own` on a nonlinear
  preset -- the shader from the CPU's seeds against the shader from
  level 0, at a zoom under the cap.
- **G6** The picture: a grand julian at zoom 40 rendered from the
  app shows structure, not blocks, and the address colouring is
  continuous across the frame.

## 6. Cost and risk

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

## 7. Order of work

1. Jacobians and singular distances for the six kernels, with G1.
2. `SeedPoint::apply_map` for rung 1 in f64 and BigFloat; `seed_beam`
   with the three rules; G2 at f64 precision on the grand julian.
3. G2 at BigFloat precision past zoom 20; G3.
4. GPU gate G5; the app; G6.
5. Rung 2 (sqrt kernels), same gates.
6. The 3D twin.
7. Record here what `τ` came out as and what the cap became.
