# Per-Transform Color Adjustments

**Status:** Planning
**Created:** 2025-11-16
**Priority:** Medium

## Overview

Add per-transform color adjustment controls (brightness, saturation, hue shift, etc.) to allow fine-grained control over color output independently from palette selection.

## Goals

- Give each transform individual color adjustment controls
- Work with all ColorModes (Palette, TransformIndex, Speed)
- Can be toggled on/off per transform
- GPU-based for performance (apply per-sample)
- Start simple (brightness/opacity), expand later

## Design Decisions

### Data Structure

Add a `ColorAdjustment` struct to hold adjustment parameters:

```rust
// src/scene/transforms.rs

#[derive(Debug, Clone, Copy)]
pub struct ColorAdjustment {
    pub enabled: bool,          // Enable/disable adjustments for this transform
    pub brightness: f32,        // -1.0 to 1.0 (default 0.0 = no change)
    pub opacity: f32,           // 0.0 to 1.0 (default 1.0 = fully opaque)
    // Future additions:
    // pub saturation: f32,     // 0.0 to 2.0 (default 1.0 = no change)
    // pub hue_shift: f32,      // 0.0 to 360.0 degrees (default 0.0)
    // pub contrast: f32,       // 0.0 to 2.0 (default 1.0 = no change)
}

impl Default for ColorAdjustment {
    fn default() -> Self {
        Self {
            enabled: false,
            brightness: 0.0,
            opacity: 1.0,
        }
    }
}

pub struct Transform {
    // ... existing fields ...
    pub color: f32,
    pub color_adjust: ColorAdjustment,  // NEW
}
```

### GPU Buffer Layout

Extend `GpuTransform` to include color adjustment parameters:

```rust
// src/gpu/buffers.rs

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuTransform {
    // ... existing fields (affine, variations, etc.) ...
    pub color: f32,

    // NEW: Color adjustments (4 bytes each)
    pub color_brightness: f32,   // -1.0 to 1.0
    pub color_opacity: f32,      // 0.0 to 1.0
    pub color_adjust_enabled: u32,  // 0 or 1 (bool as u32)
    pub _padding3: u32,          // Align to 16 bytes
}
```

**Buffer Size Impact:**
- Current: ~160 bytes per transform
- After: ~176 bytes per transform (+16 bytes)
- Total increase: 16 × 32 transforms = 512 bytes (negligible)

### Shader Implementation

Apply adjustments in compute shader after color is determined:

```wgsl
// shaders/core/trajectory.wgsl (2D) and trajectory_3d.wgsl (3D)

// After determining base color from palette/mode
var final_color = base_color;  // vec3<f32>

// Apply per-transform color adjustments if enabled
let xform_data = transforms[current_xform];
if (xform_data.color_adjust_enabled != 0u) {
    // Brightness adjustment: simple additive
    final_color += vec3<f32>(xform_data.color_brightness);
    final_color = clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Opacity affects density (alpha channel)
    density *= xform_data.color_opacity;
}

// Write to histogram with adjusted color
let write_color = vec4<f32>(final_color, density);
```

**Shader Changes:**
- Minimal performance impact (2 conditionals, 3 math ops per iteration)
- Applied per-sample (most accurate)
- Works with all color modes

### UI Design

Add collapsible section in Transform panel:

```
Transform Panel
├─ Affine Parameters
├─ Variations
│  └─ (variation controls)
└─ Color Adjustments           ← NEW SECTION
   ├─ [✓] Enable Color Adjustments
   ├─ Brightness:  [====|====] 0.0  (-1.0 to 1.0)
   └─ Opacity:     [====|====] 1.0  (0.0 to 1.0)
```

**UI Code Pattern:**
```rust
// src/ui/transforms.rs

ui.collapsing("Color Adjustments", |ui| {
    let mut enabled = transform.color_adjust.enabled;
    if ui.checkbox(&mut enabled, "Enable").changed() {
        config_manager.update_param(
            ConfigPath::TransformColorAdjustEnabled { index },
            enabled.into(),
            false,
        )?;
    }

    if enabled {
        // Brightness slider
        let mut brightness = transform.color_adjust.brightness;
        let response = ui.add(
            egui::Slider::new(&mut brightness, -1.0..=1.0)
                .text("Brightness")
        );
        if response.changed() {
            config_manager.update_param(
                ConfigPath::TransformColorBrightness { index },
                brightness.into(),
                response.dragged(),
            )?;
        }

        // Opacity slider
        let mut opacity = transform.color_adjust.opacity;
        let response = ui.add(
            egui::Slider::new(&mut opacity, 0.0..=1.0)
                .text("Opacity")
        );
        if response.changed() {
            config_manager.update_param(
                ConfigPath::TransformColorOpacity { index },
                opacity.into(),
                response.dragged(),
            )?;
        }
    }
});
```

## Implementation Plan

### Phase 1: Core Structure
**Files:** `src/scene/transforms.rs`, `src/config/delta.rs`

1. Add `ColorAdjustment` struct to `transforms.rs`
2. Add `color_adjust` field to `Transform`
3. Implement `Default` (all adjustments neutral)
4. Add to `Transform::new()` initialization

5. Add ConfigPath variants:
   ```rust
   pub enum ConfigPath {
       // ... existing variants ...
       TransformColorAdjustEnabled { index: usize },
       TransformColorBrightness { index: usize },
       TransformColorOpacity { index: usize },
   }
   ```

6. Add ConfigValue handling in `get_value()` and `set_value()`

### Phase 2: GPU Integration
**Files:** `src/gpu/buffers.rs`, `shaders/core/header.wgsl`

1. Update `GpuTransform` struct:
   - Add `color_brightness: f32`
   - Add `color_opacity: f32`
   - Add `color_adjust_enabled: u32`
   - Add padding for alignment

2. Update `transform_to_gpu()` conversion:
   ```rust
   fn transform_to_gpu(transform: &Transform, variation_registry: &VariationRegistry) -> GpuTransform {
       GpuTransform {
           // ... existing fields ...
           color_brightness: transform.color_adjust.brightness,
           color_opacity: transform.color_adjust.opacity,
           color_adjust_enabled: if transform.color_adjust.enabled { 1 } else { 0 },
           _padding3: 0,
       }
   }
   ```

3. Update WGSL struct in `shaders/core/header.wgsl`:
   ```wgsl
   struct Transform {
       // ... existing fields ...
       color_brightness: f32,
       color_opacity: f32,
       color_adjust_enabled: u32,
       _padding3: u32,
   }
   ```

### Phase 3: Shader Application
**Files:** `shaders/core/trajectory.wgsl`, `shaders/core/trajectory_3d.wgsl`

1. Find color output section (after palette lookup / color determination)

2. Add adjustment application:
   ```wgsl
   // Apply per-transform color adjustments
   if (xform.color_adjust_enabled != 0u) {
       result_color += vec3<f32>(xform.color_brightness);
       result_color = clamp(result_color, vec3<f32>(0.0), vec3<f32>(1.0));
       density *= xform.color_opacity;
   }
   ```

3. Test with all ColorModes:
   - Palette (most common)
   - TransformIndex
   - Speed

### Phase 4: UI Controls
**Files:** `src/ui/transforms.rs`

1. Add collapsible "Color Adjustments" section
2. Add enable checkbox
3. Add brightness slider (-1.0 to 1.0)
4. Add opacity slider (0.0 to 1.0)
5. Wire up to ConfigManager with preview mode support

### Phase 5: Serialization & Compatibility
**Files:** `src/scene/transforms.rs`, `src/config/fractal_config.rs`

1. Ensure `ColorAdjustment` derives `Serialize`, `Deserialize`
2. Add defaults for backward compatibility:
   ```rust
   #[serde(default)]
   pub color_adjust: ColorAdjustment,
   ```
3. Test loading old presets (should get default neutral values)
4. Test saving/loading with adjustments enabled

### Phase 6: Testing
**Files:** `tests/`, visual testing

1. Unit tests:
   - Default values are neutral
   - Serialization round-trip
   - ConfigManager updates

2. Integration tests:
   - GPU buffer upload
   - Shader compilation
   - All ColorModes work

3. Visual tests:
   - Brightness makes colors lighter/darker
   - Opacity fades transform contribution
   - Works with multiple transforms
   - Undo/redo works correctly

## Future Enhancements

Once basic brightness/opacity are working, add:

### Phase 7: Saturation Control
```rust
pub saturation: f32,  // 0.0 to 2.0 (default 1.0)
```

```wgsl
// Convert RGB to HSV, adjust S, convert back
if (xform.color_saturation != 1.0) {
    result_color = rgb_to_hsv(result_color);
    result_color.y *= xform.color_saturation;
    result_color = hsv_to_rgb(result_color);
}
```

### Phase 8: Hue Shift
```rust
pub hue_shift: f32,  // 0.0 to 360.0 degrees (default 0.0)
```

```wgsl
// Rotate hue in HSV space
if (xform.color_hue_shift != 0.0) {
    result_color = rgb_to_hsv(result_color);
    result_color.x = fract(result_color.x + xform.color_hue_shift / 360.0);
    result_color = hsv_to_rgb(result_color);
}
```

### Phase 9: Contrast
```rust
pub contrast: f32,  // 0.0 to 2.0 (default 1.0)
```

```wgsl
// Stretch/compress around midpoint
if (xform.color_contrast != 1.0) {
    let mid = vec3<f32>(0.5);
    result_color = mid + (result_color - mid) * xform.color_contrast;
    result_color = clamp(result_color, vec3<f32>(0.0), vec3<f32>(1.0));
}
```

## Performance Considerations

**GPU Buffer:**
- Current: 32 transforms × 160 bytes = 5,120 bytes
- After: 32 transforms × 176 bytes = 5,632 bytes
- Increase: 512 bytes (0.5 KB) - negligible

**Shader Performance:**
- Brightness: 1 vector add, 1 clamp (~2 cycles)
- Opacity: 1 scalar multiply (~1 cycle)
- Total: ~3 cycles per iteration (negligible)
- Future (HSV): ~20 cycles per iteration (still fast)

**Memory:**
- Transform struct size: +16 bytes
- No additional textures or buffers needed
- Minimal serialization overhead

## Use Cases

1. **Fade specific transforms** - Use opacity to reduce contribution
2. **Brighten highlights** - Increase brightness on accent transforms
3. **Darken shadows** - Decrease brightness on base transforms
4. **Color variety** - Different transforms with different hue shifts
5. **Contrast control** - Fine-tune color depth per transform

## Open Questions

1. **Color space for adjustments**: RGB (simple) vs HSV (more intuitive)?
   - Start with RGB for brightness (simple additive)
   - Use HSV for saturation/hue (more complex but better)

2. **Adjustment order**: Brightness → Saturation → Hue → Opacity?
   - Generally: Hue → Saturation → Brightness → Opacity

3. **Opacity vs Density**: Should opacity affect alpha or density?
   - Decision: Affect density (same as reducing variation weights)

4. **Clipping behavior**: Clamp or wrap when colors go out of range?
   - Decision: Clamp for brightness/contrast, wrap for hue

## Success Criteria

- [ ] ColorAdjustment struct implemented with defaults
- [ ] GPU buffers updated with new fields
- [ ] Shaders apply brightness and opacity adjustments
- [ ] UI has collapsible Color Adjustments section
- [ ] Works with all ColorModes (Palette, TransformIndex, Speed)
- [ ] Backward compatible (old presets load correctly)
- [ ] Undo/redo works for all adjustment parameters
- [ ] No visible performance impact
- [ ] Can create visually distinct effects with adjustments

## Notes

- Start minimal (brightness + opacity only)
- Expand to saturation/hue/contrast once core is working
- Keep all adjustments optional (enabled flag)
- Default to neutral values (no effect when disabled)
- Use ConfigManager for proper undo/redo support
