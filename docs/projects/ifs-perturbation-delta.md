# Perturbation in delta form: the second design (plan, 2026-09-17)

The first design is
[ifs-nonlinear-perturbation.md](../archive/projects/ifs-nonlinear-perturbation.md): the
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
design cannot fix by choosing better -- and §2a adds a third, that
its choice of level stops a level or three short half the time,
because a collapsed seed that cannot win prices the deeper levels
out, and no per-seed model has been able to say which seed wins: a lineage whose view has
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

**The forced-level half, done the same day.**
`EscapeRenderer::ifs_force_level` is the test-only field that routes
`ensure_ifs_seeds` through `seed_beam_at`, and
`probe_what_the_f32_term_ranks_on_the_gpu` renders EVERY level the
walk can reach at a target and zoom, comparing the rendered distance
against the exact level-0 continuation at the same absolute depth.
That is the first time the objective's ORDERING -- the only thing an
argmin uses -- has been measured against a rendered picture, and it
is worse than the per-level agreement suggested.

| target, zoom | best level | its error | chosen | chosen's error |
|---|---|---|---|---|
| 0, 2^26 | 4 | 0.367 px | 4 | 0.367 px |
| 0, 2^33 | 8 | 0.482 px | 9 | 2.457 px |
| 1, 2^26 | 7 | 0.161 px | 8 | 1.812 px |
| 1, 2^33 | 10 | 0.156 px | 11 | 0.300 px |
| 2, 2^26 | 3 | 0.322 px | 3 | 0.322 px |
| 2, 2^33 | 7 | 0.181 px | 7 | 0.181 px |

Right in three, and in all three misses **one level too deep**,
costing 1.9x to 11.2x in rendered error.

**Reviewed again the same day, and that table is an artefact of its
metric.** Its error is the 90th percentile of the ABSOLUTE distance
error over a uniform grid of the frame, and most of a frame is far
from the set, where a pixel 500 out read as 499 is the same colour.
Measured again with a second population -- every pixel whose true
distance is under eight, two thousand and more of them per row, the
pixels that ARE the picture -- the ranking is different:

| target, zoom | chosen | near error | best near | its error |
|---|---|---|---|---|
| 0, 2^26 | 4 | 0.283 px | 7 | 0.025 px |
| 0, 2^33 | 9 | 0.035 px | 9 | 0.035 px |
| 1, 2^26 | 8 | 0.021 px | 8 | 0.021 px |
| 1, 2^33 | 11 | 0.033 px | 11 | 0.033 px |
| 2, 2^26 | 3 | 0.192 px | 6 | 0.025 px |
| 2, 2^33 | 7 | 0.116 px | 8 | 0.017 px |

Near the set the objective is right in three and too SHALLOW in
three, by one to three levels -- the opposite direction from the
whole-frame verdict -- and every chosen level is within a third of a
pixel of the truth on the pixels that matter. The two cases the
whole-frame metric called "one level too deep" (targets 0 and 1 at
2^33 and 2^26) are the objective picking the best level there is.
The min-over-seeds model, which the whole-frame metric favoured, is
worse near the set by 24x and 25x in two cases. Withdrawn as a
candidate.

What does survive, and it is the finding from the first review in a
form that finally holds: the f32 model overstates the levels it
rejects. At target 0, 2^26 it prices levels 5 to 8 at 26 to 3e10
pixels, and near the set they render at 0.05, 0.027, 0.025 and 0.059
-- every one better than the chosen level's 0.283. That is why the
objective stops shallow, and it is the max over a collapsed seed that
cannot win, as §2a's per-seed detail showed.

**The objective's own curvature term has the same flaw as the
whole-frame metric.** It is the absolute distance error at five fixed
probes -- the centre and the corners -- wherever the set happens to
be relative to them. A corner 300 pixels from the set contributes an
absolute error that is invisible; a centre one pixel from it
contributes one that is the picture. So the level choice is made on a
far-field-contaminated number, and the near-set weighting that made
this table honest is the obvious repair to try on it: measure the
curvature only at probes whose answer is small, or weight each by
one over its distance. Not tried yet, and it is the one candidate
here with a reason behind it rather than a fit.

**Why R1 kept moving, said plainly.** Four commits measured four
different quantities and each called itself R1: the rounding alone;
the GPU against f64 from the same seeds, which is f32 alone, at the
chosen level, which is a selection-biased sample since the objective
chose the level where its own model was smallest; the GPU against
the level-0 truth over the whole frame, which is dominated by the far
field; and now the same near the set. Only the last is the quantity
the picture cares about. The others were not wrong as numbers; they
were wrong as answers to the question, and each was recorded as if
it were the answer.

**Why, and it is the same non-separability a fourth time.** The level
tables show the f32 model rejecting the best level by twenty orders
of magnitude: at target 1, 2^26 it prices level 7 at 4.6e11 pixels
and the render is 0.161 out, the best of any level. The per-seed
detail (`probe_which_seed_prices_the_level`) says why. At that level
the beam holds three seeds whose view expansion runs 3.99e3, 1.26e-1
and 1.07e-8, and whose f32 costs therefore run 1.2, 3.9e4 and 4.6e11
pixels. The model takes the MAXIMUM. The answer is a MINIMUM.

Two structural facts came out of the same detail, and both are worth
keeping:

- `σ_per_px · grown` is constant across the beam to three digits
  (2.47e10, 2.38e10, 2.44e10). So a seed's f32 cost is proportional
  to its own σ, and its bound is `σ·(r−R)` -- the error is RELATIVE
  to the scale that seed answers on. A seed sitting 1e18 pixels away
  has a huge absolute error that cannot corrupt an answer of 0.16.
- The carried bounds at a handover are **negative and equal** across
  the beam (−1.36e7 pixels at that level), inherited from a common
  ancestor. So no test on the carried bound can tell which seed will
  win: the bound has not bitten yet, and the answer comes from the
  continuation.

**Four candidate repairs, all measured, none shipped.**

| f32 term | cost against the best level, whole frame | near the set |
|---|---|---|
| max over seeds (shipped) | 1.00, 5.10, 11.22, 1.92, 1.00, 1.00 | 11.5, 1.00, 1.00, 1.00, 7.6, 6.8 |
| min over seeds | 1.65, 2.07, 4.48, 1.92, 1.00, 1.00 | 2.0, 24.0, 25.3, 1.00, 7.6, 6.8 |
| restricted to probe winners | measured worse at the chosen level, above | |
| gap-weighted by the bound | identical to the max: the bounds are equal | |

The whole-frame column is the artefact explained above; the near
column is the one that counts. Nothing here beats the shipped
maximum near the set, and nothing is shipped. The candidate with a
reason behind it -- near-weighting the objective's own curvature
probes -- is untried. §3 is still the answer: the delta form has no
level to choose.

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

## 3a. The difference forms are built and exact, 2026-09-18

Item 5's first step, which §9 puts before `estimate_delta`: G1 on the
rational and algebraic kernels.

`kernel_difference_gen` in
[ifs_analysis.rs](../../src/scene/ifs_analysis.rs) computes
`m⁻¹(Z + δ) − m⁻¹(Z)` **without forming the difference**, over the
`Real` trait D2 built, so one body serves f64, f32 and `BigFloat`.
Every form has each term `O(δ)`:

| kernel | form |
|---|---|
| Root `(a, b)` | `Z^a·Q + P·conj(Z)^b + P·Q` |
| Spherical | `(δ\|Z\|² − Z·t) / (\|Z\|²·\|W\|²)` |
| Hemisphere | `(Z·t/(√A + √B) + δ√A) / (√A·√B)` |
| Bubble, inner | `2(Z·t/(r + r') + δ(1 + r)) / ((1 + r)(1 + r'))` |
| Bubble, outer | `2(−Z·x·t/(r + r') + δ·x·(1 + r') − Z(1 + r)·t) / (x·x')` |

with `W = Z + δ` and `t = 2Z·δ + |δ|²` the norm's own
cancellation-free difference. `P` and `Q` are the two power
differences, each from `w^n − z^n = δ·Σ_{j<n} w^j z^{n−1−j}`. Every
square-root difference goes through `(a − b)/(√a + √b)`.

**A root is `v^a·conj(v)^b` when it is a polynomial at all.**
`m⁻¹(v) = |v|^{|n|/d}·e^{i·n·arg v}` is `ρ^{a+b}e^{i(a−b)θ}`, so the
exponents are `a = (|n|/d + n)/2` and `b = (|n|/d − n)/2`, and the
form exists exactly when both are whole and non-negative. `julia`
and any `julian` at `dist` 1 are `(n, 0)`; a negative power at
`dist` 1 is `(0, |n|)`, the conjugate power; `dist = 1/k` works too.
`dist` 2 does not, and falls to the Taylor rung with the disc and
the blob.

**G1, measured.** Against the same inverse taken at 512 bits and
subtracted THERE, at δ from 1 to 1e-30 relative to `|Z|`, over 8,815
points of nine kernel-and-branch fixtures: **worst relative error
1.4e-14**, relative to `|D|` and not to `|m⁻¹(Z)|`. That distinction
is the whole gate, and the companion measurement is what shows it:

| \|δ\|/\|Z\| | direct subtraction | exact form |
|---|---|---|
| 1 | 2.1e-16 | 0 |
| 1e-8 | 8.6e-9 | 1.5e-16 |
| 1e-12 | 2.2e-5 | 4.0e-17 |
| 1e-16 | **1.0** | 4.9e-17 |
| 1e-24 | **1.0** | 7.2e-17 |

A relative error of 1.0 is not a poor answer; it is no correct digits
at all. That is where today's walk is at a deep zoom, and it is
asserted rather than assumed, so if the direct form ever became
accurate there the exact forms would be shown to be dead weight.

**The f32 half, split by the rebase criterion.** In f32 against its
own f64 value the forms hold to **4.2e-7** across 2,457 points --
and to 1.7e-5 across the 98 where `|Z + δ| < |δ|`. That second
number is not a flaw in the form: there `Z` and `δ` nearly cancel,
so `Z + δ` has lost digits before any kernel touches it, and an
outer bubble squares what is left. It is the measured justification
for §3's third rebase criterion, and the gate asserts the two sides
differ by at least tenfold, so the split stays a measurement rather
than a story.

**The composition is gated too.** `Map2::difference` wraps the
kernel's form in the two affines and the weight, and a slip there --
the wrong matrix, the weight on the wrong side -- shows in none of
the kernel gates. It is bracketed from both ends:
`the_composed_difference_is_the_maps_own` checks it against the
DIRECT subtraction at a δ of 1e-3, where f64 still has eleven good
digits, and against `J·δ` at 1e-9, where the answer must approach the
dual-number Jacobian to first order. Worst across six maps: 5.1e-12
far, 2.8e-6 near. **For the affine the near figure is exactly zero**
-- the difference IS `M⁻¹δ` and the Jacobian IS `M⁻¹`, the same
arithmetic -- which is G0's argument in miniature.

**`BigFloat` now implements `Real`.** Not `Transcendental` -- `exp`,
`sin` and `cos` are still item 8 -- so what it can run is exactly
the rational and algebraic rungs, which is exactly what G1 needed.
`kernel_inverse_real` is the inverse over `Real` alone: spherical,
hemisphere, bubble and a whole-power root, in one body, bit-identical
to the transcendental one on the first three and within 1.3e-15 on
the roots. That is the beginning of `big_kernel_inverse`'s
replacement, though it has not been swapped in yet.

## 3b. The delta walk runs on the CPU, 2026-09-18

Item 5's second step. `reference_beam` walks the centre to the level
budget and keeps its beam at EVERY level; `estimate_delta` is a
pixel's own walk against it, carrying `(row, δ)` instead of a
position. Both in
[ifs_estimate.rs](../../src/scene/ifs_estimate.rs). The shipped path
is untouched: `seed_beam` and `estimate_seeded` are exactly as they
were, so nothing rendered can have moved.

**G2, and the two bugs it caught.** Worst disagreement with the
direct f64 walk, over sixty-four pixels at four zooms:

| set | 2^12 | 2^18 | 2^24 | 2^28 |
|---|---|---|---|---|
| dragon | 0 | 0 | 0 | 0 |
| bubble set | 0 | 0 | 0 | 0 |
| gasket | 2.4e-12 | 1.5e-10 | 1.1e-8 | 1.6e-7 |
| julia | 1.5e-11 | 6.6e-10 | 9.0e-8 | 1.2e-6 |

Pixels, against a bar of a thousandth of one. The dragon and the
bubble set are EXACT -- the same bits, not a tolerance. The other
two grow about tenfold every six zoom levels and end six orders
below the bar. Which of the two walks that residue belongs to is NOT
established: the direct walk's own one-ulp jitter was measured and is
zero everywhere here, so it is not the reference moving, and the
remaining candidates are the delta form's f64 rounding accumulating
and the `excess2` column's own 1e-16. Both are below anything that
matters at these zooms, and the shader step is where the question
becomes worth answering.

Neither bug was in the difference forms, and neither was visible in
an aggregate:

- **σ came from the reference row instead of the pixel.** `σ_min`
  varies across the view like everything else, by `O(|δ|/s)` a
  level, and sixty levels of a tenth of a percent compound to six --
  eight pixels on a julia at 2^12. Evaluating it at `Z + δ` costs
  nothing, because σ is a smooth `O(1)` factor and so needs the
  position only to RELATIVE precision, which the sum has at any
  depth.
- **One level too many in the budget.** The direct walk scores its
  live set and then expands, so its last expansion is never scored;
  scoring it here found a radius crossing the ball at level sixty
  that the reference had already stopped looking for. A tenth of a
  pixel at 2^18, and nothing at all at the other three zooms.

A third thing the gate had to learn: on a julia the direct f64 walk
is not automatically a reference. Sixty levels of a squaring map
amplify its own last digit, so the bar is `max(1e-3, 4×)` the
direct walk's answer moved one ULP of the pixel's position. Measured,
that jitter is ZERO on every fixture here, so the bar is the
thousandth everywhere and the clause costs nothing -- but it is what
the gate would fall back on rather than loosening a fixed tolerance.

**The pixel's own radius is exact in δ, not linearised in it.** §3
proposed `r + û·δ`, which is wrong by `O(|δ|²/r)` -- near the rebase
cap, a fraction of the ball. The row carries `u = Z − c` and
`excess2 = |Z − c|² − R²` instead, and

```text
r² = excess2 + R² + t,   t = 2u·δ + |δ|²
r − R = (excess2 + t)/(r + R)
```

has nothing to cancel at any δ. The limit left is `excess2`
computed in f64, whose absolute error of 1e-16 swallows `t` at
around 2^50; past there the reference has to compute that column in
its own precision, which it has and f64 does not.

**D6, and it is a stronger statement than the first design could
make.** The old gate checked that an objective picked the same level
at two pixel sizes. There is no objective now: a lineage rebases when
its own δ reaches a fraction of the ball or when `Z + δ` cancels, and
neither quantity contains `px`. So four times the resolution moves
not one rebase level, which the gate asserts as EQUALITY. What they
do depend on is the zoom:

| median rebase level | 2^12 | 2^20 | 2^28 |
|---|---|---|---|
| dragon | 16 | 31 | 48 |
| julia | 18 | 28 | 34 |
| gasket | 10 | 18 | 26 |

Two zoom levels buy roughly one more level of delta carry on the
dragon. That is the whole claim of this plan in one table: the first
design picked ONE level for the whole view, and this picks each
lineage's own, deeper every time the view shrinks.

**Still open in item 5**: the Taylor rung (D4), so the disc and the
blob rebase immediately and walk exactly as they do today; the
shader, which is the next step and where the f32 arithmetic finally
gets measured; and G3, G5, G6 and G7, which are all shader gates.

## 3c. The difference forms reach the shader, 2026-09-18

Item 5's third step, and the last one before the shader's own walk.
`IFS_DIFFERENCE` in [ifs.rs](../../src/escape/ifs.rs) is
`kernel_difference_gen` and `Map2::difference` in WGSL, its own const
for the same reason `IFS_JACOBIAN` is: it depends on nothing but the
map rows, so a gate can compile it against a twenty-line harness and
check the arithmetic instead of standing up the whole walk to reach
it.

`the_shader_differences_are_the_cpu_ones` runs one flame per kernel
so every arm is reached, at four hundred points each, against the
CPU's own answer:

| | affine | root n=3 | root n=-2 | spherical | bubble | hemisphere |
|---|---|---|---|---|---|---|
| worst relative | 4.5e-8 | 2.5e-7 | 2.0e-7 | 4.6e-7 | 3.0e-6 | 9.7e-6 |

f32's own precision is 6e-8, so the spread across the six is the
conditioning of each form and not a transcription difference.

**The exponent pair is a ROW FIELD, not a test the shader does.** A
root's inverse is `v^a·conj(v)^b` when `a = (|n|/d + n)/2` and
`b = (|n|/d − n)/2` are whole, and that is a tolerance. The shader's
f32 and `root_powers`' f64 would not always agree about a borderline
`dist`, and a disagreement there is not a rounding: one side has an
exact form and the other walks a different rung. So `IfsMapGpu` grew
a `delta` vec4 carrying `(a, b, has_form, _)` computed once on the
CPU -- 96 bytes to 112, and the layout gate says so.

**δ in that gate is a thousandth of the ball, not a millionth.** The
comparison is f32 against f64, so at a δ small enough to be
interesting the f32 answer is dominated by its own rounding and says
nothing about the transcription. What it checks is that the two
expressions ARE the same expression; the conditioning at small δ is
`the_difference_forms_survive_f32`, measured in Rust where the
arithmetic is the same on both sides.

**No gaps are stored on a reference row**, and §3's table listed
them. A gap is a branch whose image the point is outside of, and the
one-shot handover had to carry the reference's because the
continuation could not see the levels the prefix walked. Here every
level is in hand, so a pixel asks `image_gap` at its own `Z + δ` --
what the shipped walk already does, exact rather than the
reference's value less `|δ|`, and one fewer column in the row the
shader will read. Dropping them changed no number in G2.

## 3d. The delta walk runs on the GPU, 2026-09-18

Item 5's fourth step. `IFS_DELTA_WALK` is `estimate_delta` in WGSL,
spliced as a WHOLE ALTERNATIVE `ifs_evaluate` rather than a branch
inside one, so the shipped shader's text is what it has always been
and the presets cannot move by this existing -- asserted on the
source, not on a render. A `delta` formula parameter turns it on, and
it joins the pipeline key beside the beam width, which was already
compiled in.

**Measured against `estimate_delta` in f64, per pixel.** The relative
figure above a pixel and the absolute one below it, since a distance
of thirty thousand pixels is background and an absolute comparison
there measures f32's mantissa rather than the walk:

| set | 2^8 | 2^16 | 2^24 | the shipped walk at 2^24 |
|---|---|---|---|---|
| dragon | 0 | 0 | 0 | 0 |
| bubble set | 0 (median) | 0 | 0 | 0 |
| gasket | 2.8e-5 | 1.8e-5 | **2.7e-5** | **1.6e-3** |
| julia dust | 1.0e-2 | 9.1e-3 | 2.0e-4 | identical |

The gasket is the row the plan is about: **sixty times closer than
the walk it replaces**, and not drifting with the zoom where the
shipped walk's error grows seventeenfold from 2^8 to 2^24.

**Four bugs, and each one was found by a number rather than by
reading.**

- **The level-0 basis came from a seed.** A seed's basis is the view
  composed with the prefix at the HANDOVER level, which is what the
  seeded walk wants and is not the view. The delta walk read 1,259
  pixels out on a gasket at 2^8. The view basis lives in the
  reference's header row now, where nothing else can be mistaken for
  it.
- **`estimate_delta` drained its beam and then broke.** When every
  branch of every lineage is a gap the loop ends, and a drained
  `live` is empty at that point -- the walk had thrown away the beam
  whose bounds are the answer. It takes the beam now, as the direct
  walk does.
- **`meta` is a reserved keyword in WGSL.** The row's field is `link`
  on both sides.
- **A set with no exact form must be DECLINED, not walked.** This is
  the one worth stating properly.

### A root of negative distance has no form, and rebasing it is worse
### than not trying

`m⁻¹(v) = |v|^{|n|/d}` with `d < 0` has a negative exponent, so
`root_powers` finds no whole pair and there is no polynomial
difference. A lineage on such a map therefore rebases at LEVEL 0 --
and rebasing at level 0 throws away the `BigFloat` prefix the seeded
walk keeps. Measured on a julia dust at 2^24: the delta walk read
0.5% from the f64 reference where the seeded walk read 0.02%.
**Twenty-five times worse**, and not a bug in the walk: it correctly
declined a kernel it has no form for, and declining at level 0 is the
expensive way to do it.

So `Ifs2::has_delta_forms` gates the whole path, and the renderer
asks it in both places -- the seeds and the pipeline -- so the buffer
and the shader cannot disagree about which walk is running. A
declined set is the shipped walk EXACTLY, which the gate asserts as
equality rather than closeness.

That also settles what the Taylor rung (D4) is for. It is not a
refinement: without it, every set with a disc, a blob or a
fractional root is outside this plan entirely.

**One outlier the gate is deliberately not about.** On the bubble set
at 2^8 one pixel of 144 has both shaders reading 178.8 where the f64
walk reads 3.1 -- a branch the three do not agree about at a corner
-- and the two shaders agree there to the bit. The absolute bar is
therefore on the MEDIAN, and the comparative clause, which is the one
with teeth, is on the worst.

### G6: the picture holds to 2^64, and the fixture took two tries

The gate the first design never had. No reference exists past 2^40,
so the reference is the PICTURE: a render at `2^(z+2)` shows the
centre quarter of the render at `2^z`, so the deeper one downsampled
by four must reproduce the shallower one's middle, in pixels, with
the deeper distances divided by four.

| median drift, px | 2^16 | 2^28 | 2^40 | 2^52 | 2^64 |
|---|---|---|---|---|---|
| shipped | 0 | 0 | 0.0007 | 0 | 0.0028 |
| delta | 0 | 0 | **0** | 0 | 0.0028 |

Both hold to 2^64 on this set, and at 2^40 the delta walk is exact
where the shipped walk has begun to drift.

**The first fixture measured nothing, and it is worth saying why.**
The obvious centre is `(5/14, 1/7)`, the fixed point of `S₀∘S₁∘S₂`.
That cycle scales by `(1/2)³` about it, so the walk's own state
REPEATS every three zoom levels: both walks reproduced the picture at
2^60 exactly, having drawn it at 2^18, and every row read 0.0000.
That is the same trap `tetra_cycle_target` warns about in another
guise -- a dyadic centre is not the only kind a walk can hide behind.
The centre is now an APERIODIC address of the gasket, three hundred
maps long, built exactly: the point of an address is `Σ tₐᵢ·2⁻ⁱ` and
every `t` is a quarter or a half, so `4x·2^k` is a whole number and
the decimal expansion is finite.

**What it does not cover**: the set is AFFINE, whose basis carry is
exact either way, so this measures the f32 arithmetic and the
handover rather than the difference forms. A curved set with exact
forms would be the stronger test and needs a deep centre on a bubble
attractor, which has no closed form the way a gasket address does.

**Still open in item 5**: the Taylor rung (D4), G3's lineage trace,
and G7's cost. The walk is off by default until those land.

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

1. ~~**R1 to R5**~~ -- done 2026-09-17, §2a, forced levels included
   and reviewed twice. Near the set the objective is within a third
   of a pixel everywhere tested, right in three of six cases and one
   to three levels too SHALLOW in the rest, because the f32 model
   overstates the levels it rejects. Four candidate repairs
   measured, none better near the set, none shipped. One untried
   with a reason: near-weighting the objective's curvature probes.
   The ceiling, both keys, the dead code and the widened gate all
   landed and changed no chosen level.
2. ~~**The measure**~~ -- done 2026-09-17, and it went well past
   "at shallow zoom". The factorisation holds against a chaos game
   on five fixtures covering every kernel class; the colouring
   ships, reads a coarse pass built by the flame renderer, and
   carries the handover's prefix so it zooms as far as the distance
   walk does. Open there: the brightness normalisation, the Monte
   Carlo variant for overlapping sets, and a curved set's 1.296 at a
   deep handover -- which is §3 HERE, since a handover hands over a
   linearisation and the quadratic is what straightens it.
3. ~~**The scalar-generic kernel**~~ -- done 2026-09-17,
   [ifs-general.md](ifs-general.md) §9a to §9c. `Real`,
   `Transcendental` and `Dual` landed; the six planar kernels are
   one body each, bit-identical to the f64 bodies they replaced;
   the Jacobian and the Hessian are differentiated rather than
   derived, and the central difference the Hessian used had NO
   correct digits within 1e-8 of a pole, which is where the handover
   lives. All twenty-two inverses -- twelve affine roles, seven
   planar kernels, three solid -- are registered beside their own
   forward WGSL, and G1 runs over that list. Open: `BigFloat`
   implements neither trait, which is item 8 below, so the
   difference forms of §3 here have one implementation for the
   algebraic kernels and will need the second after item 8.
4. ~~**Xaos**~~ -- done 2026-09-18, [ifs-general.md](ifs-general.md)
   §9d. The walk expands admissible children only, on the CPU and in
   both shaders, and the measure became the Markov chain it always
   was under a graph. A uniform xaos is bit-identical to none; a
   four-cycle rejects 1994 of 2000 free-chaos points and moves 4096
   of 9216 pixels, with CPU and GPU agreeing at every sample point.
   A colour-speed leftover in the shader's measure fold was found and
   repaired on the way.
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
