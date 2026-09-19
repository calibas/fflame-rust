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

### Stage 1 — biased selection + windowed correction (the core)

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

### Stage 2 — cylinder targeting (the free lunch)

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

### Stage 1 -- nothing exists, and nothing is needed

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
