# Tone Curve Debugging History

## Problem Statement
When enabling "Use Tone Curve" with a Linear preset (which should be an identity function), the visual appearance changes:
- Darker/low-density areas become slightly more saturated
- The effect is subtle but consistent
- A linear curve should produce NO visual change

## Changes Attempted (Chronological)

### 1. Fixed Drag Interaction (KEEP - Working)
**Files**: `src/ui/tone_mapping.rs`
**Change**: Implemented persistent drag state using egui's temporary storage
**Result**: Drag interaction now works properly
**Status**: ✅ WORKING - Keep this change

### 2. Added Separate Sampler for Curve LUT
**Files**:
- `src/gpu/buffers.rs` (added `curve_lut_sampler` field)
- `src/gpu/pipelines.rs` (bind group updated)

**Initial attempt**: Nearest-neighbor filtering
**Result**: No change to saturation issue

**Second attempt**: Linear filtering
**Result**: No change to saturation issue

**Current state**: Using linear filtering
**Status**: ❓ Unclear if this should be kept

### 3. Changed Texture Format: Rgba8Unorm → Rgba16Float
**Files**:
- `src/gpu/buffers.rs` (texture format)
- `src/scene/tonemap.rs` (LUT generation changed from u8 to f16)

**Reasoning**: Thought 8-bit quantization was causing precision loss
**Result**: No change to saturation issue
**Precision test**: f16 max error is 0.0002 (0.02%) - negligible
**Status**: ❓ Could revert to Rgba8Unorm, but f16 has better precision

### 4. Changed Shader Sampling: textureLoad → textureSample → textureLoad → textureSample
**Files**: `shaders/tonemap.wgsl`

**Attempt A**: Used `textureLoad` with integer indices
```wgsl
let r_idx = u32(clamp(color.r * 255.0, 0.0, 255.0));
let r = textureLoad(curve_lut_texture, r_idx, 0).r;
```
**Result**: Saturation issue persisted

**Attempt B**: Switched to `textureSample` with normalized coordinates
```wgsl
let r = textureSample(curve_lut_texture, curve_lut_sampler, color.r).r;
```
**Result**: Saturation issue persisted

**Current state**: Using `textureSample`
**Status**: ❌ Multiple approaches failed

### 5. Changed LUT Generation: Texel Centers vs Edge Values
**Files**: `src/scene/tonemap.rs`

**Attempt A**: Generate at texel centers `(i + 0.5) / 256.0`
**Result**: Grey background bug (areas with zero density became grey)

**Attempt B**: Generate at edge values `i / 255.0`
**Result**: Fixed grey background, but saturation issue persisted

**Current state**: Using `i / 255.0`
**Status**: ❓ Current approach seems correct

### 6. Changed bytes_per_row for 1D Texture Upload
**Files**: `src/gpu/buffers.rs` (both initial upload and update_curve_lut)

**Before**: `bytes_per_row: Some(256 * 4 * 2)`
**After**: `bytes_per_row: None`
**Reasoning**: 1D textures might not need row stride
**Result**: No change to saturation issue
**Status**: ❓ Unclear which is correct for 1D textures

### 7. Added Debug Logging
**Files**:
- `src/gpu/buffers.rs` (logs LUT data size on upload)
- `src/renderer/compute_kernel.rs` (logs use_curve changes and curve updates)

**Status**: ✅ KEEP - Useful for debugging

### 8. Added Test Programs
**Files**:
- `src/bin/test_curve.rs` - Tests CPU-side LUT generation
- `src/bin/test_curve_f16.rs` - Tests f16 precision and interpolation
- `src/bin/compare_lut_precision.rs` - Measures f16 precision loss

**Status**: ✅ KEEP - Useful for verification

## Key Findings

### What We Proved Works Correctly:
1. **LUT Generation**: CPU-side curve evaluation is correct (identity function produces `y = x`)
2. **f16 Precision**: Max error from f16 conversion is 0.0002 - too small to cause visible changes
3. **Fixed Coordinate Sampling**: When sampling at constant coordinate 0.5, we get exactly 0.5 back (black screen test)
4. **Texture Upload**: The texture data is being uploaded correctly

### What Still Doesn't Work:
1. **Variable Coordinate Sampling**: When using actual color values as coordinates, we get errors (greenish-blue tint in error visualization)
2. **Visual Result**: Linear curve still causes saturation increase in darker areas

### Contradiction:
- Sampling at fixed coordinate 0.5 returns exactly 0.5 ✅
- But sampling at variable coordinates (color values) produces errors ❌
- This suggests the problem is NOT in the texture data or sampling mechanism itself
- The issue might be elsewhere in the shader pipeline

## Current Shader Code Path

**Without curve enabled** (`use_curve = 0`):
```wgsl
color *= exposure;
color = clamp(color, vec3(0.0), vec3(1.0));  // Linear mode
// Skip curve block
color = pow(color, vec3(1.0 / gamma));
color = clamp(color, vec3(0.0), vec3(1.0));
```

**With curve enabled** (`use_curve = 1`):
```wgsl
color *= exposure;
color = clamp(color, vec3(0.0), vec3(1.0));  // Linear mode
// Apply curve
let r = textureSample(curve_lut_texture, curve_lut_sampler, color.r).r;
let g = textureSample(curve_lut_texture, curve_lut_sampler, color.g).r;
let b = textureSample(curve_lut_texture, curve_lut_sampler, color.b).r;
color = vec3(r, g, b);
// Continue
color = pow(color, vec3(1.0 / gamma));
color = clamp(color, vec3(0.0), vec3(1.0));
```

## Hypothesis (Unproven)
User suggested: Maybe the curve-enabled path is actually CORRECT, and there's a bug in the curve-disabled path that we haven't found yet.

## Files Modified (Summary)

### Core Changes:
- `src/gpu/buffers.rs` - Texture format, sampler, upload parameters
- `src/gpu/pipelines.rs` - Bind group for curve sampler
- `src/scene/tonemap.rs` - LUT generation (u8 → f16)
- `shaders/tonemap.wgsl` - Sampling method and debug code
- `src/ui/tone_mapping.rs` - Drag interaction fix

### Debug/Test Files (Keep):
- `src/bin/test_curve.rs`
- `src/bin/test_curve_f16.rs`
- `src/bin/compare_lut_precision.rs`

### Debug Logging (Keep):
- `src/gpu/buffers.rs` - Upload logging
- `src/renderer/compute_kernel.rs` - State change logging

## Quantitative Analysis (2025-10-23)

With deterministic RNG and improved testing tools, we can now quantify the bug precisely.

### Test Setup:
- Preset: Simple2
- Frames: 100 (deterministic RNG)
- Resolution: 1920x1080
- Comparison: No curve vs Linear curve (should be identical)

### Results:
```
Total pixel difference: 1,556,909
Average difference per pixel: 0.75
Maximum pixel difference: 35
Different pixels: 253,845 (12.24%)

Per-Channel Statistics:
  Red   - Avg: 0.23 (0.09%), Max: 15 (5.88%)
  Green - Avg: 0.30 (0.12%), Max: 14 (5.49%)
  Blue  - Avg: 0.22 (0.09%), Max: 15 (5.88%)
  Alpha - Avg: 0.00 (0.00%), Max: 0 (0.00%)
```

### Analysis:
1. **Small but real bug**: Average error ~0.1% per channel, max ~6%
2. **Green channel slightly higher**: 0.30 avg vs 0.22-0.23 for R/B
3. **Affects 12% of pixels**: Only pixels with non-zero color values
4. **Alpha PERFECT match**: 0.00% error - confirms bug is RGB-only, not in density calculation
5. **Subtle visual impact**: 0.1% average error is nearly imperceptible

### Key Insight from Alpha Channel:
The fact that alpha matches perfectly (0.00% error) is significant:
- Both code paths read from same accumulation buffer correctly
- Density calculation is identical in both paths
- **Bug is isolated to RGB tone curve application** (lines 72-77 in tonemap.wgsl)
- This narrows the search to texture sampling of the curve LUT

### Conclusion:
The bug is REAL but SUBTLE. A linear curve should produce 0% difference, but we see ~0.1% average error per channel. This suggests a minor precision or sampling issue in the curve application path.

## Systematic Testing (2025-10-23)

With deterministic RNG and quantitative tools, tested various hypothetical fixes:

### Test 1: Nearest Neighbor Filtering
**Change**: `FilterMode::Linear` → `FilterMode::Nearest` in curve_lut_sampler
**Hypothesis**: Linear filtering interpolation might introduce errors
**Result**: ❌ WORSE
- Linear: Avg 0.23-0.30 (0.09-0.12%), Max 14-15 (5.49-5.88%)
- Nearest: Avg 0.25-0.34 (0.10-0.13%), Max 20 (7.84%)
**Conclusion**: Linear filtering is actually closer to correct
**Status**: REVERTED

### Test 2: Texel Center LUT Generation
**Change**: `i / 255.0` → `(i + 0.5) / 256.0` in generate_lut()
**Hypothesis**: Maybe sampling coordinates don't align with LUT generation
**Result**: ❌ MUCH WORSE
- Edge values: Avg 0.23-0.30 (0.09-0.12%), 12.24% pixels affected
- Texel centers: Avg 13.22-13.28 (5.18-5.21%), 89.59% pixels affected!
**Conclusion**: Edge values (i/255) are correct for proper range coverage (LUT[0]=f(0.0), LUT[255]=f(1.0))
**Status**: REVERTED (confirms previous finding from history)

### Test 3: f16 vs u8 Texture Format
**Status**: PENDING - Need to test

### Test 4: Edge Case Handling
**Status**: PENDING - Check behavior at 0.0 and 1.0

## Next Steps (Proposed)
1. ✅ Create quantifiable test framework (CLI PNG export) - DONE
2. ✅ Build image comparison tool with per-channel stats - DONE
3. ✅ Test if RNG is deterministic - DONE
4. ✅ Get numerical measurements instead of visual assessments - DONE
5. ✅ Test filtering modes - DONE (Linear is better than Nearest)
6. ✅ Test LUT coordinate generation - DONE (Edge values i/255 are correct)
7. Test f16 vs u8 texture format
8. Investigate shader sampling precision
9. Consider if 0.1% error is acceptable or needs fixing
