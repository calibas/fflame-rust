# Visual Baseline Testing vs Apophysis

## Goal
Establish visual baseline to verify that fractal patterns match Apophysis 7X reference implementation.

## Coordinate System Analysis

### Key Findings

**Triangle Editor Coordinate System:**
- Both Apophysis and this app use **Y-up** display in Triangle Editor
- Identity transform shows: X:(1,0), Y:(0,1), O:(0,0) forming an L-shape
- Y-axis goes UP as you move UP in the editor (mathematical convention)

**Fractal Math Coordinate System:**
- Internal fractal calculations use **Y-up** (standard mathematical coordinates)
- Affine transform: `x' = a*x + b*y + e`, `y' = -(c*x + d*y + f)` **← Y is negated!**
- Variations operate on the post-affine coordinates (after Y negation)

**Display Coordinate System:**
- Screen rendering uses standard pixel mapping (no additional Y-flip needed)
- Direct conversion: `pixel = center + transformed * scale`
- The affine Y-negation handles the coordinate system difference

**Critical Insight:**
- Only **one Y-negation** is needed: in the affine transform application
- Triangle Editor shows Y-up coordinates (matches mathematical convention)
- Affine transform negates Y during application (matches Apophysis behavior)
- Display uses standard pixel mapping (no flip needed)
- Moving triangle RIGHT in editor → fractal moves LEFT on screen (coordinate system translation)
- Moving triangle UP in editor → fractal moves UP on screen (after affine negation)

### Implementation

**The Fix:**
```wgsl
// In apply_affine() function (both 2D and 3D shaders):
fn apply_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        -(xform.c * p.x + xform.d * p.y + xform.f)  // Y is negated
    );
}
```

**Apophysis Reference:**
- Source: https://github.com/xyrus02/apophysis-7x
- Uses same principle: Y-up fractal math, Y-down display rendering

## Test Results

### ✅ Single Transform, Single Variation Tests

**Test: spherical.flame**
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

**Test: linear.flame**
- **Config**: Single transform, a=d≈1.0, linear=1.0
- **Result**: ✅ **VISUAL MATCH** (after display Y-flip fix)
- **Rotation test**: 45° clockwise rotation matches in both apps
- **Translation test**: Moving transform right in editor → fractal moves left on screen (matches Apophysis)
- **Scale test**: Increasing transform scale 90% → 150% matches behavior
- **Conclusion**: Linear variation works correctly with display Y-flip

**Settings:**
```json
{
  "a": 0.9958939, "b": 0.0, "c": 0.0, "d": 0.9958939, "e": 0.0, "f": 0.0,
  "variations": { "linear": 1.0 }
}
```

### 🔍 Pending Tests

**Test: sinusoidal.flame**
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
1. **Triangle Editor** (Y-up) → Affine coefficients (a,b,c,d,e,f)
2. **Fractal Iteration**:
   - Apply affine with Y-negation: `p' = (a*x + b*y + e, -(c*x + d*y + f))`
   - Apply variations: `p'' = Σ(weight_i * variation_i(p'))`
3. **Display Rendering**:
   - Convert to pixels: `pixel = center + fractal * scale`
   - Render to screen

**Why Y-Negation in Affine Transform?**
- **Triangle Editor Y-up**: Matches mathematical convention (X right, Y up)
- **Apophysis Coordinate System**: Uses Y-up in editor but negates Y in affine application
- **Solution**: Single Y-negation in affine transform matches Apophysis exactly
- **No display flip needed**: Fractal coordinates map directly to screen pixels

## References

- Apophysis 7X Source: https://github.com/xyrus02/apophysis-7x
- Fractal Flame Algorithm: Scott Draves (https://flam3.com/)
- Affine Transform: Standard 2D transformation matrix

## Next Steps

1. Test Simple preset with multiple transforms and variations
2. Test sinusoidal.flame
3. Test complex multi-transform flames
4. Verify all 26 variations render identically to Apophysis
5. Document any variation-specific differences

## Notes

- Spherical variation formula: `p / (r² + ε)` where r² = x² + y²
- Linear variation formula: `p` (identity)
- Affine application order: `affine(p) → variations(p')` is correct
- **Y-negation in affine transform is the ONLY coordinate conversion needed**
- No negation in triangle coordinate conversion required
- No display Y-flip required
