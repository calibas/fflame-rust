# Variation Parameters System - Implementation Plan

**Goal:** Add parameter support to variations (like Apophysis), allowing variations to have configurable settings (e.g., `julian_power`, `julian_dist`).

**Reference:** [Apophysis 7X Julian Variation](https://github.com/xyrus02/apophysis-7x/blob/b39211fc2f29e177434009733181a1839a73bbfc/src/Variations/varJuliaN.pas#L10)

---

## 1. Data Model (Rust Side)

### A. Parameter Definition (src/variations/mod.rs)

```rust
/// Parameter definition for a variation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariationParameter {
    pub name: String,           // e.g., "power" (used in code)
    pub display_name: String,   // e.g., "Power" (shown in UI)
    pub param_type: ParamType,
    pub default_value: f32,
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ParamType {
    Float,     // Continuous values
    Integer,   // Whole numbers (stored as f32, cast for UI)
    Angle,     // 0-360 degrees (or radians)
}

// Add to VariationInfo:
pub struct VariationInfo {
    pub name: String,
    pub display_name: String,
    pub category: VariationCategory,
    pub wgsl_function: String,
    pub needs_rng: bool,
    pub is_core: bool,
    pub wgsl_source: Option<String>,
    pub parameters: Vec<VariationParameter>,  // NEW
}
```

### B. Parameter Storage (src/scene/transforms.rs)

```rust
pub struct Transform {
    // Affine matrix (existing)
    pub a: f32, pub b: f32, pub c: f32, pub d: f32, pub e: f32, pub f: f32,
    pub g: f32,  // Z offset for 3D
    pub weight: f32,

    // Variation weights (existing)
    pub variations: HashMap<String, f32>,

    // NEW: Variation parameters
    // Key format: "variation_name.param_name" (e.g., "julian.power")
    pub variation_params: HashMap<String, f32>,

    // Color (existing)
    pub color: [f32; 3],
    pub color_speed: f32,
}

impl Transform {
    /// Set a parameter for a specific variation
    pub fn set_variation_param(&mut self, variation: &str, param: &str, value: f32) {
        let key = format!("{}.{}", variation, param);
        self.variation_params.insert(key, value);
    }

    /// Get a parameter value (or default if not set)
    pub fn get_variation_param(&self, variation: &str, param: &str) -> Option<f32> {
        let key = format!("{}.{}", variation, param);
        self.variation_params.get(&key).copied()
    }

    /// Get parameter with fallback to default from registry
    pub fn get_variation_param_or_default(
        &self,
        variation: &str,
        param: &str,
        registry: &VariationRegistry,
    ) -> f32 {
        self.get_variation_param(variation, param)
            .or_else(|| {
                registry.get(variation)
                    .and_then(|info| info.get_param_default(param))
            })
            .unwrap_or(0.0)
    }
}
```

---

## 2. GPU Representation

### A. Separate Parameter Buffer

```rust
// src/gpu/buffers.rs

/// Maximum parameters per variation (expandable if needed)
pub const MAX_PARAMS_PER_VARIATION: usize = 8;

/// GPU representation of variation parameters for ONE transform
/// Total size: 24 variations × 8 params = 192 floats = 768 bytes per transform
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVariationParams {
    // Flat array indexed by: variation_id * MAX_PARAMS_PER_VARIATION + param_slot
    // Each variation gets MAX_PARAMS_PER_VARIATION consecutive slots
    pub params: [f32; 24 * MAX_PARAMS_PER_VARIATION],  // 192 floats
}

impl GpuVariationParams {
    /// Create from Transform using VariationRegistry
    pub fn from_transform(
        xform: &Transform,
        registry: &VariationRegistry,
    ) -> Self {
        let mut params = [0.0f32; 192];

        // For each active variation, copy its parameters
        for (var_name, _weight) in &xform.variations {
            if let Some(info) = registry.get(var_name) {
                let var_id = registry.get_id(var_name).unwrap_or(0);

                for (param_idx, param_def) in info.parameters.iter().enumerate() {
                    if param_idx >= MAX_PARAMS_PER_VARIATION {
                        break;  // Safety check
                    }

                    let value = xform.get_variation_param(var_name, &param_def.name)
                        .unwrap_or(param_def.default_value);

                    let buffer_idx = var_id * MAX_PARAMS_PER_VARIATION + param_idx;
                    params[buffer_idx] = value;
                }
            }
        }

        Self { params }
    }
}

// Add to FlameBuffers:
pub struct FlameBuffers {
    pub transform_buffer: Buffer,              // Existing
    pub params_buffer: Buffer,                 // Existing
    pub tonemap_params_buffer: Buffer,         // Existing
    pub accumulate_params_buffer: Buffer,      // Existing
    pub variation_params_buffer: Buffer,       // NEW: [GpuVariationParams; MAX_TRANSFORMS]

    // ... textures, etc.
}
```

**Memory Usage:**
- 192 floats × 4 bytes × 32 transforms = **24,576 bytes (24 KB)**
- Negligible for modern GPUs
- Most slots will be 0.0 (unused), but this is fine

---

## 3. Shader Integration (WGSL)

### A. Shader Parameter Structure

```wgsl
// shaders/core/header.wgsl

struct VariationParams {
    // Indexed as: params[variation_id * 8 + param_slot]
    params: array<f32, 192>,  // 24 variations × 8 params
}

// Add new binding (adjust binding number as needed)
@group(0) @binding(3)
var<storage, read> variation_params: array<VariationParams, 32>;  // MAX_TRANSFORMS
```

### B. Parameter Access Helper

```wgsl
// shaders/core/utilities.wgsl

/// Get a variation parameter value for a specific transform
fn get_variation_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    let idx = variation_id * 8u + param_slot;
    return variation_params[xform_id].params[idx];
}
```

### C. Variation Function Updates

**Before (no parameters):**
```wgsl
fn variation_julian(p: vec2<f32>) -> vec2<f32> {
    let power = 2.0;  // Hardcoded
    let dist = 1.0;   // Hardcoded

    let r = pow(dot(p, p), dist / f32(power) / 2.0);
    let theta = (atan2(p.y, p.x) + 2.0 * PI * random()) / f32(power);
    return r * vec2(cos(theta), sin(theta));
}
```

**After (with parameters):**
```wgsl
const JULIAN_ID: u32 = 13u;       // Registry index for julian
const JULIAN_POWER_SLOT: u32 = 0u;
const JULIAN_DIST_SLOT: u32 = 1u;

fn variation_julian(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
    // Extract parameters from buffer
    let power = get_variation_param(xform_id, JULIAN_ID, JULIAN_POWER_SLOT);
    let dist = get_variation_param(xform_id, JULIAN_ID, JULIAN_DIST_SLOT);

    let r = pow(dot(p, p), dist / power / 2.0);
    let theta = (atan2(p.y, p.x) + 2.0 * PI * random()) / power;
    return r * vec2(cos(theta), sin(theta));
}
```

**Note:** All variation functions need to accept `xform_id` parameter. This requires updating shader builder and all variation signatures.

### D. Generated Constants (ShaderBuilder)

```rust
// src/shader_builder_v2.rs

impl ShaderBuilder {
    /// Generate variation ID constants for shader
    fn build_variation_constants(&self, active_variations: &HashMap<String, f32>) -> String {
        let mut constants = String::from("// Variation IDs\n");

        for (name, &registry_id) in self.registry.id_map() {
            if active_variations.contains_key(name) {
                let const_name = format!("{}_ID", name.to_uppercase());
                constants.push_str(&format!("const {}: u32 = {}u;\n", const_name, registry_id));

                // Also generate parameter slot constants
                if let Some(info) = self.registry.get(name) {
                    for (idx, param) in info.parameters.iter().enumerate() {
                        let param_const = format!("{}_{}_SLOT", name.to_uppercase(), param.name.to_uppercase());
                        constants.push_str(&format!("const {}: u32 = {}u;\n", param_const, idx));
                    }
                }
            }
        }

        constants.push('\n');
        constants
    }
}
```

---

## 4. UI Integration

### A. Transform Window Updates (src/ui/transforms.rs)

Add parameter sliders below each active variation:

```rust
// Inside variation list rendering

for var_name in registry.names() {
    let mut weight = transform.get_variation(var_name);
    let mut changed = false;

    ui.horizontal(|ui| {
        let active = weight.abs() > 1e-6;
        if ui.checkbox(&mut active, "").changed() {
            weight = if active { 1.0 } else { 0.0 };
            changed = true;
        }

        ui.label(registry.get(var_name).unwrap().display_name);

        if active {
            if ui.add(egui::Slider::new(&mut weight, -2.0..=2.0)
                .step_by(0.01)).changed() {
                changed = true;
            }
        }
    });

    // NEW: Show parameters if variation is active
    if weight.abs() > 1e-6 {
        if let Some(info) = registry.get(var_name) {
            if !info.parameters.is_empty() {
                ui.indent(format!("params_{}", var_name), |ui| {
                    for param in &info.parameters {
                        let mut value = transform.get_variation_param(var_name, &param.name)
                            .unwrap_or(param.default_value);

                        let param_changed = match param.param_type {
                            ParamType::Float => {
                                let min = param.min_value.unwrap_or(-10.0);
                                let max = param.max_value.unwrap_or(10.0);
                                ui.add(egui::Slider::new(&mut value, min..=max)
                                    .text(&param.display_name)
                                    .step_by(0.01)).changed()
                            }
                            ParamType::Integer => {
                                let min = param.min_value.unwrap_or(1.0) as i32;
                                let max = param.max_value.unwrap_or(10.0) as i32;
                                let mut int_val = value as i32;
                                let changed = ui.add(egui::Slider::new(&mut int_val, min..=max)
                                    .text(&param.display_name)).changed();
                                value = int_val as f32;
                                changed
                            }
                            ParamType::Angle => {
                                let min = param.min_value.unwrap_or(0.0);
                                let max = param.max_value.unwrap_or(360.0);
                                ui.add(egui::Slider::new(&mut value, min..=max)
                                    .text(&param.display_name)
                                    .suffix("°")).changed()
                            }
                        };

                        if param_changed {
                            transform.set_variation_param(var_name, &param.name, value);
                            changed = true;
                        }
                    }
                });
            }
        }
    }

    if changed {
        transform.set_variation(var_name, weight);
        *flame_changed = true;
    }
}
```

### B. Visual Layout Example

```
┌─ Transforms ────────────────────────────┐
│ Transform 0                             │
│   Affine                                │
│     a: 0.80  b: 0.00  c: 0.00           │
│     d: 0.80  e: 0.10  f: 0.00           │
│   Weight: 1.00                          │
│   Variations                            │
│     ☑ Linear      [========|    ] 0.50  │
│     ☑ Julian      [============|  ] 0.80│
│         Power:  [===|           ] 3     │
│         Dist:   [====|          ] 1.00  │
│     ☐ Sinusoidal                        │
│     ☐ Spherical                         │
│   Color                                 │
│     RGB: [1.0, 0.5, 0.2]                │
│     Speed: 0.5                          │
└─────────────────────────────────────────┘
```

---

## 5. Serialization (Config Files)

### JSON Format

```json
{
  "flame": {
    "name": "Example with Parameters",
    "transforms": [
      {
        "a": 0.8, "b": 0.0, "c": 0.0,
        "d": 0.8, "e": 0.1, "f": 0.0, "g": 0.0,
        "weight": 1.0,
        "variations": {
          "linear": 0.5,
          "julian": 0.8
        },
        "variation_params": {
          "julian.power": 3.0,
          "julian.dist": 1.0
        },
        "color": [1.0, 0.5, 0.2],
        "color_speed": 0.5
      }
    ]
  }
}
```

### Backward Compatibility

```rust
// Deserialize with default for missing field
#[derive(Serialize, Deserialize)]
pub struct Transform {
    // ... existing fields ...

    #[serde(default)]
    pub variation_params: HashMap<String, f32>,  // Defaults to empty if missing
}
```

Old configs without `variation_params` will use default values from registry.

---

## 6. Implementation Phases

### **Phase 1: Data Model** ✅ Foundation
**Files:** `src/variations/mod.rs`, `src/scene/transforms.rs`

1. Add `VariationParameter` struct
2. Add `ParamType` enum
3. Add `parameters: Vec<VariationParameter>` to `VariationInfo`
4. Add `variation_params: HashMap<String, f32>` to `Transform`
5. Implement helper methods:
   - `Transform::set_variation_param()`
   - `Transform::get_variation_param()`
   - `Transform::get_variation_param_or_default()`
   - `VariationInfo::get_param_default()`
6. Update `Transform::default()` to initialize empty HashMap
7. Add `#[serde(default)]` to `variation_params` field
8. Write unit tests for parameter get/set

**Deliverable:** Transform can store and retrieve variation parameters

---

### **Phase 2: GPU Pipeline** 🔧 Core
**Files:** `src/gpu/buffers.rs`, `src/renderer/compute_kernel.rs`

1. Add `MAX_PARAMS_PER_VARIATION` constant (= 8)
2. Create `GpuVariationParams` struct (192 floats)
3. Implement `GpuVariationParams::from_transform()`
4. Add `variation_params_buffer` to `FlameBuffers`
5. Update `FlameBuffers::new()` to create parameter buffer
6. Update `FlameRenderer::update_flame()` to write parameter buffer
7. Add buffer binding to pipeline layout
8. Test buffer roundtrip (CPU → GPU)

**Deliverable:** Parameters are uploaded to GPU

---

### **Phase 3: Shader Updates** 🎨 GPU
**Files:** `shaders/core/header.wgsl`, `shaders/core/utilities.wgsl`, `src/shader_builder_v2.rs`

1. Add `VariationParams` struct to WGSL
2. Add `@binding(3) variation_params` storage buffer
3. Implement `get_variation_param()` helper function
4. Update `ShaderBuilder` to generate variation ID constants
5. Update one test variation (julian) to use parameters:
   - Update function signature to accept `xform_id`
   - Replace hardcoded values with parameter reads
6. Update `apply_variations()` to pass `xform_id` to variation functions
7. Verify shader compilation
8. Test parameter changes visually

**Deliverable:** At least one variation (julian) works with parameters

---

### **Phase 4: UI** 🖼️ User-facing
**Files:** `src/ui/transforms.rs`

1. Update variation rendering to show parameter sliders
2. Add indented section below variation weight
3. Implement slider widgets for each param type:
   - Float: continuous slider
   - Integer: discrete slider
   - Angle: 0-360° slider with suffix
4. Connect parameter changes to `flame_changed` flag
5. Test real-time parameter updates
6. Add parameter reset button (restore defaults)

**Deliverable:** Users can adjust variation parameters via UI

---

### **Phase 5: Registry Population** 📚 Content
**Files:** `src/variations/mod.rs`

1. Define parameters for julian:
   - power (Integer, default: 2, range: 1-10)
   - dist (Float, default: 1.0, range: 0.1-5.0)
2. Define parameters for waves:
   - scalex, scaley, freqx, freqy
3. Define parameters for curl:
   - c1, c2
4. Update variation registration to include parameters
5. Test each variation with different parameter values

**Deliverable:** Common variations have usable parameters

---

### **Phase 6: Full Variation Updates** 🔄 Polish
**Files:** `shaders/core/variations_2d.wgsl`, `shaders/core/variations_3d.wgsl`

1. Update all variation function signatures to accept `xform_id`
2. Replace hardcoded constants with parameter reads
3. Update shader builder to handle parameterless variations (pass dummy xform_id)
4. Test all variations still work
5. Update documentation

**Deliverable:** All variations support parameter system

---

### **Phase 7: Plugin System** 🔌 Extensibility
**Files:** `src/variations/plugin.rs` (new), plugin examples

1. Define plugin manifest format (JSON/TOML):
   ```json
   {
     "name": "my_variation",
     "display_name": "My Variation",
     "parameters": [
       {"name": "strength", "type": "Float", "default": 1.0, "min": 0.0, "max": 5.0}
     ],
     "wgsl": "fn variation_my_variation(p: vec2<f32>, xform_id: u32) -> vec2<f32> { ... }"
   }
   ```
2. Implement plugin loader
3. Parse parameter definitions
4. Register plugin variations with parameters
5. Test external plugin with parameters

**Deliverable:** Plugins can define parameters

---

## 7. Example Variations with Parameters

| Variation | Parameter | Type | Default | Range | Description |
|-----------|-----------|------|---------|-------|-------------|
| **julian** | power | Integer | 2 | 1-10 | Number of petals/symmetry |
|  | dist | Float | 1.0 | 0.1-5.0 | Distance scaling factor |
| **waves** | scalex | Float | 1.0 | 0.1-5.0 | X-axis amplitude |
|  | scaley | Float | 1.0 | 0.1-5.0 | Y-axis amplitude |
|  | freqx | Float | 1.0 | 0.1-10.0 | X-axis frequency |
|  | freqy | Float | 1.0 | 0.1-10.0 | Y-axis frequency |
| **curl** | c1 | Float | 1.0 | -5.0-5.0 | First curl parameter |
|  | c2 | Float | 0.0 | -5.0-5.0 | Second curl parameter |
| **rectangles** | x | Float | 1.0 | 0.1-5.0 | X-dimension |
|  | y | Float | 1.0 | 0.1-5.0 | Y-dimension |
| **ngon** | sides | Integer | 5 | 3-20 | Number of sides |
|  | power | Float | 3.0 | 0.1-10.0 | Exponent |
|  | corners | Float | 2.0 | 0.0-10.0 | Corner sharpness |
|  | circle | Float | 1.0 | 0.0-5.0 | Circularity |

---

## 8. Technical Considerations

### Memory Layout

**GPU Buffer Size:**
```
24 variations × 8 params × 4 bytes = 768 bytes per transform
768 bytes × 32 transforms = 24,576 bytes (24 KB) total
```

**Pros:**
- Simple, predictable indexing
- Fast GPU access (coalesced reads)
- No branching or indirection
- Easy to debug

**Cons:**
- Wasted space for unused parameters
- Fixed maximum of 8 params per variation (can increase if needed)

**Verdict:** Acceptable tradeoff. Modern GPUs handle this easily.

---

### Performance

**Storage Buffer Reads:**
- Cached by GPU
- Parameters are uniform across workgroup (good for cache)
- No runtime overhead compared to hardcoded values

**Shader Compilation:**
- Generated constants eliminate string lookups
- Direct indexing into parameter array

---

### Alternatives Considered

**1. Uniform Buffer (instead of Storage Buffer)**
- ❌ Too small (64 KB limit)
- ❌ Doesn't fit 32 transforms × 768 bytes

**2. Dynamic Parameter Array (only active params)**
- ✅ Saves memory
- ❌ Complex indexing logic
- ❌ More CPU work to pack parameters
- **Verdict:** Not worth the complexity

**3. Separate Buffer per Variation**
- ❌ Too many buffers (24+)
- ❌ Binding slot limits
- ❌ Poor cache locality

---

## 9. Testing Strategy

### Unit Tests
```rust
#[test]
fn test_variation_params() {
    let mut transform = Transform::new();

    // Set parameter
    transform.set_variation_param("julian", "power", 5.0);

    // Get parameter
    assert_eq!(transform.get_variation_param("julian", "power"), Some(5.0));

    // Non-existent parameter
    assert_eq!(transform.get_variation_param("julian", "nonexistent"), None);
}

#[test]
fn test_gpu_params_roundtrip() {
    let mut transform = Transform::new();
    transform.set_variation("julian", 0.8);
    transform.set_variation_param("julian", "power", 3.0);
    transform.set_variation_param("julian", "dist", 1.5);

    let registry = VariationRegistry::new();
    let gpu_params = GpuVariationParams::from_transform(&transform, &registry);

    // Verify julian params are in correct slots
    let julian_id = registry.get_id("julian").unwrap();
    assert_eq!(gpu_params.params[julian_id * 8 + 0], 3.0);  // power
    assert_eq!(gpu_params.params[julian_id * 8 + 1], 1.5);  // dist
}
```

### Visual Tests
1. Load julian with power=2 → verify 2-fold symmetry
2. Change power to 5 → verify 5-fold symmetry
3. Adjust dist parameter → verify scaling changes
4. Test all parameter types (float, int, angle)

### Serialization Tests
```rust
#[test]
fn test_param_serialization() {
    let mut transform = Transform::new();
    transform.set_variation("julian", 0.8);
    transform.set_variation_param("julian", "power", 5.0);

    let json = serde_json::to_string(&transform).unwrap();
    let loaded: Transform = serde_json::from_str(&json).unwrap();

    assert_eq!(loaded.get_variation_param("julian", "power"), Some(5.0));
}
```

---

## 10. Migration Path

### Existing Configs
```rust
// Old config (no variation_params field)
{
  "variations": {"julian": 0.8}
}

// Auto-migrates to:
{
  "variations": {"julian": 0.8},
  "variation_params": {}  // Empty, uses defaults
}
```

### Defaults Applied
When `variation_params` is empty/missing, use registry defaults:
```rust
impl Transform {
    pub fn get_variation_param_or_default(&self, var: &str, param: &str, registry: &VariationRegistry) -> f32 {
        self.get_variation_param(var, param)
            .or_else(|| registry.get(var)?.get_param_default(param))
            .unwrap_or(0.0)
    }
}
```

---

## 11. Future Extensions

### A. Parameter Expressions
Allow parameters to reference each other or transform properties:
```json
"variation_params": {
  "julian.power": {"expr": "floor(transform.a * 5)"}
}
```

### B. Animation Keyframes
```json
"variation_params": {
  "julian.power": {
    "keyframes": [
      {"time": 0.0, "value": 2.0},
      {"time": 1.0, "value": 10.0}
    ]
  }
}
```

### C. Parameter Presets
```json
"julian_presets": {
  "flower": {"power": 5, "dist": 1.0},
  "star": {"power": 7, "dist": 0.5}
}
```

---

## 12. Documentation Needs

1. **User Guide:** How to use variation parameters in UI
2. **Developer Guide:** How to add parameters to variations
3. **Plugin Guide:** Parameter definition in plugin manifests
4. **WGSL Reference:** How to access parameters in shaders
5. **Migration Guide:** Updating old configs

---

## Summary

**Recommended Implementation Order:**
1. Phase 1 (Data Model) - Foundation
2. Phase 2 (GPU Pipeline) - Core functionality
3. Phase 3 (Shaders) - Test with julian
4. Phase 4 (UI) - User-facing controls
5. Phase 5 (Registry) - Populate common variations
6. Phase 6 (Full Updates) - All variations
7. Phase 7 (Plugins) - Extensibility

**Estimated Effort:**
- Phase 1-4: ~2-3 days (core system)
- Phase 5-6: ~1-2 days (content)
- Phase 7: ~1 day (plugins)
- **Total: ~4-6 days**

**Critical Path:**
Data Model → GPU Pipeline → Shader Updates → UI

**First Milestone:**
Get julian variation working with power/dist parameters end-to-end (Phases 1-3).
