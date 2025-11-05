# Parameter Type Migration Guide

## Overview

This document provides a systematic approach to migrating variation parameters from generic types (Float, Integer) to semantic types (Boolean, Enum, UnlimitedFloat) for improved UX and Apophysis compatibility.

**Status:** Phase 2.2 - Ready for migration (infrastructure complete)

## Available ParamTypes

### 1. Float (bounded)
- **Use for:** Continuous values with practical min/max bounds
- **UI:** Slider with defined range
- **Example:** `blob.high` (0.0 to 2.0)

### 2. UnlimitedFloat
- **Use for:** Unbounded continuous values, multipliers, coordinates
- **UI:** DragValue with full f32 range (-3.4E38 to +3.4E38)
- **Example:** `pre_falloff2.mul_x`, `crop.left`

### 3. Integer
- **Use for:** Discrete numeric values with range
- **UI:** Integer slider
- **Example:** `julian.power` (-10 to 10), `ngon.sides` (3 to 20)

### 4. Boolean
- **Use for:** Binary flags (on/off, enable/disable)
- **UI:** Checkbox
- **Example:** `crop.zero` (0 or 1)

### 5. Angle
- **Use for:** Angular values in degrees
- **UI:** Slider with degree suffix, 0-360 range
- **Example:** `radial_blur.angle`

### 6. Enum { choices }
- **Use for:** Discrete modes/options with named choices
- **UI:** ComboBox dropdown with labels
- **Example:** `pre_falloff2.type` (Linear/Radial/Gaussian)

## Migration Checklist Per Parameter

For each parameter, answer these questions:

### 1. Semantic Meaning
- [ ] Is it a flag/switch? → **Boolean**
- [ ] Is it a mode selection? → **Enum**
- [ ] Is it an angle? → **Angle**
- [ ] Is it discrete with range? → **Integer**
- [ ] Is it continuous with bounds? → **Float**
- [ ] Is it unbounded? → **UnlimitedFloat**

### 2. Check Apophysis Source
- [ ] What type does Apophysis use?
- [ ] What values are actually used in practice?
- [ ] Are there constraints in the calculation?

### 3. User Experience
- [ ] What's the most intuitive control?
- [ ] Should users be able to enter extreme values?
- [ ] Are negative values meaningful?

## Systematic Migration Plan

### Phase 1: Low-Hanging Fruit (Boolean/Enum)

Convert obvious flags and mode switches first (high impact, low risk):

#### Boolean Candidates
- [ ] `crop.zero` (0 or 1)
- [ ] `pre_crop.zero` (0 or 1)
- [ ] `post_crop.zero` (0 or 1)
- [ ] `rectangles.x` (0 or 1)
- [ ] `rectangles.y` (0 or 1)
- [ ] `falloff2.invert` (0 or 1) ✅ DONE
- [ ] `post_falloff2.invert` (0 or 1)
- [ ] `pre_falloff2.invert` (0 or 1)
- [ ] Any other 0/1 flags discovered

#### Enum Candidates
- [ ] `falloff2.type` (0=linear, 1=radial, 2=gaussian) ✅ DONE
- [ ] `pre_falloff2.type` (0=linear, 1=radial, 2=gaussian)
- [ ] `post_falloff2.type` (0=linear, 1=radial, 2=gaussian)
- [ ] Any other mode switches discovered

### Phase 2: Coordinate/Multiplier Parameters (UnlimitedFloat)

Review parameters that represent unbounded values:

#### Multiplier Parameters
- [ ] `pre_falloff2.mul_x`
- [ ] `pre_falloff2.mul_y`
- [ ] `pre_falloff2.mul_z`
- [ ] `pre_falloff2.mul_c`
- [ ] `post_falloff2.mul_x`
- [ ] `post_falloff2.mul_y`
- [ ] `post_falloff2.mul_z`
- [ ] `post_falloff2.mul_c`

#### Coordinate Parameters
- [ ] `pre_falloff2.x0`
- [ ] `pre_falloff2.y0`
- [ ] `pre_falloff2.z0`
- [ ] `post_falloff2.x0`
- [ ] `post_falloff2.y0`
- [ ] `post_falloff2.z0`
- [ ] `crop.left`
- [ ] `crop.top`
- [ ] `crop.right`
- [ ] `crop.bottom`
- [ ] `pre_crop.*` (same as crop)
- [ ] `post_crop.*` (same as crop)

### Phase 3: Review Remaining Float Parameters

For each Float parameter, decide if it should:
- Stay as Float (keep current bounds)
- Expand bounds but stay as Float
- Convert to UnlimitedFloat

#### Priority List (26 Parameterized Variations)

**Core Variations:**
- [ ] julian (2 params: power=Integer ✓, dist=Float)
- [ ] blob (3 params: high, low, waves)
- [ ] pdj (4 params: a, b, c, d)
- [ ] wedge (4 params: angle, hole, count, swirl)
- [ ] epispiral (4 params: thickness, n, holes, waves)
- [ ] bwraps (3 params: cellsize, space, gain)
- [ ] juliascope (2 params: power=Integer ✓, dist=Float)
- [ ] julia3dz (1 param: power=Integer ✓)

**Extended Variations:**
- [ ] rings2 (1 param: val)
- [ ] fan2 (2 params: x, y)
- [ ] curl (2 params: c1, c2)
- [ ] curl3d (3 params: cx, cy, cz)
- [ ] radial_blur (1 param: angle=Angle ✓)
- [ ] blur_pixelize (1 param: size)
- [ ] rectangles (2 params: x=Boolean?, y=Boolean?)
- [ ] splits (2 params: x, y)
- [ ] separation (4 params: x, y, xinside, yinside)
- [ ] ngon (4 params: sides=Integer ✓, power, circle, corners)
- [ ] mobius (8 params: re_a, im_a, re_b, im_b, re_c, im_c, re_d, im_d)
- [ ] crop (6 params: left, top, right, bottom, scatter_area, zero=Boolean?)
- [ ] auger (4 params: freq, weight, scale, sym)
- [ ] pre_bwraps (5 params: cellsize, space, gain, inner_twist, outer_twist)
- [ ] post_bwraps (5 params: same as pre_bwraps)
- [ ] pre_crop (6 params: same as crop)
- [ ] post_crop (6 params: same as crop)
- [ ] falloff2 (11 params: partially done ✅)
- [ ] pre_falloff2 (11 params: same as falloff2)
- [ ] post_falloff2 (11 params: same as falloff2)

## Migration Procedure

### For Each Variation:

1. **Document Current State**
   ```bash
   # Note current parameter types and ranges in spreadsheet/notes
   ```

2. **Check Apophysis Source**
   - Look at Pascal source code
   - Check parameter constraints
   - Note actual usage patterns

3. **Make Changes**
   ```rust
   // Example: Converting crop.zero from Integer to Boolean
   VariationParameter {
       name: "zero".to_string(),
       display_name: "Zero".to_string(),
       param_type: ParamType::Boolean,  // Changed from Integer
       default_value: 0.0,
       min_value: None,  // Changed from Some(0.0)
       max_value: None,  // Changed from Some(1.0)
   },
   ```

4. **Build and Test**
   ```bash
   cargo build --lib --release
   # Run app and test the specific variation
   ```

5. **Commit Per Variation (or Small Batches)**
   ```bash
   git add -A
   git commit -m "FEAT: Convert crop parameters to semantic types"
   ```

## Helper Function Usage

### Creating Enum Parameters

```rust
// Simple enum with choices
param_type: ParamType::enum_choices(&["Linear", "Radial", "Gaussian"])

// Enum with verbose labels
param_type: ParamType::enum_choices(&[
    "None",
    "Fade In",
    "Fade Out",
    "Fade Both"
])
```

### Boolean Parameters

```rust
VariationParameter {
    name: "invert".to_string(),
    display_name: "Invert".to_string(),
    param_type: ParamType::Boolean,
    default_value: 0.0,
    min_value: None,  // Not used for Boolean
    max_value: None,  // Not used for Boolean
}
```

### UnlimitedFloat Parameters

```rust
VariationParameter {
    name: "mul_x".to_string(),
    display_name: "Mul X".to_string(),
    param_type: ParamType::UnlimitedFloat,
    default_value: 1.0,
    min_value: None,  // Full f32 range
    max_value: None,
}
```

## Testing Checklist Per Variation

After converting parameters:

- [ ] Build succeeds without errors
- [ ] UI renders correct control type (slider/checkbox/dropdown/dragvalue)
- [ ] Default values work correctly
- [ ] Parameter changes update fractal
- [ ] Undo/redo works
- [ ] Config export/import preserves values
- [ ] Values clamp appropriately (for UnlimitedFloat: f32 range)

## Common Patterns

### Pattern 1: Crop-Style Variations
All crop variations (crop, pre_crop, post_crop) should have identical types:
- `left`, `top`, `right`, `bottom`: **UnlimitedFloat** (coordinates)
- `scatter_area`: Review range (possibly Float or UnlimitedFloat)
- `zero`: **Boolean**

### Pattern 2: Falloff-Style Variations
Both pre_falloff2 and post_falloff2 should match:
- `scatter`, `mindist`: Review (Float or UnlimitedFloat?)
- `mul_x`, `mul_y`, `mul_z`, `mul_c`: **UnlimitedFloat** (multipliers)
- `x0`, `y0`, `z0`: **UnlimitedFloat** (coordinates)
- `invert`: **Boolean**
- `type`: **Enum** (Linear/Radial/Gaussian)

### Pattern 3: Complex Number Parameters
Mobius and similar variations with re/im pairs:
- All real/imaginary parts: Review range (possibly UnlimitedFloat)

## Decision Framework

When in doubt, use this hierarchy:

1. **Is it 0 or 1 only?** → Boolean
2. **Is it a named mode/option?** → Enum
3. **Is it an angle?** → Angle
4. **Must it be discrete?** → Integer (with range)
5. **Does it need to be unbounded?** → UnlimitedFloat
6. **Does it have practical bounds?** → Float (with min/max)

## Batch Commit Strategy

Group related changes:

```bash
# Batch 1: All Boolean conversions
git commit -m "FEAT: Convert all boolean flags to ParamType::Boolean"

# Batch 2: All Enum conversions
git commit -m "FEAT: Convert mode selectors to ParamType::Enum"

# Batch 3: Falloff variations
git commit -m "FEAT: Convert falloff2 parameters to semantic types"

# Batch 4: Crop variations
git commit -m "FEAT: Convert crop parameters to semantic types"
```

## Progress Tracking

**Completed:**
- ✅ falloff2.invert → Boolean
- ✅ falloff2.type → Enum

**Next Priority:**
- pre_falloff2 (match falloff2 changes)
- post_falloff2 (match falloff2 changes)
- All crop variations (left/top/right/bottom, zero)
- rectangles (x, y)

**Total Progress: 2/100+ parameters converted**

## Notes

- Don't rush - review each parameter carefully
- Test after each batch
- Document any unusual decisions
- Update this file as patterns emerge
- Coordinate with any Apophysis XML import work

---

**Last Updated:** 2025-11-04
**Status:** Ready for systematic migration
**Owner:** TBD
