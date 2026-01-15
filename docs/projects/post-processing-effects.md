# Post-Processing Effects System

## Overview

A flexible post-processing pipeline with two effect chains: pre-tonemap (density-aware) and post-tonemap (color effects). Effects are stored in FractalConfig and integrated with the ConfigManager state system for undo/redo support.

## Architecture

```
Accumulate → [Pre-Tonemap Chain] → Tonemap → [Post-Tonemap Chain] → Display
```

### Pre-Tonemap Effects (Fragment Shaders)
Operate on raw accumulated data (RGBA: RGB color + density in alpha).
- Have access to density values for density-aware operations
- Output to intermediate texture (same format as input)

**Planned Effects:**
- **DensityBlur** - Gaussian blur weighted by density threshold
- **Sharpen** - Detail enhancement / unsharp mask
- **DensityEdge** - Edge detection based on density gradients

### Post-Tonemap Effects (Fragment Shaders)
Operate on final RGB colors after tonemapping.
- Standard image processing on display-ready colors
- No density information available

**Planned Effects:**
- **ColorGrade** - LUT-based color grading
- **HueCycle** - Psychedelic hue rotation (animatable)
- **Bloom** - Glow effect on bright areas
- **ChromaticAberration** - RGB channel offset
- **Vignette** - Darkening toward edges
- **FilmGrain** - Animated noise overlay

## Data Model

### FractalConfig Changes

```rust
pub struct FractalConfig {
    // ... existing fields ...

    /// Pre-tonemap effects chain (order matters)
    #[serde(default)]
    pub pre_effects: Vec<PreEffect>,

    /// Post-tonemap effects chain (order matters)
    #[serde(default)]
    pub post_effects: Vec<PostEffect>,
}
```

### Effect Enums

```rust
/// Pre-tonemap effect with parameters
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PreEffect {
    DensityBlur {
        enabled: bool,
        radius: f32,           // 0-10 pixels
        threshold: f32,        // density threshold (0-100% of range)
        falloff: f32,          // transition smoothness
    },
    Sharpen {
        enabled: bool,
        amount: f32,           // 0-2
        radius: f32,           // 0-5 pixels
        threshold: f32,        // density threshold to apply
    },
    DensityEdge {
        enabled: bool,
        strength: f32,         // 0-1
        mode: EdgeMode,        // Sobel, Laplacian, etc.
    },
}

/// Post-tonemap effect with parameters
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PostEffect {
    HueCycle {
        enabled: bool,
        speed: f32,            // degrees per second (for animation)
        offset: f32,           // static offset in degrees
    },
    Bloom {
        enabled: bool,
        threshold: f32,        // brightness threshold
        intensity: f32,        // bloom strength
        radius: f32,           // blur radius
    },
    ChromaticAberration {
        enabled: bool,
        amount: f32,           // pixel offset amount
        radial: bool,          // radial vs uniform
    },
    Vignette {
        enabled: bool,
        intensity: f32,        // 0-1
        radius: f32,           // 0-1 (where falloff starts)
        softness: f32,         // falloff curve
    },
    FilmGrain {
        enabled: bool,
        intensity: f32,        // 0-1
        size: f32,             // grain size
        animated: bool,        // change each frame
    },
    ColorGrade {
        enabled: bool,
        lut_index: usize,      // index into LUT library
        intensity: f32,        // blend with original (0-1)
    },
}
```

## ConfigPath Integration

### New ConfigPath Variants

```rust
pub enum ConfigPath {
    // ... existing variants ...

    // Pre-effects chain
    PreEffectEnabled { index: usize },
    PreEffectParam { index: usize, param: String },
    PreEffectOrder,  // For reordering the chain

    // Post-effects chain
    PostEffectEnabled { index: usize },
    PostEffectParam { index: usize, param: String },
    PostEffectOrder,  // For reordering the chain
}
```

### UpdateType

Effects changes should return `UpdateType::ToneMappingOnly` since they don't affect accumulation, only the display pipeline.

### ConfigValue Extensions

```rust
pub enum ConfigValue {
    // ... existing variants ...

    PreEffect(PreEffect),
    PostEffect(PostEffect),
    EffectOrder(Vec<usize>),  // For reordering
}
```

## Render Pipeline Changes

### New Render Passes

```rust
impl FlameRenderer {
    pub fn render_frame(&mut self, encoder: &mut CommandEncoder) {
        // 1. Compute pass (existing)
        self.compute_pass(encoder);

        // 2. Accumulate pass (existing)
        self.accumulate_pass(encoder);

        // 3. Pre-tonemap effects chain (NEW)
        let pre_input = self.accumulated_texture();
        let pre_output = self.run_effect_chain(
            encoder,
            &self.pre_effects,
            pre_input,
            true  // has_density = true
        );

        // 4. Tonemap pass (existing, but reads from pre_output)
        self.tonemap_pass(encoder, pre_output);

        // 5. Post-tonemap effects chain (NEW)
        let post_input = self.tonemapped_texture();
        let _final = self.run_effect_chain(
            encoder,
            &self.post_effects,
            post_input,
            false  // has_density = false
        );
    }

    fn run_effect_chain(
        &self,
        encoder: &mut CommandEncoder,
        effects: &[Effect],
        input: &TextureView,
        has_density: bool,
    ) -> TextureView {
        // Ping-pong between two intermediate textures
        // Only run enabled effects
        // Return final output texture
    }
}
```

### Shader Organization

```
shaders/
  effects/
    pre/
      density_blur.wgsl
      sharpen.wgsl
      density_edge.wgsl
    post/
      hue_cycle.wgsl
      bloom.wgsl      (may need multiple passes)
      chromatic_aberration.wgsl
      vignette.wgsl
      film_grain.wgsl
      color_grade.wgsl
```

### Uniform Buffer for Effects

```rust
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EffectParams {
    // Common
    pub enabled: u32,
    pub time: f32,           // for animated effects
    pub resolution: [f32; 2],

    // Effect-specific (padded to 256 bytes for uniform alignment)
    pub param0: f32,
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
    // ... up to ~60 floats
}
```

## UI Integration

### Effects Panel

New panel or collapsible section in Colors panel:

```
[Effects]
  [x] Pre-Tonemap Effects
      [Density Blur]        [^] [v] [x]
        Radius: [====o====] 3.0
        Threshold: [==o=====] 25%
      [Sharpen]             [^] [v] [x]
        Amount: [=====o===] 1.2

  [x] Post-Tonemap Effects
      [Hue Cycle]           [^] [v] [x]
        Speed: [===o======] 30 deg/s
      [Vignette]            [^] [v] [x]
        Intensity: [====o===] 0.5

  [+ Add Pre Effect]  [+ Add Post Effect]
```

### Effect Controls
- Enable/disable toggle per effect
- Reorder buttons (up/down arrows)
- Remove button (x)
- Effect-specific parameter sliders
- All changes go through ConfigManager for undo/redo

## Animation System Integration

The existing animation system (`src/animation/mod.rs`) supports keyframe and procedural tracks for any ConfigPath parameter. Effect parameters will integrate seamlessly.

### Existing Animation Architecture

```rust
// Animation with parameter tracks (from src/animation/mod.rs)
pub struct Animation {
    pub name: String,
    pub base_config: Option<FractalConfig>,
    pub duration: f64,
    pub tracks: HashMap<String, Track>,  // ConfigPath string → Track
    pub circular_tracks: Vec<CircularTrack>,
    pub loop_mode: LoopMode,
}

// Track can be keyframes or procedural oscillator
pub enum TrackSource {
    Keyframes { keyframes: Vec<Keyframe> },
    Oscillator { oscillator_type, center, amplitude, frequency, phase },
}
```

### Adding Effect Parameters to Animation

Effect parameters become animatable by adding their ConfigPath strings to `animatable_parameters()` in `src/ui/track_editor.rs`:

```rust
// Add to track_editor.rs animatable_parameters()
ParameterCategory {
    name: t!("track_editor.category_effects").to_string(),
    params: vec![
        // Pre-tonemap effects
        ("Density Blur Radius".to_string(), "PreEffect.DensityBlur.radius".to_string()),
        ("Density Blur Threshold".to_string(), "PreEffect.DensityBlur.threshold".to_string()),
        // Post-tonemap effects
        ("Hue Cycle Speed".to_string(), "PostEffect.HueCycle.speed".to_string()),
        ("Hue Cycle Offset".to_string(), "PostEffect.HueCycle.offset".to_string()),
        ("Bloom Intensity".to_string(), "PostEffect.Bloom.intensity".to_string()),
        ("Vignette Intensity".to_string(), "PostEffect.Vignette.intensity".to_string()),
        // ... etc
    ],
},
```

### Animation Use Cases for Effects

1. **Keyframe animation** - Manually animate effect intensity over time
   ```rust
   Track::linear(json!(0.0), json!(1.0), 10.0)  // Fade in bloom over 10s
   ```

2. **Oscillator animation** - Procedural looping effects
   ```rust
   Track::oscillator(OscillatorType::Sine, 180.0, 180.0, 0.5)  // Hue cycle ±180° at 0.5Hz
   ```

3. **Beat-sync** - Use oscillators synced to music tempo
   ```rust
   // 120 BPM = 2 Hz
   Track::oscillator(OscillatorType::Square, 0.5, 0.5, 2.0)  // Flash vignette on beat
   ```

### Real-time vs Export Animation

- **Real-time playback**: `AnimationController` updates ConfigManager each frame
- **Export**: Renders each frame at specified iterations, encodes to video via ffmpeg
- Effect parameters work identically in both modes

### Time Uniform for Shader Effects

Some effects need frame time for internal animation (e.g., FilmGrain noise seed):

```wgsl
struct EffectParams {
    enabled: u32,
    time: f32,        // Current animation time (seconds)
    frame_index: u32, // For noise seed variation
    // ... effect-specific params
}
```

The `time` uniform comes from `AnimationController.current_time()` during playback, or elapsed wall-clock time during interactive use.

## Implementation Phases

### Phase 1: Infrastructure
- [ ] Add PreEffect/PostEffect enums to config
- [ ] Add ConfigPath variants for effects
- [ ] Create effect chain runner in renderer
- [ ] Add intermediate textures for ping-pong

### Phase 2: First Effects
- [ ] Implement DensityBlur (pre-tonemap)
- [ ] Implement HueCycle (post-tonemap)
- [ ] Basic UI for enabling/configuring effects

### Phase 3: Full Effect Suite
- [ ] Remaining pre-tonemap effects
- [ ] Remaining post-tonemap effects
- [ ] Effect reordering UI
- [ ] Add/remove effects UI

### Phase 4: Polish
- [ ] Effect presets
- [ ] LUT library for ColorGrade
- [ ] Performance optimization (skip disabled effects)
- [ ] Animation timeline integration

## Performance Considerations

- **Skip disabled effects** - Don't even bind/dispatch if effect.enabled == false
- **Separable blur** - Use H+V passes for gaussian blur (O(r) vs O(r^2))
- **Texture reuse** - Ping-pong between two intermediate textures
- **Conditional compilation** - Could generate uber-shader with only enabled effects for final export

## Open Questions

1. Should effect order be per-config or global preference?
2. How to handle effect presets (save/load effect chains)?
3. Should some effects (like Bloom) have multiple internal passes?
4. LUT format for ColorGrade - standard .cube files?
