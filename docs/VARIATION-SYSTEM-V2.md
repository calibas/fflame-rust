## Name-Based Variation System (V2 Design)

### Overview

The variation system uses **string names** instead of fixed array indices, allowing:
- Dynamic plugin loading without ID conflicts
- Human-readable variation references
- Flexible variation ordering
- Easy serialization/deserialization

### Architecture

```
┌─────────────────────────────────────┐
│ VariationRegistry                   │
│ - Maps names → metadata             │
│ - Assigns runtime IDs               │
│ - Loads plugins dynamically         │
└─────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────┐
│ Transform                           │
│ variations: HashMap<String, f32>    │
│ {                                   │
│   "linear": 0.5,                    │
│   "swirl": 0.3,                     │
│   "curl_3d": 0.2                    │
│ }                                   │
└─────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────┐
│ ShaderBuilder                       │
│ 1. Extract active names             │
│ 2. Assign runtime IDs: 0, 1, 2...  │
│ 3. Generate shader with mapping     │
└─────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────┐
│ GPU Shader                          │
│ variations[0] = linear weight       │
│ variations[1] = swirl weight        │
│ variations[2] = curl_3d weight      │
└─────────────────────────────────────┘
```

### Core Components

#### 1. VariationRegistry (`src/variations/mod.rs`)

**Purpose**: Central registry of all variations (core + plugins)

```rust
pub struct VariationRegistry {
    variations: HashMap<String, VariationInfo>,
    ordered_names: Vec<String>,
}

pub struct VariationInfo {
    name: String,              // "curl_3d"
    display_name: String,      // "Curl 3D"
    category: VariationCategory,
    wgsl_function: String,     // "variation_curl_3d"
    needs_rng: bool,
    is_core: bool,
    wgsl_source: Option<String>, // For plugins
}
```

**Usage**:
```rust
let mut registry = VariationRegistry::new(); // Auto-registers core variations

// Register plugin
registry.register_plugin(
    "curl_3d".to_string(),
    "Curl 3D".to_string(),
    VariationCategory::Plugin,
    include_str!("curl_3d.wgsl").to_string(),
    false // doesn't need RNG
);

// Get info
let info = registry.get("curl_3d").unwrap();
println!("{}", info.display_name); // "Curl 3D"
```

#### 2. Transform with Named Variations

**Old (Array-based)**:
```rust
pub struct Transform {
    variations: [f32; 24], // Fixed size, index-dependent
}
```

**New (Name-based)**:
```rust
pub struct Transform {
    variations: HashMap<String, f32>, // Dynamic, name-based
}

// Usage
let mut xform = Transform::new();
xform.set_variation("linear", 0.5);
xform.set_variation("swirl", 0.3);
xform.set_variation("curl_3d", 0.2); // Plugin!
```

#### 3. Runtime ID Assignment

The registry assigns IDs dynamically based on which variations are actually used:

```rust
// Flame has these active variations:
let active = vec!["linear", "swirl", "curl_3d"];

// Registry assigns IDs:
let id_map = registry.assign_ids(&active);
// {
//   "linear": 0,
//   "swirl": 1,
//   "curl_3d": 2,
// }
```

**GPU Buffer** (written to shader):
```
variations[0] = 0.5  // linear
variations[1] = 0.3  // swirl
variations[2] = 0.2  // curl_3d
```

**Key Point**: The ID only matters during shader execution. Next frame, if the active set changes, IDs are reassigned and shader recompiled.

### Shader Generation

#### Generated Shader (example):

```wgsl
// Core variations (always included)
fn variation_linear(p: vec3<f32>) -> vec3<f32> { return p; }
fn variation_swirl(p: vec3<f32>) -> vec3<f32> { /* ... */ }

// Plugin variation (injected if active)
fn variation_curl_3d(p: vec3<f32>) -> vec3<f32> {
    let c1 = 1.0;
    let c2 = 2.0;
    // ... curl math
}

// Generated dispatch function (based on active variations)
fn apply_variations(xform: Transform, p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    var result = vec3<f32>(0.0, 0.0, 0.0);

    // 0: Linear
    if (xform.variations[0] != 0.0) {
        result += xform.variations[0] * variation_linear(p);
    }

    // 1: Swirl
    if (xform.variations[1] != 0.0) {
        result += xform.variations[1] * variation_swirl(p);
    }

    // 2: Curl 3D
    if (xform.variations[2] != 0.0) {
        result += xform.variations[2] * variation_curl_3d(p);
    }

    return result;
}
```

### Plugin System

#### Creating a Plugin Variation

1. **Write WGSL code** (`shaders/plugins/curl_3d.wgsl`):
```wgsl
fn variation_curl_3d(p: vec3<f32>) -> vec3<f32> {
    // Your variation math
    let result = /* ... */;
    return result;
}
```

2. **Register plugin**:
```rust
// At startup or dynamically
let wgsl_source = std::fs::read_to_string("shaders/plugins/curl_3d.wgsl")?;

registry.register_plugin(
    "curl_3d".to_string(),
    "Curl 3D".to_string(),
    VariationCategory::Plugin,
    wgsl_source,
    false // needs_rng
);
```

3. **Use in flame**:
```rust
transform.set_variation("curl_3d", 0.5);
```

4. **Shader auto-recompiles** with curl_3d injected!

### Migration Strategy

#### Phase 1: Add Named System (Alongside Array System)

```rust
pub struct Transform {
    // Old (kept for compatibility)
    variations_array: [f32; 24],

    // New
    variations: HashMap<String, f32>,
}

impl Transform {
    // Convert array → map (for loading old files)
    fn from_array(arr: [f32; 24], registry: &VariationRegistry) -> HashMap<String, f32> {
        let mut map = HashMap::new();
        for (i, &weight) in arr.iter().enumerate() {
            if weight.abs() > 1e-6 {
                if let Some(name) = registry.names().get(i) {
                    map.insert(name.clone(), weight);
                }
            }
        }
        map
    }

    // Convert map → array (for GPU upload during transition)
    fn to_array(&self, id_map: &HashMap<String, u32>, max_size: usize) -> Vec<f32> {
        let mut arr = vec![0.0; max_size];
        for (name, &weight) in &self.variations {
            if let Some(&id) = id_map.get(name) {
                arr[id as usize] = weight;
            }
        }
        arr
    }
}
```

#### Phase 2: Update GPU Buffers

**Old**: Fixed-size array
```rust
#[repr(C)]
struct GpuTransform {
    // ...
    variations: [f32; 24], // Fixed size
}
```

**New**: Dynamic-size array
```rust
#[repr(C)]
struct GpuTransform {
    // ...
    // Variations array size = number of active variations
}

// Buffer written with only active variations
let active_count = flame.count_active_variations();
let buffer_size = std::mem::size_of::<GpuTransformHeader>()
                  + active_count * std::mem::size_of::<f32>();
```

#### Phase 3: Update Serialization

**JSON Format** (backward compatible):
```json
{
  "transform": {
    "a": 0.7,
    "d": 0.7,
    "variations": {
      "linear": 0.5,
      "swirl": 0.3,
      "curl_3d": 0.2
    }
  }
}
```

**Old format** (still supported):
```json
{
  "transform": {
    "a": 0.7,
    "d": 0.7,
    "variations": [0.5, 0.0, 0.0, 0.3, /* ... 20 more zeros */]
  }
}
```

Custom deserializer handles both!

### Benefits

✅ **No ID Conflicts**: Plugins don't need to coordinate indices
✅ **Human Readable**: `"curl_3d"` vs `24`
✅ **Flexible Loading**: Load plugins in any order
✅ **Smaller GPU Buffers**: Only send active variations
✅ **Better Serialization**: JSON shows what variations are actually used
✅ **Easier Debugging**: Log "linear=0.5, swirl=0.3" instead of "variations[0]=0.5, variations[3]=0.3"

### Performance

- **Runtime ID assignment**: O(n) where n = active variations (typically 2-5)
- **Shader compilation**: Same as before (~100-500ms)
- **GPU execution**: Identical (uses integer indices in shader)
- **Memory**: HashMap overhead ~24 bytes/variation (negligible)

### Example: Full Workflow

```rust
// 1. Setup registry
let mut registry = VariationRegistry::new(); // Core variations auto-registered

// 2. Load plugins
for file in glob("shaders/plugins/*.wgsl")? {
    let name = extract_name(&file); // "curl_3d" from "curl_3d.wgsl"
    let source = std::fs::read_to_string(file)?;
    registry.register_plugin(name, /* ... */);
}

// 3. Create flame with named variations
let mut xform = Transform::new();
xform.set_variation("linear", 0.5);
xform.set_variation("swirl", 0.3);
xform.set_variation("curl_3d", 0.2); // Plugin!

let flame = Flame::new(vec![xform]);

// 4. Build shader
let active = flame.extract_active_variations(); // {"linear": 0.5, "swirl": 0.3, "curl_3d": 0.2}
let id_map = registry.assign_ids(&active.keys().collect()); // {"linear": 0, "swirl": 1, "curl_3d": 2}

let shader_builder = ShaderBuilder::new(registry);
let wgsl = shader_builder.build_trajectory_3d(&active);

// Shader now has:
// - variation_linear, variation_swirl, variation_curl_3d functions
// - apply_variations dispatching to IDs 0, 1, 2

// 5. Upload to GPU
let gpu_variations = xform.to_gpu_array(&id_map); // [0.5, 0.3, 0.2]
queue.write_buffer(&variations_buffer, &gpu_variations);

// Done! Shader uses runtime IDs internally, names externally.
```

### UI Integration

```rust
// Variation selector
let registry = app.variation_registry();
for info in registry.by_category(VariationCategory::Basic2D) {
    if ui.button(&info.display_name).clicked() {
        transform.set_variation(&info.name, 0.5);
    }
}

// Show active variations
for (name, weight) in &transform.variations {
    if let Some(info) = registry.get(name) {
        ui.horizontal(|ui| {
            ui.label(&info.display_name);
            ui.add(egui::Slider::new(weight, 0.0..=2.0));
        });
    }
}
```

### Future: Hot-Reload Plugins

```rust
// Watch plugins directory
let watcher = notify::watcher(/* ... */);
watcher.watch("shaders/plugins", RecursiveMode::NonRecursive)?;

// On file change
for event in watcher {
    if event.path.extension() == Some("wgsl") {
        let name = extract_name(&event.path);
        let source = std::fs::read_to_string(&event.path)?;

        // Update registry
        registry.register_plugin(name, /* ... */);

        // Trigger shader recompile if this variation is active
        if flame.uses_variation(&name) {
            shader_cache.invalidate();
        }
    }
}
```

### Migration Timeline

1. **Phase 1** (Week 1): Implement name-based system alongside array system
2. **Phase 2** (Week 2): Update Transform to use HashMap internally, keep array interface
3. **Phase 3** (Week 3): Update serialization to support both formats
4. **Phase 4** (Week 4): Update UI to use variation names
5. **Phase 5** (Week 5): Remove array-based code (breaking change, bump version)

### Backward Compatibility

**During Migration**:
- Old `.flame` files with arrays → auto-converted to HashMap
- New `.flame` files save as HashMap
- Export option: "Save as legacy format (array)" for compatibility

**Post-Migration**:
- Only HashMap format supported
- Old files still loadable (via conversion on load)

This is a much better design! Want me to implement the full migration?
