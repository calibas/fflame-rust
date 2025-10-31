# Configuration and State Management

Complete guide to the delta-based configuration system, ConfigManager, undo/redo, and serialization.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overview and module organization
- [TRANSFORMS.md](TRANSFORMS.md) - Transform structure details
- [UI.md](UI.md) - Config import/export UI controls and delta-based UI patterns
- [EXPORT.md](EXPORT.md) - PNG metadata embedding

**Project Documentation (Delta System):**
- [projects/delta-based-state-management.md](../../docs/projects/delta-based-state-management.md) - Original 2,600-line plan (RETIRED, historical)
- [projects/delta-system-completed.md](../../docs/projects/delta-system-completed.md) - Summary of completed work (Phases 1-10)
- [projects/complete-delta-migration.md](../../docs/projects/complete-delta-migration.md) - Active migration plan (Phases 11-14)
- [projects/MIGRATION-STATUS.md](../../docs/projects/MIGRATION-STATUS.md) - Detailed migration tracking

---

## Overview

The application uses a **delta-based state management system** centered around `ConfigManager`. This replaces the previous flag-based approach with:
- Type-safe parameter identification (`ConfigPath`)
- Automatic undo/redo with delta tracking
- Selective GPU updates via `UpdateType` classification
- Lazy undo helpers for continuous controls (sliders, mouse)
- Live preview mode for temporary changes (palette editor)

**Key Files:**
- [src/config/manager.rs](../../src/config/manager.rs) - ConfigManager implementation (1,237 lines)
- [src/config/delta.rs](../../src/config/delta.rs) - ConfigPath, ConfigValue, ConfigDelta enums (568 lines)
- [src/config/fractal_config.rs](../../src/config/fractal_config.rs) - FractalConfig struct
- [src/config/slider.rs](../../src/config/slider.rs) - UI helpers (config_slider, lazy_slider) (299 lines)
- [src/config/defaults.rs](../../src/config/defaults.rs) - Default value constants

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

The `ConfigManager` is the central gateway for all configuration changes. It replaces the old flag-based system with type-safe delta tracking and automatic undo/redo.

**Location:** [src/config/manager.rs](../../src/config/manager.rs)

### Architecture

**Key Components:**
1. **active_config** - Current state (visible to user)
2. **preview_config** - Temporary state during live editing (e.g., palette editor)
3. **undo_stack** - Vec<FractalConfig> (max 50 states)
4. **redo_stack** - Vec<FractalConfig> (cleared on new changes)
5. **lazy_undo_helper** - Smart throttling for continuous controls

### Core Structure

```rust
pub struct ConfigManager {
    active_config: FractalConfig,        // Current state
    preview_config: Option<FractalConfig>, // Live preview mode
    undo_stack: Vec<FractalConfig>,      // Undo history (max 50)
    redo_stack: Vec<FractalConfig>,      // Redo history
    lazy_undo_helper: LazyUndoHelper,    // Throttled undo for sliders/mouse
}

impl ConfigManager {
    pub fn new(initial_config: FractalConfig) -> Self {
        Self {
            active_config: initial_config,
            preview_config: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            lazy_undo_helper: LazyUndoHelper::new(),
        }
    }

    // Main entry point for parameter updates
    pub fn update_config(&mut self, path: ConfigPath, value: ConfigValue) -> UpdateType {
        // 1. Compute delta
        let old_value = self.get_value(&path);
        if old_value == value {
            return UpdateType::None; // No change
        }

        // 2. Capture undo state (if not throttled)
        if self.should_capture_for_path(&path) {
            self.push_undo();
        }

        // 3. Apply change
        self.set_value(&path, value);

        // 4. Classify update type for selective GPU updates
        self.classify_update(&path)
    }

    // Undo/redo operations
    pub fn undo(&mut self) -> Option<ConfigDelta> { /* ... */ }
    pub fn redo(&mut self) -> Option<ConfigDelta> { /* ... */ }

    // Live preview mode (palette editor)
    pub fn enter_preview_mode(&mut self) { /* ... */ }
    pub fn commit_preview(&mut self) { /* ... */ }
    pub fn revert_preview(&mut self) { /* ... */ }
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
    None,       // No GPU update
    View,       // Camera/zoom → reset accumulation
    Color,      // Palette → reset accumulation
    Flame,      // Transform/variation → reset accumulation
    ToneMap,    // Tone mapping → no reset
    Rendering,  // Speed/quality settings
}
```

**Mapping:**
```rust
fn classify_update(path: &ConfigPath) -> UpdateType {
    match path {
        ConfigPath::Zoom | ConfigPath::PanX | ConfigPath::PanY | ConfigPath::Rotation
        | ConfigPath::CameraRotationX | ConfigPath::CameraRotationY => UpdateType::View,

        ConfigPath::ColorMode | ConfigPath::PaletteIndex | ConfigPath::Palette(_)
        | ConfigPath::SpeedFactor | ConfigPath::BackgroundColor => UpdateType::Color,

        ConfigPath::TransformAffine { .. } | ConfigPath::TransformVariation { .. }
        | ConfigPath::TransformVariationParam { .. } | ConfigPath::TransformWeight { .. }
        | ConfigPath::TransformColor { .. } | ConfigPath::TransformColorSpeed { .. }
        | ConfigPath::TransformCount | ConfigPath::RenderMode | ConfigPath::ProjectionType
            => UpdateType::Flame,

        ConfigPath::Exposure | ConfigPath::Gamma | ConfigPath::TonemapMode
        | ConfigPath::TonemapCurve | ConfigPath::UseCurve => UpdateType::ToneMap,

        ConfigPath::IterationsPerThread | ConfigPath::SpeedMultiplier
        | ConfigPath::HistogramColorScale | /* ... */ => UpdateType::Rendering,

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

### LazyUndoHelper - Smart Throttling

**Purpose:** Prevent undo stack bloat during continuous slider drags or mouse panning.

**Location:** [src/config/slider.rs](../../src/config/slider.rs)

**Behavior:**
```rust
pub struct LazyUndoHelper {
    last_capture_time: Option<Instant>,
    throttle_duration: Duration,  // Default: 500ms
}

impl LazyUndoHelper {
    // Should we capture undo state now?
    pub fn should_capture(&mut self) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_capture_time {
            if now.duration_since(last) < self.throttle_duration {
                return false; // Too soon, skip capture
            }
        }
        self.last_capture_time = Some(now);
        true
    }

    // Force commit on mouse release / slider drag end
    pub fn force_commit_now(&mut self) -> bool {
        self.last_capture_time = Some(Instant::now());
        true // Always capture final state
    }
}
```

**Usage Pattern:**
- Drag start: Capture initial state
- During drag: Skip captures (throttled)
- Drag end: Capture final state (forced commit)
- Result: 2 undo entries per slider drag (initial + final)

### Live Preview Mode

**Use Case:** Palette editor - allow temporary color changes with instant revert.

**Flow:**
```rust
// Enter preview mode (save current state)
config_manager.enter_preview_mode();

// Make temporary changes
config_manager.update_config(ConfigPath::PaletteIndex, ConfigValue::UInt(5));
// User sees immediate visual update

// Option 1: Commit changes (keep modifications)
config_manager.commit_preview();

// Option 2: Revert changes (restore saved state)
config_manager.revert_preview();
```

**Implementation:**
```rust
pub fn enter_preview_mode(&mut self) {
    // Save current state
    self.preview_config = Some(self.active_config.clone());
}

pub fn commit_preview(&mut self) {
    // Capture undo for the entire preview session
    if self.preview_config.is_some() {
        self.push_undo();
        self.preview_config = None;
    }
}

pub fn revert_preview(&mut self) {
    // Restore saved state (no undo entry)
    if let Some(saved) = self.preview_config.take() {
        self.active_config = saved;
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

## UI Helpers for ConfigManager

**Location:** [src/config/slider.rs](../../src/config/slider.rs)

### config_slider() - Immediate Undo

Basic slider with automatic undo capture on every change:

```rust
use crate::config::slider::config_slider;

config_slider(ui, &mut config_manager, ConfigPath::Exposure, 0.1..=5.0)
    .text("Exposure")
    .suffix("x")
    .show();
```

**Behavior:**
- Captures undo state on first change
- No throttling - every value change creates undo entry
- Best for: Discrete controls, toggles, dropdowns
- NOT recommended for: Continuous sliders, mouse drags

### lazy_slider() - Throttled Undo

Slider with intelligent undo throttling (500ms minimum between captures):

```rust
use crate::config::slider::lazy_slider;

lazy_slider(ui, &mut config_manager, ConfigPath::PanX, -5.0..=5.0)
    .text("Pan X")
    .show();
```

**Behavior:**
- Drag start: Captures initial state
- During drag: Skips undo captures (throttled)
- Drag end: Captures final state (forced commit)
- Result: 2 undo entries per drag session (initial + final)
- Best for: Continuous sliders, view controls

### config_drag_value() - DragValue with Undo

Similar to config_slider but uses egui's DragValue widget:

```rust
use crate::config::slider::config_drag_value;

config_drag_value(ui, &mut config_manager, ConfigPath::SpeedMultiplier, 1..=16)
    .prefix("Speed: ")
    .suffix("x")
    .show();
```

### Handling UpdateType

All helpers return `UpdateType` - handle in App:

```rust
// In UI window
let update_type = lazy_slider(ui, config_manager, ConfigPath::Zoom, 0.1..=10.0)
    .text("Zoom")
    .show();

// In App::render()
match update_type {
    UpdateType::View => {
        let config = self.config_manager.active_config();
        self.flame_renderer.update_view(
            config.zoom, config.pan_x, config.pan_y, config.rotation
        );
        self.flame_renderer.reset();
    }
    UpdateType::Flame => {
        let config = self.config_manager.active_config();
        self.flame_renderer.update_flame(&config.flame, true);
        self.flame_renderer.reset();
    }
    UpdateType::ToneMap => {
        let config = self.config_manager.active_config();
        self.flame_renderer.update_tonemap(
            config.tonemap_mode, config.tonemap_curve,
            config.use_curve, config.exposure, config.gamma
        );
        // No reset needed for tone mapping
    }
    _ => {}
}
```

### Custom Controls Pattern

For custom UI controls (not using helpers):

```rust
// Manual ConfigManager integration
let mut value = config_manager.active_config().exposure;
if ui.add(egui::Slider::new(&mut value, 0.1..=5.0).text("Exposure")).changed() {
    let update_type = config_manager.update_config(
        ConfigPath::Exposure,
        ConfigValue::Float(value)
    );
    // Handle update_type...
}
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

**Last Updated:** 2025-10-31
**Related Docs:** [ARCHITECTURE.md](../ARCHITECTURE.md), [TRANSFORMS.md](TRANSFORMS.md), [UI.md](UI.md), [EXPORT.md](EXPORT.md)

**Major Changes (2025-10-31):**
- Replaced UndoHistory with ConfigManager delta-based system
- Added ConfigPath, ConfigValue, ConfigDelta documentation
- Added UpdateType classification for selective GPU updates
- Documented LazyUndoHelper for smart undo throttling
- Added Live Preview Mode documentation
- Added UI helper functions (config_slider, lazy_slider, etc.)
- All 100+ parameters now have type-safe ConfigPath variants
