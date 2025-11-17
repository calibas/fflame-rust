# Configuration and State Management

Complete guide to the delta-based configuration system, ConfigManager, undo/redo, and serialization.

## ✅ Migration Status: COMPLETE (2025-11-17)

**All UI controls now use ConfigManager with simplified immediate updates:**
- ✅ View controls (zoom, pan, rotation, camera) - Real-time with 100ms overwrite window
- ✅ Settings sliders (iterations, blend, compression) - Coalescing within 2s window
- ✅ Tone mapping (exposure, gamma, curves) - Immediate updates, no accumulation reset
- ✅ Variation weights and parameters - Real-time with overwrite mode
- ✅ Color controls (palette, background) - Real-time with overwrite mode
- ✅ **Triangle Editor** - Batch updates for multi-param changes
- ✅ Palette editor - Direct updates, no separate preview mode

**Key Achievement**: Eliminated blank frames and preview mode complexity. All updates are immediate with automatic coalescing for undo history.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overview and module organization
- [TRANSFORMS.md](TRANSFORMS.md) - Transform structure details
- [UI.md](UI.md) - Config import/export UI controls and delta-based UI patterns
- [EXPORT.md](EXPORT.md) - PNG metadata embedding

**Archived Migration Documentation:**
All delta migration planning docs have been archived to [archive/delta-migration/](../archive/delta-migration/):
- [delta-system-completed.md](../archive/delta-migration/delta-system-completed.md) - Summary of completed work (Phases 1-10)
- [complete-delta-migration.md](../archive/delta-migration/complete-delta-migration.md) - Final migration phases (11-16)
- [MIGRATION-STATUS.md](../archive/delta-migration/MIGRATION-STATUS.md) - Detailed migration tracking
- [remove-preview-mode.md](../archive/remove-preview-mode.md) - Removal of preview mode system (PR #23)

---

## Overview

The application uses a **simplified delta-based state management system** centered around `ConfigManager`. This system provides:
- Type-safe parameter identification (`ConfigPath`)
- Automatic undo/redo with delta tracking and coalescing (2 second window)
- Selective GPU updates via `UpdateType` classification
- Real-time rendering with 100ms overwrite window (no blank frames)
- Immediate updates for all parameters (no lazy/preview distinction)

**Key Files:**
- [src/config/manager.rs](../../src/config/manager.rs) - ConfigManager implementation (~800 lines)
- [src/config/delta.rs](../../src/config/delta.rs) - ConfigPath, ConfigValue, ConfigDelta enums (568 lines)
- [src/config/fractal_config.rs](../../src/config/fractal_config.rs) - FractalConfig struct
- [src/config/slider.rs](../../src/config/slider.rs) - UI helper (config_slider) (299 lines)
- [src/config/defaults.rs](../../src/config/defaults.rs) - Default value constants
- [src/app/mod.rs](../../src/app/mod.rs) - 100ms overwrite window implementation

---

## FractalConfig Structure

The `FractalConfig` struct represents **complete application state** for exact reproducibility. Everything needed to recreate a fractal is stored in this single struct.

**Location:** [src/config/fractal_config.rs](../../src/config/fractal_config.rs)

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

## ConfigManager - Delta-Based State Management

The `ConfigManager` is the central gateway for all configuration changes. It provides type-safe delta tracking, automatic undo/redo with coalescing, and selective GPU updates.

**Location:** [src/config/manager.rs](../../src/config/manager.rs)

### Architecture

**Key Components:**
1. **current** - Current configuration state
2. **undo_history** - Vector of ConfigChange entries (max 50)
3. **redo_history** - Vector of ConfigChange entries
4. **coalescing_window** - Automatic merging of rapid changes (2 second window)
5. **modify_session** - Batch update tracking for multi-param changes

### Core Structure

```rust
pub struct ConfigManager {
    current: FractalConfig,              // Current state
    undo_history: Vec<ConfigChange>,     // Undo history (max 50)
    redo_history: Vec<ConfigChange>,     // Redo history
    last_change_time: Option<Instant>,   // For coalescing window
    modify_session: Option<ModifySession>, // Batch update tracking
}

impl ConfigManager {
    pub fn new(initial_config: FractalConfig) -> Self {
        Self {
            current: initial_config,
            undo_history: Vec::new(),
            redo_history: Vec::new(),
            last_change_time: None,
            modify_session: None,
        }
    }

    // Main entry point for parameter updates
    pub fn update_param(
        &mut self,
        path: ConfigPath,
        new_value: ConfigValue,
    ) -> Result<UpdateType, ConfigError> {
        // 1. Check for actual change
        let old_value = self.get_value(&path)?;
        if old_value.approx_eq(&new_value) {
            return Ok(UpdateType::None);
        }

        // 2. Create delta and push to undo (coalescing happens automatically)
        let delta = ConfigDelta::new(path.clone(), old_value, new_value.clone());
        let change = ConfigChange::single(delta);
        let update_type = change.update_type();

        self.push_undo(change);  // Automatic coalescing within 2s window

        // 3. Apply change
        self.set_value(&path, new_value)?;
        self.record_action(update_type);

        Ok(update_type)
    }

    // Batch updates for multi-param changes
    pub fn update_batch(
        &mut self,
        changes: Vec<(ConfigPath, ConfigValue)>,
        description: &str,
    ) -> Result<UpdateType, ConfigError> { /* ... */ }

    // Undo/redo operations
    pub fn undo(&mut self) -> Option<ConfigChange> { /* ... */ }
    pub fn redo(&mut self) -> Option<ConfigChange> { /* ... */ }
}
```

### ConfigPath - Type-Safe Parameter Identification

**Location:** [src/config/delta.rs](../../src/config/delta.rs)

`ConfigPath` is an enum with 100+ variants covering every editable parameter:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConfigPath {
    // View parameters (no fractal recalc)
    Zoom,
    PanX,
    PanY,
    Rotation,
    CameraRotationX,
    CameraRotationY,

    // Tone mapping (no reset needed)
    Exposure,
    Gamma,
    DensityScale,
    TonemapMode,
    TonemapCurve,
    UseCurve,

    // Color (reset needed, no recompute)
    ColorMode,
    PaletteIndex,
    Palette(Box<Palette>),
    SpeedFactor,
    BackgroundColor,

    // Rendering settings
    IterationsPerThread,
    SpeedMultiplier,
    HistogramColorScale,
    LowDensitySmoothing,
    DensityCompressionStrength,
    BlendFactor,
    UseDynamicBlend,
    TargetIterationsPerPixel,
    MaxIterations,
    DeterministicRng,

    // Transform-level (reset + recompute)
    TransformCount,
    TransformWeight { index: usize },
    TransformColor { index: usize, component: ColorComponent },
    TransformColorSpeed { index: usize },
    TransformAffine { index: usize, param: AffineParam },
    TransformVariation { index: usize, variation: String },
    TransformVariationParam { index: usize, variation: String, param: String },

    // Flame-level
    RenderMode,
    ProjectionType,
}

// Human-readable display for undo history
impl Display for ConfigPath {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ConfigPath::Exposure => write!(f, "Exposure"),
            ConfigPath::TransformAffine { index, param } =>
                write!(f, "Transform {} → Affine {}", index + 1, param),
            ConfigPath::TransformVariation { index, variation } =>
                write!(f, "Transform {} → {} variation", index + 1, variation),
            // ... etc (100+ variants)
        }
    }
}
```

### ConfigValue - Type-Safe Value Container

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    Float(f32),
    Int(i32),
    UInt(u32),
    UInt64(u64),
    Bool(bool),
    String(String),
    ColorRgb([f32; 3]),
    ToneMapMode(ToneMapMode),
    ColorMode(ColorMode),
    RenderMode(RenderMode),
    ProjectionType(ProjectionType),
    ToneCurve(ToneCurve),
    Palette(Box<Palette>),
}
```

### UpdateType - Selective GPU Updates

`UpdateType` classifies changes to determine minimal GPU work required:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateType {
    None,             // No GPU update
    ViewOnly,         // Camera/zoom → use overwrite mode
    ColorOnly,        // Palette → use overwrite mode
    IterationReset,   // Transform/variation → use overwrite mode + iteration reset after 100ms
    ToneMappingOnly,  // Tone mapping → no accumulation buffer changes
}
```

**How Updates Work:**

1. **ViewOnly / ColorOnly / IterationReset** - Trigger 100ms overwrite window:
   - `blend_factor = 1.0` replaces accumulation buffer (no blending)
   - `batch_size = 1` accumulates every frame (smooth 60fps updates)
   - Window keeps overwrite ON for 100ms after last change (~6 frames)
   - After window expires: return to normal accumulation (`blend_factor = 0.1`, `batch_size = 4`)

2. **IterationReset Additional Behavior**:
   - When 100ms window expires, reset iteration counter to 0
   - Provides clean rebuild after transform/variation changes
   - No blank frames (overwrite mode prevents buffer clear)

3. **ToneMappingOnly**:
   - No overwrite mode (excluded from had_changes check)
   - Continues accumulating while tone mapping parameters update
   - No performance impact on sample generation

**Mapping:**
```rust
fn classify_update(path: &ConfigPath) -> UpdateType {
    match path {
        ConfigPath::Zoom | ConfigPath::PanX | ConfigPath::PanY | ConfigPath::Rotation
        | ConfigPath::CameraRotationX | ConfigPath::CameraRotationY => UpdateType::ViewOnly,

        ConfigPath::ColorMode | ConfigPath::PaletteIndex | ConfigPath::Palette(_)
        | ConfigPath::SpeedFactor | ConfigPath::BackgroundColor => UpdateType::ColorOnly,

        ConfigPath::TransformAffine { .. } | ConfigPath::TransformVariation { .. }
        | ConfigPath::TransformVariationParam { .. } | ConfigPath::TransformWeight { .. }
        | ConfigPath::TransformColor { .. } | ConfigPath::TransformColorSpeed { .. }
        | ConfigPath::TransformCount | ConfigPath::RenderMode | ConfigPath::ProjectionType
            => UpdateType::IterationReset,

        ConfigPath::Exposure | ConfigPath::Gamma | ConfigPath::TonemapMode
        | ConfigPath::TonemapCurve | ConfigPath::UseCurve => UpdateType::ToneMappingOnly,

        _ => UpdateType::None,
    }
}
```

### Undo/Redo Implementation

**Undo Operation:**
```rust
pub fn undo(&mut self) -> Option<ConfigDelta> {
    if self.undo_stack.is_empty() {
        return None;
    }

    // Push current state to redo stack
    self.redo_stack.push(self.active_config.clone());

    // Pop previous state from undo stack
    let previous_config = self.undo_stack.pop().unwrap();

    // Compute delta for display
    let delta = self.compute_delta(&previous_config, &self.active_config);

    // Apply previous state
    self.active_config = previous_config;

    Some(delta)
}
```

**Redo Operation:**
```rust
pub fn redo(&mut self) -> Option<ConfigDelta> {
    if self.redo_stack.is_empty() {
        return None;
    }

    // Push current state to undo stack
    self.undo_stack.push(self.active_config.clone());

    // Pop next state from redo stack
    let next_config = self.redo_stack.pop().unwrap();

    // Compute delta for display
    let delta = self.compute_delta(&self.active_config, &next_config);

    // Apply next state
    self.active_config = next_config;

    Some(delta)
}
```

### Coalescing - Automatic Undo Merging

**Purpose:** Prevent undo stack bloat during rapid parameter changes (e.g., slider drags, mouse panning).

**How It Works:**
- ConfigManager tracks `last_change_time` for each update
- Changes to the **same ConfigPath** within 2 seconds are merged into single undo entry
- Changes to **different ConfigPath** always create separate undo entries
- No explicit lazy/throttling logic needed - coalescing is automatic

**Example:**
```rust
// User drags Exposure slider for 1 second (100 changes)
config_manager.update_param(ConfigPath::Exposure, 1.0.into())?;
config_manager.update_param(ConfigPath::Exposure, 1.1.into())?;  // Merged with previous
config_manager.update_param(ConfigPath::Exposure, 1.2.into())?;  // Merged with previous
// ... 97 more updates ...
// Result: 1 undo entry (1.0 → 1.2)

// User changes Gamma (different path)
config_manager.update_param(ConfigPath::Gamma, 2.2.into())?;  // New undo entry

// Wait 2+ seconds, then change Exposure again
thread::sleep(Duration::from_secs(2));
config_manager.update_param(ConfigPath::Exposure, 1.5.into())?;  // New undo entry (window expired)
```

**Benefits:**
- Slider drags create 1 undo entry (not 100+)
- Mouse panning creates 1 undo entry per drag session
- Multi-parameter changes (different paths) properly tracked
- No manual commit logic needed in UI code

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

## UI Helpers for ConfigManager

**Location:** [src/config/slider.rs](../../src/config/slider.rs)

### config_slider() - Simple Slider Helper

Basic slider with automatic coalescing (no manual commit logic needed):

```rust
use crate::config::slider::config_slider;

config_slider(ui, &mut config_manager, ConfigPath::Exposure, 0.1..=5.0)
    .text("Exposure")
    .suffix("x")
    .show();
```

**Behavior:**
- Every change calls `config_manager.update_param()`
- Coalescing automatically merges rapid changes to same parameter
- Result: 1 undo entry per drag session (automatic 2s window)
- Works for all controls: sliders, drag values, checkboxes, etc.

### Standard UI Pattern

All UI controls follow the same simple pattern:

```rust
// Read current value
let mut value = config_manager.current().exposure;

// Show UI control
let response = ui.add(egui::Slider::new(&mut value, 0.1..=5.0).text("Exposure"));

// Update if changed
if response.changed() {
    config_manager.update_param(ConfigPath::Exposure, value.into())?;
}
```

**That's it!** No lazy parameters, no force_commit calls, no preview mode logic.

### Handling UpdateType

Update actions are tracked automatically by App via `config_manager.consume_actions()`:

```rust
// In App::render() after UI updates
let actions = self.config_manager.consume_actions();

// Handle GPU updates based on what changed
if actions.update_view {
    let config = self.config_manager.current();
    self.flame_renderer.update_view(/* ... */);
}
if actions.update_flame {
    let config = self.config_manager.current();
    self.flame_renderer.update_flame(/* ... */);
}
if actions.update_tone_curve {
    let config = self.config_manager.current();
    self.flame_renderer.update_tonemap(/* ... */);
}
// etc.
```

**100ms Overwrite Window** (automatic in [src/app/mod.rs](../../src/app/mod.rs)):
- Triggered by `actions.update_view`, `actions.update_palette`, or `actions.update_flame`
- Keeps overwrite mode ON for 100ms after last change (~6 frames at 60fps)
- Provides smooth real-time updates with no blank frames
- No UI code needs to know about this - it's handled automatically

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

**Last Updated:** 2025-11-17
**Related Docs:** [ARCHITECTURE.md](../ARCHITECTURE.md), [TRANSFORMS.md](TRANSFORMS.md), [UI.md](UI.md), [EXPORT.md](EXPORT.md)

**Major Changes:**

**2025-10-31 (PR #22 - Undo/Redo Improvements):**
- Replaced UndoHistory with ConfigManager delta-based system
- Added ConfigPath, ConfigValue, ConfigDelta documentation
- Added UpdateType classification for selective GPU updates
- All 100+ parameters now have type-safe ConfigPath variants

**2025-11-17 (PR #23 - Remove Preview Mode):**
- Removed preview mode system and lazy parameter complexity
- Simplified all UI controls to immediate updates with automatic coalescing
- Implemented 100ms overwrite window for real-time rendering without blank frames
- Removed LazyUndoHelper - replaced with automatic coalescing (2s window)
- Removed live preview mode - palette editor uses direct updates
- Simplified UpdateType enum (ViewOnly, ColorOnly, IterationReset, ToneMappingOnly)
