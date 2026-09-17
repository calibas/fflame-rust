# The measure by the inverse walk: the flame's density at any zoom (plan, 2026-09-17)

The third of three plans written together; the first is
[ifs-perturbation-delta.md](ifs-perturbation-delta.md), whose §9
orders the work of all three, and the second is
[ifs-general.md](ifs-general.md).

**What is being asked for.** Mode D renders a DISTANCE to the
attractor: a boundary, coloured by level or address. The flame's
picture is its invariant MEASURE -- density and colour, log-mapped
-- and the chaos game samples that measure, which is why it starves
at depth. Asked from use: "do per-pixel escape-time, then reverse
direction and use that as a starting point for the chaos game." This
plan is that idea in the form that works, and it is the flame look
at any zoom on the walk that already exists.

**What it buys.** For every flame the walk accepts, the flame's own
density and colour at every zoom the walk reaches, with no forward
chaos game at the zoom at all -- so no starvation, and no precision
problem beyond the walk's. It composes with the walk's existing
deep-zoom machinery rather than adding a second one, and it is the
reason the other two plans are worth their cost.

**What it does not buy.** Not flames the walk refuses (that is
[ifs-general.md](ifs-general.md)'s reach); not direct-colour
variations, which colour by evaluating the forward map; not 3D; and
not the forward-targeted chaos game of
[flame-deep-zoom.md](flame-deep-zoom.md), which this plan may make
unnecessary or may not, and says how to tell.

---

## 1. What is there

- **The walk**, `ifs_evaluate` in the mode-D shader and
  `estimate_seeded` / `estimate_aux_ranked` on the CPU: from a
  pixel, inverse maps by a beam of addresses, each lineage carrying
  its position, σ, bound, address and escape state; the answer is
  the MINIMUM over lineages of the distance bound. `IfsRecord` keeps
  per pixel the distance, level, address, colour and point.
- **The accumulator format.** The escape pass hands `render_with`
  an `Rgba32Float` image in the flame accumulator's layout -- rgb
  the density-weighted mean colour, a the raw hit count -- and
  everything from the tonemap on is shared. The log tonemap
  normalises by `sample_density = total_iters / pixel_count`.
- **The chaos game's own machinery**: `select_transform_xaos` with
  the row-conditional matrix; the flam3 colour rule
  `c' = c·(1+s)/2 + col_i·(1−s)/2` with `s` the transform's
  `color_speed`; the per-sample `density_weight`.
- **[flame-deep-zoom.md](flame-deep-zoom.md)**, a plan with no
  code: importance-sampled selection with a windowed correction
  (stage 1), and "cylinder targeting" (stage 2) -- enumerating the
  address prefixes whose pieces meet the viewport and forcing them,
  weighted by their probability. Its §2 derives the window from the
  same unrolling this plan uses. Its stage 3 says perturbation
  cannot apply to a chaos game because there is no reference orbit;
  that is true of the unforced game and false of a forced prefix,
  which IS a reference orbit, and
  [ifs-perturbation-delta.md](ifs-perturbation-delta.md) §8 notes
  it.

## 2. The idea

**Reversing the walk literally returns the pixel.** Forward composed
with inverse is the identity; the escape point pushed back through
its own address is the pixel it came from. What the walk produces
that is worth keeping is the ADDRESS -- the branches whose piece
contains the pixel -- and, at the depth where the piece is the size
of the pixel, the point `q_a = S_a⁻¹(x)`, which is the pixel's
preimage on the attractor at ordinary scale.

**The measure factorises through it.** The invariant measure
satisfies

```
μ = Σ_i p_i · (S_i)_* μ
```

so for any set `B`, `μ(B) = Σ_i p_i · μ(S_i⁻¹ B)`, and unrolled `k`
times

```
μ(B) = Σ_{|a| = k} p_a · μ(S_a⁻¹ B),     p_a = Π p_{a_j}
```

which is [flame-deep-zoom.md](flame-deep-zoom.md) §2's window
identity read the other way. Take `B` a pixel of area `px²` at `x`.
`S_a⁻¹ B` is a region around `q_a` of area `px² · |det J_{S_a⁻¹}(x)|`,
and at the depth where that area is a coarse pixel, `μ(S_a⁻¹ B)` is
the coarse density there times the area. So

```
density(x) = Σ_a  p_a · ρ(q_a) · |det J_{S_a⁻¹}(x)|
```

with `ρ` the density per unit area of an ORDINARY render of the
whole attractor, made once. Three factors per lineage, all of them
things the walk has or can carry: the address's probability, a
texture lookup at its endpoint, and the determinant along it. No
forward pass, and no sample ever generated at the zoom.

**Colour comes the same way.** The flam3 rule applied along the
address in forward order, `a_k` first, from a starting colour `c_0`:
each step damps `c_0` by `(1+s)/2`, so after `k` steps the start
contributes `Π (1+s_j)/2`, which is `2⁻ᵏ` at `s = 0`. The address
decides the colour, and the coarse pass's colour at `q_a` is the
correction, which a second coarse channel supplies where the
damping is not enough (D5). The pixel's colour is the
density-weighted mean over lineages, which is what the accumulator's
rgb already means.

**The mask is free.** A lineage that escapes the ball has
`μ(S_a⁻¹ B) = 0` exactly; one whose endpoint reads `ρ = 0` reads
zero. And `q ∉ A ⟹ S_i⁻¹ q ∉ A` for every `i`, since
`A = ∪ S_i(A)`, so a lineage whose intermediate point reads zero
density is dead at every deeper level: the coarse density is an
exact pruning key for this walk, better than the distance to the
ball's centre the distance walk ranks by (D3).

## 3. Decisions

- **D1. The coarse pass is the flame renderer, over the ball, once
  per flame.** The headless `render_with` at a resolution chosen
  from the ball's diameter (2048 across it to start; a decision
  after G7 measures), with the flame's own iteration budget, cached
  by the flame's hash beside `ensure_ifs_seeds`'s cache. It is
  view-independent: pan and zoom never re-render it. In the app it
  runs on the flame renderer the escape mode already has idle, and
  in the CLI it is one more headless render before the escape pass.
  Its output binds to the mode-D pass as a texture in group 1
  beside `ifs_maps`, both channels: the hit count and the mean
  colour.
- **D2. The stop is per lineage, at the largest region the
  linearisation still describes** -- sixteen coarse cells on an
  affine set (§5c; it was one cell and one cell is wrong), one to
  four on an inversion set (§5d), because the footprint is a
  first-order parallelogram and a curved map outgrows it. At one
  cell neither lookup integrates anything and the estimator reads
  0.24 to 0.89 of the truth; at the right depth it is within 3% on
  every fixture, affine and nonlinear. Carrying the quadratic the
  distance walk already computes would remove the distinction. Lineages expand at
  different rates; on an inversion set some contract and never get
  there -- those read the coarse pixel containing their endpoint,
  which averages the density over a region larger than their true
  preimage. That is the same approximation any texture lookup
  makes, and it is biased where the density varies inside a coarse
  pixel, which D1's resolution bounds.
- **D3. The beam sum first, Monte Carlo second.** The distance walk
  keeps `beam` lineages by the distance to the ball's centre and
  answers a minimum. This walk keeps `beam` lineages by the coarse
  density at their point (the exact key of §2), and answers a SUM.
  On a non-overlapping set one lineage survives and the sum is
  exact; on an overlapping set the beam truncates it, which is a
  bias toward darkness where many addresses cover a pixel. The
  unbiased form is the backward chaos game: at each level choose ONE
  child with probability `p_i` among the admissible ones, weight the
  endpoint by the product of the probabilities the choice skipped,
  and average over passes -- progressive, noisy, and exactly the
  chaos game's own estimator run backward. Built once the beam sum's
  bias is measured on the D9 overlap fixtures (G1), and only if it
  is visible.
- **D4. Units, and what brightness means at depth.** The coarse
  pass's hit count `h(q)` is `N·μ(coarse pixel)`. The deep pixel's
  count is `Σ_a p_a · h(q_a) · |det| · px² / coarse_px²`: "the hits
  this pixel would have had, had every one of the coarse pass's `N`
  samples been targeted". At zoom 1 this reproduces the coarse
  render's own brightness through the same tonemap with the same
  `total_iters` (G4). At depth the measure in the view is tiny and
  the log tonemap goes dark, exactly as the chaos game does when it
  starves and the user compensates by hand. The flam3 normalisation
  is iteration-invariant, not zoom-invariant, so a choice is
  needed: `sample_density` normalised by the VIEW's measure (the sum
  over the frame of the deep counts) makes brightness zoom-invariant
  by construction, and is the default; the flame's own is the
  alternative, kept as a toggle for the shallow case where the two
  differ and the user has calibrated to one.
- **D5. Colour by the address, corrected by a coarse channel.** The
  address rule from `c_0 = col_{a_k}` is exact to `Π(1+s)/2`, under
  one palette step at `k ≥ 8` and `s = 0`; where `color_speed` is
  near 1 the damping is slow and the correction matters. The coarse
  pass therefore also stores the mean PALETTE COORDINATE (not the
  colour) per pixel -- a second coarse render with a colouring that
  plots `c` -- and the lineage starts from it. Direct-colour
  variations (`WritesColor`, `WritesRgb`) set the colour from the
  forward map's value and have no address form; a flame with one is
  refused for this colouring, with the reason.
- **D6. It is a colouring on `ifs_flame`, and the walk knows.** The
  user picks "Measure" beside Distance, Level, Address and Trap.
  The walk variant (sum, density key, per-lineage stop,
  determinant) is selected by the colouring at assembly time, so a
  distance colouring's walk is byte-identical to today's and the
  recolor cache keeps working for it. `IfsRecord` gains nothing:
  the measure writes the accumulator's rgb and a directly.
- **D7. Xaos and finals come from the general plan.** With
  [ifs-general.md](ifs-general.md) D4 the admissible children and
  the row-normalised `p_{i→j}` replace `p_i`; with D5 there a final
  is one more level. Until then the same flames are refused here as
  there.
- **D8. Against the forward plan.** [flame-deep-zoom.md](flame-deep-zoom.md)
  stage 2 enumerates the same addresses forward and forces samples
  through them; it needs the forward maps evaluated at the zoom,
  which is a delta form of every variation in the prefix and a CPU
  evaluation of each, and it gets per-sample colour exactly,
  including direct colour. This plan needs neither and gets colour
  to a damping factor. Build this first; measure what the forward
  plan would add (direct colour; the overlap bias) on real flames;
  decide then.

## 4. What changes, and what does not

| piece | today | here |
|---|---|---|
| mode-D shader | one walk, four colourings | the walk selected by the colouring; the measure walk with sum, density key, stop rule, determinant |
| `Kernel` step in WGSL | position, σ | plus `det J` per step (closed form per kernel; the delta plan's rows carry it once they exist) |
| group 1 | `ifs_maps` | plus the coarse texture (two channels, or two textures) and its extent |
| `ensure_ifs_seeds` | the prefix per view | plus the coarse pass per flame, cached |
| `render_with`, escape branch | one dispatch | a headless flame render first, when the colouring is Measure |
| `IfsRecord`, the distance colourings, the presets | | unchanged |
| the tonemap | `sample_density` from `total_iters` | D4's view normalisation as a parameter it already has the shape for |

## 5. Order of work

0. ~~**The factorisation itself**~~ -- done 2026-09-17, §5a, on the
   CPU with no plumbing at all. Exact on the dragon, approximate on
   the gasket, with the single-cell lookup named as the reason and
   the domain condition `cpx > px` found.
1. **The coarse pass and the lookup.** The headless render into a
   texture over the ball; bound to the mode-D pass; a probe
   colouring that paints the coarse density at the walk's ESCAPE
   endpoint, as a check that the binding, the extent and the
   coordinates agree. No estimator yet. **The lookup's filtering is
   now part of this item, not a refinement of it** (§5a).
2. ~~**The beam's truncation**~~ -- measured 2026-09-17, §5b:
   free on every set tested but the overlapping band, where beam 8
   loses 12% and beam 2 loses 70%. The shipped beam of eight is
   enough. What remains of this item is the SHADER half: probability,
   determinant, stop rule and sum in WGSL, against the CPU
   enumeration as its reference.
3. ~~**Colour**~~ -- measured 2026-09-17, §5f: the address fold is
   four to twenty-six times better than a coarse lookup and its
   error falls toward zero with the zoom, inside one palette entry
   on the affine sets and three on the grand julian. **Units (D4)
   are still open** and are the remaining half of this item.
4. **Monte Carlo** (D3), if G1 shows the beam sum's bias on the
   overlap fixtures.
5. **Depth.** Nothing to build: the walk is the seeded walk, and
   the delta plan's walk when it lands. G5 is the gate.

Items 1 to 3 are item 2 of the delta plan's §9, and they run on
today's walk unchanged.

## 5a. The factorisation holds, 2026-09-17

Proved on the CPU before any plumbing:
`probe_the_measure_through_the_inverse_walk`. A CPU chaos game builds
the coarse density over the ball; the estimator enumerates EVERY
address (no beam -- the formula and the truncation are separate
questions and this is the first one) to the depth at which its
preimage of a pixel reaches one coarse cell, summing
`p_a · ρ(q_a) · |det D(S_a⁻¹)(x)|`; and the reference is a direct
chaos game at the same view, twenty million samples, which is what
mode A actually draws.

**On a measure of dimension two it is exact.** The Heighway dragon,
whose two pieces just touch and whose attractor tiles the plane:

| coarse | zoom | median ratio | p10 | p90 |
|---|---|---|---|---|
| 128 | 2^2 | 0.999 | 0.672 | 1.318 |
| 128 | 2^4 | 0.999 | 0.722 | 1.244 |
| 512 | 2^2 | 1.004 | 0.772 | 1.183 |
| 512 | 2^4 | 0.996 | 0.801 | 1.184 |
| 2048 | 2^2 | 1.017 | 0.757 | 1.362 |
| 2048 | 2^4 | 0.989 | 0.722 | 1.290 |

Six resolution-and-zoom combinations, median within 1.7% of one
every time, and the spread is the reference's own Poisson noise --
it narrows as the coarse resolution rises and the comparison
threshold is only forty samples a pixel. **The three factors are the
right three.** No forward sample was drawn at the zoom.

**On a fractal measure it is approximate, and the size of the
approximation is the open question.** The Sierpinski gasket has
dimension log3/log2 ≈ 1.585, so there is no density per unit area to
look up: `ρ = h/(N·cpx²)` diverges as `cpx^(D−2)`. Medians run 1.05
to 1.35 at equal weights and 0.82 to 1.24 at weights 6:1:1, with one
row at 0.243 whose p10 and p90 are 0.008 and 11.3. The cause is
**the single-cell lookup**: the estimator asks for the density in a
grid square and the truth is the measure of one particular
same-sized region, and on a fractal two same-area regions differ
without bound. It does not improve with resolution, because the
mismatch is scale-free.

The repairs, none tried: integrate `ρ` over the preimage region by
sampling several points rather than one; or filter `ρ` (a mip chain
over the coarse pass) and read it at the region's scale; or carry
the region's shape from the composed Jacobian and read an
anisotropic footprint, which is what texture hardware does for
exactly this reason. D1's resolution question (G7) is downstream of
whichever is chosen.

**And a domain condition that was not in the plan.** The stop rule
`det·px² ≥ cpx²` needs `cpx > px`: the coarse cell must be COARSER
than the view pixel. Below that `want < 1`, the walk takes no step,
and the estimator degenerates to a plain coarse lookup at a scale
finer than the thing it is compared against -- which on a fractal
measure reports `cpx^(D−2)` too much. Every drifting row in the
sweep is one of these, and the dragon is immune to them because
`D = 2` makes that factor one. It cannot arise at a real zoom, since
the coarse pass covers the whole ball and the view is inside it, but
it invalidated a third of this probe's own rows and it is marked in
the output now.

## 5b. The beam is cheap, the lookup is not solved, 2026-09-17

Same probe, extended: the enumeration of §5a is now the reference a
truncated beam is measured against, which is what separates the
formula from the truncation. Two overlapping fixtures joined the
three affine ones.

**The beam costs nothing on a non-overlapping set, and D3 is right
about where it costs.** Ratios to the direct chaos game, median:

| set | enumerate | beam 8 | beam 2 |
|---|---|---|---|
| gasket, equal, 2^6 | 0.879 | 0.879 | 0.879 |
| gasket, 6:1:1, 2^4 | 0.506 | 0.506 | 0.506 |
| dragon, 2^4 | 0.985 | 0.985 | 0.985 |
| fat gasket, 2^4 | 0.984 | 0.984 | 0.984 |
| **overlapping band, 2^6** | **0.722** | **0.638** | **0.204** |

Identical to three decimals everywhere except the band, where many
addresses cover one pixel and a truncated SUM is biased dark exactly
as D3 predicted -- beam 8 loses twelve per cent and beam 2 loses
seventy. The fat gasket, whose pieces also overlap, loses nothing,
so the cost is not "overlap" but how MANY addresses carry real
weight. A beam of eight is the shipped default and is enough on
everything tested; D3's Monte Carlo is for the band's shape of set,
and now has a number to beat.

**The footprint lookup is not the repair §5a hoped for.** Reading ρ
over the preimage of the WHOLE pixel -- the composed Jacobian maps
the pixel square to that region, so it is available for nothing --
instead of at its centre:

| set, zoom | point median / spread | footprint median / spread |
|---|---|---|
| gasket 6:1:1, 2^4 | 0.237 / **36.9x** | 0.506 / **3.4x** |
| gasket equal, 2^6 | 1.053 / 1.65x | 0.879 / 1.28x |
| dragon, 2^4 | 0.995 / 1.22x | 0.985 / 1.16x |
| overlapping band, 2^4 | **1.019** / 1.01x | **0.654** / 1.13x |

It trades bias for variance and neither wins outright. On the
pathological 6:1:1 gasket it cuts the spread elevenfold, which is
the difference between a usable picture and a speckled one. On the
band it reads a third low where the point lookup is within two per
cent. **The bias is structural, not quadrature**: sixteen samples
and a hundred and forty-four give 0.668 and 0.654.

Why is not settled. The suspicion is that with the stop rule putting
the region at about ONE coarse cell, neither estimator is really
integrating anything -- the point lookup reads one cell and the
footprint reads a handful of neighbours, and at that scale the cell
grid is too coarse for either to be the measure of the region. If
that is right the fix is to stop DEEPER, with the region spanning
many cells so the average is a real integral, at the cost of more
addresses. That is a change to D2's stop rule and it is the next
thing to measure. The band fixture is also thin -- sixty-four
comparable pixels -- so its evidence is the weakest here.

Until then the point lookup stays the baseline, and §5a's statement
of the limitation stands unchanged.

## 5c. The stop rule was the bug, 2026-09-17

§5b's suspicion was that with the region at about ONE coarse cell
neither estimator integrates anything, and the fix would be to stop
deeper. Tested by sweeping the stop rule -- how many coarse cells the
preimage region may reach before the lookup -- with the beam held at
16 so nothing else varies. **The suspicion was right, and the
correction is large.**

Median ratio to the direct chaos game, footprint lookup:

| set, zoom | 1 cell | 4 cells | 16 cells |
|---|---|---|---|
| gasket equal, 2^4 | 0.893 | 1.025 | **1.000** |
| gasket equal, 2^6 | 0.879 | 0.947 | **0.987** |
| gasket 6:1:1, 2^4 | 0.506 | 1.007 | **1.006** |
| gasket 6:1:1, 2^6 | 0.741 | 1.059 | **0.977** |
| dragon, 2^4 | 0.985 | 0.995 | **1.000** |
| fat gasket, 2^4 | 0.984 | 0.988 | **0.996** |
| overlapping band, 2^4 | 0.666 | 0.857 | **0.974** |

Seven fixtures and zooms, every one within 2.6% of unity at sixteen
cells, spreads 1.03x to 1.26x. The 6:1:1 gasket -- the pathological
case that read 0.237 with a spread of 37x in §5a -- reads 1.006 with
a spread of 1.26x. **The estimator works.**

**And the point lookup fails the same test, which is the
confirmation.** Reading one cell for a region spanning many gets
worse as the region grows, monotonically and by a lot: the band at
2^4 reads 1.019, 2.083, 4.250, 7.537 as the stop goes 1, 4, 16, 64
cells. A hypothesis that only explained the footprint's improvement
would be a story; this is the same mechanism predicting a
degradation, and the degradation is there.

**Two conditions the measurement puts on the design.**

*The quadrature must scale with the region.* At 64 cells with 16
samples the footprint degrades again (gasket 6:1:1 reads 2.139);
with 64 samples it recovers to 0.987. The rule is roughly one sample
per cell, so sixteen cells wants sixteen or more. Sixty-four texture
reads per address is too many for the shader, and the standard
answer is the one §5a listed and this now selects: **a mip chain
over the coarse pass, read at the region's scale**, which is one
filtered fetch for the same integral. D1 gains that.

*A deeper stop needs a wider beam on an overlapping set.* The band at
2^6 goes the wrong way -- 0.696, 0.742, 0.509, 0.297 -- because a
deeper stop means more addresses and a beam of 16 truncates more of
them. §5b measured the truncation at a fixed stop; this says the two
interact, and that D3's Monte Carlo is wanted exactly where D2 wants
depth. Every other fixture is unaffected, so this is the band's
shape of set rather than overlap as such.

**Settled, then:** footprint lookup, stop at about sixteen coarse
cells, quadrature matched to the region (a mip in the shader), beam
wide enough for the set. The point lookup of §5a and §5b is
withdrawn as the baseline.

## 5d. It works on the sets the project is for, 2026-09-17

Every fixture through §5c is affine, which is not what any of this
exists for. Two nonlinear ones added: a `julia` dust, and the grand
julian of every report this month -- three inversions at powers 2,
15 and 8.

**A root's probability splits between its branches, and that is
load-bearing.** A `julia` is the square root with a random sign: its
FORWARD map is two-valued and the chaos game draws the branch
uniformly, while its inverse -- the square -- is single-valued. For
any point exactly one forward branch has it in its image, and that
is the one the single inverse undoes. So a step of the inverse walk
carries `p_i / n_i`, not `p_i`, where `n_i` is the forward map's
branch count (`|n|` for a root, one otherwise).

Getting this wrong is the branch-0 error of
[ifs-distance-rendering.md](ifs-distance-rendering.md) §8.15 in a
different hat. Measured by removing it: the julia dust goes from
0.983 to **4.295** at a four-cell stop and from 1.028 to **32.890**
at sixty-four cells -- the factor compounding per level, as a factor
of two per branch should -- while the affine dragon does not move at
all (0.995 either way), because it has one branch and nothing to
divide. That control is what says the correction is the right one
rather than a fitted constant.

**With it, the nonlinear sets read true.** Footprint lookup, median
ratio to the direct chaos game:

| set, zoom | 1 cell | 4 cells | 16 cells | 64 cells |
|---|---|---|---|---|
| julia dust, 2^4 | 0.992 | 0.983 | **1.015** | 1.028 |
| julia dust, 2^6 | 0.868 | 0.948 | **1.016** | 0.964 |
| grand julian, 2^4 | **0.994** | **0.996** | 1.028 | 1.027 |
| grand julian, 2^6 | **1.012** | **1.018** | 1.024 | 1.082 |

The point lookup is hopeless on all of them -- spreads of 8x to 42x
against the footprint's 1.3x to 1.5x -- which is §5c's conclusion
holding on a harder set.

**But the stop rule's best depth is not the same as on an affine
set, and the reason matters.** The affine fixtures want sixteen
cells (§5c); the grand julian is best at one to four and its spread
GROWS past that -- 1.33x, 1.33x, 1.87x, 2.22x at 2^4. The footprint
is a FIRST-ORDER description of the preimage region: the composed
Jacobian maps the pixel square to a parallelogram, which is exact
for an affine map at any size and wrong for a curved one once the
region is large. So the deeper the stop, the worse the parallelogram
fits, and an inversion curves hard.

That is the same second-order term
[ifs-perturbation-delta.md](ifs-perturbation-delta.md) already
carries as `Q` for the distance walk, and it would straighten this
too -- the region is a parallelogram plus the quadratic, and the
walk computes the quadratic anyway. Until then D2's sixteen cells is
an affine number, and the honest rule is **the largest stop whose
region the linearisation still describes**, which is a per-set
quantity the walk can measure from `Q`'s size against the basis's.

**What this still does not say.** Nothing about colour or units --
§5 items 3 and after. `bubble` is §5e; `hemisphere` needs nothing,
since `branch_count` reads one for it -- it is 1-to-1.

## 5e. A bubble's branches go the other way, 2026-09-17

§5d's correction divides a root's probability by its forward branch
count. `bubble` is the mirror image and needs the opposite
treatment, which makes it the case that tells the two apart.

A bubble's FORWARD map `4p/(|p|²+4)` is single-valued and 2-to-1, so
its INVERSE has two branches -- and `analyse_2d` already expands that
into two MAPS sharing a `transform_index` (one map per
(transform, branch); `branch_count` reads 2 for Bubble and up to 12
for Disc). Those two maps are alternative PREIMAGES of one forward
map, so the preimage of a set is their union and each carries the
WHOLE of its transform's probability. A root's branches divide the
probability; a bubble's do not.

Which means the probability must be normalised over TRANSFORMS and
not over maps -- and the probe was normalising over maps, which was
invisible on every fixture until this one because no other
transform expands.

| | dragon | grand julian | bubble pair |
|---|---|---|---|
| over transforms (right) | 0.987 | 0.994 | **0.983** |
| over maps (wrong) | 0.987 | 0.994 | **0.035** |

A factor of twenty-eight on the bubble and nothing at all on the
other two, because their transforms are one map each. The size is
the walk's depth: halving each step's probability costs `2^-k`, and
`2^-5` is 0.031 against the 0.035 measured. Controls that do not
move are what make a correction a correction.

With it the bubble pair reads 0.983 and 0.984 at one- and four-cell
stops, spread 1.23x and 1.25x -- the same quality as every other
fixture. Its deeper stops degrade like the grand julian's (0.920 at
sixteen cells, 0.619 at sixty-four) and for the same §5d reason, the
first-order footprint outgrowing a curved map.

Not covered: `disc`, whose twelve branches are the same shape of
correction as bubble's two and which is the one kernel where
`branch_count` depends on the ball.

## 5f. Colour comes out of the address, 2026-09-17

§2's colour claim, measured. The flam3 rule
`c' = c(1+s)/2 + col_i(1-s)/2` folded along the address in FORWARD
order -- the walk applies `S_{a_1}⁻¹` first, so the forward sequence
runs `a_k` first and `a_1` LAST, and the shallowest branch dominates
with weight a half -- from a `c_0` read out of a second coarse
channel (D5's mean palette coordinate). Each fixture's transforms
were given distinct colours spread over the palette and
`color_speed` zero, so a right answer is not right by everything
being equal. The reference is the direct chaos game's own mean
palette coordinate per pixel.

**The control has to be chosen carefully or it says nothing.** The
first run put the coarse grid at 512 across the ball, where a cell
spans two to eight view pixels -- and a plain coarse lookup AT THE
PIXEL was as good as the address rule, because at that scale it is
just reading the answer out of a coarse render of the same thing. At
64 across the ball, where a cell spans sixteen to sixty-four view
pixels and a coarse lookup has to average over all of them, the
comparison means something:

| set, zoom | address rule | coarse lookup |
|---|---|---|
| gasket equal, 2^2 | 0.0010 | 0.0034 |
| gasket equal, 2^4 | 0.0003 | 0.0039 |
| gasket equal, 2^6 | **0.0000** | 0.0023 |
| dragon, 2^2 | 0.0010 | 0.0122 |
| dragon, 2^4 | 0.0001 | 0.0052 |
| fat gasket, 2^4 | 0.0002 | 0.0032 |
| grand julian, 2^4 | 0.0134 | 0.0409 |
| grand julian, 2^6 | 0.0128 | 0.0440 |
| bubble pair, 2^4 | 0.0032 | 0.0228 |

Median absolute error in the palette coordinate, which runs 0 to 1;
one entry of a 256-colour palette is 0.0039. **The address rule is
four to twenty-six times better than the lookup, and its error falls
toward zero as the zoom deepens** -- 0.0010, 0.0003, 0.0000 on the
gasket -- which is the `2⁻ᵏ` damping of `c_0` doing exactly what §2
said it would. On the affine sets it is well inside one palette
entry; the grand julian's 0.013 is three entries and the largest
seen.

The `julia` dust reads 0.0000 both ways and is no evidence: it has
ONE transform, so every address carries the same colour.

**And a coarse-resolution floor, found by accident.** The 64-across
grid that made the colour control honest broke the DENSITY on the
bubble pair -- median 0.518 at a four-cell stop against 0.984 at 512
across, and 0.036 at sixty-four cells. §5a's sweep of 128, 512 and
2048 found the formula resolution-stable, and 64 is simply below the
useful range: the attractor occupies a fraction of 4096 cells and ρ
is not resolved. D1's 2048 stands; the floor is somewhere between 64
and 128 and is set by how much of the grid the attractor covers,
which is a per-flame quantity worth reporting beside the Extent.

The colour numbers above are therefore measured in a regime where
the density weights are approximate on two of the fixtures. The
comparison survives it -- both variants share the weights, so the
ratio between them is unaffected -- but the absolute colour error on
the bubble pair and the grand julian would be worth re-reading at
2048 once D1 is built.

## 5g. The estimator is an API and a gate, 2026-09-17

Everything in §5a to §5f lived inside one probe, which means it was
a measurement and not a capability. Promoted:

- `CoarseMeasure` -- an ordinary render of the whole attractor as the
  hit count and the summed palette coordinate per cell, with
  `density` and `palette` readers. D1's coarse pass, as a type.
- `MeasureMaps::of(ifs, flame)` -- the per-map probability and
  colour, and the **one place both branch corrections live**: a
  root's probability divided by its forward branch count (§5d), and
  the normalisation over transforms rather than maps that a bubble
  needs (§5e). Each is commented with what it measured and with the
  control that did not move.
- `estimate_measure(...) -> MeasureEstimate` -- the density, the
  palette coordinate and the address count, with the footprint
  lookup, the `MEASURE_CELLS` stop rule and the beam.
- `MEASURE_CELLS = 4.0`, documented as the compromise: sixteen is
  right on an affine set, four on a curved one, and four is within
  3% on both.

And `the_measure_agrees_with_the_chaos_game` is a GATE, not a probe
-- it runs in the ordinary suite in four seconds. Four fixtures,
chosen because each breaks differently and all four are needed to
pin `MeasureMaps`: the dragon (affine), the 6:1:1 gasket
(non-uniform weights), the grand julian (a root's forward branches),
and the bubble pair (an inverse's branches). Six million samples
each side, at 2^2 and 2^4:

| fixture | stop | density 2^5 | 2^6 | colour 2^5 | 2^6 |
|---|---|---|---|---|---|
| dragon | 16 cells | 0.998 | 0.971 | 0.0001 | 0.0002 |
| gasket 6:1:1 | 16 cells | 0.987 | 1.032 | 0.0002 | 0.0000 |
| grand julian | 4 cells | 0.956 | 0.901 | 0.0087 | 0.0086 |
| bubble pair | 4 cells | 0.973 | 0.980 | 0.0026 | 0.0038 |

**Each fixture gates at its own stop depth, and that is the point.**
An affine map's preimage of a pixel is exactly the parallelogram the
composed Jacobian describes, at any size, so it can stop deep; a
curved one outgrows it. The 6:1:1 gasket reads 1.234 at four cells
and 1.032 at sixteen, and the bubble pair reads 0.957 at four and
0.894 at sixteen -- opposite directions, and one constant for both
would hide it. Checked that the gasket's 1.234 is the estimator and
not the reference: at 40M samples rather than 6M it reads 1.267, so
it does not converge.

Held to `0.88..=1.15` on the median density and 0.02 on the median
palette error. The tolerance is the REFERENCE's noise at this sample
count, not the estimator's accuracy.

**Checked that it can fail, both ways.** With the root's branch
division removed it reports 5.276 on the grand julian; with the
normalisation over transforms replaced by one over maps it reports
0.128 on the bubble pair. Each correction has its own fixture and
its own failure.

**Two things the gate found while being built.**

*The view has to sit where the estimator is defined.* The first
version ran at 2^2 and 2^4 with forty pixels across and a 256-cell
coarse grid, which puts the coarse cell FINER than the view pixel --
§5a's degenerate regime. In it the 6:1:1 gasket read 0.864 and 1.137
and the colour read 0.085. Moved to `VP·2^zoom >= 2·RES` (sixteen
pixels at 2^5 and 2^6) the same fixture reads 0.997 and 1.001 with a
colour error of 0.0000. None of that was the estimator.

*The colour must read the same footprint as the weight.* It did not:
the weight averaged ρ over the preimage region while the colour
point-sampled the palette at its centre. An address whose footprint
straddles a populated cell while its centre sits in an empty one
then gets a positive weight and the palette's mid-grey fallback. On
a sparse attractor that is common, and it cost the grand julian
0.057 in palette coordinate where every other fixture read under
0.001. Reading both from one footprint, weighted by the measure at
each sample, takes it to 0.0118.

**`disc` is gated too, on a fixture built for it.** The obvious one
-- the fixture that kernel's other tests use -- has a single POINT
for an attractor: both its maps fix the origin and both contract
toward it, so six million chaos-game samples land in one coarse cell
of 65536, and the pre-existing `chaos_sample` helper collapses there
as well, so it is the fixture and not the sampler. **Other tests
using that fixture are asserting less than they look like they
are**, which is worth someone's attention separately.

Translations give the maps different fixed points.
`probe_hunt_a_disc_fixture_that_spreads` tried five arrangements; a
disc with two affines spreads over 7631 cells of a 128-grid, and on
it the estimator reads 0.955 and 0.959 with a colour error of
0.0010. That closes the last kernel class: affine, a root's forward
branches, a bubble's inverse branches, and now a disc's twelve.

The gate asserts a fixture's coarse pass lights more than a hundred
cells, which is what caught the degenerate one and what stops any
future fixture from passing by having nothing to measure.

## 5h. Units, and what the zoom does to brightness, 2026-09-17

D4's measurable half was already settled by every row above: the
estimator answers in the same units as [`CoarseMeasure::density`],
which is why a ratio against an ordinary render of the same view is
one and why the gate can assert that at all. What was left is the
policy, and `probe_what_brightness_does_at_depth` gives it a number.

Two quantities as the zoom climbs from 2^5 to 2^20, in stops:

| set | measure the view holds | median density per unit area |
|---|---|---|
| dragon | **−30.0** | 0.0 |
| gasket | −23.3 | +6.5 |
| grand julian | −21.6 | +7.4 |

**The first column is the starvation, measured.** A chaos game has to
find that measure by sampling, so holding brightness at 2^20 would
take 2^21 to 2^30 times the samples. That is the wall this plan
exists to go around, and the estimator does not care: it reads the
same coarse pass at every zoom.

**The second column is the attractor's dimension, read off the
picture.** Density per unit area scales as `2^(z(2−D))`, so the rate
IS `2 − D`. The dragon's measure is two-dimensional and its density
is flat to 0.0 stops over fifteen zoom levels; the gasket gains 6.5
over fifteen, a rate of 0.43 against `2 − log3/log2 = 0.415`; the
grand julian gains 7.4, a rate of 0.49, so its measure carries
dimension about 1.51. A quantity that falls out of the estimator and
agrees with the arithmetic to two digits is a good sign the units
are right.

So the tonemap's choice is between a brightness that falls by
twenty-odd stops (the flame's own normalisation, which is
iteration-invariant and not zoom-invariant) and one that divides by
the view's own measure and does not. D4's default stands.

**And the probe found a trap that was in the gate too.** Centring a
view on the densest coarse CELL is not the same as centring it on
the attractor: a cell that is dense on average can have the set
nowhere near its geometric middle, and once the view is smaller than
a cell it misses entirely. The grand julian read a flat ZERO past
2^12 for that reason and nothing was wrong with the estimator.
Centring on a chaos-game sample is not enough either -- a random
attractor point on the 6:1:1 gasket left eight comparable pixels in
frame. Both now centre on **the attractor sample whose coarse cell
holds the most measure**, which is on the set and where the
reference has the statistics to be one.

## 5i. The shader's half, begun: the coarse pass reaches the GPU, 2026-09-17

The estimator is settled on the CPU, so the remaining work is the
shader. Built and verified so far, as the plumbing that carries the
coarse pass across:

- `pack_coarse` flattens a [`CoarseMeasure`] to a header of
  `(res, radius)` and the grid's centre, then one `vec2<f32>` of
  (density per unit AREA, mean palette coordinate) per cell. Density
  rather than a hit count, because a count means nothing without the
  sample total and the cell size and the shader would have to be told
  both.
- `read_coarse` is the reader the WGSL will mirror, and
  `the_packed_coarse_grid_is_the_measure_it_came_from` checks a
  round-trip at every cell's middle and near its corner, over a
  fixture with holes in it so the empty-cell path is exercised, and
  that outside the ball reads no measure. **A packing with no reader
  beside it is a stride waiting to disagree** -- `SEED_VEC4S` went
  from four words to six with the shader's stride left a literal
  four, invisible on a one-seed walk and 11% wrong on eight.
- Group 1 gains binding 3, a read-only storage buffer, created at one
  dummy element and bound for every mode-D walk. A layout entry the
  shader does not read is allowed and costs nothing, which is the
  same reasoning that binds the seed chain for the planar walk.
  `EscapeRenderer::set_coarse` grows and writes it, and bumps
  `ifs_token` on a resize because a new buffer is a new binding and
  every cached bind group naming the old one is stale.

**Verified inert.** All 1214 unit tests pass and the eleven shipped
mode-D presets are byte-identical to `output/ifs-before3/` with the
binding added, which is what says the layout change cost nothing.

**What is left, and it is specified rather than guessed.** The
template must select the WALK, not just the colouring -- a mode-D
colouring receives an `IfsResult` and cannot run a different walk
(D6). So:

1. A `MEASURE` template flag in the mode-D assembler, beside the
   existing `SOLID` one, so the WGSL is byte-identical when off.
2. `ifs_measure(uv) -> vec2<f32>` in WGSL, transcribed from
   `estimate_measure`: a frontier of (point, probability, composed
   Jacobian, two colour accumulators), the determinant as the stop
   test against `cells·(cpx/px)²`, the 4×4 footprint reading density
   and palette together, and the beam kept by largest contribution.
   **No address is carried and none is needed** -- the colour fold
   accumulates forward (§5j), so a lineage is nine floats and there
   is nothing to size.

   **It needs the six kernel Jacobians in WGSL, and two cheaper ways
   round that were tried and measured worse.** Carrying three points
   -- the pixel's centre and its two edge neighbours, pushed through
   the same inverses, whose parallelogram is the preimage without any
   derivative -- costs the same six floats and reads **0.617** on the
   grand julian against 0.956, because under an expanding inverse the
   three separate until the parallelogram is a secant over a region
   the map has curved right out of. A local central difference per
   step keeps it tangent but reads **0.790** on the 6:1:1 gasket
   against 0.987. So the analytic Jacobian stays, and porting the six
   is mechanical rather than new: `Kernel::inverse_jacobian` exists
   in f64 and `the_kernels_jacobians_are_the_derivative` already
   gates it to 3.5e-5.
3. Per-map probability and colour speed. `IfsMapGpu` has no spare
   word, so this is either a second small storage buffer or two more
   floats on the row; the row is 80 bytes and already has `color`,
   so extending it is the smaller change but touches the layout gate.
4. The gate: render at 2^5 and 2^6 on the five fixtures, read the
   density back out of the recolor cache as
   `probe_what_the_f32_term_costs_on_the_gpu` does, and compare
   against `estimate_measure` in f64 -- not against the chaos game,
   which is the CPU reference's job. A shallow control where the
   walk takes no step separates the arithmetic from the walk.

## 5j. The colour fold needs no address, 2026-09-17

The flam3 rule runs `a_k` first and `a_1` last, the reverse of the
order the walk discovers them in, so `estimate_measure` carried each
lineage's whole address and folded at the end -- a heap allocation
per lineage, and nothing a shader could size.

It does not need the address. With `h = (1+s)/2` and
`g = col·(1−s)/2` for a map, the reversed fold is

```text
c = c_0 · ∏_{j≤k} h_j  +  Σ_i g_i · ∏_{j<i} h_j
```

and that inner product is over the PREFIX `a_1..a_{i-1}`, which the
walk already has in hand. A lineage carries the running product and
the running sum, two floats, and the fold needs no history at all.
Verified by the gate: identical to the last digit on all ten rows.

This is what makes the shader's version sizeable. The earlier note
that the address "needs only its last few entries, and that bound
should be measured" is moot -- it needs none.

## 5k. The kernel Jacobians are in the shader, 2026-09-17

§5i settled that the measure walk needs them and that the two cheaper
ways round were worse. Ported: `IFS_JACOBIAN`, a WGSL const holding
`ifs_bubble_dscale`, `ifs_kernel_jacobian` over all six kernels, and
`ifs_map_jacobian` composing `pre · J_kernel · inv` -- the shader's
own chain, since its first affine is the post-inverse with `1/w`
folded in.

**Its own const, not text inside `IFS_TEMPLATE`.** It depends on
nothing but the map rows and `ff_atan2`, so
`the_shader_jacobians_are_the_cpu_ones` compiles it against a
twenty-line harness with three bindings and checks the arithmetic,
instead of standing up the whole walk to reach it.

Against [`Kernel::inverse_jacobian`] in f64, four hundred points per
kernel spread over the ball, skipping points the CPU declines or that
sit within a hundredth of the ball of a singularity -- a pole is not
a disagreement, it is a place with no derivative:

| kernel | worst entry, relative |
|---|---|
| root n=3 d=1 | 1.4e-6 |
| root n=15 d=-1 | 6.2e-6 |
| spherical | 4.5e-7 |
| bubble | 3.8e-6 |
| hemisphere | 9.3e-6 |
| disc | 5.2e-7 |
| blob | 4.6e-6 |

That is f32's own precision, and the comparison is **every entry**
rather than a norm, because WGSL matrices are column-major and a
transposed one still renders a picture. Checked: transposing the
root's reads 1.99 relative and fails.

Two fixtures had to be built for it rather than borrowed -- the plain
affine one has no variations and does not qualify, and a bare blob
has no invariant ball -- which is the third time this project has
found a kernel fixture that could not carry a test.

The const is not spliced into the walk yet, because nothing reads it
until `ifs_measure` exists. That is the next piece.

## 5l. The measure walk runs on the GPU, 2026-09-17

Built: `IFS_MEASURE` (the coarse binding, its reader, the footprint,
and the walk), the `ifs_measure` colouring, and the splice. **The
walk is selected by the COLOURING at assembly time**, because a
mode-D colouring receives an `IfsResult` and never the pixel, so it
cannot run a walk of its own (D6). `assemble_ifs` swaps the
`let res = ifs_evaluate(uv);` line for a measure block and splices
`IFS_JACOBIAN` and `IFS_MEASURE` beside the formula -- so every other
mode-D shader is byte-identical, which the eleven pixel-identical
presets confirm.

The answer rides in two of `IfsResult`'s fields: `distance` carries
the density and `color` the palette coordinate. That is the one place
mode D reuses those names, and it is why the colouring only makes
sense with this walk.

**It is exact on every fixture** -- though it took §5n to get there,
and two of the three read wrong until then for a reason that was
never the shader's:

| | density ratio | colour error |
|---|---|---|
| dragon | **1.0000** | **0.00000** |
| gasket | **1.0146** | **0.00000** |
| julia dust | **1.0000** | **0.00000** |

Against `estimate_measure` in f64 from the same coarse pass. That
exercises the whole machine: the packing, the coarse binding and
lookup, the six Jacobians, the composition, the stop rule, the beam,
the probability, the footprint and the colour fold.

**Two fixtures disagreed, and this section's explanation of them was
WRONG.** It blamed the beam choosing between equally-keyed lineages
on sets whose maps are alike up to a translation, and had the gate
report those two rather than assert on them. Both were in fact the
forced level not reaching the walk, in two separate places (§5n).

The ties are real -- giving the gasket unequal weights took its
colour from 0.145 to 0.014 -- but they were not what either fixture
was showing, and believing they were let the gate stop asserting on
exactly the two cases that had something to say. Every fixture is
asserted again.

**And the walk was correct only at handover level 0** until §5o.
That limit is now lifted; what follows is why it existed. The seeds carry a position and a basis but not the
probability or the two colour accumulators of the prefix that reached
them, so a deeper handover silently drops three numbers. Carrying
them is three more floats on `Seed` and is the deep-zoom follow-on --
without it this paints the measure only as far as a pixel's own f32
position reaches, about 2^17.

## 5m. The coarse pass is real, and the colouring was black, 2026-09-17

§5l shipped a colouring that rendered **black**, and nothing caught
it. The only caller of `set_coarse` was its own gate: in the app the
buffer stayed at its one dummy element, every lookup read no measure,
and every pixel came back zero. A feature reachable from the panel
that draws nothing is worse than one that is not there.

**D1, built.** `coarse_measure_for` renders the flame's own measure
over its ball with the renderer that draws every other flame.

*The palette is an inverse-sRGB grey ramp, and that is the trick.*
The chaos game plots `srgb_to_linear(palette(c))`, which is
`palette(c)^2.2`, and the accumulator keeps the density-weighted MEAN
of it. A ramp storing `t^(1/2.2)` therefore plots exactly `c`, so the
accumulator's red channel comes back as the mean palette coordinate
with no second render and no engine change.

*The framing follows `world_to_pixel`*, which maps
`(p − pan)·zoom·min(w,h)/4` about the centre, so a view spanning the
ball exactly is `pan = ball.centre`, `zoom = 2/radius`.

*And it is normalised by what LANDED, not by what was dispatched.*
Counting the dispatch read **12.5x** the chaos game's density.
Chasing which constant that was -- the histogram's `color_scale`, the
burn-in the dispatch counts and the plot does not, the samples a
bad-value respawn drops -- would have been chasing a number that
cancels: the invariant measure is a probability measure, so the
divisor is the sample total that reached the grid, and every constant
divides out.

`the_rendered_coarse_pass_is_the_chaos_games` checks the two
properties the walk uses, against a CPU chaos game over the same
ball -- not pixel by pixel, since the two draw different samples:

| | cells lit by both | overlap | density ratio | palette error |
|---|---|---|---|---|
| gasket | 5350 | 0.899 | **1.000** | 0.0083 |
| dragon | 9667 | 0.993 | **1.000** | 0.0081 |

The overlap is what sees a framing error -- a shifted, scaled or
flipped grid lights different cells -- and the palette column is what
says the ramp inverted the gamma.

**Wired at both production sites.** `EscapeRenderer::ensure_coarse`
builds and uploads it beside `set_ifs`, keyed on `ifs_token` so a
flame edit rebuilds it and a pan or a zoom does not, and only for the
measure colouring so nothing else pays. 1024 across the ball: §5a
found the estimator resolution-stable over 128, 512 and 2048 while
§5f found 64 too coarse to be a measure, so the floor matters and the
ceiling buys little.

**And a gate that goes through the render path**, which is the only
kind that could have caught the black frame.
`the_measure_colouring_draws_through_the_render_path` calls
`renderer::render` as the CLI and the app do, and asserts a picture
rather than a value: 2157 of 9216 pixels lit, 185 distinct
brightnesses, and of the 867 pixels the DISTANCE colouring calls
exterior, **none** are lit. The measure sits on the set.

## 5n. The gasket's gap is a stop two levels short, 2026-09-17

§5l guessed f32 for the shader's 2.4x to 3.4x density gap on a
gasket. **That guess was wrong, and the measurement that killed it is
worth keeping**: at the points the lookup actually happens the median
`|q|` is 0.85, one f32 ulp there is **0.0000 of a coarse cell**, and
rounding every footprint sample to f32 moves `ρ` by a factor of
**1.000** on all three sets (`probe_whether_the_measure_gap_is_f32`).
The walk does not expand to huge magnitudes the way the guess
assumed. The lookup is not precision-sensitive at all.

What it is: **the shader stops two levels short.** Reporting the stop
depth from both sides --

| set | shader depth | reference depth |
|---|---|---|
| dragon | 8 | 12 |
| gasket | 4 | 6 |

-- and the two differ by exactly 16x in AREA every time, since a
dragon's inverse doubles area per level and a gasket's quadruples.

Localised further. The stop compares a region's area against
`cells·cpx²`, and `cpx` agrees between the two to every digit. The
difference is the PIXEL's own area, from which every region's grows:
the shader's is 16x the reference's, so the seeds' basis is 4x per
axis wider than the view the same `EscapeConfig` implies. It is not
the gate's arithmetic -- rewriting the reference to derive its span
the way `ensure_ifs_seeds` does, from `4/zoom_factor` and
`view_basis`, changed nothing.

**This is a real defect, not a gate artefact**: the shader walks two
levels shallower than intended on every set, and §5c measured a
too-shallow stop reading 0.24 to 0.89 of the truth. The dragon hides
it because a dimension-two measure has a scale-invariant density and
cannot tell the depths apart; the gasket, at dimension 1.585, can.
The julia dust reads exact, which says the rest of the walk is right.

**Found, and it was the test hook rather than the shader -- twice.**

`seed_beam_at` did not force a level on an AFFINE set. The
level-choosing machinery is skipped for affine maps -- one pays no
curvature, so the deepest handover always wins and there is nothing
to choose -- and the forced level rode inside it. `force` is not the
objective; it is a test asking for a particular level, and it now
applies either way.

And `ensure_ifs_seeds` builds the centre two ways -- at the zoom's
precision through `centre_at_precision`, and from f64 when those
strings will not parse -- and only the first carried the force.
`from_decimal` takes plain decimals, and a small coordinate written
by `{:?}` comes out in scientific notation, so the gasket's centre
fell to the f64 path and the force was dropped there. That is why
fixing the first place left the gasket unchanged, and why the dragon
moved and it did not.

With both fixed every fixture is exact: dragon 1.0000, gasket
1.0146, julia dust 1.0000, and all three colours 0.00000. The grand
julian's cap measurement is unchanged to every digit and the eleven
presets stay byte-identical, so nothing the objective chooses moved.

**What this cost.** Three explanations were offered before the right
one: f32 precision, tied beam keys, a transcription error. The first
two were written into this document as findings and are struck
through above. The measurement that settled it -- reporting the stop
DEPTH from both sides -- took a single run, and every guess before it
was made without that number in hand.

## 5o. The measure walk takes the handover, 2026-09-17

§5l's limit: the walk was correct only at handover level 0, because
the seeds carry where a lineage IS but not what it has ACCUMULATED.
Three numbers were missing -- the product of the branch probabilities
that reached it, and the two running terms of the colour fold -- and
a deeper handover dropped them silently.

**Packed into the three slots the layout already had free**, at word
3's last and word 5's last two. No stride change, so `MAX_SEEDS`
stays at ten where the beam allows eight. Folded in `pack_seeds`
from `Seed::address`, which is an exact list of branches -- the
packed address is a base-N fraction and loses its tail, so this is
the one place the fold can happen. `PackedIfs` carries the
`MeasureMaps` for it, which `pack_flame` was already building for the
rows.

The shader then starts its frontier from EVERY seed rather than from
seed 0, each with its own position, basis, quadratic and prefix.

**Two bugs on the way, and the second is the interesting one.**

The probability reached the shader correctly -- seed 0 read 0.5 with
a count of 2 on a two-map set -- and the density still came out at
exactly **2.0000**. The fault was `pixel_area`, which I took from a
seed's basis. A seed's basis is the view composed with the prefix's
Jacobians, so at a deep handover it has already been expanded by the
walk; dividing by it cancels the prefix back out of every
determinant. `params.span` is the view and nothing else.

With that fixed the affine fixtures are exact at every handover level
tested:

| | L0 | L1 | L2 | L4 |
|---|---|---|---|---|
| dragon | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| gasket | 1.0146 | 1.0146 | 1.0146 | 1.0146 |
| julia dust | **1.0000** | 1.2959 | 1.2959 | 1.2959 |

**The julia dust's 1.296 is not a bug and not accumulating.** A
handover hands over a LINEARISATION: the seed's basis is the Jacobian
at the reference and every pixel continues from it. For an affine map
that is exact at any depth, which is why the two affine rows do not
move. For a curved one it is not, and the figure is identical at L1,
L2 and L4 because that walk reaches level 1 and no further -- one
nonlinear step, one fixed offset. It is §5d's finding arriving at the
handover, and it is what the delta plan's quadratic carry would
straighten. The gate holds the affine sets at every level and the
curved one at level 0.

**So the measure now zooms as far as the distance walk does**, which
was the point of the exercise: the coarse pass is view-independent,
the handover carries the prefix, and no forward sample is drawn at
the zoom.

## 6. Gates

- **G1. The measure is the chaos game's.** At 2^2 to 2^6 on the
  Sierpinski gasket, the dragon, a julia and the three D9 overlap
  fixtures: the measure walk's density against a converged chaos
  game of the same flame at the same view, compared as
  log-densities (the chaos game is noisy; the tolerance is its own
  variance, measured from two seeds). The gasket and the dragon must
  agree to that tolerance at beam 1; the overlap fixtures report
  their bias per beam width, which decides D3's second half.
- **G2. Colour.** The same views, the mean palette coordinate per
  pixel against the chaos game's, with and without the coarse
  correction, at `color_speed` 0, 0.5 and 0.9.
- **G3. Xaos and finals**, when [ifs-general.md](ifs-general.md)
  D4 and D5 land: G1 on a xaos flame and on a flame with a final.
- **G4. Units.** The coarse flame rendered through the measure walk
  at zoom 1 with the flame's own tonemap settings, against the
  coarse render itself: identical to the tonemap's quantisation.
- **G5. Depth.** The delta plan's G6, zoom self-consistency, on the
  measure: the centre quarter at 2^z downsampled against 2^(z−2),
  for z to 40 today and to 64 with the delta walk.
- **G6. GPU equals CPU.** `estimate_measure` against the shader at
  2^4 and 2^20, as the distance gates do.
- **G7. Cost.** The coarse pass once per flame (seconds, stated);
  the measure walk per pixel against the distance walk's at 2^4 and
  2^20; the resolution of D1 against G1's agreement, to choose it.

## 7. Cost and risk

| risk | consequence | what bounds it |
|---|---|---|
| the coarse pass has zero hits in a low-density region that is on the set | lineages pruned by the density key that should live; a piece missing | a threshold under which the key does not prune (a hit count of one is not zero); the coarse budget is the flame's own and can be raised; G1's gasket has no low-density region and the julia has, so it shows |
| the beam sum's bias on overlapping flames is large | dark where the flame is bright | G1 measures it per beam; D3's Monte Carlo is the answer if so |
| brightness at depth reads wrong to a user calibrated on the chaos game | "it doesn't look like my flame" at depth | D4 says which normalisation is on and offers the other; the shallow case is pinned by G4 |
| the coarse pass at 2048² is too coarse for a set with fine density structure | intra-pixel bias in the lookup, blur at depth | G7 measures agreement against resolution; a resolution parameter beside Extent |
| ~~the single-cell lookup on a fractal measure~~ | ~~medians off by tens of per cent~~ | **Solved, §5c**: footprint lookup with the stop at sixteen cells reads within 2.6% on every fixture. The residue is the quadrature's cost, which a mip chain answers |
| a deeper stop needs a wider beam where many addresses carry weight (§5c) | the overlapping band reads 0.30 at 2^6 with beam 16 | D3's Monte Carlo, wanted exactly where D2 wants depth; every other fixture is unaffected |
| the footprint is a first-order parallelogram and a curved map outgrows it (§5d) | the grand julian's spread doubles between a four-cell and a sixty-four-cell stop | D2 stops at the depth the linearisation still covers; the quadratic the distance walk carries would lift it |
| a root's probability must be split between its forward branches (§5d) | the julia dust reads 4.3x at depth 2 and 32.9x at depth 6 without it | measured, corrected, and the affine control pins it: the dragon does not move |
| a bubble's INVERSE branches must NOT be (§5e) | normalising over maps rather than transforms reads 0.035 against 0.983 | measured, corrected; the two kernels need opposite treatment and the fixture set now contains both |
| the coarse grid can be too coarse for the density (§5f) | a 64-across grid reads 0.52 on the bubble pair where 512 reads 0.98 | D1's 2048 stands; the floor is between 64 and 128 and depends on how much of the grid the attractor covers |
| the determinant along a contracting lineage underflows f32 | a lineage weighted zero that should count | the same scaled-float care the delta walk takes with σ; carry `log det` |

## 8. Deliberately not here

- **A forward pass at the zoom** and everything that needs one:
  direct-colour variations, per-sample effects inside the view. D8
  says when to revisit.
- **3D.** The measure of a solid is a volume density; the solid
  pipeline's occlusion and lighting would read it as a volume, which
  is the retired density-volume experiment's territory. After the
  planar measure has shipped and been looked at.
- **The hybrid** the design doc's §7 names -- the distance as a mask
  or a trap inside the chaos game. The measure colouring subsumes
  the mask; the trap stays a colouring of the distance walk.
