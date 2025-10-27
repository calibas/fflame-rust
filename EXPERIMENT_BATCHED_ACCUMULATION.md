# Batched Accumulation Experiment

## Branch: `experiment/batched-accumulation`
## Status: Ready for testing
## Date: 2025-10-26

## Hypothesis

**We may be optimizing the wrong bottleneck.** The real constraint might be accumulation frequency, not atomic operations.

## The Idea

Current architecture forces low `iterations_per_thread` to maintain quality (due to frequent accumulation passes). But what if we:

1. Let histogram accumulate for multiple frames (4-16 frames)
2. Process (accumulate/tonemap) less frequently
3. Increase `iterations_per_thread` to maximize GPU throughput

**Expected result:** Higher throughput without quality loss.

## Implementation

### Changes Made

1. **Added `accumulation_batch_size` field** (default: 1 = normal, 4 = batched)
2. **Modified `compute_pass`** to optionally skip histogram clear
3. **Modified render loop** to accumulate every N frames
4. **Histogram accumulates** across multiple frames before processing

### Current Configuration (Updated)

- `accumulation_batch_size = 4` (process every 4 frames)
- `iterations_per_thread = 1024` (4× increase for CLI export)
- `scale = 100` (u16 packing)

**Update:** Default iterations_per_thread increased from 256 to 1024 to properly utilize batched accumulation. Initial test with 256 showed 10% slowdown - expected since we weren't leveraging the benefit of batching.

## Testing Plan

### Step 1: ~~Baseline (Current Settings)~~ COMPLETED
Initial test:
- `batch_size = 4`
- `iterations_per_thread = 256`
- Result: 10% slower (expected - not utilizing batching benefit)

### Step 2: High Iterations Test (CURRENT)
Updated configuration:
- `batch_size = 4`
- `iterations_per_thread = 1024` (4× baseline)
- Hypothesis: 4× throughput compensates for batching overhead
- Observe: Does net performance improve vs baseline?

### Step 3: Overflow Check
Look for grey artifacts in bright areas (indicates overflow at scale=100)

### Step 4: Performance Comparison

**Metrics to track:**
- Total throughput (Giter/sec)
- Frame time (ms per frame)
- Accumulate frequency vs quality
- Any visual artifacts

## What to Look For

### Success Indicators ✅
1. **Higher throughput** with high `iterations_per_thread`
2. **Same visual quality** as baseline
3. **No overflow artifacts** (no grey areas)
4. **FPS not dropping** significantly

### Failure Indicators ❌
1. Quality degradation (chunky, banding)
2. Overflow artifacts (grey noise in bright areas)
3. No throughput improvement
4. Stuttering or frame drops

## Expected Outcomes

### Optimistic Scenario
- 2-4× throughput increase with high `iterations_per_thread`
- Perfect quality maintained
- Proves accumulation frequency was the bottleneck

### Realistic Scenario
- 20-50% throughput increase
- Minor quality differences (acceptable)
- Some overflow at extreme densities

### Pessimistic Scenario
- No improvement or worse performance
- Quality loss unacceptable
- Proves atomic operations ARE the bottleneck

## How to Test

### Run the App
```bash
cargo run --release
```

### Observe Metrics
- Watch FPS counter
- Check total iterations counter
- Note any visual artifacts

### Compare Configurations

**Configuration A (Normal):**
```rust
accumulation_batch_size: 1
iterations_per_thread: 256
```

**Configuration B (Batched):**
```rust
accumulation_batch_size: 4
iterations_per_thread: 256
```

**Configuration C (Batched + High Iters):**
```rust
accumulation_batch_size: 4
iterations_per_thread: 1024
```

### Visual Quality Check
Load complex presets:
- Check bright areas for grey noise
- Check edges for banding/stepping
- Compare side-by-side with main branch

## Next Steps Based on Results

### If Successful
1. Fine-tune batch size (8? 16?)
2. Add UI control for batch_size
3. Document overflow limits for different scales
4. Merge to main with feature flag

### If Mixed Results
1. Profile to find actual bottleneck
2. Consider hybrid approach
3. Make batch_size configurable

### If Unsuccessful
1. Document findings
2. Revert to main
3. Accept current performance as optimal
4. Focus on other optimizations

## Files Modified

- `src/app/mod.rs` - Added batch_size field and batched logic
- `src/renderer/compute_kernel.rs` - Added clear_histogram parameter
- `src/app/export.rs` - Updated compute_pass call
- `docs/DEEPER_HISTOGRAM_PROPOSAL.md` - Full analysis

## Reverting

To go back to normal behavior:
```bash
git checkout main
```

Or change in code:
```rust
accumulation_batch_size: 1
```

---

**Let's see if we're optimizing the right thing!** 🧪
