# Unified render pipeline

## Goal

Collapse the three render paths (interactive `FlameRenderer`, high-res
`HighResExporter`, and dead-code `TiledRenderer`) into one. The current
arrangement creates duplicated state, divergent shader builds, and
silent cross-path bugs (the recent branch shipped a high-res export
that renders 5 pixels at 8K because of accumulated drift between the
paths). Treat the interactive renderer as the single canonical engine
and have the export entry points dispatch through it — possibly
multiple times for tiled high-res.

Hard non-goals for this branch:
- **No measurable performance regression** for the interactive path at
  any resolution where it works today. The merge must produce an
  equally-fast or faster shader and dispatch loop for sub-4K rendering.
- **No new broken-or-not-broken regressions** in the export paths
  beyond what's already broken on `main`. Anything we touch we own.

## What's actually different across paths today

| Path | Shader | Output strategy | Histogram | Init pass | Notes |
|---|---|---|---|---|---|
| `FlameRenderer` (interactive + ≤4K export + thumbnails) | `main_template.wgsl` | `atomicAdd` directly into a single full-resolution histogram buffer | One GPU buffer, `width × height × 16 B` | Yes | Driven by `compute_pass()` in a loop for interactive, called once-per-frame in `render()` for headless |
| `HighResExporter` (>4K export only) | `main_export.wgsl` | Iteration shader writes `Sample{x, y, r, g, b}` records to a flat sample buffer; CPU reads back and accumulates into a CPU histogram | RAM, unbounded | **No** (pre-existing bug on `main`) | Tonemap is GPU after histogram texture upload |
| `TiledRenderer` (dead code) | `main_tiled.wgsl` | `atomicAdd` to per-tile histograms, `tile_size² × 16 B` each | Multiple GPU buffers; sequential per-tile | N/A — never instantiated | Was advertised as the ≤4K path but the actual dispatch goes through `FlameRenderer` |

Three consumers already share a path (interactive, ≤4K export, thumbnails)
— it's `HighResExporter` that's the outlier. The user's reframing
*"normal renderer is just a tiled render with one tile"* is correct: the
only thing that fundamentally changes at higher resolutions is whether
the full-resolution histogram fits in one storage buffer.

## The real constraint that justified separate paths

`max_storage_buffer_binding_size` ≈ 128–256 MB on most GPUs. The
histogram is `width × height × 16 bytes` (4 channels × 4 bytes per
atomic u32). Threshold:

| Resolution | Histogram size | Fits a 128 MB binding? |
|---|---|---|
| 1920 × 1080 | 32 MB | ✓ |
| 3840 × 2160 (4K) | 127 MB | borderline |
| 4096 × 4096 | 268 MB | ✗ |
| 8000 × 8000 | 1 GB | ✗ |

That's the entire reason `HighResExporter` exists. There's no other
fundamental difference — the iteration math is identical.

## The merge

### Single iteration shader

`main_template.wgsl` becomes the canonical iteration shader. It already
does everything `main_export.wgsl` does *plus* histogram accumulation;
the export shader is a strict subset that swaps the histogram write
for a sample-stream write. We unify by template-gating the output
mode on the existing template processor:

```wgsl
{{#if OUTPUT_HISTOGRAM_DIRECT}}
    // Sub-4K single-tile case: atomicAdd into the full histogram.
    atomicAdd(&histogram[base_idx + 0u], r_u32);
    // ...
{{else}}
    // Multi-tile case: write to a sample buffer; a second compute
    // pass distributes samples to the right per-tile histogram.
    let sample_idx = atomicAdd(&sample_counter.count, 1u);
    samples[sample_idx] = Sample(...);
{{/if}}
```

Net effect on `main_template.wgsl` for the interactive path: zero —
when `OUTPUT_HISTOGRAM_DIRECT = true` (the default for sub-4K),
the compiled shader is byte-identical to today's. No regression.

`main_export.wgsl` and `main_tiled.wgsl` get deleted. `header_export.wgsl`
and `header_tiled.wgsl` get folded into `header.wgsl` with their unique
bindings (samples, sample_counter, tile_params) gated on the same flag.

### Sample-stream → tiled-histogram accumulator

For multi-tile rendering, add a second compute pass that consumes the
sample buffer and dispatches per-tile histogram writes. Two viable
layouts:

**A) One sample buffer + many tile histograms, accumulator strides.**
Run `iterate` to fill the sample buffer. Run `accumulate_tile` once per
tile, each dispatch reads the sample buffer and only writes samples
landing in this tile's pixel range. Repeat until total iterations done.

**B) One sample buffer + one tile histogram, sequential per-tile.**
Run `iterate` and `accumulate` for tile 0 fully; readback that tile's
histogram → tonemap → store; reset histogram; repeat for tile 1. RNG
seed is re-set per tile so each tile gets the same sample stream.

(A) is bandwidth-heavier (sample buffer read N times for N tiles) but
finishes all iteration up front and only then accumulates. (B) is
serial-per-tile but each tile is independent. (B) matches what the
current dead `TiledRenderer` was designed around. Pick (B) — it
maps onto today's `HighResExporter` chunked-readback flow with less
disruption.

### Single dispatch entry point

`FlameRenderer::compute_pass()` becomes the canonical dispatch. For
sub-4K interactive and export, it dispatches one tile (the full
canvas) with `OUTPUT_HISTOGRAM_DIRECT = true`. For >4K export, the
caller loops over tiles, swapping in tile params, with
`OUTPUT_HISTOGRAM_DIRECT = false` and the accumulate pass enabled.

`HighResExporter` shrinks to a thin wrapper around `FlameRenderer` that
manages the tile loop and histogram readback. `TiledRenderer` deletes
entirely.

### Tonemap

Single tonemap path. The `sample_density` formula is already
resolution-normalized in `FlameRenderer::tonemap_for_export` and was
recently brought into line for `HighResExporter` (last branch);
post-merge there's only one formula to maintain.

## Multi-flame rendering

Out of scope for this branch.

The user mentioned wanting to render multiple flames simultaneously.
That's largely orthogonal to the pipeline merge — it's a question of
*what* renders into a histogram (one flame's iteration loop, or
multiple flames composited), not *how many* shader+buffer paths
exist. Doing both at once would mix concerns and risk landing neither
cleanly.

Sequence the branches:
1. **This branch** — collapse the three render paths to one.
2. **Follow-up** — add multi-flame on top of the unified path.

The pipeline merge actually makes multi-flame easier: there will be
exactly one place to plumb in additional flames' bind groups,
exactly one dispatch loop to extend, exactly one tonemap composite
to write.

## Folding in the deferred issues

Each item from the prior branch's "Known issues / deferred" list maps
into this work:

1. **High-res export renders 5 pixels at 8K.** Pre-existing bug. The
   pipeline merge inherits a `HighResExporter` that's known broken at
   the relevant resolution; we don't need to fix it in place, we just
   need the tile-based replacement to actually work. The replacement
   path's correctness needs to be demonstrated by exporting at 8K
   and getting the same image as the interactive path produces at
   sub-4K (modulo iteration count).

2. **`TiledRenderer` is dead code.** Delete it after we've harvested
   anything useful from it (the tile-grid math in `src/export/tiled.rs`
   is probably reusable; `src/export/renderer.rs::TiledRenderer` itself
   is not).

3. **`PoolFinalTransform*` ConfigPath naming is transitional.** Rename
   to `FinalTransform*` and remove the legacy compat aliases. The
   compat aliases routed to `final_transforms[0]` for old animation
   tracks; we can keep that behavior by adding a one-shot migration in
   `from_string_key` that maps the legacy names to indexed paths at
   parse time. Caveats: any in-memory tracks already loaded against
   the legacy variants need re-mapping at flame load. Worth a separate
   small task within this branch.

4. **Dead `has_final_transform` and `final_transform_index` in
   `GpuParams`.** The `GpuParams` layout is now scoped to the unified
   shader by definition — if we're regenerating the bind group layout
   anyway, this is the moment to drop these fields. Saves a few bytes
   per dispatch and removes 6 stale-set sites.

## Migration plan

Incremental, performance-preserving. Each phase ends with a working
build, all existing tests passing, and benchmarks at-or-better.

**Phase 1 — establish the canonical shader.** Add the `OUTPUT_MODE`
template flag to `main_template.wgsl`. Default value matches today's
behavior. No call-site changes. Verify benchmarks unchanged.

**Phase 2 — fold `main_export.wgsl` into `main_template.wgsl`.**
Move the sample-stream output block under the new template flag.
Delete `main_export.wgsl` and `header_export.wgsl`. `HighResExporter`
now builds via `build_from_template` with `OUTPUT_MODE = SAMPLES`.
Verify high-res export produces the same image as before this branch
(may still be broken at 8K — that's expected; we haven't rewritten
the dispatch yet).

**Phase 3 — fold `main_tiled.wgsl` (delete it).** It's used by dead
code only; just remove. Same for `header_tiled.wgsl`.

**Phase 4 — accumulate pass.** Add an `accumulate_samples_to_tile`
compute shader that consumes the sample buffer and writes into a
single tile-sized histogram. This is new code. Replace the CPU
sample-accumulation loop in `HighResExporter` with this GPU pass.

**Phase 5 — tile loop.** Sequence per-tile dispatches in a wrapper
around `FlameRenderer::compute_pass()`. `HighResExporter` becomes a
thin coordinator; the interactive path is unchanged.

**Phase 6 — fix the actual 8K bug.** With the unified pipeline,
the bug should either disappear (if it was a path-specific issue) or
be reproducible in the interactive path at 8K, which makes it much
easier to diagnose. Likely candidates: sample-buffer overflow,
atomic-counter saturation, sample readback chunking off-by-one.

**Phase 7 — delete `HighResExporter` and `TiledRenderer`.** Replace
their consumers (`app/export.rs::export_headless_cpu`,
`app/config.rs::export_high_res_cpu_background`,
`animation/export.rs`) with the unified path. Remove
`src/export/renderer.rs` and the bulk of `src/export/high_res.rs`.

**Phase 8 — companion cleanups.** Rename `PoolFinalTransform*` →
`FinalTransform*` with the migration shim. Drop dead
`has_final_transform`/`final_transform_index` fields. Update docs.

## Acceptance criteria

Before merging this branch:

1. **Performance.** Benchmark suite at parity or better with `main`
   for sub-4K interactive rendering. The CSV in
   `benchmark_results/benchmark_history.csv` is the source of truth.
2. **Visual regression.** `python scripts/run_benchmarks.py` passes
   pixel-perfect hash comparison on the 8 visual test configs.
3. **High-res export works.** Manually verify 8K export of the same
   flame the user tested last branch (`output/simple3.fflame`)
   produces a non-black image with the expected fractal structure.
4. **Test suite green.** All 200+ unit tests pass.
5. **One render path remains.** `grep -r "FlameRenderer\|HighResExporter\|TiledRenderer"`
   on `src/` should find only `FlameRenderer` (or whatever the unified
   name ends up being).
6. **Shader files reduced.** Only `main_template.wgsl` and
   `header.wgsl` survive (plus the variation/affine/utility includes
   they reference). `main_export.wgsl`, `main_tiled.wgsl`,
   `header_export.wgsl`, `header_tiled.wgsl` deleted.

## Things to watch out for

- **The chaos game's sample sequence is RNG-driven.** For the
  per-tile re-render strategy (B), each tile must use the same RNG
  seed to produce the same sample sequence. Otherwise tile boundaries
  show as mismatched density. Today's `FlameRenderer` doesn't expose
  a per-dispatch seed override — we'll need to add one.

- **`samples_per_dispatch` sizing.** The current `HighResExporter`
  sizes the sample buffer for `workgroups × threads × iterations_per_thread`.
  Burn-in iterations don't emit samples, so the buffer is over-provisioned.
  But the atomic counter saturating is the suspect for the 5-pixel 8K
  bug — verify that the counter type, the buffer size, and the readback
  count all agree. May want to use a `u64` counter or chunk dispatches
  more aggressively at high resolution.

- **Tonemap at tile boundaries.** Per-tile tonemap then stitch is
  lossless if the tonemap is per-pixel (no spatial filtering). Verify
  that the curve LUT and exposure/gamma are applied per-pixel and
  don't reach across tile boundaries.

- **Init pass.** The unified path's init dispatch needs to run once
  before iteration starts, then survive across all tile dispatches
  (don't reinit between tiles — the variation_params buffer persists).

- **WASM.** WebGPU has tighter buffer limits than desktop. Verify that
  the unified path's small-tile fallback works in WASM — at minimum
  for sub-4K, since that's all WASM exports today.
