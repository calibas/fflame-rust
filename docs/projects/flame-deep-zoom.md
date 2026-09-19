# Deep Zoom for Flames — importance-sampled chaos game

**Status:** the destination of the `ifs-distance` branch as of
2026-09-18 ([ifs-perturbation-delta.md](ifs-perturbation-delta.md)
§10). No stage has code yet, but stages 2 and 3 have most of their
infrastructure built under the inverse-walk plans -- §10 below says
what exists per stage and where. Stage 1 is next. Sibling of
[escape-time-fractals.md](escape-time-fractals.md): that plan is
per-pixel fragment rendering; this one is a sampling upgrade to the
existing chaos game. They share two pieces of infrastructure (§7),
and the inverse-walk plans share a good deal more (§10).

**Origin:** a discovered trick — slaving the weight of a
point-contracting transform to the zoom level redirects iterations
toward where the camera is looking, at the cost of visibly changing
the rendered structure. This plan is the principled version: keep the
redirection, delete the corruption.

---

## 1. The two walls, and their order

Zooming a flame hits two independent limits:

1. **Sample starvation.** The chaos game samples the invariant
   measure over the whole attractor; the fraction of samples landing
   in the viewport falls off polynomially with zoom. This is severe
   by ~100× zoom — the image starves long before anything numerical
   breaks.
2. **f32 splat precision.** Orbit coordinates are O(1); at ~10⁵–10⁶×
   zoom the structure inside the viewport lives below f32 ulp and
   the splats quantize. Unlike escape-time fractals there is no
   reference orbit to perturb around — chaos-game orbits are chaotic,
   nothing smooth to linearize against.

Starvation binds three to four decades before precision does. That
ordering is the whole shape of the plan: the sampling half (stages
0–2) ships value with zero precision work; the precision half (stage
3) is honest about being partial.

## 2. What the discovered trick is, formally

Boosting a sink transform's weight from p to q is **importance
sampling with the correction term dropped**. Two facts make the
repair precise:

- The attractor's *support* does not depend on the weights (any
  strictly positive weights, same point set). What changes is the
  *density* on it — and in a flame the density is the image. Hence
  "it works and changes the picture."
- The unbiased estimator deposits, instead of 1, the likelihood
  ratio of the orbit's recent transform choices:
  `w = ∏ p(choice)/q(choice)`. With that weight in the histogram the
  rendered measure is exactly the true one, however aggressive the
  bias.

**The window subtlety** (important — the naive version is wrong): the
product over the orbit's *entire* history has variance that grows
without bound. It is also unnecessary. Contraction means the last m
choices determine the point's position to sub-pixel precision; older
choices only select position *within* sub-pixel. Unrolling the
invariant-measure fixed point m levels shows a product over the last
m choices is correct to below pixel resolution, provided

```
m ≥ log(pixel_size / attractor_size) / log(λ_max)
```

with λ_max the flame's largest per-transform Lipschitz constant —
typically ~20–40 at deep zoom. Windowing caps the variance by
construction, which is why resampling (§ stage 4) is a contingency,
not a stage.

## 3. Why this engine specifically is close to it already

- **The deposit mechanism exists.** Every splat already carries a
  per-sample weight: `density_weight` in
  `shaders/core/main_template.wgsl` (seeded from multi-emit
  `src_weight`, multiplied by depth-density compensation, far-fade,
  solid occlusion, then applied to all four histogram channels as
  `weighted_scale`). Color recovery is Σcolor/Σdensity, so a common
  factor on all channels is color-invariant — the importance weight
  is one more multiplication into a variable that already does this.
- **Selection is centralized.** `select_transform_const` /
  `select_transform_xaos` in `shaders/core/utilities.wgsl` are the
  only places a transform is chosen. Biasing = uploading a second
  weight table; correcting = reading a ratio from a second table.
- **Xaos is already a matrix.** Under xaos the true selection
  probability is row-conditional, so the correction ratio is a
  matrix r[prev][i] = p(prev→i)/q(prev→i) — the same
  NUM_TRANSFORMS² layout as `xaos_weights`, computed CPU-side with
  the row normalizers folded in.

## 4. Stages

### Stage 0 — the trick as a script (ships any time, no engine work)

A Rhai script slaving transform weights to zoom reproduces the
discovered trick as-is: biased, uncorrected, structure-changing —
useful as an aesthetic study and as the demand signal for the rest.
Candidate embedded example script. Nothing below depends on it.

### Stage 1 — biased selection + windowed correction (the core) — BUILT 2026-09-18, §11

Engine-side mechanism, deliberately policy-free — it takes an
arbitrary biased table q and makes it unbiased; *how* q is chosen is
UI/script policy layered on top (open question 1).

- CPU: compute q (v1 policy: per-transform bias factors, optionally
  zoom-slaved), the ratio matrix r[prev][i] including row
  normalizers, and upload both. q must respect the xaos sparsity
  pattern: never positive where xaos forbids, never zero where it
  allows — violating either genuinely changes the support.
- Kernel: selection reads q; a per-thread `var w` accumulates
  `w *= r[prev][chosen]` per iteration; deposits multiply `w` into
  `density_weight`.
- **Windowing, v1 form — epoch reset with warm-up gate:** every 2m
  iterations reset w ← 1; deposit only when the window length since
  reset is ≥ m. Costs one f32 + a counter per thread and halves the
  deposit rate — acceptable because at depth the *biased* deposits
  land in-viewport ~always vs ~never unbiased. Exact ring-buffer
  window only if epoch artifacts ever show. m comes from λ_max (§7).
- Bad-value respawn and fuse: priming samples already skip the
  histogram; a respawn resets the epoch (w ← 1, warm-up again).
- **Quantization floor:** histogram adds are u32 at fixed
  `color_scale = 100`; weights below ~0.01 round toward zero — bias
  of exactly the kind the correction removes. v1: clamp the ratio
  band so the window product stays within [1/16, 16] (bounded,
  well above the floor). Unbiased alternative if the clamp ever
  matters: Russian-roulette stochastic deposit on the fractional
  part (one RNG call; the kernel has RNG in hand).
- **Gating:** a shader-builder template flag (SOLID precedent) —
  WGSL byte-identical when off; q ≡ p makes it a measured no-op
  when on.

### Stage 2 — cylinder targeting (the free lunch) — enumeration built 2026-09-18, §12

The viewport ∩ attractor corresponds to a set of *symbol prefixes*:
sequences (i₁…i_k) whose composed map has an image touching the
viewport. Compositions contract geometrically, so a CPU
branch-and-bound over composed bounds enumerates them cheaply.
Sampling becomes: random suffix for burn-in (true weights), then the
*forced* prefix — every sample lands in the viewport, deposited with
the prefix's true probability ∏p as its weight. This is the windowed
estimator with the window chosen deterministically instead of
sampled, and it is where the asymptotics change: cost per useful
sample grows *logarithmically* with zoom (prefix length) instead of
polynomially (starvation).

- Prefixes must be xaos-admissible paths, with probabilities from
  the row-normalized chain.
- Bounds: exact ellipse images for affine-only transforms;
  conservative per-variation Lipschitz constants otherwise — the
  same analysis §7 names. Loose bounds cost efficiency (some forced
  samples miss the viewport, weight still correct), never
  correctness.
- Forced-prefix choice is itself randomized over the admissible set
  proportional to cylinder measure, so coverage inside the viewport
  is unbiased.

### Stage 3 — precision (partial, honest)

- The conjugation trick M∘Tᵢ∘M⁻¹ stays inside the flame vocabulary
  **only for affine-only flames** — conjugating a variation produces
  a function we can't express. For affine or affine-heavy flames:
  compose the camera zoom with the (contracting) forced prefix at
  f64/extended precision on the CPU into one well-conditioned O(1)
  map, and run the conjugated system. Density correction is the
  cylinder measure — no Jacobian estimation needed.
- With nonlinear variations in the prefix there is no exact
  recentering; the realistic ceiling stays ~10⁵–10⁶× set by f32.
  Stated as a limit, not solved. (No reference-orbit analogue
  exists; perturbation theory does not apply to chaotic orbits.)
  **Corrected 2026-09-17:** that is true of the unforced game and
  false of a forced prefix. A forced prefix is deterministic, its
  image of the ball's centre is a reference orbit, and the
  difference forms of
  [ifs-perturbation-delta.md](ifs-perturbation-delta.md) §3 carry a
  sample's offset through it in the forward direction. Whether a
  forward pass is needed at all is decided by
  [ifs-measure-by-inverse-walk.md](ifs-measure-by-inverse-walk.md)
  D8, which reads the measure through the inverse walk without one.

### Stage 4 — resampling (contingency only)

If windowed weights still show variance in real renders: a particle
SIR pass — clone high-weight / kill low-weight orbits in a
compaction dispatch. Real machinery; build only on demonstrated
need. Windowing (stage 1) is expected to make this moot.

## 5. Config, contract, UI

- New skip-if-default fields on `FractalConfig` (bias policy,
  per-transform bias factors or auto mode, enable flag) → new
  ConfigPaths. **Key-path additions move the engine contract's
  shape** — coordinate with the API repo like any such change.
- Not written to `.flame` XML (no Apo/JWF equivalent) — same policy
  as depth-density compensation.
- UI v1: a small section (View panel or Performance panel — open
  question) with the enable toggle and bias strength; scripting gets
  it free via ConfigPaths.

## 6. Testing

- **Off = byte-identical WGSL** (template-flag test, SOLID
  precedent).
- **q ≡ p on = bit-identical render** to off (ratio table all 1s,
  weight path exercised but neutral).
- **Unbiasedness**: same flame at moderate zoom, biased vs unbiased,
  equal total iterations budget — tolerance compare (solid-* style;
  the estimators agree in expectation, not per-pixel).
- **Xaos sparsity property test**: q generation never creates or
  destroys admissible edges.
- **Quantization**: unit test that the clamp band keeps window
  products above the u32 floor.

## 7. Shared infrastructure with the escape-time plan

Named in both plans so neither builds it privately:

1. **The Lipschitz / largest-singular-value extension of the
   contractiveness machinery.** The existing metric
   (`flame.contractiveness()`, `mean_log_scale` in
   `src/script/api.rs`) is log-scale/determinant-flavored. Escape-time
   mode C needs the singular-value form to gate its pass count; this
   plan needs it three times — the window size m (stage 1), the
   branch-and-bound bounds (stage 2), and the conjugation
   conditioning (stage 3). Build once, shared.
2. **The deep camera representation** (decimal-string center +
   `zoom_log2: f64`) — designed in the escape-time plan §3; a flame
   deep-zoom camera hits the identical f32-center wall and should
   reuse the type rather than invent a second one.
3. **Address-tree composition** ("compose maps down the symbol tree
   with conservative bounds") is the same skeleton as mode C's
   analysis pass — shared shape, not necessarily shared code.

## 8. Bloat ledger

Zero new dependencies. Genuinely new: one template flag + the weight
window in the kernel (a few lines), two small GPU tables (biased
weights, ratio matrix), the bias-policy CPU code, the stage-2
branch-and-bound module, config fields + a small UI section. Reused
wholesale: the per-sample weight channel, selection functions, xaos
buffer layout, histogram/color recovery (weight-invariant by
construction), visual harness.

## 9. Open questions

1. **Bias policy**: manual per-transform sliders vs zoom-slaved auto
   vs cylinder-derived (stage 2 subsumes the policy question — the
   admissible-prefix measure *is* the right bias). Mechanism/policy
   split keeps this open without blocking stage 1.
2. **UI home** for the toggle (View vs Performance panel).
3. **Governor interplay**: biased frames concentrate atomics on few
   pixels — contention may shift frame timing; watch, don't
   pre-engineer.
4. **Multi-emit variations**: emissions already seed `src_weight`;
   confirm the importance weight composes multiplicatively (expected
   yes — same channel).
5. Does stage 0 ship as an embedded example script in 0.5.x, ahead
   of any engine work?

## 10. What exists now, per stage, 2026-09-18

Written when the inverse-walk work was reviewed against this plan
([ifs-perturbation-delta.md](ifs-perturbation-delta.md) §10). The
review's structural point, which decides the order below: a forced
prefix and the inverse walk are the same tree walked from opposite
ends, and the forward direction is the easy one -- the maps
CONTRACT, so a sample's offset only shrinks, and there is one lineage
with no ranking, no rebase and no remainder. Every hard part of the
inverse delta walk is an artefact of walking the expanding direction.

### Stage 1 -- built, 2026-09-18 (§11)

The hooks are where §3 said: `select_transform_const` /
`select_transform_xaos` in `shaders/core/utilities.wgsl` are the only
places a transform is chosen, and `density_weight` in
`shaders/core/main_template.wgsl` is the per-sample channel every
deposit already multiplies. The xaos buffer layout is the ratio
matrix's layout. None of the inverse-walk machinery is involved, and
this stage is first for exactly that reason: it is the demand signal
and it is the cheapest item on the branch.

### Stage 2 -- the enumeration is written; the sampler is not

- **The tree with bounds.** `reference_beam` in
  `src/scene/ifs_estimate.rs` walks the view centre's inverse orbit
  in `BigFloat`, keeps EVERY child of every kept row at every level,
  and carries per row the composed basis, `σ·(r − R)` in pixels, the
  parent index and the map. A row with a non-positive bound is a
  prefix whose image touches the viewport. That is the
  branch-and-bound, and it prunes exactly by the composed-image test
  §4 asked for.
- **The prefix probabilities under xaos.** `XaosGraph` in
  `src/scene/ifs_analysis.rs` and `MeasureMaps::step_probability` in
  `ifs_estimate.rs` are the row-normalised chain with its stationary
  distribution; the measure walk composes `∏p` along an address
  already, and a root's probability is split across its forward
  branches there (measure plan §5d), which a forced prefix through a
  root needs too.
- **The weight's units.** The measure walk's coarse pass and its
  `density` are what a forced sample's `∏p` deposit is measured
  against, so the unbiasedness gate of §6 has a reference.
- **Not written:** the sampler itself -- a burn-in at true weights,
  then the forced prefix applied FORWARD on the GPU, deposited with
  `∏p` -- and the CPU side that chooses prefixes proportional to
  cylinder measure. Also not written: the fallback for a variation
  with no `InverseDef`, which is forward branch-and-bound from the
  ball with a per-variation Lipschitz bound (§7 item 1). The inverse
  walk enumerates only where every variation has an inverse; the
  corpus meter says that is three flames in forty-five today, so the
  fallback is not optional and it is the reach lever for
  [ifs-general.md](ifs-general.md) as well.

### Stage 3 -- the algebra exists in the inverse direction

- **The scalar-generic kernels.** `Real`, `Transcendental`, `Dual`,
  `Dual3` in `src/scene/ifs_real.rs`; the planar and solid kernels
  are one body each over them, and `BigFloat` implements both traits
  -- so a forced prefix's reference orbit can be walked at any width
  and the forward Jacobian is a `Dual` pushed through the same body.
- **The difference-form technique.** `kernel_difference_gen` computes
  `m⁻¹(Z + δ) − m⁻¹(Z)` with every term `O(δ)`, gated to 1.4e-14
  over thirty decades of `δ/|Z|` against a 512-bit subtraction. The
  FORWARD forms this stage needs are the same construction on the
  forward bodies -- and for the polynomial pairs they are literally
  the same functions, since a kernel's inverse form is its inverse
  variation's forward form. Simpler than the inverse ones: a
  contracting map needs no cap, no re-anchor and no remainder.
- **Affine prefixes need none of it**, as the stage always said:
  compose the prefix and the zoom at f64 on the CPU into one
  well-conditioned map, as `seed_beam`'s basis carry does.

### Order

Stage 1; then stage 2 reading `reference_beam`, with the Lipschitz
fallback built alongside it; then stage 3 as forward forms. Stage 0
can ship as a script at any point. Stage 4 stays a contingency.

## 11. Stage 1, built and measured, 2026-09-18

The mechanism is in and correct; its payoff is not demonstrated. Both
halves of that are the point of this section.

### What landed

`FractalConfig::importance` (`enabled`, a per-transform `bias`
vector, the window `m`), `scene::importance::build_table` turning it
into the two tables the kernel reads, a `bias_table` storage buffer at
the free `@binding(11)`, and an `IMPORTANCE_SAMPLING` template flag
gating biased selection, the window's product, the warm-up and the
deposit. Off, the feature contributes no code at all.

The ratio is `(1/b_i)·(Σ_j b_j w_j x[prev][j]) / (Σ_j w_j x[prev][j])`
-- the per-transform factor undone times the row normalisers' own
ratio, with the xaos entry cancelling, which is what lets one formula
serve both selection arms. The sparsity pattern is preserved by
construction, since every bias factor is forced strictly positive.

### Two things the measurements changed

**The warm-up darkened the picture, and `q ≡ p` is what found it.**
The gate deposits at window positions `m..2m` -- `m+1` of every `2m`
iterations -- while the tone map normalises by
`total_iters / pixel_count`, which counts iterations and not deposits.
Rendered at `q ≡ p`, where nothing is biased and nothing should move,
the mean colour sat 0.038 below the truth at `m = 8` and stayed there
at 16 and 32, because the rate is about one half whatever `m` is. The
deposit now carries an exact `2m/(m+1)`. Without the neutral render
this would have been invisible -- it looks exactly like the
correction being imperfect.

**Rounding the deposit's SCALE beats rounding every channel**, which
is not what the theory says. The histogram is u32 and the shipped
path truncates, so a corrected weight under `1/color_scale` would
vanish; stochastic rounding keeps it in expectation. Per-channel
rounding is unbiased in isolation, but the reference it has to agree
with truncates its colour channels too, so matching that convention
is what agrees: measured at `q ≡ p`, mean colour against the feature
off is 0.0048 rounding the scale and 0.0130 rounding every channel.
Scale-only also keeps the deposit EXACT at `m = 1`, which is what
lets the neutrality gate assert bit-identity rather than a tolerance.

### The gates

| | |
|---|---|
| off ⇒ no feature code in the WGSL | asserted, both selection arms, 2D and 3D |
| the whole dump diff | the `Params` pad renamed plus 106 blank lines; **not one code line** |
| `q ≡ p` ⇒ bit-identical render | 0 of 65536 bytes differ |
| the ratio takes `q` back to `p` | `q_i·r_i = p_i` per transform, per xaos row |
| the bias moves no admissible edge | asserted over a sparse xaos with zero, negative and NaN factors |
| the correction renders the true measure | 0.010 against the uncorrected 0.207 -- 95% recovered |
| shipped renders | 330 of 330 visual tests unchanged |

### The window has an OPTIMUM, which §2 does not say

§2 gives a lower bound on `m` from contraction and treats larger as
simply more correct. Measured on a gasket at a 4× bias, mean colour
error against the unbiased truth:

| `m` | 4 | 8 | 16 | 32 |
|---|---|---|---|---|
| error | 0.037 | **0.008** | 0.015 | 0.119 |

A clean U. Too short and the deposited weight covers fewer choices
than the observable needs; too long and the typical product,
`exp(m·E_q[ln r])`, falls under the histogram's own resolution and
the estimate rides on a rare heavy tail -- 0.16 at `m = 8` against
6e-4 at `m = 32`. Sixteen times the samples brings `m = 32` only from
0.119 to 0.056 while `m = 8` sits still at 0.008, which is what says
one is variance and the other is converged. **The lower bound is a
lower bound; the upper one is variance, and nothing in §2 bounds it.**

### ...and the payoff is NOT demonstrated

This is the part that matters for what comes next. The correction
restores the true measure exactly, so the deposited density -- and so
the brightness at a given exposure -- is identical by construction.
Counting lit pixels finds a ratio of 1.01 and is right to. What could
improve is NOISE, so each configuration was rendered twice on
different RNG streams, on the gasket at `S₀`'s own fixed point, which
is the most favourable case there is:

| zoom | unbiased | biased 4× | ratio |
|---|---|---|---|
| 2^2 | 0.0024 | 0.0148 | 6.08 |
| 2^4 | 0.0029 | 0.0108 | 3.70 |
| 2^6 | nothing lit on either side | | |

**Four to six times noisier, not quieter.** The weight's own variance
costs more than the redirection saves at every zoom where the
comparison can be made, and past 2^6 it cannot be made: the view
holds so little of the measure that both sides render black, and
compensating the exposure by the gasket's own dimension does not
bring it back.

Three readings, and the probe states all three rather than picking
one:

- **a fixed per-transform bias is still a polynomial share of the
  orbit.** It moves the constant, and the constant is swamped by the
  weight variance. Stage 2's forced prefix changes the asymptotics
  instead, and carries the prefix's own `∏p` as the weight rather
  than a product of ratios -- so it has no window, no epoch and no
  accumulated variance at all. This measurement is an argument for
  going straight there;
- **the policy may simply be wrong.** A 4× boost on one transform is
  a guess; open question 1 says the admissible-prefix measure is the
  principled bias. This fixture cannot tell "the mechanism does not
  pay" from "nobody has chosen a good `q`";
- **the fixture may be too kind to the unbiased game.** A gasket is
  self-similar, so every neighbourhood is reachable by a short
  address; a real flame's deep view may need a long and improbable
  one, which is where redirection is worth most.

So stage 1 ships as a mechanism with a measured cost and no measured
benefit, off by default, and the honest next step is stage 2 rather
than a policy layer on top of this.

### Not built

The `ConfigPath` entries (so scripting and undo reach it) and the UI
section. Both are policy surface, and §9's open question 1 is still
open; a `.fflame` carries the settings and the CLI renders them
today, which is enough to measure with.

## 13. Stage 2's kernel half, 2026-09-18

The forced prefix runs. A targeted render is the untargeted render,
from a sixteenth of the iterations — verified to a zoom of 2^10, at a
speedup of 729x.

### A word is ONE affine

Every map here is affine, so `S_a = S_{a_k} ∘ … ∘ S_{a_1}` composes
on the CPU to a single 2×2 and a translation — forcing a prefix of
eighteen transforms costs the kernel one matrix multiply, not
eighteen. That is stage 3's note arriving early, and it is what makes
the kernel half small.

The COLOUR folds the same way: flam3's `c ← c·h + g` per transform is
an affine map of `c`, so a whole word is `c ← c·H + G` with
`H = ∏ h_j` and `G = Σ_i g_i ∏_{j>i} h_j`. Two more numbers per word,
and the plotted colour is exact rather than approximated.

At the plot, the kernel applies the word to `current`, plots, and
RESTORES — so the final chain, post-symmetry, the depth effects and
the deposit all act on the forced point with no change of their own,
and the free orbit carries on from where it was. Feeding the forced
point forward would collapse the walk into the one cylinder the word
names.

### The weight is an iteration count, not a deposit

The estimator says each forced sample carries `P(A_V)`. Depositing
that would be hopeless: at 2^20 it is 2.6e-9, and the u32 histogram
would round every deposit to zero but for a one-in-a-hundred-million
tail — handing the whole advantage back to quantisation, which is
exactly the trap stage 1 fell into from the other side.

So the deposit carries ONE, at full resolution, and the tone map is
told the render did `N / P(A_V)` iterations. That is the same
statement — a forced render is doing the work of that many unbiased
iterations — and saying it there costs nothing.

### What it measures

| zoom | P(A_V) | speedup | lit ref / tgt | overlap | brightness |
|---|---|---|---|---|---|
| 2^2 | 1.00e0 | 0.5 | 594 / 603 | 100.0% | 0.514 / 0.508 |
| 2^4 | 1.11e-1 | 3.0 | 595 / 603 | 99.8% | 0.266 / 0.263 |
| 2^6 | 1.23e-2 | 16.2 | 600 / 594 | 99.0% | 0.153 / 0.154 |
| 2^8 | 1.37e-3 | 104.1 | 596 / 594 | 99.7% | 0.093 / 0.093 |
| 2^10 | 1.52e-4 | 729.0 | 588 / 594 | 99.5% | 0.056 / 0.057 |

The same picture at seven hundred times the rate. The 2^2 row is the
DECLINE path on purpose: its speedup is 0.5, so the renderer refuses
to target and the two differ only by iteration count — a decline that
silently drew something else would show here as clearly as a forced
prefix naming the wrong word.

It stops at 2^10 because the REFERENCE gives out, not the target. At
2^12 the unbiased render lights 252 pixels to the targeted one's 378
and the overlap falls to 78%: the targeted render is drawing
structure its starved reference never reaches, which is the direction
the stage exists to produce and is also exactly what makes it
unverifiable. There is no comparison past the point where nothing
else can draw the picture.

Off, the feature contributes **no code at all**: the whole
canonical-dump diff is 40 blank lines where the stripped `{{#if}}`
blocks were, and all 330 visual tests are unchanged.

### An ordering bug worth recording

For one build the render came out as the plain one — the buffer
uploaded, the shader never asking for it. `constants_from_config`
hard-codes the flag off because it has no frame size and does not run
the enumeration, and the renderer's override ran AFTER the shader was
built. The verdict is threaded as a parameter now, exactly as
`census` is beside it, and the enumeration runs first. It was visible
only because the gate compared against a reference; a brightness
check alone would have called it a pass.

### The wrong diagnosis, and what it actually was

This section previously read *"Past 2^4 there is nothing to compare
against, and it is not the sampling"*, and attributed it to the tone
map's `total_iters / pixel_count` normalisation. **That was wrong,
and the way it was wrong is worth keeping.**

The evidence looked airtight: past 2^4 both renders came out with max
channel ZERO — not merely dark — and four thousand times the exposure
brought neither back, while the enumeration could show the samples
were landing in the view. Every one of those observations was true.
The conclusion did not follow.

It was the PALETTE. The fixture zooms toward `S₀`'s fixed point, so
the cylinder a deep view selects is the all-`S₀` word; flam3's colour
rule walks the colour coordinate toward the colour of whatever
transform ran last; and the fixture gave transform 0 the colour
`0.0`, which is the black end of the palette. Full density, correctly
deposited, rendered black. Moving the fixture's colours off zero
(`gasket_config` starts at 0.5 now) brought back **six zoom levels
that were thought to be out of reach**, and the table above is the
result.

The lesson is that a black render has more than one cause and they
present identically. "Max channel zero, unmoved by exposure" reads as
starvation, and a colour coordinate pinned at the palette's black end
produces exactly that signature with a completely healthy histogram
underneath. The discriminator that would have caught it in one step:
compare the render against a CPU chaos game's PIXEL OCCUPANCY, not
its sample count. The CPU said 594 distinct pixels at every zoom
while the GPU said one — a contradiction no exposure theory explains.

The iteration-count normalisation IS a real limit on deep views —
that is what §14 is about, and it is measured there on a fixture that
does not have this defect. It simply was not what made this gate
dark.

## 12. Stage 2's enumeration, built and measured, 2026-09-18

The CPU half: `scene::cylinder` enumerates the words whose image
reaches the viewport, with the probabilities the forced sampler
needs. The kernel half — burn in, force a word, plot — is next.

### The identity, stated so it can be checked

`μ = Σ_i p_i (S_i)_* μ` unrolled along a complete antichain `A`, a
prefix-free set every infinite path crosses exactly once:

```text
μ|_V = Σ_{a ∈ A_V} p_a · (S_a)_* μ |_V,   A_V = {a ∈ A : S_a(B) ∩ V ≠ ∅}
```

Sample `a` with probability `p_a / P(A_V)`, draw `x ~ μ` from the
true chaos game, plot `S_a(x)` with weight `P(A_V)`. **Exact, for
any `A_V` built this way**, however loose the bound that built it: a
loose bound admits words whose image does not really reach `V`, and
their samples land outside and are not seen. Efficiency, never
correctness.

The contrast with stage 1 is the whole point. The weight is one
number for the view rather than a product accumulated along an orbit,
so there is no window, no epoch and **no weight variance at all** —
which is what stage 1's 6x noise penalty was.

### What it measures

On a gasket centred at `S₀`'s own fixed point, so the view is on the
set at every scale:

| zoom | words | depth | P(A_V) | speedup |
|---|---|---|---|---|
| 2^1 | 3 | 1 | 1.00e0 | 0.5x |
| 2^3 | 1 | 1 | 3.33e-1 | 1.5x |
| 2^6 | 1 | 4 | 1.23e-2 | 16.2x |
| 2^9 | 1 | 7 | 4.57e-4 | 273x |
| 2^12 | 1 | 10 | 1.69e-5 | 5,368x |
| 2^16 | 1 | 14 | 2.09e-7 | 318,865x |
| 2^20 | 1 | 18 | 2.58e-9 | 20,390,552x |

`speedup` is `1/(mass·(depth+1))`: every sample lands in view at a
cost of `depth+1` map applications, against the unbiased game's one
application of which a `mass` fraction is useful. **That is the
change of asymptotics** — `1/mass` grows like `zoom^D` while `depth`
grows like `log zoom` — and it is what stage 1 could not do.

**Below 2^2 it is a LOSS**, which is the right answer: the whole
attractor fits the view, every sample is already useful, and the
prefix is pure overhead. The renderer has to be able to decline.

Only ONE word survives from 2^3 on: a view that small sits inside a
single cylinder, so the branching is transient. The `MAX_WORDS` cap
is for a viewport straddling many pieces, not for the common case.

### What is gated

| | |
|---|---|
| the kept mass bounds the measure in view | `P(A_V)` against a 400k chaos sample, ratio 2.00 to 2.02 across six zooms |
| the antichain is prefix-free | no kept word is a prefix of another, three zooms |
| a word's image contains where its points land | the sample pushed through each word, against the claimed disc |
| both stopping rules | every kept word reaches the view AND fits inside it |
| the speedup grows, and the depth tracks the zoom | asserted per octave |
| the refusals refuse | nonlinear by index, xaos, expanding, off-attractor |

The mass/sampled ratio being a constant 2.00 is the bound's own
looseness and nothing else: the view is tested as the disc through
the frame's corners, which is `√2` wider than the frame in each
direction. Two forced samples per useful one, at every depth.

### Two fixture errors worth recording

The first centre was `(0.25, 0.25)`, an arbitrary point. A gasket is
measure zero, so a generic point is not in it, and the view came up
EMPTY past 2^16 — the enumeration was right and the fixture was
wrong. The centre is `S₀`'s fixed point now, which is in the set at
every scale by construction.

And the first speedup gate asserted monotone growth from 2^1, where
targeting is a loss and the value is flat. Asserting growth where
there is nothing to gain would have been asserting noise.

### Deliberately not here

**Affine transforms only, and no xaos.** Both are about the bound
rather than the identity. A nonlinear map's image bound needs a
Lipschitz constant per variation — §7 item 1, the shared piece the
escape-time plan also wants, still unbuilt — and an enumeration on a
guessed constant would be wrong rather than loose. Under xaos the
first symbol's probability is conditional on the burn-in's last
transform, so the sampling table is per-predecessor; the identity is
unchanged and the bookkeeping can follow.

`NoCylinders` names which one turned a flame away, so the panel can
say so rather than silently rendering the ordinary way.

## 14. Auto exposure, built and measured, 2026-09-19

The tone map normalises by `sample_density = total_iters /
pixel_count` — which assumes the frame holds all the work. A zoomed
view does not: most of the attractor is off-screen, so the samples
that DID land get divided by a count dominated by ones that did not,
and the picture fades as the zoom deepens. `auto_exposure` measures
the share that landed and scales the normalisation by it.

### Reusing the solid renorm, not Density Levels

Density Levels was the obvious candidate and is the wrong half of the
right idea. Its OUTPUT is opacity — `apply_levels` reaches
`fractal_alpha` and nothing else, as `tonemap.wgsl` says in as many
words — and a deep view is not a transparency problem. Its INPUT
(`DensityHistogram`: accumulator readback, percentiles, mean density)
is real measurement infrastructure, but it is a full-frame readback
producing percentiles nobody here needs.

The closer precedent was already a term in the line being changed:
`solid_density_fraction`. Solid rendering hit the structurally
identical problem — occlusion culls most dispatched samples, so
`total_iters/pixel_count` overstates what reached the image — and
fixed it by measuring the surviving fraction on the GPU and
multiplying it into `sample_density`, with an EMA-smoothed
interactive path and a blocking exact path for one-shot renders.
Frame coverage is the same sentence with a different numerator, and
it reuses `BoundsTracker` outright for both paths.

    sample_density = samples_in_buffer
                   * solid_density_fraction
                   * frame_coverage_fraction     <- new
                   * cylinder_iteration_scale()
                   / pixel_count

### The counters

Two u32 words in their own 32-byte buffer at `@binding(16)`: plot
attempts that landed in frame, and plot attempts. Gated on the same
`should_plot` the plot itself uses, so only the GEOMETRIC miss is
measured — a sample already suppressed by opacity or the importance
window is in neither the numerator nor the denominator. That
narrowing is the solid renorm's hard-won lesson: folding an artistic
weight into the fraction makes that dial shift global brightness.

They are counted per thread in registers and flushed ONCE after the
iteration loop, deliberately not subsampled. Solid can subsample one
thread in 1024 because its fraction is order-one; at depth the
in-frame count is small by definition, and sampling a thousandth of
it would read zero exactly where the number is needed. Two atomics
per thread instead of two per iteration costs nothing.

### What it measures

Gasket, generic point of the set, 96x96, 64M iterations:

| zoom | coverage gpu / cpu | max off / on | mean off / on |
|---|---|---|---|
| 2^2 | 7.22e-1 / 7.22e-1 | 234 / 252 | 0.0814 / 0.0878 |
| 2^4 | 1.12e-1 / 1.11e-1 | 167 / 255 | 0.0642 / 0.1065 |
| 2^6 | 8.28e-3 / 8.25e-3 | 101 / 255 | 0.0265 / 0.0788 |
| 2^8 | 9.02e-4 / 9.10e-4 | 60 / 255 | 0.0160 / 0.0785 |
| 2^10 | 1.04e-4 / 1.07e-4 | 37 / 255 | 0.0094 / 0.0749 |
| 2^12 | 8.28e-6 / 9.50e-6 | 28 / 255 | 0.0029 / 0.0352 |

The `cpu` column is an independent chaos game on the CPU counting
in-rectangle hits. It agrees to two or three figures at every zoom,
which is what makes the fraction trustworthy rather than merely
plausible; the gate asserts it.

Without the correction the image fades monotonically — max channel
234 down to 28 — while the structure is still fully sampled. With it,
a 2^10 view is exposed to full scale from the same iterations.

### Off by default, deliberately

Coverage is below one for nearly every flame: some samples always fly
off-frame. Switching this on by default would change the brightness
of essentially every render ever made, so it does not. When it should
engage on its own is a separate decision and a better one to take
with the coverage curve in hand than in advance. Off, the feature
contributes no code at all — the whole canonical-dump diff is 48
blank lines where the stripped `{{#if}}` blocks were, zero non-blank
lines, and all 330 visual tests are unchanged.

### Where it gives up

`coverage_from` refuses to report on fewer than 32 landings. Below
that the ratio is dominated by its own shot noise and a frame holding
a handful of samples has no exposure that makes it a picture;
refusing leaves the last good value rather than setting the
brightness from a coin flip. A magnitude floor was tried first — a
clamp at 1e-6 — and was quietly binding at 2^14, which is inside the
range the feature is FOR. A minimum-hits rule is the honest bound;
the clamp now only stops a zero reaching the divide.

The tiled high-resolution exporter allocates the buffer so the
layout is uniform but does not auto-expose: it renders many views and
coverage is a property of one view.

