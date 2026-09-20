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
