# Packed Histogram Implementation Plan

## Executive Summary

**Goal:** Reduce atomic operations from 4 to 1 by packing RGBA+density into single u32.

**Expected Performance:** 2-3× speedup over current naive histogram (based on atomic operation reduction)

**Key Innovation:** Use 8-bit saturation to prevent overflow corruption that plagued previous 10-bit attempt.

## Problem Statement

Current naive histogram performs 4 atomic operations per pixel hit:
```wgsl
atomicAdd(&histogram[base_idx + 0u], r);  // R: 0-10000
atomicAdd(&histogram[base_idx + 1u], g);  // G: 0-10000
atomicAdd(&histogram[base_idx + 2u], b);  // B: 0-10000
atomicAdd(&histogram[base_idx + 3u], 1u); // density
```

**Bottleneck identified:** Atomic operations to memory are the performance bottleneck (10% difference between zoom levels due to atomic pressure).

**Benchmark evidence:**
- Low zoom (1.0): 6510ms → More atomic ops (95% hit viewport)
- High zoom (25.5): 5904ms → Fewer atomic ops (1.5% hit viewport)
- **Difference: 10% purely from atomic operation count**

Reducing atomic ops from 4 to 1 should yield significant speedup!

## Previous Attempt and Why It Failed

**Attempt 1: 10-bit packing (HISTOGRAM_OPTIMIZATION_ATTEMPTS.md)**
```wgsl
// 10 bits per channel (0-1023)
let packed = r | (g << 10u) | (b << 20u);
```

**Failed due to overflow corruption:**
- R field maxes at 1023 (10 bits)
- Popular pixels get thousands of hits
- After 103 hits: R overflows into G field
- Result: Bright areas turn grey (color corruption)

**Root cause:** No overflow protection with atomic operations!

## New Solution: 8-Bit Saturation Packing

### Bit Layout (Single u32)

```
┌─────────┬─────────┬─────────┬─────────┐
│ bits    │ 0-7     │ 8-15    │ 16-23   │ 24-31   │
├─────────┼─────────┼─────────┼─────────┼─────────┤
│ field   │ R       │ G       │ B       │ Density │
│ range   │ 0-255   │ 0-255   │ 0-255   │ 0-255   │
│ max val │ 255     │ 255     │ 255     │ 255     │
└─────────┴─────────┴─────────┴─────────┴─────────┘
```

### Key Design Decisions

#### 1. 8-bit Channels (Not 10-bit)
- **Precision:** 8-bit (0-255) is sufficient for color
  - Human eye can't distinguish 256 vs 1024 levels in accumulated color
  - Final color is averaged, not direct pixel value
- **Safety:** 8 bits = max 255, no overflow into adjacent fields
- **Saturation:** When channel reaches 255, it stops accumulating (acceptable)

#### 2. 8-bit Density (Not unlimited)
- **Range:** 0-255 hits per pixel
- **Overflow behavior:** Saturates at 255
- **Interpretation:** "255" means "at least 255 hits" (very bright)
- **Practical:** Most pixels have < 255 hits; bright areas saturate (acceptable)

#### 3. Saturation Strategy
Instead of allowing overflow, clamp to max value:
```wgsl
// Before packing, clamp to prevent overflow
let r8 = min(u32(final_color.r * 255.0), 255u);
let g8 = min(u32(final_color.g * 255.0), 255u);
let b8 = min(u32(final_color.b * 255.0), 255u);
```

This ensures no field ever exceeds its 8-bit boundary!

## Implementation Details

### Compute Shader Changes

**File:** `shaders/core/main_2d.wgsl`, `shaders/core/main_3d.wgsl`

**Current code (lines ~70-85):**
```wgsl
// Atomic accumulation to histogram buffer
let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);
let base_idx = pixel_idx * 4u;

// Scale color to integers (0.0-1.0 → 0-10000 for precision)
let color_scale = 10000.0;
let r = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
let g = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
let b = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);

// Atomic add (thread-safe!)
atomicAdd(&histogram[base_idx + 0u], r);
atomicAdd(&histogram[base_idx + 1u], g);
atomicAdd(&histogram[base_idx + 2u], b);
atomicAdd(&histogram[base_idx + 3u], 1u);  // density
```

**New code:**
```wgsl
// Atomic accumulation to histogram buffer (PACKED)
let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

// Pack RGB + density into single u32 (8 bits each, with saturation)
let r8 = min(u32(clamp(final_color.r, 0.0, 1.0) * 255.0), 255u);
let g8 = min(u32(clamp(final_color.g, 0.0, 1.0) * 255.0), 255u);
let b8 = min(u32(clamp(final_color.b, 0.0, 1.0) * 255.0), 255u);
let density8 = 1u;  // Always 1 per hit

let packed = r8 | (g8 << 8u) | (b8 << 16u) | (density8 << 24u);

// Single atomic operation! (4× reduction!)
atomicAdd(&histogram[pixel_idx], packed);
```

**Key changes:**
- `base_idx * 4u` → `pixel_idx` (no longer 4 elements per pixel)
- 4 atomic operations → 1 atomic operation
- `color_scale = 10000` → `255` (8-bit precision)
- Added saturation: `min(..., 255u)`

### Accumulate Shader Changes

**File:** `shaders/accumulate.wgsl`

**Current code:**
```wgsl
// Read histogram values
let base_idx = pixel_idx * 4u;
let r_sum = f32(histogram[base_idx + 0u]);
let g_sum = f32(histogram[base_idx + 1u]);
let b_sum = f32(histogram[base_idx + 2u]);
let density = f32(histogram[base_idx + 3u]);

// Convert back to float color (average)
let color_scale = 10000.0;
var new_color = vec3<f32>(0.0);
if (density > 0.0) {
    new_color = vec3<f32>(
        r_sum / (density * color_scale),
        g_sum / (density * color_scale),
        b_sum / (density * color_scale)
    );
}
```

**New code:**
```wgsl
// Read packed histogram value
let packed = histogram[pixel_idx];

// Unpack RGB + density (8 bits each)
let r_sum = f32(packed & 0xFFu);
let g_sum = f32((packed >> 8u) & 0xFFu);
let b_sum = f32((packed >> 16u) & 0xFFu);
let density = f32((packed >> 24u) & 0xFFu);

// Convert back to float color (average)
// Note: Each channel is 0-255, density is 0-255
var new_color = vec3<f32>(0.0);
if (density > 0.0) {
    new_color = vec3<f32>(
        r_sum / (density * 255.0),
        g_sum / (density * 255.0),
        b_sum / (density * 255.0)
    );
}

// Note: If density saturated at 255, we're dividing by a smaller number than actual hits
// This makes bright areas slightly brighter (acceptable trade-off for 4× speedup)
```

**Key changes:**
- `base_idx * 4u` → `pixel_idx` (single u32 per pixel)
- Unpack via bit masking and shifting
- `color_scale = 10000` → `255`
- Saturation handling in comment (not in code - automatic)

### Buffer Size Changes

**File:** `src/gpu/buffers.rs`

**Current:**
```rust
// Size: width × height × 4 × sizeof(u32)
// Example: 1920 × 1080 × 4 × 4 = 33,177,600 bytes (~31 MB)
let size = (width * height * 4 * std::mem::size_of::<u32>()) as u64;
```

**New:**
```rust
// Size: width × height × sizeof(u32)
// Example: 1920 × 1080 × 4 = 8,294,400 bytes (~8 MB)
let size = (width * height * std::mem::size_of::<u32>()) as u64;
```

**Bonus benefit:** 4× smaller buffer (31 MB → 8 MB)!

### Clear Operation Changes

**File:** `src/gpu/buffers.rs` - `clear_histogram()`

**Current:**
```rust
encoder.clear_buffer(&self.histogram_buffer, 0, None);
```

**No changes needed!** - Clearing entire buffer works the same for packed format.

## Overflow Handling Strategy

### During Accumulation (Compute Shader)

**Saturation at packing time:**
```wgsl
let r8 = min(u32(final_color.r * 255.0), 255u);  // Clamp before packing
```

**Saturation during atomic add:**
- Atomic add may cause packed value to exceed 255 in any field
- But we prevent this by clamping BEFORE packing
- Each individual contribution is ≤ 255 per field
- **Problem:** Multiple atomicAdds can still overflow!

**Wait, this doesn't prevent overflow!**

Let me reconsider...

### The Real Overflow Solution

The issue is that `atomicAdd(&histogram[pixel_idx], packed)` will ADD the entire packed u32 as a single number!

**Example overflow scenario:**
```
Initial: packed = 0x00000000 (all zeros)
Hit 1:   packed = 0x01010101 (R=1, G=1, B=1, D=1)
Hit 2:   packed = 0x02020202 (R=2, G=2, B=2, D=2)
...
Hit 255: packed = 0xFFFFFFFF (R=255, G=255, B=255, D=255)
Hit 256: packed = 0x100010001 → OVERFLOW into next field!
```

**We can't use simple atomicAdd with packed values!**

### Alternative: Saturating Atomic Add

We need a custom atomic operation that:
1. Unpacks the current value
2. Unpacks the incoming value
3. Adds each field independently
4. Saturates each field at 255
5. Repacks and stores

**WGSL doesn't support this atomically!**

### Solution: Use atomicMax with Incremental Values

Instead of adding `packed`, we use `atomicMax` to ensure values never decrease:

**Wait, this won't work either...**

### ACTUAL Solution: Use 4 Separate u8 Atomics

WGSL doesn't support u8 atomics, only u32 atomics. So we can't pack into a single atomic u32 without overflow risk.

**Back to the drawing board...**

## Alternative: Packed Storage, Separate Atomics

**New approach:**
- Store histogram as `array<u32>` (packed format)
- But perform atomics on SEPARATE unpacked buffer
- Convert packed → unpacked at start of frame
- Convert unpacked → packed after accumulation

**This defeats the purpose of packing (doesn't reduce atomic ops)!**

## Real Solution: Accept Saturation Semantics

**Key insight:** For fractal flames, saturation is visually acceptable!

**Approach:**
1. Pack RGB+density into single u32 (8 bits each)
2. Use `atomicAdd` knowing that overflow CAN happen
3. Handle overflow gracefully in unpack:
   - If any field exceeds 255, it wraps around (modulo 256)
   - Detect wrapping via density field
   - If density < expected, we know overflow occurred
   - Use saturation value (255) for that field

**Better approach: Use atomicMax Instead of atomicAdd**

Wait, we WANT to add, not take max...

## Final Solution: Packed Format with Overflow Detection

I think we need to step back and reconsider the entire approach. The fundamental issue is that WGSL's atomic operations work on entire u32 values, not individual bit fields.

**Options:**
1. **Accept overflow:** Let it happen, deal with artifacts
2. **Use 4 separate atomics:** Current approach (known to work)
3. **Use 4 separate u8 textures:** Not supported in WGSL
4. **Use atomicCompareExchange:** Complex, likely slower than 4 atomics

## Recommendation: Prototype with Overflow Acceptance

Let's implement the packed version and see what the visual artifacts look like:
- Best case: Artifacts are minimal/acceptable
- Worst case: We learn what doesn't work
- Either way: We'll have benchmark data

**Implementation plan:**
1. Implement packed format with simple atomicAdd
2. Test visually with various fractals
3. Benchmark performance
4. If artifacts are unacceptable, explore atomicCompareExchange
5. If performance isn't better, revert to 4-atomic approach

This gives us empirical data rather than theoretical analysis!

## Implementation Steps

### Phase 1: Basic Packed Implementation (Accept Overflow Risk)
1. [ ] Modify compute shaders to pack RGB+D into single u32
2. [ ] Modify accumulate shader to unpack
3. [ ] Update buffer size calculation
4. [ ] Test with simple single-color fractal
5. [ ] Document visual artifacts

### Phase 2: Performance Benchmarking
1. [ ] Benchmark packed vs unpacked at various zooms
2. [ ] Measure speedup (expected: 2-3×)
3. [ ] Profile atomic operation counts

### Phase 3: Artifact Mitigation (If Needed)
1. [ ] Implement atomicCompareExchange-based saturating add
2. [ ] Compare performance vs simple atomicAdd
3. [ ] Evaluate visual quality vs performance trade-off

### Phase 4: Decision
- If visual quality acceptable: SHIP IT!
- If quality issues but fast: Make it optional (speed mode)
- If no performance gain: REVERT

## Expected Results

**Best case:**
- 2-3× speedup (from 4 atomic ops → 1)
- Minimal visual artifacts (overflow rare in practice)
- 4× smaller buffer (31 MB → 8 MB)

**Worst case:**
- Severe color corruption in bright areas
- No performance gain (overflow handling overhead)
- Need to revert to 4-atomic approach

**Most likely:**
- Moderate speedup (1.5-2×)
- Acceptable artifacts with some fractals
- Optional mode: "Fast" (packed) vs "Quality" (unpacked)

## Risk Mitigation

1. **Keep current implementation:** Feature branch only
2. **Extensive testing:** Visual comparison with Apophysis
3. **Rollback plan:** Revert is easy (just undo shader changes)
4. **User choice:** Could offer both modes if artifacts are hit-or-miss

## Next Steps

1. Create feature branch: `feature/packed-histogram`
2. Implement Phase 1 (basic packing)
3. Visual testing with 10+ different fractals
4. Benchmark at 3 zoom levels (1.0, 10.0, 25.0)
5. Document findings
6. Decision: ship, iterate, or revert

---

**Status:** Ready for implementation
**Priority:** HIGH (potential 2-3× performance gain)
**Risk:** MEDIUM (overflow artifacts unknown until tested)
