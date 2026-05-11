# Parallel-tile GPU accumulate (between binding-size and VRAM-size)

## Goal

Fill the resolution band where the histogram exceeds one storage-buffer
binding but still fits VRAM. Today these renders fall to
`HighResExporter`'s CPU-histogram path; this branch wires the GPU
ParallelTiles dispatch that the prior `accumulator-unification` branch
stubbed out in `pick_strategy` but never actually implemented.

## Context

After the `accumulator-unification` branch (merged), the render-path
landscape is:

| Strategy | Iter cost | Histogram location | Currently wired? |
|---|---|---|---|
| **Direct** | 1× | One GPU buffer (one binding) | ✅ FlameRenderer |
| **ParallelTiles** | 1× | N GPU buffers, all bound to accumulate dispatch | ❌ Falls through to HighResExporter CPU path |
| **SerialTiles** | N× | One tile-sized GPU buffer at a time, re-iterate per tile | ❌ Not implemented |
| **HighResExporter CPU** | 1× | Sample stream on GPU, histogram in main RAM | ✅ Fallback for everything not-Direct |

`pick_strategy(width, height, &Limits)` returns `Direct |
ParallelTiles | SerialTiles` correctly. `HighResExporter::new`
allocates a GPU histogram only when the strategy is `Direct`;
otherwise it leaves the GPU buffer unallocated and the export loop
falls back to CPU readback + rayon-parallel histogram fill.

After the previous branch's Phase 8d/8e routing changes,
`HighResExporter` is *only* invoked when the histogram exceeds the
adapter's actual binding size — so the `Direct` branch inside
`HighResExporter::new` is now unreachable. Every call into
`HighResExporter` runs the CPU path.

On the user's hardware (2 GB binding, ~10 GB VRAM):
- ≤ 8K (≤ 1 GB hist): FlameRenderer Direct
- 12K → ~32K: HighResExporter CPU (this branch's target)
- 32K+ : HighResExporter CPU (would remain so even with Strategy A)

## Decision: build A, skip B, keep HighResExporter

The original `accumulator-unification` plan ended with "delete
HighResExporter once Phase 6 (SerialTiles GPU wiring) lands." That
framing was wrong. SerialTiles costs N× iteration time — for a
16-tile render, 16× wall time vs HighResExporter's 1× iter + CPU
accumulate. The CPU path is genuinely better than SerialTiles for
its regime; deleting it in favor of SerialTiles would regress
performance.

The right end state is *two* canonical paths:

1. **GPU all the way** when the histogram fits VRAM (Direct or
   ParallelTiles). Iterations run once, accumulate is GPU atomic
   add, no CPU readback in the hot loop.
2. **GPU iterate + CPU histogram** when the histogram exceeds VRAM
   (HighResExporter as it exists today). Iterations still run once;
   accumulation goes through main RAM because GPU memory isn't
   sufficient.

SerialTiles is the third theoretical option (re-iterate per tile)
but it loses to option 2 on both axes — iteration cost and
implementation complexity. **Don't build it.**

## What ParallelTiles needs to do

Inputs: a render whose histogram size is between
`max_storage_buffer_binding_size` and total available VRAM.

Plan:
1. Compute tile layout. Tile size = largest histogram chunk that
   fits one binding. Tile count = `ceil(total / binding_size)`.
   Horizontal tile slicing (full image width, varying tile height)
   preserves the row-major flat layout the rest of the pipeline
   already speaks.
2. Allocate N GPU buffers, one per tile. Each ≤ binding-size.
   Total ≤ VRAM.
3. Per iterate dispatch:
   - Iterate runs as today, writing samples to the sample stream
     buffer (a single binding's worth — sample count is bounded
     by the dispatch-size choice, not by image resolution).
   - Read the sample counter back to host (4 bytes, cheap).
   - Run **one** accumulate dispatch that binds all N tile
     histograms at once. Each thread reads one sample, computes
     which tile its `(x, y)` falls into, atomic-adds into that
     tile's binding.
   - Reset sample counter.
4. After total iterations target reached: read back each tile
   histogram, stitch on host into a single flat
   `Vec<HistogramPixel>`, hand to the existing GPU tonemap path.

Iteration cost: 1× (same as Direct). The only extra cost vs Direct
is the accumulate dispatch reading more bindings, which is a small
constant per-sample cost.

### Shader work

The existing `shaders/core/accumulate_samples.wgsl` writes to a
single histogram binding. For ParallelTiles it needs to write to
one of N tile bindings based on the sample's coordinate. Two ways:

**Option A — N storage buffer bindings, shader switches on tile index.**
The shader has `var<storage, read_write> tile_0, tile_1, ..., tile_N`
and computes `tile_idx = pixel_y / tile_height`, then a switch
dispatches the atomic adds to the right binding.

Pros: clean WGSL, each binding stays within
`max_storage_buffer_binding_size`.

Cons: shader bindings are baked in at compile time. Generating
shader source for N tiles per dispatch (with template gating) is
manageable but adds complexity. Caps at
`max_storage_buffers_per_shader_stage` — typically 8 on desktop
hardware. So at most 8 tiles per dispatch.

**Option B — single big concatenated buffer, shader computes flat offset.**
One buffer of total histogram size, bound as a single big binding,
shader writes to `histogram[tile_idx * tile_pixels + local_pixel_idx]`.

Pros: simplest shader, no template gating.

Cons: requires total histogram to fit one binding. Defeats the whole
purpose. So this only works for the case Direct already handles.

**Recommended: Option A, with the 8-tile-per-dispatch cap as a
hard upper limit.**
For renders that need more than 8 tiles, run multiple accumulate
dispatches per iterate, each binding a different subset of tiles.
Samples landing outside the bound tiles in a given dispatch are
silently dropped (the shader's bounds check already handles this).
Wasted work, but acceptable given the cost amortization vs CPU
fallback.

Alternative implementation: keep the existing single-binding
accumulate shader (`accumulate_samples.wgsl`), but call it N times
per iterate batch, each call binding a different sub-range of the
big concatenated buffer with `BufferBinding { offset, size }`. The
shader's existing `bound_x / bound_y / bound_width / bound_height`
filter drops samples that don't belong to this tile. Same N
dispatches per iterate, simpler shader. Trade-off: each dispatch
scans the full sample stream.

I'd recommend this second approach. It reuses the existing shader
verbatim and the per-dispatch overhead is dominated by the iterate
cost anyway.

### Host-side work

In `HighResExporter` (or a successor module):
1. At construction: if `pick_strategy == ParallelTiles { tiles_x: 1, tiles_y, tile_width, tile_height }`,
   allocate the concatenated histogram buffer (total size, single
   buffer if `≤ max_buffer_size`; otherwise multiple buffers — but
   this case probably belongs in HighResExporter CPU path anyway).
2. In the export loop: per iterate dispatch, run `tiles_y`
   accumulate dispatches (or one big dispatch with all tile
   bindings, depending on chosen option). Reset sample counter
   each cycle.
3. At end of iteration: readback the concatenated buffer to host
   as a contiguous `Vec<u8>`, cast to `&[u32]`, walk and convert
   to `Vec<HistogramPixel>` for the existing tonemap.

The CPU stitching step is roughly the same cost as today's CPU
histogram accumulation, but it runs *once* at the end instead of
per iterate dispatch. Should be a big wall-time win for the
multi-tile resolution band.

### Routing changes

`pick_strategy` already returns `ParallelTiles` correctly. The
routing in `app/export.rs::export_headless` and
`app/config.rs::export_high_res_cpu_background` would update to:

```rust
let strategy = pick_strategy(width, height, &device_limits);
match strategy {
    Direct => route_to_flame_renderer(),
    ParallelTiles { .. } => route_to_high_res_exporter_parallel(),  // new
    SerialTiles { .. } => route_to_high_res_exporter_cpu(),         // current "everything else"
}
```

Actually probably cleaner: the runtime check becomes "does the
histogram fit VRAM, however split across buffers?" rather than
"does it fit one binding?". Need to think about how to probe
VRAM size — `Limits::max_buffer_size` is a per-buffer cap, not
total VRAM. Might need to estimate or accept "we tried, it
failed, fall back."

## Why HighResExporter stays

| | GPU all-the-way | HighResExporter CPU |
|---|---|---|
| Iter cost | 1× | 1× |
| Histogram storage | VRAM | Main RAM (effectively unbounded) |
| Per-dispatch overhead | None | Sample readback + CPU scatter |
| Wall time at 8K | Fast | Slow (CPU readback dominates) |
| Wall time at 32K when histogram doesn't fit VRAM | N/A — can't run | Works |

GPU is faster when memory permits. CPU is the only option when it
doesn't. Both have legitimate domains.

Reference impls back this up:
- **flam3** (CPU-only) is the canonical batch tool.
- **Ember/Fractorium** prefer GPU but explicitly fall back to CPU
  density estimation when the requested resolution exceeds GPU
  capacity.
- **JWildfire** runs entirely on CPU for its high-quality render
  modes; GPU mode has lower resolution limits.

The "delete HighResExporter" goal from the previous branch's plan
was overreach driven by an aesthetic preference for one path,
ignoring the legitimate dual-regime tradeoff. After this branch,
HighResExporter is the >VRAM fallback only — its small-resolution
code path (already dead since the routing changes in Phase 8d)
can be deleted.

## Risks and open questions

1. **VRAM probing.** WebGPU doesn't expose total VRAM directly.
   We can probe `max_buffer_size` (per-buffer cap) but not "how
   many buffers can I allocate." Crude approach: try to allocate
   the histogram, on failure fall back to CPU path. Better: ship
   conservative tile count heuristics (e.g. assume 4 GB usable
   VRAM) and let the user override.

2. **Sample-stream size at high res.** The sample stream is sized
   to hold one iterate dispatch's worth of samples (currently
   ~128 MB). At very high tile counts (many small tiles), the
   stream might fill faster than it can drain through accumulate.
   Mitigation: scale dispatch size down with tile count.

3. **Atomic contention.** Single big histogram buffer with all
   threads atomic-adding could contend on hot pixels. Per-tile
   bindings spread this out (each tile is its own atomic domain).
   Empirical measurement at high resolution is the right gate
   here.

4. **WASM.** WebGPU on browsers caps binding size at 128 MB
   (sometimes lower). ParallelTiles would fire at any 4K+ export
   on WASM. The CPU fallback path still works there; need to
   verify the routing decides correctly.

5. **Mismatch with `RenderStrategy::ParallelTiles` enum shape.**
   The current enum carries `tiles_x, tiles_y, tile_width,
   tile_height`. With multi-buffer (>1 storage buffer for the
   concatenated histogram), we might need additional fields like
   `buffers_used, tiles_per_buffer`. Not load-bearing yet —
   sort out at implementation time.

## Phased rollout

Implementation order, each phase ending with a working build and
tests passing:

1. **P1 — Concatenated tile-histogram buffer.** Allocate one big
   GPU buffer when ParallelTiles strategy is chosen. Verify size
   limits, error path for VRAM exhaustion.
2. **P2 — Accumulate dispatch loop.** Per iterate dispatch, run N
   accumulate dispatches each binding a sub-range of the
   concatenated buffer. Verify samples land in the right tile.
3. **P3 — Readback + stitch.** End-of-iteration readback of the
   full concatenated buffer, walk into `Vec<HistogramPixel>`,
   hand to existing tonemap.
4. **P4 — Routing.** Update `app/export.rs` and `app/config.rs`
   to route ParallelTiles distinct from SerialTiles (which stays
   as CPU fallback alias).
5. **P5 — Validation.** 12K and 16K visual comparison against
   pre-branch CPU output. Should be brightness-equivalent (same
   tonemap, same accumulator semantics). Wall time should be
   notably faster.
6. **P6 — Cleanup.** Delete the dead Direct branch in
   `HighResExporter::new`. Drop the dead `RenderStrategy::SerialTiles`
   variant if we're committing to "skip SerialTiles" (or keep
   it documented as a future fallback for >VRAM cases on devices
   without enough main RAM, e.g. some embedded scenarios).

## Acceptance criteria

1. 12K bubble-3d render through ParallelTiles produces output
   visually equivalent to the same render through HighResExporter
   CPU path, faster wall time.
2. 8K still routes through FlameRenderer Direct (no regression).
3. >VRAM renders still fall to HighResExporter CPU (no regression
   on the truly-extreme case).
4. All existing unit tests pass.
5. Visual regression suite at parity.

## Out of scope

- SerialTiles GPU wiring. Skipped per the analysis above —
  HighResExporter CPU path is strictly better in the regime where
  SerialTiles would apply.
- Full `HighResExporter` deletion. It earns its place as the
  >VRAM fallback. The dead small-resolution and dead Direct
  code paths inside it can go; the CPU-histogram path stays.
- Multi-flame rendering. Orthogonal to this work, mentioned in
  the original unified-render-pipeline doc as a follow-on.
