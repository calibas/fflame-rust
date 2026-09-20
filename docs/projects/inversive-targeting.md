# Cylinder targeting for the inversive family

**Status: measured, not built.** This is the plan, and §1 is the
measurement that wrote it. Nothing below is agreed yet; §6 lists what
has to be decided before phase 0 starts.

The question this answers: three of the seven hand-picked zoom flames
in `output/flame-zoom/` are refused by cylinder targeting because
their maps have a pole — `spherical` is `p/|p|²`, `julian` with
`dist = -1` is `|p|^(-1/2)` — and no disc that contains the attractor
can avoid the pole. The obvious fix is a region with a hole, an
annulus. The measurement says that is not the fix, and that the three
flames are two different problems.

## 1. The measurement

`scripts/inversive_probe.py` runs the **exact** maps on the CPU —
formulas copied from the shipped WGSL, guards included — not bounds.
Three questions per flame.

### 1a. Is the attractor bounded?

Extent and distance-to-pole against sample size. A bounded attractor
settles; an unbounded one keeps going however many points are drawn.

    spherical        n=20000    |p| max 13.3   gap(xf0) 8.2e-2   gap(xf1) 7.5e-2
                     n=5000000  |p| max 20.7   gap(xf0) 5.5e-2   gap(xf1) 4.8e-2

    grand-julian     n=20000    |p| max 22.8   gap(xf0) 1.9e-3
                     n=200000   |p| max 29.3   gap(xf0) 6.4e-4
                     n=1000000  |p| max 74.6   gap(xf0) 1.8e-4
                     n=5000000  |p| max 307    gap(xf0) 1.1e-5

    julian-disc      n=20000    |p| max 1.13   gap(xf1) 2.4e-4
                     n=1000000  |p| max 1.25   gap(xf1) 6.3e-6

**All three are unbounded in exact arithmetic**, for two different
reasons:

- `spherical.fflame` has two transforms that are pure translations
  (`linear`, `e = ±3`). A translation maps no bounded set into
  itself, so k translations in a row — probability (1/16)ᵏ, never
  zero — reach any distance. The inversions then send those far
  points arbitrarily close to their poles.
- `grand-julian` has no translation. Its cascade is the inversion
  itself: a point within `d` of xform 0's pole maps to radius
  `d^(-1/2)`, and a point at radius 11 maps back to within 1e-3 of the
  pole. Each round of the cascade carries less measure, but never
  none. `julian-disc` is the same with a weak pole (exponent −1/50),
  so its extent grows only logarithmically — bounded in f32, not in
  principle.

A first version of this probe reported an invariant disc-minus-holes
region for `grand-julian`, "no leak in 12,000 samples". It was wrong:
the leak is a patch of area ~1e-6 (the points at radius 11 that land
in the pole's hole) and uniform sampling cannot see it. The
sample-size scan above is what caught it. **Forward-sampled
invariance tests are blind to exactly the leaks inversive maps have.**

So: **no bounded invariant region of any shape exists for any of the
three.** Annulus support, as "find an invariant annulus", cannot work
on the flames it was proposed for.

### 1b. How much measure is in the tail?

If the tail is thin, a region that is invariant *except for a set of
small measure* is still useful: enumerate on it, drop the words that
leave, count what was dropped, show the number. Per hole radius `h`
around each pole: the attractor measure inside the holes, the outer
radius the hole boundary's image then forces, and the measure beyond
that radius.

    grand-julian    h        in holes   R needed   beyond R   total lost
                    1e-2     5.8e-4     10.0       4.4e-4     1.0e-3
                    1e-3     3.0e-6     31.6       3.0e-6     6.0e-6
                    1e-4     1.0e-6     100        1.0e-6     2.0e-6

    julian-disc     1e-4     2.0e-5     1.25       1.0e-6     2.1e-5
                    1e-5     2.0e-6     1.30       1.0e-6     3.0e-6

    spherical       1e-1     6.1e-4     10.3       5.2e-4     1.1e-3
                    3e-2     0          33.6       0          0
                    1e-3     0          500        0          0

`grand-julian` at `h = 1e-3, R = 32` loses six samples in a million.
`spherical` needs no hole at all: the shipped `spherical` has the
`+1e-6` guard in its denominator, which caps its output at 500
everywhere — `SPHERICAL_BOUND` already returns that for a
pole-containing disc — and the measure beyond radius 500 needs ~160
consecutive translations, which is zero for every purpose.

**The tail is thin. A leaky region with an accounted loss is viable
for all three.**

### 1c. Do words contract?

The spectral norm of the composed Jacobian over windows of L orbit
steps. `random2`, a flame that targets today, as the control.

    L         random2     spherical    grand-julian   julian-disc
    1         6.8e-1      9.3e-1       3.7e-1         1.5e+0
    10        3.0e-2      5.6e-2       6.3e-6         7.8e-1
    20        9.0e-4      2.5e-3       5.4e-11        6.1e-1
    40        8.0e-7      5.3e-6       1.0e-20        1.9e-2
    96        —           1.9e-13      9.3e-49        1.3e-5
    200       —           2.2e-26      1.4e-100       3.8e-11

    rate/step  -0.35      -0.30        -1.15          -0.12
    reach 1e-6 by L=96:    99.5%        100%           30%

`grand-julian` contracts three times faster than the control.
`spherical` contracts like the control. `julian-disc` is weak: median
words need ~120 steps to reach 1e-6, `MAX_DEPTH` is 96, and 10% of
its words are still above 1e-3 at depth 200.

## 2. Why the region's shape was the wrong question

Two facts about the bounds, both visible in `src/variations/bound.rs`
once the measurement said where to look.

**A disc that contains a pole never shrinks through the inversion.**
`SPHERICAL_BOUND` on such a disc is the global `D(0, 500)`. Translate
it, invert it again, it is `D(0, 500)` again. The true orbit contracts
at −0.30/step; the disc bound cannot see it because the contraction is
in the fine structure and the disc is the whole plane. A hole fixes
the *first* step only: the image of a holed disc under inversion is an
annulus, the next affine moves its hole off the next pole, and the
next inversion sees a pole-containing disc again. Re-imposing the root
holes at every step (intersect with `R₀`, sound because
`S_w(A) ⊆ A ⊆ R₀ ∪ leak`) keeps `near > 0` — but the Lipschitz
bound over a radius-33 disc with a 0.03 hole is `33/0.03²`, worse than
global. **No disc-shaped bound follows an inversion. An exact circle
image does** — see §3.

**`JULIAN_BOUND` is a global bound, and it has to be.** `julian`
draws a random branch and sends one input to `|power|` blobs spread
around a circle. The union of those blobs is an annulus at the origin
whatever the input's size, so the tightest single disc containing the
output of a tiny input disc is `D(0, w·near^e)` — `O(1)`. The derived
evaluator agrees, for the same reason: `rng_nextf` is `[0, 1)`, so the
angle interval is the full turn. **A word through `julian` cannot
shrink unless the branch is part of the word.**

These two facts split the three flames into two families that need
different machinery, plus one piece both need.

## 3. The design

### 3.0 Shared: a leaky root and an accounted loss

Replaces `invariant_ball` and the per-map `NotContractive` check,
which the measurement shows are unsatisfiable for every inversive
flame and unnecessary for the sound ones.

- The root region `R₀` is an outer disc minus one hole per pole. The
  outer disc and the holes are chosen by policy (§6, decision A),
  not found by a fixed-point search that cannot converge.
- `disc_of` intersects the pushed region with `R₀` after every
  symbol. Sound: the true image `S_w(A)` lies in `A`, and `A` lies in
  `R₀` up to the leaked set. This is what keeps a pole inside a hole
  at every step.
- A word whose region cannot be bounded (a bound declines, or the
  region leaves `R₀` entirely) is **dropped and its probability added
  to `Cylinders::lost`**. The antichain is complete up to `lost`, the
  render is unbiased up to `lost`, and the panel shows `lost`.
- The kernel counts, with one more atomic beside the frame-coverage
  pair, the fraction of *actual* samples that land in a hole or
  beyond the outer disc. That is the measured leak, from the real
  render, and it is the honesty gate: the panel shows it, and a
  picture gate asserts it agrees with the enumeration's `lost` to
  within sampling error.
- Subtree pruning ("this word's region misses the view, expand it no
  further") stays sound without invariance, because the true images
  nest — `S_{aw}(A) ⊆ S_w(A)` always — even when the computed regions
  do not. Only termination relied on invariance, and termination is
  the caps.

### 3.1 Family M — Möbius-exact regions (`spherical.fflame`)

`spherical` is inversion in the unit circle (`1/z̄`); an affine that
is a similarity is `az + b`; a composition of these is a Möbius or
anti-Möbius map, and **Möbius maps take circles to circles exactly.**
A disc containing the pole maps to the *exterior* of a circle.

- Region type: a **generalized disc** — a disc, or the complement of
  one — plus holes, all circles.
- For a transform that is `spherical` over a similarity affine, or
  `linear` over any affine: push the region through **exactly**, no
  bound, no looseness. This is the DFS of *Indra's Pearls*, which is
  cylinder targeting for a Möbius IFS.
- The `+1e-6` guard makes the shipped map inexact by `ε/|q|³`; with a
  hole of radius 0.03 that is 4e-2 absolute at the hole boundary,
  1e-3 relative. Inflate the image circle by it. Sound and small.
- A non-similarity affine (`σ_max ≠ σ_min`) turns a circle into an
  ellipse; bound it by the circle of radius `σ_max·r`. Looseness
  `σ_max/σ_min` per such step, a constant factor, the same class as
  the 7%/level the derived bounds already pay.
- Words then shrink at the true rate, −0.30/step: `spherical.fflame`
  reaches a 1e-6 view by depth ~50.

This covers every flame built from `linear` and `spherical`, which is
the whole "Kleinian" style, not just one file.

### 3.2 Family J — branch-resolved words (`grand-julian`, `julian-disc`)

- The alphabet becomes `(transform, branch)` for any transform
  carrying a random-branch variation (`julian`, `juliascope`,
  `julian3D`, …), with probability `p_a / |power|`. `grand-julian`'s
  branching factor goes from 3 to 25; at −1.15/step the antichain is
  still small.
- The replay kernel **forces the branch**: `pack_words` carries it per
  symbol, and the variation's WGSL reads
  `select(floor(|power|·rng_nextf(rng)), forced, forced ≥ 0)`. The
  select compiles out under the existing template flag, so untargeted
  shaders stay byte-identical (the shader-dump gate enforces this).
- With the branch fixed, the derived evaluator sees `rng_nextf` as a
  point, the angle interval is `(θ ± δ)/power`, and the output box is
  one blob — tight, and shrinking with the input. `JULIAN_BOUND`'s
  global form is kept as a fallback (`min` of the two, as
  `BUBBLE_BOUND` already does), and its `near` comes from the hole:
  `w·h^e`, finite.
- Everything else is family M's machinery with a bound in place of an
  exact circle: holes are under-approximated (affine by `σ_min`; a
  hole not at the pole is dropped through an inversive map, which is
  sound), the root holes are re-imposed by intersection.

`grand-julian` is then a two-day gate. `julian-disc` additionally
needs depth ~200 (§3.3).

### 3.3 Depth for weak contraction (`julian-disc`)

`MAX_DEPTH = 96` is arbitrary; `julian-disc` wants ~200. Raising it is
one line. The cost is not: `disc_of` recomputes each candidate from
the root (§ "words extend on the inside", `cylinder.rs`), so a
candidate at depth 200 costs 200 bound evaluations, and the frontier
cap is 4096. Measure first with the perf meter; if it is seconds per
pan, the fix is to make `Bounder::apply` for hand-bounded variations
allocation-free (~1 µs) before touching the cap. Deferred until
families M and J are in, since it is the one flame of the three that
is marginal on the physics rather than blocked on the machinery.

## 4. Phases, each ending in a picture gate on a real flame

0. **Leaky root + accounted loss + kernel leak counter.** Gate: every
   flame that targets today still passes
   `a_targeted_render_is_the_untargeted_render` with `lost = 0`, and
   a hand-built flame with a deliberate 1e-3 tail shows `lost` and
   the kernel counter agreeing to sampling error.
1. **Family M.** Gate: `spherical.fflame` enumerates at its saved
   framing and 1e6 deeper, and the targeted render matches the
   untargeted one to within `lost`. A second gate on an
   Indra's-Pearls-style two-inversion flame with a known limit set.
2. **Family J.** Gate: `grand-julian` enumerates and matches; the
   shader dumps for every untargeted flame are unchanged
   (`canonical_shader_dumps`); `derived_bounds_contain_the_real_wgsl`
   extended to run `julian` branch-fixed and confirm containment
   per blob.
3. **Depth.** Measure, then decide.

Rough cost, if the decisions in §6 go the recommended way: phase 0 a
day, phase 1 two days, phase 2 two to three days (the kernel change is
the risk), phase 3 a day. Five to seven working days for the three
flames, and family M covers a whole style, not one file.

## 5. Not in this plan

- Any variation with a pole on a **curve** rather than at a point
  (`curl`, `rays`, `cross`). A hole is a disc; a curve-shaped hole is
  a different region type. The corpus meter names these already.
- 3D. Both `linear3d-*` flames are 3D-mode and need a z-semantics
  decision, not geometry. Separate, and lower priority per the owner.
- Overlap (`random1`, similarity dimension 3.25). Not a region
  problem; the words genuinely multiply.

## 6. Decisions before phase 0

**A. The leak policy.** Fixed hole radius (1e-3 of the attractor
scale, `R` from the hole boundary's image) versus a target loss
(shrink holes until the kernel counter reads below 1e-5). Recommended:
fixed, exposed in the panel as an advanced control, with the measured
leak shown beside it — one render decides, no loop.

**B. Whether `lost > 0` is ever hidden.** Recommended: never. It is
one number in the targeting status line, and it is the difference
between "unbiased" and "unbiased up to this".

**C. Order of families.** M first is recommended: it is exact, it
needs no kernel change, and `spherical`-style flames are common. J
second, because forcing the branch touches the shader.

**D. Which random-branch variations get the forced branch in phase 2.**
`julian` alone is enough for both files. `juliascope` and `julian3D`
are the same edit.

## 7. What the probe is for afterwards

`scripts/inversive_probe.py` stays. Run it on any flame that targeting
refuses and it says which of the three it is: unbounded attractor
(§1a), thick tail (§1b), or weak contraction (§1c). Those are the
three ways a flame can be outside this design, and each has a
different answer.
