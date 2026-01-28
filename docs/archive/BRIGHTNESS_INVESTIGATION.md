# Brightness Investigation - Progressive Rendering

**Goal:** Achieve constant brightness throughout progressive rendering, matching Apophysis behavior.

**Problem:** Brightness changes as iterations accumulate, making it difficult to judge final result during rendering.

## Key Formula Components

The Apophysis brightness formula has two density inputs:

```rust
k2 = 1 / (area × white_level × sample_density)
ls = (k1 × log10(1 + white_level × bucket_count × k2)) / (white_level × bucket_count)
```

- **sample_density**: Sets k2 coefficient scale (brightness curve calibration)
- **bucket_count**: Actual per-pixel accumulated hits (varies per pixel)

## Experiments Tried

### Attempt 1: Using total_iterations (Current - Commit 1d2f954)
```rust
base_density = self.total_iterations / pixel_area
sample_density = base_density × 2^(2×zoom)
```

**Behavior:**
- ❌ Starts very bright when exiting overwrite mode (low iterations → tiny sample_density → huge k2)
- ❌ Gradually darkens as iterations accumulate
- ❌ Brightness flash on transition from preview

**Why:** sample_density grows over time → k2 shrinks → less brightness boost → image darkens

---

### Attempt 2: Using max_iterations
```rust
base_density = max_iterations / pixel_area
sample_density = base_density × 2^(2×zoom)
```

**Behavior:**
- ✅ No brightness flash
- ❌ Starts too dark (using target density before it's accumulated)
- ❌ Gradually brightens as actual density catches up to reference
- ❌ Brightness still changing throughout render

**Why:** sample_density is constant (correct!) but too high for early frames where actual bucket_count is low

---

### Attempt 3: Using max(total_iterations, 50% threshold)
```rust
threshold = max(max_iterations / 2, 10M)
effective = max(total_iterations, threshold)
base_density = effective / pixel_area
sample_density = base_density × 2^(2×zoom)
```

**Behavior:**
- ⚠️ First half: Image gradually brightens (using threshold, actual density accumulating)
- ⚠️ Second half: Image gradually darkens (using total_iterations, which grows)
- ❌ Brightness changes throughout entire render
- ❌ Wasting iterations with incorrect brightness

**Why:** sample_density changes at 50% mark → two different brightness curves

---

## Analysis

The core issue: **Apophysis does single-pass rendering** with known total iterations, so sample_density is constant. **We do progressive rendering** where actual density builds up gradually.

Three conflicting requirements:
1. Need stable sample_density for constant brightness (suggests using max_iterations)
2. Need actual density reflected in brightness (suggests using total_iterations)
3. Need to avoid bright flash at low iterations (suggests using threshold)

## Hypothesis to Test

The brightness formula might be fundamentally designed for single-pass rendering. For progressive rendering, we may need to:

1. **Decouple sample_density from accumulated density**
   - Use a fixed reference (max_iterations or user-specified target)
   - Accept that early frames will be darker (they have less actual data!)

2. **Or: Scale brightness by accumulation progress**
   - Apply multiplier based on total_iterations / max_iterations
   - Compensate for lower actual density in early frames

3. **Or: Use exposure compensation**
   - Keep sample_density = max_iterations (constant)
   - Apply temporary exposure boost in early frames
   - Fade out exposure boost as iterations accumulate

## Current Status

Using Attempt 1 (total_iterations) as baseline.

**Answer:** Apophysis only displays **after completion**. When brightness formula runs, `total_iterations == max_iterations`. No progressive rendering issue.

## The Real Problem

We're doing **real-time progressive rendering** which Apophysis doesn't support. Need to define "correct" brightness during accumulation.

## Attempt 4: Track effective_iterations (Commits 896a472, c635138)
```rust
// During accumulation (not overwrite):
effective_iterations += samples_this_frame

// When exiting overwrite (reset_iteration_counter):
effective_iterations = 0

// For brightness:
base_density = effective_iterations.max(1M) / pixel_area
```

**Behavior:**
- ❌ Much too bright at first (effective_iterations starts at 0)
- ❌ Gradually approaches correct brightness at end of iterations
- ❌ Still getting bright flash at beginning

**Why:** Low effective_iterations → low sample_density → high k2 → excessive brightness

---

## Current Understanding

The brightness formula is fundamentally incompatible with progressive rendering where density builds gradually. All attempts fail because:

1. **Low iterations early** → formula assumes low density → brightens image
2. **High iterations late** → formula assumes high density → darkens image
3. **But actual buffer density grows continuously** → mismatch at all times

**The real issue:** `sample_density` in k2 is meant to be a **constant reference** for calibration, but `bucket_count` (actual pixel hits) grows over time. When they don't match, brightness is wrong.

**Possible solution:** Use max_iterations as fixed reference (constant brightness curve), but that makes early frames genuinely darker because they have less actual data. Need to determine: is that the correct behavior?

---

## Attempt 5: Using iterations_per_frame (Commit 098789b)
```rust
const NUM_WORKGROUPS: u32 = 128;
let iterations_per_frame = (NUM_WORKGROUPS * iterations_per_thread * batch_size) as f32;
let base_density = iterations_per_frame / pixel_area;
sample_density = base_density;
```

**Behavior:**
- ✅ Consistent brightness frame-to-frame (no drift over time)
- ✅ Works in both overwrite and normal modes
- ❌ Brightness changes when adjusting iterations_per_thread slider
- ❌ Lower iterations_per_thread makes image darker (wrong!)

**Why:** iterations_per_thread is a performance knob (how to chunk work), not a visual parameter. Making brightness depend on it couples appearance to performance settings.

**Problem:** The logarithmic formula is sublinear. When both bucket_count and sample_density scale proportionally with iterations_per_thread, the brightness doesn't scale proportionally due to the log10() term.

---

## Attempt 6: Using effective_iterations
```rust
let base_density = if self.effective_iterations > 0 && pixel_area > 0.0 {
    self.effective_iterations as f32 / pixel_area
} else {
    1.0  // First frame fallback
};
sample_density = base_density;
```

**Behavior:**
- ⚠️ Still brightness changes with iterations_per_thread (slightly better than Attempt 5)
- ❌ Early frames much darker, late frames much brighter
- ❌ Units mismatch: sample_density in "iterations/pixel", bucket_count in "hits/pixel"

**Why it fails:**
- effective_iterations counts ALL iterations (including misses and opacity failures)
- bucket_count counts only HITS that land on pixels
- For typical fractals, only 10-30% of iterations result in hits
- Example: 10M iterations → 800×600 = 20.8 iterations/pixel, but only ~2-6 actual hits/pixel
- Formula compares apples (iterations) to oranges (hits)

---

## Attempt 7: Fixed reference value
```rust
let sample_density = 50.0;  // Fixed constant
```

**Behavior:**
- ❌ Still brightness changes with iterations_per_thread
- Works better but not perfect

**Why it fails:**
- bucket_count growth rate actually DOES depend on iterations_per_thread
- More iterations per frame → more hits per frame → faster bucket_count growth
- Fixed sample_density doesn't account for this variable growth rate

---

## THE ACTUAL FIX: Normalized reference value (Current)
```rust
let sample_density = 5000.0 * (iterations_per_thread as f32 / 256.0);
```

**Behavior:**
- ✅ Brightness completely independent of iterations_per_thread slider adjustments
- ✅ Consistent brightness when changing performance settings
- ✅ Zoom compensation works correctly (via area calculation)
- ✅ Brightness only depends on exposure/gamma (user controls)

**Why this is correct:**
- bucket_count accumulation rate scales with iterations_per_thread
- sample_density must scale proportionally to maintain constant ratio
- Normalized to default (256): sample_density scales as (iterations_per_thread / 256.0)
- Base value 5000.0 chosen empirically (~100x higher than Apophysis due to batch rendering)
- At default (256): 5000.0 × 1.0 = 5000.0
- At half (128): 5000.0 × 0.5 = 2500.0
- At double (512): 5000.0 × 2.0 = 10000.0

**Key insight:** Both bucket_count and sample_density must scale together with iterations_per_thread to maintain a constant ratio. The normalization factor (iterations_per_thread / 256.0) ensures they scale proportionally, making brightness independent of the performance setting.
