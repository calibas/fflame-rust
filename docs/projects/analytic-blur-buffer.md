# Analytic Blur Buffer (experimental)

Status: **Phase 1 complete** on branch `analytic-blur-buffer`. The sections
below are the original design; this status block records what actually
shipped and where it diverged.

## v1 as built (Phase 1)

Interactive / 2D-direct path only, end-to-end and golden-test-passing.

- **New opt-in variations, not modified originals.** `analytic_blur` and
  `analytic_gaussian_blur` (`src/variations/defs/analytic_blurs.rs`) are
  byte-identical copies of `blur` / `gaussian_blur` tagged
  `Feature::AnalyticBlur`. The stochastic originals are untouched, so a flame
  opts in by *choosing* the analytic variant. (The design below says "tag
  `blur`/`gaussian_blur`" — superseded by this.)
- **Host gate** (`Flame::analytic_blur_active`, `Transform::analytic_blur`):
  exactly one normal-phase `AnalyticBlur` variation on a transform, no other
  RNG variation on it; and whole-flame: **2D render mode**, no Linked/Final
  attachments, no subflames, **no post-symmetry**. Any miss → the feature is
  entirely off (no codegen, no buffers) and the flame renders stochastically.
  3D (even orthographic) and post-symmetry are deferred (they break the
  single-linear-map plot tail / fan one sample into many copies).
- **Plot routing is direct-mode + 2D only** (`HAS_ANALYTIC_BLUR` block in
  `main_template.wgsl`): the mean splat = realized output − `M_post·(w·offset)`
  goes to the transform's blur slice (binding 13), and the realized sample is
  suppressed from the main histogram. The mean pixel gets its own bounds
  check. Sample-emit / export stays stochastic (the block is gated to
  `OUTPUT_HISTOGRAM_DIRECT`; naga strips binding 13 from the export shader).
- **Per-transform buffers capped at `MAX_BLUR_BUFFERS = 4`** (concatenated
  slices in one buffer; eligible transforms beyond 4 keep slot −1 →
  stochastic). `Transform.analytic_blur_slot` carries the slot to the shader.
  The slot count is **further capped by a memory budget**
  (`FlameBuffers::max_blur_slots` = `max_binding / (3·W·H·16)`, since each slot
  needs three full-res buffers): a multi-blur flame or a high-res render only
  gets as many analytic slots as fit, the rest falling back to stochastic, so
  it can't OOM / exceed a binding. Threaded through `from_flame` (slot
  assignment), `ensure_blur_buffers`, and the renderer's `blur_slots`.
- **Export coverage:** analytic blur runs on the FlameRenderer export path
  (when the histogram fits one storage binding — ~4K–11K depending on the
  GPU). Larger exports route to the tiled/CPU `HighResExporter`, which is
  sample-emit and doesn't wire analytic blur (Phase 2), so the blur renders
  stochastically there. Both paths are crash-free; the per-frame dither lives
  in the upscale pass and therefore applies on the FlameRenderer path only.
- **Convolution at reduced resolution** (3 stages in `accumulate_pass`, before
  the spatial filter). The blur is low-frequency, so convolving it at full res
  is wasteful — and fatal for perf, since the cost is O(half²)/pixel and a
  typical blur's half is 64+px (a 17× slowdown before this was added). So:
  `blur_downsample.wgsl` sums each D×D cell into a low-res slice →
  `blur_convolve.wgsl` convolves at low res with a low-res-scale kernel →
  `blur_upscale.wgsl` bilinearly upsamples + adds into the main histogram
  (÷D² so density and the colour ratio are preserved end-to-end; no atomics —
  each output pixel is uniquely owned). D is chosen per view/flame so the
  low-res kernel half ≈ `TARGET_LOWRES_HALF` (48); cost ∝ 1/D⁴ → negligible at
  any blur size, over-blur ≈ 1/TARGET ≈ 2%. The full-res splat buffer is
  cleared each batch alongside the histogram; the two low-res scratch buffers
  are fully overwritten each pass.
- **Kernel** (`src/variations/analytic_blur.rs`, `maybe_rebuild_blur_kernels`):
  deterministic CPU Monte-Carlo of the variation's offset sampler mapped by
  `world→pixel linear · weight · M_post`, bilinearly binned, normalized.
  Half-extent **clamped to `MAX_KERNEL_HALF = 64`px** (large full-image blurs
  truncate — a documented v1 limitation; the convolution is O(half²)/pixel).
  Rebuilt only when the view (zoom/rotation) or flame changes.
- **Golden test:** `scripts/verify_analytic_blur.py` (energy + no-bias +
  smoother-than-stochastic) plus the `build_kernel` unit tests.
- **Knobs** (`analytic_blur` / `analytic_gaussian_blur` params, read into
  GpuTransform): `strength` (default 1.0) scales the mean-splat density so the
  smooth blur dominates/masks overlapping transforms; `residual` (default 0)
  routes the next N plots after a blur through its buffer too, smoothing the
  propagated fuzz (softer trailing structure, reuses the blur kernel as an
  approximation). Both default to no-ops. The analytic blur only de-noises the
  blur transform's own plot (+ N residual plots); other transforms' grain and
  the IFS structure itself are unaffected — for general grain use the spatial
  filter (`filter_radius`).

Deferred to later phases: export/tiled sample-emit routing (Phase 2); more
analytic-blur variations, low-res buffer, and 3D-through-projection (Phase 3).

## The idea

Blur-family variations (`blur`, `gaussian_blur`, …) add input-independent
random fuzz to a transform's output. Rendered stochastically, a blurred
region needs a *lot* of samples to look smooth — the fuzz is pure noise
that only averages out slowly.

Instead, render the fuzz **analytically**:

1. When the chaos game lands on a blur-eligible transform, plot the
   **mean** of its output (the deterministic center, fuzz removed) into a
   **separate per-transform blur buffer** — *not* the main histogram.
2. After iteration, **convolve** each blur buffer with that transform's
   blur kernel (the fuzz distribution, pushed through the transform's
   linear map to pixel space).
3. **Add** the convolved blur buffer(s) into the main histogram, then run
   the existing accumulate + tonemap **once**.

The orbit still advances the *real* stochastic point, so downstream
structure is unaffected — only the blur transform's own plot is made
analytic. The result: smooth blur at a fraction of the sample count.

## Why it composites cleanly

The blur buffer uses the **same `[Rsum, Gsum, Bsum, density]` 4×u32
layout** as the main histogram (`shaders/core/header.wgsl`,
`shaders/core/main_template.wgsl` plot section). Convolving all four
channels with the same normalized kernel preserves the `Σcolor/Σdensity`
ratio that `accumulate.wgsl` divides by. Adding the result into the main
histogram and tonemapping once means the normalization is subsumed into
the single final divide — nothing to get wrong, no muddy-fringe failure
(we never resolve the blur buffer to RGB on its own).

## The four invariants (correctness)

These come from the design discussion and are non-negotiable:

1. **Mean-splat.** The blur buffer receives the transform output with the
   analytic-blur variation's offset set to its **mean** (`(0,0)` for the
   origin-centered blurs), carrying the **same color index** a normal splat
   would. Not the realized noisy point.

   The analytic blur is an *additive* normal-phase term:
   `Σ w_i·var_i(affine_p)` includes `w_blur·offset`. So
   `mean_preaffine = realized_preaffine − w_blur·offset`, and after the
   (linear) post-affine `M_post`:
   `mean_plot = realized_plot − M_post·(w_blur·offset)`. The shader isolates
   `w_blur·offset` (the blur variation writes its raw offset to a register),
   plots the mean to the blur buffer, and advances the orbit with the
   realized point. **Crucially, the transform's *other* variations may be
   nonlinear** (`spherical`, …) — they're the deterministic structure the
   mean-splat already captures; only the additive fuzz's path to the pixel
   must be linear.

2. **Route, don't duplicate.** A blur-terminated iteration plots to the
   blur buffer **instead of** the main histogram, not in addition. The
   orbit feeds the real stochastic point forward as usual.

3. **Kernel = base blur kernel pushed through the transform's linear map.**
   The fuzz is added in variation-output space, then mapped to pixels by
   `post-affine ∘ (finals) ∘ projection`. The plot-space kernel is the
   base distribution's covariance transformed by the **linear (Jacobian)**
   part of that map — anisotropic if it shears/scales. Build it from the
   affine, not as a fixed disc.

4. **Gate the analytic path (the hard boundary).** A transform is eligible
   **only** when:
   - it has **exactly one** active analytic-blur variation, in the **normal
     phase** (so the offset is additive — a pre/post-moved blur would route
     through other variations and break linearity), and
   - **no other** active variation uses RNG (any other stochastic term —
     including input-dependent `radial_blur`/`farblur`/`post_rblur`/`exblur`
     — breaks the input-independence), and
   - the **fuzz's path to the pixel is linear**: affine post-affine,
     affine-or-absent finals, orthographic projection.

   Non-blur companions may be nonlinear (captured by the mean-splat). Any
   gate failure → **stochastic fallback** to the main histogram. Explicit
   per-transform check, never an assumption. (The resolution-independent
   part — one normal-phase blur, no other RNG var — is checked on the
   `Transform`; the plot-path-linearity part is added by the renderer.)

## The golden test (gates merge)

Render the same flame twice at high sample count — analytic blur buffer
vs. fully stochastic — and diff. They must match within noise. This one
test catches kernel-normalization errors, the D²-brightness bug, and any
gate mistake immediately. Lands with phase 1; no analytic-blur work merges
without it passing.

---

## Architecture

### Eligibility + kernel (host, per transform)

At shader/buffer build time, classify each active transform:

- `analytic_blur_eligible: bool` — passes the gate (invariant 4).
- `kernel: Mat2` (2×2 covariance / linear map) — the base blur extent
  pushed through the transform's linear map to pixel space (invariant 3).
- `kernel_shape: Disc | Gaussian` — from the variation's metadata.

**The host Monte-Carlos the kernel from the variation's own offset
formula.** `blur` (uniform-radius disc, areal density ∝ 1/r) and
`gaussian_blur` (Irwin-Hall(4) bell) are both isotropic with σ²=1/6 at
weight 1, but have *different radial profiles*, so a generic gaussian
kernel would fail the golden test for `blur`. Instead, each
analytic-blur variation provides a Rust **offset sampler** mirroring its
WGSL offset (`θ=rand·2π`, `r=rand` for `blur`; `r=Σ4 rand − 2` for
`gaussian_blur`). At build time the host draws ~10⁵ offsets, maps each
through the transform's pixel-space linear map (`weight · post-affine
linear · world→pixel`), and bins them into a small normalized 2D kernel
array. This **matches the stochastic offset distribution by construction**
(it *is* that distribution, sampled), handles anisotropy via the linear
map, and needs no per-variation PDF derivation. The kernel array is
uploaded per eligible transform; the convolution reads it directly.

The combined "is the feature active at all?" flag = `any transform
eligible`. **If false, nothing below is allocated or compiled in.**

### New `Feature::AnalyticBlur`

A variation tagged `AnalyticBlur` (a) is input-independent fuzz and (b)
carries a kernel descriptor. Added to the `Feature` enum
(`src/variations/definition.rs`) and mirrored in `VariationInfo`
(`src/variations/mod.rs`). The shader builder
(`src/shader_builder_v2.rs`) reads it to:
- compute per-transform eligibility,
- emit the mean-splat routing in the plot section (template-gated),
- strip everything when no transform is eligible.

v1 variations: `blur`, `gaussian_blur` only. More follow once the test
harness is trusted.

### Per-transform blur buffers (feature-gated)

One mean-splat buffer per *eligible* transform, **same 4×u32 layout** as
the histogram, full image resolution (v1 — see Resolution). Allocated
lazily: zero buffers when no transform is eligible. Buffer count = number
of eligible transforms (typically 1–3; `MAX_TRANSFORMS=128` is the
ceiling, never the norm).

### Plot routing (compute shader, both output modes)

The plot section of `main_template.wgsl` has two modes:
`OUTPUT_HISTOGRAM_DIRECT=true` (interactive: atomic-add into histogram)
and `=false` (export: emit a `Sample`). Both must route a blur-terminated
iteration to the blur buffer:

- **Direct mode:** atomic-add the mean-splat into the selected transform's
  blur buffer (4 atomic adds) instead of the main histogram.
- **Sample-emit mode:** emit a `Sample` tagged with the blur-buffer target
  (a transform index / blur flag — extend the `Sample` struct or reserve a
  channel). The scatter pass (`accumulate_samples.wgsl`) routes tagged
  samples into the per-transform blur histogram instead of the main one.

Gated by a template flag (`HAS_ANALYTIC_BLUR`) so non-blur flames compile
byte-identical to today.

### Convolution pass (new)

A compute shader: for each eligible transform, convolve its blur buffer
with the transform's pixel-space kernel and **add** into the main
histogram. Because the kernel support is small (a few px for v1's modest
radii) and the disc/anisotropic kernels are **not separable**, use a
direct small 2D kernel — do **not** force everything through a separable
Gaussian path (`shaders/histogram_blur.wgsl`'s separable bilateral pass is
the wrong tool here). Runs after iteration, before accumulate/tonemap.

### accumulate / tonemap: unchanged

Both consume whatever's in the main histogram. Once the convolved blur is
added in, they run exactly as today.

---

## Phase 2 plan — low-res-native + CPU-histogram inject

The original "tiled = the hard part" framing (full-image convolution across
tile seams) was an artifact of the pre-low-res design. With the low-res
convolution the seam problem evaporates: the convolution runs on a **tiny
low-res buffer that fits one binding**, and the only full-res touches
(splat, upscale) are per-pixel-local, so they tile trivially.

Two locked decisions drive the plan:

1. **Low-res native across the board.** Both the direct (FlameRenderer) and
   tiled (sample-emit) paths write mean-splats straight into a **low-res**
   buffer at `mean_pixel ÷ D`. The full-res splat buffer and the downsample
   pass are **removed entirely** — the splat target *is* the convolution
   input. (Mean quantization to D px is negligible: the blur is ≈16·D px
   wide.)
2. **Shared CPU/GPU upscale math.** The bicubic-B-spline + ÷D² + stochastic
   dither upscale is ONE algorithm, implemented identically in WGSL
   (`blur_upscale.wgsl`) and Rust (for the tiled CPU inject), guarded by a
   unit test that runs both on the same low-res input and asserts they match.

### Step 1 — low-res-native rework (direct path; refactor, no new capability)

- **Buffers**: 2 low-res buffers (atomic splat target + convolved scratch)
  instead of 1 full-res + 2 low-res. Sized at actual low-res dims
  (`ceil(W/D)×ceil(H/D)`), reallocated when D changes — D is the discrete
  cost-based step, so reallocation is rare interactively and **never on
  exports** (fixed view). Memory drops from ~3×full-res to ~tiny, raising
  the FlameRenderer resolution ceiling and loosening `max_blur_slots`.
- **Routing** (`main_template`, direct path): splat the mean at
  `mean_pixel ÷ D` into the low-res atomic buffer (binding 13, now low-res
  sized). It reads D + low-res dims from the existing `blur_convolve_params`
  uniform, bound to the **main** compute bind group at a new binding (14) —
  chosen over a `GpuParams` field to avoid std140 churn.
- **Drop** `blur_downsample.wgsl`, its pipeline, bind group, dispatch.
- **Device ordering (the real friction)**: the low-res splat buffer's size
  depends on D, which is computed per-frame by `maybe_rebuild_blur_kernels`
  and consumed by the main dispatch in `compute_pass` — so the buffer must be
  (re)sized *before* that dispatch. `compute_pass` (or a small prep step)
  gains `device` access; `maybe_rebuild` ensures the low-res buffers and
  rebuilds the compute bind group when D changes, ahead of the main dispatch.
- **Watch**: atomic contention rises (D² fewer splat pixels) — expected fine,
  perf-check it.

### Step 2 — tiled blur via CPU-histogram inject

Both `ParallelTiles` (GPU tile histogram → read back) and `SerialTiles`
(CPU sample binning) converge to one CPU `Vec<HistogramPixel>` right before
`tonemap_gpu` (`high_res.rs`). That convergence is the single inject point.

- **Sample-emit routing** (`main_template`, `OUTPUT_HISTOGRAM_DIRECT=false`):
  emit a mean-splat `Sample` tagged with its blur slot (+ strength/residual),
  suppressing the realized sample. The main shader emits the full-res mean
  pixel; the scatter does the ÷D (so the sample-emit main shader needs no D).
- **Tagged scatter** (`accumulate_samples.wgsl`): route tagged samples into
  the low-res blur buffer at `mean_pixel ÷ D` (reads `blur_convolve_params`)
  instead of the tile histogram.
- **Exporter** (`high_res.rs`): port the host kernel build
  (`maybe_rebuild_blur_kernels`); allocate the 2 low-res buffers (tiny); run
  the GPU low-res convolve; read the convolved buffer back to CPU; **CPU
  upscale-add** into the `histogram` Vec before tonemap, using the shared
  upscale math (decision #2). One inject covers both tiled modes; the convolve
  is on the tiny low-res buffer so there is no tile-seam handling.
- **Note**: the tiled export convolves once at the end (not per batch), so the
  dither is a single static pattern — negligible at export sample counts.

### Sequencing

Step 1 first — it unifies the buffer model, proves low-res-native on the
well-tested direct path, and delivers the memory/ceiling win on its own.
Step 2 then reuses the same low-res buffers, convolve pass, and shared
upscale math.

---

## Resolution (shipped in v1)

The design originally deferred the low-res buffer as a perf optimization. In
practice it's **required**: the convolution is O(half²)/pixel/frame and a
typical blur's half is large, so full-res convolution ran ~17× slower than
stochastic. v1 therefore convolves at reduced resolution (downsample →
low-res convolve → bilinear upscale + `1/D²` energy carry), with `D` chosen
per view/flame from the mapped support rather than a fixed `R/4`. See the
"v1 as built" convolution bullet. The brightness/energy bookkeeping the
design flagged is handled by the `÷D²` on upscale (verified: `D=1`
reproduces the full-res result exactly).

---

## Phased plan

**Phase 1 — interactive, full-res, golden test. ✅ DONE** (see "v1 as built").
- `Feature::AnalyticBlur` on new `analytic_blur` / `analytic_gaussian_blur`
  (copies of the originals — originals untouched).
- Host eligibility + per-transform pixel-space kernel; `HAS_ANALYTIC_BLUR`
  flag; zero-allocation when no transform eligible.
- Per-transform blur buffers (cap 4) + mean-splat routing (2D direct mode
  only) + convolution-add pass.
- Golden diff test (analytic vs stochastic within noise). **Passes.**

**Phase 2 — export / tiled.** See "Phase 2 plan" above. Step 1: low-res-native
rework of the direct path (drop the full-res splat buffer + downsample; splat
straight to low-res; memory + ceiling win). Step 2: tiled blur via a single
CPU-histogram inject (sample-emit tag + scatter to low-res, GPU low-res
convolve, CPU upscale-add) covering both ParallelTiles and SerialTiles.

**Phase 3 — coverage.** More analytic-blur variations (`circleblur`,
`sineblur`, `blur_circle`, `pre_blur3D`, …); 3D kernel through the projection
Jacobian (depth-varying — currently gated out). (The low-res blur buffer
optimization landed early in v1 — see "Resolution".)

## Open questions / risks

- **Kernel through finals + perspective.** v1 gates these out (linear path
  only). Perspective makes the Jacobian depth-varying; finals with
  nonlinear variations break linearity. Both are explicit gate failures →
  stochastic fallback, never silent mis-shaping.
- **Mean of each blur.** Centered blurs have mean `(0,0)` offset; verify
  per-variation (`gaussian_blur`'s `sum-of-4 − 2` is mean-0; `blur`'s
  uniform disc is mean-0). The metadata must state the mean, not assume 0.
- **Color at the mean.** The mean-splat carries the iteration's
  `color_index` exactly as a normal splat would; the convolution spreads
  `Rsum`/`Gsum`/`Bsum` and `density` together, so recovered color is
  kernel-invariant.
- **Multiple blur transforms.** Each needs its own buffer+kernel; they sum
  independently into the histogram. No cross-talk.

## Touch points (reference — as built)

- Feature/variations: `src/variations/definition.rs` (`Feature::AnalyticBlur`),
  `src/variations/defs/analytic_blurs.rs` (the two new variations) + registration
  in `defs/mod.rs`, `src/variations/analytic_blur.rs` (offset samplers +
  `build_kernel`).
- Gate: `Flame::analytic_blur_active` / `Transform::analytic_blur` in
  `src/scene/transforms.rs`.
- Codegen + flag: `src/shader_builder_v2.rs` (`has_analytic_blur`,
  `blur_contribution` capture), `shaders/core/main_template.wgsl` (plot
  routing), `shaders/core/header.wgsl` (binding 13 + `analytic_blur_slot`).
- Buffers + passes: `src/gpu/buffers.rs` (blur histograms, kernel/convolve
  buffers, slot assignment), `src/gpu/pipelines.rs` (convolve pipeline +
  bind group), `src/renderer/compute_kernel.rs` (allocation, kernel rebuild,
  convolve dispatch), `shaders/blur_convolve.wgsl`.
- Export (Phase 2, not yet wired): `src/export/high_res.rs`,
  `shaders/core/accumulate_samples.wgsl`.
- Test: `scripts/verify_analytic_blur.py` + `build_kernel` unit tests.
