# Accumulator unification (Phase 8 of the unified-render-pipeline merge)

## Goal

Collapse the last remaining split between the interactive renderer and
high-res export into a single canonical accumulator format. Today both
paths share the iteration shader (Phase 2 of the prior branch); they
diverge at the accumulate pass, which keeps `HighResExporter` alive as
a parallel code path. This branch closes that gap so headless export
can route through `FlameRenderer.render()` directly and
`HighResExporter` can be deleted.

The previous branch deferred this as Phase 8 with a documented blocker:
the two paths' accumulators store the same conceptual data (per-pixel
average color + density count) at *different scales*, and the tonemap
formulas are calibrated against those scales — so a naïve routing
change under-exposes export images. This branch is that blocker's
follow-up.

## Where we are

**Interactive (`shaders/accumulate.wgsl`):**
- Color: EMA of recent batch averages. Per accumulate pass:
  `blended_rgb = prev * (1 - α) + (r_sum / density) * α` where α is
  `blend_factor` (default 0.1, optionally exponential-decay).
- Density (alpha channel): additive but *scaled* —
  `prev.a + density * 0.01 * blend_factor`. Each batch's density
  contribution is multiplied by the same blend_factor that's smoothing
  color, so the absolute density-buffer scale depends on
  `blend_factor × frame_count`.
- Tonemap reads color directly and density via `sample_density` param,
  whose constants are tuned for this scale.

**Export (`HighResExporter::tonemap_gpu`):**
- Color: cumulative raw sum — `pixel.r += sample.r` per emitted sample
  (not per batch).
- Density: cumulative count — `pixel.count += 1` per sample.
- Tonemap reads `pixel.r / pixel.count` for averaged color and
  `pixel.count` for density.

The tonemap *formula* in both paths uses the same density/exposure/log
math, but its `sample_density` calibration parameter differs by a
blend-factor-and-frame-count factor that doesn't have a clean closed
form. That's why route-swapping in the previous branch produced a
noticeably dim result.

## What the field does

Research into reference fractal flame renderers (full sources cited
in commit messages):

| Program | Accumulator | Preview vs final |
|---|---|---|
| flam3 (Draves, C) | Cumulative `(R, G, B, density)`, int → double | Single path (batch only) |
| Apophysis 7X | Cumulative `TBucket { Red, Green, Blue, Count }` | Same accumulator; preview is a separate `TRenderer` instance at lower quality |
| **Fractorium / Ember** (C++/OpenCL) | Cumulative `tvec4<float>` everywhere | **Same accumulator.** Distinction is operational, not structural |
| JWildfire (Java) | Cumulative `RasterPoint { red, green, blue, count }` | Same accumulator; interactive updater tonemaps the live raster directly |
| cuburn (CUDA) | Cumulative 4×float32 | Single path (batch animations) |
| Chaotica (commercial) | Cumulative (per public docs) | Progressive — F5 retonemaps the running buffer |
| Draves & Reckase paper | Cumulative `(R, G, B, A)`; final = `color_sum / hit_count` then log-scaled | Paper does not address live preview |

**Field consensus is unambiguous: one cumulative-add accumulator,
shared between preview and final.** The EMA-blend approach in our
interactive path is a one-off — every reference impl uses cumulative
sums.

The seam between "interactive" and "final" is managed in three places,
**none of which is the accumulator**:

1. **Iteration scheduling.** Preview iterates fewer samples per UI
   tick or runs at lower quality.
2. **Density-estimation filter.** Fractorium toggles between cheap
   "log scaling" (preview) and expensive "Full DE" (final). Same
   accumulator, different post-processing.
3. **Parameter-change response.** Reset and re-iterate; never blend
   the buffer across parameter sets.

## Why our current EMA approach exists

Probably an inherited UX choice — running averages give smooth
fade-ins during chaos game iterations. The downsides we now hit:

- The accumulator scale depends on `blend_factor` and frame count,
  forcing the tonemap to compensate via magic constants
  (`5000.0 * (iterations_per_thread / 256.0) * (reference_pixels / total_pixels)`).
- That same scale-dependence blocks the pipeline merge.
- The visual benefit is small: in steady state, cumulative
  `r_sum / count` converges to the same answer as the EMA, just on a
  different brightness ramp. The "smoothness" the EMA provides between
  parameter changes is mostly redundant with our existing 100ms
  overwrite window that already resets on parameter change.

## The cautionary note from flam3

flam3's tonemap constants `k1` and `k2` embed sample count:

```
k2 = oversample² · nbatches / (contrast · area · 255 · sample_density · sumfilt)
```

Brightness *drifts as samples accumulate* unless the tonemap is
scale-invariant. Fractorium inherits this formula and the same
sensitivity. Renderers that want stable preview brightness as samples
grow (Chaotica, JWildfire's interactive mode) instead normalize
*inside* the tonemap — divide by current density — so the absolute
accumulator scale doesn't matter.

That's the calibration discipline we'd need: **make the tonemap
scale-invariant in the accumulator**, then the format doesn't matter.

## Plan

Five phases, ordered. 8a is the prerequisite for everything else;
8d and 8e are mechanical once 8a–c work. Each phase ends with a
working build, all tests passing, visual parity with `main` on the
benchmark suite.

### Phase 8a — make the tonemap scale-invariant

Audit `update_tonemap_params`, `tonemap_for_export`, and
`tonemap.wgsl`. Find every place that depends on absolute accumulator
scale (any formula multiplying density by a hardcoded magic number) and
rewrite to depend on a ratio instead. Concrete proposals to evaluate:

- `sample_density` becomes `density / iteration_count` per pixel —
  dimensionally correct (samples per pixel), scale-invariant.
- Or: `sample_density` becomes `density / max_density_in_image` —
  matches flam3's `α[x][y] = log(freq) / log(freq_max)` formulation.
- Or: directly mirror Fractorium's normalization formula (research
  pending — see open question below).

Acceptance: the same flame renders identically at iteration counts
spanning 4-5 orders of magnitude (e.g. 1M, 10M, 100M, 1B samples)
with no exposure compensation needed. **Today this is not true** —
brightness drifts up as samples accumulate, which is why we have
the EMA-blend "smoothing it out."

### Phase 8b — switch interactive accumulate to cumulative

Replace the EMA in `accumulate.wgsl`:

- Color: `r_accum += r_sum / density × density = r_sum` — i.e.
  cumulative raw sum (matches HighResExporter's CPU path).
- Density (alpha): `a_accum += density` — drop the `* 0.01 * blend_factor`
  scaling.

The `convergence_gate` for `target_iterations_per_pixel` keeps
working (gates writes regardless of accumulation strategy). The
`overwrite_mode` reset-on-change still works — it just clears the
buffer instead of replacing the EMA target.

Risks:
- "Smooth color convergence" feel in interactive may be missed.
  Mitigation: keep `overwrite_mode` for slider drag (already does
  full reset within 100ms window).
- `blend_factor` UI control becomes meaningless. Either remove it or
  retask it as "samples per accumulate dispatch" (a quality knob
  rather than a smoothing knob).

### Phase 8c — verify `UpdateType` paths cover the responsiveness story

Slider-drag scenarios after 8a + 8b:
- **Tonemap-only changes** (gamma, exposure, palette rotation, levels):
  `UpdateType::ToneMap` → re-tonemap existing buffer, don't re-iterate.
  Already implemented; should feel *better* under cumulative (no
  EMA's lag).
- **Color-mode changes** (palette index, color speed): `UpdateType::Color`
  → may need re-iteration if color flow differs. Verify.
- **Flame changes** (transform params, variations, weights):
  `UpdateType::Flame` → reset + re-iterate. Same as today.
- **View changes** (zoom, pan, rotation): `UpdateType::View` →
  reset + re-iterate.

Acceptance: tonemap-only sliders feel as fast or faster than today;
flame-changing sliders feel about the same (since they already reset).

### Phase 8d — route headless export through `FlameRenderer.render()`

`app::export::export_headless` calls `pick_strategy` against the
adapter's actual binding limit (the device-creation already requests
adapter limits as of Phase 5). When `Direct`, route to
`FlameRenderer.render()` directly via the unified `render()` API.
When tiled, fall back to `HighResExporter` until Phase 6 adds
SerialTiles GPU wiring.

This is the routing change that the previous branch reverted — but
now the accumulator scales match, so the underexposure regression
won't reproduce.

### Phase 8e — delete `HighResExporter`

`HighResExporter`'s only remaining job is the CPU-fallback case
(histogram exceeds binding size). Two options:

1. **Defer Phase 6 first.** Wire SerialTiles to GPU accumulate,
   delete `HighResExporter` outright. Single canonical path for
   all resolutions on all devices.
2. **Fold the CPU fallback into a small wrapper** around
   `FlameRenderer.render()`. Tile-loop sequences per-tile dispatches,
   reads back, stitches. Drops most of the existing 1300-LOC
   `HighResExporter` (init pipeline, separate device, separate
   tonemap pipeline, etc. — all redundant after 8d).

Option 2 is incremental; option 1 closes the loop more cleanly. Decide
based on how Phase 6 looks after 8a–c land.

## Risks

| Risk | Mitigation |
|---|---|
| Tonemap recalibration shifts brightness on every existing flame | Pixel-perfect comparison via the visual regression harness (`scripts/run_benchmarks.py`) on all 8 test configs |
| EMA's color-smoothing feel is missed in interactive | Keep `overwrite_mode` reset window; potentially add a debug toggle to A/B compare during evaluation |
| `target_iterations_per_pixel` thresholds were tuned against EMA scale | Audit existing config defaults; may need to scale by typical sample/density ratio |
| `sample_density` constant is referenced from many places | Centralize the formula in one helper before changing it |
| WASM has tighter binding limits — different routing decisions than desktop | Verify Phase 8d's `pick_strategy` works on WASM; CPU fallback (or Phase 6) covers the rest |

## How Ember solves it (Phase 8a research result)

**Ember recomputes K2 every preview frame against the *current*
sample count.** From `Source/Ember/Renderer.cpp:618–636` (the live
preview branch, `forceOutput == true`):

```cpp
T quality = (static_cast<T>(m_Stats.m_Iters) / static_cast<T>(FinalDimensions()))
            * (m_Scale * m_Scale);
m_K2 = static_cast<bucketT>((Supersample() * Supersample())
        / (area * quality * m_TemporalFilter->SumFilt()));
```

with the standard flam3 log applied at line 935:

```cpp
const bucketT logScale = (m_K1 * std::log(1 + m_HistBuckets[i].a * m_K2)) / m_HistBuckets[i].a;
```

The trick is that `quality` is **the live iterations-per-pixel**:
`m_Stats.m_Iters / FinalDimensions()` (sum of all threads' iterations
divided by pixel count, summed cumulatively from the start). The
inline comment is explicit: *"the normal calculation of K2 ... will
scale the colors to be very dark. Correct it by pretending the number
of iters done is the exact quality desired."*

The math: per-pixel `density.a` grows linearly with iters-per-pixel
(it's literally a count of hits at that pixel). `K2 ∝ 1 /
iters-per-pixel` shrinks linearly. Their product `density × K2` is
scale-invariant — both factors absorb iters-per-pixel and it
cancels. Therefore `log(1 + density × K2)` stabilizes as samples
accumulate.

This is option (c) from the planning options above: same formula,
recompute K2 every frame.

## What our tonemap currently does

`shaders/tonemap.wgsl:255–266`:

```wgsl
let k2 = 1.0 / (contrast * tonemap_params.area * tonemap_params.white_level * tonemap_params.sample_density);
let log10_value = log(1.0 + tonemap_params.white_level * count * k2) / log(10.0);
return (k1 * log10_value) / (tonemap_params.white_level * count);
```

Same shape as Ember. The bug is in how `sample_density` is computed
host-side. `compute_kernel.rs::update_tonemap:1244`:

```rust
let mut sample_density = 5000.0
    * (iterations_per_thread as f32 / 256.0)
    * (reference_pixels / total_pixels);
```

This is a **fixed calibration constant** — it doesn't depend on
`total_iterations`. `iterations_per_thread` is the per-frame batch
size (compile-time-ish), not the running total. So `sample_density`
stays the same as iterations accumulate, which means `k2` stays the
same, which means `density × k2` keeps growing as density grows —
brightness drifts.

(`update_tonemap_params:871` does have
`sample_density = total_iterations / area` for the simpler entry
point, but that uses *fractal-space area* rather than *pixel count*
as the denominator, so it has the wrong dimensions. And it's not the
formula the live interactive path uses.)

## Concrete fix for Phase 8a

Replace the magic-number `sample_density` with Ember's formula:

```rust
// Track total_iters cumulatively (we already do — `self.total_iterations` in
// FlameRenderer, also passed via `result.total_iterations` in the headless
// render API).
let pixel_count = (width as f32) * (height as f32);
let iters_per_pixel = (total_iterations as f32) / pixel_count;
// Guard against zero before any iteration has run.
let sample_density = iters_per_pixel.max(1.0);
```

Then `k2 = 1 / (area × white_level × sample_density)` recomputes
naturally as `total_iterations` grows, `density × k2` stays
scale-invariant, and brightness no longer drifts with sample count.

The EMA-blend in interactive accumulate exists *because of* this
brightness drift — it papers over the tonemap mis-calibration by
keeping the displayed buffer in a steady-state ratio. Once the
tonemap is scale-invariant, the EMA isn't needed for stability and
we can switch to cumulative add (Phase 8b).

### Audit checklist for 8a

Before flipping the formula, find every `sample_density` write site
and decide whether it should:
- Use the new running-iters formula (interactive + headless export
  during iteration)
- Use a fixed target value (one-shot batch with known total iter
  count, where `iters_per_pixel = config.max_iterations / pixel_count`
  is the final, not running, value)

Sites to check (`grep -n "sample_density" src/`):
- `compute_kernel.rs::update_tonemap_params` — currently
  `total_iterations / area`. Wrong dimensions; replace with
  `total_iterations / pixel_count`.
- `compute_kernel.rs::tonemap_for_export` — currently the magic
  `5000.0 * (iters_per_thread / 256) * (ref_pixels / total_pixels)`
  formula. Replace with running-iters.
- `compute_kernel.rs::update_tonemap` — same magic-number formula
  (with optional live-preview /8 divisor). Replace with running-iters
  (drop the `/8` once EMA is gone — it was compensating for the
  EMA's slower convergence).
- `compute_kernel.rs::set_transparent_mode` — same magic formula.
  Replace.
- `export/high_res.rs::tonemap_gpu` — same magic formula. Replace
  (and feed total_iters from the export's iteration loop, which
  already has it as `total_samples_accumulated`).

After 8a, every site reads the same simple formula:
`sample_density = max(total_iters / pixel_count, 1.0)`. The "magic
number" tunings (5000.0, 256, reference_pixels = 1M) all disappear.

### Risks to flag for 8a

- `area` already varies with zoom (`area = pixel_count /
  (pixels_per_unit_zoomed)²`). The Ember formula's
  `1 / (area × quality)` therefore correctly handles zoom-dependent
  brightness. Our existing formula does too — so this risk is
  contained, but **verify**: render the same flame at 1× and 4× zoom,
  confirm brightness is equivalent.
- `density_scale` config knob: still applies as a multiplier on the
  alpha pre-tonemap (controls transparency level for transparent
  PNG export). Not a calibration knob; orthogonal.
- `brightness`, `contrast`, `gamma`, `prefilter_white` user-facing
  knobs continue to live in `k1`. Only `k2`'s composition changes.
- `iterations_per_thread` previously appeared in `sample_density`
  (the `/256` factor). After the change it disappears — `iterations_per_thread`
  will only affect render speed, not displayed brightness. **This was
  already the *stated* invariant** in the existing code's giant
  comment block (`compute_kernel.rs:1226–1235`) — the comment promises
  brightness is independent of `iterations_per_thread`, but the
  formula doesn't actually deliver that promise (sample_density is
  scaled by `iterations_per_thread / 256`, which is silly given that
  density itself scales by the per-frame iteration count). Phase 8a
  brings the formula in line with the comment.
- Pixel-perfect visual regression on the 8 test configs is the
  acceptance gate. Brightness *will* change slightly because today's
  formula is mis-calibrated; the question is whether the new one
  produces images closer to or further from "what the user expects."

### What flam3, Apophysis, and JWildfire do (sanity check)

All three use the same `k1·log(1 + density·k2)/density` formula.
flam3 computes `k2` once before render with the *target* total
sample count (it's a batch tool — no preview drift to worry about).
Apophysis 7X recomputes K2 each preview pass with the current
quality (same approach as Ember). JWildfire's interactive mode
recomputes per-frame using its own `quality` running counter.

The pattern is universal: **K2 is a dynamic value, not a calibration
constant, and it's recomputed against running sample count.**

## Outcome of 8a research

Phase 8a is a **focused, low-risk change** with one load-bearing
edit (the `sample_density` formula) propagated to ~5 call sites.
The math is well-understood (Ember-equivalent). The acceptance gate
is the existing visual regression suite. Expect to spend most of the
implementation time on visual A/B comparison, not on the formula
itself.

## Acceptance criteria

1. Same flame at 1M, 10M, 100M, 1B sample counts produces identical
   brightness/contrast images (modulo Monte Carlo noise).
2. Visual regression suite passes on all 8 test configs.
3. Headless export at any resolution that fits one storage-buffer
   binding routes through `FlameRenderer.render()`.
4. `HighResExporter` is deleted (or shrunk to a thin tile-loop
   wrapper, depending on 8e choice).
5. Interactive slider drag feels at least as responsive as today on
   the configs the user exercises.
6. All unit tests pass.
