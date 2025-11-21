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
