# Color System Architecture

**Overview:** The color system handles color generation, palette management, and histogram-based atomic accumulation for thread-safe GPU rendering.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [TRANSFORMS.md](TRANSFORMS.md) - Color modes in flame algorithm
- [RENDERER.md](RENDERER.md) - Accumulate pass details
- [BUFFERS.md](BUFFERS.md) - Histogram buffer layout
- [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) - Palette library system (713 palettes)

**Code locations:**
- [src/scene/palette.rs](../../src/scene/palette.rs) - Palette system
- [shaders/core/main_template.wgsl](../../shaders/core/main_template.wgsl) - Color generation
- [shaders/accumulate.wgsl](../../shaders/accumulate.wgsl) - Histogram decoding

---

## Color Modes

The renderer supports three color generation modes:

### Transform Color Mode (mode=0)

**Behavior:** Each transform has an associated RGB color. Colors blend via exponential moving average as transforms are selected.

**Algorithm:**
```rust
// In shader (each iteration)
color_index = color_index * (1.0 - xform.color_speed) + xform.color_speed;
final_color = xform.color;  // Use transform's RGB color
```

**UI Control:**
- Transform window → Color section → RGB sliders (0.0-1.0)
- Color speed slider (0.0-1.0) controls blend rate

**Use Case:** Each transform contributes its own color, creating multicolored fractals.

### Palette Color Mode (mode=1)

**Behavior:** Colors are looked up from a 1D palette texture using `color_index`.

**Algorithm:**
```rust
// Color index evolves same as Transform mode
color_index = color_index * (1.0 - xform.color_speed) + xform.color_speed;

// But final color comes from palette lookup
final_color = textureSample(palette_texture, palette_sampler, color_index);
```

**UI Control:**
- Settings window → Color Settings → Color Mode → Palette
- Palette dropdown (built-in + loaded palettes)

**Use Case:** Smooth color gradients across the entire fractal.

### Speed Color Mode (mode=2)

**Behavior:** Color based on point movement speed (distance traveled per iteration).

**Algorithm:**
```rust
// Calculate speed (distance moved this iteration)
let speed = length(p_after - p_before);
let normalized_speed = speed * speed_factor;

// Lookup palette by speed
final_color = textureSample(palette_texture, palette_sampler, normalized_speed);
```

**UI Control:**
- Settings window → Color Settings → Color Mode → Speed
- Speed Factor slider (0.0-2.0) controls sensitivity

**Use Case:** Creates "heat map" effect where fast-moving areas have different colors than slow areas.

---

## Palette System

### Palette Structure

**Location:** [src/scene/palette.rs](../../src/scene/palette.rs)

```rust
pub struct Palette {
    pub name: String,
    pub stops: Vec<ColorStop>,
}

pub struct ColorStop {
    pub position: f32,  // 0.0 to 1.0
    pub color: [f32; 3],  // RGB [0.0-1.0]
}
```

**Interpolation:**
```rust
pub fn sample(&self, t: f32) -> [f32; 3] {
    // Clamp to [0, 1]
    let t = t.clamp(0.0, 1.0);

    // Find surrounding stops
    let mut prev_stop = &self.stops[0];
    let mut next_stop = &self.stops[self.stops.len() - 1];

    for i in 0..self.stops.len() - 1 {
        if t >= self.stops[i].position && t <= self.stops[i + 1].position {
            prev_stop = &self.stops[i];
            next_stop = &self.stops[i + 1];
            break;
        }
    }

    // Linear interpolation
    let range = next_stop.position - prev_stop.position;
    let factor = if range > 0.0 {
        (t - prev_stop.position) / range
    } else {
        0.0
    };

    [
        prev_stop.color[0] + factor * (next_stop.color[0] - prev_stop.color[0]),
        prev_stop.color[1] + factor * (next_stop.color[1] - prev_stop.color[1]),
        prev_stop.color[2] + factor * (next_stop.color[2] - prev_stop.color[2]),
    ]
}
```

### Built-in Palettes

**Location:** [src/scene/palette.rs](../../src/scene/palette.rs)

**Examples:**

**Fire:**
```rust
pub fn fire() -> Self {
    Self {
        name: "Fire".to_string(),
        stops: vec![
            ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },      // Black
            ColorStop { position: 0.3, color: [0.5, 0.0, 0.0] },      // Dark red
            ColorStop { position: 0.6, color: [1.0, 0.5, 0.0] },      // Orange
            ColorStop { position: 1.0, color: [1.0, 1.0, 0.0] },      // Yellow
        ],
    }
}
```

**Ocean:**
```rust
pub fn ocean() -> Self {
    Self {
        name: "Ocean".to_string(),
        stops: vec![
            ColorStop { position: 0.0, color: [0.0, 0.0, 0.3] },      // Deep blue
            ColorStop { position: 0.5, color: [0.0, 0.5, 0.7] },      // Blue
            ColorStop { position: 1.0, color: [0.0, 0.8, 1.0] },      // Cyan
        ],
    }
}
```

**Rainbow:**
```rust
pub fn rainbow() -> Self {
    Self {
        name: "Rainbow".to_string(),
        stops: vec![
            ColorStop { position: 0.0, color: [1.0, 0.0, 0.0] },      // Red
            ColorStop { position: 0.17, color: [1.0, 0.5, 0.0] },     // Orange
            ColorStop { position: 0.33, color: [1.0, 1.0, 0.0] },     // Yellow
            ColorStop { position: 0.5, color: [0.0, 1.0, 0.0] },      // Green
            ColorStop { position: 0.67, color: [0.0, 0.0, 1.0] },     // Blue
            ColorStop { position: 0.83, color: [0.3, 0.0, 0.5] },     // Indigo
            ColorStop { position: 1.0, color: [0.5, 0.0, 1.0] },      // Violet
        ],
    }
}
```

### PaletteLibrary

**Location:** [src/scene/palette.rs](../../src/scene/palette.rs)

**Overview:** The palette library manages 713 palettes organized into packs.

```rust
pub struct PaletteLibrary {
    palettes: Vec<Palette>,      // Flat list for dropdown (Colors panel)
    packs: Vec<PalettePack>,      // Pack storage
    enabled_packs: Vec<bool>,     // Runtime enable state
}

pub struct PalettePack {
    pub pack_name: String,
    pub description: String,
    pub enabled_by_default: bool,
    pub palettes: Vec<Palette>,
}
```

**Included Packs:**
- **Starter Pack** (12 palettes) - Enabled by default
  - Fire, Ocean, Forest, Sunset, Galaxy, Copper, Ice, Lava, Neon, Earth, Rainbow, Monochrome
- **Apophysis Pack** (701 palettes) - Disabled by default
  - Complete classic Apophysis gradient collection

**Loading Routes:**
1. Grayscale (hardcoded - always first)
2. Legacy `assets/palettes/*.palette` files (desktop only)
3. Palette packs from `assets/palettes/packs/*.json` (desktop)
4. Embedded Starter Pack (WASM only - compile-time via `include_str!()`)
5. Fallback palettes (fire, cool, rainbow, purple_pink) if no packs loaded
6. Runtime imports (editor, file picker, config load, palette library selection)

**All routes use `add_or_update()`:**
- Case-insensitive duplicate checking
- First palette loaded with a name wins
- Duplicates logged and skipped

**See:** [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) for complete documentation

### Palette File Format

**File extension:** `.palette`

**Format:** JSON
```json
{
  "name": "My Custom Palette",
  "stops": [
    {
      "position": 0.0,
      "color": [1.0, 0.0, 0.0]
    },
    {
      "position": 0.5,
      "color": [0.0, 1.0, 0.0]
    },
    {
      "position": 1.0,
      "color": [0.0, 0.0, 1.0]
    }
  ]
}
```

**Import/Export:**
- Palette Editor window → Import/Export Palette section
- "Export Palette" → Save to file or clipboard
- "Load Palette" → Import from file or clipboard
- Imported palettes automatically added to library

### GPU Upload

Palettes are uploaded as 1D textures for fast GPU sampling:

```rust
// Generate 256-sample 1D texture
let mut palette_data = vec![0u8; 256 * 4];  // 256 RGBA pixels

for i in 0..256 {
    let t = i as f32 / 255.0;
    let color = palette.sample(t);

    palette_data[i * 4 + 0] = (color[0] * 255.0) as u8;  // R
    palette_data[i * 4 + 1] = (color[1] * 255.0) as u8;  // G
    palette_data[i * 4 + 2] = (color[2] * 255.0) as u8;  // B
    palette_data[i * 4 + 3] = 255;                       // A
}

// Upload to GPU
queue.write_texture(
    palette_texture.as_image_copy(),
    &palette_data,
    ImageDataLayout { ... },
    Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
);
```

**Shader Access:**
```wgsl
@group(0) @binding(3) var palette_texture: texture_1d<f32>;
@group(0) @binding(4) var palette_sampler: sampler;

fn get_color(color_index: f32) -> vec3<f32> {
    return textureSample(palette_texture, palette_sampler, color_index).rgb;
}
```

---

## Histogram Color Accumulation System

### Overview

The renderer uses a **u32 histogram buffer** for thread-safe atomic color accumulation on the GPU. This replaced direct texture writes which had race conditions.

**Architecture:**
```
1. Compute Pass (main_2d/3d.wgsl)
   → Atomic u32 writes to histogram buffer

2. Accumulate Pass (accumulate.wgsl)
   → Read histogram, decode, blend, clear

3. Tonemap Pass (tonemap.wgsl)
   → Display
```

### Histogram Format (U32 Unpacked)

**Current implementation** (Added 2025-10-27)

**Layout:** 4× u32 per pixel (separate R, G, B, Density channels)
```
Pixel Index: i = y * width + x
Base Index:  base = i * 4

histogram[base + 0] = R (u32, 0 to 4,294,967,295)
histogram[base + 1] = G (u32, 0 to 4,294,967,295)
histogram[base + 2] = B (u32, 0 to 4,294,967,295)
histogram[base + 3] = Density (u32, count of hits)
```

**Memory Usage:** `width × height × 4 × 4 bytes`
- 1920×1080: ~31.5 MB
- 800×600: ~9.2 MB

**Benefits:**
- ✅ Overflow eliminated (4.2B max vs 65K for u16)
- ✅ Proper HDR behavior (bright areas stay bright)
- ✅ 2.4% performance cost (acceptable for correct output)
- ✅ Simple, maintainable codebase

### Encoding (Compute Shader)

**Location:** [shaders/core/main_template.wgsl](../../shaders/core/main_template.wgsl)

```wgsl
fn write_to_histogram(screen_pos: vec2<u32>, color: vec3<f32>) {
    let pixel_idx = screen_pos.y * params.width + screen_pos.x;
    let base_idx = pixel_idx * 4u;

    // Encode with fixed scale
    let scale = params.histogram_color_scale;  // Default: 100.0

    let r_u32 = u32(clamp(color.r, 0.0, 1.0) * scale);
    let g_u32 = u32(clamp(color.g, 0.0, 1.0) * scale);
    let b_u32 = u32(clamp(color.b, 0.0, 1.0) * scale);
    let density_u32 = u32(scale);

    // Atomic accumulation (thread-safe)
    atomicAdd(&histogram[base_idx + 0u], r_u32);
    atomicAdd(&histogram[base_idx + 1u], g_u32);
    atomicAdd(&histogram[base_idx + 2u], b_u32);
    atomicAdd(&histogram[base_idx + 3u], density_u32);
}
```

**Key Points:**
- Colors clamped to [0, 1] before scaling
- Scale factor applies to both color and density
- Atomic operations ensure thread safety
- No race conditions (unlike direct texture writes)

### Decoding (Accumulate Shader)

**Location:** [shaders/accumulate.wgsl](../../shaders/accumulate.wgsl)

```wgsl
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_coords = global_id.xy;

    // Read histogram values
    let pixel_idx = pixel_coords.y * params.width + pixel_coords.x;
    let base_idx = pixel_idx * 4u;

    let r_sum = f32(histogram[base_idx + 0u]);
    let g_sum = f32(histogram[base_idx + 1u]);
    let b_sum = f32(histogram[base_idx + 2u]);
    let density = f32(histogram[base_idx + 3u]);

    // Decode to color (scale cancels out)
    let color = vec3(r_sum, g_sum, b_sum) / (density + 1e-6);

    // Read previous accumulation
    let prev_accum = textureLoad(prev_accumulation, pixel_coords, 0);

    // Apply accumulation controls
    let density_factor = pow(density, params.low_density_smoothing);
    let compression_factor = 1.0 / (1.0 + density * params.density_compression_strength * 0.01);
    let convergence_gate = if iteration_counts[pixel_idx] < params.target_iterations_per_pixel { 1.0 } else { 0.0 };

    let adjusted_blend = params.blend_factor * density_factor * compression_factor * convergence_gate;

    // Exponential moving average
    let result_color = mix(prev_accum.rgb, color, adjusted_blend);
    let result_density = mix(prev_accum.a, density, adjusted_blend);

    // Write to output
    textureStore(output_texture, pixel_coords, vec4(result_color, result_density));

    // Clear histogram for next frame
    histogram[base_idx + 0u] = 0u;
    histogram[base_idx + 1u] = 0u;
    histogram[base_idx + 2u] = 0u;
    histogram[base_idx + 3u] = 0u;
}
```

**Key Points:**
- Scale factor cancels out during division: `(r*scale) / (density*scale) = r/density`
- Histogram is cleared after reading (write zeros)
- Accumulation controls applied before blending

### Evolution History

**1. Original (Removed):** Direct texture writes
- Used `textureStore()` to write colors directly
- **Problem:** Race conditions with concurrent writes (undefined behavior per WebGPU spec)
- **Symptom:** Visual artifacts, incorrect colors

**2. U16 Packed (2025-10-26):** 3× u32 per pixel
- Format: `[u32: (R16|G16), u32: B16, u32: density]`
- **Problem:** RGB channels overflow after ~1,310 hits at scale=50
- **Symptom:** Bright areas wrap to dark colors (0xFFFF → 0x0000)
- Capacity: 65,535 max value per channel

**3. U32 Unpacked (2025-10-27, current):** 4× u32 per pixel
- Format: `[R32, G32, B32, density32]`
- **Benefit:** Eliminates overflow - 4.2 billion max value per channel
- **Tradeoff:** 33% larger memory footprint (3→4 words)
- Capacity: 42.9M hits before overflow (at scale=100) = 91 minutes continuous rendering

### Performance Characteristics

**Benchmark Results** (simple3 @ 1920×1080, 1024 iters/thread):
```
Commit    Description                  Time (ms)  Throughput (Giter/s)
------    -----------                  ---------  --------------------
dd80003   textureStore (baseline)      ~6800      ~5.86  [had race conditions]
9ac278a   u16 packed histogram         1570       25.36  [overflow issues]
a8301de   u32 unpacked histogram       1607       24.76  [current, no overflow]
```

**Performance vs Baseline:**
- U32 histogram: **2.4% slower** than u16 packed
- Acceptable tradeoff for correct visual output
- 76% of memory bandwidth: 4 words vs 3 words

**Why Acceptable:**
- Eliminates visual artifacts (overflow wraparound)
- Proper HDR behavior (bright areas stay bright)
- Clean, maintainable codebase
- Future-proof for high iteration counts

### UI Control

**Location:** Settings window → Rendering section → "Histogram Color Scale" slider

**Range:** 1.0 to 1000.0
**Default:** 100.0

**Effect:**
- Higher values: Better precision, still safe from overflow
- Lower values: Less precision, but more headroom (unnecessary with u32)
- Recommend ≥100 for smooth gradients

### Design Decisions

**Why U32 instead of F32?**
- Atomic operations on f32 are undefined in WGSL/WebGPU
- Integer atomics are guaranteed safe and correct
- Scale factor provides adequate precision

**Why Global Scale instead of Per-Pixel Adaptive?**
- Simpler implementation (single uniform constant)
- Faster access (uniform vs storage buffer read)
- Eliminated 1.9 MB scale_buffer overhead
- Avoids complex convergence detection logic

**Why Separate Density Channel?**
- Allows correct averaging: `color = sum / density`
- Preserves HDR information for tone mapping
- Matches traditional flame renderer architecture

---

## Accumulation Controls

Fine-grained control over how colors blend over time.

### Blend Rate

**Parameter:** `blend_factor` (0.01-1.0, default 0.1)

**Effect:** Controls how quickly new samples blend with history.
- Low (0.01): Slow, smooth convergence (100 samples to reach 63%)
- High (1.0): Fast, flickery (instant replacement)
- Default (0.1): Good balance (10 samples to reach 63%)

### Dynamic Blend Mode

**Toggle:** Exponential vs Fixed Rate

**Exponential (old default):**
```rust
blend_factor = 1.0 / (samples_accumulated as f32)
```
- Starts fast (1.0 when samples=1)
- Slows down over time (0.1 when samples=10, 0.01 when samples=100)
- Good for initial convergence

**Fixed Rate (new option):**
```rust
blend_factor = constant  // User-specified (e.g., 0.1)
```
- Constant blend rate regardless of sample count
- Smoother at low sample counts
- More predictable behavior

### Low-Density Smoothing

**Parameter:** `low_density_smoothing` (0.0-1.0, default 0.5)

**Effect:** Reduces blend rate in sparse/dark areas to reduce noise.

**Formula:**
```wgsl
let density_factor = pow(density, low_density_smoothing);
adjusted_blend = blend_factor * density_factor;
```

**Behavior:**
- 0.0: No smoothing (uniform blend rate)
- 0.5: Moderate (default)
- 1.0: Maximum smoothing (very slow in sparse areas)

### Density Compression

**Parameter:** `density_compression_strength` (0.0-100.0, default 0.0)

**Effect:** Slows accumulation in bright areas to reveal detail.

**Formula:**
```wgsl
let compression_factor = 1.0 / (1.0 + density * strength * 0.01);
adjusted_blend = blend_factor * compression_factor;
```

**Behavior:**
- 0.0: No compression (default)
- 25.0: Gentle (20% rate in bright areas)
- 50.0: Moderate (2% rate)
- 100.0: Strong (1% rate)

**Use Case:** Prevents over-bright areas from saturating, reveals fine detail.

### Per-Pixel Iteration Limiting

**Parameter:** `target_iterations_per_pixel` (0-1M, default 0)

**Effect:** Stops accumulating pixel after N hits.

**Implementation:**
- Tracked via atomic counters in compute shader (~5% overhead)
- Gated after initial density to avoid empty spots
- 0 = disabled (default)

**Use Case:**
- Prevents over-sampling dense areas
- Allows sparse areas to catch up
- Low limits (5-100) for quick previews
- High limits (100K-1M) for quality renders

---

## Related Documentation

- [archive/histogram/](../archive/histogram/) - Complete histogram evolution (15 historical docs)
  - HISTOGRAM_FINAL.md - Complete evolution timeline and final solution
  - HISTOGRAM_OPTIMIZATION_ATTEMPTS.md - Failed optimization attempts
- [COLOR_PIPELINE.md](COLOR_PIPELINE.md) - Complete color pipeline documentation

---

## Common Color System Tasks

| Task | Files to Modify |
|------|-----------------|
| Add built-in palette | [palette.rs](../../src/scene/palette.rs) `PaletteLibrary::new()` |
| Change color mode algorithm | [main_template.wgsl](../../shaders/core/main_template.wgsl) |
| Modify histogram format | [buffers.rs](../../src/gpu/buffers.rs), [main_template.wgsl](../../shaders/core/main_template.wgsl), [accumulate.wgsl](../../shaders/accumulate.wgsl) |
| Change accumulation formula | [accumulate.wgsl](../../shaders/accumulate.wgsl), [buffers.rs](../../src/gpu/buffers.rs) `AccumulateParams` |
| Add color mode | [transforms.rs](../../src/scene/transforms.rs), shaders, [ui/mod.rs](../../src/ui/mod.rs) |
| Import/export palette | Use Palette Editor UI → Import/Export section |

---

**Last Updated:** 2026-01-24
**Related Documentation:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [TRANSFORMS.md](TRANSFORMS.md) - Color modes in flame algorithm
- [RENDERER.md](RENDERER.md) - Rendering pipeline
- [BUFFERS.md](BUFFERS.md) - GPU data structures
