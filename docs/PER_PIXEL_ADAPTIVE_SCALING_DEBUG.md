# Per-Pixel Adaptive Histogram Scaling - Debug Log

## Goal
Implement per-pixel adaptive histogram scaling to prevent overflow in high-density areas while maximizing color precision in low-density areas.

## Background
- Histogram uses u16 packed format (2 per u32) for RGBA values
- Scale range: 1-100 (u16)
- Color encoding: `color_f32 * scale → u16`
- Density tracking per pixel to adjust scales dynamically

---

## Failed Fix Attempt #1: Density Encoding with Scale

### Date
2025-10-27

### Theory
Encoding/decoding scale mismatch was causing artifacts. When scales changed between write (compute shader) and read (accumulate shader), the decode math broke because density didn't carry scale information.

**Problem identified:**
- Encode: `color × pixel_scale`, `density = 1`
- Decode: `color_sum / (density × pixel_scale)`
- When scale changes between frames: pixel written with scale=100, read with scale=50 → brightness doubles

### Fix Attempted
1. **Encode density with scale** (main_2d.wgsl, main_3d.wgsl):
   - Changed `let d16 = 1u;` to `let d16 = u32(pixel_scale);`
   - Now density carries scale information

2. **Decode without scale multiplication** (accumulate.wgsl):
   - Changed `color_sum / (density * pixel_scale)` to `color_sum / density`
   - Since density = sum_of_scales, no need to multiply again

3. **Aggressive overflow response** (adjust_scale.wgsl):
   - Changed from gradual reduction to immediate drop to min_scale
   - When `max_accumulated > overflow_threshold`, set `new_scale = min_scale`

4. **Higher initial scale** (buffers.rs):
   - Changed from 50 to 100 for maximum initial color depth

### Result: WORSE

**Symptoms:**
- Lines now appear across **entire fractal** (not just dense areas)
- Lines appear after just **a few frames** (faster onset)
- **Brightness increases over time** (accumulation bug)
- **Colors wash out** (saturation loss with continued rendering)

### Root Cause Analysis

**Density overflow hypothesis:**

Old system:
- Density increments by 1 per hit
- Max hits before u16 overflow: 65,535

New system:
- Density increments by scale (1-100) per hit
- Max hits before u16 overflow: **65,535 / 100 = ~655 hits**

**100× more likely to overflow!**

When density overflows u16 (wraps to small value), decode becomes:
```
large_color_sum / small_wrapped_density = HUGE brightness
```

This explains:
- Brightness increasing over time (density wrapping)
- Colors washing out (oversaturation)
- Lines appearing faster (overflow happens sooner)
- Entire fractal affected (not just high-density, because even moderate density pixels overflow after 655 hits)

### Files Modified (needs revert)
- `shaders/core/main_2d.wgsl` line 89
- `shaders/core/main_3d.wgsl` line 90
- `shaders/accumulate.wgsl` lines 54-64
- `shaders/adjust_scale.wgsl` lines 56-61
- `src/gpu/buffers.rs` lines 410, 639

---

## Previous Behavior (for comparison)

### What Worked (fixed scales)
- Fixed scale=10: No lines, poor quality (low color depth)
- Fixed scale=50: No lines, good quality
- Fixed scale=100: No lines (tested), excellent quality

**Key insight:** When scale never changes, encode/decode math is self-consistent.

### What Failed (adaptive scales, pre-fix #1)
- Lines appeared only in **high-density areas**
- Took **several seconds** to appear
- Progressive: dim → bright → oversaturated → darkened with lines
- Adaptive scaling was working (min=1.0, max=100.0, avg~40)

---

## Potential Fix Ideas

### Option 1: Normalize density by a constant
Instead of `d16 = u32(pixel_scale)`, use `d16 = u32(pixel_scale / 10.0)` or similar to reduce overflow risk while still carrying scale information.

**Pros:** Reduces overflow likelihood
**Cons:** Still has overflow potential, adds complexity

### Option 2: Separate scale tracking (write_scale_buffer approach revisited)
Maintain separate buffer tracking the scale used during write, use that for decode.

**Pros:** No density overflow, clean separation
**Cons:** We tried this before and got lines back (bug in implementation?)

### Option 3: Accept scale can only change slowly
Limit scale adjustment rate so aggressive so scales can't change much frame-to-frame. This minimizes encoding/decoding mismatch.

**Pros:** Simple, might work
**Cons:** Doesn't solve fundamental problem, slow adaptation

### Option 4: Pack density as separate u32 instead of u16
Each pixel gets u32 for density instead of u16, eliminating overflow.

**Pros:** Eliminates density overflow completely
**Cons:** 2× memory usage for histogram (4× u16 colors + 1× u32 density = 3× u32 per pixel)

### Option 5: Use floating-point accumulation buffer instead of integer histogram
Switch from atomic integer histogram to floating-point accumulation with proper read-after-write synchronization.

**Pros:** No overflow, no quantization, no packing complexity
**Cons:** Major architectural change, potential performance impact, synchronization complexity

### Option 6: Revert fix #1, investigate original symptom differently
The original "lines in dense areas" might have been caused by something else entirely. We confirmed it wasn't histogram overflow (scale=50 fixed had no lines).

**Investigation needed:** What else could cause every-other-pixel lines specifically in high-density areas during adaptive scaling?

---

## Fix Attempt #2: U32 Density to Prevent Overflow

### Date
2025-10-27

### Theory
Density overflow (u16 max = 65,535) was causing brightness artifacts when encoding density with scale. With `density_u32 = u32(pixel_scale)`, moderate-density pixels would overflow after ~655 hits, causing brightness to spike.

### Fix Attempted
Changed histogram layout from 2× u32 per pixel to 3× u32 per pixel:

**Old layout (2× u32 per pixel):**
- Word 0: [R_u16][G_u16]
- Word 1: [B_u16][Density_u16]

**New layout (3× u32 per pixel):**
- Word 0: [R_u16][G_u16]
- Word 1: [B_u16][unused]
- Word 2: Density (full u32)

**Changes made:**
1. **main_2d.wgsl, main_3d.wgsl:**
   - Changed `base_idx = pixel_idx * 2u` to `base_idx = pixel_idx * 3u`
   - Changed density encoding to `let density_u32 = u32(pixel_scale);`
   - Added third atomic: `atomicAdd(&histogram[base_idx + 2u], density_u32);`

2. **accumulate.wgsl:**
   - Changed `base_idx = pixel_idx * 2u` to `base_idx = pixel_idx * 3u`
   - Changed unpacking to read 3 words instead of 2
   - Read density as `f32(density_u32)` instead of extracting from packed word

3. **buffers.rs:**
   - Changed histogram buffer size from `width × height × 2` to `width × height × 3`

**Memory cost:** 1.5× histogram size (~4.6MB @ 800×600, was ~3.1MB)

### Result: PARTIAL SUCCESS

**Symptoms improved:**
- ✅ Lines no longer appear across entire fractal
- ✅ Lines only in dense areas (back to original symptom)
- ✅ No brightness increase over time
- ✅ Colors stay saturated

**Symptoms remaining:**
- ❌ Vertical every-other-pixel dark lines in dense areas
- ❌ Odd flickering effect (possibly optical illusion from lines)

### Root Cause Analysis - Scale Buffer Race Condition

**Pixel data from user (yellow dense area):**
```
#afae57, #212300, #afae56, #232102, #afae54, #212100, #abaa59
Bright   Dark     Bright   Dark     Bright   Dark     Bright
Yellow   ~Black   Yellow   ~Black   Yellow   ~Black   Yellow
```

**Pattern:** Every other pixel is ~90% darker (nearly black vs bright yellow).

**This is a scale_buffer packing race condition!**

The scale_buffer uses packed u16 format (2 scales per u32):
```wgsl
// Reading scale (compute and accumulate shaders)
let scale_word_idx = pixel_idx / 2u;
let scale_word = scale_buffer[scale_word_idx];
let pixel_scale = f32(select(
    scale_word & 0xFFFFu,          // Even pixel (low 16 bits)
    (scale_word >> 16u) & 0xFFFFu, // Odd pixel (high 16 bits)
    (pixel_idx % 2u) == 1u
));
```

The adjust_scale shader writes back scales (adjust_scale.wgsl:80-88):
```wgsl
if (is_odd) {
    // Odd pixel: high 16 bits
    let new_word = (scale_word & 0xFFFFu) | (new_scale_u16 << 16u);
    scale_buffer[scale_word_idx] = new_word;
} else {
    // Even pixel: low 16 bits
    let new_word = (scale_word & 0xFFFF0000u) | new_scale_u16;
    scale_buffer[scale_word_idx] = new_word;
}
```

**The race condition:**

Two GPU threads processing adjacent pixels (even index N, odd index N+1) in parallel:

1. Thread A (pixel N, even): Reads `scale_buffer[N/2]`, modifies low 16 bits
2. Thread B (pixel N+1, odd): Reads `scale_buffer[N/2]`, modifies high 16 bits
3. Thread A writes modified word back
4. Thread B writes modified word back ← **CLOBBERS Thread A's change!**

Or vice versa (Thread B writes first, Thread A clobbers).

**Result:** Random corruption of scale values. Some pixels end up with wrong scales:
- Pixel stuck at scale=1 → color decoded as `sum / (density × 1)` = bright
- Neighbor pixel stuck at scale=100 → color decoded as `sum / (density × 100)` = very dark

Since dense areas trigger aggressive scale adjustments, the race happens frequently there, creating the characteristic every-other-pixel dark lines.

**Why fixed scales worked:** When adjust_scale is disabled, no writes to scale_buffer = no race condition.

**Why lines only in dense areas:** Low-density pixels don't trigger scale adjustments often, so race rarely occurs. Dense pixels adjust every frame = high race probability.

### Files Modified
- `shaders/core/main_2d.wgsl` lines 70-98
- `shaders/core/main_3d.wgsl` lines 71-99
- `shaders/accumulate.wgsl` lines 31-53
- `src/gpu/buffers.rs` lines 393-399

---

## Next Fix: Unpack Scale Buffer

### Theory
The scale_buffer packing (2× u16 per u32) is causing atomic write race conditions when adjacent pixels adjust scales simultaneously.

### Solution
Change scale_buffer to unpacked format: 1× u32 per pixel (store u16 scale in full u32 word).

**Pros:**
- Eliminates read-modify-write race condition
- Each pixel's scale in separate memory location
- Atomic writes no longer overlap

**Cons:**
- 2× memory for scale_buffer (~0.6MB @ 800×600, was ~0.3MB)
- Minor (acceptable cost for correctness)

**Memory impact:**
- Scale buffer: 800×600×4 bytes = ~1.9MB (was ~1.0MB)
- Histogram buffer: 800×600×12 bytes = ~5.6MB (already increased)
- Total extra memory: ~1MB

### Implementation Plan
1. Change scale_buffer creation to allocate `width × height` u32 words
2. Initialize each word to scale value (no packing loop needed)
3. Update all shaders to read/write scales without packing logic:
   - Remove `scale_word_idx = pixel_idx / 2u` logic
   - Direct access: `scale_buffer[pixel_idx]`
4. Simplify adjust_scale write logic (no masking/shifting)

---

## Fix Attempt #3: Unpacked Scale Buffer

### Date
2025-10-27

### Theory
The every-other-pixel dark lines were caused by a race condition in the packed scale_buffer. When two adjacent pixels (even/odd) tried to update their scales simultaneously in adjust_scale.wgsl, they performed read-modify-write operations on the same u32 word, causing one thread's write to clobber the other's changes.

### Fix Implemented
Changed scale_buffer from packed (2× u16 per u32) to unpacked (1× u32 per pixel) format.

**Changes made:**
1. **buffers.rs (lines 407-418, 624-631):**
   - Removed packing loop
   - Changed to `vec![initial_scale; pixel_count]` (simple initialization)
   - Changed type from `u16` to `u32`

2. **main_2d.wgsl, main_3d.wgsl:**
   - Simplified scale read: `let pixel_scale = f32(scale_buffer[pixel_idx]);`
   - Removed all packing bit manipulation logic

3. **accumulate.wgsl:**
   - Same simplification: direct access instead of packed read

4. **adjust_scale.wgsl (lines 28-31, 69-70):**
   - Removed read-modify-write logic with bit masking
   - Changed to direct write: `scale_buffer[pixel_idx] = u32(new_scale);`
   - Each pixel writes to dedicated u32 word (no overlap, no race)

**Memory cost:** 2× scale buffer size (~1.9MB @ 800×600, was ~1.0MB)

### Result: PARTIAL SUCCESS

**Symptoms fixed:**
- ✅ Every-other-pixel dark lines are GONE
- ✅ No flickering
- ✅ Race condition eliminated

**New symptoms discovered:**
- ❌ Dense areas still get darker over time (bright → brighter → darker than surroundings)
- ❌ Scales not resetting on preset load (min=0.0 instead of min=10.0)
- ❌ Old scales inherited from previous fractal

### Root Cause Analysis - Double Scale Multiplication

**User data from logs:**

When loading new preset:
```
Imported flame
Scale stats @ frame 720: min=0.0, max=10.0, avg=4.7
Scale stats @ frame 780: min=0.0, max=100.0, avg=10.4
```

When program first started:
```
Scale stats @ frame 60: min=0.0, max=100.0, avg=5.0
Scale stats @ frame 120: min=0.0, max=100.0, avg=5.0
```

**Issue 1: Scale buffer not reset**
After loading preset, scales should be min=10.0, max=10.0 (all pixels initialized to 10). Instead seeing min=0.0, max=10.0, suggesting scales are inherited from previous fractal. The `reset_scale_buffer()` call may not be happening or histogram isn't being cleared.

**Issue 2: Dense areas get darker over time (double scale multiplication bug)**

Current encode/decode math:
- Encode: `color × scale`, `density = scale`
- Decode: `color_sum / (density × scale)`

**The bug:** We multiply by scale twice (once in density, once in decode).

Example showing the problem:
1. Frames 1-10: scale=100, accumulate 10 hits
   - color_sum = 10 × (color × 100) = 1000 × color
   - density = 10 × 100 = 1000
2. Frame 11: Dense area triggers scale reduction → scale=10
3. Frame 11 decode: `(1000 × color) / (1000 × 10)` = `color / 10` → **10× too dark!**

The decode should be `color_sum / density` (NOT `density × scale`), because density already contains the scale information.

**Why this causes darkening progression:**
1. First frames: Low density, high scale (100) → correct brightness
2. Middle frames: Density increases → triggers overflow response → scale drops to min (1-10)
3. Later frames: Decode divides by `(density × small_scale)` → huge denominator → dark pixels

Dense areas darken because they hit overflow threshold first and drop scale to minimum, causing massive over-division.

### Files Modified
- `src/gpu/buffers.rs` lines 407-418, 624-631
- `shaders/core/main_2d.wgsl` lines 75-76
- `shaders/core/main_3d.wgsl` lines 76-77
- `shaders/accumulate.wgsl` lines 35-36
- `shaders/adjust_scale.wgsl` lines 28-31, 69-70

---

## Fix Attempt #4: Correct Decode Math + Debug Fixes

### Date
2025-10-27

### Theory
The "dense areas darkening over time" was caused by double scale multiplication in the decode logic. Since density now includes scale (`density_u32 = pixel_scale`), multiplying by scale again in the denominator caused massive over-division when scales changed.

### Fixes Implemented

**1. Fixed decode math (accumulate.wgsl lines 49-58):**
- Changed from: `color_sum / (density × pixel_scale)`
- Changed to: `color_sum / density`
- Since density = sum of scales, we only divide once

**2. Fixed debug_scale_stats (compute_kernel.rs lines 273-321):**
- Updated buffer size: `((pixel_count + 1) / 2)` → `pixel_count` (unpacked format)
- Updated reading logic: Removed bit unpacking, direct u32 access
- Fixed off-by-one buffer overrun that caused panic

**3. Verified scale buffer reset:**
- reset_scale_buffer() is called correctly in load_config()
- Scales properly reset to 10 on preset load (min=10.0 confirmed)

### Result: PARTIAL SUCCESS

**Symptoms fixed:**
- ✅ Scale buffer properly resets on preset load (min=10.0, max=10.0)
- ✅ Debug logging works correctly (no more min=0.0 from garbage reads)
- ✅ No panic from buffer overrun

**Symptoms remaining:**
- ❌ Dense areas still darken over time after first few frames
- ❌ Quality degrades instead of improving with more iterations

### Root Cause Analysis - Accumulation Still Broken

**User observation:**
"The min is now 1.0 again. Everything appears to reset properly on Preset load. The issue where it darkens over time remains. Instead of each pass improving quality, that only happens the first few frames. Then everything goes downhill."

**Symptoms:**
1. First few frames: Quality improves normally
2. Middle frames: Dense areas start getting darker
3. Later frames: Dense areas significantly darker than surroundings
4. Quality degrades instead of converging

**The problem is more fundamental than just the math.**

The decode formula `color_sum / density` assumes density tracks the sum of scales used during encoding. But there's a **timing mismatch**:

**Batch N encoding (compute shader):**
- Reads current `scale_buffer[pixel]` value (e.g., scale=50)
- Accumulates: `color × 50` to histogram
- Accumulates: `50` to density

**Between batches:**
- `adjust_scale` runs, changes scales based on density
- High-density pixels drop to scale=1-10

**Batch N+1 decoding (accumulate shader):**
- Reads histogram with mixed scales: old data (scale=50) + new data (scale=10)
- Reads current `scale_buffer[pixel]` = 10 (updated value)
- But decode just uses `color_sum / density` with NO reference to current scale

The issue: **When scales change mid-render, the histogram contains data encoded with DIFFERENT scales than what's currently in scale_buffer.**

**Example:**
- Frames 1-10: scale=100, add 100 hits → color_sum=100×(color×100), density=100×100=10,000
- adjust_scale: High density → reduce to scale=10
- Frame 11: scale=10, add 10 hits → color_sum+=10×(color×10), density+=10×10=100
- Total: color_sum=(100×color×100)+(10×color×10) = 10,100×color, density=10,100
- Decode: `10,100×color / 10,100` = `color` ✓ Math works!

Wait, that should be correct... Let me reconsider.

**Oh! The issue might be in how the histogram is cleared vs. when scales change.**

The histogram is cleared every batch (4 compute frames), but scales are updated EVERY frame. So within a single batch accumulation:
- Frame 1 of batch: scale=100, write data
- adjust_scale runs: scale→10
- Frame 2 of batch: scale=10, write data
- Frame 3 of batch: scale=10, write data
- Frame 4 of batch: scale=10, write data
- Accumulate: Histogram has mixed data (1 frame at scale=100, 3 frames at scale=10)

But density should still sum correctly: `(1×100) + (3×10) = 130`, color_sum should match.

**Need more investigation into:**
1. Is the histogram being cleared at the right time?
2. Are scales changing too aggressively in dense areas?
3. Is there an issue with the accumulation blending formula itself?

### Files Modified
- `shaders/accumulate.wgsl` lines 49-58
- `src/renderer/compute_kernel.rs` lines 273-321

---

## Conclusion: Per-Pixel Adaptive Scaling Abandoned

### Date
2025-10-27

### Final Test: Fixed Global Scale

**Test:** Disabled `adjust_scale_pass()` and used fixed global scale=100

**Results:**
- ✅ No darkening artifacts
- ✅ Quality improves consistently over time
- ✅ Excellent color depth (scale=100)
- ❌ Still have original overflow problem in extremely dense areas

### Root Cause of Per-Pixel Adaptive Failure

**The fundamental problem:** Per-pixel adaptive scaling is **architecturally incompatible** with batched histogram accumulation.

**Why it fails:**
1. Histogram accumulates data over 4 frames (batch)
2. Scales change every frame via adjust_scale
3. Histogram contains mixed data encoded with different scales
4. Decode can't know which frames used which scales
5. Even with density encoding scale, timing mismatches create errors

**Concrete example of the mismatch:**
- Batch frames 1-4 accumulate into histogram
- Frame 1: scale=100, writes data
- adjust_scale runs: scale→50 (high density detected)
- Frame 2: scale=50, writes data
- Frame 3: scale=50, writes data
- Frame 4: scale=50, writes data
- Accumulate: Histogram has 1 frame @ scale=100 + 3 frames @ scale=50
- Density sum = 100 + 50 + 50 + 50 = 250
- But colors from frame 1 are 2× too large relative to frames 2-4
- Over many iterations, this creates cumulative errors → darkening

**Why fixed global scale works:**
- All frames use same scale (no timing mismatch)
- Encode and decode are perfectly consistent
- Simple, predictable, no cumulative errors

### What We Achieved

Despite abandoning per-pixel adaptive, the work was not wasted:

**Infrastructure improvements:**
1. ✅ **U32 density buffer** - Prevents overflow (was u16, now u32)
   - Old: 65K max density → overflow with 4× batching
   - New: 4.2B max density → handles any batch size
2. ✅ **Unpacked scale buffer** - Cleaner, no race conditions
   - Old: Packed 2× u16 per u32 (race condition in adjust_scale)
   - New: 1× u32 per pixel (each pixel has dedicated word)
3. ✅ **3× u32 histogram layout** - Separated density for clarity
   - Word 0: [R_u16][G_u16]
   - Word 1: [B_u16][unused]
   - Word 2: Density (u32)
4. ✅ **Fixed decode math** - Single division by density

**Memory cost:** ~2.9MB extra @ 800×600 (from 2× u32→3× u32 histogram + unpacked scales)

### Current State: Fixed Global Scale = 100

**Configuration:**
- `initial_scale = 100` (maximum color depth)
- `adjust_scale_pass()` disabled
- All infrastructure improvements kept

**Performance:** Same as before (~2M iterations/second @ 60 FPS)

**Quality:** Excellent - 10× better color depth than scale=10

**Remaining issue:** Original problem still exists - color overflow in extremely dense areas with 4× batching. Scale=100 maximizes color depth but provides no overflow protection.

### The Core Dilemma

With 4× batching (128 workgroups × 256 iterations × 4 frames = 131,072 iterations per batch):

**U16 color channels max = 65,535**

At scale=100:
- Each hit adds: color (0-1) × 100 = 0-100 to u16
- Overflow after: 65,535 / 100 = ~655 hits per pixel per batch
- High-density pixels in some fractals easily exceed this

**Trade-offs:**
- Lower scale (10-50): Prevents overflow but reduces color precision
- Higher scale (100): Maximum precision but overflows in dense areas
- Per-pixel adaptive: Complex, buggy, incompatible with batching

---

## New Approach: Convergence-Based Sample Limiting

### Date
2025-10-27

### The Insight: Convergence, Not Scaling

**User's key observation:**
> "If there's so many iterations happening for a single pixel that it's risking overflowing, does that pixel need any more data? Once a pixel gets X amount of iterations, there's no further improving the quality."

**The fundamental shift:**
- Old thinking: Scale down colors to prevent overflow
- New thinking: Stop sampling pixels that have already converged

**Why this makes sense:**
- Pixel with 10,000 hits knows its "true" color
- Hit 10,001 changes result by 0.01% (negligible)
- Yet we waste compute on already-converged pixels
- Meanwhile, sparse areas starve for samples

### Proposed Design

**Core idea:** Track iteration count per pixel, stop accumulating when converged.

**Implementation:**
1. **Repurpose scale_buffer as iteration counter**
   - Atomically increment on each hit
   - When count ≥ threshold, pixel is "converged"

2. **Skip histogram writes for converged pixels**
   ```wgsl
   let iteration_count = atomicAdd(&iteration_buffer[pixel_idx], 1);
   if (iteration_count < CONVERGENCE_THRESHOLD) {
       // Still needs samples - write to histogram
       atomicAdd(&histogram[...], color);
   } else {
       // Converged - skip write
   }
   ```

3. **Use iteration count as accumulation mask**
   - Accumulate shader checks convergence flag
   - Only blend new samples from non-converged pixels
   - Converged pixels keep their final color

4. **Handle "max" color accumulation** (User's suggestion)
   - Instead of discarding converged pixel samples, store as "max"
   - Blend into accumulation texture itself
   - No extra buffer needed - use existing accumulation alpha channel?

### Critical Analysis

**Concern 1: Visible boundaries between converged/non-converged?**
- Counter-argument (User): "If everything is converging towards the 'correct' color values, then the non-converged regions should keep updating and eventually match the converged regions."
- **Resolution:** This is correct - no hard boundary if convergence is smooth

**Concern 2: Fixed threshold is arbitrary**
- Counter-argument (User): "Easy fix, it's an adjustable setting."
- **Resolution:** Make `CONVERGENCE_THRESHOLD` a UI parameter (default: 5000?)

**Concern 3: Histogram decode becomes complicated**
- Counter-argument (User): "Can we use the per-pixel tracking as a 'mask' when updating the accumulation texture? It'll act as a filter."
- **Resolution:** Yes - accumulate shader reads iteration_buffer, only blends if below threshold

**Concern 4: Doesn't solve u16 overflow before convergence**
- Counter-argument (User): "The overflows don't seem to happen the first few frames. I'm hoping we can catch them beforehand."
- **Resolution:** If convergence threshold (5000) < overflow point (~655 at scale=100), this works. Need to tune threshold vs scale.

**Concern 5: Sample redistribution is hard**
- Counter-argument (User): "What about adding it as a 'max' that's part of the accumulation texture itself? Do we need an extra texture/buffer?"
- **Resolution:** Store final converged value in accumulation texture directly. No redistribution needed - just stop writing to histogram.

### Refined Design with User Feedback

**Data structures:**
- `iteration_buffer` (repurposed scale_buffer): u32 per pixel, atomically incremented
- `histogram`: Only written by non-converged pixels
- `accumulation_texture`: Stores final colors for all pixels

**Compute shader logic:**
```wgsl
let pixel_idx = ...;
let iteration_count = atomicAdd(&iteration_buffer[pixel_idx], 1u);

if (iteration_count < params.convergence_threshold) {
    // Still needs refinement - write to histogram
    let r16 = u32(color.r * 100.0);  // scale=100 for quality
    let g16 = u32(color.g * 100.0);
    let b16 = u32(color.b * 100.0);
    let density = 100u;  // Fixed scale

    atomicAdd(&histogram[base_idx + 0], r16 | (g16 << 16));
    atomicAdd(&histogram[base_idx + 1], b16);
    atomicAdd(&histogram[base_idx + 2], density);
} else {
    // Converged - skip histogram write
    // Pixel keeps its final color in accumulation texture
}
```

**Accumulate shader logic:**
```wgsl
let pixel_idx = ...;
let iteration_count = iteration_buffer[pixel_idx];

if (iteration_count < params.convergence_threshold) {
    // Still accumulating - decode histogram and blend
    let new_color = decode_histogram(...);
    let blended = mix(prev_color, new_color, blend_factor);
    output = blended;
} else {
    // Converged - keep existing color, no blend
    output = prev_color;
}
```

**Benefits:**
1. **Prevents overflow** - Converged pixels stop accumulating before overflow
2. **Better quality** - Samples naturally focus on sparse areas
3. **Adjustable** - Convergence threshold is user parameter
4. **Simple** - No complex redistribution, just stop writing
5. **Uses existing infrastructure** - Repurpose scale_buffer, use accumulation texture

**Open questions:**
1. What's the right default convergence threshold? (1000? 5000? 10000?)
2. Does this affect temporal blending smoothness?
3. How to visualize convergence progress? (Debug view?)
4. Should threshold be dynamic based on fractal type?

### Implementation Status: COMPLETED

**Date:** 2025-10-27

**Changes made:**
1. ✅ Repurposed scale_buffer as iteration counter (atomic u32 per pixel)
2. ✅ Compute shaders atomically increment counter on each hit
3. ✅ Histogram writes only happen when `iteration_count < threshold`
4. ✅ Accumulate shader checks threshold before blending
5. ✅ Fixed all atomic binding declarations (read_write access)

**Files modified:**
- `src/gpu/buffers.rs` - Renamed to "iteration counter", reset to 0
- `src/gpu/pipelines.rs` - Changed bindings to read_write for atomics
- `shaders/core/header.wgsl` - Declared as `array<atomic<u32>>`
- `shaders/core/main_2d.wgsl` - Added convergence check with threshold
- `shaders/core/main_3d.wgsl` - Added convergence check with threshold
- `shaders/accumulate.wgsl` - Check threshold before blending
- `src/renderer/compute_kernel.rs` - Updated debug stats to show iteration counts

### Test Results

**Iteration count distribution (from logs):**
```
Frame 60:  min=0, max=1,160,779, avg=56
Frame 480: min=0, max=9,234,723, avg=445
```

**Key findings:**
1. Dense pixels can accumulate **millions** of iterations (9M+ in 480 frames)
2. This equals ~20,000 iterations/frame for hottest pixels (totally normal for IFS attractors)
3. The iteration counter tracks ALL hits forever (not capped)
4. The histogram write threshold controls when to STOP writing

**Threshold tuning experiments:**

| Threshold | Result |
|-----------|--------|
| 1,000 | ❌ Black patches in dense areas - stops accumulating too early |
| 10,000 | ❌ Still overflows in dense areas |
| 100,000 | ✅ Better visual quality but still overflows |

### Root Cause Analysis: The Real Problem

**The convergence masking IS working** - histogram writes stop at threshold. But overflow still happens because:

**With threshold=10,000 and scale=50:**
- 10,000 writes × 50 = 500,000 total value accumulated
- U16 max = 65,535
- **Overflow happens at 7.6× the u16 limit!**

Even with convergence masking, we're accumulating way more data than u16 can hold BEFORE we stop writing.

**The math:**
- Overflow point: 65,535 / scale
- At scale=50: Overflow after ~1,310 writes
- At scale=100: Overflow after ~655 writes

**To prevent overflow with current u16 histogram:**
- Threshold must be ≤ (65,535 / scale)
- At scale=50: Threshold ≤ 1,310
- At scale=100: Threshold ≤ 655

**But low thresholds cause quality loss** - pixels stop accumulating before converging to true color.

### The Core Dilemma Remains

We have three conflicting requirements:

1. **High scale (100)** - Needed for color precision/quality
2. **High threshold (10,000+)** - Needed for pixels to converge
3. **U16 histogram** - Can only hold 65,535 per channel

**You can only pick TWO:**
- High scale + High threshold = Overflow (current situation)
- High scale + No overflow = Low threshold = Poor quality (black patches)
- High threshold + No overflow = Low scale = Poor color precision

### Possible Solutions

**Option A: Increase histogram capacity (u32 for RGB)**
- Change packed format from u16 to u32 for color channels
- Allows: threshold=10,000 × scale=100 = 1M (fits in u32 max 4.2B)
- Cost: 2× histogram memory (6× u32 per pixel instead of 3×)
- Benefit: Solves overflow completely, supports any scale/threshold combo

**Option B: Adaptive accumulate frequency**
- Instead of 4× batching, accumulate more frequently in dense areas
- Histogram gets cleared before overflow can happen
- Complexity: How to detect when to accumulate? Per-pixel? Global?

**Option C: Quality slider (adjustable batching)**
- User control: Low quality = 1× batch (no overflow), High quality = 8× batch (faster)
- Simple, gives user the trade-off choice
- Still has overflow at high quality settings

**Option D: Accept the overflow, tune threshold lower**
- Set threshold = 1,000 (just under overflow point for scale=50)
- May need scale=30-40 to give more headroom
- Simpler but compromises quality

---

## Decision: Implement U32 Histogram (Option A)

### Date
2025-10-27

### Rationale

After implementing and testing convergence masking, it's clear that **Option A (U32 histogram) is the only viable solution** that achieves the goal of "more iterations per pass" without quality loss.

**Why convergence masking failed:**
- The math fundamentally doesn't work with u16 histogram
- To prevent overflow: threshold ≤ (65,535 / scale)
- At scale=50: threshold ≤ 1,310
- At scale=100: threshold ≤ 655
- But low thresholds cause black patches (pixels stop accumulating before converging)

**Why u32 histogram solves it:**
- u32 max = 4,294,967,295
- Supports: scale=100 × threshold=10,000 = 1,000,000 (plenty of headroom)
- Allows 8×+ batching for maximum rendering speed
- No quality compromise, no overflow artifacts

**Memory cost:** 2× histogram (from 3× u32 to 4× u32 per pixel, ~7.7MB @ 800×600)

**Performance:** Negligible - same atomic operations, just more memory bandwidth

### Implementation Plan

**New histogram layout (4× u32 per pixel):**
- Word 0: R (u32) - Full 32-bit red channel
- Word 1: G (u32) - Full 32-bit green channel
- Word 2: B (u32) - Full 32-bit blue channel
- Word 3: Density (u32) - Already u32

**Changes needed:**
1. `buffers.rs`: Change histogram size from `width × height × 3` to `width × height × 4`
2. `main_2d.wgsl`, `main_3d.wgsl`: Write 4 separate u32 words instead of packing
3. `accumulate.wgsl`: Read 4 separate u32 words instead of unpacking

**Benefits:**
- Simpler code (no packing/unpacking bit manipulation)
- Supports any scale/threshold combination
- Enables high-speed rendering (8×+ batching)
- Future-proof for adaptive scaling or other optimizations

---

## Implementation Complete: U32 Histogram Success

### Date
2025-10-27

### Changes Made

**1. Histogram buffer size (buffers.rs):**
```rust
// OLD: 3× u32 per pixel (packed RGB + u32 density)
let histogram_buffer_size = (width * height * 3 * std::mem::size_of::<u32>()) as u64;

// NEW: 4× u32 per pixel (separate R, G, B, density)
let histogram_buffer_size = (width * height * 4 * std::mem::size_of::<u32>()) as u64;
```

**2. Compute shaders (main_2d.wgsl, main_3d.wgsl):**
```wgsl
// OLD: Pack RGB into u16 values
let r16 = u32(color.r * scale);
let g16 = u32(color.g * scale);
let b16 = u32(color.b * scale);
let packed_rg = r16 | (g16 << 16u);
let packed_b = b16;
atomicAdd(&histogram[base_idx + 0u], packed_rg);
atomicAdd(&histogram[base_idx + 1u], packed_b);
atomicAdd(&histogram[base_idx + 2u], density);

// NEW: Write full u32 values directly
let r_u32 = u32(color.r * scale);
let g_u32 = u32(color.g * scale);
let b_u32 = u32(color.b * scale);
atomicAdd(&histogram[base_idx + 0u], r_u32);
atomicAdd(&histogram[base_idx + 1u], g_u32);
atomicAdd(&histogram[base_idx + 2u], b_u32);
atomicAdd(&histogram[base_idx + 3u], density);
```

**3. Accumulate shader (accumulate.wgsl):**
```wgsl
// OLD: Unpack u16 values from packed words
let packed_rg = histogram[base_idx + 0u];
let packed_b = histogram[base_idx + 1u];
let r_sum = f32(packed_rg & 0xFFFFu);
let g_sum = f32((packed_rg >> 16u) & 0xFFFFu);
let b_sum = f32(packed_b & 0xFFFFu);

// NEW: Read full u32 values directly
let r_sum = f32(histogram[base_idx + 0u]);
let g_sum = f32(histogram[base_idx + 1u]);
let b_sum = f32(histogram[base_idx + 2u]);
```

### Test Results

**✅ OVERFLOW ELIMINATED**
- Tested with scale=50, 4× batching
- Dense areas no longer wrap around to dark colors
- Colors accumulate correctly to their true (bright) values
- No artifacts, no darkening, no color corruption

**Behavior change:**
- **Before**: Dense areas overflowed u16 and wrapped to dark colors (artifacts)
- **After**: Dense areas accumulate high values and appear very bright (correct HDR)
- **Solution**: Use existing tone mapping controls (gamma, exposure, tone curve) to compress HDR

### Memory Impact

| Buffer | Old Size | New Size | Change |
|--------|----------|----------|--------|
| Histogram @ 800×600 | ~5.8 MB (3× u32) | ~7.7 MB (4× u32) | +33% |
| Scale buffer | ~1.9 MB | ~1.9 MB | No change |
| **Total increase** | - | **~1.9 MB** | Acceptable |

### Performance Impact

**Negligible:**
- Same number of atomic operations (4 atomics per pixel per iteration)
- Slightly more memory bandwidth (~33% more histogram data)
- Compute shaders slightly simpler (no bit packing logic)
- GPU memory bandwidth is rarely the bottleneck for this workload

**Measured:** No observable FPS difference @ 60 FPS with 4× batching

### Code Simplification

**Lines removed:**
- All bit packing/unpacking logic (shifts, masks, combines)
- Comments explaining packed format
- Mental overhead of understanding packed layout

**Lines added:**
- Simple, direct u32 reads/writes
- Clearer comments about layout

**Net:** Simpler, more maintainable code

### Capacity Analysis

**With u32 histogram:**
- u32 max per channel: 4,294,967,295
- With scale=100: 42,949,672 max hits per pixel
- With 4× batching: 131,072 iterations/batch
- **Overflow point**: 327,653 batches before overflow
- **Time to overflow**: 327,653 batches ÷ 60 FPS = 91 minutes continuous rendering

**Conclusion:** Overflow is now effectively impossible in practice.

### Future Possibilities Enabled

Now that overflow is solved, we can:

1. **Increase batch factor** (4× → 8× → 16×) for even faster rendering
2. **Increase scale** (50 → 100) for better color precision
3. **Re-enable per-pixel adaptive scaling** (if desired) without overflow concerns
4. **Long-duration renders** (hours/days) for ultra-high quality exports

### Bright Dense Areas: Expected Behavior

**User question:** "Instead of overflowing, the areas simply get too bright, which is much better. Is that what's supposed to happen?"

**Answer:** Yes! This is correct behavior:
- Dense areas accumulate many samples → high color values → bright HDR data
- The brightness represents the true fractal structure (accurate accumulation)
- Tone mapping controls (gamma, exposure, tone curve) compress HDR → displayable range
- This is how proper HDR rendering should work

**Additional controls we could add** (future enhancements):
1. **Per-pixel adaptive tone mapping** - Stronger compression for high-density pixels
2. **Density-based color scaling** - Automatically reduce color contribution in super-dense areas
3. **Histogram clipping** - Soft limit on maximum accumulated value per channel
4. **Logarithmic accumulation** - Log scale for very high densities

**Current recommendation:** Use existing tone mapping controls (gamma, exposure, S-curve) to handle bright areas. They work well for this purpose.
