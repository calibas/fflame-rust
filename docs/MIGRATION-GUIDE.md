# Migration Guide: Name-Based Variation System

## Current Status

✅ **Implemented**:
- `src/variations/mod.rs` - Variation registry system
- `src/shader_builder_v2.rs` - Name-based shader builder
- `src/scene/transforms_v2.rs` - Transform with HashMap variations
- Full backward compatibility with array format

⏳ **Ready to Migrate** (requires code changes):
- Replace `transforms.rs` with `transforms_v2.rs`
- Update all Transform usage throughout codebase
- Update GPU buffer writing
- Update UI variation controls
- Update shader_cache to use new builder

## Migration Steps

### Step 1: Switch Transform Implementation

**File**: `src/scene/transforms.rs`

Replace the current Transform struct with the V2 version, or rename:
```bash
mv src/scene/transforms.rs src/scene/transforms_legacy.rs
mv src/scene/transforms_v2.rs src/scene/transforms.rs
```

This is safe because transforms_v2 has backward-compatible deserialization.

### Step 2: Update Flame Struct

**File**: `src/scene/transforms.rs` (find the Flame struct)

Add variation registry:
```rust
pub struct Flame {
    pub transforms: Vec<Transform>,
    pub render_mode: RenderMode,
    pub projection: ProjectionType,

    // Add this:
    #[serde(skip)]
    pub variation_registry: VariationRegistry,
}

impl Default for Flame {
    fn default() -> Self {
        Self {
            transforms: vec![Transform::new()],
            render_mode: RenderMode::TwoD,
            projection: ProjectionType::Orthographic,
            variation_registry: VariationRegistry::new(), // Add this
        }
    }
}

impl Flame {
    /// Extract all active variation names from all transforms
    pub fn extract_active_variations(&self) -> HashMap<String, f32> {
        let mut all_variations = HashMap::new();

        for transform in &self.transforms {
            for (name, weight) in &transform.variations {
                // Track if this variation is used anywhere
                if weight.abs() > 1e-6 {
                    all_variations.insert(name.clone(), *weight);
                }
            }
        }

        all_variations
    }

    /// Get runtime ID mapping for active variations
    pub fn get_id_mapping(&self) -> HashMap<String, u32> {
        let active: Vec<String> = self.extract_active_variations().keys().cloned().collect();
        self.variation_registry.assign_ids(&active)
    }
}
```

### Step 3: Update GPU Buffer Writing

**File**: `src/gpu/buffers.rs`

Update `GpuTransform` to use dynamic array:
```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuTransform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
    pub g: f32,
    pub weight: f32,
    // Variations will be written separately
    pub color: [f32; 3],
    pub color_speed: f32,
    pub _pad: [f32; 2], // Align to 16 bytes
}

// Write transforms to GPU
pub fn write_transforms(queue: &Queue, buffer: &Buffer, flame: &Flame) {
    let id_map = flame.get_id_mapping();
    let max_variations = id_map.len().max(24); // At least 24 for compatibility

    for (i, transform) in flame.transforms.iter().enumerate() {
        // Write affine + metadata
        let gpu_xform = GpuTransform {
            a: transform.a,
            b: transform.b,
            c: transform.c,
            d: transform.d,
            e: transform.e,
            f: transform.f,
            g: transform.g,
            weight: transform.weight,
            color: transform.color,
            color_speed: transform.color_speed,
            _pad: [0.0; 2],
        };

        let offset = i * (std::mem::size_of::<GpuTransform>() + max_variations * 4);
        queue.write_buffer(buffer, offset as u64, bytemuck::bytes_of(&gpu_xform));

        // Write variations array
        let variations_array = transform.to_gpu_array(&id_map, max_variations);
        let var_offset = offset + std::mem::size_of::<GpuTransform>();
        queue.write_buffer(buffer, var_offset as u64, bytemuck::cast_slice(&variations_array));
    }
}
```

### Step 4: Update Shader Header

**File**: `shaders/core/header.wgsl`

Make variations array size dynamic (use storage buffer):
```wgsl
// OLD: Fixed-size array
struct Transform {
    // ...
    variations: array<f32, 24>,
    // ...
}

// NEW: Use @stride for dynamic sizing
struct Transform {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
    weight: f32,
    // Variations array follows (size determined by shader generation)
}

// Store variations separately
@group(0) @binding(5) var<storage, read> variations: array<array<f32>>;

// Access: variations[transform_id][variation_id]
```

**OR** keep fixed size but use MAX_VARIATIONS from active set:
```wgsl
// Generate at runtime
const MAX_VARIATIONS: u32 = {{MAX_VARIATIONS}}; // Replaced by shader builder

struct Transform {
    // ...
    variations: array<f32, MAX_VARIATIONS>,
    // ...
}
```

### Step 5: Update ShaderBuilder

**File**: Replace `src/shader_builder.rs` with updated version

```rust
// In shader_builder_v2.rs, add template replacement
pub fn build_trajectory_3d(&self, active_variations: &HashMap<String, f32>) -> String {
    let id_map = self.registry.assign_ids(&active_variations.keys().cloned().collect());
    let max_variations = id_map.len().max(24);

    let mut shader = String::new();

    // 1. Header with MAX_VARIATIONS substitution
    let header = include_str!("../shaders/core/header.wgsl");
    let header = header.replace("{{MAX_VARIATIONS}}", &max_variations.to_string());
    shader.push_str(&header);

    // ... rest of shader generation
}
```

### Step 6: Update ShaderCache

**File**: `src/shader_cache.rs`

Update to use named variations:
```rust
pub fn ensure_current(
    &mut self,
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    flame: &Flame
) -> bool {
    let needed = flame.extract_active_variations();

    // Compare by keys (variation names)
    let needed_names: std::collections::HashSet<String> = needed.keys().cloned().collect();
    let current_names: std::collections::HashSet<String> = self.active_variations.keys().cloned().collect();

    if needed_names == current_names {
        return false; // No rebuild needed
    }

    log::info!("Recompiling shaders: variations changed from {:?} to {:?}", current_names, needed_names);

    // Rebuild...
    let builder = ShaderBuilder::new(flame.variation_registry.clone());
    self.shader_source_2d = builder.build_trajectory_2d(&needed);
    self.shader_source_3d = builder.build_trajectory_3d(&needed);

    // ... recreate pipelines
    true
}
```

### Step 7: Update UI

**File**: `src/ui/mod.rs`

Replace variation array UI with named variation UI:
```rust
fn render_variations_ui(ui: &mut egui::Ui, transform: &mut Transform, registry: &VariationRegistry) -> bool {
    let mut changed = false;

    // Group by category
    for category in [VariationCategory::Basic2D, VariationCategory::Advanced2D, /* ... */] {
        ui.collapsing(format!("{:?} Variations", category), |ui| {
            for info in registry.by_category(category) {
                let mut weight = transform.get_variation(&info.name);

                ui.horizontal(|ui| {
                    ui.label(&info.display_name);
                    if ui.add(egui::Slider::new(&mut weight, 0.0..=2.0)).changed() {
                        transform.set_variation(&info.name, weight);
                        changed = true;
                    }

                    if weight.abs() > 1e-6 && ui.button("Clear").clicked() {
                        transform.set_variation(&info.name, 0.0);
                        changed = true;
                    }
                });
            }
        });
    }

    changed
}

// In main UI render function:
if render_variations_ui(ui, transform, &app.flame.variation_registry) {
    app.reset_accumulation();
}
```

### Step 8: Update Presets

**File**: `src/scene/presets.rs`

Update preset functions to use named variations:
```rust
pub fn sierpinski() -> Flame {
    let mut xform1 = Transform::new();
    xform1.a = 0.5; xform1.d = 0.5;
    xform1.set_variation("linear", 1.0); // Named!
    xform1.color = [1.0, 0.0, 0.0];

    let mut xform2 = Transform::new();
    xform2.a = 0.5; xform2.d = 0.5; xform2.e = 0.5;
    xform2.set_variation("linear", 1.0);
    xform2.color = [0.0, 1.0, 0.0];

    Flame {
        transforms: vec![xform1, xform2],
        render_mode: RenderMode::TwoD,
        projection: ProjectionType::Orthographic,
        variation_registry: VariationRegistry::new(),
    }
}
```

### Step 9: Run Tests

```bash
# Run Transform V2 tests
cargo test transforms_v2

# Run full test suite
cargo test

# Test serialization compatibility
cargo test --test regression
```

### Step 10: Update Documentation

Update these files:
- `docs/STATUS.md` - Mark variation system as name-based
- `docs/ARCHITECTURE.md` - Update variation system description
- `CLAUDE.md` - Update variation usage examples

## Testing Backward Compatibility

Create test cases:
```rust
#[test]
fn test_load_old_flame_file() {
    // Old format with array
    let json = r#"{
        "transforms": [{
            "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "e": 0.0, "f": 0.0, "g": 0.0,
            "weight": 1.0,
            "variations": [0.5, 0.0, 0.0, 0.3, 0.0, /* ... */],
            "color": [1.0, 1.0, 1.0],
            "color_speed": 0.5
        }],
        "render_mode": "TwoD"
    }"#;

    let flame: Flame = serde_json::from_str(json).unwrap();
    assert_eq!(flame.transforms[0].get_variation("linear"), 0.5);
    assert_eq!(flame.transforms[0].get_variation("swirl"), 0.3);
}

#[test]
fn test_save_new_format() {
    let mut flame = Flame::default();
    flame.transforms[0].set_variation("linear", 0.5);
    flame.transforms[0].set_variation("swirl", 0.3);

    let json = serde_json::to_string_pretty(&flame).unwrap();
    assert!(json.contains(r#""variations":{"linear":0.5,"swirl":0.3}"#) ||
            json.contains(r#""variations":{"swirl":0.3,"linear":0.5}"#));
}
```

## Rollback Plan

If issues arise, rollback is simple:
```bash
git checkout src/scene/transforms.rs  # Restore old version
git checkout src/shader_builder.rs    # Restore old version
git checkout src/shader_cache.rs      # Restore old version
cargo build
```

All .flame files remain compatible because old code reads arrays, new code reads both.

## Estimated Time

- **Step 1-3**: 30 minutes (Transform replacement)
- **Step 4-5**: 1 hour (GPU buffer + shader updates)
- **Step 6-7**: 1 hour (Cache + UI updates)
- **Step 8**: 30 minutes (Presets)
- **Step 9-10**: 1 hour (Testing + docs)

**Total**: ~4 hours for complete migration

## Benefits After Migration

✅ Plugin variations with no ID conflicts
✅ Human-readable JSON configs
✅ Smaller GPU buffers
✅ Easier debugging
✅ Hot-reload support (future)
✅ 100+ variation support

## Questions?

Review `docs/VARIATION-SYSTEM-V2.md` for full design details.

The implementation is ready - just need to execute the migration steps!
