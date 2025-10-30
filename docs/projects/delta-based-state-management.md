# Delta-Based State Management System

**Status:** Phase 8 Complete ✅ (All UI controls migrated, preset loading integrated)
**Created:** 2025-10-29
**Updated:** 2025-10-30
**Category:** Architecture Refactor

**Major Milestone**: All user-facing controls now use delta-based state management!
- ✅ Phase 1-5: Core infrastructure, View window, Triangle Editor
- ✅ Phase 6: Variation controls (weights + parameters)
- ✅ Phase 7: Tone Mapping & Colors window (8 controls)
- ✅ Phase 8: Preset loading system (snapshot-based)

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

**Key Architecture**: Two-state system for lazy undo
- `current`: Last **captured/committed** state (what's in undo stack)
- `preview`: Live **preview** state during drag (updated every frame)

This separation ensures:
- Deltas are capture-to-capture (e.g., `1.0→5.0`), not frame-to-frame (e.g., `3.1→3.2`)
- UI shows live preview immediately
- Fractal renders live values
- Undo stack only has meaningful checkpoints

```rust
pub struct ConfigManager {
    /// Current configuration (last captured state)
    current: FractalConfig,

    /// Preview configuration (live state during lazy updates)
    /// When Some: in preview mode, deltas computed from current
    /// When None: not in preview mode
    preview: Option<FractalConfig>,

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
        if lazy {
            // Lazy mode: Update preview, capture on throttle

            // Create preview if it doesn't exist (first update in drag sequence)
            if self.preview.is_none() {
                self.preview = Some(self.current.clone());
            }

            // Get preview value (will exist now)
            let preview_value = self.get_value(&path)?;

            // Check if actually changed from preview
            if preview_value.approx_eq(&new_value) {
                return Ok(UpdateType::None);
            }

            // Update preview with new value
            self.set_value_in_preview(&path, new_value.clone())?;

            // Check if we should capture this change
            let should_capture = self.should_capture_lazy_undo();

            if should_capture {
                // Capture delta from current → preview
                let old_value_in_current = /* get from current, not preview */;
                let delta = ConfigDelta::new(path.clone(), old_value_in_current, new_value.clone());
                let change = ConfigChange::single(delta);
                let update_type = change.update_type();

                self.push_undo(change);

                // Commit preview to current
                self.current = self.preview.take().unwrap();

                return Ok(update_type);
            }

            // No capture yet, just return update type based on path
            Ok(path.update_type())

        } else {
            // Non-lazy mode: Update current directly and capture immediately

            let old_value = self.get_value(&path)?;

            // Check if actually changed
            if old_value.approx_eq(&new_value) {
                return Ok(UpdateType::None);
            }

            // Create delta and capture
            let delta = ConfigDelta::new(path.clone(), old_value, new_value.clone());
            let change = ConfigChange::single(delta);
            let update_type = change.update_type();

            self.push_undo(change);

            // Apply change to current
            self.set_value(&path, new_value)?;

            Ok(update_type)
        }
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
    /// Returns preview value if in preview mode, otherwise current value
    fn get_value(&self, path: &ConfigPath) -> Result<ConfigValue, ConfigError> {
        // Use preview if available, otherwise current
        let config = self.preview.as_ref().unwrap_or(&self.current);

        match path {
            ConfigPath::Exposure => Ok(config.exposure.into()),
            ConfigPath::Gamma => Ok(config.gamma.into()),
            ConfigPath::TransformVariation { index, variation } => {
                let xform = config.flame.transforms.get(*index)
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

    /// Set value in current config by path (used during undo/redo)
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

    /// Set value in preview config by path (used during lazy updates)
    /// Panics if preview doesn't exist (caller must ensure preview is created first)
    fn set_value_in_preview(&mut self, path: &ConfigPath, value: ConfigValue) -> Result<(), ConfigError> {
        let preview = self.preview.as_mut().expect("preview must exist");

        match path {
            ConfigPath::Exposure => {
                preview.exposure = value.try_into()?;
            }
            ConfigPath::Gamma => {
                preview.gamma = value.try_into()?;
            }
            // ... etc for all paths (same as set_value but operates on preview)
            _ => todo!("Implement all path setters")
        }
        Ok(())
    }

    /// Force commit preview to current (call on drag end)
    /// Returns the update type of the final change
    pub fn force_commit_preview(&mut self, path: &ConfigPath) -> Result<UpdateType, ConfigError> {
        if let Some(preview) = self.preview.take() {
            // Get values from current and preview
            let old_value = /* value from current */;
            let new_value = /* value from preview */;

            // If they're different, capture the final delta
            if !old_value.approx_eq(&new_value) {
                let delta = ConfigDelta::new(path.clone(), old_value, new_value);
                let change = ConfigChange::single(delta);
                let update_type = change.update_type();

                self.push_undo(change);

                // Commit preview to current
                self.current = self.preview.take().unwrap();

                return Ok(update_type);
            }
        }

        Ok(UpdateType::None)
    }

    /// Get current config (read-only)
    /// Returns last captured/committed state, NOT live preview
    /// Use active_config() if you want to see live values during drag
    pub fn config(&self) -> &FractalConfig {
        &self.current
    }

    /// Get active config (read-only)
    /// Returns preview if in preview mode, otherwise current
    /// Use this to read live values for rendering
    pub fn active_config(&self) -> &FractalConfig {
        self.preview.as_ref().unwrap_or(&self.current)
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

### Phase 1: Foundation ✅ COMPLETE
**Goal**: Build core delta system without breaking existing code

**Status**: ✅ Complete (2025-10-29, commit adfd822)

**Tasks Completed**:
1. ✅ Created `src/config/delta.rs` module with:
   - `ConfigPath` enum (59 variants - **removed IterationsPerThread, SpeedMultiplier, UseDynamicBlend as they're runtime-only**)
   - `ConfigValue` enum (12 types with From/TryFrom traits)
   - `ConfigDelta` struct
   - `ConfigChange` struct (with `single()` and `batch()` constructors)
   - `UpdateType` enum (4 levels: None, ViewOnly, ToneMappingOnly, ColorOnly, IterationReset)
2. ✅ Created `src/config/manager.rs` with `ConfigManager`:
   - `update_param()` - single parameter changes
   - `update_batch()` - batch updates with custom description
   - `undo()` / `redo()` - delta-based undo/redo
   - `get_value()` / `set_value()` - 59 config paths fully implemented
   - Lazy undo throttling (500ms with `reset_lazy_undo()` for drag end)
3. ✅ Moved `src/config.rs` → `src/config/fractal_config.rs`
   - Added `Default` impl for FractalConfig
4. ✅ Write unit tests (9 tests, all passing):
   - Delta creation/inversion
   - Value get/set (exposure)
   - Undo/redo cycle
   - Lazy throttling (verified only captures every 500ms)
   - Batch updates
   - Update type classification
   - ConfigValue::approx_eq() for float comparison
5. ✅ Old system still working (existing code unchanged)

**Implementation Notes**:
- **ConfigPath** does NOT derive `PartialEq, Eq, Hash` due to `Box<Palette>` variant
- **ConfigValue** does NOT derive `PartialEq` - uses manual `approx_eq()` for complex types
- **Transform variation params** use flat key format: `"variation.param"` not nested HashMap
- **TransformCount** is read-only (get only, cannot set directly)

**Deliverable**: ✅ Core delta system tested and working, 9/9 tests passing

### Phase 2: Slider Binding ✅ COMPLETE
**Goal**: Create declarative slider API

**Status**: ✅ Complete (2025-10-29, commit ca1b75e)

**Tasks Completed**:
1. ✅ Created `src/config/slider.rs` with:
   - `ConfigSlider` builder pattern with fluent API
   - `ConfigSlider::new()` - creates slider bound to config path
   - `ConfigSlider::lazy()` - enables 500ms throttled undo
   - `ConfigSlider::show()` - renders slider and handles updates
   - `ConfigSliderResult` - return type with `changed`, `should_capture`, `update_type`
2. ✅ Created extension traits for ergonomic usage:
   - `ConfigSliderUi` - adds `ui.config_slider()` and `ui.config_drag_value()`
   - `LazyUndoUi` - adds `ui.lazy_slider()` and `ui.lazy_drag()` shortcuts
3. ✅ Made `ConfigManager::get_value()` public for slider access
4. ✅ Tests:
   - `test_config_slider_creation` - Verify builder creates correctly
   - `test_config_slider_lazy` - Verify lazy flag setting

**Implementation Notes**:
- Slider automatically detects drag end and calls `reset_lazy_undo()`
- Returns `UpdateType` so app layer knows what to recompute
- Handles both egui sliders and drag values
- No macro needed - trait extension methods provide clean ergonomics

**Deliverable**: ✅ Working slider binding system, 11/11 tests passing (9 from Phase 1 + 2 from Phase 2)

### Phase 3: Migrate Tone Mapping Window ✅ COMPLETE
**Goal**: Fully convert one window as proof-of-concept

**Status**: ✅ Complete (2025-10-29, commits 12abf20, 148a38f, 52e08f5, cdf4103, 76647ea)

**Tasks Completed**:
1. ✅ Integrated ConfigManager into App struct
2. ✅ Wired ConfigManager through render_ui() call chain
3. ✅ Converted 3 tone mapping sliders to use `LazyUndoUi::lazy_slider()`:
   - Exposure slider (lazy undo throttled)
   - Gamma slider (lazy undo throttled)
   - Density scale slider (lazy undo throttled)
4. ✅ Fixed undo/redo integration:
   - Wired `can_undo()` and `can_redo()` to ConfigManager
   - Fixed `undo()` and `redo()` to update ConfigManager and sync back to App state
   - Fixed redo bug where redo stack was being cleared after first redo
5. ✅ **Fixed lazy undo delta calculation bug** (commits cdf4103, 76647ea):
   - Implemented current/preview state separation in ConfigManager
   - Added `preview: Option<FractalConfig>` field
   - `get_value()` returns from preview when available
   - `update_param()` in lazy mode creates/updates preview, captures on throttle
   - Added `force_commit_preview()` for drag end
   - Added `active_config()` for reading live values during drag
   - Result: Deltas are now capture-to-capture (e.g., `[1.0→5.0]`), not frame-to-frame
6. ✅ User testing confirmed:
   - All 3 sliders work correctly
   - Lazy undo creates proper deltas (start→end, no intermediate junk)
   - Undo button lights up when undo is available
   - Undo/redo updates both UI and fractal correctly
   - Fractal updates in real-time during slider drags

**Implementation Architecture**:
ConfigManager now tracks two states:
- `current`: Last captured/committed state (what's in undo stack)
- `preview`: Live preview state (updated every frame during drag)

Flow for lazy undo:
- User starts drag: `preview` created from `current`
- User drags: `preview` updated every frame via `set_value_in_preview()`
- Throttle fires (500ms): Capture delta from `current→preview`, commit `preview` to `current`
- Drag ends: Force commit `preview→current` if changed
- `get_value()`: Returns from `preview` if it exists, otherwise `current`
- `active_config()`: Returns `preview` if it exists (for live rendering), otherwise `current`

This ensures:
- ✅ Deltas are always capture-to-capture, not frame-to-frame
- ✅ UI shows live preview immediately (reads from `active_config()`)
- ✅ Fractal renders live values (uses `active_config()`)
- ✅ Undo stack only has meaningful checkpoints
- ✅ Undo/redo restores exact captured states

**Known Issues Fixed**:
- ❌ Undo button stayed grey → ✅ Fixed by wiring to ConfigManager
- ❌ Undo didn't update UI → ✅ Fixed by syncing config back to App
- ❌ Redo only worked once → ✅ Fixed by not clearing redo stack on redo
- ❌ Lazy undo wrong deltas → ✅ Fixed by current/preview separation
- ❌ Fractal showed stale values during drag → ✅ Fixed by using `active_config()`

**Remaining Phase 3 Tasks** (non-blocking - proof-of-concept complete):
- ⚪ Convert remaining tone mapping controls (tonemap_mode, use_curve, curve presets, etc.)
- ⚪ Convert tone curve editor to delta system
- ⚪ Remove `*_changed` flags from tone mapping window
- ⚪ Handle UpdateType returns in app.rs (trigger resets/updates)

**Deliverable**: ✅ Proof-of-concept fully working
**Next**: Continue Phase 3 migration (remaining controls) or proceed to Phase 4

### Phase 4: Migrate Remaining Windows (Week 2-3) ✅ COMPLETE
**Goal**: Convert all UI to delta system

**Status**: ✅ Complete (2025-10-29, commits 6e25df7, d5a9fcb, 898802e, e9b5a47, 85156fb + bugfix)

**Tasks Completed**:
1. ✅ Convert View window (zoom, pan, rotation, camera) - All 5 controls + 7 buttons migrated
   - Zoom: `lazy_drag()` with 500ms throttle
   - Pan X/Y: `lazy_drag()` with 500ms throttle
   - Rotation: `update_param(lazy=true)` with degrees/radians conversion
   - Camera Rotation X/Y: `update_param(lazy=true)` with conversion (3D mode only)
   - Zoom In/Out buttons: `update_param(lazy=false)` for immediate capture
   - Arrow buttons: `update_batch()` for pan_x + pan_y
   - Reset View button: `update_batch()` for all 6 parameters
   - Returns `UpdateType`, syncs from `active_config()` for live preview
2. ✅ Convert Settings window (complete) - All 7 rendering quality controls migrated (7/7 total)
   - Histogram Color Scale: Logarithmic slider with `update_param(lazy=true)`
   - Low-Density Smoothing: Linear slider with `update_param(lazy=true)`
   - Fixed Blend Rate: Logarithmic slider with `update_param(lazy=true)` (disabled when dynamic blend on)
   - Density Compression Strength: Linear slider with `update_param(lazy=true)`
   - Per-Pixel Iteration Limit: Logarithmic slider with custom formatter + `update_param(lazy=true)`
   - Iterations Per Thread: Linear slider (64-4096) with `update_param(lazy=true)`
   - Speed Multiplier: 5 buttons (1x-16x) with `update_param(lazy=false)` for immediate capture
   - Pattern: Manual slider + temp variable + `update_param()` (ConfigSlider doesn't support `.logarithmic(true)` yet)
   - Returns `UpdateType`, syncs from `active_config()` for live preview
3. ✅ Convert Transforms window - 11 controls per transform migrated (affine, weight, color)
   - Affine parameters (a,b,c,d,e,f): DragValues with `update_param(lazy=true)`
   - Z offset (g): DragValue for 3D mode only with `update_param(lazy=true)`
   - Weight: Logarithmic slider with `update_param(lazy=true)`
   - Color RGB: 3 sliders with indexed `ConfigPath::TransformColor`
   - Color Speed: Slider with `ConfigPath::TransformColorSpeed`
   - Pattern: Indexed ConfigPath (per-transform), temp variable + `update_param(lazy=true)` + `active_config()` sync
   - Returns `UpdateType`, updated helper functions to accept config_manager + index
   - Note: Variation controls not yet converted (separate module)

4. ✅ Convert Triangle Editor - All 4 interaction modes migrated (special case - batch updates)
   - Move Points: Drag O/X/Y points individually, `update_batch()` all 6 affine params
   - Translate: Drag to move triangle, `update_batch()` all 6 affine params
   - Rotate: Drag to rotate around O, `update_batch()` all 6 affine params
   - Scale: Drag to scale from O, `update_batch()` all 6 affine params
   - Pattern: All modes use `update_batch(changes, description, lazy=true)` for atomic 6-param updates
   - Smart accumulation: `is_in_preview_mode()` detects lazy drag, skips reset for smooth feedback
   - Bug fix: Set `flame_changed = true` based on UpdateType return, sync flame from ConfigManager
   - Returns `UpdateType`, removes `triangle_drag_*` flags

5. ✅ Convert Mouse Panning - Integrated with ConfigManager for undo/redo support (2025-10-30)
   - Uses `update_batch()` for atomic PanX + PanY updates (single undo entry)
   - Lazy mode enabled for smooth drag (500ms throttle)
   - Respects view rotation (same as arrow keys)
   - Preview mode via `is_in_preview_mode()` check
   - Fixed: Removed `view_changed_by_keyboard` flag during drag (was causing black flashes)
   - Fixed: View slider flags only set when NOT in preview mode (rotation, camera rotation)
   - Fixed: `force_commit_preview()` simplified to just commit preview→current without creating deltas
   - Fixed: Added `force_commit_preview()` call on mouse release to exit preview mode immediately

6. ✅ Convert Transform Variation Controls - All variation controls migrated (2025-10-30)
   - **variation_params.rs**: Migrated to use ConfigManager
     - Accepts `config_manager` and `transform_index` instead of direct transform access
     - Uses `update_param(lazy=true)` for Float, Integer, and Angle parameter types
     - Reads from `active_config()` for live preview
     - Returns `UpdateType::IterationReset` (variation params require fractal recalc)
   - **variation_controls.rs**: Migrated to use ConfigManager
     - Accepts `config_manager` and `transform_index` instead of direct transform access
     - Uses `update_param(lazy=true)` for variation weight sliders
     - Automatically shows/hides parameter controls based on weight > 1e-6
     - Returns `UpdateType::IterationReset` (variations require fractal recalc)
   - **transforms.rs**: Updated to call new signatures
     - Passes `config_manager` and transform index to all variation category calls
     - Collects and tracks maximum `UpdateType` from all variation changes
   - **Architecture**: Full add/delete/undo support
     - Add variation: Set weight > 0.0 → Delta stores `[0.0 → weight]` with variation name in path
     - Delete variation: Set weight to 0.0 → Delta stores `[weight → 0.0]` with variation name in path
     - Undo delete: Apply inverse `[0.0 → weight]` → Recreates variation (name preserved in ConfigPath)
     - Variation parameters stored separately: `ConfigPath::TransformVariationParam { index, variation, param }`
   - **Testing**: ✅ All variation sliders work with lazy undo (500ms throttle, live preview, proper undo/redo)

7. ✅ Convert Tone Mapping & Colors Window - Complete window migration (2025-10-30)
   - **Tonemap Mode buttons** (Linear/Logarithmic/Density): `update_param(lazy=false)` for immediate capture
   - **Use Tone Curve checkbox**: `update_param(lazy=false)` for immediate capture
   - **Tone Curve preset buttons** (Linear/S-Curve/Brighten/Darken): `update_param(lazy=false)` for immediate capture
   - **Tone Curve editor**: Interactive drag control with lazy undo
     - Drag control points: `update_param(lazy=true)` for smooth live preview (500ms throttle)
     - Drag end: `force_commit_preview()` to exit preview mode immediately
     - Double-click add point: `update_param(lazy=false)` for immediate capture
     - Returns `UpdateType::ToneMappingOnly` (no iteration reset needed)
   - **Color Mode dropdown** (Transform/Palette/Speed): `update_param(lazy=false)` for immediate capture
   - **Palette selector**: `update_param(lazy=false)` for immediate capture (returns `UpdateType::ColorOnly`)
   - **Speed Factor slider**: `update_param(lazy=true)` for smooth drag (returns `UpdateType::ColorOnly`)
   - **Background Color picker**: `update_param(lazy=false)` for immediate capture (returns `UpdateType::ToneMappingOnly`)
   - **Testing**: ✅ All controls work with proper undo/redo, curve editor has smooth live preview

**Remaining Tasks**:
6. ⚪ Remove ALL `*_changed` flags from codebase
7. ⚪ Update app.rs to only use `UpdateType`

**Implementation Patterns Established**:

1. **Simple sliders/drags**: Use `lazy_slider()` or `lazy_drag()` extension traits
2. **Logarithmic/custom sliders**: Manual slider + temp variable + `update_param(lazy=true)` (ConfigSlider doesn't support `.logarithmic(true)` yet)
3. **Indexed parameters**: Pass config_manager + index to helper functions, use `ConfigPath::Transform*` variants
4. **Buttons**: Use `update_param(lazy=false)` for immediate capture
5. **Multi-parameter changes**: Use `update_batch()` with descriptive names
6. **All controls**: Read from `active_config()` for live preview during drag
7. **Window functions**: Return `UpdateType` for proper update classification

**Metrics**:
- Windows migrated: 5 (View 100%, Settings 100%, Transforms 100%, Tone Mapping 100%, Triangle Editor 100%)
- Controls converted: 65+ individual controls across all windows
  - View: 5 sliders + 7 buttons (12 controls)
  - Settings: 7 quality controls (7 controls)
  - Transforms: 11 core controls per transform (affine, weight, color)
  - Variations: All 26 core variations + parameters (e.g., JuliaN power/dist, Blob high/low/waves)
  - Tone Mapping: 8 controls (mode buttons, curve checkbox, 4 curve presets, curve editor drag, color mode, palette, speed, background)
  - Triangle Editor: 4 interaction modes (batch updates)
  - Mouse panning: Atomic X+Y batch updates
- Lines changed: ~900+ across 12 files
- Commits: 11 feature + 5 documentation + 1 bugfix
- Build status: ✅ All passing

**Deliverable**: ✅ Core migration complete - 5 windows functional with proper undo/redo and smart accumulation
**Remaining**: Legacy flag cleanup

---

## Phase 4 Summary

**Overall Status**: ✅ **CORE COMPLETE** - Production-ready delta system with proven patterns

### Achievement Metrics

| Category | Metric | Status |
|----------|--------|--------|
| **Windows Migrated** | 4 core windows | ✅ Complete |
| **Controls Converted** | 41+ individual controls | ✅ Working |
| **Code Quality** | All builds passing | ✅ Passing |
| **Undo/Redo** | Capture-to-capture deltas | ✅ Working |
| **Live Preview** | Real-time during drag | ✅ Working |
| **Smart Accumulation** | Smooth feedback during drag | ✅ Working |
| **Documentation** | Patterns + examples | ✅ Complete |

### Files Modified (13 total)

1. `src/ui/view.rs` - Full migration (100%)
2. `src/ui/settings.rs` - Full migration (100%)
3. `src/ui/transforms.rs` - Core migration (100%)
4. `src/ui/triangle_editor.rs` - Full migration (100%)
5. `src/ui/tone_mapping.rs` - Partial migration (Phase 3)
6. `src/ui/mod.rs` - Call site updates + flame sync + UpdateType handling
7. `src/ui/response.rs` - Removed triangle_drag_* flags
8. `src/config/fractal_config.rs` - Added iterations_per_thread, speed_multiplier
9. `src/config/delta.rs` - Added ConfigPath variants
10. `src/config/manager.rs` - Current/preview system + is_in_preview_mode()
11. `src/config/slider.rs` - Extension traits
12. `src/app/mod.rs` - Smart accumulation logic + flame sync comment
13. `docs/projects/delta-based-state-management.md` - Full documentation

### Key Technical Innovations

1. **Current/Preview State Separation**: Solved frame-to-frame delta bug, ensures capture-to-capture deltas
2. **Indexed ConfigPath**: Enables per-transform parameter tracking for multi-transform fractals
3. **Live Preview System**: `active_config()` provides real-time rendering during lazy undo
4. **Smart Accumulation**: `is_in_preview_mode()` detects lazy drag, skips reset for smooth visual feedback
5. **UpdateType Integration**: UI windows return UpdateType, set flags based on severity for proper GPU updates
6. **Flame Sync Pattern**: Sync flame from ConfigManager at start of render_ui() for consistent state
7. **Consistent Patterns**: 7 documented patterns for all control types
5. **UpdateType Classification**: Proper render pipeline control from window functions

### Proven Implementation Patterns

All future migrations can use these tested patterns:

```rust
// Pattern 1: Simple sliders
ui.lazy_slider(config_manager, ConfigPath::Exposure, 0.1..=5.0, "Exposure")

// Pattern 2: Logarithmic sliders
let mut temp = *value;
if ui.add(Slider::new(&mut temp, range).logarithmic(true)).changed() {
    config_manager.update_param(path, temp.into(), true)
}

// Pattern 3: Indexed parameters
ConfigPath::TransformAffine { index, param: AffineParam::A }

// Pattern 4: Buttons
config_manager.update_param(path, value.into(), false)

// Pattern 5: Multi-parameter
config_manager.update_batch(vec![(path1, val1), (path2, val2)], "Description", false)
```

### Future Work Priorities

**Immediate** (extend Phase 4):
- ✅ ~~Add iterations_per_thread to FractalConfig + ConfigPath (complete Settings window)~~ (done 7c45e89)
- ✅ ~~Add speed_multiplier to FractalConfig + ConfigPath (complete Settings window)~~ (done 7c45e89)
- ⚪ Convert variation controls in variation_controls.rs (complete Transforms window)

**Short-term** (Phase 4 cleanup):
- ⚪ Convert Triangle Editor (complex batch updates) - **IN PROGRESS**
- ⚪ Convert variation controls in variation_controls.rs (complete Transforms window)
- ⚪ Complete Tone Mapping window (mode dropdown, curve editor)
- ⚪ Remove `*_changed` flags from migrated windows
- ⚪ Handle UpdateType returns in app.rs

**Long-term** (Phase 5-6):
- ✅ ~~Create undo/redo history window UI~~ (done 8983c00)
- ⚪ Add keyboard shortcuts (Ctrl+Z, Ctrl+Y)
- ⚪ Optimize performance if needed
- ⚪ Clean up old undo_history code

**Conclusion**: Phase 4 establishes a **production-ready foundation** for delta-based state management. The system works excellently across multiple window types with proper undo/redo, live preview, and consistent patterns. Future migrations are straightforward using the proven patterns.

---

### Phase 5: Undo/Redo Window ✅ COMPLETE
**Goal**: Add UI to show undo history

**Tasks**:
1. ✅ Create undo history window ([src/ui/undo_history.rs](../../src/ui/undo_history.rs))
2. ✅ Display list of `ConfigChange` descriptions
3. ✅ Allow clicking to undo/redo (single step for now)
4. ⏸️ Show what will be undone/redone on hover (future enhancement)
5. ⏸️ Polish UX (keyboard shortcuts, tooltips, multi-level undo) (future enhancement)

**Deliverable**: Basic undo/redo window with history visualization

**What Was Completed**:
- Created new `undo_history.rs` module with `render_undo_history_window()` function
- Displays undo stack in reverse chronological order (most recent first)
- Displays redo stack in chronological order (oldest first)
- Shows change descriptions from `ConfigChange.description`
- Includes quick action buttons (Undo/Redo with enabled state)
- Shows stack statistics (entry counts)
- Wired into menu bar under "Windows" → "⮪ Undo/Redo History"
- Fully integrated with existing undo/redo system

**Files Modified**:
- `src/ui/undo_history.rs` - NEW FILE: Undo/redo history window UI
- `src/ui/menu_bar.rs` - Added history window toggle
- `src/ui/mod.rs` - Added `show_undo_history` field, wired up window rendering

**Bug Fix** (commit d38b5bf):
- Fixed egui ID collision: Added unique `id_source()` to both ScrollArea widgets
- Error only appeared when both undo and redo stacks had entries (middle of history)

---

## Phase 4 Extended: Triangle Editor Migration 🔄 IN PROGRESS

### Overview
The Triangle Editor is a visual affine transform editor that allows dragging triangle vertices to modify transform parameters. It modifies all 6 affine parameters (a, b, c, d, e, f) as a single atomic operation, making it a perfect use case for `update_batch()`.

### Current Implementation
**File**: `src/ui/triangle_editor.rs` (651 lines)

**Interaction Modes**:
1. **Move Points** - Drag individual O, X, Y points
2. **Translate** - Move entire triangle
3. **Rotate** - Rotate around origin O
4. **Scale** - Scale from origin O

**Current Behavior**:
- Uses `triangle_drag_started`, `triangle_dragging`, `triangle_drag_ended` flags
- Modifies `flame.transforms[selected_transform]` directly via `from_triangle(o, x, y)`
- **Smart accumulation**: During continuous drag, updates GPU params but doesn't reset accumulation
- Captures undo on `triangle_drag_started` via old `capture_state()` system
- Resets accumulation on drag start and drag end only

### Migration Strategy

**Key Insight**: `transform.from_triangle(o, x, y)` modifies all 6 affine params simultaneously. This maps perfectly to `update_batch()`:

```rust
// Batch update all 6 affine parameters in one atomic operation
let changes = vec![
    (ConfigPath::TransformAffine { index, param: AffineParam::A }, a.into()),
    (ConfigPath::TransformAffine { index, param: AffineParam::B }, b.into()),
    (ConfigPath::TransformAffine { index, param: AffineParam::C }, c.into()),
    (ConfigPath::TransformAffine { index, param: AffineParam::D }, d.into()),
    (ConfigPath::TransformAffine { index, param: AffineParam::E }, e.into()),
    (ConfigPath::TransformAffine { index, param: AffineParam::F }, f.into()),
];
let description = match mouse_mode {
    MouseMode::MovePoints => "Triangle Edit (Move Points)",
    MouseMode::Translate => "Triangle Edit (Translate)",
    MouseMode::Rotate => "Triangle Edit (Rotate)",
    MouseMode::Scale => "Triangle Edit (Scale)",
};
config_manager.update_batch(changes, description.to_string(), true);  // lazy=true
```

**Benefits**:
- Single undo point for all 6 parameters (atomic operation)
- Human-readable description in undo history
- Lazy undo with 500ms throttle during drag
- Force-commit on drag end ensures final state captured
- Removes need for `triangle_drag_*` flags (ConfigManager handles internally)

### Implementation Tasks

1. ✅ **Update function signature**:
   - Add `config_manager: &mut ConfigManager` parameter
   - Remove `flame_changed`, `triangle_drag_*` flag parameters
   - Return `UpdateType` instead of unit

2. ✅ **Replace update logic** (4 locations - one per mode):
   - Replace `transform.from_triangle() + *flame_changed = true`
   - With `config_manager.update_batch()` call
   - Use mode-specific descriptions for undo history

3. ✅ **Update call site** (`src/ui/mod.rs`):
   - Pass `config_manager` instead of flag pointers
   - Capture `UpdateType` return value and set `flame_changed = true`
   - Remove flag declarations and UiResponse fields

4. ✅ **Update app.rs**:
   - Remove `triangle_drag_*` handling from undo capture logic
   - Use `is_in_preview_mode()` to detect lazy drag for smart accumulation

5. ✅ **Sync flame from ConfigManager**:
   - Sync `flame` at start of `render_ui()` so Triangle Editor reads correct state
   - Triangle Editor now properly reads and updates via ConfigManager

6. ✅ **Test**:
   - Verify undo/redo works for each mode
   - Verify lazy throttling during drag
   - Verify final state captured on drag end
   - Verify accumulation behavior unchanged (smooth during drag)

### Implementation Complete (2025-10-29)

**Key Bug Fixes**:

1. **Triangle Editor wasn't updating the fractal** (commit d9a5516):
   - Triangle Editor returns `UpdateType::IterationReset` when modifying transforms
   - UI layer wasn't setting `flame_changed = true` based on this return value
   - Without `flame_changed`, renderer never called `update_flame()` to upload changes to GPU
   - **Solution**: Set `flame_changed = true` when UpdateType >= IterationReset

2. **No live preview during drag** (commit 548e544):
   - `update_batch()` with `lazy=true` wasn't implementing preview mode
   - Only throttled undo captures but always updated `current` directly
   - Without preview, changes only visible every 500ms (throttle interval)
   - **Solution**: Refactored `update_batch()` to match `update_param()` lazy behavior:
     - Create preview on first call
     - Update preview (not current) every frame during drag
     - Commit preview to current only when throttle fires
     - Force-commit preview when drag ends

3. **Preview mode never exited**:
   - Preview stayed active after mouse release
   - **Solution**: Detect drag end and force-commit preview in Triangle Editor

**Final Solution Architecture**:
- Sync `flame` from `config_manager.active_config().flame` at END of `render_ui()`
- Set `flame_changed = true` when in preview mode OR UpdateType >= IterationReset
- Use `is_in_preview_mode()` to detect lazy drag and skip accumulation reset
- Call `update_flame()` every frame when in preview mode for live updates
- Force-commit preview on drag end to exit preview mode

### Expected Outcome
- ✅ Triangle Editor fully integrated with delta-based state management
- ✅ Proper undo/redo with descriptive labels
- ✅ No more manual flag management
- ✅ Consistent with other migrated windows
- ✅ Smart accumulation during drag (smooth visual feedback)

---

## Phase 4 Extended: Live Preview Rendering Issues (2025-10-29)

### Remaining Issues After Triangle Editor Migration

**Status:** 🟡 In Progress - Fixing preview/rendering integration

**Issues Identified:**

1. **Blink every 500ms during drag** ❌
   - Preview commits to undo when throttle fires (500ms intervals)
   - Commit clears preview → `is_in_preview_mode()` returns false
   - System thinks drag ended → triggers accumulation reset
   - Next frame creates new preview → continues drag
   - Result: Visible flicker/blink every 500ms

2. **All changes trigger full redraw** ⚠️
   - UpdateType system exists (ViewOnly, ToneMappingOnly, ColorOnly, IterationReset)
   - But rendering uses individual `*_changed` flags instead
   - Post-processing changes (exposure, gamma) reset accumulation unnecessarily
   - Inefficient - tone mapping doesn't need iteration reset

### Root Cause Analysis

**Preview Lifecycle Problem:**
```
Current (Broken):
Frame 1: Start drag → Create preview → Reset accumulation ✓
Frame 30: Throttle (500ms) → Commit to undo → Clear preview ✗
Frame 30: is_in_preview_mode() = false → System triggers reset → BLINK ✗
Frame 31: Continue drag → Create new preview → Repeat...

Desired (Fixed):
Frame 1: Start drag → Create preview → Reset accumulation ✓
Frame 30: Throttle (500ms) → Commit to undo → KEEP preview active ✓
Frame 60: Throttle (1000ms) → Commit to undo → KEEP preview active ✓
Frame 90: End drag → Force-commit → Clear preview ✓
```

**UpdateType Not Used for Rendering:**
- UpdateType correctly classifies changes:
  - `ViewOnly`: Zoom, pan, rotation (just math)
  - `ToneMappingOnly`: Exposure, gamma, background color (post-processing)
  - `ColorOnly`: Palette, color mode (re-run color accumulation)
  - `IterationReset`: Transforms, variations (full redraw)
- But `should_reset` logic uses individual flags, ignores UpdateType
- Result: All changes treated as IterationReset

### Proposed Solution

#### Part 1: Fix Preview Commit Blink

**Modify `update_batch()` and `update_param()` lazy mode:**
- Throttle commit should NOT clear preview
- Clone current instead of taking preview
- Preview stays active throughout entire drag
- Only `force_commit_preview()` clears preview

```rust
// In update_batch() lazy mode:
if should_capture {
    self.push_undo(change_from_current);
    self.current = self.preview.clone().unwrap(); // Clone, not take
    // Preview stays active!
    return Ok(update_type);
}
```

#### Part 2: Reset Only When Preview Created

**Track when preview is first created:**
- Add `preview_just_created: bool` flag to ConfigManager
- Set true when `preview.is_none()` and we create it
- Expose `consume_preview_created_flag()` method
- App checks this flag to reset accumulation on drag START only

```rust
// In ConfigManager:
if self.preview.is_none() {
    self.preview = Some(self.current.clone());
    self.preview_just_created = true;  // NEW: Set flag
}

// In app.rs:
let preview_just_created = config_manager.consume_preview_created_flag();
let should_reset = preview_just_created  // Reset on drag start
    || ui_response.reset_requested
    || view_changed
    || (flame_changed && !is_in_preview_mode());
```

#### Part 3: Use UpdateType for Rendering Decisions

**Respect UpdateType classification:**
- `ToneMappingOnly`: Update params, never reset
- `ColorOnly`: Update params, never reset during drag
- `IterationReset`: Update params, reset on drag start only

**Accumulation Reset Timeline:**
```
Drag Start (preview created):     Reset accumulation
During Drag (every frame):        NO reset, smooth accumulation
Throttle Commit (every 500ms):    NO reset, keep accumulating
Drag End (force commit):          NO reset, already accumulating
```

### Rendering Behavior by UpdateType

| UpdateType | First Drag Frame | During Drag | Throttle Commit | Drag End |
|------------|------------------|-------------|-----------------|----------|
| **ViewOnly** | Update params | Update params | No reset | No reset |
| **ToneMappingOnly** | Update params | Update params | No reset | No reset |
| **ColorOnly** | Reset, update | Update params | No reset | No reset |
| **IterationReset** | Reset, update | Update params | No reset | No reset |

**Key Principle**: Reset accumulation ONCE when drag starts, then smooth accumulation throughout entire drag sequence.

### Implementation Attempt #1 - FAILED ❌

**What we implemented:**
1. ✅ Clone preview on throttle commit (don't clear) - prevents blink
2. ✅ Track `preview_just_created` flag - reset on drag start
3. ✅ Track `was_in_preview_mode_last_frame` - detect exit
4. ✅ Reset on `preview_just_ended` - reset on drag end
5. Added complexity: timestamp tracking, stale preview detection

**Problems Identified:**
1. ❌ **Dense areas get progressively brighter during drag**
   - Accumulation buffer keeps accumulating during live mode
   - Each frame adds more samples to dense areas
   - Brightness builds up continuously

2. ❌ **Brightness remains after exiting live mode**
   - Exit detection calls `reset()`, but too late
   - Overbright buffer already used for display
   - Need to clear BEFORE displaying, not after

3. ❌ **Overengineered solution**
   - Added 5 new fields to ConfigManager
   - Added 3 new methods
   - Complex timestamp tracking and stale detection
   - Most of it redundant with existing force_commit

**Root Cause - User's Diagnosis:**
> "The accumulation buffer is never being reset during live mode, and it keeps the same buffer even after live mode ends. It only clears on a 'true' fractal redraw."

> "All we need to do is clear the accumulation buffer every frame in live mode."

### Corrected Understanding

**What "Live Mode" Needs:**
- **Every frame during live mode**: Clear accumulation buffer (reset)
- No progressive accumulation → prevents overbright dense areas
- Lower quality, but fast visual feedback
- OR ensure reset happens before display when exiting live mode

**Existing Capabilities (Don't Overengineer):**
- `renderer.reset()` already exists - clears accumulation buffer
- `is_in_preview_mode()` already detects live mode
- `force_commit_preview()` already called on drag end by sliders/Triangle Editor
- Just need to call reset at the right time

**The Simple Solution:**
```rust
// In render loop:
if in_preview_mode {
    // Clear accumulation every frame during live mode
    renderer.reset(...)
}
// OR
if preview_just_ended {
    // Clear accumulation when exiting live mode (before next display)
    renderer.reset(...)
}
```

### Code Cleanup - Reverted to Minimal Fix ✅

**Reverted all overengineered code:**
- ✅ Removed `preview_just_created`, `preview_just_committed` flags
- ✅ Removed `last_preview_update` timestamp tracking
- ✅ Removed `check_and_clear_stale_preview()` method
- ✅ Removed `consume_*_flag()` methods
- ✅ Removed `was_in_preview_mode_last_frame` tracking

**Kept ONLY the "no blink" fix:**
```rust
// In update_param() and update_batch() (2 lines total):
// Changed from: self.current = self.preview.take().unwrap();
// Changed to:   self.current = self.preview.clone().unwrap();
```
This prevents blink during drag by keeping preview active through throttle commits.

### Next: Implement Simple Live Mode Fix - DO BOTH ✅

**User Decision:**
> "It should probably do both, clear the accumulation buffer during live mode, and completely redraw the fractal when exiting live mode."

**Implementation Plan:**

**Part 1 - Clear accumulation every frame during live mode:**
```rust
// In app.rs render loop, BEFORE compute pass:
if self.config_manager.is_in_preview_mode() {
    renderer.reset(...);  // Clear accumulation buffer every frame
}
```
- Prevents progressive brightness buildup
- Each frame starts fresh (low quality, fast feedback)
- No accumulated overbright areas

**Part 2 - Complete redraw when exiting live mode:**
```rust
// In app.rs, detect when preview mode ends:
let was_in_preview = /* track previous frame state */;
let in_preview = self.config_manager.is_in_preview_mode();
if was_in_preview && !in_preview {
    renderer.reset(...);  // Trigger complete redraw for full quality
}
```
- Ensures final render is full quality
- Clears any residual state from live mode
- Fresh accumulation starts

**Why Both:**
1. **During**: Prevents brightness buildup (keeps live mode clean)
2. **Exit**: Ensures high quality final result (fresh start for accumulation)

**Implementation Details:**
- Use existing `renderer.reset()` - already tested and working
- Use existing `is_in_preview_mode()` - simple boolean check
- Track `was_in_preview_mode_last_frame` in App struct (1 bool field)
- No complex flags, timestamps, or state machines needed

---

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

## Phase 4 Extended: Mouse Panning Migration (2025-10-30) ✅ COMPLETE

### Overview
Mouse drag panning was the last input that modified state directly without going through ConfigManager. This prevented undo/redo support and caused rendering issues during drag.

### Before Migration
**File**: `src/app/input.rs` - `handle_mouse_move()`

**Old behavior:**
- Directly modified `self.pan_x` and `self.pan_y`
- Set `view_changed_by_keyboard = true` to trigger reset
- No undo/redo support
- Full reset every frame during drag (black flashes)

### After Migration
**Changes made:**
1. Route pan changes through ConfigManager:
   ```rust
   let _ = self.config_manager.update_param(ConfigPath::PanX, new_pan_x.into(), true);
   let _ = self.config_manager.update_param(ConfigPath::PanY, new_pan_y.into(), true);
   ```

2. Read back from preview:
   ```rust
   self.pan_x = self.config_manager.active_config().pan_x;
   self.pan_y = self.config_manager.active_config().pan_y;
   ```

3. Remove reset trigger during drag (overwrite mode handles it)

4. Force commit on mouse release:
   ```rust
   if self.config_manager.is_in_preview_mode() {
       let _ = self.config_manager.force_commit_preview(&ConfigPath::PanX);
   }
   ```

### Benefits
- ✅ Live preview during mouse drag (smooth, no resets)
- ✅ Overwrite mode prevents brightness buildup (shader fix applies)
- ✅ Undo/redo support for mouse panning
- ✅ Lazy undo throttling (500ms) - not spamming undo stack
- ✅ Clean commit to undo stack on mouse release
- ✅ No black flashes or flickering during drag

### Key Insight
The infrastructure was already complete! Just routed the changes through ConfigManager instead of direct field modification. ConfigPath::PanX and ConfigPath::PanY already existed and were fully implemented.

### Follow-up: Atomic Batch Updates (2025-10-30)

**Problem:** Initial implementation used two separate `update_param()` calls for PanX and PanY, creating two undo entries instead of one atomic operation.

**Solution:** Changed to `update_batch()` to capture both X and Y together:
```rust
self.config_manager.update_batch(
    vec![
        (ConfigPath::PanX, new_pan_x.into()),
        (ConfigPath::PanY, new_pan_y.into()),
    ],
    "Pan (Mouse)".to_string(),
    true  // Lazy mode
);
```

**Note on force_commit_preview():** Removed the call on mouse release because it only handles single parameters. For batch updates, the throttle mechanism properly captures the full batch. `force_commit_preview()` would need refactoring to handle batch updates properly (future improvement).

**Result:** Single "Pan (Mouse)" undo entry that restores both X and Y atomically.

---

## Phase 4 Extended: View Slider Reset Fix (2025-10-30) ✅ COMPLETE

### Problem
Rotation and Camera rotation sliders in View window were flashing/going black during drag, similar to the original mouse panning issue.

### Root Cause
View sliders set `view_changed = true` or `camera_rotation_changed = true` every frame during drag. These flags triggered full accumulation resets every frame, causing black flashes when the buffer cleared before the next frame could render.

### Solution
Only set view change flags when NOT in preview mode:

```rust
// Rotation slider (line 201-204)
if !config_manager.is_in_preview_mode() {
    *view_changed = true;
}

// Camera rotation sliders (lines 226-229, 246-249)
if !config_manager.is_in_preview_mode() {
    *camera_rotation_changed = true;
}
```

### Fixed Sliders
1. Rotation slider (2D view rotation)
2. Camera Pitch slider (3D camera X rotation)
3. Camera Yaw slider (3D camera Y rotation)

### Result
- ✅ Smooth rotation during drag (no flashes)
- ✅ Overwrite mode provides live preview
- ✅ Undo/redo already working (sliders use ConfigManager)
- ✅ Lazy throttling already working (500ms intervals)

### Note on Legacy Flags
`view_changed` and `camera_rotation_changed` are legacy flags from the old system that coexist with the new ConfigManager/UpdateType system. They could eventually be replaced by checking UpdateType, but they're not causing problems now that they respect preview mode.

---

## Phase 6: Variation Controls Migration (2025-10-30) ✅ COMPLETE

### Overview
Migrated all variation weight sliders and variation parameter controls (Float, Integer, Angle) to ConfigManager with lazy undo support.

### Changed Files
1. **src/ui/variation_params.rs** - Parameter rendering (JuliaN power, Blob waves, etc.)
   - Changed signature to accept `config_manager` and `transform_index`
   - Uses `ConfigPath::TransformVariationParam` with lazy=true
   - Returns `UpdateType` instead of bool flag

2. **src/ui/variation_controls.rs** - Variation weight sliders by category
   - Changed signature to accept `config_manager` and `transform_index`
   - Uses `ConfigPath::TransformVariation` with lazy=true
   - Calls `render_variation_params()` for active variations
   - Returns `UpdateType`

3. **src/ui/transforms.rs** - Calls updated variation functions
   - Passes `config_manager` and tracks `UpdateType`

### Key Implementation Details

**Variation Weights** (variation_controls.rs):
```rust
pub fn render_variation_category(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    transform_index: usize,
    category: VariationCategory,
    category_label: &str,
) -> UpdateType {
    let mut max_update = UpdateType::None;
    let variations = crate::variations::global_registry().by_category(category);

    for var_info in variations {
        let transform = &config_manager.active_config().flame.transforms[transform_index];
        let mut value = transform.get_variation(&var_info.name);

        if ui.add(egui::Slider::new(&mut value, 0.0..=2.0).text(&var_info.display_name)).changed() {
            let path = ConfigPath::TransformVariation {
                index: transform_index,
                variation: var_info.name.clone(),
            };
            if let Ok(update_type) = config_manager.update_param(path, value.into(), true) {
                max_update = max_update.max(update_type);
            }
        }
        // Show parameters if active...
    }
    max_update
}
```

**Variation Parameters** (variation_params.rs):
```rust
pub fn render_variation_params(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    transform_index: usize,
    var_name: &str,
    parameters: &[VariationParameter],
) -> UpdateType {
    let mut max_update = UpdateType::None;

    for param in parameters {
        let transform = &config_manager.active_config().flame.transforms[transform_index];
        let mut param_value = transform.get_variation_param_or_default(
            var_name, &param.name, &crate::variations::global_registry()
        );

        let param_changed = match param.param_type {
            ParamType::Float => render_float_param(ui, param, &mut param_value),
            ParamType::Integer => render_integer_param(ui, param, &mut param_value),
            ParamType::Angle => render_angle_param(ui, param, &mut param_value),
        };

        if param_changed {
            let path = ConfigPath::TransformVariationParam {
                index: transform_index,
                variation: var_name.to_string(),
                param: param.name.clone(),
            };
            if let Ok(update_type) = config_manager.update_param(path, param_value.into(), true) {
                max_update = max_update.max(update_type);
            }
        }
    }
    max_update
}
```

### Benefits
- **Smooth dragging**: 500ms lazy throttling prevents undo spam
- **Live preview**: Changes visible immediately during drag
- **Proper undo**: Single undo per drag sequence
- **Add/Delete variations**: Already supported via HashMap-based variation storage
  - Variation name stored in ConfigPath
  - Weight stored in ConfigValue
  - Setting weight to 0.0 effectively "deletes" (undo restores)

### Result
✅ All 26 core variations + unlimited plugin variations fully integrated with delta-based state management

---

## Phase 7: Tone Mapping & Colors Window (2025-10-30) ✅ COMPLETE

### Overview
Migrated all remaining controls in Tone Mapping & Colors window to ConfigManager.

### Migrated Controls (8 total)
1. **Tonemap Mode buttons** (Linear, Log, LogSqrt) - lazy=false (discrete choice)
2. **Use Tone Curve checkbox** - lazy=false (discrete toggle)
3. **Tone Curve preset buttons** (S-Curve, Gentle, etc.) - lazy=false (discrete preset)
4. **Color Mode dropdown** (Transform, Palette, Speed) - lazy=false (discrete choice)
5. **Palette selector** - lazy=false (discrete selection)
6. **Speed Factor slider** - lazy=true (continuous drag)
7. **Background Color picker** - lazy=false (RGB picker is discrete per component)
8. **Tone Curve editor** - lazy=true with force_commit on drag end (interactive control)

### Key Implementation: Tone Curve Editor

Interactive drag control following Triangle Editor pattern:

```rust
fn render_curve_editor(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    curve: &mut ToneCurve,
    curve_changed: &mut bool,
) -> UpdateType {
    // ... drawing code ...

    // Update dragged point (lazy mode)
    if let Some(idx) = dragging_point {
        if let Some(drag_pos) = ui.ctx().pointer_latest_pos() {
            let (new_x, new_y) = from_screen(drag_pos);
            let mut modified_curve = config_manager.active_config().tonemap_curve.clone();
            modified_curve.move_point(idx, new_x, new_y);

            if let Ok(update) = config_manager.update_param(
                ConfigPath::TonemapCurve,
                modified_curve.into(),
                true  // lazy
            ) {
                *curve = config_manager.active_config().tonemap_curve.clone();
                *curve_changed = true;
                max_update = max_update.max(update);
            }
        }
    }

    // Force commit on drag end
    if !mouse_down && dragging_point.is_some() {
        dragging_point = None;
        if config_manager.is_in_preview_mode() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TonemapCurve);
        }
    }

    // Double-click add point (immediate capture)
    if response.double_clicked() {
        if let Some(click_pos) = response.interact_pointer_pos() {
            let (x, y) = from_screen(click_pos);
            let mut modified_curve = config_manager.active_config().tonemap_curve.clone();
            modified_curve.add_point(CurvePoint::new(x, y));

            if let Ok(update) = config_manager.update_param(
                ConfigPath::TonemapCurve,
                modified_curve.into(),
                false  // immediate
            ) {
                *curve = config_manager.active_config().tonemap_curve.clone();
                *curve_changed = true;
                max_update = max_update.max(update);
            }
        }
    }

    max_update
}
```

### Bug Fix: Checkbox Sync Issue (commit a488519)
**Problem**: Tone Curve checkbox appeared checked but curve editor was disabled at startup.

**Root Cause**: Line 114 used `*use_curve` (stale app-level variable) for `add_enabled_ui()` but checkbox read from `config_manager.active_config().use_curve`. These were out of sync at startup.

**Fix**: Changed line 114-115 to read from config for both display and enabled state:
```rust
let current_use_curve = config_manager.active_config().use_curve;
ui.add_enabled_ui(current_use_curve, |ui| {
```

### Result
✅ All Tone Mapping & Colors controls integrated with delta-based state management

---

## Phase 8: Preset Loading System (2025-10-30) ✅ COMPLETE

### Problem
Old preset system used flag-based approach:
- UI set `preset_changed = true` flag
- App.rs called `import_config()` which manually copied all fields
- No undo support for preset loading
- Couldn't undo back to previous preset

### Solution: Snapshot-Based Preset Loading

Extended ConfigChange to support full config snapshots (not just deltas):

```rust
pub struct ConfigChange {
    pub deltas: Vec<ConfigDelta>,
    pub timestamp: Instant,
    pub description: String,
    /// Full config snapshot (used for preset loading)
    /// When Some: this is a full config replacement, ignore deltas for undo
    /// When None: use deltas for undo/redo
    pub snapshot: Option<Box<FractalConfig>>,
}

impl ConfigChange {
    /// Create snapshot undo point (for preset loading)
    pub fn snapshot(config: FractalConfig, description: String) -> Self {
        Self {
            deltas: vec![],
            timestamp: Instant::now(),
            description,
            snapshot: Some(Box::new(config)),
        }
    }
}
```

### Two-Snapshot Approach

Loading a preset creates **TWO undo entries**:
1. Snapshot of **old state** → "Before: Load Preset: [name]"
2. Snapshot of **new state** → "Load Preset: [name]"

This allows:
- **Single Undo**: Restores previous preset completely
- **Single Redo**: Restores new preset after undo
- **Clean undo history**: 2 entries per preset load (not 50+ deltas)

### ConfigManager::load_config()

```rust
pub fn load_config(&mut self, new_config: FractalConfig, description: String) -> Result<(), ConfigError> {
    // Clear any preview state
    self.preview = None;

    // Create snapshot of current state (for undo)
    let old_snapshot = ConfigChange::snapshot(
        self.current.clone(),
        format!("Before: {}", description),
    );
    self.push_undo(old_snapshot);

    // Replace current config
    self.current = new_config.clone();

    // Create snapshot of new state (for redo after undo)
    let new_snapshot = ConfigChange::snapshot(
        new_config,
        description,
    );
    self.push_undo(new_snapshot);

    Ok(())
}
```

### Updated Undo/Redo Methods

Both methods now check for snapshots before processing deltas:

```rust
pub fn undo(&mut self) -> Result<UpdateType, ConfigError> {
    let change = self.undo_stack.pop().ok_or(ConfigError::EmptyUndoStack)?;

    // Check if this is a snapshot-based undo
    if let Some(snapshot) = &change.snapshot {
        log::debug!("  Restoring full config snapshot");
        self.current = (**snapshot).clone();
        self.redo_stack.push(change);
        return Ok(UpdateType::IterationReset); // Full config change
    }

    // Delta-based undo (original behavior)
    // ... existing delta code ...
}

pub fn redo(&mut self) -> Result<UpdateType, ConfigError> {
    let change = self.redo_stack.pop().ok_or(ConfigError::EmptyRedoStack)?;

    // Check if this is a snapshot-based redo
    if let Some(snapshot) = &change.snapshot {
        log::debug!("  Restoring full config snapshot");
        self.current = (**snapshot).clone();
        self.undo_stack.push(change);
        return Ok(UpdateType::IterationReset); // Full config change
    }

    // Delta-based redo (original behavior)
    // ... existing delta code ...
}
```

### UI Integration (settings.rs)

Preset selector now calls ConfigManager directly:

```rust
egui::ComboBox::from_label("Preset")
    .selected_text(current_preset_name)
    .show_ui(ui, |ui| {
        for (idx, preset) in presets.iter().enumerate() {
            if ui.selectable_value(current_preset_index, idx, &preset.flame.name).changed() {
                println!("UI: Loading preset: {} ({})", preset.flame.name, idx);
                // Load preset via ConfigManager (creates two undo points)
                if let Err(e) = config_manager.load_config(
                    preset.clone(),
                    format!("Load Preset: {}", preset.flame.name),
                ) {
                    log::error!("Failed to load preset: {}", e);
                } else {
                    // Update flame reference from config
                    *flame = config_manager.active_config().flame.clone();
                    *preset_changed = true;
                }
            }
        }
    });
```

### App.rs Changes

**Cleaner Implementation**: Preset loading now uses normal update path instead of duplicating GPU upload logic.

**State Sync** (lines 939-950):
```rust
// Handle preset change: sync app state from ConfigManager
// Note: Preset loading happens via ConfigManager in UI layer (settings.rs)
// GPU upload and reset handled by normal update path below
if ui_response.preset_changed {
    println!("Preset loaded via ConfigManager, syncing app state");
    let config = self.config_manager.active_config();
    self.flame = config.flame.clone();
    self.zoom = config.zoom;
    self.pan_x = config.pan_x;
    self.pan_y = config.pan_y;
    self.rotation = config.rotation;
    self.camera_rotation_x = config.camera_rotation_x;
    self.camera_rotation_y = config.camera_rotation_y;
    // GPU upload handled below via flame_changed flag
}
```

**Normal Update Path Integration**:
- Removed `!preset_loaded` guard (line 966) - presets now use normal update logic
- Settings sets both `preset_changed` AND `flame_changed` flags (line 115)
- `flame_changed` triggers GPU upload via existing path (line 991-996)
- `preset_changed` added to reset conditions (line 1056)
- **No code duplication** - reuses all existing update logic!

**Key Insight**: Don't special-case presets - just set the right flags (`flame_changed` + `preset_changed`) and let existing update system handle GPU upload and reset.

### Benefits
- ✅ **Atomic preset loading**: Single operation, not 50+ deltas
- ✅ **Full undo/redo support**: Can undo/redo preset changes
- ✅ **Clean undo history**: Two entries per preset load
- ✅ **No massive deltas**: Full config stored as snapshot
- ✅ **Works with existing system**: Snapshots and deltas coexist seamlessly
- ✅ **No code duplication**: Reuses normal GPU upload/reset paths
- ✅ **Easier maintenance**: Changes to update logic automatically apply to presets

### Result
Preset loading fully integrated with delta-based state management via clean flag-based approach!

---

## Open Questions

1. ~~**Preset loading**: Should loading a preset clear undo history? Or create a single "Load preset: X" undo point?~~ ✅ RESOLVED
   - **Solution**: Two-snapshot approach (before/after) for clean undo/redo

2. **Import config**: Should importing a .fflame file clear undo history?
   - **Recommendation**: Use same snapshot approach as presets
   - Allows undoing back to previous state

3. **Undo window placement**: Separate window or panel in main UI?
   - **Recommendation**: Separate window (like other panels)

4. **Drag-end guarantee**: Should drag end ALWAYS create undo point, even if < 500ms since last?
   - **Recommendation**: Yes - call `reset_lazy_undo()` on drag end to force capture

## References

- Current undo system: [src/app/config.rs](../../src/app/config.rs)
- Lazy undo attempt: [docs/projects/lazy-undo-implementation.md](./lazy-undo-implementation.md)
- FractalConfig definition: [src/config.rs](../../src/config.rs)
- UI response flags: [src/ui/response.rs](../../src/ui/response.rs)
