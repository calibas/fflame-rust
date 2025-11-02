# Variation Testing Plan

## Status: Execution Order Fixed ✅

With the 4-phase execution model now implemented, we can proceed to test individual variations.

## Testing Strategy

For each variation, create a simple test case:
1. Single transform with only that variation active
2. Non-symmetrical affine to reveal directional differences
3. Compare rendering with Apophysis side-by-side

## Test Affine (Non-Symmetrical)
```
a=0.34284, c=0.564847, b=-0.564847, d=0.34284, e=0, f=0
```
This is a rotation + scale that reveals any coordinate ordering issues.

## Variations to Test

### Already Verified ✅
- ✅ Linear (variation 0)
- ✅ Spherical (variation 2)
- ✅ Diamond (variation 11) - Fixed atan2(x,y) convention

### Fixed atan2(x,y) - Need Visual Verification
- ⚠️ Polar (variation 5)
- ⚠️ Handkerchief (variation 6)
- ⚠️ Heart (variation 7)
- ⚠️ Disc (variation 8)
- ⚠️ Spiral (variation 9)
- ⚠️ Hyperbolic (variation 10)
- ⚠️ Ex (variation 12)
- ⚠️ Blob (variation 25)

### Use Standard atan2(y,x) - Need Visual Verification
- ⚠️ Julia (variation 13)
- ⚠️ JuliaN (variation 24)

### Not Yet Tested
- ❓ Sinusoidal (variation 1)
- ❓ Swirl (variation 3)
- ❓ Horseshoe (variation 4)
- ❓ Bent (variation 14)
- ❓ Waves (variation 15)

### 3D Variations
- ❓ ZCone (variation 16) - Inline implementation
- ❓ Flatten (variation 17) - Inline implementation, Post phase
- ❓ Hemisphere (variation 18)
- ❓ PreRotateX (variation 19) - Pre phase
- ❓ PreRotateY (variation 20) - Pre phase
- ❓ PostRotateX (variation 21) - Post phase
- ❓ PostRotateY (variation 22) - Post phase
- ❓ ZScale (variation 23) - Inline implementation, Post phase

## Testing Workflow

1. **Create test XML** with single variation:
```xml
<flame name="test_variation_X">
  <xform weight="1" color="0" VARIATION="1.0" coefs="0.34284 0.564847 -0.564847 0.34284 0 0" />
  <palette count="256" format="RGB">
    ... (simple gradient) ...
  </palette>
</flame>
```

2. **Load in Apophysis 7X** and render at 800x600, 10M iterations

3. **Load in our implementation** and render at same resolution/iterations

4. **Compare visually** - should be pixel-perfect match

5. **If mismatch found:**
   - Check variation formula against Apophysis source
   - Check atan2 convention
   - Check sign of coordinates
   - Check precalculation (r, θ, φ)

## Quick Test Cases

### Test 1: Sinusoidal (Simple)
```xml
<xform weight="0.5" color="0" sinusoidal="1" coefs="1 0 0 1 0 0" />
<xform weight="0.5" color="0" linear="0.5" coefs="0.5 0 0 0.5 0 0" />
```

### Test 2: Swirl (Uses atan2 and trig)
```xml
<xform weight="1" color="0" swirl="1" coefs="0.34284 0.564847 -0.564847 0.34284 0 0" />
```

### Test 3: Pre-Rotation (3D, Phase 1)
```xml
<xform weight="1" color="0" linear="0.5" pre_rotate_y="0.5" coefs="1 0 0 1 0 0" />
```

### Test 4: Post-Rotation + Flatten (3D, Phase 4)
```xml
<xform weight="1" color="0" linear="0.5" post_rotate_x="0.3" flatten="1" coefs="1 0 0 1 0 0" />
```

## Common Issues to Look For

1. **Coordinate sign errors** - Check if X or Y is negated
2. **atan2 argument order** - Core uses atan2(x,y), plugins use atan2(y,x)
3. **Precalculation errors** - r, θ, φ calculated from wrong coordinates
4. **Phase errors** - Variation in wrong execution phase (should be fixed now)
5. **Direct vs weighted** - Pre/post should modify directly, not weighted sum (should be fixed now)

## Success Criteria

A variation is **verified correct** when:
- ✅ Renders pixel-perfect match with Apophysis (at same iterations/resolution)
- ✅ Works correctly in both 2D and 3D modes (if applicable)
- ✅ Maintains correctness with different affine transforms
- ✅ Combines correctly with other variations

## Next Steps After All Variations Verified

1. Test complex multi-transform flames
2. Test preset library flames
3. Performance benchmarking
4. Document any remaining known differences
