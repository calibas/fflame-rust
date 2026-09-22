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

   *Done, 2026-09-20.* The counter is `GpuParams::leak_probe` plus
   `coverage[2]`: an ordinary untargeted render counts, in world
   space, every plot attempt outside a claimed disc. Calibrated by
   `the_leak_probe_agrees_that_the_root_ball_holds_everything` in both
   directions — exactly zero at the gasket's root ball, 97.9% at a
   quarter of its radius. That zero is itself new information:
   `invariant_ball` proves its answer with forward BOUNDS, and until
   now nothing had checked the conclusion against the shader that
   actually runs.

   It also caught the transient immediately: at `burn_in = 0` the
   probe reads 6.1e-3, because the chaos game starts at a random point
   and the first iterations of every thread deposit points genuinely
   outside any invariant region. Real, and not a hole in the ball.

   *Accounting done, 2026-09-20.* `Cylinders::lost` is plumbed to the
   panel and `every_working_flame_loses_nothing` holds it at zero over
   64 successful enumerations. Note what that gate also says: **no
   flame can produce a non-zero `lost` today**, because a transform
   that fails to bound is refused by `plan` up front rather than
   dropped word by word. So the cross-check against the kernel counter
   has nothing to measure until phase 1 builds the leaky region that
   creates the loss. The counter is therefore built and tested against
   a *deliberately undersized* root disc on a flame that works, which
   validates the instrument on a known answer before phase 1 relies
   on it.
1. **Family M.** *Geometry built and measured, 2026-09-20; the root
   policy is BLOCKED on a decision — see §8.* Gate: `spherical.fflame` enumerates at its saved
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

**Settled 2026-09-20; the recommendation was taken in all four cases.**
A: fixed hole radius, shown in the panel beside the measured leak.
B: `lost` is never hidden. C: family M first. D: `julian` alone in
phase 2.


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


## 8. Family M: the geometry works, the root does not

**Built and tested:** `src/scene/mobius.rs`. `Region` is an outer disc
minus a hole per pole, closed under similarities and under inversion,
with the pole's hole turning into the next outer disc. Seventeen
tests, of which two carry the weight: containment against the
**shipped** body (guard included), and `inverting_twice_is_the_identity`
— an involution, which only an exact map survives. `detect` reads the
four maps of `spherical.fflame` with their poles.

**And it does not enumerate**, for a reason worth writing down.

### The measurement

Walking a random weighted word of 200 symbols from the root, against
the hole radius:

    hole    min region radius     leaked measure
    0.01        2.62e1  pinned        0
    0.05        2.00e1  pinned        0
    0.10        1.00e1  pinned        8.3e-4
    0.15        6.67e0  pinned        9.6e-3
    0.20        5.05e-8 CONTRACTS     3.0e-2
    0.25        1.39e-7 CONTRACTS     5.7e-2
    0.30        1.68e-3               1.2e-1

Below the transition the region is **pinned at `~1/h`**: it holds
points within `h` of the pole, they map out to `1/h`, and that happens
again at every step forever. Above it the region finally misses the
pole's neighbourhood, and eight orders of contraction appear at once.

The attractor comes within **4.8e-2** of the nearer pole. So the holes
that make the enumeration work are holes that cut the attractor, and
the leak column is the price: **3% of the picture** at the transition.

### Why this is not forced

The true cylinder images shrink perfectly well. Taking 40,000
attractor points and applying a random word to all of them:

    trial 0:  5:1.40e1  15:7.80e0  25:4.92e-2  35:1.18e-4
    trial 2:  5:1.02e1  15:4.49e0  25:2.87e-2  35:2.33e-5
    trial 3:  5:1.01e1  15:5.98e0  25:9.57e-3  35:1.89e-4
    trial 1:  5:9.70e0  15:1.78e0  25:1.06e0   35:6.36e-2   (slow one)

From 13.8 down to 1e-4 by depth 35, with nothing cut. **The flame is
enumerable; one disc minus a few holes is just too coarse to follow
it.** `S_w(A)` shrinks because `A` is invariant and fractal; `S_w(R₀)`
does not, because `R₀` is a disc-with-holes whose near-pole annulus is
attractor-free space that blows up to `1/h` every step.

### The fork

**(a) Accept the loss.** Set the hole at the transition — about
`1.5e-2` of the extent for this flame, not the `1e-3` decision A
assumed — and let the panel say `MISSING 3.0% of the fractal`. Cheap:
the remaining work is wiring `plan`, perhaps a day. But 3% is a lot to
lose, and the transition point is flame-specific, so a fixed fraction
is the wrong knob — the measurement has overtaken decision A.

**(b) Cover the attractor instead.** Replace the single root region
with a **union of M small discs** fitted to the measured orbit. Then
`S_w(A) ⊆ ⋃ S_w(D_j)`, every `D_j` is small enough to be in the
map's linear regime, and the bound tracks the true contraction with
nothing cut. Costs M pushes per word — at M ≈ 64 and 4096 words that
is ~250k pushes of a few microseconds, which is affordable. It is a
different root, not a different geometry: everything in `mobius.rs`
is reused unchanged.

(b) is the one that actually delivers deep zoom on these flames, and
it is maybe two to three days rather than one. (a) is a day and ships
a compromised picture.

Nothing is wired into `plan` yet, deliberately — the root policy is
the part in question, and building the integration against a policy
that is about to change would be work done twice.


## 9. Family M, option (b): the cover works

**Chosen 2026-09-20, built and measured.** Nothing cut, eight orders
of contraction, and a performance problem that is now the only thing
between this and a render.

### The numbers

Covering `spherical.fflame`'s attractor and pushing along random
80-symbol words:

    cover of 246 discs, extent 1.427e1
    seed 7:     80 steps   20:1.34e-2  40:1.41e-4  60:7.25e-6  80:9.32e-7
    seed 99:    80 steps   20:5.48e0   40:1.32e-3  60:8.55e-7  80:8.11e-5
    seed 12345: 80 steps   20:4.34e-3  40:7.08e-6  60:2.87e-6  80:3.33e-6
    seed 555:   80 steps   20:2.31e-1  40:6.51e-4  60:7.66e-5  80:7.27e-7

    attractor covered to 2.1e-3

Compare §8: the single region managed **nothing** without cutting 3%
of the picture away. This tracks the true cylinder image, and the
2.1e-3 is sampling residue rather than fractal deliberately discarded.

### Three things the measurements forced

**The cover must be sized by distance to the nearest pole.** A uniform
cover of 48 discs gives discs about 2 across while the attractor
passes within 4.8e-2 of a pole, so the near-pole discs swallow it and
the first push refuses. Measured: `ReachesPole` at step 1, every seed.

**The same constraint has to survive merging.** Merging 246 pole-aware
discs down to 48 rebuilt precisely the discs the construction had
avoided, and the walk died at step 2. `CoverRules` is carried so that
every operation respects it, and merging stops when nothing legal
remains — the cap is a target, not a guarantee.

**Refinement has to follow the attractor, not the disc.** Splitting a
disc geometrically (seven pieces at 0.65 of the radius) needs about
ten levels to refine by a hundred, which is 7¹⁰ pieces; the budget was
gone before the first symbol finished. The attractor near a pole is a
thin fractal, not a filled disc, so the cover carries a thinned orbit
sample and refines onto the points. A piece holding no sample point is
dropped — an honest leak, of the kind `Cylinders::lost` and the leak
probe exist to report.

### The performance problem

Greedy merging (globally tightest pair) is quadratic per merge and
cubic overall, and refinement can hand it thirty thousand discs: **129
seconds** for one 80-symbol walk. Ordering by a coarse grid and
merging along it is `n log n` per pass and halves the count each time:
**2.0 seconds** for four such walks, with the same contraction and the
same coverage. 65× and the results are unchanged.

That is still about 6 ms per push, and `plan`'s `disc_of` recomputes
from the root for every candidate — `M × depth` pushes each. Too slow
for a pan by a wide margin.

**The fix is already implied by the geometry.** Every family-M map is
`z ↦ (a·ẑ + b)/(c·ẑ + d)`, so a WORD composes to a single such map: a
2×2 complex matrix and a conjugation parity, composed in O(1) per
symbol. A child word is then the parent's matrix times one more
factor, `disc_of` becomes one cover push instead of `depth` of them,
and the cost per candidate drops from `M × depth` to `M`. This is the
same trick `pack` already plays for the affine case, where a word
becomes one matrix rather than a sequence the kernel walks.

### What is left

1. Möbius composition, so a word is one map. (The performance fix.)
2. Wire into `plan`: family-M detection, the cover as the root, and
   `lost` for words the refinement budget cannot save.
3. The picture gate: `spherical.fflame` enumerating at its saved
   framing and deeper, matching the untargeted render to within the
   reported leak.


## 10. Composition: a word is one map

**Built and measured, 2026-09-20.** The performance fix §9 asked for.

Every family-M map is `z ↦ (a·ẑ + b)/(c·ẑ + d)`, and so is every
composition of them, so a word is a 2×2 complex matrix and a parity
bit built one multiply per symbol rather than a sequence to walk.
`disc_of` costs one cover push per candidate instead of `depth` of
them.

    8 words of 30-50 symbols
    walked     697.0 ms
    composed    30.4 ms    (23x)

### The guard nearly sank it

The composed map is an exact Möbius map. The shipped one is not —
`spherical` is `p/(|p|² + 1e-6)` and no Möbius map carries that term.
Over twelve symbols the two agree to 2e-4 relative and nothing
notices. Over sixty, the shipped maps put points **842 radii outside**
the composed cover.

The reason is worth keeping: it is not that the error grows. It is
that the **disc shrinks by eight orders while the error does not**.
Injected at the last inversion, nothing afterwards contracts it.

So the discs go through the composed map, which is what makes this
fast, and the sample points go through the **real maps, symbol by
symbol**, which is cheap because they are points — then each image
disc is grown to hold the images of the points that were inside its
source. Tying each disc to its OWN points rather than to whatever
image is near it was worth five to five hundred times in tightness.

`composed_cover_contains_the_shipped_images` is the gate: 119,616
shipped images against composed covers over words of 4 to 64 symbols,
worst margin **0.0 radii outside**. It reports the margin rather than
only asserting containment, so the headroom is visible if the guard or
the policy ever changes.

The composed path comes out about five times looser than walking — one
map over the whole cover against a refinement at every symbol. That is
two or three extra levels of depth for an order of magnitude of time.

### What is left for phase 1

Wiring into `plan`: family-M detection, the cover as the root, the
composed matrix carried per node, and `lost` for words the refinement
budget cannot save. Then the picture gate — `spherical.fflame`
enumerating at its saved framing and deeper, matching the untargeted
render to within the reported leak.


## 11. The analytic bound, and why the sample stays

The point walk in §10 costs `points × depth` per candidate, which
projected to seconds per pan. The obvious replacement is to BOUND the
guard rather than sample it:

```text
E_child  =  E_parent  +  Lip(M_parent) · e_a
```

which is right, and useless. `Lip` is a worst case over the whole
region while the dynamics only contract on AVERAGE, so the product
grows like `L^depth` where the truth shrinks. Measured on
`spherical.fflame`: `Lip = 1` at the first symbol, **96.5** at the
second, and at the third the composed map's pole had moved inside a
cover disc and it was infinite. The error bound had already reached
0.49 world units while the region was still 14 across.

Measuring `Lip` over the cover's own discs rather than the disc
enclosing them bought exactly one symbol — the enclosing disc holds
the poles and the cover does not, but the composed pole moves, and it
soon lands inside a cover disc anyway.

Recorded rather than deleted, because it is the obvious idea and the
next person will have it too.

## 12. The gate was circular, and the honest one has a leak

`push_word` anchors its discs onto the cover's **own** sample, so
checking that sample back is very nearly tautological. Re-run against
attractor points drawn from an independent seed, the picture changes:

    sample  20000, anchor 4000:   5.8e-4 of unseen images outside
    sample   1000, anchor 4000:   3.0e-2

So the cover does leak, and the leak is sampling residue — attractor
that falls between the points. Which is what this design has said all
along it would do; it had simply never been measured, because the
measurement had been asking itself.

The dial, at 256 discs, against points the cover had never seen:

    anchor points   ms/word     leak
             4000     3.441   7.9e-5
             1000     0.850   8.6e-4
              400     0.363   1.4e-3
              150     0.154   4.6e-3

Roughly `leak ∝ 1/points` against `cost ∝ points` — no knee to find,
only a price to pick. **1000 points**: 0.09% of the picture missing,
under a millisecond a word. At a few hundred candidates that is a few
tenths of a second per plan, which is a deep-zoom mode rather than a
pan.

`a_words_region_holds_the_shipped_images` now draws its validation
from a different seed and asserts the LEAK is small rather than
pretending it is zero, which is the only version of the question
worth asking.

## 13. Still to do for phase 1

Wiring into `plan`: family-M detection, `MobiusFlame` as the root, the
composed matrix carried per node, and the sampling leak reported
through `Cylinders::lost` — it is not word-dropping loss, so it may
want its own field rather than to be folded in. Then the picture gate
on `spherical.fflame`.


## 14. Wired into `plan`, and what it costs

`Cylinders::plan` now tries `MobiusFlame::read` first, and when a flame
is family M it skips the affine check, the invariant ball and the
contraction check — none of which can succeed for an inversion — and
enumerates on the cover, carrying each node's word as one composed
matrix.

`spherical.fflame` gets past `NoInvariantBall` for the first time.

### Three things that had to be fixed to get there

**Family M must require an inversion.** `detect` accepts `linear` over
a similarity, so a plain affine flame — a gasket — reads as a Möbius
IFS, and it silently started taking this path: sampled cover, leak,
cover push per node, in place of an exact disc bound and a matrix
multiply. Caught by the gasket's speedup curve moving and by a flame
that should be refused no longer being refused.

**The view test has to see the cover, not the disc around it.** A
family-M word's region is a scatter of small discs; the disc enclosing
them is the attractor's own extent for the first twenty symbols.

**The frontier has to be a beam.** Measured on `spherical.fflame`
centred on its own attractor, the frontier went 4, 16, 63, 240, 887,
3113, 10752, 35696 — the branching factor to the depth, with almost
nothing pruned — while the widest region GREW from 17 to 2.8e5.

Both halves of that are real. Words contract on AVERAGE, which is what
the Lyapunov exponent says and what a random weighted word does; the
enumeration walks ALL of them, and the expanding ones stay large, keep
meeting the view, and keep branching. What saves it is that they carry
almost no measure, so the frontier keeps the most probable `BEAM` and
charges the rest to `lost`.

The beam is family-M only. Everywhere else an overfull frontier is
still `TooManyWords`: for a flame with a real invariant ball it means
the view straddles more pieces than the antichain can hold, and
answering that with a beam would turn a clear refusal into a picture
quietly missing most of itself.

### Where it stands

    spherical.fflame, its own framing:
      zoom x1e0    lost 9.927e-1
      zoom x1e3    lost 8.483e-2
      zoom x1e6    lost 8.483e-2
      zoom x1e9    lost 8.483e-2

    on a hand-picked attractor point, beam 96:
      36 s per plan, 96 words, depth 96, lost 9.76e-1

**8.5% of the picture missing at depth, and tens of seconds per plan.**
Not shippable. The geometry is done and sound; the enumeration is not.

### The open question

Is the 8.5% the cover's looseness or the flame's structure? Counting
the distinct length-k words that carry orbit samples into the view
gave 18 at depth 8 and ~30 at depth 20 — which would be tiny, and
would mean the bound is loose by orders — but only 32 of four million
samples reached the view, so the count is a lower bound on nothing
much. Settling it needs a run with enough samples IN the view, which
means a bigger view or a much longer run.

That measurement decides the next move: a tighter region (the cover's
resolution is the dial) or a different enumeration (best-first by
probability rather than breadth-first, which matches the fact that the
measure concentrates on typical words while the count does not).


## 15. The open question, settled: it is the measure, not the bound

§14 left one question — is the 8.5% the cover's looseness or the
flame's structure? Three measurements, and the answer is both, with
the second one decisive.

### The bound is loose, and resolution barely helps

Words of length k whose region meets a view, against words the orbit
actually uses to reach it:

    view radius 3e-2 (1.5e-3 of the measure)
      k   words    truth   bound   looseness
      2      16       12      16         1.3x
      4     256       75     241         3.2x
      6    4096      220    3155        14.3x

Loose, and worsening with depth. But it is not the cover's
resolution: raising `MAX_COVER` from 256 to 2048 changed **nothing**
(the greedy build stops when the sample is covered, so the cap was
never binding), and shrinking `COVER_ALPHA` by ten — ten times smaller
discs — moved 13.9x to 10.1x. A 27% gain for an order of magnitude of
work.

What the extra words are is not a mystery: they are words whose region
genuinely reaches the view and whose measure is negligible. The bound
is not wrong about them. There are simply a lot of them.

### The measure does not concentrate

Which is the real obstacle. For the same view, how many words hold 90%
of the measure that reaches it:

    depth   distinct words   words for 90%
        2               13               3
        4               79              10
        6              258              41
        8              567             146
       10             1183             579
       12             2422            1818
       16             5404            4800   (6046 samples; saturating)

It roughly triples every two levels and never settles. Cylinder
targeting exists because for an ordinary flame this number is ONE —
the view sits inside a single cylinder, and forcing that word puts
every sample in frame. Here the view's measure is spread over
thousands of words by the depth its cylinders reach the view's size.

That is the same wall `random1` hit with its similarity dimension of
3.25, reached from the other direction: `spherical.fflame` has two
pure translations, its attractor is unbounded, and its pieces overlap
so heavily that a point has no address worth forcing.

### The control was degenerate

A Schottky configuration — inversions in four mutually disjoint
circles — should be the opposite extreme and the fair test of the
machinery. It is not, because `spherical` makes each map an
**involution**: `S_i ∘ S_i` is the identity, so the enumeration's
words fold in on themselves and the regions oscillate instead of
shrinking:

    depth      1      2      3      4      5      6      7       8
    radius  2.47   28.9   20.2   56.9   32.1   0.46   5.18  2.3e-4

A real Schottky test needs loxodromic generators, which a flame cannot
express with `spherical` alone — `spherical` is inversion in a circle,
and nothing else here composes two of them into one transform.

### Where that leaves family M

The geometry is right, exact, and gated. The enumeration on top of it
is not useful yet, and the reason is not a defect in the geometry:

- for `spherical.fflame`, the measure does not concentrate, and no
  bound however tight fixes that;
- the one flame family that WOULD concentrate cannot be built from the
  variations that exist, because they give involutions.

So the next move is not more geometry. It is either a flame that suits
the machinery — which may mean a new variation, a Möbius transform
with two parameters rather than a bare inversion — or accepting that
the inversive corpus flames are targeted partially, with the missing
fraction reported, and deciding whether 8.5% is a picture worth
drawing.


## 16. Turned off, and the variation that was there all along

**Reported from the app: the window freezing for up to a minute on
`spherical.fflame` with targeting on.** That is this work. `plan` runs
on the UI thread on every pan and every zoom, and the family-M path
costs 30 to 36 seconds — which §14 measured and I wired in anyway.

`mobius::ENABLED` is now `false`, gated in `plan` so the module stays
fully exercised by its own tests. The flame goes back to refusing in
**0.6 ms** with a reason the panel prints, which is a better answer
than a frozen window and a fractal missing nine tenths of itself.

### The correction that matters

§15 concluded that a fair test of family M needs a loxodromic
generator, which "no shipped variation provides". **That was wrong,
and asserted rather than checked.** `mobius` has been in the registry
the whole time — `src/variations/defs/extended.rs`, eralex61's, body
exactly `(Az + B)/(Cz + D)` with eight real coefficients. It is now a
`Kind::Mobius` and `detect` reads it, poles and all.

What is true is narrower: **no corpus flame uses it.** So the
machinery has a customer that could exist and currently does not.

### And the fair test still fails

Two attempts, both instructive.

A group built from loxodromic matrices chosen by their TRACES: regions
oscillated (21.3, 21.4, 50.2, 21.4, … 582, …) exactly as the
involution flame's had. The Schottky condition is not about traces.
For `g = (az+b)/(cz+d)` with `ad − bc = 1`, `g` has its isometric
circle at `−d/c` and `g⁻¹` at `a/c`, both of radius `1/|c|`, and the
group is Schottky when all four are mutually disjoint. Mine were
`[−2,0]`, `[1,3]`, `[0,1]`, `[−1,0]` — overlapping.

A genuine one, circles at `±2` and `±2i` of radius 1, pairwise 2.83
apart, both generators of determinant 1 and `|trace| = 4`:

    g1 = [[2, 3], [1, 2]]       g2 = [[2i, −5], [1, 2i]]

The cover is tight — 12 discs, leak **0.000** — and the regions still
do not shrink: 4.56, 19.6, 12.0, 19.6, 4.56, 12.6, 4.56, 21.4, 13.4,
38.7, 13.4, 13.3, 12.4, 13.3. `lost` 0.959.

So the obstacle survives even a textbook Schottky group, which means
it is not the flames and not the geometry. The remaining suspect is
the enumeration itself: the IFS contains each generator AND its
inverse, so `[g, g⁻¹]` is the identity and every other word folds back
to the whole attractor. A Schottky enumeration walks REDUCED words
only; this one walks all of them.

That is a concrete, testable next step — and it is a change to the
word expansion, not to any of the geometry below it.


## 17. The measurement that should have come first

`scripts/inversive_probe.py --localize`: run the real maps, and for
samples landing in a view, count the distinct words (last k symbols)
that carry them — raw, and **reduced** with adjacent inverse pairs
cancelled, the pairs found numerically rather than assumed. Four
million steps per flame, two view centres, three radii. The number
that matters is "words for 90% of the view's measure" at the depth
where a typical cylinder is the view's size (`λ·k = ln(r/extent)`).

    control (random2, 2 affine maps, λ = −0.35)
      r=1e-2, matched depth ≈ 7:   depth 6 → 1 word,  depth 8 → 4

    spherical.fflame (λ = −0.30, core extent ~14)
      inverse pairs found: only the translations, S3∘S2 = S2∘S3 = id
      r=1e-1, matched depth ≈ 16:
        depth   raw for 90%   reduced for 90%
          8           237               169
         10          1473              1064
         12          6835              5659
         16         22579             22287   (29,023 samples: saturated)

    grand-julian (λ = −1.15, core extent ~25; words are (map, branch))
      no inverse pairs (julian is many-valued)
      r=1e-1, matched depth ≈ 5:   depth 4 →  48,  depth 6 → 1979
      r=3e-2, matched depth ≈ 6:   depth 4 →  14,  depth 6 →  443
      r=1e-2, matched depth ≈ 7:   depth 6 →  43,  depth 8 →  396
      (second centre: 90, 2786 / 3, 126 / 1, 34, 308)

Three verdicts.

**The control localizes**: one to four words at the matched depth,
bounded as the view shrinks. That is what cylinder targeting needs
and it is what an ordinary flame gives.

**`spherical.fflame` does not, and reduction does not rescue it.**
Cancelling the translation pair removes about a quarter of the words
and none of the growth; the two inversions are not self-inverse once
their affines are in front of them (checked numerically: no
`S0∘S0` or `S1∘S1` pair). Two levels ABOVE the matched depth the
measure already needs thousands of words where the control needs one;
at the matched depth it needs more than the sample can resolve. The
fold-back hypothesis of §16 is rejected for this flame. Its overlap is
genuine, and this — not the invariant ball, not the bound — is why
making it more contractive only trades `NoInvariantBall` for
`TooManyWords`.

**`grand-julian` sits in between, and may be workable.** Tens to a few
hundred branch-resolved words hold 90% of the view's measure at the
matched depth, and — the part that matters — that figure is of the
same order across three view radii spanning a decade. A bounded
antichain of order 10²–10³ is inside `MAX_WORDS`. It is an order of
magnitude worse than the control and an order better than
`spherical`.

### What this means for the code

`mobius.rs` has no customer. The flame it was built for cannot be
targeted by any enumeration, reduced or not, and the Schottky flames
that would suit it are not in the corpus. The reusable part is the
`Cover` (adaptive, pole-aware, refined onto the sample); the Möbius
composition and the abandoned `Region` are not. Recommendation: prune
to `Cover` or remove outright — the doc keeps the findings and git
keeps the code.

The viable target is **family J**, `grand-julian`, whose blocker was
never geometry: `julian`'s random branch means a word cannot shrink
unless the branch is part of it (§2), which is a kernel change (§3.2).
Before building any of it, the same two questions this section
answered for M: the cost per candidate with a bound-based cover push
(no composition to lean on, so `M × depth` per candidate — the number
that sank M), and a picture gate to aim at.


## 18. The customer exists: the Schottky flames

`output/flame-zoom/schottky{1,2}.fflame` — four `mobius` transforms
each, decomposed from the `schottky_group` variation by its own
script: two circle-pairing generators and their inverses. Both read as
family M with `det = 1` on every map and the inverse pairs `0↔2`,
`1↔3` found numerically. Their isometric circles OVERLAP (distance
1.697 against radii summing to 2.0), so they sit past the strict
Schottky condition and the group may carry relations; the results
below hold anyway.

### The geometry was right; the enumeration walks the wrong words

The same random word, region radius by depth:

    schottky1   raw                  4:6.97   8:3.40    12:13.0    16:0.649   20:0.567   24:0.567
                backtrack-avoiding   4:6.97   8:0.762   12:0.0856  16:3.9e-4  20:1.3e-6  24:8.8e-8
    schottky2   raw                  4:19.1   8:20.3    12:6.14    16:1.97    20:32.2    24:32.2
                backtrack-avoiding   4:19.1   8:3.64    12:0.0877  16:2.0e-5  20:2.7e-5  24:4.3e-6

Same cover, same maps. A word that never follows a generator with its
inverse contracts by eight orders in twenty-four symbols; a raw one
stalls. The native `schottky_group` variation walks with the Indra's
Pearls backtrack rule; a decomposed flame picks uniformly and does
not. This is the fold-back of §16, confirmed where it was supposed to
matter.

### And the measure concentrates on reduced words

`--localize` on four million steps, reduced-word contraction rate
≈ −0.8 per symbol, so the matched depth is 4–5 at `r = 1e-1`, ~6 at
`3e-2`, ~7–8 at `1e-2`. Words holding 90% of the view's measure at
those depths, two centres each:

    schottky1        raw          reduced
      r=1e-1        54–400        7–22
      r=3e-2        321–376       10–14
      r=1e-2        301–1780      9–42

    schottky2        raw          reduced
      r=1e-1        87–5299       16–117
      r=3e-2        488–893       41–76      (thin samples)

Reduced: **of order ten**, bounded across a decade of view radius, two
centres, both flames — the control was one to four. Raw: hundreds to
thousands and growing. That is the difference between an antichain
and a frontier that never closes.

### Verdict

Family M has a customer, and it was never the geometry that failed.
`mobius.rs` stays. What has to change is the word expansion:
`a·a⁻¹·w` has the same map as `w`, so its measure belongs to `w`'s
cylinder and must be credited there — otherwise the targeted render
cannot match the untargeted one. Two ways, and the choice is open:

- **Re-weight reduced words analytically.** The mass of all raw words
  reducing to `u` is `p_u` times a return-probability series
  (classical cogrowth; O(1) per node). Exact for a free group;
  undercounts if the group has further relations, which past-tangency
  circles may give it.
- **Merge nodes by composed map.** Hash the normalized Möbius matrix;
  equal maps are the same cylinder and their probabilities sum. Exact
  for any relations, not just backtracks; the frontier is keyed by map
  rather than by level.

The second is correct without assuming freeness, which these flames
do not have. The picture gate — `schottky1` targeted against
untargeted — decides.


## 19. Built: cylinders keyed by map

The second option of §18, and it works. `schottky1` is targeted, the
picture gate passes, and family M is on.

### The change

`plan_mobius` replaces the word walk for family M. Cylinders are keyed
by their composed MAP, not by their word, because the flame's alphabet
holds each generator and its inverse and a word-indexed walk treats
`a·a⁻¹·w` as a different cylinder from `w`. The measure
decomposition groups by map — equal maps push forward identically, so
their probabilities add and their regions are computed once.

Three things had to be right.

**The key.** Möbius maps are projective, so coefficients are compared
after dividing through by the largest. Measured before building
anything on it: the drift between `w` and `a·a⁻¹·w` is **1e-15 at
depth 80 and not growing** — normalising cancels the scale growth — so
the 1e-11 quantum has four orders of headroom. At depth 8, 9,842
distinct maps from 65,536 words, which is the free-group reduced
count: **merging by map is reduction**, and it stays right when the
group has extra relations, which these flames have.

**Increments, not levels.** `w` has length `k` and `a·a⁻¹·w` has
`k+2`, so a level-synchronous walk meets them at different levels and
can never merge them. This delivers probability increments largest
first; a map re-reached later simply receives more, and its region is
computed once, when its shortest word is known.

**Coalescing.** Each map is re-reached by a geometric series of
arrivals. Delivering them one at a time cost **four million pops for
seven thousand nodes** — 543 apiece — and the walk ran out of budget at
depth 13 with the frontier already turning over. Letting a node carry
what it is owed and taking the whole amount in one pop: 214,000 pops,
21 apiece, and `lost` falls from 1.4e-1 to **6.9e-8**.

### Where it lands

    schottky1, on the set          plan     words  depth   speedup    lost
      zoom 1e3                     728 ms    1751     18   6.9e1     6.9e-8
      zoom 1e5                    1005 ms    3212     23   6.5e3     4.5e-6
      zoom 1e7                     952 ms     534     23   2.8e6     6.9e-8
      zoom 1e9                     855 ms      18     23   5.8e8     6.0e-8

    schottky2                     ~1050 ms   TimedOut { nodes: 11905 }

The frontier peaks at depth 12 and drains to nothing by 19 — it
converges, which is what the word walk never did.

### The picture gate

`a_targeted_schottky_render_is_the_untargeted_render`, matched on
IN-FRAME samples rather than iterations (the reference lands
`iters · mass` of its samples in the view; a forced sample costs
`depth + 1` map applications, and comparing at equal iterations gave
the reference ten times the samples and read as the targeted render
drawing half the picture):

    zoom    words  depth   mass      speedup   lit ref/tgt  overlap  bright
    1e2      2840     19  1.82e-2   2.7e0       2913/2249    0.620   0.536/0.512
    1e3      1751     18  7.58e-4   6.9e1        975/952     0.749   0.571/0.652
    1e4      3504     23  5.07e-5   8.2e2        415/1310    0.911   0.589/0.658

Agreement improves with depth, which is the right direction: at 1e4
the targeted render lights 1310 pixels against the reference's 415 —
it resolves structure the unbiased game cannot reach.

### Two limits, both bounded and both reported

**Time.** `plan` runs on the UI thread, and node and pop caps do not
bound wall-clock: `schottky2` spent a hundred seconds inside caps it
never reached. `MOBIUS_TIME_BUDGET` is one second, and a walk cut
short with nothing to show reports `TimedOut { nodes }` rather than
`ViewIsEmpty` — "we did not look long enough" is a different thing to
say than "there is nothing there".

**Latency.** A second per view change is still a hitch on every frame
of a drag. Expensive flames now wait for the view to hold still for
250 ms before planning; cheap ones (affine, bounded) do not wait at
all, which is why `is_family_m` exists — it answers from `detect`
alone, without building a cover. Using the PREVIOUS plan while moving
was the other option and is worse: a plan carries the view it was made
for, so its words would put samples outside the frame.

### Known approximation

Merging by map merges words of different lengths, and `pack_words`
folds colour along the word — so a merged cylinder carries its
representative's colour fold, not the mixture. The fold converges
geometrically and the extra symbols of a folded word sit INSIDE, where
their contribution is most damped, so the error shrinks with depth;
the gate's brightness agrees to ~12%. Worth revisiting if colour ever
looks wrong on a shallow targeted render.


## 20. Family J, scoped: the blocker was atan2, not the arm

§2 said a word through `julian` cannot shrink unless the arm is part
of the word: the body picks one of `|power|` arms with a random draw,
so a bound covering every draw covers an annulus — `O(1)` however
small the input. Scoping family J meant testing that.

So the evaluator gained a forced arm (`Eval::rng`, `derive_branch`),
pinning the draw to that arm's slice a hundredth in from each end —
the evaluator widens every interval it makes, and an arm boundary is
exactly where widening would hand back two arms. Then measured it.

**The bound did not shrink at all**: 1.095 to 1.336 for inputs from
3e-1 down to 1e-3. The arm was never the problem.

`M::Atan2` returned `[-π, π]` unconditionally — the whole turn for any
input whatsoever. Every radial variation computes an angle and then a
sine and cosine of it, so a full-turn angle makes the image an annulus
however small the input, and no amount of arm-pinning could help.

`atan2` is continuous except across the negative real axis, so away
from the cut its extremes are at the box's corners. Two cases still
take the whole turn and are now the only ones that do: a box holding
the origin, where every angle really occurs, and one straddling the
cut, where the range wraps and an interval cannot say so.

With that fixed:

    input r     free bound     arm 0        arm 1
    3e-1            1.690e0    2.921e-1    2.921e-1
    1e-1            1.473e0    9.414e-2    9.415e-2
    3e-2            1.409e0    2.811e-2    2.812e-2
    1e-2            1.392e0    9.369e-3    9.378e-3
    1e-3            1.385e0    9.398e-4    9.492e-4

The pinned arm tracks its input at about 0.94× and contracts. The free
bound sits at 1.39 forever, which is correct — it does cover every arm
— and `the_arms_together_cover_the_free_bound` checks that pinning is
a refinement rather than a different answer (worst overhang 0.0).

### Worth more than family J

It was the loosest rule in the evaluator, and it touches every radial
variation:

    bodies that derive                514 → 517
    corpus flames blocked on a bound   28 → 26
    corpus flames with no blocker       7 → 8

The counts understate it: what actually changed is TIGHTNESS on the
bodies that already derived, which is what decides whether a word
shrinks.

### What family J still needs

The bound works. The rest is:

1. **The alphabet becomes `(transform, arm)`**, with probability
   `p_a / |power|` per arm. `grand-julian` goes from 3 symbols to 25
   (powers 2, 15, 8). At the measured −1.15 per step that is still a
   small antichain — §17 put it at tens to a few hundred
   branch-resolved words, bounded across a decade of view radius.
2. **`Bounder` has to carry the arm.** Today `one()` calls
   `derive(name, …)` with no arm; family J needs the arm threaded from
   the enumeration down to `derive_branch`. `JULIAN_BOUND`, the hand
   bound, must either take an arm too or step aside for the derived
   one when an arm is pinned.
3. **The kernel must force the arm on replay.** `pack_words` carries a
   symbol per step; it would carry `(transform, arm)`, and the
   variation's WGSL would read a forced value instead of drawing —
   `select(floor(|power| · rng_nextf(rng)), forced, forced >= 0)`,
   compiled out under the existing replay flag so untargeted shaders
   stay byte-identical (`canonical_shader_dumps` enforces that).
4. **A picture gate on `grand-julian`**, matched on in-frame samples,
   as family M's is.

Steps 1 and 2 are CPU-side and testable without touching the shader —
the same order that worked for family M, where the geometry was proven
before the kernel was involved. Step 3 is the only part that changes a
shipping shader.


## 21. Family J, built so far — and the step that was missing

### Done

**Arms reach the bound** (§20's measurement, now plumbed). Which
variations have arms is a table, `bound::ARMED`, not a feature:
`Feature::NeedsRng` cannot answer it, because `blur` draws too and its
draw is continuous — pinning it would UNDER-estimate where the point
can go, the one direction a forward bound may never err. `julian` and
`juliascope`, `ceil(|power|)` arms, capped at 64.

`Bounder` knows which of its variations is armed and refuses to guess
when two are; `apply_arm` bounds one arm, `apply` still bounds them
all. A pinned arm skips the hand bound, since the five hand bounds
answer for every arm at once.

**Arms reach the alphabet.** A symbol is `(transform, arm)` packed as
`transform | arm << 8`; arm zero is the bare transform index, so every
word written before arms existed is still valid. `ARMS_ENABLED` is
off, because the kernel cannot force an arm yet and a plan full of
arms would draw a wrong picture rather than a slow one.

Measured, on whole transforms:

    grand-julian: arms [2, 15, 8]
      unresolved    dies at step 2, "julian has no forward bound"
      arm-resolved  1e-3 → 1.35e-3 → 1.87e-5 → 5.61e-6 → 7.07e-6

### The step I left out

Asked to enumerate with arms, `grand-julian` still refuses — and not
because of arms. The refusal is `plan`'s up-front bound check and then
`invariant_ball`, both of which bound every arm at once and both of
which meet the origin, where `julian` with `dist = -1` is genuinely
unbounded. The flame reports `NoInvariantBall`, exactly as
`spherical.fflame` does: its attractor spans `|p| ∈ [1.7e-1, 2.7e1]`,
so every disc containing it contains the pole.

**Family J needs the leaky cover root too.** That is what §3.2 meant
by "family M's machinery with a bound in place of an exact circle",
and it is the part I under-weighted when I framed this as four steps.

### What the generalisation actually costs

`Cover` is coupled to `MobiusMap` in two places, and only two:

- `Cover::push(&MobiusMap, …)` — pushes each disc, refines onto the
  sample where the image would swallow a pole.
- `cover_attractor(&[MobiusMap], …)` — runs a CPU chaos game
  (`apply_point`) and reads each map's `pole()` to size the discs.

Both generalise, and one design idea makes it cheap. **`CoverRules`
does not need to know where the poles are.** It avoids them today by
being told; but a disc is fine exactly when its push SUCCEEDS and
stays finite, and any bound can answer that without knowing what a
pole is. Replacing "is this disc clear of the poles" with "does this
disc push" makes `Cover` map-agnostic, and then:

- the point walk is `Bounder::apply_arm(Ball(p, 0))`, whose centre is
  the image point;
- the push is `Bounder::apply_arm(disc, arm)`;
- nothing needs a pole.

At that point `Cover` is no longer Möbius-specific and probably should
not live in `mobius.rs`.

### The remaining shape

1. ✅ arms in the bound
2. ✅ arms in the alphabet
3. ⬜ **the cover root, generalised to push by a `Bounder`** — the
   substantial one
4. ⬜ the kernel forcing the arm on replay (the only shipping-shader
   change)
5. ⬜ a picture gate on `grand-julian`, matched on in-frame samples


## 22. Step 3: the cover generalises, and is needed exactly once

### The generalisation

`Cover` asked exactly two things of a Möbius map — push a point, push
a disc — so `push_by` takes those as closures and `push` is a thin
wrapper over it. A `Bounder` can do both: `apply_arm` on a zero-radius
ball is the image point. Family M's 27 tests pass unchanged.

**`CoverRules` no longer has to know where the poles are**, which was
the whole obstacle. Knowing a pole is a luxury only a Möbius map
affords (`−d/c`, written down); a bound cannot say where it will blow
up, and it does not need to, because a disc is fine exactly when its
push SUCCEEDS and stays finite. With poles known nothing changes.
Without them, `merge_growth` keeps a merge from quietly building the
disc that cannot be pushed — two discs that are nearly the same disc
push the same way, two far apart do not. The alternative was
validating every candidate merge by pushing it under every symbol,
which for twenty-five arms is twenty-five pushes per candidate.

`cover_by_pushing` builds one the same way: start each disc at a
fraction of the extent and halve until it pushes. A disc that never
pushes holds a point the enumeration cannot follow, and is dropped and
counted.

    grand-julian: alphabet 25, extent 2.27e1, 66 discs, leak 0.00e0
      contracts 22.7 → 1.47e-5 → 1.09e-5 → 5.42e-6 → 1.93e-6
    julian-disc:  alphabet 51, 256 discs (capped), leak 3.42e-1

### The cost, and the way round it

A push is **5.06 ms**, and `julian` is not a group — there is no
folding a word into one map the way family M does. A candidate at
depth 20 would cost 100 ms, which is exactly the wall family M hit
before composition.

**Except the cover is only needed once.** It exists to get past the
pole, and measured on both flames, one ball suffices **from step 1**:
after a single symbol the image is a ball — radius 0.119 and 0.237 —
that every symbol can push. Collapsing a cover to its enclosing disc
only grows the region, so it is sound, and checking that the collapsed
disc still pushes is what makes it safe.

So family J is affordable with no composition trick at all:

1. build the cover once;
2. push it once per symbol, collapse each image to a ball — those are
   the root images, about 125 ms of setup per plan;
3. walk the ORDINARY Ball path from there, at 14 µs a symbol.

After the setup the enumeration costs what an affine flame's does.

### Open

`julian-disc`'s cover leaks 34% at a 256-disc cap, so its construction
wants more discs or a smaller starting radius. Measured rather than
guessed at, and it does not block `grand-julian`.

### Remaining

1. ✅ arms in the bound
2. ✅ arms in the alphabet
3. ✅ the cover, generalised — and needed only for the root images
4. ⬜ `plan` using the root images, then the ordinary walk
5. ⬜ the kernel forcing the arm on replay
6. ⬜ a picture gate on `grand-julian`

## 23. Step 4: `grand-julian` plans — and the four things that had to
give first

The step as planned was one sentence: use the root images, then walk
the ordinary Ball path. Four separate things were wrong with that, and
each was found by a measurement rather than by reading.

**The up-front probe refused the flame before anything looked at it.**
`plan_inner` asked every non-affine transform for a bound on a disc of
radius 1 around the origin. For an inversive flame that is a disc
holding the pole, the answer is "unbounded", and `grand-julian` never
reached the machinery built for it. The refusal is now remembered and
only returned if no root can be found at all — and, for anything
outside family J, returned in exactly the place and order it always
was, so no other flame sees a different answer.

**One ball per symbol does not exist, and cover tightness is not why.**
The first attempt collapsed each pushed cover to its enclosing ball.
For `grand-julian` symbol 0 that ball came back at 8.46e1 against an
attractor extent of 1.87e1 — and tightening the cover from 97 discs to
2048 moved it to 2.72e1 and no further:

    cap    discs  leak      build   walkable        biggest
    1.000     97  0.00e0     27ms   15 of 25        5.93e-3
    0.300    128  0.00e0     36ms   18 of 25        1.54e-2
    0.030    476  0.00e0    148ms   21 of 25        1.37e-2
    0.003   2048  1.54e-1    723ms  22 of 25        8.74e-3

The per-symbol table says why. `julian` with `n` arms divides the angle
by `n`, so arm `k`'s image of the attractor is a `360/n`-degree sector
of an annulus AROUND THE ORIGIN, which is where the map's own pole is.
The fifteen-arm transform gives 24° sectors whose balls sit 0.08 clear
of the origin and walk fine; the two-arm transform gives a 180° sector
whose enclosing ball must contain the origin. That is geometry. No
cover refines its way out of it.

So the region is a BAG of balls, cut by recursive median bisection
until each piece pushes — pole-agnostic, because the whole premise of
this path is that a bound cannot say where it blows up. (Cutting by
angle about the centroid was tried first and is worse: the centroid of
an annular sector is not the hole's centre, so wedges taken about it
still span the hole.) The enumeration already carried a bag-shaped
region for family M, so this cost pieces-per-symbol bound evaluations
and no new machinery.

**Splitting has to be driven by walkability, not by size.** Giving
`CoverRules` a maximum image radius was not enough: an image disc well
inside a generous cap still swallowed the pole, and once that has
happened no amount of cutting the result up afterwards recovers — the
SOURCE disc has to be split. `Cover::push_by` now takes the caller's
own `accept` predicate alongside `rules`, and family J passes "the
image can itself be pushed", asked of one arm per transform because
every arm of one `julian` shares its pole.

With that, the root exists:

    grand-julian: root 1.92e1, 25 images in 219 pieces,
                  radii 6.4e-2..2.6e1, leak 4.0e-4, 2.5 s

and the two flames that should be refused are refused for the right
reason. `spherical` fails the contraction probe — 0 of 32 probe words
contracted, the worst ending at 39.5× the extent — which is the direct
statement that a disc bound through an inversion never shrinks. The
probe replaced an earlier "a root image must be small" gate, which was
asking the wrong question: `julian`'s first image is a wedge of the
attractor and is SUPPOSED to be the size of the attractor.

**`TooManyWords` was the frontier, not the antichain.** The beam that
answers a frontier growing like the branching factor to the depth was
gated to family M, with a comment reasoning that everywhere else an
overfull frontier means a straddled view and deserves a clear refusal.
That reasoning turns on the flame having a real invariant ball, and a
flame rooted in a cover has none — `grand-julian`'s frontier reaches
25981 without the beam, which is the same symptom `spherical.fflame`
showed in §15.

**And one piece of a hundred must not take the word with it.** With the
beam on, a 1e2 view planned and everything deeper came back
`ViewIsEmpty`. The cause was `bag_of` refusing the whole word when any
single piece failed to push: past depth seven, with ~100 pieces, that
is every word. A piece that blows up holds a pole the true image does
not — it is an artifact of the cover, not of the flame — so it is
dropped, which shrinks the region by the same approximation the cover
already makes between its samples.

    grand-julian at its own framing, view on the set:
      zoom 1e0   TooManyWords(4097)
      zoom 1e2   28 words, depth 4, speedup 9.1e3, lost 2.0e-1
      zoom 1e4   ViewIsEmpty
      zoom 1e6   ViewIsEmpty

So it plans, and where it plans the speedup is real. Two things are
open and neither is guessed at:

1. **Deep views still come back empty.** It is not beam width —
   measured, 96 to 1024 changed nothing except taking 59 s. Something
   stops the regions covering the view point between 1e2 and 1e4, and
   the next thing to measure is whether the surviving bag still
   contains a known orbit point at each depth, which would separate a
   dropped piece from a genuine prune.
2. **A plan costs 5–17 s.** Unusable in-app whatever else is true. The
   root is 2.5 s of it and depends only on the flame, so it wants the
   same caching the family-M settle delay already implies; the rest is
   `disc_of` recomputing from the root for every candidate, which is
   `O(depth²)` over a walk that now reaches depth 96.

`julian-disc` is still refused: its cover leaks 32–41% of the orbit at
every disc cap tried, because transform 0 carries 97% of the measure
and the orbit reaches `|p| = 3.1e-4`. `ARMS_ENABLED` stays off; nothing
here reaches a render.

## 24. Is it possible? Yes — and the forward bound is the wrong tool

Asked directly, and answered by three measurements, none of which had
been made before.

**How loose is the bound against the true cylinder, per step?**
Pushing the same sampled orbit through the same random word as points
gives the truth; the ratio to the bag's enclosing radius is:

    depth   0     1     2     3     4     5     6     7     8     9    10
    ratio  1.4   2.9   6.2   13    28    85   278   775  5e3  4e4  7e5

Two to three times looser per step, compounding, and then a FLOOR: the
bag's radius sits at ~3e-6 from depth 9 on while the truth reaches
1e-15. The floor is `WIDEN = 8·2^-23` in the interval evaluator — it
bounds an f32 computation and says so — and it caps any forward plan at
roughly zoom 1e6.

**Where does the true word go missing?** A sampled orbit's last `k`
symbols are a word whose cylinder contains the orbit's point by
construction. Followed through the expansion, beam removed:

    depth 3: bag 1 piece, enclosing 2.11e-1
    depth 4: bag 1 piece, enclosing 2.22e-1
    depth 5: bag 1 piece, enclosing 1.80e-1
    depth 6: DIED (every piece hit a pole)

The bound for the measure-typical word does not contract at all, while
random words in the previous table did. The difference is that random
words were drawn uniformly over the 25-symbol alphabet (8% `t0`) and
the measure is 75% `t0` — and `t0 = z^(-1/2)` EXPANDS wherever
|w| < 0.63, which is where the attractor lives. A ball bound has to take
the worst derivative over the ball, so it cannot contract until the
ball is already small, and it cannot get small without contracting.
That is structural: a perfect conformal ball bound would have the same
problem, and the interval evaluator's looseness only makes it worse.
The frontier went 1, 5, 51, 409, 2716, 20173, 149245 — seven times a
level — and every one of the `ViewIsEmpty` results, the 24k-word
frontiers and the 17-second plans is this and nothing else. The beam
never mattered; the cover gap hypothesis was tested and is false (both
view points sit inside a root bag).

**Does the measure concentrate?** This is the question that decides
everything, and the forward bound cannot answer it here. The inverse
maps can. Every map in the flame is `julian ∘ rotation`; `julian`'s
inverse is single-valued, and for a given OUTPUT point only one arm of
each transform can have produced it, because the arms' images are
disjoint sectors. So the words whose cylinder contains a point form a
tree of branching at most three — one per transform — that can be
walked backwards with exact arithmetic and no bound at all. Walked from
eleven orbit points, at three on-attractor tolerances, to depth 24:

    words containing the point, every depth, every point:   1
    mass of that word at depth 5:                           1.5e-7
    derivative product below 1e-6 by depth:                 8..17

**The word is unique.** The three transforms' images are nested annuli
of very different widths — `t1 = 0.2·z^(-1/15)` lands in a ring of
width 0.07, `t2 = 0.3·z^(-1/8)` in one of width 0.2, `t0` spans 0.18
to 2.5 — so a point's radius pins which transform produced it, and its
angle pins the arm. A deep view of `grand-julian` is reached by ONE
word, and forcing it puts every sample in the frame: at zoom 1e3 the
speedup is `1/(1.5e-7 · 6) ≈ 1e6`. This is not merely possible; it is
the ideal case for cylinder targeting. The forward walk found 28 words
with `lost` 0.2 at 1e2 and nothing deeper because its bounds were
five orders of magnitude too loose to see a unique word.

**What this means for the plan.** The cover root, the pieces, the bag,
the family-J beam — steps 3 and 4 — are the forward approach and are
superseded for family J. The generalised `Cover` stays (family M uses
it) and the measurements stay; the bag walk should not be extended.
The planner for an invertible-per-arm flame is the backward walk
itself: pull the VIEW back through the inverse maps, prune pre-images
that miss the attractor, cut when the pulled-back region covers it.
The prototype above ran in 0.04 s for 24 depths at three tolerances.

What it needs to become real, in order:

1. A region rather than a point pulled back — a disc through a
   conformal inverse, outer-bounded for pruning and inner-bounded for
   the cut (Koebe gives both for a univalent map). The pullback grows
   under an expanding inverse, which is the right direction.
2. The attractor-membership test against a sampled attractor. This is
   a genuine approximation, the same class as the cover's leak, and a
   missed word is a HOLE in the view, not a wrong pixel — so it wants
   a plan-time check: replay each word forward on sample points and
   count what lands in the frame. That count is `lost`.
3. The kernel forcing the arm — step 5, unchanged. `pack_words`
   already ships the full symbol; the shader indexes `transforms[sym]`
   and the armed variations draw their own arm.
4. The f32 ceiling: the kernel replays symbol by symbol in f32, so
   error compounds at roughly depth × 1e-7 relative. Zoom 1e6 is
   fine; 1e8 is not, whatever the planner does.

`ARMS_ENABLED` stays off. The diagnostics that produced every number
above are `how_loose_is_the_bag_against_the_truth`,
`where_does_the_true_word_go_missing`,
`is_the_deep_view_failure_a_cover_gap` and
`does_the_measure_concentrate_backwards` in `cylinder.rs`.

## 25. Built: the planner by the inverse walk

`src/scene/backward.rs`. It is not a new walk: `ifs_analysis::analyse_2d`
already gives every transform's inverse, its exact Jacobian by dual
numbers and one map per preimage branch, and `ifs_estimate::
estimate_measure` is the same walk with a different stop rule. This
one pulls the VIEW back through the inverses, carrying the composed
inverse Jacobian as a first-order region: prune when the region (with
margin) holds no point of the sampled attractor, trigger a cut when its
inscribed disc has grown far enough, and VERIFY every cut by replaying
the word forward on the sample — the fraction that lands in the frame
is the word's efficiency, exact for the sample and immune to the
region's distortion. The arm is recovered by trying each forward
branch and keeping the one that lands on the point.

Two things had to be added after the first run. A relative measure
floor: at zoom 1e2 the typical word cut by depth 6 and the beam then
carried 256 words of probability 1e-40 to the depth cap and replayed
each — fifteen seconds for `lost` of 1e-38. And a cheaper cut trigger:
waiting for the first-order region to fully cover the attractor walked
words twice the exact cut depth, because the smallest singular value
under-reads an anisotropic region; the replay decides anyway, so the
trigger only says when to ask.

    grand-julian, view on the set:
      zoom 1e2    15 ms   12 words  depth  6..20  eff 1.00  speedup 2.7e2   lost 1e-11
      zoom 1e3    36 ms   13 words  depth 10..23  eff 1.00  speedup 8.0e4   lost 2e-13
      zoom 1e4     5 ms    3 words  depth 12..13  eff 1.00  speedup 3.9e5   lost 0
      zoom 1e6     6 ms    2 words  depth 16..17  eff 0.93  speedup 1.6e7   lost 0
      zoom 1e8     9 ms    3 words  depth 22..23  eff 1.00  speedup 1.0e11  lost 2e-20

    through plan_armed at the other view: 15–77 ms, 4–32 words,
    speedup 1.6 at 1e0 (correctly not worth it) to 3.0e11 at 1e6.

Against §23: the forward walk took 17 s to find 28 words with `lost`
0.2 at 1e2 and nothing at all deeper.

**What it does not do yet.** `julian-disc` comes back with efficiency
0.00 at 1e6 and 1e8 and every word at the depth cap: its `disc`
transform has three preimage branches that share one symbol, and the
walk's bookkeeping for a many-branched INVERSE (as opposed to a
many-armed forward) is evidently wrong. `random1`, which has an
invariant ball and is planned exactly by the forward path, plans here
too but slowly (1–3 s) with `lost` up to 5e-2 — its measure spreads
over many overlapping words and the beam bites; not the customer, but
a measurement worth having. Nothing reaches a render: `ARMS_ENABLED`
is still off, and the kernel cannot force an arm. That is the next
step, and it is the last one between this and the app.

## 26. Step 5: the kernel forces the arm, and the planner is rewritten
around what it took to be complete

**The kernel.** `pack_words` already shipped the full symbol; the
shader indexed `transforms[sym]`. Now the replay masks the transform
out of the low byte and sets `ct_forced_arm` (a `var<private>`, -1
between symbols) before applying it, and each armed variation's draw
is wrapped by the shader builder -- `floor(abs(power) * rng_nextf(rng))`
becomes `ff_forced_arm_f(...)` -- from a table in `bound.rs` that
names the draw's exact text and its forced form. The wrap happens only
under `CYLINDER_REPLAY`: an untargeted build is byte-identical, and the
canonical shader dumps say so (a blank line moved once, and the gate
caught it). `ARMS_ENABLED` is on. `every_armed_draw_is_in_its_bodies`
and `the_forced_arm_reaches_the_built_shader_only_under_replay` are
the gates on the table and the wrap.

**The first picture was wrong, and the planner was why.** Overlap 0.46
at 1e2. A completeness test settled where the fault lay: run the chaos
game on the CPU with each sample's symbol history, keep what lands in
the view, and count how many have a planned word as their last `k`
symbols. 34%. `lost` was 1e-11; it cannot see a branch that was never
formed. The rest of this section is the sequence of representations
of a word's region that failed, each named by that test, and the one
that did not.

A centre point with a Jacobian: dropped the `t1`/`t2` children at
depth six because the region had outgrown first order and its centre
sat outside the ring. A cloud of 64 view points pulled back: 0.75,
then the same three arms -- `t2a4`, `t2a6`, `t1a7` -- missing through
jitter, boundary refinement and a wider beam, because area is the
wrong thing to sample when the measure sits on ring sectors of area
5e-3 inside regions of area 200. Densifying by measure fired only for
thin nodes and never for the one that needed it. A watch on that one
subtree showed pulled-back off-ring points with spread 3.4e9 blinding
the cut, and at another zoom the same branch, twelve points all off
the attractor, pruned -- a prune charges nothing.

**What works: the attractor sample, indexed.** 100k points, μ-
distributed and exactly on the attractor, landed once through every
symbol and filed by grid cell (a flat sorted list; 400 ms per flame,
cached). A child's candidates are index lists over its parent's
cells, a capped handful verified by replay. The view cloud keeps one
job -- the descent from a view too small to hold sample points -- with
its children admitted by "does anything land in this cell under that
symbol", which refuses the near-origin junk and keeps sparse regions.
Then three more things the test named: the candidate budget is per
child, not per cell (a node on one cell starved); a probe of eight that
finds nothing decides nothing (at an 8% hit rate it dropped 60% of a
view under one word); and the beam ranks by probability TIMES
efficiency, because probability is a cylinder's whole measure and at
depth seven of a 1e6 plan the node holding the view was dropped for
it (`lost` 2.15e-8 against a kept mass of 3e-14). The spread trigger
for the cut went too: it waited for regions to reach far outliers that
carry no measure, five levels past where words fit.

    completeness   1e2 0.95   1e3 0.93   1e4 1.00 (7 samples)
    plan           1e2 85 ms  1e3 115  1e4 133  1e6 127  1e8 156
    mass at 1e6    3.6e-10 at depth 9..18   (3e-14 at 21..31 before)

    picture gate   1e2 overlap 0.92, brightness 0.39/0.32
                   1e4 overlap 1.00 against a 23-pixel reference

**What is still approximate, stated.** Completeness is bounded by
the points per node (256): a child holding under half a percent of its
parent is sometimes missed, and the ~5% the test measures is that.
`lost` now charges a dropped node its probability times the larger of
its efficiency and one part in a hundred, which is a bound rather than
a measurement. The reference render is too starved past 1e3 to compare
brightness, so the deep views are validated by the CPU test alone.
`julian-disc` is untouched. The plan costs ~130 ms a pan, and armed
flames now take the family-M settle delay so a drag is not planned on
every event.

**And one regression the gates caught after the fact.** With
`ARMS_ENABLED` on, an armed flame the inverse walk refuses (a final
transform, two kernels in one transform) fell through to the §23
forward cover/bag walk -- 17 to 60 seconds a plan, on the UI thread.
The cylinder gate sat for an hour on `every_working_flame_loses_nothing`,
which plans every flame in `output/`. An armed flame is now planned by
the inverse walk or refused with its reason; the bag walk is never
reached from an armed flame. That test also now admits armed flames'
accounted `lost` (1e-4 to 3e-3 measured) under a 1e-2 ceiling, as it
already did for family M.

## 27. Complete by construction: holes become waste

A saved view (`output/grand-julian-missing-pieces.fflame`, zoom 546,
straddling the boundary between two of `t2`'s arm sectors) showed half
the picture missing in the app. The CPU completeness test put the
committed planner at 70% there. Every cause was the same shape: a
branch holding part of the view was DROPPED -- by the beam, the measure
floor, the time budget, the word cap, or simply never formed because
the node's capped point set did not name the cells its child lived in.
`lost` could see only some of those, and none of the ones that mattered.

The rule now is that nothing holding view measure is ever dropped.
Anything the walk cannot resolve further is FORCED as it stands: off
the beam, out of time, at the depth cap, below the floor. A shallower
word is less efficient -- more forced samples land outside the frame
and are discarded by the kernel -- but it draws everything beneath it.
The kernel's word table has no hard size (the buffer grows, the pick is
a binary search), so `MAX_WORDS` no longer truncates this planner.

Two things make that rule cheap rather than a collapse to untargeted:

- **The orbit's split.** The sample is one orbit, so each point records
  the symbol that produced it and its predecessor. A region's points
  split EXACTLY among its children, in proportion to their shares, so
  every child holding more than a sliver of its parent is found by
  construction; the landing index then tops the children up.
- **An exact completeness check.** A node is forced in place of its
  children when fewer than 90% of its own points belong to a child
  that survived. A first version compared replay estimates instead --
  two ~15%-noisy numbers against 0.9 -- and its false triggers forced
  whole ancestries: a depth-1 word holding 37% of the attractor.

Two smaller fixes the same investigation found: `gather` sampled by
AREA (two points per cell, filled from the first cells in sort order),
which starved the dense rings the measure lives on; it now takes every
k-th entry of the concatenated cell lists, which is by measure. And the
beam ranked unmeasured nodes (zero replay hits, below the replay's
resolution at shallow depth) last, so they fell off it; they are now
always carried.

    view                  coverage   efficiency   plan
    saved view (x546)     0.998      0.72         2.6 s
    test 1e2              0.997      0.93         2.6 s
    test 1e3              0.984      0.92         1.9 s
    test 1e4..1e8         1.000*     0.79-0.92    1.7-3.6 s
                                     (* where the CPU test has samples)

    saved view on the GPU, targeted vs untargeted at 4e9 iterations:
    overlap 0.978, brightness 0.414 / 0.406, targeted denser

Performance was set aside on purpose: the plan now takes 2-3.5 s on the
UI thread after the settle delay. The 1.5 s time budget still applies,
but hitting it now forces the frontier rather than dropping it, so a
slow plan costs efficiency, not pieces of the picture. The next step is
moving the plan off the UI thread.

## 28. Planning off the UI thread, and what caching can and cannot buy

**Background planning.** The app plans the expensive flames (armed,
family M) on a worker thread; `FlameRenderer::set_background_planning`
turns it on, and the headless paths (CLI, export, tests) still plan
inline so their first sample uses the plan. While a plan is generated,
the plan on screen keeps drawing if only the VIEW moved -- the replay
arm plots in world coordinates, so an old plan still puts its samples
in the right place, over part of the new frame -- and is dropped at
once if the FLAME changed, since its words may name transforms that no
longer exist. A plan that lands resets accumulation (samples under the
previous plan carry a different weight). A stale plan is cancelled
through a flag the walk checks once per node. The Deep Zoom panel shows
a spinner and "Generating a plan for this view... N s", plus a note
when the previous plan is still drawing. Measured by
`a_plan_is_made_in_the_background`: the longest `sync_cylinders` on the
caller was 18 ms while plans ran.

Three wastes surfaced on the way, all fixed:

- **Every shader-changing plan ran twice**: `sync_cylinders` planned and
  asked for a reload, and `load_config` planned again. It now skips a
  plan that is already current.
- **The key thrashed on sticky variations.** `load_config` plans against
  the sticky-adopted flame (retained variations at weight zero) and the
  per-frame sync against the raw one; hashing zero weights made the two
  keys differ, so every reload was followed by a replan, and the
  inverse walk's per-flame cache flipped between the two flames at
  400 ms a rebuild. Both keys now ignore zero-weight variations.
- **The 1.5 s budget** existed for the UI thread; it is now a 20 s safety
  net, and hitting it forces the frontier (complete, less efficient).

**Where a plan's time went** (`where_a_plan_spends_its_time`), before:

    grand-julian x1e3    23.9 s   check candidates 15.8 s, replay 6.3 s
    saved view (x546)    25.3 s   check candidates 16.5 s, replay 6.4 s

**Checking candidates was 90% wasted**: it happened before deciding
whether a child was kept (needs no points) or carried (needs them), and
~90% of children are kept. Children are now replayed first and checked
only if carried. **Nodes of a level are expanded in parallel** (rayon;
12 cores here), merged in frontier order so a plan is the same however
the threads ran. The measure floor moved 1e-5 -> 1e-4, which stops the
walk before thinly sampled levels and was both faster and MORE complete.
And a carried child left thin (under 64 points) is searched again,
wider: at a 1e3 view a node with ~1000 sample points in its region was
carried with 15, and 1.7% of the view went missing beneath it.

    view                coverage   efficiency   plan (was)
    saved view x546     0.991      0.93         0.83 s  (25.3 s)
    test 1e2            0.998      0.97         0.36 s
    test 1e3            0.994      0.95         0.40 s  (23.9 s)
    test 1e4            1.000      0.94         0.71 s
    test 1e6 / 1e8      --         0.93-0.95    0.53-0.78 s  (19.1 s)

    background, grand-julian: first plan 1.08 s, after a zoom 0.87 s,
    after a flame edit 1.28 s; GPU comparison at the saved view:
    overlap 0.967, brightness 0.417 / 0.409, no structure missing.

**What is cached.** The per-flame analysis -- 100k-point orbit sample,
its producing symbols, the grid and landing indexes -- is built once
per flame (~400 ms) and shared across threads and views.

**What a view-independent map would cost** (`what_could_a_cache_reuse`).
The attractor's box-counting dimension is ~1.15, but that is not the
number that matters: a single 1e3 view holds ~4,000 words, because this
flame's pieces overlap many times over (`t0` alone is two-to-one).
Tiling the attractor at 1e3 takes ~2,800 such views, so a map complete
to 1e3 is on the order of 10^7 words, and to 1e6 some 10^10 -- each
needing sample image points to be useful. Not storable, and building it
is planning every view. A map is only affordable to ~1e2, where a plan
already takes ~0.1-0.3 s.

**What a word cache would save.** Between a view and a nearby one, the
share of the second plan's expanded nodes the first had expanded:

    move            at 1e3   at 1e6
    pan 1/4 view     27%      34%
    pan 1 view       10%      19%
    zoom in 1.5x     20%      14%
    zoom in 4x        3%       7%
    zoom out 2x      26%      16%

Most of a plan's nodes sit near the depth where words fit the view, and
that depth is view-specific. A cache keyed by word would save 1.1x to
1.5x on typical moves -- real, but small against what parallelism and
the reorder bought, and it would need the region points of every cached
node. Recorded, not built.

**What would still pay**, in order:

1. **Plan with a margin**: plan for a view ~1.5x the screen's radius.
   A pan or zoom that stays inside it needs no plan at all -- the old
   plan is still complete there -- so the picture between plans is
   whole rather than drawn over only part of the frame; the cost is
   efficiency, up to ~2.25x until the tighter plan lands.
2. **Cheaper replays** (now the dominant cost, ~70% of CPU): replay 100
   samples and extend to 400 only near the cut threshold -- about 2x.
3. **Replays on the GPU**: the maps already exist as WGSL; thousands of
   children x 400 samples x depth is what a GPU is for. Large.

## 29. A standby plan, not a margin

The plan asked for was a margin: plan a disc wider than the view, so a
small pan or zoom needs no new plan. `what_a_margin_costs` measured what
that costs the picture you STOP on -- the share of forced sampling that
lands in the actual view:

    margin (radius)    1.0    1.25   1.5    2.0    3.0
    grand-julian 1e2   0.96   0.57   0.28   0.19   0.09
    grand-julian 1e3   0.93   0.74   0.61   0.38   0.23
    grand-julian 1e6   0.95   0.71   0.51   0.25   0.15
    saved view x546    0.93   0.70   0.65   0.46   0.30

Coverage stayed 0.99 or better at every margin, and plan times did not
change. But a 1.5x margin would make the picture fill in 1.5x to 3.5x
slower for as long as you look at it -- too high a price for something
that only matters while moving.

So the margin is a STANDBY plan. The view gets a tight plan, as before;
once it lands, a second plan for a disc twice the view's radius is made
in the background and held, not shown. When the view moves outside the
plan on screen but inside the standby -- a pan of up to one view radius,
a zoom out of up to 2x -- the standby is swapped in on that frame, and
the picture stays complete while a tight plan for the new view is made
after the view settles. Its low efficiency applies only while moving,
when the accumulation restarts every frame anyway. A tight job cancels
a standby job in flight; the panel's "Generating" line counts only tight
plans.

    a_standby_plan_covers_a_move (grand-julian, x1e3, 256x256):
      first tight plan 1.10 s, standby ready 0.58 s after it
      half-radius pan: standby swapped in on the first frame, 0.8 ms
      tight plan for the new view 0.77 s later, new standby 0.61 s
      pan of 5 radii: no swap; planned as before

A slight zoom IN stays inside the tight plan's own disc, which is still
complete there, so nothing is swapped; the tighter plan follows after
the view settles. Affine flames are untouched: their plans cost under a
millisecond and are made every frame.

## 30. Sequential replays

After §28 the replays were 64% of a plan's CPU: every child ran its word
on all 400 verification points. The number a replay produces does two
jobs -- whether a child is kept (share of at least `CUT_EFFICIENCY`, 0.9)
or carried, and the beam's ranking -- and neither touches completeness,
which is a count of points (§27). A child landing 30% in frame is
carried after 100 points as surely as after 400.

So a replay runs the first 100 of the 400 points (spread evenly through
them) and runs the rest only when the share so far is strictly between
0.8 and 0.97 -- close enough to 0.9 that a hundred points could decide
wrongly (their standard deviation there is 0.03).

    view                replay CPU        plan        coverage
    saved view x546     4.9 s -> 1.5 s    853 -> 479 ms   0.991 (same)
    grand-julian 1e3    2.6 s -> 0.8 s    496 -> 319 ms   0.994 (same)
    grand-julian 1e6    2.0 s -> 0.65 s   583 -> 438 ms

True in-frame efficiency, measured with full 400-point replays
(`what_a_margin_costs`, margin 1.0): 0.945-0.971, against 0.927-0.964
before -- a hundred points sometimes cut a word a level earlier, and it
fits.

The candidate cap was tried lower now that the orbit split and the
thin-child search find most points (§27-§28): at 512 the saved view fell
to 0.981 coverage for 15% less time, at 256 to 0.962. It stays at 1024.

What remains per plan (saved view): candidate checks 1.8 s, replays
1.5 s, gather 1.0 s of CPU. The next step is the GPU:
[gpu-cylinder-planning.md](gpu-cylinder-planning.md).
