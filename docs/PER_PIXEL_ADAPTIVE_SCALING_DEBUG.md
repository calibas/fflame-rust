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

### Next Investigation

The darkening happens progressively over many frames, suggesting cumulative error rather than a one-time math mistake. Possible causes:

1. **Aggressive scale reduction in dense areas**
   - adjust_scale may be dropping scales too quickly
   - Could cause under-representation of dense area brightness

2. **Accumulation blending issue**
   - The exponential moving average in accumulate.wgsl
   - May not handle varying densities correctly

3. **Histogram clearing timing**
   - Histogram cleared every 4 frames (batched)
   - Scales updated every frame
   - Potential for stale data
