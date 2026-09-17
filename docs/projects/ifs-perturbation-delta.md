# Perturbation in delta form: the second design (plan, 2026-09-17)

The first design is
[ifs-nonlinear-perturbation.md](ifs-nonlinear-perturbation.md): the
CPU walks the view centre's inverse orbit in `BigFloat`, hands over
ONCE at a chosen level, and every pixel continues from an f32
absolute position. §17a there records what a review of it found, and
this plan is the design that review points to. It is one of three
written together:

- this one -- the reference/delta split carried through EVERY level,
  which is what "perturbation" means in the escape engine and was
  not what the first design built;
- [ifs-general.md](ifs-general.md) -- the math a map has to supply
  moved into the variation that owns it, so the set of IFSs this
  reaches is the catalogue rather than six kernels;
- [ifs-measure-by-inverse-walk.md](ifs-measure-by-inverse-walk.md)
  -- the chaos game's density read through the walk, which is the
  flame look at any zoom and needs both of the above.

The three are ordered at the end (§9), because each is worth less
without the others and the cheap parts of all three come first.

**What it buys.** The grand julian's cap, measured at 2^30 to 2^36
depending where on the set you are, comes from two things the first
design cannot fix by choosing better: a lineage whose view has
COLLAPSED pays f32's whole ulp at any handover, and a lineage whose
view has EXPANDED pays the linearisation's curvature at any handover
deep enough for f32 to be cheap. In delta form neither is paid. A
collapsed lineage keeps a tiny delta at full relative precision and
never hands over; an expanded one rebases where its delta is O(1),
where absolute f32 is exact by construction; and for the rational
kernels the step is an exact difference, not a Taylor step, so there
is no curvature term at all. The level to hand over at -- the whole
of the first plan's §9 to §17 -- stops being a question.

**What it does not buy, said first.** Not the branch problem: a
pixel whose beam ranks differently from the reference's still has to
leave the reference, and leaving costs what the first design's
handover cost, at the level it happens. Not the bound's looseness,
the cut's image, or anything in
[ifs-distance-rendering.md](ifs-distance-rendering.md) §8.15. And
not any set the walk does not accept today: that is
[ifs-general.md](ifs-general.md)'s job.

---

## 1. What is there, and what is wrong with it

The first design, as built (its §1, §2, §13):

- `seed_beam` walks the centre's beam in `BigFloat`, carrying per
  candidate a basis `B_k = J_k · B_{k-1}` and a quadratic
  `Q_k = J_k Q_{k-1} + ½ H_k[B_{k-1}, B_{k-1}]`, and stops at the
  first of: the view disagreeing on the beam (`view_agrees`), any
  delta reaching a quarter of the ball (`HANDOVER_FRACTION`), a
  branch the reference cannot take whose gap it carries, or the
  level budget.
- Among the levels it reaches it picks the one minimising a MEASURED
  curvature (five probe continuations against the level-0 handover)
  plus a MODELLED f32 cost (`|q|·2⁻²⁴ / (px · reach_k/reach_0)`).
- The shader starts every pixel at `position + B·uv + Q(uv)` in f32
  and walks on in absolute f32.

What the review found (first plan, §17a), in the order it matters:

1. **The f32 model is pessimistic by 3× to 800×** on 42 rows and
   never optimistic. The objective has been refusing usable levels
   on the strength of it. The measurement that found this rounds the
   handover to f32 and continues in f64, so it prices the handover's
   rounding and not the f32 arithmetic after it; only a GPU render
   at a forced level says where the truth is.
2. **The objective's reference is f64** and at 1080p a pixel is 15
   f64 ulps at 2^40 and a fifth of one at 2^46. Past about 2^38 the
   walk chooses levels with a number that means nothing.
3. **The one-shot handover is the wrong shape for an inversion
   set.** The bits a level needs swing sixty between adjacent levels
   (first plan §19), because the view collapses and expands with
   every inversion. No single stored f32 position serves both.
4. Smaller: `view_agrees` tests `r` where an all-inversion set sorts
   by `σ·r` (no measured effect); `spent`, `next_cost` and
   `curvature_pixels` are dead since the early exit went; the gates
   run at 96 pixels and 2^28, which is where the last bug hid.

Items 1, 2 and 4 have repairs that fit in a day and are worth making
whatever happens next (§2). Item 3 is the design (§3).

## 2. Repairs to the first design

Cheap, independent of §3, and each one closes a hole the review
opened.

- **R1. Measure the f32 term on the GPU.** `seed_beam_at` forces a
  level; the GPU gate infrastructure
  (`the_gpu_agrees_on_a_nonlinear_set_at_depth`) compares a render
  against `estimate_seeded`. Run it at forced levels on the grand
  julian at 2^20, 2^26, 2^30, 2^33, 2^36, at 1080p-equivalent
  pixels, and tabulate GPU-vs-f64 against the two columns already
  in `probe_where_a_grand_julian_caps`. Then EITHER replace the
  model with the rounding measurement (five more continuations per
  level, from seeds rounded to f32 -- the machinery is in the probe)
  scaled by whatever factor the GPU measurement says the arithmetic
  adds, OR drop the term if the GPU number is under the curvature
  everywhere. Decided by the table, not here.
- **R2. Declare the ceiling.** Below `px / |q| < 2⁻⁴⁰` or so the
  objective's reference is not a reference. Until §3 removes the
  ceiling: past it, hold the level chosen at the deepest zoom where
  the objective was valid -- `ensure_ifs_seeds` re-walks per view,
  so this is a field on the cache -- and say so in the panel's
  extent label. Not a fix; a stop to choosing on noise.
- **R3. Agree on both keys.** `view_agrees` on `r` AND, when the
  key is weighted, on `σ_c·r` with the reach scaled by `σ_c`. Neither
  alone is sound on an inversion set (σ varies across the view too);
  both together are closer, and it measured as a no-op on 42 rows,
  so the cost is nothing.
- **R4. Delete the dead code.** `spent`, `next_cost`,
  `curvature_pixels`, and the comment block that explains a rule
  that is gone.
- **R5. The gates at 1080p.** Every handover gate that runs at 96
  pixels runs at a 1080-pixel `px` too, since the cap bug was
  invisible at 96. The direct reference is still f64, so still
  2^28; §3's gates go past it by a different route (G6).

## 2a. What the repairs found, 2026-09-17

R1 to R5 done. The headline is that **§17a was too hard on the f32
model**, and the reason is instructive.

**R1: the GPU says the model is roughly right.**
`probe_what_the_f32_term_costs_on_the_gpu` renders the reported grand
julian at 1920x1080, reads the walk's own distance back out of the
recolor cache (`IfsRecord.distance`, which `read_results_full`
already exposes), and compares it against `estimate_seeded` in f64
from the same seeds. That difference is the handover's rounding PLUS
the shader's f32 arithmetic, which is the whole of what the term
models.

**Reviewed the same day, and the first version of this section was
wrong in three ways.** The probe compared walks that ended at
different depths: the shader walks the `levels` parameter AFTER the
handover and never subtracts the handover level, and the probe's CPU
continuation subtracted it -- so every deep row was short by exactly
`level` steps, and the 2^4 control (handover 0) could not see it.
Fixed, the grand julian's numbers are unchanged to three decimals,
so the flaw was real and moved nothing; `the_gpu_agrees_on_a_nonlinear_set_at_depth`
had the same convention and is fixed with it. The control was also
too weak for what it was claimed to show -- a level-0 handover checks
the walk, not the seeded path -- so the probe now runs two SEEDED
controls beside the set under test. And the counts below were
counted by eye and miscounted; they are now from a script over the
saved output.

The controls: the affine gasket at 2^28 hands over at level 17 and
the GPU disagrees with f64 by at most 0.027 pixels, model 0.070; the
julia at 2^20 hands over at level 6 with a median disagreement of
0.025 pixels and a 90th percentile of 0.110, model 0.119 -- and a
MAXIMUM of 2.25, which the rounding-only measurement also sees (2.48).
That maximum is not rounding. It is a pixel near a ranking tie whose
f32 position falls on the other side of it and follows a different
lineage to a different, still valid, lower bound. No per-seed model
prices a branch flip, and none should: the max statistic is dominated
by them, so the model is judged against the 90th percentile and the
maximum is reported beside it.

| target, zoom | level | model | rounding only | GPU p90 | GPU max |
|---|---|---|---|---|---|
| 0, 2^26 | 4 | 3.50 px | 0.15 px | 0.42 px | 1.02 px |
| 1, 2^30 | 8 | 0.36 px | 0.09 px | 0.16 px | 0.55 px |
| 2, 2^26 | 3 | 0.24 px | 0.11 px | 0.24 px | 0.72 px |
| 3, 2^30 | 7 | 43.1 px | 0.02 px | 0.41 px | 0.93 px |

Over twenty deep grand-julian rows, against the 90th percentile the
model is within 2.5x in seven, more than 2.5x ABOVE in thirteen
(typically 3.5x to 8x), and below by more than 1.4x nowhere. Against
the maximum it is within 2.5x in fourteen, above in three and below
in three, and the three below are flips of half a pixel. So: §17a's
"3x to 800x above and below nowhere" was against the rounding alone,
which understates by one to two orders because it prices the handover
and not the arithmetic after it; against the GPU the model is
pessimistic by a factor of a few, never optimistic beyond a flip, and
it is a usable proxy. It stays.

**What R1 did not do.** The plan asked for FORCED levels, so that the
model's RANKING of levels -- the only thing an argmin uses -- could
be checked against the GPU's. The renderer builds its own seeds and
has no hook to force one, so this measured the chosen level only.
Whether the model orders levels correctly at a given zoom is still
unmeasured; a test-only field on `EscapeRenderer` that routes
`ensure_ifs_seeds` through `seed_beam_at` is the twenty lines it
needs, and it is open.

**The two rows where it is 100x pessimistic are the same lesson
again.** Target 3 at 2^26 and 2^30 model 2.69 and 43.1 pixels where
the GPU's 90th percentile is 0.017 and 0.41. In both a lineage whose view has
COLLAPSED prices the level, and the answer is a MINIMUM over
lineages, so a seed that never wins should not set the price.

Tried: restrict the term to the seeds whose address answered one of
the five probes. It is WORSE, and by a lot -- eight rows worse, two
better, ten unchanged:

| target, zoom | GPU max, max over all | GPU max, winners only |
|---|---|---|
| 0, 2^30 | 0.010 px | 0.424 px |
| 1, 2^26 | 0.037 px | 2.067 px |
| 2, 2^30 | 0.058 px | 2.562 px |
| 3, 2^20 | 0.272 px | 6.423 px |

Five probes are far too sparse a sample of "who wins": the 1296-pixel
GPU sample finds the dropped seed winning somewhere the probes did
not look. That is retirement (§16) and per-seed levels (§17) for a
third time, and the asymmetry decides it -- over-pricing a seed costs
a suboptimal level, under-pricing costs a wrong picture at pixels
nobody sampled. **The max over ALL seeds is the conservative choice
and it stays.** Reverted.

**R2: the objective's own floor.** Its reference is the level-0
handover continued in f64, so a distance it reports carries a
relative error of `F64_ULP`, which in pixels is `|q|·2.22e-16/px`.
At 1080p on this set that is 0.0065 pixels at 2^36, 0.1 at 2^40 and
**10 at 2^46** -- so §16's 2^46 row measured rounding noise, and past
about 2^40 the walk chose levels on a number that had stopped meaning
anything. The repair is `curv.max(floor)`: a measurement may not
claim an error finer than it can resolve. The floor scales as `1/px`
like both other terms, so resolution-independence survives it, which
is now gated.

**R3: both keys.** `view_agrees` tested `r` where an all-inversion
set sorts by `σ·r`. Now both, when the key is weighted. Still not
sound -- σ varies across the view and this scales the reach by the
centre's σ alone -- and measured a no-op on all 42 rows, so it is
here for agreement with the sort rather than for a bug it fixed.

**R4:** `spent`, `next_cost`, `curvature_pixels` and the
singular-distance evaluation that fed them, all dead since §16
removed the early exit. Gone; the derivation stays as a comment
because it is still the right way to think about where the error
comes from.

**R5:** `the_handover_does_not_depend_on_the_resolution` now runs the
grand julian beside the rabbit at 96, 1080 and 2160 pixels over
fourteen zooms. The rabbit's objective is smooth, so an argmin could
hold there by being flat; an inversion set's swings by orders between
adjacent levels. The magnitude bars (f32 and curvature each under a
pixel) stay on the bounded arm, because on an inversion set they are
false and §16 measured them so -- what is claimed on both is the
INDEPENDENCE, which is what the cap broke.

**Nothing moved.** All 42 rows of `probe_where_a_grand_julian_caps`
choose the same level as before, the 1212 unit tests pass, the
release check passes, and the eleven shipped mode-D presets are
byte-identical to `output/ifs-before3/`. These are repairs to what
the walk KNOWS, not to what it does.

## 3. The idea: the delta at every level

Mandelbrot perturbation stores the reference orbit `Z_n`, carries a
pixel's `δ_n` beside it, computes `δ_{n+1}` from `Z_n` and `δ_n` by
an EXACT algebraic difference (`2Zδ + δ²`, not a Taylor step), and
rebases when `|Z + δ| < |δ|`. Everything the first design did
differently from that is what it got wrong. So:

**The reference is the whole beam, at every level.** `seed_beam`
walks the centre to the full level budget in `BigFloat`, as now, and
does not stop at any handover. Per level it records, per candidate,
what a pixel needs to continue in delta form:

| field | precision | why |
|---|---|---|
| `Z_k` position | f32 | only relative precision matters -- the delta step reads `Z` as a coefficient, as the escape engine's `ref_orbit` does |
| `J_k` inverse Jacobian at `Z_k` | f32 2×2 | the Taylor rung's step, and the rebase test's scale |
| `H_k` inverse Hessian at `Z_k` | f32 3×vec2 | the Taylor rung's second order; absent for an exact-difference kernel |
| `σ_k`, `bound_k` in pixels, escape state, `done`, address | f32 / u32 | what `Seed` carries today, per level instead of once |
| parent index, branch | u32 | the tree; a child's row names its parent's |
| `gap_k` per branch the reference could not take | f32 pixels | today's `dead_min`, per level |

And not only the kept beam: **every child of every kept candidate**,
pruned or not. A pixel ranks children by its own position, and one
level of slack -- the pruned children's rows -- lets it follow a
branch the reference pruned for one level before it has to leave
(§4). Size: `beam × maps` rows per level, `zoom + 64` levels; at
beam 8, eight maps and 2^40 that is 6,656 rows of about 20 words,
half a megabyte in a storage buffer, against the escape engine's
reference orbits of millions of entries. The `fdata` seed block goes
away; the seeds become a buffer in group 1 beside `ifs_maps`.

**The pixel carries δ, and the step is the kernel's difference
form.** A lineage's state is `(row, δ)`: which reference row it
follows and its offset from it, in f32. One level is

```
δ' = D_m(Z_k, δ)          -- the inverse map's difference form
Z' = Z_{k+1}[child row]   -- the reference did this in BigFloat
```

where `D_m(Z, δ) = m⁻¹(Z + δ) − m⁻¹(Z)` computed WITHOUT forming the
difference. This is the rung structure of the first plan's §3, with
the rungs now meaning something different:

| rung | kernels | `D` |
|---|---|---|
| affine | every affine map | `M⁻¹ δ`, exact -- today's basis carry |
| rational | Spherical, Root with integer `\|n\|/d` | an exact rational form: for `v/\|v\|²` it is `(δ\|Z\|² − Z(2Z·δ + \|δ\|²)) / (\|Z\|² \|Z+δ\|²)`; for `v^n` the binomial with the `Z^n` term dropped; for the inverted root both composed |
| algebraic | Bubble, Hemisphere | the square-root difference in conjugate form, `(a − b)/(√a + √b)`, which is exact and cancels nothing |
| Taylor | Disc, Blob, anything transcendental | `J δ + ½ H[δ,δ]` with the error the first plan measured -- and a remainder bound from the third derivative's size, so the lineage knows when to rebase (§4) |

The exact forms are why the curvature term disappears for the sets
that were blocked by it. Their remaining error is f32's relative
rounding of δ, a few ulps of δ itself, whatever `|Z|` is.

**The bound is carried in delta form too.** The answer at 2^40 must
be right to a pixel at level 0, which is `σ_k · (r_k − R)` right to
`px / σ_k` at level k -- the delta's own scale, far below f32's
resolution of `r_k = |Z_k + δ − c|`. So the reference carries
`bound_k` in pixels from its f64 walk, and the pixel adds its own
first-order correction `σ_k · (û · δ) / px` with `û` the unit vector
from the ball's centre to `Z_k`, exact to `O(|δ|²/r)`. The same for
the escape residual. This is not optional: without it the bound's
per-pixel variation is lost exactly where the picture is.

**The rebase.** Two events end a lineage's delta form, and both are
per lineage, not per view:

1. *Expansion*: `|δ| > HANDOVER_FRACTION · R`. The delta is O(1), so
   `Z + δ` in absolute f32 is as accurate as δ was, and the lineage
   continues as today's seeded walk does, from `Z + δ`. This is the
   first design's cap, now applied to one lineage at the level ITS
   delta gets there, rather than to the whole view at the first
   lineage's. A collapsed lineage never reaches it and never pays.
2. *Divergence*: the pixel's ranking of the children at `Z + δ`
   keeps a child the reference pruned, or prunes one it kept. One
   level of slack is stored (§4); past it the lineage continues from
   `Z_parent + δ` in absolute f32 -- the first design's handover, at
   this pixel's own level.

And a Zhuoran-style third: a rational kernel whose reference passes
near its pole has `|Z_k|` small and `|δ|` comparable, and the exact
form is still exact but `Z + δ` is where the precision now is. When
`|Z + δ| < |δ|` re-anchor δ on the child row of whichever sibling's
`Z` is nearest, if one is within `|δ|`; else rebase to absolute.
The first plan's §19 table -- levels needing 80 bits next to levels
needing 16 -- is the trace of exactly this event, and today it is
paid; here it is a row change.

**Where BigFloat sits, and what it needs.** The CPU reference walk
is today's `seed_beam` to full depth with the per-level record kept.
Rung 1 (rational) exists in `BigFloat`; the algebraic rung needs
`sqrt`, which exists; the Taylor rung's kernels need `exp`, `sin`,
`cos` in `BigFloat`, which do not exist and are ~200 lines with
argument reduction and a series, plus gates against f64 in the range
f64 is exact. [ifs-general.md](ifs-general.md) §D2 makes every
kernel generic over the scalar so the `BigFloat` rung is the same
code as the f64 one; until then it is today's
`big_kernel_inverse`, extended per rung.

## 4. Decisions

- **D1. Every child's row is stored, not only the beam's.** One
  level of slack for a pixel whose ranking differs at the margin,
  which is the common case: two children near a tie at the centre,
  ordered the other way at a corner. Without it every such pixel
  leaves the reference at that level, and the first design's
  `view_agrees` measured that as "41% of the frame at one zoom, 0%
  at the next". The slack costs `maps×` the rows and nothing else.
- **D2. The rebase is the first design's handover, per lineage.**
  Nothing new is built for the absolute continuation: it is the
  seeded walk the shader runs today, started from `Z + δ` with the
  σ, bound and address the row carries. A pixel's beam can hold
  delta and absolute lineages side by side, and the answer is the
  minimum over both in pixels, as now.
- **D3. The exact forms are per kernel and gated against BigFloat.**
  `D_m(Z, δ)` is a function beside the inverse in
  [ifs_analysis.rs](../../src/scene/ifs_analysis.rs) (and in
  [ifs-general.md](ifs-general.md)'s shape, beside the inverse in
  the variation), in f64 and in WGSL, and G1 checks it against
  `BigFloat` differences at δ from 1e-30 to 1 relative to `|Z|`.
  The escape engine's rule for Feather -- "the delta of each
  component is its own cancellation-free binomial" -- is the
  standard, and a form that cancels anywhere in that range fails.
- **D4. The Taylor rung declares its remainder.** A kernel without
  an exact form carries `J`, `H`, and a scalar bound on the third
  derivative over the ball (a per-kernel closed form or a sampled
  maximum, stated which), so a lineage can compute `|δ|³·M/6` in
  pixels and rebase to absolute BEFORE the truncation reaches a
  tenth of a pixel. That is the first design's cap done per lineage
  and with a number rather than a measurement; it replaces the
  objective, and it is allowed to be conservative because the
  absolute continuation it rebases to is what the pixel had before.
- **D5. The 3D twin follows, with the same shape and no shader
  change to the 3×3 carry.** Root3/RootZ3 are rational, Quaternion
  is polynomial in four components. Measured to have no demonstrated
  payoff today (first plan §12: no shipped solid zooms deep enough,
  and the quaternion preset goes black from the camera dollying
  inside the object, not from precision), so it is last, and only
  once a solid that needs it exists.
- **D6. The objective is gone.** No level is chosen. What replaces
  the first plan's `the_handover_does_not_depend_on_the_resolution`
  is the same property stated of the rebase: the level a lineage
  rebases at depends on the zoom and the set, not on `px`.

## 5. What changes, and what does not

| piece | today | here |
|---|---|---|
| `seed_beam` | stops at a handover, returns `Seeds` | walks to the budget, returns a per-level `ReferenceBeam` (rows as in §3) |
| `Seed` / `Seeds` / `pack_seeds` / `fdata` block | one level, ten seeds | replaced by a storage buffer in group 1; `SEED_VEC4S`, `MAX_SEEDS` and `ifs_seed()` go |
| `estimate_seeded` | continues from one level in f64 | `estimate_delta`: the CPU reference of the new shader walk, in f64, with the rows from the same `ReferenceBeam` |
| shader `ifs_evaluate` | one seeded start, absolute walk | a lineage is `(row, δ)` or absolute; per level: difference form, bound correction, rebase tests, then the absolute step as now |
| `Kernel` | inverse, Jacobian, Hessian, σ, singular distance, gap | plus `difference(Z, δ)` and, for the Taylor rung, a remainder bound |
| `BigFloat` | add, mul, recip, sqrt, ln, atan2 | plus exp, sin, cos, for the Taylor rung's kernels |
| the affine path | the basis carry, exact | unchanged in result: `D = M⁻¹δ` is the same arithmetic, and the mode-D presets stay pixel-identical (G0) |
| `ensure_ifs_seeds` | keyed on the view | keyed the same; the record is bigger and built once per view as now |

`IfsRecord`, the colourings, the solid twin and the halo do not
change.

## 6. Gates

- **G0. The affine sets do not move.** Every shipped mode-D preset
  pixel-identical against `output/ifs-before3/`, as every change to
  the walk has had to be. The affine difference form is the affine
  basis carry, and this proves it is.
- **G1. The difference forms are exact.** Per kernel with an exact
  form, at random `Z` in the image and δ over thirty orders of
  magnitude relative to `|Z|`, `D(Z, δ)` in f64 against
  `m⁻¹(Z+δ) − m⁻¹(Z)` in `BigFloat` at 64 limbs: relative error
  under 1e-13 of `|D|`, never of `|m⁻¹(Z)|`. Then the same in f32
  against f64: under 1e-6. A form that passes only the first is a
  form with a cancellation the f64 mantissa hides.
- **G2. The delta walk is the direct walk.** `estimate_delta` at
  2^12 to 2^28 (where f64 is a reference) on the julia, the dragon,
  the gasket, the grand julian and a bubble set: agreement with the
  direct f64 walk to 1e-3 pixel at every probe, with NO f32 cost and
  no curvature tolerance -- the first design's gates needed both,
  this walk needs neither, and that is the point of it.
- **G3. Collapsed lineages never rebase.** On the grand julian at
  every zoom in the §16 table: trace which lineages rebase and at
  what level; assert the collapsed ones (reach ratio under 1) reach
  the budget in delta form.
- **G4. The Taylor rung's remainder is honest.** For Disc and Blob:
  the declared third-derivative bound against sampled central
  differences over the ball, and the walk's truncation at the
  rebase level under a tenth of a pixel, measured as G2 measures.
- **G5. GPU equals CPU.** `the_gpu_agrees_on_a_nonlinear_set_at_depth`
  extended to the grand julian and a bubble set at 2^20 and 2^33,
  1080p pixels, against `estimate_delta`: under 1% of pixels off by
  more than one level of the colouring, and the address colouring's
  seams where the CPU says they are.
- **G6. Zoom self-consistency past f64.** No reference exists past
  2^40, so the gate is the picture's own: the centre quarter of a
  render at 2^z, downsampled, against the render at 2^(z−2), for z
  from 32 to 64 on the grand julian and the julia. Agreement to the
  downsampling's tolerance, which the affine sets calibrate since
  they are exact. This is the gate the first design never had and
  the one that would have caught its ceiling.
- **G7. Cost.** `ensure_ifs_seeds` per view at 1080p and beam 8,
  as the first plan's §14 table, with the full-depth walk and the
  record build; and the shader's per-pixel cost against today's at
  2^20 and 2^40. Numbers, then a decision if either is over 2×.

## 7. Cost and risk

The CPU walks to the budget always, where today it stops at the
handover: on the grand julian that was level 7 of 100 at 2^36, so
the `BigFloat` work grows by an order at depth. The first plan's §14
measured the whole prefix at 3 ms there; ten times that is still
under the render. The shader does one difference form and a few
comparisons per lineage per level where it did one absolute step,
and reads a row from a storage buffer per level, which is the same
access pattern as the escape engine's `ref_orbit`.

The risks, ranked:

| risk | consequence | what bounds it |
|---|---|---|
| the exact rational forms have a cancellation nobody spotted | a wrong distance at some δ scale, silent | G1 over thirty orders of δ, against BigFloat, is the whole gate; the escape engine's forms went through the same |
| divergence rebases are common on overlapping sets | many pixels pay the old handover at shallow levels, and the picture is no better than today's there | measured by G3's trace on the D9 overlap fixtures; the answer then is more slack (two levels of children), which is `maps²×` rows, or accepting it |
| the Taylor rung's remainder bound is loose | Disc and Blob rebase early and gain less | D4 says so per kernel; those two are the least-used kernels in the census |
| the delta form of the bound is off by its second order | a sub-pixel bias at the halo's edge | G2 has no tolerance for it |

## 8. Deliberately not here

- **Sets the walk does not accept.** Sums, xaos, non-affine finals,
  non-contractive maps: [ifs-general.md](ifs-general.md).
- **Density.** [ifs-measure-by-inverse-walk.md](ifs-measure-by-inverse-walk.md)
  reads this walk's endpoints; nothing here changes for it.
- **The forced-prefix chaos game** of
  [flame-deep-zoom.md](flame-deep-zoom.md) stage 2. Its prefix IS a
  reference orbit and this plan's difference forms apply to it in
  the FORWARD direction, which that plan's stage 3 said was
  impossible. Noted there; built, if it is, after the measure plan
  shows whether a forward pass is needed at all.

## 9. Order of work, across the three plans

The cheap and the decisive first. Each item names its plan.

1. ~~**R1 to R5**~~ -- done 2026-09-17, §2a, with R1's forced-level
   half still open (the ranking of levels is unmeasured; the chosen
   level is). R1 did not move the cap: the model it was auditing is
   pessimistic by a factor of a few against the GPU and never
   optimistic beyond a branch flip, and the refinement it suggested
   measured worse. The ceiling, both keys, the dead code and the
   widened gate all landed and changed no chosen level.
2. **The measure at shallow zoom**
   ([ifs-measure-by-inverse-walk.md](ifs-measure-by-inverse-walk.md)
   §5 items 1 to 3): the coarse pass, the lookup colouring, the
   beam sum, gated against the direct chaos game at 2^2 to 2^6. It
   uses today's walk unchanged and it is the first picture that
   looks like a flame; if it does not, the rest of that plan is
   moot and this one is unaffected.
3. **The scalar-generic kernel** ([ifs-general.md](ifs-general.md)
   D1 and D2): forward, inverse and difference form written once
   over a `Real` trait; derivatives by dual numbers; the six kernels
   moved into their variations. Before §3 here, because the
   difference forms and the `BigFloat` rung are then one
   implementation each instead of three.
4. **Xaos** ([ifs-general.md](ifs-general.md) D4): small, and it
   unlocks a large share of the census for both the walk and the
   measure.
5. **The delta walk** (here, §3 to §6): G1 on the rational and
   algebraic kernels first, then `estimate_delta`, then the shader.
   G6 is the gate that says whether the grand julian went past
   2^40, and it is the only one that can.
6. **Sums, by Newton** ([ifs-general.md](ifs-general.md) D3): the
   largest reach into the catalogue and the least certain
   mechanism; after the delta walk, because the delta walk's rows
   carry the Jacobians Newton needs on the GPU.
7. **The Monte Carlo measure**
   ([ifs-measure-by-inverse-walk.md](ifs-measure-by-inverse-walk.md)
   §5 item 4) for overlapping sets, once the beam sum's bias is
   measured on them.
8. **`BigFloat` exp, sin, cos**, and the Taylor rung's remainders,
   for Disc, Blob and the non-integer roots.
9. **3D** (D5 here), once a solid exists that a deep zoom would
   show.
