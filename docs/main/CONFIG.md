# Configuration and State Management

Complete guide to the configuration system, state management, undo/redo, and serialization.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overview and module organization
- [TRANSFORMS.md](TRANSFORMS.md) - Transform structure details
- [UI.md](UI.md) - Config import/export UI controls
- [EXPORT.md](EXPORT.md) - PNG metadata embedding

---

## FractalConfig Structure

The `FractalConfig` struct represents **complete application state** for exact reproducibility. Everything needed to recreate a fractal is stored in this single struct.

**Location:** [src/config.rs](../../src/config.rs)

### Core Fields

```rust
pub struct FractalConfig {
    // 1. Flame Definition
    pub flame: Flame,                    // Transform collection, render mode, projection

    // 2. View State
    pub zoom: f32,                       // Zoom level (1.0 = default)
    pub pan_x: f32,                      // Horizontal pan (0.0 = centered)
    pub pan_y: f32,                      // Vertical pan (0.0 = centered)
    pub rotation: f32,                   // View rotation in radians (0.0 = no rotation)
    pub camera_pitch: f32,               // 3D camera pitch (vertical orbit, 0.0 = level)
    pub camera_yaw: f32,                 // 3D camera yaw (horizontal orbit, 0.0 = front)

    // 3. Rendering Settings
    pub density_scale: f32,              // Density multiplier (default: 50.0)
    pub speed_factor: f32,               // Speed multiplier (default: 1.0)
    pub max_iterations: u64,             // Target iteration count (default: 1 billion)

    // 4. Color Settings
    pub color_mode: ColorMode,           // Transform/Palette/Speed
    pub palette_index: usize,            // Library index for palette
    pub background_color: [f32; 3],      // RGB background (default: black)
    pub palette: Option<Palette>,        // Embedded palette data (added 2025-10-24)

    // 5. Tone Mapping
    pub tonemap_mode: TonemapMode,       // Logarithmic/Linear
    pub tonemap_curve: f32,              // S-curve strength (0.0-1.0)
    pub use_curve: bool,                 // Enable tone curve (default: true)
    pub exposure: f32,                   // Exposure adjustment (default: 1.0)
    pub gamma: f32,                      // Gamma correction (default: 2.2)

    // 6. Reproducibility
    pub deterministic_rng: bool,         // Enable fixed RNG seeds (default: false)
}
```

### Added for Full Reproducibility (2025-10-24)

Four critical fields ensure **exact reproduction** across sessions:

1. **`palette: Option<Palette>`** - Embeds actual palette data
   - Problem: Palette library indices could change if palettes are added/removed
   - Solution: Store the actual palette ColorStop array
   - Backward compatible: Old configs without this field use `palette_index`

2. **`use_curve: bool`** - Whether to apply tone curve
   - Problem: Tone curve application was implicit (always on)
   - Solution: Explicit flag allows exact reproduction
   - Default: true (matches previous behavior)

3. **`max_iterations: u64`** - Exact iteration count target
   - Problem: Interactive mode renders forever, testing needs fixed counts
   - Solution: Store target iteration count for reproducibility
   - Default: 1 billion (reasonable test target)

4. **`deterministic_rng: bool`** - Enable reproducible RNG
   - Problem: GPU RNG uses global timestamp → different results each run
   - Solution: Fixed seed mode for exact reproduction
   - Default: false (random behavior for interactive use)

### Flame Field

The `flame` field contains the core fractal definition:

```rust
pub struct Flame {
    pub name: String,                    // Flame name
    pub transforms: Vec<Transform>,      // Transform collection
    pub render_mode: RenderMode,         // 2D or 3D
    pub projection: ProjectionType,      // Orthographic or Perspective
}
```

**Transform structure** (see [TRANSFORMS.md](TRANSFORMS.md) for details):
- Affine matrix: `a, b, c, d, e, f` (2D transformation)
- Z offset: `g` (3D depth in 3D mode)
- Variation weights: `variations: [f32; 24]` (16 basic 2D + 8 3D)
- Variation parameters: `variation_params: HashMap<String, HashMap<String, f32>>`
- Color: `color: f32` (0.0-1.0, palette index)
- Speed: `color_speed: f32` (color update rate)
- Weight: `weight: f32` (transform selection probability)

---

## JSON Serialization

Configs use **JSON** format (not RON as originally planned) for human readability and wide tool support.

### Serialization

**Location:** [src/config.rs](../../src/config.rs#L29-L36)

```rust
impl FractalConfig {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
```

### File Format (.fflame)

**Extension:** `.fflame`

**Example structure:**
```json
{
  "flame": {
    "name": "My Flame",
    "transforms": [
      {
        "a": 0.707, "b": -0.707, "c": 0.707, "d": 0.707,
        "e": 0.0, "f": 0.0, "g": 0.0,
        "weight": 0.5,
        "variations": [0.5, 0.5, 0.0, ...],
        "variation_params": {
          "julian": {"power": 3.0, "dist": 1.0}
        },
        "color": 0.3,
        "color_speed": 0.5
      }
    ],
    "render_mode": "TwoD",
    "projection": "Orthographic"
  },
  "zoom": 1.0,
  "pan_x": 0.0,
  "pan_y": 0.0,
  "rotation": 0.0,
  "camera_pitch": 0.0,
  "camera_yaw": 0.0,
  "density_scale": 50.0,
  "speed_factor": 1.0,
  "max_iterations": 1000000000,
  "color_mode": "Palette",
  "palette_index": 2,
  "background_color": [0.0, 0.0, 0.0],
  "palette": {
    "name": "Fire",
    "stops": [
      {"position": 0.0, "color": [0.0, 0.0, 0.0]},
      {"position": 0.5, "color": [1.0, 0.5, 0.0]},
      {"position": 1.0, "color": [1.0, 1.0, 0.0]}
    ]
  },
  "tonemap_mode": "Logarithmic",
  "tonemap_curve": 0.0,
  "use_curve": true,
  "exposure": 1.0,
  "gamma": 2.2,
  "deterministic_rng": false
}
```

### Backward Compatibility

**Transform variations array:**
- Old presets: 16-element array (basic 2D only)
- New presets: 24-element array (16 basic 2D + 8 3D)
- Deserialization: Custom deserializer auto-pads old arrays with zeros

**Implementation:** [src/scene/transforms.rs](../../src/scene/transforms.rs)

```rust
#[serde(deserialize_with = "deserialize_variations_24")]
pub variations: [f32; 24],

fn deserialize_variations_24<'de, D>(deserializer: D) -> Result<[f32; 24], D::Error>
where
    D: Deserializer<'de>,
{
    let v: Vec<f32> = Vec::deserialize(deserializer)?;
    if v.len() == 16 {
        // Old preset: pad with zeros for 3D variations
        let mut arr = [0.0; 24];
        arr[0..16].copy_from_slice(&v);
        Ok(arr)
    } else if v.len() == 24 {
        Ok(v.try_into().unwrap())
    } else {
        Err(de::Error::custom("variations array must be 16 or 24 elements"))
    }
}
```

---

## Preset System

The preset system stores **complete FractalConfig** (not just Flame) to ensure exact recreation of all settings.

**Location:** [src/scene/presets.rs](../../src/scene/presets.rs)

### PresetLibrary Structure

```rust
pub struct PresetLibrary {
    pub presets: Vec<FractalConfig>,     // All presets (built-in + loaded)
}

impl PresetLibrary {
    pub fn new() -> Self {
        let mut lib = PresetLibrary {
            presets: Vec::new(),
        };

        // 1. Load built-in code-based presets
        lib.presets.push(flame_to_config(presets::sierpinski_triangle()));
        lib.presets.push(flame_to_config(presets::julia_set()));
        // ... more built-in presets

        // 2. Load file-based presets (desktop only)
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(configs) = load_configs_from_dir("assets/presets") {
            lib.presets.extend(configs);
        }

        lib
    }
}
```

### Creating Presets

**Option 1: Code-based (built-in)**

Add function in [src/scene/presets.rs](../../src/scene/presets.rs):

```rust
pub fn my_preset() -> Flame {
    let mut flame = Flame::new("My Preset");

    let mut xform = Transform::new();
    xform.a = 0.7; xform.d = 0.7;
    xform.variations[0] = 0.5;  // Linear
    xform.variations[1] = 0.5;  // Sinusoidal
    flame.transforms.push(xform);

    flame
}
```

Add to `PresetLibrary::new()`:
```rust
lib.presets.push(flame_to_config(presets::my_preset()));
```

**Option 2: File-based (auto-loaded)**

1. Create `.fflame` file in `assets/presets/` directory
2. Desktop builds auto-load on startup
3. WASM builds use built-in presets only

**Option 3: Export current state**

1. Use Config Import/Export → Save Config
2. Save as `.fflame` file in `assets/presets/`
3. Restart app to see in preset dropdown (desktop only)

### Helper Function: flame_to_config()

Converts a `Flame` struct to a complete `FractalConfig` with sensible defaults:

```rust
pub fn flame_to_config(flame: Flame) -> FractalConfig {
    FractalConfig {
        flame,
        zoom: 1.0,
        pan_x: 0.0,
        pan_y: 0.0,
        rotation: 0.0,
        camera_pitch: 0.0,
        camera_yaw: 0.0,
        density_scale: 50.0,
        speed_factor: 1.0,
        max_iterations: 1_000_000_000,
        color_mode: ColorMode::Palette,
        palette_index: 2,
        background_color: [0.0, 0.0, 0.0],
        palette: None,
        tonemap_mode: TonemapMode::Logarithmic,
        tonemap_curve: 0.0,
        use_curve: true,
        exposure: 1.0,
        gamma: 2.2,
        deterministic_rng: false,
    }
}
```

### Critical Implementation Details (2025-10-20 Bug Fixes)

**Problem 1: Buffer Overrun**
- Old: Transform buffer sized for current flame only
- Issue: Loading preset with more transforms than current flame caused buffer overrun
- Fix: Pre-allocate for `MAX_TRANSFORMS` (32)

**Problem 2: Residual Transforms**
- Old: Writing N transforms left old data in slots N+1 to 31
- Issue: Ghost transforms from previous presets
- Fix: Zero-pad remaining slots after writing

**Problem 3: Reset Overwrites Params**
- Old: `reset()` cleared both accumulation AND GPU params
- Issue: `load_config()` sets params → `reset()` zeros them → broken render
- Fix: `reset()` only clears accumulation buffers, never touches params

**Implementation:** [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs)

```rust
pub fn load_config(&mut self, config: &FractalConfig) {
    // 1. Update all flame parameters atomically
    self.update_flame(&config.flame, /* zero_padding: */ true);

    // 2. Update view parameters
    self.update_view(config.zoom, config.pan_x, config.pan_y, config.rotation);

    // 3. Update camera (3D mode)
    self.update_camera(config.camera_pitch, config.camera_yaw);

    // 4. Update tone mapping
    self.update_tonemap(config.tonemap_mode, config.tonemap_curve,
                       config.use_curve, config.exposure, config.gamma);

    // 5. Clear accumulation (safe - no params touched)
    self.reset();
}
```

---

## Undo/Redo System

The undo system maintains a **50-state circular buffer** of complete configurations.

**Location:** [src/undo.rs](../../src/undo.rs)

### UndoHistory Structure

```rust
pub struct UndoHistory {
    history: Vec<FractalConfig>,         // Circular buffer (max 50)
    current: usize,                      // Current position
    max_size: usize,                     // Buffer limit (50)
}

impl UndoHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: Vec::new(),
            current: 0,
            max_size,
        }
    }

    pub fn push(&mut self, config: FractalConfig) {
        // Remove future states if we're in the middle
        self.history.truncate(self.current + 1);

        // Add new state
        self.history.push(config);

        // Trim if over limit
        if self.history.len() > self.max_size {
            self.history.remove(0);
        } else {
            self.current += 1;
        }
    }

    pub fn undo(&mut self) -> Option<&FractalConfig> {
        if self.current > 0 {
            self.current -= 1;
            Some(&self.history[self.current])
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<&FractalConfig> {
        if self.current < self.history.len() - 1 {
            self.current += 1;
            Some(&self.history[self.current])
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        self.current > 0
    }

    pub fn can_redo(&self) -> bool {
        self.current < self.history.len() - 1
    }
}
```

### State Capture Pattern

**Location:** [src/app/config.rs](../../src/app/config.rs)

**Before making changes:**
```rust
// Capture current state for undo
if ui_response.flame_changed {
    self.capture_state();
    // ... apply changes
}
```

**Implementation:**
```rust
pub fn capture_state(&mut self) {
    let config = self.export_config();
    self.undo_history.push(config);
}

pub fn undo(&mut self) -> bool {
    if let Some(config) = self.undo_history.undo() {
        self.import_config(config.clone());
        true
    } else {
        false
    }
}

pub fn redo(&mut self) -> bool {
    if let Some(config) = self.undo_history.redo() {
        self.import_config(config.clone());
        true
    } else {
        false
    }
}
```

### Keyboard Shortcuts

**Location:** [src/app/input.rs](../../src/app/input.rs)

- **Ctrl+Z**: Undo
- **Ctrl+Y**: Redo

```rust
if modifiers.control() && key == winit::keyboard::Key::Character("z".into()) {
    if self.undo() {
        self.view_changed_by_keyboard = true;
    }
}
```

---

## Config Import/Export

The UI provides import/export controls for saving and loading configurations.

**Location:** [src/ui/mod.rs](../../src/ui/mod.rs) - Config Import/Export section

### Export Flow

**UI:** Config Import/Export window → Export section

```rust
if ui.button("💾 Save Config").clicked() {
    ui_response.export_requested = true;
}
if ui.button("📋 Copy to Clipboard").clicked() {
    ui_response.copy_config_requested = true;
}
```

**Handler:** [src/app/mod.rs](../../src/app/mod.rs#L630-L644)

```rust
if ui_response.export_requested {
    let config = self.export_config();

    // Open file dialog
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Fractal Flame", &["fflame"])
        .save_file()
    {
        if let Ok(json) = config.to_json() {
            std::fs::write(path, json).ok();
        }
    }
}
```

### Import Flow

**UI:** Config Import/Export window → Import section

```rust
if ui.button("📂 Load Config").clicked() {
    ui_response.import_requested = true;
}
if ui.button("📋 Paste from Clipboard").clicked() {
    ui_response.paste_config_requested = true;
}
```

**Handler:** [src/app/mod.rs](../../src/app/mod.rs#L647-L680)

```rust
if ui_response.import_requested {
    // Open file dialog
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Fractal Flame", &["fflame"])
        .pick_file()
    {
        if let Ok(json) = std::fs::read_to_string(path) {
            if let Ok(config) = FractalConfig::from_json(&json) {
                self.capture_state();  // Capture before importing
                self.import_config(config);
            }
        }
    }
}
```

### Palette Library Sync

When importing a config with embedded palette data, the palette is automatically added to the library if not already present:

```rust
pub fn import_config(&mut self, config: FractalConfig) {
    // Sync palette library if config has embedded palette
    if let Some(ref palette) = config.palette {
        // Check if palette exists in library
        let exists = self.palette_library.palettes.iter()
            .any(|p| p.name == palette.name);

        if !exists {
            // Add to library
            self.palette_library.palettes.push(palette.clone());

            // Update palette index to point to new palette
            config.palette_index = self.palette_library.palettes.len() - 1;
        }
    }

    // Load config into renderer
    self.renderer.load_config(&config);

    // Update local state
    self.zoom = config.zoom;
    self.pan_x = config.pan_x;
    // ... etc
}
```

---

## Asset Loading System

Desktop builds auto-load presets and palettes from the filesystem.

**Location:** [src/scene/assets.rs](../../src/scene/assets.rs)

### Directory Structure

```
assets/
├── presets/
│   ├── sierpinski.fflame
│   ├── julia.fflame
│   └── custom.fflame
└── palettes/
    ├── fire.palette
    ├── ocean.palette
    └── custom.palette
```

### Load Functions

**Load Configs:**
```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn load_configs_from_dir(path: &str) -> Option<Vec<FractalConfig>> {
    let dir = std::fs::read_dir(path).ok()?;
    let mut configs = Vec::new();

    for entry in dir.flatten() {
        let path = entry.path();
        if path.extension() == Some(OsStr::new("fflame")) {
            if let Ok(json) = std::fs::read_to_string(&path) {
                if let Ok(config) = FractalConfig::from_json(&json) {
                    configs.push(config);
                }
            }
        }
    }

    Some(configs)
}
```

**Load Palettes:**
```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn load_palettes_from_dir(path: &str) -> Option<Vec<Palette>> {
    // Similar to load_configs_from_dir, but for .palette files
    // ...
}
```

### Platform-Specific Behavior

**Desktop (Windows/macOS/Linux):**
- Auto-loads from `assets/presets/` and `assets/palettes/`
- User can add files to these directories
- Restart app to see new assets

**WASM (Web):**
- Uses built-in presets/palettes only
- No filesystem access
- All assets compiled into binary

**Conditional Compilation:**
```rust
#[cfg(not(target_arch = "wasm32"))]
{
    // Desktop: Load from filesystem
    if let Some(configs) = load_configs_from_dir("assets/presets") {
        lib.presets.extend(configs);
    }
}
```

---

## Export Preset Files Example

The `export_presets` example generates `.fflame` files from code-based presets.

**Location:** [examples/export_presets.rs](../../examples/export_presets.rs)

**Usage:**
```bash
cargo run --example export_presets
```

**Output:**
```
Exported: assets/presets/sierpinski_triangle.fflame
Exported: assets/presets/julia_set.fflame
...
```

**Implementation:**
```rust
fn main() -> anyhow::Result<()> {
    let library = PresetLibrary::new();

    std::fs::create_dir_all("assets/presets")?;

    for config in &library.presets {
        let name = config.flame.name
            .to_lowercase()
            .replace(" ", "_");
        let filename = format!("assets/presets/{}.fflame", name);

        let json = config.to_json()?;
        std::fs::write(&filename, json)?;

        println!("Exported: {}", filename);
    }

    Ok(())
}
```

---

## State Management Best Practices

### When to Capture State

**Always capture before:**
- Editing transform parameters
- Adding/deleting transforms
- Changing flame settings
- Loading presets
- Importing configs

**Don't capture for:**
- View changes (zoom, pan, rotation) - too frequent
- Continuous sliders during drag - only on release
- Performance settings

### Reset Behavior

**What `reset()` does:**
- Clears accumulation buffers (ping-pong textures)
- Clears histogram buffer
- Resets iteration counters
- Swaps to clean buffer

**What `reset()` does NOT do:**
- Touch GPU parameter buffers
- Modify transform data
- Change palette
- Alter tone mapping settings

**When to call `reset()`:**
- After any flame/view/palette change
- After loading config/preset
- When user clicks "Reset" button
- When switching render modes (2D ↔ 3D)

### Atomic Config Loading

When loading a complete config, use `load_config()` instead of individual update functions to ensure atomic application of all settings:

```rust
// GOOD: Atomic load
self.renderer.load_config(&config);

// BAD: Piecemeal updates (race conditions possible)
self.renderer.update_flame(&config.flame);
self.renderer.update_view(config.zoom, config.pan_x, config.pan_y, config.rotation);
// ... reset() might be called between these, causing inconsistency
```

---

## Common Tasks

### Create New Preset

1. Write flame function in `presets.rs`
2. Add to `PresetLibrary::new()`
3. Or: Export current state → save as `.fflame` in `assets/presets/`

### Add Config Field

1. Add field to `FractalConfig` struct
2. Update `Default` impl
3. Update `export_config()` and `import_config()` in `app.rs`
4. Add UI control if needed
5. Handle in `load_config()` if GPU-side change

### Change Undo History Size

Modify constant in `app.rs`:
```rust
const MAX_UNDO_HISTORY: usize = 50;  // Change this value
```

### Debug Config Issues

Enable config logging:
```rust
pub fn import_config(&mut self, config: FractalConfig) {
    println!("Importing config:");
    println!("  Flame: {}", config.flame.name);
    println!("  Transforms: {}", config.flame.transforms.len());
    println!("  Render mode: {:?}", config.flame.render_mode);
    // ...

    self.renderer.load_config(&config);
}
```

---

**Last Updated:** 2025-10-28
**Related Docs:** [ARCHITECTURE.md](../ARCHITECTURE.md), [TRANSFORMS.md](TRANSFORMS.md), [UI.md](UI.md), [EXPORT.md](EXPORT.md)
