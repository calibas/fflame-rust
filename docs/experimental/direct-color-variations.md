# Direct Color Variations Support

**Status:** Planning
**Priority:** Medium
**Estimated Effort:** 10-14 hours
**Complexity:** High (variation system extension + shader architecture)

---

## Overview

Implement DirectColor (DC) variations that can directly modify the color coordinate during iteration, bypassing the standard color evolution formula. Add a `direct_color` parameter (Apophysis: `pluginColor`) to control blending strength.

---

## Background

### Standard Color Evolution (Current Implementation)

**Step 1: Color Speed Blending**
```wgsl
let symmetry = xform.color_speed;
let colorC1 = (1.0 + symmetry) / 2.0;
let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
color_index = color_index * colorC1 + colorC2;
```

Blends inherited color with transform base color using `color_speed` (symmetry).

**Step 2: Variations Execute**
Variations modify position (x, y, z) but **not color**.

**Result:** Color evolves through transform color blending only.

---

### Apophysis 3-Step Color System

Apophysis uses a 3-step color blending system (XForm.pas:312-313, 1067, 1078-1081):

**Step 1: Color Speed Blending** ✅ (Already implemented)
```pascal
colorC1 := (1 + symmetry)/2;
colorC2 := color*(1 - symmetry)/2;
CPpoint.c := CPpoint.c * colorC1 + colorC2;
vc := CPpoint.c;
```

**Step 2: Variation Execution** ⚠️ (Variations can't modify color yet)
```pascal
for i:= 0 to FNrFunctions-1 do
  FCalcFunctionList[i];  // DirectColor variations can modify 'vc'
```

DirectColor variations can modify `vc` (variation color variable).

**Step 3: Direct Color Blending** ❌ (Missing!)
```pascal
CPpoint.c := CPpoint.c + pluginColor * (vc - CPpoint.c);
```

Blends variation-modified color back:
```
c_final = c_base + direct_color × (vc - c_base)
```

Where:
- `c_base` = color after Step 1 (color_speed blending)
- `vc` = color modified by DirectColor variations
- `direct_color` = blend strength (0.0 to 1.0)

**Examples:**
- `direct_color = 0.0` → No effect, use `c_base` (standard evolution)
- `direct_color = 1.0` → Fully use `vc` (DirectColor variation controls color)
- `direct_color = 0.5` → 50% blend between `c_base` and `vc`

---

## DirectColor Variations

DirectColor variations set the color coordinate `vc` based on position, iteration, or other calculations.

### From Apophysis Plugin Directory

**11 DirectColor Variations:**

1. **dc_linear** - Linear gradient based on position
   ```c
   TC = fmod(fabs(0.5 * (ldcs * ((c * FPx + s * FPy + offset)) + 1.0)), 1.0);
   ```
   Color from rotated position along a line.

2. **dc_bubble** - Radial distance from center
   ```c
   TC = fmod(fabs(bdcs * (sqr(FPx + centerx) + sqr(FPy + centery))), 1.0);
   ```
   Color based on distance from bubble center.

3. **dc_triangle** - Triangle-based coloring

4. **dc_mandelbrot** - Mandelbrot iteration coloring

5. **dc_cube** - Cube-based coloring

6. **dc_carpet** - Carpet pattern coloring

7. **dc_gridout** - Grid pattern coloring

8. **dc_boarders** - Border-based coloring

9. **dc_ztransl** - Z-translation coloring (3D)

10. **dc_image** - Image-based coloring

11. **julian2dc** - Julian with color from angle/radius
    ```c
    TC = fmod(fabs(r * col + (angle / M_2PI) * (1 - col)), 1.0);
    ```
    Blends radius-based and angle-based coloring.

---

## Proposed Implementation

### Phase 1: Add direct_color Parameter (2-3 hours)

**Goal:** Add transform parameter for DirectColor blending strength.

**Step 1.1: Transform Data Structure**
```rust
// src/scene/transforms.rs
pub struct Transform {
    // ... existing fields ...
    pub color: f32,        // Palette position (0.0-1.0)
    pub color_speed: f32,  // Symmetry (-1.0 to 1.0)
    pub opacity: f32,      // Transform opacity (0.0-1.0)

    /// DirectColor blend strength (Apophysis: pluginColor)
    /// 0.0 = No DirectColor effect (standard evolution)
    /// 1.0 = Full DirectColor effect (variations control color)
    pub direct_color: f32,  // NEW
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            direct_color: 0.0,  // No effect by default
        }
    }
}
```

**Step 1.2: GPU Buffer**
```rust
// src/gpu/buffers.rs
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuTransform {
    // ... existing fields ...

    // Color (vec4 for alignment)
    pub color: f32,
    pub color_speed: f32,
    pub opacity: f32,
    pub direct_color: f32,  // NEW (replaces _padding)
}

impl GpuTransform {
    pub fn from_transform(xform: &Transform, registry: &VariationRegistry) -> Self {
        Self {
            // ... existing fields ...
            color: xform.color,
            color_speed: xform.color_speed,
            opacity: xform.opacity,
            direct_color: xform.direct_color,  // NEW
        }
    }
}
```

**Step 1.3: Shader Header**
```wgsl
// shaders/core/header.wgsl
struct Transform {
    // ... existing fields ...

    // Color (vec4 for alignment)
    color: f32,
    color_speed: f32,
    opacity: f32,
    direct_color: f32,  // NEW
}
```

**Step 1.4: ConfigManager Integration**
```rust
// src/config/delta.rs
pub enum ConfigPath {
    // ... existing variants ...

    /// DirectColor blend strength (0.0 to 1.0)
    TransformDirectColor { index: usize },
}

impl Display for ConfigPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ... existing cases ...
            ConfigPath::TransformDirectColor { index } => {
                write!(f, "Transform {} → Direct Color", index + 1)
            }
        }
    }
}

impl ConfigPath {
    pub fn update_type(&self) -> UpdateType {
        match self {
            // ... existing cases ...
            ConfigPath::TransformDirectColor { .. } => UpdateType::Flame,
        }
    }
}
```

```rust
// src/config/manager.rs
impl ConfigManager {
    fn get_value(&self, path: &ConfigPath) -> Result<ConfigValue> {
        match path {
            // ... existing cases ...
            ConfigPath::TransformDirectColor { index } => {
                let xform = config.flame.transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.direct_color.into())
            }
        }
    }

    fn apply_delta_commit(&mut self, delta: &ConfigDelta) -> Result<UpdateType> {
        match &delta.path {
            // ... existing cases ...
            ConfigPath::TransformDirectColor { index } => {
                let xform = self.current.flame.transforms.get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.direct_color = delta.new_value.try_into()?;
                Ok(UpdateType::Flame)
            }
        }
    }

    fn apply_delta_preview(&mut self, delta: &ConfigDelta) -> Result<UpdateType> {
        // Same as commit mode
        match &delta.path {
            // ... existing cases ...
            ConfigPath::TransformDirectColor { index } => {
                let xform = preview.flame.transforms.get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.direct_color = delta.new_value.try_into()?;
                Ok(UpdateType::Flame)
            }
        }
    }
}
```

**Step 1.5: UI Slider**
```rust
// src/ui/transforms.rs
fn render_color_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // ... existing color controls (palette position, color_speed, opacity) ...

    // Direct Color slider
    ui.horizontal(|ui| {
        ui.label("Direct Color:");
        if ui.button("ⓘ")
            .on_hover_text("Blend strength for DirectColor variations.\n\
                            0.0 = Standard color evolution\n\
                            1.0 = Variations control color")
            .clicked()
        {
            // TODO: Show help panel
        }
    });

    let mut temp_direct_color = transform.direct_color;
    let response = ui.add(
        egui::Slider::new(&mut temp_direct_color, 0.0..=1.0)
            .text("Direct Color")
    );

    if response.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformDirectColor { index },
            temp_direct_color.into(),
            response.dragged()
        ) {
            transform.direct_color = config_manager.active_config()
                .flame.transforms[index].direct_color;
            max_update = max_update.max(update_type);
        }
    }

    if response.drag_stopped() {
        let _ = config_manager.force_commit_preview(
            &ConfigPath::TransformDirectColor { index }
        );
    }

    max_update
}
```

**Step 1.6: XML Import/Export**
```rust
// src/apophysis_xml.rs

// Import
fn parse_xform_direct_color(xform_elem: &Element) -> f32 {
    xform_elem
        .get_attr("pluginColor")
        .or_else(|| xform_elem.get_attr("plugin_color"))  // Alternative name
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0)  // Default: no DirectColor effect
}

// Export
fn write_xform_direct_color(xform: &Transform, writer: &mut Writer) -> Result<()> {
    // Only write if non-zero (optimization)
    if xform.direct_color.abs() > 1e-6 {
        writer.write_attribute("pluginColor", &format!("{:.6}", xform.direct_color))?;
    }
    Ok(())
}
```

---

### Phase 2: Shader Color Blending (2-3 hours)

**Goal:** Implement Apophysis Step 3 formula in shaders.

**Step 2.1: Add Variation Color Variable**

Update both 2D and 3D shaders to track variation color:

```wgsl
// shaders/core/main_2d.wgsl
// shaders/core/main_3d.wgsl

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // ... burn-in loop ...

    for (var iter = 0u; iter < params.iterations_per_thread; iter++) {
        // ... select transform ...

        // Calculate speed (distance traveled)
        let speed = length(current - old_pos);

        // STEP 1: Color Speed Blending (existing)
        var color_base: f32;  // Color after color_speed blending
        if (params.color_mode == 0u) {
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
            color_base = color_index * colorC1 + colorC2;
        } else {
            color_base = color_index;
        }

        // STEP 2: Apply transform (affine + variations)
        current = apply_transform(&xform, current, xform_id, &rng);

        // NEW: Initialize variation color to base color
        var vc = color_base;

        // STEP 2: Variation execution (with DirectColor support)
        // DirectColor variations can modify 'vc'
        // TODO: This will be implemented in Phase 3

        // STEP 3: Direct Color Blending (NEW)
        if (xform.direct_color > 0.0) {
            // Blend between base color and variation color
            color_index = color_base + xform.direct_color * (vc - color_base);
        } else {
            color_index = color_base;
        }

        // ... rest of iteration (histogram accumulation) ...
    }
}
```

**Step 2.2: Test with Existing Flames**

Since `direct_color` defaults to 0.0:
- Existing flames should render identically
- No visual change unless `direct_color > 0.0`
- Shader change should have no performance impact

---

### Phase 3: DirectColor Variation Support (6-8 hours)

**Goal:** Extend variation system to return color values.

**Challenge:** Current variations only return position:
```wgsl
fn variation_linear(p: vec2<f32>) -> vec2<f32>
fn variation_sinusoidal(p: vec2<f32>) -> vec2<f32>
```

DirectColor variations need to return **both** position and color:
```wgsl
struct VariationResult {
    pos: vec2<f32>,  // or vec3<f32> for 3D
    color: f32,      // Optional color override
    has_color: bool, // Whether color was set
}
```

**Step 3.1: Extend Variation Registry**

Mark variations as DirectColor capable:
```rust
// src/variations/mod.rs
#[derive(Debug, Clone)]
pub struct VariationInfo {
    pub name: String,
    pub display_name: String,
    pub category: VariationCategory,
    pub needs_rng: bool,
    pub parameters: Vec<VariationParameter>,
    pub is_direct_color: bool,  // NEW: Can this variation modify color?
}

impl VariationRegistry {
    pub fn register_direct_color(
        &mut self,
        name: &str,
        display_name: &str,
        category: VariationCategory,
        needs_rng: bool,
    ) {
        self.register(VariationInfo {
            name: name.to_string(),
            display_name: display_name.to_string(),
            category,
            needs_rng,
            parameters: Vec::new(),
            is_direct_color: true,  // NEW
        });
    }
}
```

**Step 3.2: Update Shader Builder**

Generate different function signatures for DirectColor variations:

```rust
// src/shader_builder_v2.rs
impl ShaderBuilder {
    fn generate_variation_call(
        &self,
        var_name: &str,
        var_id: u32,
        info: &VariationInfo,
    ) -> String {
        if info.is_direct_color {
            // DirectColor variation returns both position and color
            let call = if info.needs_rng {
                if info.parameters.is_empty() {
                    format!("variation_{}(p, &rng)", var_name)
                } else {
                    format!("variation_{}(p, xform_id, &rng)", var_name)
                }
            } else {
                if info.parameters.is_empty() {
                    format!("variation_{}(p)", var_name)
                } else {
                    format!("variation_{}(p, xform_id)", var_name)
                }
            };

            format!(
                "let result_{} = {};\n\
                 result += weight_{} * result_{}.pos;\n\
                 if (result_{}.has_color) {{\n\
                     vc = result_{}.color;\n\
                 }}",
                var_id, call, var_id, var_id, var_id, var_id
            )
        } else {
            // Standard variation (position only)
            // ... existing code ...
        }
    }
}
```

**Step 3.3: Implement DirectColor Variations**

**Example: dc_linear**
```wgsl
// shaders/core/variations_2d.wgsl (or variations_3d.wgsl)

struct VariationResult2D {
    pos: vec2<f32>,
    color: f32,
    has_color: bool,
}

fn variation_dc_linear(p: vec2<f32>, xform_id: u32) -> VariationResult2D {
    // Parameters
    let offset = get_param(xform_id, VAR_DC_LINEAR, 0u);  // offset
    let angle = get_param(xform_id, VAR_DC_LINEAR, 1u);   // angle (radians)

    // Rotation
    let c = cos(angle);
    let s = sin(angle);

    // Color from rotated position
    let color_val = fract(abs(0.5 * (c * p.x + s * p.y + offset) + 1.0));

    return VariationResult2D(
        p,              // Position unchanged (linear is DC-only)
        color_val,      // Color from position
        true            // Has color
    );
}
```

**Example: dc_bubble**
```wgsl
fn variation_dc_bubble(p: vec2<f32>, xform_id: u32) -> VariationResult2D {
    // Parameters
    let center_x = get_param(xform_id, VAR_DC_BUBBLE, 0u);
    let center_y = get_param(xform_id, VAR_DC_BUBBLE, 1u);
    let scale = get_param(xform_id, VAR_DC_BUBBLE, 2u);

    // Color from distance to center
    let dx = p.x + center_x;
    let dy = p.y + center_y;
    let dist_sq = dx * dx + dy * dy;
    let color_val = fract(abs(scale * dist_sq));

    return VariationResult2D(
        p,              // Position unchanged
        color_val,      // Color from distance
        true            // Has color
    );
}
```

**Example: julian2dc**
```wgsl
fn variation_julian2dc(p: vec2<f32>, xform_id: u32, rng: ptr<function, RngState>) -> VariationResult2D {
    // Parameters
    let power = get_param(xform_id, VAR_JULIAN2DC, 0u);
    let dist = get_param(xform_id, VAR_JULIAN2DC, 1u);
    let col = get_param(xform_id, VAR_JULIAN2DC, 2u);  // Color blend (0-1)

    // Julian calculation
    let r = pow(length(p), dist / power);
    let theta = atan2(p.y, p.x);
    let t = (theta + TWO_PI * f32(rng_next_u32(rng) % u32(abs(power)))) / power;

    let new_pos = vec2(r * cos(t), r * sin(t));

    // Color: blend between radius and angle
    let color_val = fract(abs(r * col + (theta / TWO_PI) * (1.0 - col)));

    return VariationResult2D(
        new_pos,        // Position modified (standard julian)
        color_val,      // Color from r/theta
        true            // Has color
    );
}
```

**Step 3.4: Register DirectColor Variations**
```rust
// src/variations/mod.rs
impl VariationRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();

        // ... existing variations ...

        // DirectColor variations
        registry.register_direct_color(
            "dc_linear",
            "DC Linear",
            VariationCategory::DirectColor,
            false,
        );
        registry.add_parameters("dc_linear", vec![
            VariationParameter::new_float("offset", "Offset", 0.0, Some(-5.0), Some(5.0)),
            VariationParameter::new_angle("angle", "Angle", 0.0),
        ]);

        registry.register_direct_color(
            "dc_bubble",
            "DC Bubble",
            VariationCategory::DirectColor,
            false,
        );
        registry.add_parameters("dc_bubble", vec![
            VariationParameter::new_float("center_x", "Center X", 0.0, Some(-2.0), Some(2.0)),
            VariationParameter::new_float("center_y", "Center Y", 0.0, Some(-2.0), Some(2.0)),
            VariationParameter::new_float("scale", "Scale", 1.0, Some(0.1), Some(10.0)),
        ]);

        registry.register_direct_color(
            "julian2dc",
            "Julian 2 DC",
            VariationCategory::DirectColor,
            true,  // Needs RNG
        );
        registry.add_parameters("julian2dc", vec![
            VariationParameter::new_integer("power", "Power", 2, Some(-10), Some(10)),
            VariationParameter::new_float("dist", "Distance", 1.0, Some(0.1), Some(5.0)),
            VariationParameter::new_float("col", "Color Blend", 0.5, Some(0.0), Some(1.0)),
        ]);

        registry
    }
}
```

---

## Implementation Summary

### Phase 1: Add direct_color Parameter (2-3 hours)
- ✅ Add `direct_color: f32` to Transform struct (default: 0.0)
- ✅ Add to GPU buffer (GpuTransform)
- ✅ Add to shader struct (header.wgsl)
- ✅ Add ConfigPath::TransformDirectColor
- ✅ Add UI slider (0.0 to 1.0)
- ✅ Parse `pluginColor` from XML
- ✅ Write `pluginColor` to XML (if non-zero)

### Phase 2: Shader Color Blending (2-3 hours)
- ✅ Add `vc` (variation color) variable to shaders
- ✅ Implement Step 3 formula: `c_final = c_base + direct_color × (vc - c_base)`
- ✅ Test with existing flames (should have no effect)

### Phase 3: DirectColor Variation Support (6-8 hours)
- ✅ Extend VariationRegistry with `is_direct_color` flag
- ✅ Update ShaderBuilder to handle DirectColor variations
- ✅ Implement 3 core DirectColor variations:
  - `dc_linear` - Linear gradient
  - `dc_bubble` - Radial gradient
  - `julian2dc` - Julian with color
- ✅ Add VariationCategory::DirectColor to registry

---

## Files to Create/Modify

### Modified Files
- `src/scene/transforms.rs` - Add `direct_color` field
- `src/gpu/buffers.rs` - Add `direct_color` to GpuTransform (replaces `_padding`)
- `shaders/core/header.wgsl` - Add `direct_color` to Transform struct
- `shaders/core/main_2d.wgsl` - Implement Step 3 formula, add `vc` variable
- `shaders/core/main_3d.wgsl` - Implement Step 3 formula, add `vc` variable
- `src/config/delta.rs` - Add `TransformDirectColor` variant
- `src/config/manager.rs` - Implement get/set for direct_color
- `src/ui/transforms.rs` - Add Direct Color slider
- `src/apophysis_xml.rs` - Parse/write `pluginColor` attribute
- `src/variations/mod.rs` - Add `is_direct_color` field, add DirectColor category
- `src/shader_builder_v2.rs` - Handle DirectColor variation calls
- `shaders/core/variations_2d.wgsl` - Add `VariationResult2D` struct and DirectColor variations
- `shaders/core/variations_3d.wgsl` - Add `VariationResult3D` struct and DirectColor variations

---

## Use Cases

### 1. Linear Gradient Coloring
```rust
// Transform with dc_linear
xform.variations.insert("dc_linear".to_string(), 1.0);
xform.direct_color = 1.0;  // Full DirectColor effect
```
Color evolves as linear gradient across position.

### 2. Radial Color Burst
```rust
// Transform with dc_bubble
xform.variations.insert("dc_bubble".to_string(), 1.0);
xform.direct_color = 0.7;  // 70% DirectColor, 30% standard evolution
```
Creates radial color patterns from center.

### 3. Julian with Color Bands
```rust
// Transform with julian2dc
xform.variations.insert("julian2dc".to_string(), 1.0);
xform.direct_color = 1.0;
// Set parameters: power=5, dist=1.0, col=0.5
```
Julian shape with color bands based on angle/radius.

### 4. Blended DirectColor
```rust
// Mix DirectColor with standard evolution
xform.direct_color = 0.5;  // 50/50 blend
```
Subtle color hints from DirectColor, combined with palette evolution.

---

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_direct_color_default() {
    let xform = Transform::default();
    assert_eq!(xform.direct_color, 0.0);
}

#[test]
fn test_direct_color_xml_import() {
    let xml = r#"<xform ... pluginColor="0.75" />"#;
    let xform = parse_xform(xml);
    assert!((xform.direct_color - 0.75).abs() < 1e-6);
}

#[test]
fn test_direct_color_roundtrip() {
    // Import → export → import preserves direct_color
}
```

### Visual Tests
1. **No effect test:** Load flame with `direct_color=0.0`, verify identical to current
2. **Full DC test:** Load flame with `dc_linear` and `direct_color=1.0`, verify gradient
3. **Blend test:** Test various `direct_color` values (0.25, 0.5, 0.75)
4. **Apophysis comparison:** Import Apophysis flame with DirectColor, compare visually

### Performance Tests
- Measure iteration speed with/without DirectColor variations
- Ensure `VariationResult` struct doesn't slow standard variations
- Target: < 5% performance impact for DirectColor variations

---

## Technical Considerations

### Shader Architecture Challenge

**Problem:** Current variations return position only. DirectColor needs position + color.

**Solution:** Use result structs for DirectColor variations:
```wgsl
struct VariationResult2D {
    pos: vec2<f32>,
    color: f32,
    has_color: bool,
}
```

Standard variations still return `vec2<f32>` (no change).

### Backward Compatibility

- `direct_color` defaults to 0.0 → No effect on existing flames
- Shader Step 3 only executes if `direct_color > 0.0` → No performance impact
- Old configs without `direct_color` field load correctly

### 2D vs 3D Variations

DirectColor variations need separate implementations:
- `VariationResult2D` for 2D mode (`variations_2d.wgsl`)
- `VariationResult3D` for 3D mode (`variations_3d.wgsl`)

Most DirectColor variations are 2D (position-based coloring).

### DirectColor + Opacity

Both `direct_color` and `opacity` can be used together:
- `opacity` controls transform visibility (histogram accumulation)
- `direct_color` controls color blending strength (separate concern)

---

## Success Criteria

### Functionality
- [x] `direct_color` parameter stored per transform
- [x] Step 3 formula implemented in shaders
- [x] DirectColor variations can modify `vc`
- [x] XML import/export preserves `pluginColor`
- [x] UI slider for `direct_color` (0.0 to 1.0)
- [x] Undo/redo works for `direct_color` changes

### Variations
- [x] At least 3 DirectColor variations implemented:
  - `dc_linear`
  - `dc_bubble`
  - `julian2dc`
- [x] DirectColor variations appear in UI variation list
- [x] Parameters work correctly with DirectColor variations

### Compatibility
- [x] Flames without DirectColor render identically
- [x] Apophysis flames with DirectColor import correctly
- [x] Visual output reasonably matches Apophysis

### Performance
- [x] < 5% performance impact for DirectColor variations
- [x] No impact when `direct_color = 0.0` (default)

---

## Risks and Mitigations

### Risk: Shader Complexity Increase
**Impact:** Medium
**Mitigation:** Keep `VariationResult` struct simple (pos + color + flag). Standard variations unchanged.

### Risk: Visual Differences from Apophysis
**Impact:** Medium
**Mitigation:** Test with real Apophysis flames. Iterate on DirectColor variation implementations.

### Risk: UI Clutter
**Impact:** Low
**Mitigation:** Place Direct Color slider in "Advanced" section with info icon. Most users won't need it.

### Risk: Performance Impact
**Impact:** Low
**Mitigation:** DirectColor variations are optional. Step 3 formula is simple (one lerp). Profile early.

---

## Future Enhancements

### More DirectColor Variations
- `dc_triangle` - Triangle pattern coloring
- `dc_mandelbrot` - Mandelbrot iteration coloring
- `dc_cube` - Cube-based coloring (3D)
- `dc_carpet` - Carpet pattern coloring
- `dc_gridout` - Grid pattern coloring
- `dc_boarders` - Border-based coloring
- `dc_ztransl` - Z-translation coloring (3D)
- `dc_image` - Image-based coloring (requires texture support)

### DirectColor Presets
- Color mode presets: "Gradient", "Radial", "Angular"
- Quick apply DirectColor configuration
- Preview DirectColor effect before applying

### DirectColor Visualization
- Show color gradient preview in UI
- Visualize DirectColor effect in triangle editor
- Color map display panel

---

## Related Documentation

- `docs/main/COLOR.md` - Color system reference
- `docs/main/VARIATIONS.md` - Variation system reference
- `docs/projects/apophysis-remaining-features.md` - Feature #5 (DirectColor)
- Apophysis Source:
  - `XForm.pas:312-313, 1067, 1078-1081` - Color blending formula
  - `varGenericPlugin.pas:63, 338` - DirectColor plugin interface
  - `src/Plugin/dc_linear.c:48` - dc_linear implementation
  - `src/Plugin/dc_bubble.c:52` - dc_bubble implementation
  - `src/Plugin/julian2dc.c:75` - julian2dc implementation

---

## Priority Justification

**Medium Priority** because:
- Required for full Apophysis compatibility
- Some flames use DirectColor variations (not rare)
- Enables advanced color effects beyond palette
- Foundation for future DirectColor variations

**Should implement after:**
1. XML Export (#3)
2. Final Transform (#7)

**Should implement before:**
3. Xaos (#6) - Xaos is more complex and less common
4. UI improvements - DirectColor is core functionality

**Reasoning:** DirectColor is more common than Xaos, and simpler than full UI overhaul. It's a natural next step after basic Apophysis compatibility.

---

**Created:** 2025-11-08
**Status:** Planning
**Next Steps:** Review design, implement Phase 1 (direct_color parameter)
