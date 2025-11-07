# Gamma and Brightness Verification

**Date:** 2025-01-05
**Updated:** 2025-01-05 (Deep dive into Apophysis pipeline)
**Status:** Complete analysis for Phase 3.3 vibrancy implementation

## Executive Summary

**CRITICAL DISCOVERY:** The variable `ls` is **reused for TWO different purposes** in Apophysis:
1. **Stage 3A:** Brightness scaling factor from logarithmic lookup table
2. **Stage 3C:** Vibrancy-weighted alpha multiplier (overwrites previous value!)

All three UI sliders (Gamma, Brightness, Vibrancy) divide by 100 before storage.

## Apophysis Implementation

### UI Slider Conversion
From Adjust.pas:1033, 1041, and vibrancy:
```pascal
cp.Gamma := scrollGamma.Position / 100;       // Range: 0-500 → 0.0-5.0
cp.Brightness := ScrollBrightness.Position / 100;  // Range: 0-500 → 0.0-5.0
cp.Vibrancy := scrollVibrancy.Position / 100;      // Range: 0-3000 → 0.0-30.0
```
**All three sliders divide by 100 before storing.**

### The Complete Apophysis Pipeline (4 Stages)

#### Stage 1: Iteration → Histogram (Accumulation Buffer)
From RenderingImplementation.pas:319-336:
```pascal
Bucket := @buckets[Round(bhs * py)][Round(bws * px)];
MapColor := @ColorMap[Round(p.c * 255)];

Bucket.Red   := Bucket.Red   + MapColor.Red;
Bucket.Green := Bucket.Green + MapColor.Green;
Bucket.Blue  := Bucket.Blue  + MapColor.Blue;
Bucket.Count := Bucket.Count + 1;
```

**Critical insight:** Buckets accumulate **COLORED values**, not just hit counts!
- `Bucket.Red/Green/Blue`: Sum of all palette RGB values that hit this location
- `Bucket.Count`: Hit count for spatial location

Bucket structure (RenderingCommon.pas:31-43):
```pascal
TBucket = Record
  Red, Green, Blue, Count: Double;  // 64-bit build (or Single for 32-bit)
end;
```

#### Stage 2: Build Brightness Lookup Table
From ImageMaker.pas:450:
```pascal
k1 := (fcp.Contrast * BRIGHT_ADJUST * fcp.brightness * 268 * PREFILTER_WHITE) / 256.0;
```
Where `BRIGHT_ADJUST = 2.3` (ControlPoint.pas:39)

From ImageMaker.pas:457:
```pascal
lsa[i] := (k1 * log10(1 + fcp.White_level * i * k2)) / (fcp.White_level * i);
```

This creates a **logarithmic brightness curve**. The 2.3 multiplier means:
- Brightness slider value of 1.0 actually gets scaled to 2.3 internally
- Logarithmic curve is steeper/brighter than without this constant

#### Stage 3A: Apply Brightness to Palette Colors
From ImageMaker.pas:560-563:
```pascal
fp[0] := lsa[Round(bucket.Red)] * bucket.Red;       // brightness-scaled red
fp[1] := lsa[Round(bucket.Green)] * bucket.Green;   // brightness-scaled green
fp[2] := lsa[Round(bucket.Blue)] * bucket.Blue;     // brightness-scaled blue
fp[3] := bucket.Count;                               // raw density
```

**First use of `ls`:** The `lsa[]` lookup table values are brightness scaling factors.
**Critical:** `fp[0..2]` are now **brightness-scaled palette colors**. Both algorithms in Stage 3D operate on these brightness-scaled values!

#### Stage 3B: Apply Gamma to Density
From ImageMaker.pas:408-411:
```pascal
if fcp.gamma = 0 then
  gamma := fcp.gamma
else
  gamma := 1 / fcp.gamma;
```
**Gamma is inverted (1/gamma) before use!**

From ImageMaker.pas:597:
```pascal
alpha := power(fp[3], gamma);  // Gamma-corrected density
```

**Effect:**
- gamma < 1: Results in high exponent (1/0.5 = 2.0), darkens
- gamma = 1: No change (1/1 = 1.0)
- gamma > 1: Results in low exponent (1/2 = 0.5), brightens

#### Stage 3C: Calculate Vibrancy-Weighted Brightness (ls REUSED!)
From ImageMaker.pas:412-413:
```pascal
vib := round(fcp.vibrancy * 256.0);
notvib := 256 - vib;
```

From ImageMaker.pas:599:
```pascal
ls := vib * alpha / fp[3];  // OVERWRITES previous ls from Stage 3A!
```

**Second use of `ls`:** Vibrancy-weighted alpha multiplier
- `ls = (vibrancy × 256) × power(density, 1/gamma) / density`
- The ratio `power(density, 1/gamma) / density` creates the vibrancy effect
- Boosts colors in dim areas more than the old algorithm would

#### Stage 3D: Vibrancy Blend
From ImageMaker.pas:613-615:
```pascal
if (notvib > 0) then begin
  ri := Round(ls * fp[0] + notvib * power(fp[0], gamma));
  gi := Round(ls * fp[1] + notvib * power(fp[1], gamma));
  bi := Round(ls * fp[2] + notvib * power(fp[2], gamma));
end else begin
  ri := Round(ls * fp[0]);
  gi := Round(ls * fp[1]);
  bi := Round(ls * fp[2]);
end;
```

Breaking down the blend:
- **New algorithm:** `ls * fp[0]` = `(vib × alpha/fp[3]) × brightness_scaled_color`
- **Old algorithm:** `notvib * power(fp[0], gamma)` = `notvib × power(brightness_scaled_color, 1/gamma)`

**Both algorithms work on brightness-scaled values from Stage 3A!**

---

## Our Accumulation Method vs Apophysis

### Our Pipeline
We use **direct accumulation** similar to Apophysis buckets:

1. **Iteration → Accumulation Buffer** (compute shader):
   - Each hit adds palette RGB color to texture pixel: `color.rgb += palette_color`
   - Density accumulated separately in alpha channel: `color.a += 0.01`
   - **Same as Apophysis:** We accumulate colored values, not just hit counts!

2. **Accumulation → Display** (tonemap shader):
   - Read accumulated texture (RGB + density)
   - Apply tone mapping (brightness, gamma, vibrancy)
   - Output final pixel color

### Key Difference: When Brightness is Applied

**Apophysis:**
- Stage 1: Accumulate bucket colors
- Stage 3A: Apply brightness **during tone mapping** via `lsa[]` lookup table

**Our Implementation:**
- Currently: Apply exposure **during tone mapping** as simple multiplication
- No logarithmic brightness curve with BRIGHT_ADJUST = 2.3
- No separate brightness parameter (we use exposure instead)

### Mapping to Our System

Our accumulation texture structure:
```rust
// Accumulation buffer (Rgba16Float)
color.r = sum of red palette values       // Like bucket.Red
color.g = sum of green palette values     // Like bucket.Green
color.b = sum of blue palette values      // Like bucket.Blue
color.a = hit count × 0.01                // Like bucket.Count (scaled)
```

Our tone mapping shader should replicate Stages 2-3D:
- Stage 2: Build brightness lookup (or approximate with formula)
- Stage 3A: Apply brightness to accumulated colors
- Stage 3B: Apply gamma to density
- Stage 3C: Calculate vibrancy-weighted multiplier
- Stage 3D: Blend old/new algorithms

---

## Current Status & Next Steps

### What We Have
- ✅ Accumulation buffer stores colored values (like Apophysis buckets)
- ✅ Density tracked separately in alpha channel
- ✅ Basic tone mapping in place

### What Needs Implementation
To match Apophysis exactly, we need to replicate Stages 2-3D in our tonemap shader:

1. **Stage 2: Brightness Lookup** (currently missing)
   - Add `brightness: f32` parameter to FractalConfig
   - Calculate logarithmic brightness curve in shader
   - Formula: `lsa[i] = (k1 * log10(1 + white_level * i * k2)) / (white_level * i)`
   - Where: `k1 = (contrast * 2.3 * brightness * 268 * PREFILTER_WHITE) / 256.0`

2. **Stage 3A: Apply Brightness to Colors** (currently using simple exposure)
   - Replace: `color *= exposure`
   - With: `color *= brightness_scale` (from logarithmic curve)

3. **Stage 3B: Gamma to Density** (partially implemented)
   - ✅ We invert gamma: `1.0 / gamma`
   - ✅ Apply to alpha/density
   - Need to verify formula matches exactly

4. **Stage 3C: Vibrancy-Weighted Multiplier** (currently incorrect)
   - Calculate: `ls = vib * alpha / density`
   - This is the key vibrancy magic!

5. **Stage 3D: Vibrancy Blend** (currently incorrect)
   - New algorithm: `ls * color` (where color is brightness-scaled)
   - Old algorithm: `notvib * pow(color, 1/gamma)` (where color is brightness-scaled)
   - Blend: `new + old`

---

## Summary

This document traces the complete Apophysis pipeline for brightness, gamma, and vibrancy application. Key discoveries:

1. **`ls` variable is reused** for two different purposes (brightness lookup, then vibrancy multiplier)
2. **All UI sliders divide by 100** before storage
3. **Gamma is inverted** (1/gamma) before use in power functions
4. **Brightness uses BRIGHT_ADJUST = 2.3** constant and logarithmic mapping
5. **Buckets accumulate colored values**, not just hit counts
6. **Both vibrancy algorithms** operate on brightness-scaled palette colors

Next step: Wait for full Apophysis implementation report to guide our shader updates.
