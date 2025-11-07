# Visual Baseline Testing vs Apophysis

## Goal
Establish visual baseline to verify that fractal patterns match Apophysis 7X reference implementation.

## Coordinate System Analysis

### Key Findings (Updated 2025-10-25)

**CRITICAL DISCOVERY:** Apophysis uses **Y-down coordinate system** for triangle display but **standard math** for iteration!

**Triangle Editor Coordinate System:**
- Apophysis displays triangle coordinates in **Y-down** system (negates f, b, c)
- Example: XML `coefs="0.932358 0.414263 -0.54106 1.001987 -0.041391 -0.016556"`
  - Apophysis shows: X:(0.932358, **-0.414263**), Y:(**0.54106**, 1.00198), O:(-0.041391, **0.016556**)
  - XML stores: a=0.932358, b=0.414263, c=-0.54106, d=1.001987, e=-0.041391, f=-0.016556
  - Pattern: Apophysis displays (a, -b), (-c, d), (e, -f) for X, Y, O respectively

**Fractal Math Coordinate System:**
- Internal fractal calculations use **standard mathematical coordinates** (no Y negation!)
- Affine transform: `x' = a*x + b*y + e`, `y' = c*x + d*y + f` (standard formula)
- Variations operate on post-affine coordinates using standard math
- **Previous Y-negation in shader was incorrect!**

**Display Coordinate System:**
- Screen rendering uses standard pixel mapping (no additional Y-flip needed)
- Direct conversion: `pixel = center + transformed * scale`
- Triangle Editor canvas already applies Y-flip for visualization (line 117: `rect.max.y - ...`)

**Critical Insight:**
- **Y-negation only for triangle visualization**, not for iteration!
- Triangle Editor `to_triangle()`: O=[e, -f], X=[e+a, -f-b], Y=[e-c, -f+d] (matches Apophysis display)
- Triangle Editor `from_triangle()`: Inverse conversion with negations
- Affine transform in shader: Standard formula without negation
- This separates **display coordinates** (Y-down) from **iteration math** (standard)

### Implementation

**Triangle Coordinate Conversion (for visualization):**
```rust
// In transforms.rs - converts to Apophysis Y-down display coordinates
pub fn to_triangle(&self) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let o = [self.e, -self.f];
    let x = [self.e + self.a, -self.f - self.b];
    let y = [self.e - self.c, -self.f + self.d];
    (o, x, y)
}

pub fn from_triangle(&mut self, o: [f32; 2], x: [f32; 2], y: [f32; 2]) {
    self.a = x[0] - o[0];
    self.b = -(x[1] - o[1]);  // Negate
    self.c = -(y[0] - o[0]);  // Negate
    self.d = y[1] - o[1];
    self.e = o[0];
    self.f = -o[1];  // Negate
}
```

**Shader Affine Transform (for iteration):**
```wgsl
// In affine.wgsl and variations_3d.wgsl - standard mathematical formula
fn apply_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        xform.c * p.x + xform.d * p.y + xform.f  // NO negation!
    );
}
```

**Apophysis Reference:**
- Source: https://github.com/xyrus02/apophysis-7x
- Triangle display uses Y-down (UI convention), but iteration uses standard math

## Test Results

### ✅ Single Transform, Single Variation Tests

**Test: spherical.fflame**
- **Config**: Single transform, spherical=1.0, no rotation
- **Result**: ✅ **VISUAL MATCH** - Renders identically in both Apophysis and this app
- **Tested scales**: a=d=0.9 (90%), a=d=1.5 (150%) - both match perfectly
- **Conclusion**: Basic affine transform + single variation works correctly across different scales

**Settings:**
```json
{
  "a": 0.9-1.5, "b": 0.0, "c": 0.0, "d": 0.9-1.5, "e": 0.0, "f": 0.0,
  "variations": { "spherical": 1.0 }
}
```

**Test: linear.fflame**
- **Config**: Single transform, a=d≈1.0, linear=1.0
- **Result**: ✅ **VISUAL MATCH** (after removing incorrect Y-negation from shader)
- **Rotation test**: 45° clockwise rotation matches in both apps
- **Translation test**: Transform movement matches Apophysis exactly
- **Scale test**: Increasing transform scale 90% → 150% matches behavior
- **Conclusion**: Linear variation works correctly with standard affine formula

**Settings:**
```json
{
  "a": 0.9958939, "b": 0.0, "c": 0.0, "d": 0.9958939, "e": 0.0, "f": 0.0,
  "variations": { "linear": 1.0 }
}
```

**Test: Complex multi-transform fractal**
- **Config**: Multiple transforms with rotations, translations, various variations
- **Result**: ✅ **VERY SIMILAR** (after affine formula fix - 2025-10-25)
- **Key Fix**: Removed Y-negation from shader `apply_affine()`, added Y-down conversion to `to_triangle()`
- **Conclusion**: Core affine transformation now matches Apophysis behavior

### 🔍 Pending Tests

**Test: sinusoidal.fflame**
- Single transform, sinusoidal variation only
- Status: Not yet tested

**Test: Simple preset**
- Multiple transforms with multiple variations
- Status: Ready to test after display Y-flip fix

**Test: Multiple Transforms**
- Multiple transforms with same variation
- Status: Not yet tested

**Test: Multiple Variations**
- Single transform with multiple variations weighted
- Status: Not yet tested

**Test: Rotated Transforms**
- Single transform with b,c ≠ 0 (rotation/shear)
- Tested: 45° rotation matches
- Status: Basic rotation confirmed working

## Known Working Components

✅ **Verified Working:**
- Affine transformation (scale, translation, rotation)
- Single variation application (spherical, linear)
- Triangle editor coordinate display and interaction
- Iteration algorithm
- Point accumulation
- Density mapping
- Display coordinate conversion (with Y-flip)

## Architecture Summary

**Data Flow:**
1. **Triangle Editor** (Y-down display) → Affine coefficients (a,b,c,d,e,f)
   - User edits in Y-down coordinates (matches Apophysis Triangle Editor)
   - `from_triangle()` converts Y-down to standard affine coefficients
2. **Fractal Iteration**:
   - Apply affine: `p' = (a*x + b*y + e, c*x + d*y + f)` (standard math, no negation)
   - Apply variations: `p'' = Σ(weight_i * variation_i(p'))`
3. **Display Rendering**:
   - Convert to pixels: `pixel = center + fractal * scale`
   - Triangle Editor canvas applies Y-flip for visualization
   - Main render output uses standard pixel mapping

**Why Y-Negation ONLY in Triangle Display?**
- **Apophysis Triangle Editor**: Shows Y-down coordinates (UI convention)
- **Apophysis Iteration**: Uses standard mathematical affine transformation
- **Solution**: Y-negation only in `to_triangle()`/`from_triangle()`, NOT in shader
- **Result**: Triangle editor matches Apophysis display, iteration uses standard math

## References

- Apophysis 7X Source: https://github.com/xyrus02/apophysis-7x
- Fractal Flame Algorithm: Scott Draves (https://flam3.com/)
- Affine Transform: Standard 2D transformation matrix

## Next Steps

1. Test Simple preset with multiple transforms and variations
2. Test sinusoidal.fflame
3. Test complex multi-transform flames
4. Verify all 26 variations render identically to Apophysis
5. Document any variation-specific differences

## Notes

- Spherical variation formula: `p / (r² + ε)` where r² = x² + y²
- Linear variation formula: `p` (identity)
- Affine application order: `affine(p) → variations(p')` is correct
- **Y-negation ONLY for triangle visualization, NOT for iteration**
- Triangle coordinate conversion (`to_triangle`/`from_triangle`) handles Y-down display
- Shader affine transform uses standard mathematical formula (no Y-negation)
- Triangle Editor canvas already has Y-flip for visualization (line 117 in triangle_editor.rs)

## Debugging History

### 2025-10-25: Major Breakthrough
**Problem:** Fractals rendered differently than Apophysis despite matching affine coefficients.

**Investigation:**
1. Compared XML `coefs` values with Triangle Editor display
2. Found Apophysis displays (a, -b), (-c, d), (e, -f) for X, Y, O points
3. Realized Apophysis uses Y-down for triangle **display** but standard math for **iteration**
4. Our code had it backwards: Y-negation in shader, none in triangle conversion

**Solution:**
1. Added Y-negation to `to_triangle()`/`from_triangle()` for display compatibility
2. Removed Y-negation from shader `apply_affine()` to use standard math
3. Result: Triangle Editor matches Apophysis exactly, complex fractals render very similarly

**Key Insight:** Separate display coordinate system (Y-down) from iteration math (standard).
