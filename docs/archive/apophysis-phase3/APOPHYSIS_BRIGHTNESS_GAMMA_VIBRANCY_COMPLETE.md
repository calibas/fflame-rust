# Complete Brightness, Gamma, and Vibrancy Flow in Apophysis 7X

**Date:** 2025-01-05
**Status:** Complete authoritative reference for Phase 3.3 implementation

This document traces the EXACT flow of how brightness, gamma, and vibrancy values are converted from UI sliders through to final pixel rendering in Apophysis 7X.

---

## Step 1: UI Slider Values → Internal Values

### Gamma Slider
- **UI Range**: Typically 0-1000 (slider position)
- **Conversion**: `cp.Gamma := scrollGamma.Position / 100` (Adjust.pas:1033)
- **Example**: Slider at 400 → `cp.gamma = 4.0`

### Brightness Slider
- **UI Range**: Typically 0-1000 (slider position)
- **Conversion**: `cp.Brightness := scrollBrightness.Position / 100` (Adjust.pas:1041)
- **Example**: Slider at 100 → `cp.brightness = 1.0`

### Vibrancy Slider
- **UI Range**: 0-3000 (or 0-100 if `LimitVibrancy` enabled) (Adjust.pas:708)
- **Conversion**: `cp.Vibrancy := scrollVibrancy.Position / 100` (Adjust.pas:1025)
- **Example**: Slider at 100 → `cp.vibrancy = 1.0`

---

## Step 2: Image Maker Preparation (Setup Phase)

At the start of `MakeImage()` in ImageMaker.pas:408-458:

### Gamma Inversion
```pascal
if fcp.gamma = 0 then
  gamma := fcp.gamma
else
  gamma := 1 / fcp.gamma;
```
- **Important**: Gamma is INVERTED!
- Example: `cp.gamma = 4.0` → `gamma = 0.25` (used in power functions)

### Vibrancy Scaling
```pascal
vib := round(fcp.vibrancy * 256.0);
notvib := 256 - vib;
```
- Scaled to 0-256 range for integer math
- Example: `cp.vibrancy = 1.0` → `vib = 256`, `notvib = 0`

### Brightness Scaling Constants
```pascal
k1 := (fcp.Contrast * BRIGHT_ADJUST * fcp.brightness * 268 * PREFILTER_WHITE) / 256.0;
k2 := (FOversample * FOversample) / (fcp.Contrast * area * fcp.White_level * sample_density);
```

Where:
- `BRIGHT_ADJUST = 2.3` (ControlPoint.pas:39)
- `PREFILTER_WHITE = (1 shl 26)` = 67,108,864 (ControlPoint.pas:37)

**Key Point**: Brightness is multiplied by 2.3 before being used!

### Logarithmic Lookup Table Creation
```pascal
for i := 0 to 1024 do begin
  if i = 0 then lsa[0] := 0
  else lsa[i] := (k1 * log10(1 + fcp.White_level * i * k2)) / (fcp.White_level * i);
end;
```

This creates a lookup table (`lsa`) that converts bucket hit counts to brightness scaling factors. This incorporates **brightness** via `k1`.

---

## Step 3: Pixel-by-Pixel Rendering (Per-Pixel Phase)

For each output pixel at position `(j, i)`:

### Stage 3A: Gather Filtered Bucket Data

The code reads nearby buckets (spatial filtering) and accumulates their color data.

For the simple case (no density estimation), at ImageMaker.pas:512-522:

```pascal
bucket := GetBucket(bx, by);
if bucket.count < 1024 then
  ls := lsa[Round(bucket.count)] / PREFILTER_WHITE
else
  ls := (k1 * log10(1 + fcp.White_level * bucket.count * k2)) /
        (fcp.White_level * bucket.count) / PREFILTER_WHITE;

fp[0] := ls * bucket.Red;    // Accumulated red from palette lookups
fp[1] := ls * bucket.Green;  // Accumulated green from palette lookups
fp[2] := ls * bucket.Blue;   // Accumulated blue from palette lookups
fp[3] := ls * bucket.Count * fcp.white_level;  // Weighted count (density)
```

**Key Point**: At this stage, `ls` is a **brightness scaling factor from the logarithmic curve** that includes the effect of **brightness** (via `k1`). This `ls` is applied to the accumulated RGB values from the palette.

After this step:
- `fp[0..2]` = brightness-scaled color values (still in palette color space)
- `fp[3]` = weighted density value

### Stage 3B: Apply Gamma to Density (Alpha Channel)

At ImageMaker.pas:591-597 (transparency mode shown, non-transparency is similar at line 647):

```pascal
if (fp[3] > 0.0) then begin
  if fp[3] <= fcp.gamma_threshold then begin
    frac := fp[3] / fcp.gamma_threshold;
    alpha := (1 - frac) * fp[3] * funcval + frac * power(fp[3], gamma);
  end
  else
    alpha := power(fp[3], gamma);
```

**Key Point**: **Gamma is applied to the density/alpha channel** (`fp[3]`), not directly to color!
- `gamma` (the inverted value) is used as the exponent
- Lower gamma values (UI > 1) darken, higher gamma values (UI < 1) brighten

### Stage 3C: Calculate Vibrancy-Scaled Brightness

At ImageMaker.pas:599:

```pascal
ls := vib * alpha / fp[3];
```

**IMPORTANT**: The variable `ls` is now **REUSED** and OVERWRITTEN with a new meaning!
- Previously: `ls` was the logarithmic brightness scaling factor
- Now: `ls` is the vibrancy-weighted alpha-to-color scaling factor

Expanding this:
```
ls = vibrancy * 256 * power(density, 1/gamma) / raw_density
```

### Stage 3D: Blend Old and New Color Algorithms

At ImageMaker.pas:612-621:

```pascal
if (notvib > 0) then begin
  ri := Round(ls * fp[0] + notvib * power(fp[0], gamma));
  gi := Round(ls * fp[1] + notvib * power(fp[1], gamma));
  bi := Round(ls * fp[2] + notvib * power(fp[2], gamma));
end
else begin
  ri := Round(ls * fp[0]);
  gi := Round(ls * fp[1]);
  bi := Round(ls * fp[2]);
end;
```

Where:
- `ls * fp[0]` = **NEW algorithm**: brightness-scaled color × (vibrancy × gamma-corrected-alpha / density)
- `notvib * power(fp[0], gamma)` = **OLD algorithm**: (1-vibrancy) × gamma-corrected color

**Key Points**:
1. The "new algorithm" applies gamma to brightness/alpha SEPARATELY from color
2. The "old algorithm" applies gamma DIRECTLY to each color channel
3. Vibrancy blends between these two approaches

Let's expand the new algorithm component:
```
NEW = ls * fp[0]
    = (vib * alpha / fp[3]) * fp[0]
    = (vib * power(fp[3], 1/gamma) / fp[3]) * (brightness_scaled_color)
```

Since `fp[0]` already has brightness applied (from Stage 3A), the new algorithm is:
```
NEW = vibrancy × gamma_corrected_alpha × brightness_scaled_color / raw_density
```

---

## Complete Formula

For a single color channel, the final value is:

```
final = vibrancy_component + old_algorithm_component

vibrancy_component = vibrancy × (gamma_corrected_density / raw_density) × brightness_scaled_palette_color

old_algorithm_component = (1 - vibrancy) × power(brightness_scaled_palette_color, 1/gamma)
```

Where:
- `brightness_scaled_palette_color` = result of Stage 3A (logarithmic curve with k1)
- `gamma_corrected_density` = `power(raw_density, 1/gamma)`
- `vibrancy` is scaled 0-256
- `gamma` is the inverted UI value (1/UI_gamma)

---

## Summary of Interactions

### Brightness
- Enters via `k1` calculation (multiplied by 2.3)
- Applied logarithmically in Stage 3A through the lookup table `lsa`
- Affects BOTH the new and old algorithm paths
- Higher brightness = brighter image (logarithmic scaling)

### Gamma
- Inverted: `gamma = 1 / UI_value`
- Applied to density in Stage 3B
- Applied to color channels in old algorithm (Stage 3D)
- Lower UI values → higher gamma exponent → brighter image
- Higher UI values → lower gamma exponent → darker image

### Vibrancy
- Scaled by 256
- Controls blend between new (separate brightness/color gamma) and old (direct color gamma) algorithms
- `vibrancy = 0`: Pure old algorithm (gamma applied to colors)
- `vibrancy = 1`: Pure new algorithm (gamma applied to brightness only)
- `vibrancy > 1`: Over-amplified new algorithm (hyper-vibrant colors)

### Key Interaction
When vibrancy = 1 (pure new algorithm):
1. Brightness affects color linearly (from Stage 3A)
2. Gamma affects only the alpha/brightness multiplier (from Stage 3B)
3. Result: Colors stay saturated even in dim areas (more "vibrant")

When vibrancy = 0 (pure old algorithm):
1. Brightness affects color linearly (from Stage 3A)
2. Gamma affects the color values directly (raising each channel to power)
3. Result: Dim areas lose saturation (colors become more pastel/washed out)

---

## Implementation Notes for Our Renderer

### Our Accumulation Structure
We already match Apophysis in Stage 1:
```rust
// Accumulation buffer (Rgba16Float)
color.r = sum of red palette values       // Like bucket.Red
color.g = sum of green palette values     // Like bucket.Green
color.b = sum of blue palette values      // Like bucket.Blue
color.a = hit count × 0.01                // Like bucket.Count (scaled)
```

### What We Need to Implement in Tonemap Shader

**Stage 2: Brightness Lookup (currently missing)**
- Add `brightness: f32` parameter to FractalConfig
- Calculate logarithmic brightness curve in shader
- Formula: `lsa[i] = (k1 * log10(1 + white_level * i * k2)) / (white_level * i)`
- Where: `k1 = (contrast * 2.3 * brightness * 268 * PREFILTER_WHITE) / 256.0`

**Stage 3A: Apply Brightness to Colors (currently using simple exposure)**
- Replace: `color *= exposure`
- With: `color *= brightness_scale` (from logarithmic curve)

**Stage 3B: Gamma to Density (partially implemented)**
- ✅ We invert gamma: `1.0 / gamma`
- ✅ Apply to alpha/density
- Need to verify formula matches exactly

**Stage 3C: Vibrancy-Weighted Multiplier (currently incorrect)**
- Calculate: `ls = vib * alpha / density`
- This is the key vibrancy magic!

**Stage 3D: Vibrancy Blend (currently incorrect)**
- New algorithm: `ls * color` (where color is brightness-scaled)
- Old algorithm: `notvib * pow(color, 1/gamma)` (where color is brightness-scaled)
- Blend: `new + old`

---

## Constants Reference

From Apophysis source:
- `BRIGHT_ADJUST = 2.3` (ControlPoint.pas:39)
- `PREFILTER_WHITE = (1 shl 26)` = 67,108,864 (ControlPoint.pas:37)
- Default `contrast = 1.0`
- Default `white_level = 200.0`
- Default `oversample = 2`

These constants are used in the k1/k2 calculations for the brightness lookup table.
