# Delta-Based State Management System

**Status:** Planning
**Created:** 2025-10-29
**Category:** Architecture Refactor

## Problem Statement

The current UI/fractal/undo system uses dozens of boolean flags (`*_changed`, `flame_changed`, etc.) to track changes. This leads to:

1. **Complex flag management**: Each UI component sets multiple flags
2. **Unclear undo capture logic**: Hard to know what triggers undo
3. **Inefficient updates**: Can't distinguish between view-only vs full fractal recompute
4. **Poor UX**: Undo history shows no information about what changed
5. **Lazy undo issues**: Per-widget helpers create cross-talk and extra captures

## Solution Overview

**Core Concept**: All fractal changes flow through a single gateway that:
1. Detects what changed (delta)
2. Decides if undo capture needed (with smart lazy throttling)
3. Applies selective updates based on change type
4. Records human-readable change description for undo window

**Key Innovation**: Store undo history as **deltas** (what changed), not full configs.

## Architecture

### 1. Configuration Path System

**Purpose**: Type-safe way to identify any parameter in FractalConfig

```rust
/// Identifies a specific parameter in the configuration
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConfigPath {
    // View parameters (no fractal recalc needed)
    Zoom,
    PanX,
    PanY,
    Rotation,
    CameraRotationX,
    CameraRotationY,

    // Tone mapping (no iteration reset needed)
    Exposure,
    Gamma,
    DensityScale,
    TonemapMode,
    TonemapCurve,
    UseCurve,

    // Color (no iteration reset, just color buffer update)
    ColorMode,
    PaletteIndex,
    Palette(Box<Palette>),  // Embed palette data for undo
    SpeedFactor,
    BackgroundColor,

    // Rendering settings (affects iteration speed/quality)
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

    // Transform-level changes (require iteration reset)
    TransformCount,
    TransformWeight { index: usize },
    TransformColor { index: usize, component: ColorComponent },  // R, G, B
    TransformColorSpeed { index: usize },
    TransformAffine { index: usize, param: AffineParam },  // a, b, c, d, e, f, g
    TransformVariation { index: usize, variation: String },
    TransformVariationParam { index: usize, variation: String, param: String },

    // Flame-level (require iteration reset)
    RenderMode,
    ProjectionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorComponent { R, G, B }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AffineParam { A, B, C, D, E, F, G }
```

**Display trait** for human-readable strings:
```rust
impl Display for ConfigPath {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ConfigPath::Exposure => write!(f, "Exposure"),
            ConfigPath::TransformVariation { index, variation } => {
                write!(f, "Transform {} → {} variation", index + 1, variation)
            }
            ConfigPath::TransformAffine { index, param } => {
                write!(f, "Transform {} → Affine {}", index + 1, param)
            }
            // ... etc
        }
    }
}
```

### 2. Configuration Value System

**Purpose**: Type-safe container for any config value

```rust
/// A value that can be stored in FractalConfig
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
    Palette(Palette),
}

impl ConfigValue {
    /// Check if two values are approximately equal (for floats)
    pub fn approx_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ConfigValue::Float(a), ConfigValue::Float(b)) => {
                (a - b).abs() < 1e-6
            }
            _ => self == other
        }
    }
}
```

**Conversion traits** from/to specific types:
```rust
impl From<f32> for ConfigValue {
    fn from(v: f32) -> Self { ConfigValue::Float(v) }
}

impl TryFrom<ConfigValue> for f32 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Float(f) => Ok(f),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}
// ... similar for other types
```

### 3. Configuration Delta System

**Purpose**: Record what changed and by how much

```rust
/// A single parameter change
#[derive(Debug, Clone)]
pub struct ConfigDelta {
    pub path: ConfigPath,
    pub old_value: ConfigValue,
    pub new_value: ConfigValue,
    pub timestamp: Instant,
}

impl ConfigDelta {
    /// Create delta by comparing values
    pub fn new(path: ConfigPath, old: ConfigValue, new: ConfigValue) -> Self {
        Self {
            path,
            old_value: old,
            new_value: new,
            timestamp: Instant::now(),
        }
    }

    /// Invert delta (for undo)
    pub fn invert(&self) -> Self {
        Self {
            path: self.path.clone(),
            old_value: self.new_value.clone(),
            new_value: self.old_value.clone(),
            timestamp: Instant::now(),
        }
    }

    /// Human-readable description
    pub fn description(&self) -> String {
        format!("{}: {} → {}", self.path, self.old_value, self.new_value)
    }
}

/// A batch of related changes (single undo point)
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub deltas: Vec<ConfigDelta>,
    pub timestamp: Instant,
    pub description: String,  // e.g., "Load preset: Sierpinski"
}

impl ConfigChange {
    /// Create from single delta
    pub fn single(delta: ConfigDelta) -> Self {
        let description = delta.description();
        Self {
            deltas: vec![delta],
            timestamp: delta.timestamp,
            description,
        }
    }

    /// Create from multiple deltas with custom description
    pub fn batch(deltas: Vec<ConfigDelta>, description: String) -> Self {
        let timestamp = deltas.first()
            .map(|d| d.timestamp)
            .unwrap_or_else(Instant::now);
        Self {
            deltas,
            timestamp,
            description,
        }
    }

    /// Invert change (for undo)
    pub fn invert(&self) -> Self {
        Self {
            deltas: self.deltas.iter().rev().map(|d| d.invert()).collect(),
            timestamp: Instant::now(),
            description: format!("Undo: {}", self.description),
        }
    }

    /// Determine update type needed for these changes
    pub fn update_type(&self) -> UpdateType {
        let mut result = UpdateType::None;
        for delta in &self.deltas {
            result = result.merge(delta.path.update_type());
        }
        result
    }
}
```

### 4. Update Type Classification

**Purpose**: Determine what needs to be updated based on which parameters changed

```rust
/// What kind of update is needed for a change
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdateType {
    None,            // No update needed (shouldn't happen)
    ViewOnly,        // Just update view transform (zoom, pan, rotation)
    ToneMappingOnly, // Re-run tonemap pass (exposure, gamma)
    ColorOnly,       // Re-run color accumulation (palette, color mode)
    IterationReset,  // Full reset - clear accumulation, restart iterations
}

impl UpdateType {
    /// Merge two update types (take the more severe one)
    pub fn merge(self, other: Self) -> Self {
        self.max(other)
    }
}

impl ConfigPath {
    /// What kind of update does changing this parameter require?
    pub fn update_type(&self) -> UpdateType {
        match self {
            // View parameters - just math, no GPU work
            ConfigPath::Zoom
            | ConfigPath::PanX
            | ConfigPath::PanY
            | ConfigPath::Rotation
            | ConfigPath::CameraRotationX
            | ConfigPath::CameraRotationY => UpdateType::ViewOnly,

            // Tone mapping - re-run tonemap shader
            ConfigPath::Exposure
            | ConfigPath::Gamma
            | ConfigPath::DensityScale
            | ConfigPath::TonemapMode
            | ConfigPath::TonemapCurve
            | ConfigPath::UseCurve
            | ConfigPath::BackgroundColor => UpdateType::ToneMappingOnly,

            // Color parameters - re-run accumulation with new colors
            ConfigPath::ColorMode
            | ConfigPath::PaletteIndex
            | ConfigPath::Palette(_)
            | ConfigPath::SpeedFactor => UpdateType::ColorOnly,

            // Rendering settings - affect iteration behavior
            ConfigPath::IterationsPerThread
            | ConfigPath::SpeedMultiplier
            | ConfigPath::HistogramColorScale
            | ConfigPath::LowDensitySmoothing
            | ConfigPath::DensityCompressionStrength
            | ConfigPath::BlendFactor
            | ConfigPath::UseDynamicBlend
            | ConfigPath::TargetIterationsPerPixel => UpdateType::IterationReset,

            // Transform/flame changes - full reset
            ConfigPath::TransformCount
            | ConfigPath::TransformWeight { .. }
            | ConfigPath::TransformColor { .. }
            | ConfigPath::TransformColorSpeed { .. }
            | ConfigPath::TransformAffine { .. }
            | ConfigPath::TransformVariation { .. }
            | ConfigPath::TransformVariationParam { .. }
            | ConfigPath::RenderMode
            | ConfigPath::ProjectionType
            | ConfigPath::MaxIterations
            | ConfigPath::DeterministicRng => UpdateType::IterationReset,
        }
    }
}
```

### 5. Configuration Manager

**Purpose**: Central authority for all config changes, undo/redo, and updates

```rust
pub struct ConfigManager {
    /// Current configuration
    current: FractalConfig,

    /// Undo stack (deltas, not full configs)
    undo_stack: Vec<ConfigChange>,

    /// Redo stack
    redo_stack: Vec<ConfigChange>,

    /// Maximum undo history
    max_undo_depth: usize,

    /// Last time we created a lazy undo point
    last_lazy_undo: Option<Instant>,

    /// Lazy undo throttle duration (500ms)
    lazy_throttle: Duration,
}

impl ConfigManager {
    pub fn new(config: FractalConfig) -> Self {
        Self {
            current: config,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo_depth: 50,
            last_lazy_undo: None,
            lazy_throttle: Duration::from_millis(500),
        }
    }

    /// Apply a single parameter change
    pub fn update_param(
        &mut self,
        path: ConfigPath,
        new_value: ConfigValue,
        lazy: bool,
    ) -> Result<UpdateType, ConfigError> {
        // Get current value
        let old_value = self.get_value(&path)?;

        // Check if actually changed
        if old_value.approx_eq(&new_value) {
            return Ok(UpdateType::None);
        }

        // Create delta
        let delta = ConfigDelta::new(path, old_value, new_value.clone());
        let change = ConfigChange::single(delta);

        // Decide if we should capture undo
        let should_capture = if lazy {
            self.should_capture_lazy_undo()
        } else {
            true
        };

        // Capture undo point if needed
        if should_capture {
            self.push_undo(change.clone());
        }

        // Apply change to current config
        self.set_value(&change.deltas[0].path, new_value)?;

        // Return what kind of update is needed
        Ok(change.update_type())
    }

    /// Apply a batch of changes (single undo point)
    pub fn update_batch(
        &mut self,
        changes: Vec<(ConfigPath, ConfigValue)>,
        description: String,
        lazy: bool,
    ) -> Result<UpdateType, ConfigError> {
        let mut deltas = Vec::new();

        // Create deltas for all changes
        for (path, new_value) in changes {
            let old_value = self.get_value(&path)?;
            if !old_value.approx_eq(&new_value) {
                deltas.push(ConfigDelta::new(path, old_value, new_value));
            }
        }

        if deltas.is_empty() {
            return Ok(UpdateType::None);
        }

        let change = ConfigChange::batch(deltas, description);

        // Decide if we should capture undo
        let should_capture = if lazy {
            self.should_capture_lazy_undo()
        } else {
            true
        };

        // Capture undo point if needed
        if should_capture {
            self.push_undo(change.clone());
        }

        // Apply all changes
        for delta in &change.deltas {
            self.set_value(&delta.path, delta.new_value.clone())?;
        }

        Ok(change.update_type())
    }

    /// Undo last change
    pub fn undo(&mut self) -> Result<UpdateType, ConfigError> {
        let change = self.undo_stack.pop()
            .ok_or(ConfigError::EmptyUndoStack)?;

        let inverted = change.invert();

        // Apply inverted deltas
        for delta in &inverted.deltas {
            self.set_value(&delta.path, delta.new_value.clone())?;
        }

        // Push to redo stack
        self.redo_stack.push(change);

        Ok(inverted.update_type())
    }

    /// Redo last undone change
    pub fn redo(&mut self) -> Result<UpdateType, ConfigError> {
        let change = self.redo_stack.pop()
            .ok_or(ConfigError::EmptyRedoStack)?;

        // Apply deltas
        for delta in &change.deltas {
            self.set_value(&delta.path, delta.new_value.clone())?;
        }

        // Push to undo stack
        self.push_undo(change);

        Ok(change.update_type())
    }

    /// Check if we should capture lazy undo (throttling logic)
    fn should_capture_lazy_undo(&mut self) -> bool {
        let now = Instant::now();

        match self.last_lazy_undo {
            None => {
                // First lazy change - always capture
                self.last_lazy_undo = Some(now);
                true
            }
            Some(last) => {
                let elapsed = now.duration_since(last);
                if elapsed >= self.lazy_throttle {
                    // Enough time passed - capture
                    self.last_lazy_undo = Some(now);
                    true
                } else {
                    // Too soon - skip
                    false
                }
            }
        }
    }

    /// Reset lazy undo timer (call on drag end to ensure final state is captured)
    pub fn reset_lazy_undo(&mut self) {
        self.last_lazy_undo = None;
    }

    /// Push change to undo stack, maintaining depth limit
    fn push_undo(&mut self, change: ConfigChange) {
        self.undo_stack.push(change);

        // Trim if over limit
        if self.undo_stack.len() > self.max_undo_depth {
            self.undo_stack.remove(0);
        }

        // Clear redo stack (new change invalidates redo)
        self.redo_stack.clear();
    }

    /// Get value from config by path
    fn get_value(&self, path: &ConfigPath) -> Result<ConfigValue, ConfigError> {
        match path {
            ConfigPath::Exposure => Ok(self.current.exposure.into()),
            ConfigPath::Gamma => Ok(self.current.gamma.into()),
            ConfigPath::TransformVariation { index, variation } => {
                let xform = self.current.flame.transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight = xform.variations.get(variation)
                    .copied()
                    .unwrap_or(0.0);
                Ok(weight.into())
            }
            // ... etc for all paths
            _ => todo!("Implement all path getters")
        }
    }

    /// Set value in config by path
    fn set_value(&mut self, path: &ConfigPath, value: ConfigValue) -> Result<(), ConfigError> {
        match path {
            ConfigPath::Exposure => {
                self.current.exposure = value.try_into()?;
            }
            ConfigPath::Gamma => {
                self.current.gamma = value.try_into()?;
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = self.current.flame.transforms.get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight: f32 = value.try_into()?;
                if weight == 0.0 {
                    xform.variations.remove(variation);
                } else {
                    xform.variations.insert(variation.clone(), weight);
                }
            }
            // ... etc for all paths
            _ => todo!("Implement all path setters")
        }
        Ok(())
    }

    /// Get current config (read-only)
    pub fn config(&self) -> &FractalConfig {
        &self.current
    }

    /// Get undo stack (for displaying in undo window)
    pub fn undo_history(&self) -> &[ConfigChange] {
        &self.undo_stack
    }

    /// Get redo stack
    pub fn redo_history(&self) -> &[ConfigChange] {
        &self.redo_stack
    }
}

#[derive(Debug)]
pub enum ConfigError {
    TypeMismatch,
    InvalidIndex,
    EmptyUndoStack,
    EmptyRedoStack,
}
```

### 6. UI Slider Binding System

**Purpose**: Declarative slider creation that auto-wires to ConfigManager

```rust
/// Builder for creating sliders bound to config parameters
pub struct ConfigSlider<'a> {
    manager: &'a mut ConfigManager,
    path: ConfigPath,
    current_value: f32,
    range: RangeInclusive<f32>,
    label: String,
    lazy: bool,
    speed: Option<f32>,  // For DragValue
}

impl<'a> ConfigSlider<'a> {
    pub fn new(
        manager: &'a mut ConfigManager,
        path: ConfigPath,
        range: RangeInclusive<f32>,
        label: impl Into<String>,
    ) -> Result<Self, ConfigError> {
        let current_value = manager.get_value(&path)?.try_into()?;
        Ok(Self {
            manager,
            path,
            current_value,
            range,
            label: label.into(),
            lazy: false,
            speed: None,
        })
    }

    /// Enable lazy undo (throttled captures)
    pub fn lazy(mut self, lazy: bool) -> Self {
        self.lazy = lazy;
        self
    }

    /// Use DragValue instead of Slider
    pub fn drag_speed(mut self, speed: f32) -> Self {
        self.speed = Some(speed);
        self
    }

    /// Render the slider and handle updates
    pub fn show(mut self, ui: &mut egui::Ui) -> Result<UpdateType, ConfigError> {
        let response = if let Some(speed) = self.speed {
            ui.add(egui::DragValue::new(&mut self.current_value).speed(speed))
        } else {
            ui.add(egui::Slider::new(&mut self.current_value, self.range.clone()).text(&self.label))
        };

        if response.changed() {
            // Value changed - update config
            let update_type = self.manager.update_param(
                self.path.clone(),
                self.current_value.into(),
                self.lazy,
            )?;

            // On drag end, reset lazy timer to ensure final state is captured
            if response.drag_stopped() && self.lazy {
                self.manager.reset_lazy_undo();
            }

            Ok(update_type)
        } else {
            Ok(UpdateType::None)
        }
    }
}

// Convenience macro for common pattern
#[macro_export]
macro_rules! config_slider {
    ($ui:expr, $manager:expr, $path:expr, $range:expr, $label:expr) => {
        ConfigSlider::new($manager, $path, $range, $label)?.lazy(true).show($ui)?
    };
}
```

**Usage in UI code**:
```rust
// Old way (with flags):
if ui.add(egui::Slider::new(exposure, 0.1..=5.0).text("Exposure")).changed() {
    *exposure_changed = true;
}

// New way (declarative):
let update = ConfigSlider::new(config_manager, ConfigPath::Exposure, 0.1..=5.0, "Exposure")?
    .lazy(true)
    .show(ui)?;

// Or with macro:
let update = config_slider!(ui, config_manager, ConfigPath::Exposure, 0.1..=5.0, "Exposure");

// Then handle update type
match update {
    UpdateType::ToneMappingOnly => {
        // Re-run tonemap pass
    }
    UpdateType::IterationReset => {
        // Clear accumulation, restart iterations
    }
    _ => {}
}
```

### 7. Application Integration

**Purpose**: Wire ConfigManager into main app loop

```rust
// In App struct:
pub struct App {
    // ... existing fields ...
    config_manager: ConfigManager,

    // Remove all these:
    // flame_changed: bool,
    // view_changed: bool,
    // exposure_changed: bool,
    // etc. (ALL the flags go away!)
}

impl App {
    pub fn new(...) -> Self {
        let config = FractalConfig::default();
        let config_manager = ConfigManager::new(config);

        Self {
            // ...
            config_manager,
        }
    }

    /// Main render loop
    pub fn render(&mut self, window: &Window) {
        // UI returns what kind of update is needed
        let update_type = self.ui_layer.render(
            window,
            &mut self.config_manager,  // Pass manager, not individual params
        );

        // Handle update based on type
        match update_type {
            UpdateType::None => {
                // No changes - do nothing
            }
            UpdateType::ViewOnly => {
                // Just update view transform (cheap)
                self.update_view_transform();
            }
            UpdateType::ToneMappingOnly => {
                // Re-run tonemap pass
                self.flame_renderer.update_tonemap_params(self.config_manager.config());
            }
            UpdateType::ColorOnly => {
                // Re-run color accumulation
                self.flame_renderer.update_color_params(self.config_manager.config());
            }
            UpdateType::IterationReset => {
                // Full reset
                self.flame_renderer.load_config(self.config_manager.config());
                self.flame_renderer.reset();
            }
        }

        // Render frame
        // ...
    }

    /// Handle keyboard shortcuts
    fn handle_keyboard(&mut self, input: &KeyboardInput) {
        match input.logical_key {
            Key::Character("z") if input.modifiers.control() => {
                // Undo
                if let Ok(update_type) = self.config_manager.undo() {
                    self.handle_update(update_type);
                }
            }
            Key::Character("y") if input.modifiers.control() => {
                // Redo
                if let Ok(update_type) = self.config_manager.redo() {
                    self.handle_update(update_type);
                }
            }
            _ => {}
        }
    }
}
```

**UI layer changes**:
```rust
impl EguiLayer {
    pub fn render(
        &mut self,
        window: &Window,
        config_manager: &mut ConfigManager,
    ) -> UpdateType {
        let mut max_update = UpdateType::None;

        // Each window returns its update type
        let update = self.render_tone_mapping_window(ctx, config_manager);
        max_update = max_update.merge(update);

        let update = self.render_view_window(ctx, config_manager);
        max_update = max_update.merge(update);

        // etc...

        max_update
    }
}
```

## Migration Plan

### Phase 1: Foundation (Week 1)
**Goal**: Build core delta system without breaking existing code

**Tasks**:
1. Create `src/config/delta.rs` module with:
   - `ConfigPath` enum
   - `ConfigValue` enum
   - `ConfigDelta` struct
   - `ConfigChange` struct
   - `UpdateType` enum
2. Create `src/config/manager.rs` with `ConfigManager`
3. Write unit tests for:
   - Delta creation/inversion
   - Value get/set for all paths
   - Undo/redo logic
   - Lazy throttling
4. Keep existing flag system working

**Deliverable**: Core delta system tested and working, old system unchanged

### Phase 2: Slider Binding (Week 1-2)
**Goal**: Create declarative slider API

**Tasks**:
1. Implement `ConfigSlider` builder
2. Create `config_slider!` macro
3. Test with a few simple sliders (exposure, gamma)
4. Verify undo/redo works correctly
5. Verify lazy throttling works (no extra undo points)

**Deliverable**: Working slider binding system, proven with 2-3 sliders

### Phase 3: Migrate Tone Mapping Window (Week 2)
**Goal**: Fully convert one window as proof-of-concept

**Tasks**:
1. Convert all tone mapping sliders to `ConfigSlider`
2. Convert tone curve editor to delta system
3. Remove all `*_changed` flags from tone mapping
4. Update app.rs to handle `UpdateType` from tone mapping window
5. Thorough testing:
   - Slider changes work
   - Undo/redo works
   - Lazy undo throttles correctly
   - No extra undo points
   - Correct update types triggered

**Deliverable**: Tone Mapping window fully on new system, old flags removed

### Phase 4: Migrate Remaining Windows (Week 2-3)
**Goal**: Convert all UI to delta system

**Tasks**:
1. Convert View window (zoom, pan, rotation, camera)
2. Convert Settings window (iterations, blend, etc.)
3. Convert Transforms window (affine, variations, colors)
4. Convert Triangle Editor (special case - batch updates)
5. Remove ALL `*_changed` flags from codebase
6. Update app.rs to only use `UpdateType`

**Deliverable**: Entire UI on delta system, flag-based code deleted

### Phase 5: Undo/Redo Window (Week 3-4)
**Goal**: Add UI to show undo history

**Tasks**:
1. Create undo history window
2. Display list of `ConfigChange` descriptions
3. Allow clicking to undo/redo to specific point
4. Show what will be undone/redone on hover
5. Polish UX (keyboard shortcuts, tooltips, etc.)

**Deliverable**: Full undo/redo window with history visualization

### Phase 6: Optimization & Polish (Week 4)
**Goal**: Performance tuning and edge cases

**Tasks**:
1. Profile delta system overhead
2. Optimize hot paths if needed
3. Handle edge cases:
   - Loading presets (batch update)
   - Import/export (should undo be cleared?)
   - Reset button (batch update)
4. Update documentation
5. Final testing

**Deliverable**: Production-ready delta-based state management

## Benefits

### Immediate (Phase 3+)
1. ✅ **No more flag spaghetti** - all `*_changed` flags gone
2. ✅ **Correct lazy undo** - single throttle point, no cross-talk
3. ✅ **Smart updates** - only recompute what's needed
4. ✅ **Cleaner code** - declarative sliders, centralized logic

### Medium-term (Phase 5+)
5. ✅ **Better UX** - undo window shows what changed
6. ✅ **Easier debugging** - can see exact delta history
7. ✅ **Foundation for features**:
   - Animation (interpolate between deltas)
   - Macros/scripting (record/replay deltas)
   - Collaborative editing (sync deltas)

### Long-term
8. ✅ **Maintainability** - adding new parameters is trivial
9. ✅ **Testability** - can unit test config changes in isolation
10. ✅ **Performance** - can batch/dedupe deltas

## Implementation Notes

### Delta Storage Reconstruction

**Concern**: Undo/redo requires reconstructing config from deltas

**Solution**: Reconstruct from current state backwards:

```rust
impl ConfigManager {
    /// Reconstruct config at specific undo depth
    fn reconstruct_at_depth(&self, depth: usize) -> FractalConfig {
        let mut config = self.current.clone();

        // Apply inverted deltas from most recent to target depth
        for i in (depth..self.undo_stack.len()).rev() {
            let change = &self.undo_stack[i];
            for delta in change.deltas.iter().rev() {
                let inverted = delta.invert();
                // Apply inverted delta to config
                self.apply_delta(&mut config, &inverted);
            }
        }

        config
    }
}
```

**Performance**:
- Worst case: 50 undo steps * 10 deltas/step = 500 operations
- Each operation is a simple field assignment
- Total time: < 1ms (negligible)

**Alternative**: Periodic snapshots every 10 steps
- Reduces reconstruction to max 10 deltas
- Trades memory for speed
- Not needed unless profiling shows it's slow

### Float Comparison

**Concern**: Floating point rounding might cause false change detection

**Solution**: Use epsilon comparison in `ConfigValue::approx_eq()`:
```rust
const EPSILON: f32 = 1e-6;

pub fn approx_eq(&self, other: &Self) -> bool {
    match (self, other) {
        (ConfigValue::Float(a), ConfigValue::Float(b)) => {
            (a - b).abs() < EPSILON
        }
        (ConfigValue::ColorRgb(a), ConfigValue::ColorRgb(b)) => {
            a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < EPSILON)
        }
        _ => self == other
    }
}
```

### Batch Updates

**Use cases**:
- Load preset → entire config changes
- RGB color picker → r, g, b change together
- Reset view → zoom, pan_x, pan_y, rotation reset

**Pattern**:
```rust
// Reset view
config_manager.update_batch(
    vec![
        (ConfigPath::Zoom, 1.0.into()),
        (ConfigPath::PanX, 0.0.into()),
        (ConfigPath::PanY, 0.0.into()),
        (ConfigPath::Rotation, 0.0.into()),
    ],
    "Reset View".to_string(),
    false,  // Not lazy - always capture
)?;

// Load preset
config_manager.update_batch(
    preset_to_deltas(&preset),
    format!("Load preset: {}", preset.name),
    false,
)?;
```

### Display Formatting

**ConfigValue Display**:
```rust
impl Display for ConfigValue {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ConfigValue::Float(v) => write!(f, "{:.3}", v),
            ConfigValue::Int(v) => write!(f, "{}", v),
            ConfigValue::Bool(v) => write!(f, "{}", v),
            ConfigValue::ColorRgb([r, g, b]) => {
                write!(f, "RGB({:.2}, {:.2}, {:.2})", r, g, b)
            }
            ConfigValue::ToneMapMode(m) => write!(f, "{:?}", m),
            // etc.
        }
    }
}
```

**Undo Window Display**:
```
Undo History:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[0] Load preset: Sierpinski (12 changes)
[1] Transform 1 → Linear: 0.500 → 0.800
[2] Exposure: 1.000 → 0.400
[3] Zoom: 1.000 → 2.500
[4] Transform 1 → Affine a: 0.500 → 0.750
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
                                    ↑ Current
```

## Testing Strategy

### Unit Tests
- Delta creation/inversion correctness
- Value get/set for all ConfigPath variants
- Lazy undo throttling timing
- Batch update merging
- UpdateType classification

### Integration Tests
- Full undo/redo cycle preserves state
- Lazy undo creates correct number of points
- Batch updates create single undo point
- UpdateType triggers correct renderer operations

### Manual Testing
- Drag slider quickly → max 2-3 undo points
- Undo/redo → state restored exactly
- Load preset → single undo point, can undo entire load
- Performance → no lag when changing parameters

## Risks & Mitigations

### Risk 1: Delta reconstruction performance
**Impact**: Medium
**Likelihood**: Low
**Mitigation**: Profile early, add snapshots if needed

### Risk 2: Missing ConfigPath variants
**Impact**: High (crashes)
**Likelihood**: Medium
**Mitigation**: Comprehensive tests, use `todo!()` for unimplemented paths during migration

### Risk 3: Migration timeline too aggressive
**Impact**: Medium
**Likelihood**: Medium
**Mitigation**: Do Phase 3 thoroughly before proceeding, adjust timeline as needed

### Risk 4: Lazy undo still not working right
**Impact**: High (defeats purpose)
**Likelihood**: Low (much simpler than current approach)
**Mitigation**: Phase 2 focuses entirely on getting this right with test cases

## Success Criteria

### Must Have (MVP)
- ✅ All UI uses ConfigManager (no flags)
- ✅ Undo/redo works correctly
- ✅ Lazy undo throttles to ~500ms (no extra points)
- ✅ Correct update types trigger correct renderer operations
- ✅ No performance regression

### Should Have
- ✅ Undo window shows history
- ✅ Can undo/redo to specific point
- ✅ Batch updates work (preset load, reset view)

### Nice to Have
- ⚪ Undo across app restarts (persist undo stack)
- ⚪ Undo preview (show what will change on hover)
- ⚪ Configurable throttle duration

## Open Questions

1. **Preset loading**: Should loading a preset clear undo history? Or create a single "Load preset: X" undo point?
   - **Recommendation**: Single undo point - allows undoing preset load

2. **Import config**: Should importing a .fflame file clear undo history?
   - **Recommendation**: Yes - it's a "new document" conceptually

3. **Undo window placement**: Separate window or panel in main UI?
   - **Recommendation**: Separate window (like other panels)

4. **Drag-end guarantee**: Should drag end ALWAYS create undo point, even if < 500ms since last?
   - **Recommendation**: Yes - call `reset_lazy_undo()` on drag end to force capture

## References

- Current undo system: [src/app/config.rs](../../src/app/config.rs)
- Lazy undo attempt: [docs/projects/lazy-undo-implementation.md](./lazy-undo-implementation.md)
- FractalConfig definition: [src/config.rs](../../src/config.rs)
- UI response flags: [src/ui/response.rs](../../src/ui/response.rs)
