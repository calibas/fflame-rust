# Exact accumulation for long renders

**Status:** built 2026-11-20 on branch `exact-accumulator` (both
decisions as recommended, *Decisions*); see *Built* at the end.

## Why

PR #170 gave every walk its own random stream, so a render keeps adding
new samples past a few billion iterations, and Max Iterations now goes
to 100T. The accumulators were never asked to count that far:

1. **The flame accumulator's hit count is an f32 running sum**
   (`accumulate.wgsl`: `total_density = prev.a + new_density`). Past
   2^24 a pixel's count cannot hold every integer, and each dispatch's
   addition rounds. Positive increments round the same way, so the
   error builds up. Simulated (Poisson hits a dispatch, f32 sequential
   sum against the exact count):

   | dispatches into a pixel | ~32 hits a dispatch | ~3,200 hits a dispatch |
   |---|---|---|
   | 100,000 | 0.000% | 0.000% |
   | 1,000,000 | +0.010% | -0.003% |
   | 4,000,000 | +0.23% | -2.6% |
   | 30,000,000 | -1.1% | -28% |

   A pixel hit once a dispatch freezes at 2^25 (adding 1 rounds to
   nothing). At the default 128 workgroups a dispatch is ~8.4M samples:
   1T is ~120,000 dispatches (exact), 100T ~12,000,000 (bright pixels
   drift). The governor's shorter dispatches reach the same counts sooner.
   The header's "precision wall around ~10^9 iters/pixel" and fixed EMA
   "precision-stable indefinitely" are about the colour only; the count
   drifts in every mode.

2. **The flame accumulator's colour** is a running mean updated as
   `prev * (1 - a) + new * a`. With `a` tiny, `1 - a` rounds to a
   multiple of 2^-24 and the mean drifts: simulated with an exact count,
   the mean colour is off by 4e-3 (one 8-bit level) after 3,000,000
   dispatches. Written `prev + (new - prev) * a`, the same arithmetic,
   it stays at 5e-6 to 3e-5.

3. **Tiled exports** (`HighResExporter`, ParallelTiles: the histogram
   past one storage binding -- on the web's default limits, above ~8
   megapixels) keep a u32 histogram for the WHOLE export, read back once
   at the end. A pixel's density word is 100 x its hits: it wraps past
   ~43M hits. At 100T on 10+ megapixels the bright pixels get there.
   The colour words are no larger than the density word.

4. **Solid exports' occlusion counters** (`accumulate_samples.wgsl`,
   1-in-1024 subsampled, 16 a sample, u32) wrap after ~2.7e11 samples,
   numerator and denominator both, so the survival ratio is garbage past
   that.

## The design

### Flame accumulator: an exact count, made when needed (decision 1)

- A storage buffer `accum_count`, `vec2<u32>` per pixel (lo, hi): the
  pixel's density in the histogram's own integer units (density x 100).
  `accumulate.wgsl` adds the dispatch's density word with a carry
  (`lo' = lo + n; hi' = hi + u32(lo' < lo)`), integer only, so Metal's
  fast-math cannot touch it. The `a` channel is written as
  `(f32(hi) * 2^32 + f32(lo)) / 100`: within two f32 steps of the
  count (measured: one), never accumulated.
  Everything that reads `a` (the tonemap, effects, readbacks, exports)
  is unchanged.
- **Made when a render needs it**: `FlameRenderer` counts accumulate
  passes since the buffer last held nothing (`samples_in_buffer` reset);
  at 2^18 (262,144) passes, where the f32 count is still exact, it makes
  the buffer. A zero count with a non-zero `a` means the buffer is new:
  the shader seeds it from `round(a * 100)` that pass. Until then a
  4-byte dummy is bound and a flag in `AccumulateParams` (a pad today)
  is off: ordinary renders pay nothing and stay bit-identical. Freed
  with the accumulator (resize, destroy).
- Resets: overwrite mode stores this dispatch's word alone; the solid
  depth-tightening reset zeroes it with `prev`; `clear_all` clears it
  with the textures (a count must never outlive its texture; the
  reverse is safe, it re-seeds).
- Cost: 8 B/px while it exists (17 MB at 1080p, 66 MB at 4K, ~1 GB at
  the largest single-binding export); the accumulate pass reads and
  writes 8 more bytes a pixel. Timed before and after
  (`time_ordinary_flames` covers the compute pass; the accumulate pass
  timed on its own).

### Colour: the delta form

`blended_rgb = prev.rgb + (new_color - prev.rgb) * alpha`, in every
mode. Changes the last bits of every picture's colour: the shader dumps
do not include `accumulate.wgsl`; the visual suite is expected to pass
unchanged (checked).

### Tiled exports: fold before the u32 wraps (decision 2)

- A small reduction pass finds the largest density word over the tile
  histogram's RGBD regions (one dispatch a tile, `atomicMax` into a
  4-byte result) and reads it back.
- Checked only once the samples since the last fold could exceed 2^31 /
  100 on one pixel, then every 8 dispatches; the growth between checks
  projects the next one. When the projection reaches 2^31, **fold**:
  read the RGBD words back (`read_gpu_histogram`'s copy), add them into
  a running f64 `Vec<HistogramPixel>`, and clear the RGBD regions --
  never the depth regions, which gate the scatter. The end of the export
  folds once more and uses the running sum.
- **Occlusion counters**: read and cleared at every check (8 bytes),
  summed in f64 on the host (as the CPU path's `cpu_occ_stats` does).
- Exports that never get near the limit never fold: one 4-byte readback
  every 8 dispatches past the threshold.
- To check during the build: the export's analytic-blur low-res buffer
  (u32 sums over the whole export, more hits per cell than a pixel) and
  whether the app's solid path has a whole-render occlusion counter.

## Tests

- `accumulate.wgsl` on a few pixels, its buffers written directly: the
  carry (lo near 2^32), the seed from `a`, overwrite and the depth
  reset; `a` matches the exact count to one rounding.
- Drift: one accumulate pass repeated thousands of times on a fixed
  histogram from a large seeded count: exact with the buffer, the f32
  sum visibly off without it (the test shows both).
- The pass threshold: the buffer appears at 2^18 passes, seeded; not
  before; freed on resize.
- Tiled export: a small ParallelTiles export with the fold threshold
  forced low folds several times and matches the same export unfolded.
- Lib suite, the visual suite, the WASM check; timing before and after.

## Docs

`accumulate.wgsl`'s header and the `use_fixed_ema` comments
(compute_kernel.rs, buffers.rs) corrected; RENDERER.md (the
accumulator), EXPORT.md (the fold, occlusion counters), BUFFERS.md
(the new binding).

## Built (2026-11-20)

- **Part 1** as designed. The derived `a` is within two f32 steps of the
  count (measured one: the low word, the sum and the /100 each round),
  not "one rounding" as first written. Tests in `compute_kernel.rs`
  (`exact_count_tests`): a long render's count keeps every hit (2^28
  plus 3 a pass, 1000 passes; the f32 sum shown stuck beside it), the
  carry, the threshold and seed, resets, a steady colour (the lerp form
  moves 0.3 to 0.29999983 over the same passes on the CPU).
- **Part 2** as designed, plus the blur splat buffer. Found while
  building: the in-app long-render route (`max_iterations` above 250M,
  so nearly every in-app export) takes this path as one tile -- the
  wrap was reachable from the app, not only by huge exports. Tests in
  `high_res.rs` (`fold_tests`): a 4x4 export of 1.2B samples wraps
  unfolded (the test shows it) and counts every sample folded; plain and
  analytic-blur exports folded many times match the unfolded export to
  the byte, a solid one within tolerance. The shadow buffers gained
  `COPY_DST` (the check clears the occlusion words).
- Timing: a 1080p export at 2B iterations, 2.21-2.27 s with the check on
  and off (never folds). The lib suite, the visual suite (383/383: the
  delta colour form changes no reference beyond tolerance), the WASM
  and no-default-features checks pass.

## Decisions (2026-11-20)

1. **Turning it on: automatic.** No setting; the buffer is made only
   when a render is long enough to need it; exports decide the same way.
2. **Tiled exports in the same change**: fold before the u32 wraps, by a
   reduction pass's maximum.
