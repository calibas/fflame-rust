# Why Packed Atomic Operations Fail

## Date: 2025-10-26
## Status: CONFIRMED - Packed atomics fundamentally broken for this use case

## The Problem

Both u8 and f16 packed histogram implementations produced visual artifacts:
- **u8 packed:** Grey noise in bright areas (overflow at ~256 hits)
- **f16 packed:** Psychedelic color noise in bright areas (bit corruption)

## Root Cause: atomicAdd on Packed Values

### How atomicAdd Works

```wgsl
atomicAdd(&buffer[index], value)
```

This performs:
```
old_value = buffer[index]          // Read u32 as integer
new_value = old_value + value      // INTEGER addition
buffer[index] = new_value          // Write u32 as integer
```

**Key insight:** atomicAdd treats the entire u32 as a SINGLE INTEGER, regardless of how you packed the bits.

### Why u8 Packing Fails

```wgsl
// Pack 4× u8 into single u32
let packed = r8 | (g8 << 8) | (b8 << 16) | (d8 << 24)
// Example: R=255, G=0, B=0, D=1
// Packed = 0x01000000 + 0xFF = 0x010000FF

atomicAdd(&histogram[pixel_idx], packed)
```

**What happens on next hit:**
```
First hit:  0x010000FF (R=255, G=0, B=0, D=1)
Second hit: 0x010000FF (adding same value)
Result:     0x020001FE (R=254, G=1, B=0, D=2)  ❌ WRONG!
```

The integer addition caused:
- R channel overflow (255+255 = 510, wraps to 254 with carry)
- Carry bit corrupted G channel (0+0+carry = 1)
- D channel incremented correctly (1+1 = 2)

This is why we saw grey noise - channels were overflowing and corrupting each other.

### Why f16 Packing Fails

```wgsl
// Pack 2× f16 into single u32
let packed_rg = pack2x16float(vec2<f32>(0.5, 0.3))
// Produces u32 with two IEEE 754 half-precision floats
// Bit layout: [G mantissa/exp][R mantissa/exp]

atomicAdd(&histogram[base_idx], packed_rg)
```

**What happens:**
1. `pack2x16float()` creates proper f16 bit patterns
2. `atomicAdd()` treats entire u32 as integer
3. Integer addition corrupts float bit patterns
4. `unpack2x16float()` interprets corrupted bits as floats
5. Result: Random-looking float values (psychedelic noise)

**Example:**
```
R=0.5, G=0.3 packed = 0x4000_3C00 (proper f16)
Add same value:       0x4000_3C00
Result:               0x8000_7800 ❌
Unpacked: R = -0.0, G = +inf (CORRUPTED!)
```

The mantissa and exponent bits got added as integers, producing nonsense floats.

### Why This Happens in High Density Areas

- **Low density:** Few hits per pixel, small accumulated values
  - u8: Values < 256, no overflow yet
  - f16: Small corrupted bits still look plausible
- **High density:** Many hits per pixel, large accumulated values
  - u8: Overflow at 256, corruption becomes visible
  - f16: Accumulated corruption becomes extreme (inf, nan, random)

This is why artifacts only appear in bright (high density) regions.

## The Fundamental Limitation

**WGSL atomic operations only support INTEGER semantics:**
- `atomicAdd` performs integer addition
- `atomicAnd`, `atomicOr`, `atomicXor` perform bitwise operations
- `atomicMin`, `atomicMax` perform integer comparison

**There is NO way to pack multiple values and atomically add them separately.**

Any packing scheme will have this problem:
- Bit fields: Overflow corruption (u8)
- Floats: Bit pattern corruption (f16)
- Fixed-point: Same as bit fields (overflow corruption)

## Correct Solution: Separate Atomics

The only correct approach with current WGSL is 4 separate atomic operations:

```wgsl
// Convert color to scaled integers
let color_scale = 10000.0;
let r = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
let g = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
let b = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);

// Four separate atomic operations (REQUIRED for correctness)
atomicAdd(&histogram[base_idx + 0u], r);
atomicAdd(&histogram[base_idx + 1u], g);
atomicAdd(&histogram[base_idx + 2u], b);
atomicAdd(&histogram[base_idx + 3u], 1u);
```

**Why this works:**
- Each channel has dedicated u32
- No bit packing or sharing
- Integer addition preserves values correctly
- Scale factor (10000) maintains precision

## Performance Cost of Correctness

**Naive histogram (4 atomics):** 5933ms baseline
**Packed attempts:**
- u8 (1 atomic): 4998ms but BROKEN (grey noise)
- f16 (2 atomics): 5248ms but BROKEN (color noise)

**Conclusion:** The naive approach is ~10% slower but is the ONLY correct implementation.

## Alternative Approaches (Not Implemented)

### 1. Per-Workgroup Local Accumulation
- Accumulate in workgroup shared memory (no atomics needed)
- Single atomic per channel per workgroup at end
- Reduces atomic pressure by ~256× (workgroup size)
- **Complexity:** High (requires careful synchronization)
- **Benefit:** Potentially 2-3× speedup

### 2. Ping-Pong Textures (No Atomics)
- Write samples to texture with additive blending
- **Problem:** GPU blending is not atomic, causes race conditions
- **Status:** Already tried, this is why we have histograms!

### 3. Compute Shader Splitting
- Split computation across multiple passes
- Fewer threads → less atomic contention
- **Tradeoff:** More dispatches = more overhead

### 4. Wait for WGSL Float Atomics
- Future WGSL spec may add `atomicAddFloat()`
- Would enable direct float accumulation
- **Status:** Not in current spec, years away

## Recommendation

**Accept the naive histogram performance.** The 10% cost is the price of correctness. Visual quality is non-negotiable for a fractal renderer.

Further optimization requires architectural changes (local accumulation, etc.) that are complex and risky.

## References

- WGSL Atomic Operations: https://www.w3.org/TR/WGSL/#atomic-builtin-functions
- IEEE 754 Half Precision: https://en.wikipedia.org/wiki/Half-precision_floating-point_format
- GPU Atomics Performance: https://developer.nvidia.com/blog/gpu-pro-tip-fast-histograms-using-shared-atomics-maxwell/
