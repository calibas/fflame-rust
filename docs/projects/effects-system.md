# Effects System

## Overview

A flexible shader effects pipeline with two effect chains that run between accumulation and display. Effects are stored in FractalConfig as dynamic lists (0 to N effects) and integrated with ConfigManager for undo/redo.

**Zero-cost guarantee**: Empty effect lists = zero render passes, zero texture allocations, zero performance impact.

## Architecture

```
Accumulate → [Density Effects] → Tonemap → [Color Effects] → Display
```

### Density Effects (Fragment Shaders)
Operate on raw accumulated data (RGBA: RGB color + density in alpha).
- Have access to density values for density-aware operations
- Output to intermediate texture (same format as input)
- Run BEFORE tonemapping

**Example Effects:**
- **density_blur** - Gaussian blur weighted by density threshold
- **sharpen** - Detail enhancement / unsharp mask
- **density_edge** - Edge detection based on density gradients

### Color Effects (Fragment Shaders)
Operate on final RGB colors after tonemapping.
- Standard image processing on display-ready colors
- No density information available
- Run AFTER tonemapping

**Example Effects:**
- **color_grade** - LUT-based color grading
- **hue_cycle** - Psychedelic hue rotation (animatable)
- **bloom** - Glow effect on bright areas
- **chromatic_aberration** - RGB channel offset
- **vignette** - Darkening toward edges
- **film_grain** - Animated noise overlay

## Data Model

### Effect Registry

Effects are registered by string name (like variations), not hardcoded enum variants.
Display names use the i18n system (`effects.{name}.name` keys in locales/*.yml).

```rust
/// Metadata for a registered effect
pub struct EffectInfo {
    pub name: String,           // "density_blur", "vignette", etc.
    pub category: EffectCategory,
    pub shader_path: String,    // "shaders/effects/density_blur.wgsl"
    pub parameters: Vec<EffectParameter>,
}

pub enum EffectCategory {
    Density,  // Runs before tonemap, has density access
    Color,    // Runs after tonemap, RGB only
}

pub struct EffectParameter {
    pub name: String,
    pub param_type: ParamType,  // Float, Integer, Bool, Angle
    pub default_value: f32,
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
}
```

### Effect Instance (per-config)

Each effect in a config is an instance with its own parameters:

```rust
/// A single effect instance in a config
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectInstance {
    /// Effect type name (must match registered effect)
    pub effect_type: String,

    /// Whether this effect is currently active
    pub enabled: bool,

    /// Parameter values (name → value)
    /// Missing params use defaults from registry
    pub params: HashMap<String, f32>,
}
```

### FractalConfig Changes

```rust
pub struct FractalConfig {
    // ... existing fields ...

    /// Density effects chain (order matters, runs before tonemap)
    /// Empty Vec = zero cost, no render passes
    #[serde(default)]
    pub density_effects: Vec<EffectInstance>,

    /// Color effects chain (order matters, runs after tonemap)
    /// Empty Vec = zero cost, no render passes
    #[serde(default)]
    pub color_effects: Vec<EffectInstance>,
}
```

## ConfigPath Integration

### ConfigPath Variants

```rust
pub enum ConfigPath {
    // ... existing variants ...

    /// Effect at index in density_effects chain
    DensityEffect { index: usize },

    /// Specific parameter of density effect
    DensityEffectParam { index: usize, param: String },

    /// Effect at index in color_effects chain
    ColorEffect { index: usize },

    /// Specific parameter of color effect
    ColorEffectParam { index: usize, param: String },
}
```

### UpdateType

Effect changes return `UpdateType::ToneMappingOnly` - they don't affect accumulation, only the display pipeline.

## Render Pipeline

### Zero-Cost Empty Lists

```rust
impl FlameRenderer {
    pub fn render_frame(&mut self, encoder: &mut CommandEncoder) {
        // 1. Compute pass (existing)
        self.compute_pass(encoder);

        // 2. Accumulate pass (existing)
        self.accumulate_pass(encoder);

        // 3. Density effects chain
        // If density_effects is empty: NO render passes, direct passthrough
        let density_output = if self.density_effects.is_empty() {
            self.accumulated_texture()  // Zero cost - just use existing texture
        } else {
            self.run_effect_chain(encoder, &self.density_effects, self.accumulated_texture())
        };

        // 4. Tonemap pass (existing, reads from density_output)
        self.tonemap_pass(encoder, density_output);

        // 5. Color effects chain
        // If color_effects is empty: NO render passes, direct passthrough
        if !self.color_effects.is_empty() {
            self.run_effect_chain(encoder, &self.color_effects, self.tonemapped_texture());
        }
    }
}
```

### Effect Chain Runner

```rust
fn run_effect_chain(
    &mut self,
    encoder: &mut CommandEncoder,
    effects: &[EffectInstance],
    input: &TextureView,
) -> &TextureView {
    // Filter to only enabled effects
    let active: Vec<_> = effects.iter()
        .filter(|e| e.enabled)
        .collect();

    // Zero enabled effects = zero cost
    if active.is_empty() {
        return input;
    }

    // Ping-pong between two intermediate textures
    // Textures allocated lazily on first use
    let mut current_input = input;

    for effect in active {
        let pipeline = self.get_or_compile_effect_pipeline(&effect.effect_type);
        let output = self.get_ping_pong_texture();

        // Single render pass per effect
        self.run_effect_pass(encoder, pipeline, current_input, output, &effect.params);

        current_input = output;
    }

    current_input
}
```

### Shader Organization

```
shaders/
  effects/
    density/
      density_blur.wgsl
      sharpen.wgsl
      density_edge.wgsl
    color/
      hue_cycle.wgsl
      bloom.wgsl
      chromatic_aberration.wgsl
      vignette.wgsl
      film_grain.wgsl
      color_grade.wgsl
```

Each effect is a standalone fragment shader file. Shaders are loaded and compiled on demand when first used.

### Effect Shader Template

```wgsl
// Example: vignette.wgsl

struct EffectParams {
    intensity: f32,
    radius: f32,
    softness: f32,
    time: f32,        // For animated effects
    resolution: vec2<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> params: EffectParams;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let coords = vec2<i32>(id.xy);
    let color = textureLoad(input_texture, coords, 0);

    // Effect logic here
    let uv = vec2<f32>(id.xy) / params.resolution;
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(uv, center);
    let vignette = smoothstep(params.radius, params.radius - params.softness, dist);
    let result = vec4<f32>(color.rgb * mix(1.0, vignette, params.intensity), color.a);

    textureStore(output_texture, coords, result);
}
```

## UI Integration

### Effects Panel

New panel or collapsible section:

```
[Effects]
  Density Effects (before tonemap)
    [density_blur]              [^] [v] [x]
      Radius: [====o====] 3.0
      Threshold: [==o=====] 25%
    [sharpen]                   [^] [v] [x]
      Amount: [=====o===] 1.2

  Color Effects (after tonemap)
    [hue_cycle]                 [^] [v] [x]
      Speed: [===o======] 30 deg/s
    [vignette]                  [^] [v] [x]
      Intensity: [====o===] 0.5

  [+ Add Effect...]
```

### Effect Controls
- Enable/disable checkbox per effect
- Reorder buttons (up/down arrows)
- Remove button (x)
- Effect-specific parameter sliders
- All changes go through ConfigManager for undo/redo

### Add Effect Dialog
- Shows all registered effects grouped by category
- Click to add to appropriate chain (density or color)
- New effects added at end of chain, enabled by default

## Animation System Integration

Effect parameters integrate with the existing animation system via ConfigPath strings:

```rust
// Animatable effect parameters
ParameterCategory {
    name: "Effects".to_string(),
    params: vec![
        // Parameters discovered dynamically from effect registry
        ("Vignette Intensity", "ColorEffectParam.0.intensity"),
        ("Bloom Threshold", "ColorEffectParam.1.threshold"),
        // etc.
    ],
}
```

### Time Uniform

Effects that need animation time (film_grain, hue_cycle) receive it via the params struct:

```wgsl
struct EffectParams {
    // ... effect-specific params ...
    time: f32,        // Current time in seconds
    frame_index: u32, // For noise seed variation
}
```

## Performance Guarantees

1. **Zero effects = zero cost**
   - Empty `Vec` means no render passes, no texture allocations
   - No shader compilation, no bind group creation

2. **Disabled effects = zero cost**
   - `enabled: false` effects are skipped entirely
   - Not even bound to the pipeline

3. **Lazy resource allocation**
   - Ping-pong textures allocated only when first effect runs
   - Effect shaders compiled on demand, cached thereafter

4. **Efficient multi-effect chains**
   - Single texture read/write per effect
   - Ping-pong avoids unnecessary copies
   - Separable blur uses H+V passes (O(r) vs O(r²))

## Implementation Phases

### Phase 1: Infrastructure ✅
- [x] Create `EffectRegistry` with registration API
- [x] Add `EffectInstance` to FractalConfig
- [x] Add ConfigPath variants for effects
- [x] Create effect chain runner with ping-pong textures
- [x] Lazy texture allocation

### Phase 2: First Effects ✅
- [x] Implement `vignette` (simple color effect)
- [x] Implement `density_blur` (density effect)
- [x] Basic UI for adding/configuring effects
- [x] Effect enable/disable

### Phase 3: Full Suite ✅
- [x] Color effects: hue_shift, film_grain, chromatic_aberration
- [x] Density effects: sharpen
- [x] i18n support for effect names and parameters
- [x] Effect reordering UI (up/down buttons with undo/redo support)
- ~~[ ] Effect presets~~ (skip for now)

### Phase 3b: New Single-Pass Effects ✅
Based on analysis of [new-shaders.md](new-shaders.md), these can be implemented with current architecture:

**Color Effects (single-pass fragment shaders):**
- [x] `kaleidoscope` - N-fold rotational symmetry via polar coordinate folding
- [x] `plasma` - Classic demoscene summed sinusoids effect
- [x] `tunnel` - Polar coordinate texture mapping
- [x] `sobel_edges` - Edge detection with neon glow aesthetic
- [x] `domain_warp` - Organic flowing distortion using FBM noise

**Density Effects:**
- [x] `bilateral_blur` - Edge-preserving blur (better quality than Gaussian)

### Phase 4: Polish
- ~~[ ] LUT library for color_grade~~ (skip for now)
- [ ] Multi-pass effects (bloom with separate blur passes)
- [x] Animation timeline integration
- ~~[ ] Effect import/export~~ (skip for now)

---

## Shader Compatibility Analysis

Based on review of [new-shaders.md](new-shaders.md), here's what our current single-pass fragment shader architecture can support:

### ✅ Can Implement Now (Single-Pass)

| Effect | Category | Description |
|--------|----------|-------------|
| Bilateral Filtering✅ | Density | Edge-preserving blur, O(n²) per pixel |
| Kaleidoscope✅ | Color | UV manipulation, very cheap |
| Domain Warping✅ | Color | Organic distortion using FBM |
| Simplex/Worley✅ Noise | Color | Procedural overlays/distortions |
| Sobel Edge Detection✅ | Color | Neon outline effect |
| Plasma✅ | Color | Classic demoscene effect |
| Tunnel✅ | Color | Polar coordinate warp |
| Mandelbrot/Julia Overlay✅ | Color | Fractal blend/overlay |

### ⚠️ Partial Implementation Possible

| Effect | What Works | Limitation |
|--------|-----------|------------|
| Bloom | Brightness extraction | Full blur needs multi-pass |
| IQ Cosine Palette | Color remapping | Already have palette system |

### ❌ Requires Architecture Changes

| Effect | Reason |
|--------|--------|
| Gaussian Splatting | Needs vertex shader + point data pipeline |
| Temporal Reprojection | Needs previous frame history buffer |
| SVGF | 5-7 passes with variance tracking |
| Guided Filtering | 3 passes |
| Anisotropic Diffusion | 10-100+ iterations |
| Feedback Effects | Ping-pong with previous frame |
| Reaction-Diffusion | 10-50 compute iterations per frame |

---

## Open Questions

1. Should we support custom user effects (load .wgsl from disk)?
2. LUT format for color_grade - standard .cube files?
3. Should bloom use multiple blur passes or single approximation?
4. Effect presets - save/load as separate files or embed in config?
