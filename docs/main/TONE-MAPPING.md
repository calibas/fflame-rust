# Tone Mapping System

This document describes the tone mapping parameters and their effects on the final rendered image.

## Overview

All tone mapping parameters are **post-processing** - they're applied in the tonemap shader after the fractal has been rendered to the accumulation buffer. The compute shader renders raw density + color sums, and the tonemap shader converts them to displayable RGB.

**Shader location:** [shaders/tonemap.wgsl](../../shaders/tonemap.wgsl)

## Processing Pipeline

The tonemap shader processes pixels in this order:

1. **Brightness** - Logarithmic scaling based on Apophysis algorithm (Stage 3A)
2. **Gamma** - Applied to density to calculate alpha (Stage 3B)
3. **Vibrancy** - Blends between old/new color algorithms (Stage 3C-3D)
4. **HSV Adjustments** - Saturation, Hue (Stage 3E)
5. **Exposure** - Simple multiplier (final step before curve)
6. **Tone Curve** - Optional LUT-based adjustment
7. **Alpha Blending** - Background compositing (Stage 3F)

## Tone Map Mode

Controls the overall tone mapping algorithm. Each mode processes the accumulation buffer differently.

| Mode | Description |
|------|-------------|
| **Linear** | Simple linear scaling with gamma correction. Bright areas clip to white. Good for low-dynamic-range flames or when you want direct control over the output. Uses Exposure directly as a multiplier before gamma. |
| **Logarithmic** | Apophysis-compatible algorithm using logarithmic brightness curves. Compresses bright areas while preserving detail in both highlights and shadows. Default and recommended for most flames. Uses Brightness parameter for curve shape, Exposure for final scaling. |
| **Density** | Raw density visualization as grayscale. Ignores color entirely - shows accumulated hit count. Useful for analyzing density distribution, finding hot spots, and debugging flame structure. Uses Exposure as sensitivity control. |

**When to use each mode:**
- **Logarithmic:** Default choice, matches Apophysis rendering
- **Linear:** When the fractal has low dynamic range, or you need predictable direct control
- **Density:** For debugging, analyzing density distribution, or creating black & white renders

## Parameter Reference

### Exposure
**Range:** 0.01 - 10.0 | **Default:** 1.0

Final brightness multiplier applied after all other tone mapping. Simple linear scaling - 2.0 doubles brightness, 0.5 halves it.

**Tooltip:** "Final brightness multiplier. 1.0 = no change, 2.0 = twice as bright."

---

### Gamma
**Range:** 0.1 - 10.0 | **Default:** 4.0

Controls the contrast curve applied to density. The value is inverted internally (1/gamma) for Apophysis compatibility.

- **Low values (< 2.0):** High contrast, dark shadows, bright highlights
- **Default (4.0):** Balanced Apophysis-style rendering
- **High values (> 6.0):** Low contrast, more visible shadow detail

**Tooltip:** "Contrast curve for density. Lower = more contrast, higher = flatter. Apophysis default is 4.0."

---

### Gamma Threshold
**Range:** 0.0 - 1000.0 | **Default:** 0.0025

Smooths the gamma curve at low densities to reduce noise in sparse areas. At densities below this threshold, the curve blends between linear and gamma response.

- **0.0:** No smoothing (pure gamma curve)
- **0.0025:** Default, subtle smoothing for sparse edges
- **Higher values:** More aggressive smoothing, reduces noise but may lose detail

**Tooltip:** "Smooths gamma at low densities to reduce noise. 0 = disabled, higher = more smoothing."

---

### Brightness
**Range:** 0.001 - 100.0 | **Default:** 1.0

Apophysis logarithmic brightness scaling. Controls the steepness of the density-to-brightness curve using the formula:

```
ls = (k1 * log10(1 + white_level * count * k2)) / (white_level * count)
```

This is NOT a simple multiplier - it changes the shape of the response curve:
- **Low values:** Darker overall, compressed dynamic range
- **1.0:** Apophysis default response
- **High values:** Brighter, especially in dense areas

**Tooltip:** "Apophysis brightness curve steepness. Affects how density maps to luminance. 1.0 = default."

---

### Vibrancy
**Range:** 0.0 - 30.0 | **Default:** 1.0

Blends between two color processing algorithms:

- **0.0 (Old algorithm):** Gamma is applied directly to RGB colors. Colors wash out in bright areas.
- **1.0 (New algorithm):** Gamma is applied to density only. Colors stay saturated even in bright areas.
- **Values 0-1:** Blend between both algorithms

Values above 1.0 are scaled to 256 internally for Apophysis compatibility (1.0 = 256 in Apophysis terms).

**Tooltip:** "Color saturation preservation. 0 = colors wash out in bright areas, 1 = colors stay saturated."

---

### Saturation
**Range:** 0.0 - 3.0 | **Default:** 1.0

HSV saturation multiplier applied after vibrancy processing.

- **0.0:** Grayscale output
- **1.0:** No change
- **> 1.0:** Boosted color intensity (may cause clipping)

**Tooltip:** "Color saturation multiplier. 0 = grayscale, 1 = no change, >1 = more colorful."

---

### Hue Shift
**Range:** -360° - 360° | **Default:** 0.0

Rotates all colors around the HSV color wheel. Useful for shifting the overall palette feel without editing the palette itself.

- **0°:** No change
- **180° / -180°:** Complementary colors
- **±120°:** Shift by one color triad

**Tooltip:** "Rotate all colors around the color wheel. 180° = complementary colors."

---

### Density Scale
**Range:** 0.01 - 10.0 | **Default:** 1.0

Scales raw density for linear alpha calculation. Only affects the linear component of alpha blending (controlled by Alpha Blend Low/High).

- **Low values:** Edges become transparent more slowly
- **High values:** Edges become opaque faster

**Tooltip:** "Scales density for alpha calculation. Affects edge opacity when using linear alpha blending."

---

### Alpha Blend Low
**Range:** 0.0 - 1.0 | **Default:** 0.3

Density threshold where alpha blending starts transitioning from gamma-corrected to linear. Below this value, gamma-corrected alpha is used (fast opacity rise, avoids dark halos at edges).

**Tooltip:** "Start blending to linear alpha at this density. Lower = sharper edges, may cause halos."

---

### Alpha Blend High
**Range:** 0.0 - 1.0 | **Default:** 0.7

Density threshold where alpha blending is fully linear. Above this value, linear alpha preserves density detail in solid areas.

**Tooltip:** "Full linear alpha above this density. Higher = more detail preservation in dense areas."

---

## Brightness vs Vibrancy vs Exposure

These three parameters all affect perceived brightness but work differently:

| Parameter | Type | When Applied | Effect |
|-----------|------|--------------|--------|
| **Brightness** | Logarithmic curve | Early (Stage 3A) | Changes density-to-luminance mapping shape |
| **Vibrancy** | Color algorithm blend | Middle (Stage 3D) | Controls color saturation preservation in bright areas |
| **Exposure** | Linear multiplier | Late (after HSV) | Simple final scaling, no curve change |

**Practical usage:**
- Use **Brightness** to control how density distribution maps to luminance
- Use **Vibrancy** to keep colors saturated in bright areas
- Use **Exposure** for final overall adjustment without changing tonal relationships

## Tone Curve

The optional tone curve provides fine-grained control similar to Photoshop/Lightroom curves:

- **Linear:** Identity curve (no effect)
- **S-Curve:** Increases contrast (darker shadows, brighter highlights)
- **Brighten Shadows:** Lifts shadow detail
- **Darken Highlights:** Compresses bright areas

The curve is evaluated via a 256-sample LUT texture for GPU efficiency.

## Related Files

- [shaders/tonemap.wgsl](../../shaders/tonemap.wgsl) - Tonemap shader implementation
- [src/scene/tonemap.rs](../../src/scene/tonemap.rs) - ToneMapMode enum and ToneCurve
- [src/ui/tone_mapping.rs](../../src/ui/tone_mapping.rs) - UI controls
- [src/gpu/buffers.rs](../../src/gpu/buffers.rs) - TonemapParams GPU struct
