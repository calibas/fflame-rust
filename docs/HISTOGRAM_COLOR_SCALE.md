# Histogram Color Scale - Design Document

## The Problem

### What is Color Scale?
The histogram uses **u16 fixed-point integers** to store color values. When a sample hits a pixel:
- Color values (0.0-1.0 float range) are multiplied by `color_scale`
- Converted to integers (e.g., 0.5 red * 100 = 50)
- Stored in u16 (0-65535 range)
- Later divided back: `50 / (density * 100) = 0.5`

### The Overflow Problem
With batched accumulation (`batch_size=4`, `iterations_per_thread=1024`), dense pixels accumulate many hits before processing:

**Overflow threshold:**
- `color_scale=100`: 65535 / 100 = **655 max hits** before overflow
- `color_scale=10`: 65535 / 10 = **6553 max hits** before overflow

**When overflow occurs:**
- u16 wraps around (e.g., 70000 becomes 4465)
- Color ratios become corrupted
- Result: Color noise/artifacts in dense areas

### Why Zoom Level Matters
The more zoomed out you are, the more iterations hit the same pixel:
- **Zoomed in**: Iterations spread across many pixels → low density per pixel → safe
- **Zoomed out**: Many iterations map to same pixel → high density → overflow risk

**There is no "typical" zoom level** - this is art, not science. Users need control.

### Current Trade-off
- **`color_scale=100`**: High precision (100 color levels), but overflows at 655 hits
- **`color_scale=10`**: Lower precision (10 color levels), but 10× overflow protection

With batched accumulation, we default to `color_scale=10` to minimize overflow artifacts.

## Why This Matters for Color

### How Colors Are Applied (Data Flow)

```
1. COMPUTE SHADER (trajectory.wgsl)
   ├─ Iterate fractal flame algorithm
   ├─ Final color = weighted blend of transform colors
   ├─ Scale color by color_scale (e.g., * 10 or * 100)
   ├─ Convert to u16 integer
   └─ atomicAdd to histogram buffer (BEFORE accumulation)
       ↓
2. ACCUMULATE SHADER (accumulate.wgsl)
   ├─ Read histogram (u16 packed colors + density)
   ├─ Unpack and divide by (density * color_scale)
   ├─ Result: Average color for this pixel (0.0-1.0 range)
   ├─ Blend with previous accumulation buffer (weighted average)
   └─ Write to accumulation texture (rgba16float)
       ↓
3. TONEMAP SHADER (tonemap.wgsl)
   ├─ Read from accumulation texture
   ├─ Apply density_scale (affects brightness based on alpha/density)
   ├─ Apply tone curve, exposure, gamma
   └─ Write to display (final rendered image)
```

### Key Distinction

**Histogram Color Scale (NEW)** - Controls precision vs overflow in histogram
- Affects: Color accuracy in dense areas
- Location: BEFORE accumulation (compute shader)
- Range: Lower scale = more overflow protection, less precision
- Default: 10 for batched accumulation

**Density Scale (EXISTING)** - Controls brightness/contrast in final display
- Affects: Overall image brightness and tone mapping
- Location: AFTER accumulation (tonemap shader)
- Range: Higher scale = brighter image
- Default: User configurable (typically 1.0-10.0)

## The Solution: User Control

### Why Make It Configurable?
1. **Artistic control**: Let users balance precision vs overflow for their specific artwork
2. **Zoom adaptation**: Users can adjust for zoomed-out scenes (lower scale) vs zoomed-in (higher scale)
3. **Quality preferences**: Some users prefer overflow artifacts over precision loss
4. **Historical compatibility**: Apophysis had similar overflow issues - users are familiar with this trade-off

### Proposed Implementation

**Parameter:** `histogram_color_scale`
- Type: Float slider
- Range: 1.0 to 100.0
- Default: 10.0 (for batched accumulation)
- UI Location: Advanced Rendering Settings
- Effect: Passed to compute shader as uniform parameter

**Range breakdown:**
- **1.0-5.0**: Maximum overflow protection (65535+ hits), very low precision
- **10.0**: Balanced (6553 hits, 10 color levels) - **recommended default**
- **50.0**: Higher precision (1310 hits, 50 color levels)
- **100.0**: Maximum precision (655 hits, 100 color levels) - classic approach

**UI Label:** "Histogram Color Scale"
**Tooltip:** "Controls color precision vs overflow protection. Lower values prevent artifacts in zoomed-out scenes. Higher values give better color accuracy but overflow sooner."

### Implementation Changes Required

1. **Add parameter to ComputeParams struct** (src/gpu/buffers.rs)
   ```rust
   pub struct ComputeParams {
       // ... existing fields ...
       pub histogram_color_scale: f32,  // NEW
   }
   ```

2. **Update compute shaders** (shaders/core/main_2d.wgsl, main_3d.wgsl)
   ```wgsl
   // Replace hardcoded constant:
   let color_scale = 10.0;

   // With uniform parameter:
   let color_scale = params.histogram_color_scale;
   ```

3. **Update accumulate shader** (shaders/accumulate.wgsl)
   ```wgsl
   // Need to pass color_scale to accumulate for proper unpacking
   // Add to AccumulateParams:
   struct AccumulateParams {
       width: u32,
       height: u32,
       blend_factor: f32,
       histogram_color_scale: f32,  // NEW - must match compute shader
   }
   ```

4. **Add UI control** (src/ui/mod.rs)
   ```rust
   ui.horizontal(|ui| {
       ui.label("Histogram Color Scale:");
       if ui.add(egui::Slider::new(&mut config.histogram_color_scale, 1.0..=100.0)
           .logarithmic(true)
           .text("scale"))
           .changed()
       {
           needs_reset = true;
       }
   });
   ```

5. **Add to FractalConfig** (src/config.rs)
   ```rust
   pub struct FractalConfig {
       // ... existing fields ...
       pub histogram_color_scale: f32,  // Default: 10.0
   }
   ```

### Important: Synchronization
The **same color_scale value** must be used in both:
1. Compute shader (when packing colors into histogram)
2. Accumulate shader (when unpacking colors from histogram)

If these don't match, colors will be completely wrong. They must be synchronized each frame.

## Known Limitations

Even with user control, overflow artifacts at extreme densities are unavoidable with u16 packing:
- This is the same limitation Apophysis had
- Alternative solutions (u32 or f16 histograms) would use 2-4× more memory with performance cost
- Users can work around by adjusting scale or reducing `iterations_per_thread` / `batch_size`

## Historical Note

Apophysis (the reference implementation) had similar overflow artifacts in dense areas. This is a known trade-off of the histogram approach. The batched accumulation experiment (3.28× speedup) makes this trade-off more visible, but it's always been present.
