# Analytic Blur Buffer (experimental)

Status: **design + in progress** on branch `analytic-blur-buffer`.

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

1. **Mean-splat.** The blur buffer receives the transform output with each
   analytic-blur variation's offset set to its **mean** (the deterministic
   center — `(0,0)` offset for the origin-centered blurs), carrying the
   **same color index** a normal splat would. Not the realized noisy point.

2. **Route, don't duplicate.** A blur-terminated iteration plots to the
   blur buffer **instead of** the main histogram, not in addition. The
   orbit feeds the real stochastic point forward as usual.

3. **Kernel = base blur kernel pushed through the transform's linear map.**
   The fuzz is added in variation-output space, then mapped to pixels by
   `post-affine ∘ (finals) ∘ projection`. The plot-space kernel is the
   base distribution's covariance transformed by the **linear (Jacobian)**
   part of that map — anisotropic if it shears/scales. Build it from the
   affine, not as a fixed disc.

4. **Gate the analytic path (the hard boundary).** Eligible **only** when
   *every* RNG-using variation in the transform is an input-independent
   analytic blur, **and** the plot path from its output to the pixel is
   linear (affine post-affine, affine-or-absent finals, orthographic
   projection). Any input-dependent fuzz (`radial_blur`, `farblur`,
   `post_rblur`, `exblur`), any other stochastic variation, or a nonlinear
   downstream map → **stochastic fallback** to the main histogram. This is
   an explicit per-transform check, never an assumption.

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

## Tiled / export integration (the hard part)

The export path (`src/export/high_res.rs`) doesn't atomic-add; it
**emits `Sample`s** and scatters them into per-tile histograms
(`accumulate_samples.wgsl`), tiling by horizontal row-strips. Two
complications the convolution must respect:

1. **Tile boundaries.** A kernel near a tile edge spreads across tiles. So
   the convolution must operate at **full-image scope** for the blur
   buffer, *before* the result is added into the (tiled) main histogram —
   or tiles must overlap by the kernel radius. v1 plan: build the
   per-transform blur histogram at full image resolution, convolve it
   full-image, then add each tile's sub-region into that tile's main
   histogram during the per-tile accumulate.

2. **Size limits.** A full-image blur buffer hits the same storage-binding
   / buffer-size limits that forced tiling in the first place. v1 plan:
   analytic blur in the tiled path requires the (per-transform) blur
   buffer to fit `max_buffer_size`; if it doesn't, **fall back to
   stochastic blur** for that export (correct, just not accelerated).
   Document the threshold. (A tiled blur buffer with halo regions is a
   later phase.)

Routing in sample-emit mode: a blur-terminated iteration emits a mean-splat
`Sample` carrying its target transform index; the scatter pass sends it to
that transform's blur histogram. Same gate, same kernels as interactive.

---

## Resolution (deferred optimization)

The design's low-res buffer (`D ≈ R/4`, bilinear upscale) is a **perf
optimization, not correctness**, and we have no supersample / downsample /
upscale infra today. v1 blurs **full-res** — correct, and it sidesteps the
D²-energy/brightness bug entirely. The low-res buffer (with the `1/D²`
intensity-unit carry on upscale, and `D = max(1, R/4)`) is a later phase
once the analytic path is proven.

---

## Phased plan

**Phase 1 — interactive, full-res, `blur` + `gaussian_blur`, golden test.**
- `Feature::AnalyticBlur` + kernel metadata on `blur`/`gaussian_blur`.
- Host eligibility + per-transform pixel-space kernel; `HAS_ANALYTIC_BLUR`
  flag; zero-allocation when no transform eligible.
- Per-transform blur buffers + mean-splat routing (direct mode only) +
  convolution-add pass.
- Golden diff test (analytic vs stochastic within noise). **Must pass.**

**Phase 2 — export / tiled.** Sample-emit routing + tagged scatter +
full-image convolve-then-add-per-tile; stochastic fallback above the size
threshold.

**Phase 3 — coverage + low-res.** More analytic-blur variations
(`circleblur`, `sineblur`, `blur_circle`, `pre_blur3D`, …); the low-res
blur buffer optimization with the D² intensity carry; 3D kernel through
projection Jacobian (depth-varying — currently gated out).

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

## Touch points (reference)

- Feature/metadata: `src/variations/definition.rs`, `src/variations/mod.rs`,
  `src/variations/defs/advanced.rs` (`blur`, `gaussian_blur`).
- Codegen + gate + flag: `src/shader_builder_v2.rs`,
  `shaders/core/main_template.wgsl` (plot section), `shaders/core/header.wgsl`.
- Buffers + passes: `src/gpu/buffers.rs`, `src/renderer/compute_kernel.rs`,
  new `shaders/blur_convolve.wgsl`.
- Export: `src/export/high_res.rs`, `shaders/core/accumulate_samples.wgsl`.
- Test: a new golden-diff test (analytic vs stochastic).
