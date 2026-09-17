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
- **D2. The stop is per lineage, at SIXTEEN coarse cells** (§5c,
  measured; it was one cell and one cell is wrong). A lineage walks
  until `|det J_{S_a⁻¹}| · px²` reaches sixteen times the coarse
  pixel's area, or it escapes, or it reads zero. At one cell neither
  lookup integrates anything and the estimator reads 0.24 to 0.89 of
  the truth; at sixteen it is within 2.6% on every fixture. Lineages expand at
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
3. **Colour and units.** D4 and D5; G2 and G4.
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

**What this still does not say.** Nothing about colour, units, or a
nonlinear or inversion set -- every fixture is affine. Those are §5
items 3 and after.

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
