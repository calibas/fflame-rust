# Animation Export Performance Investigation

## Problem Statement

Animation export is slower than expected. At 1080p with 1 billion iterations per frame, we're seeing ~2 FPS export speed (500ms per frame). The system doesn't appear to be GPU-bound - fans don't spin up, suggesting the GPU is idle most of the time.

**Goal:** Increase export throughput significantly (target: 10+ FPS for reasonable iteration counts).

## Current Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Render Frame   │────▶│  Copy to Buffer │────▶│  Map & Read     │
│  (GPU compute)  │     │  (GPU copy)     │     │  (CPU wait)     │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
                                                ┌─────────────────┐
                                                │  Send to FFmpeg │
                                                │  (separate thd) │
                                                └─────────────────┘
```

### Key Functions
- `export_animation_fast()` in [src/animation/export.rs](../../src/animation/export.rs) - main export loop
- `render_frame_to_completion()` - renders until max_iterations reached
- `FlameRenderer::compute_pass()` - single GPU compute dispatch
- `FlameRenderer::accumulate_pass()` - blends compute results into accumulation buffer

### Current Timing Stats (typical)
```
Render dispatch:    ~8 ms avg
Buffer map wait:  ~450 ms avg  <-- 95% of frame time
Buffer copy:       ~10 ms avg
Channel send:       ~0 ms avg
Total frame:      ~480 ms avg
Effective FPS:     ~2.08
```

## Attempts Made

### 1. FFmpeg Writer Thread (Commit 08e4e99)
**Hypothesis:** FFmpeg pipe writes were blocking the main thread.

**Changes:**
- Moved FFmpeg stdin writes to a separate thread
- Used `mpsc::sync_channel` with bounded buffer (4 frames)

**Result:** No improvement. Channel send time is ~0ms, confirming pipe writes were not the bottleneck.

### 2. Triple/Double Buffering Staging Buffers
**Hypothesis:** Buffer mapping was blocking because we're waiting for GPU to finish before we can map.

**Changes:**
- Created multiple staging buffers
- Attempted to pipeline: render to buffer N while reading from buffer N-1

**Result:** Caused "Render error: Other" crashes. The renderer's internal state doesn't support true concurrent frame rendering.

### 3. Batch Frame Rendering (10 frames)
**Hypothesis:** Batching multiple frames before reading would amortize overhead.

**Changes:**
- Render 10 frames to 10 separate staging buffers
- Map all 10 buffers at once
- Read all back together

**Result:** Same "Render error: Other" issues. The sequential `.await` on each frame meant no actual parallelism - each frame still fully completes before the next starts.

### 4. Increased Submit Batch Size (Reverted)
**Hypothesis:** Too many `queue.submit()` calls were causing GPU idle time between batches.

**Changes:**
- Batch 32 accumulate cycles (128 compute passes) per GPU submission instead of 1

**Result:** Corrupted output, no speed improvement. The accumulation logic depends on proper synchronization between passes.

## Observations

1. **GPU fans don't spin up** - Strong indicator GPU is mostly idle
2. **"Render dispatch" only measures CPU time** - The 8ms is just command buffer preparation
3. **Buffer map wait is where GPU work actually executes** - wgpu defers execution until poll
4. **Single renderer limitation** - Can't truly parallelize frame rendering with current architecture

## Hypotheses to Investigate

### H1: Command Buffer Overhead
Each compute pass creates commands that get submitted. Even with batching, there may be per-pass overhead in wgpu/driver that's not parallelizable.

**Test:** Profile with GPU profiler (RenderDoc, Nsight) to see actual GPU timeline.

### H2: Memory Bandwidth Bottleneck
The histogram buffer and accumulation textures may be causing memory bandwidth issues, not compute.

**Test:** Reduce resolution significantly and see if speed scales linearly.

### H3: Driver/wgpu Synchronization
wgpu may be inserting implicit synchronization barriers that serialize work.

**Test:** Use `wgpu::Device::poll()` with `Maintain::Wait` after large batches to see if explicit sync helps.

### H4: PCIe Transfer Bottleneck
Reading back 1080p RGBA (8MB) per frame may be limited by PCIe bandwidth.

**Test:**
- Calculate theoretical max: PCIe 3.0 x16 = 16 GB/s = 2000 frames/s for 8MB frames
- This should NOT be the bottleneck

### H5: Accumulation Pass Bottleneck
The accumulation shader may be the slow part, not compute.

**Test:** Time accumulation separately from compute passes.

## Next Steps

### 1. Add GPU Timing Queries
Use wgpu timestamp queries to measure actual GPU execution time:
```rust
let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
    ty: wgpu::QueryType::Timestamp,
    count: 2,
});
encoder.write_timestamp(&query_set, 0); // before
// ... compute work ...
encoder.write_timestamp(&query_set, 1); // after
```

### 2. Profile with External Tools
- **Windows:** Use PIX, Nsight Graphics, or RenderDoc
- Capture a frame and analyze GPU timeline
- Look for idle gaps between dispatches

### 3. Isolate Bottleneck Components
Create micro-benchmarks for:
- Pure compute passes (no accumulation)
- Pure accumulation passes (no compute)
- Buffer copy operations
- Buffer map operations

### 4. Compare with Interactive Rendering
The interactive app achieves ~60 FPS at lower iteration counts. Compare:
- What's the iterations/second in interactive mode?
- Is export actually slower per-iteration, or just doing more iterations?

### 5. Test Without Accumulation
Modify export to skip accumulation entirely (just compute + tonemap final result).
This would produce wrong output but would isolate if accumulation is the bottleneck.

## Reference: Current Constants

```rust
// render_frame_to_completion()
const NUM_WORKGROUPS: u32 = 128;
const THREADS_PER_WORKGROUP: u64 = 64;
const BATCH_SIZE: u32 = 4;  // compute passes per accumulate

// Samples per compute pass: 128 * 64 * iterations_per_thread
// With iterations_per_thread = 256: 2,097,152 samples per dispatch
// To reach 1 billion: ~477 dispatches per frame
```

## Success Criteria

- [ ] Identify the actual bottleneck with quantified GPU timing
- [ ] Achieve 5+ FPS at 1 billion iterations per frame
- [ ] OR determine theoretical maximum and document why we can't go faster
