# Deeper Histogram Architecture - Proposal

## Date: 2025-10-26
## Status: 🤔 PROPOSED - Needs validation

---

## Core Insight

**Current assumption:** Atomic operations are the bottleneck.
**User's insight:** We might be optimizing the wrong thing - the real constraint is accumulation frequency.

## The Real Problem

From `ITERATIONS_PER_THREAD_QUALITY.md`:

> Higher `iterations_per_thread` causes 60-70% quality degradation due to fewer accumulation passes.

**Root cause:** Not the compute speed, but **how often we process the histogram**.

### Current Architecture

```
Every frame (60 FPS):
  Compute (256 iters/thread) → Histogram → Accumulate → Tonemap → Display
  └─ Clear histogram ─────────────────────┘
```

**Constraints:**
- Histogram cleared every frame
- Accumulation happens every frame
- High `iterations_per_thread` = fewer accumulation passes = quality loss

**Current workaround (speed multiplier):**
- Run more frames per second to get more accumulation passes
- Maintains quality but increases overhead

## Proposed Architecture: Deeper Histogram

### Concept: Multi-Frame Histogram Accumulation

```
Frame 1-N (high FPS):
  Compute (4096 iters/thread) → Histogram (accumulates)
  └─ DON'T clear histogram ──────┘

Every N frames (e.g., every 16 frames):
  Histogram → Accumulate → Tonemap → Display
  └─ Clear histogram after processing ──┘
```

### Benefits

1. **Higher throughput:** Increase `iterations_per_thread` to 4096+ without quality loss
2. **Less processing overhead:** Accumulate/tonemap only every N frames
3. **Same quality:** Total accumulation passes unchanged
4. **Deeper queue:** Histogram acts as accumulation buffer

### Example Throughput Calculation

**Current (256 iters/thread):**
```
128 workgroups × 64 threads × 256 iters = 2,097,152 iters/frame
At 60 FPS = 125.8 million iters/sec
```

**Proposed (4096 iters/thread, process every 16 frames):**
```
128 workgroups × 64 threads × 4096 iters = 33,554,432 iters/frame
At 960 FPS (16× speed) = 32.2 BILLION iters/sec (256× faster!)
Process every 16 frames = effective 60 FPS display rate
```

**Wait, that math doesn't work out...**

Let me recalculate more carefully:

**Current:**
- 256 iters/thread at 60 FPS display
- 2.1M iters/frame × 60 = 125.8M iters/sec

**Proposed (same total throughput):**
- 4096 iters/thread (16× more per dispatch)
- Need fewer dispatches: 60 FPS ÷ 16 = 3.75 dispatches/sec?
- That's way too slow!

**Actually, the insight is different:**

If we can let the histogram accumulate for longer, we can:
1. Run compute passes at max GPU speed (no display sync)
2. Accumulate less frequently (reduce overhead)
3. Still get smooth quality (same total accumulation passes)

## Key Questions

### 1. Histogram Overflow

**Current:** u16 scale=100, cleared every frame
- Max hits at full brightness: 655
- At 2M iters/frame, ~0.1% hit same pixel → ~2000 hits
- **Already overflows!**

**Wait, does it overflow?** Let me calculate viewport hit probability:
```
1920×1080 = 2,073,600 pixels
2M iterations / 2M pixels = 1 hit per pixel average
But fractals cluster → some pixels get 100+ hits, most get 0
```

**For deeper histogram:** Need higher capacity
- u16 scale=100: 655 hits max
- u16 scale=10: 6,553 hits max
- u32 separate atomics: 4.2 billion hits max (no overflow!)

**Tradeoff:**
- Lower scale = less precision but more capacity
- OR go back to 4× u32 atomics for unlimited capacity

### 2. Memory Bandwidth

**Current:** 2× u32 per pixel = 16 MB @ 1080p
- 2 atomic writes per hit
- 2M hits/frame × 2 atomics = 4M atomic ops/frame

**With deeper accumulation:**
- Same buffer size (16 MB)
- More total hits before clear: 2M × N frames
- More atomic ops: 4M × N ops before process

**Is this the bottleneck?**
- Current: 4M atomics in 16ms = 250M atomics/sec
- GPU atomic bandwidth: Likely 1-10 billion atomics/sec
- **Not the bottleneck yet**

### 3. Accumulation Pass Overhead

**Hypothesis:** Accumulate/tonemap passes are expensive, reducing their frequency helps.

**Current cost per frame:**
```
Compute pass: ~80% of frame time
Accumulate pass: ~10% of frame time
Tonemap pass: ~10% of frame time
```

**If we accumulate every 16 frames:**
- 16 frames of pure compute (fast)
- 1 frame with accumulate+tonemap (slower)
- Average frame time: 0.8×16 + 1.0×1 / 17 = ~0.81× (23% faster?)

**But does this help?**
- We still need to display at 60 FPS
- Can't skip display frames (user sees nothing)
- Unless we decouple compute from display...

## Alternative Architecture: Async Compute Queue

### Decoupled Compute and Display

```
Compute Thread (max GPU speed):
  Loop:
    Compute (4096 iters/thread) → Histogram (accumulates)
    If histogram full or timer expired:
      Signal "ready for processing"

Display Thread (60 FPS):
  Wait for "ready for processing"
  Histogram → Accumulate → Tonemap → Display
  Clear histogram
  Resume compute thread
```

**Benefits:**
- Compute runs at max GPU speed (no vsync)
- Display runs at comfortable 60 FPS
- Histogram depth controls queue size

**Complexity:**
- Requires async/multithreading
- GPU scheduling complexity
- Synchronization between compute and display

## Simpler Approach: Batched Accumulation

### Keep Current Architecture, Process Every N Frames

```rust
// In render loop
fn render_frame() {
    // Always run compute pass
    run_compute_pass(iterations_per_thread: 4096);

    frame_count += 1;

    // Only accumulate/display every N frames
    if frame_count % accumulation_interval == 0 {
        run_accumulate_pass();
        run_tonemap_pass();
        display();
        clear_histogram();
    }
}
```

**Problem:** User sees frozen screen for (N-1) frames

**Solution:** Show previous frame while computing
```rust
if frame_count % accumulation_interval == 0 {
    run_accumulate_pass();
    run_tonemap_pass();
    clear_histogram();
}
// Always display the last rendered frame
display_previous_frame();
```

## Overflow Analysis for Deeper Histogram

### Current Scale=100 Capacity

**Single frame (256 iters/thread):**
- 2.1M iterations total
- Average 1 hit per pixel
- Clustered distribution: Some pixels 100+ hits
- Max accumulated value: ~100 hits × scale=100 = 10,000 (within u16)

**16 frames accumulated (4096 iters/thread):**
- 33.5M iterations total per 16-frame batch
- Average 16 hits per pixel
- Clustered distribution: Some pixels 1600+ hits
- Max accumulated value: ~1600 hits × scale=100 = 160,000 ❌ **OVERFLOW!**

**Solution 1: Lower scale**
- scale=10: Max value = 16,000 (within u16=65535) ✅
- Precision: 10% quantization (might be visible)

**Solution 2: Use u32 atomics**
- Back to 4× u32 per pixel = 31 MB
- Max value = 4.2 billion (no overflow)
- Precision: can use scale=10000 (0.01% quantization)

**Solution 3: Detect and saturate**
- Check for overflow in shader
- Clamp to u16 max (65535)
- Lose some precision in brightest pixels
- Most pixels fine

## Performance Prediction

### Current (u16 packed, 256 iters/thread)
- 5335ms for 40B iterations
- 7.46 Giter/sec
- Process every frame

### Proposed (batched, 4096 iters/thread, process every 16)

**Optimistic:**
- Compute: 16× more iters/thread = 16× faster dispatch
- Less frequent accumulate/tonemap = 20% time saving
- **Predicted:** ~7 Giter/sec × 1.2 × 16 = 134 Giter/sec?

**Realistic:**
- Higher iters/thread has diminishing returns (memory access patterns)
- Atomic contention increases with more hits before clear
- **Predicted:** ~10-20 Giter/sec (30-170% faster)

**Pessimistic:**
- Overflow issues cause quality loss
- Atomic contention becomes bottleneck
- **Predicted:** Similar or worse than current

## Recommendation

### Step 1: Measure Current Bottleneck

Profile to understand where time is spent:
```
Compute pass: ??ms
Accumulate pass: ??ms
Tonemap pass: ??ms
GPU sync overhead: ??ms
```

### Step 2: Test Batched Accumulation (Simple)

Modify render loop:
```rust
// Accumulate every 4 frames instead of every frame
if frame_count % 4 == 0 {
    accumulate_and_display();
}
```

Measure:
- Does throughput increase?
- Does quality stay the same?
- Do we hit overflow?

### Step 3: Adjust Based on Results

If successful:
- Increase batch size (8, 16 frames)
- Tune `iterations_per_thread`
- Adjust scale to prevent overflow

If unsuccessful:
- Current architecture is optimal
- Accept 7.46 Giter/sec as good enough

## Open Questions

1. **What is the actual bottleneck?**
   - Atomic writes?
   - Memory bandwidth?
   - Accumulate/tonemap overhead?
   - GPU dispatch overhead?

2. **How much do fractals cluster?**
   - Average hits per pixel in hot spots?
   - Affects overflow predictions

3. **Is display sync the constraint?**
   - Are we GPU-bound or vsync-bound?
   - Can we decouple compute from display?

4. **What's the minimum accumulation frequency for quality?**
   - How infrequent can we make accumulation?
   - Does sqrt() artifact depend on absolute frequency or relative?

## Next Steps

1. **Profile current rendering** to identify bottleneck
2. **Test simple batched accumulation** (4 frames)
3. **Measure overflow** in real fractals
4. **Compare throughput** and quality
5. **Decide if complexity is worth the gain**

---

**Bottom line:** You're right that we might be optimizing the wrong thing. The key insight is that higher `iterations_per_thread` isn't inherently bad - it's only bad if we don't process the histogram frequently enough. By batching compute and decoupling from display frequency, we might unlock significantly higher throughput.

Let's test it!
