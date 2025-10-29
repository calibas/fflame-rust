/// Configuration manager - central authority for all config changes
///
/// Handles:
/// - Single gateway for all parameter updates
/// - Delta-based undo/redo with lazy throttling
/// - Selective updates based on change type
/// - Human-readable change descriptions

use super::delta::{
    AffineParam, ColorComponent, ConfigChange, ConfigDelta, ConfigPath, ConfigValue, UpdateType,
};
use super::fractal_config::FractalConfig;
use std::time::Duration;
use web_time::Instant;

/// Central manager for configuration state and undo/redo
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
        let delta = ConfigDelta::new(path.clone(), old_value, new_value.clone());
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
        self.set_value(&path, new_value)?;

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
        let change = self
            .undo_stack
            .pop()
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
        let change = self
            .redo_stack
            .pop()
            .ok_or(ConfigError::EmptyRedoStack)?;

        // Apply deltas
        for delta in &change.deltas {
            self.set_value(&delta.path, delta.new_value.clone())?;
        }

        // Push to undo stack
        self.push_undo(change.clone());

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
    pub fn get_value(&self, path: &ConfigPath) -> Result<ConfigValue, ConfigError> {
        match path {
            // View
            ConfigPath::Zoom => Ok(self.current.zoom.into()),
            ConfigPath::PanX => Ok(self.current.pan_x.into()),
            ConfigPath::PanY => Ok(self.current.pan_y.into()),
            ConfigPath::Rotation => Ok(self.current.rotation.into()),
            ConfigPath::CameraRotationX => Ok(self.current.camera_rotation_x.into()),
            ConfigPath::CameraRotationY => Ok(self.current.camera_rotation_y.into()),

            // Tone mapping
            ConfigPath::Exposure => Ok(self.current.exposure.into()),
            ConfigPath::Gamma => Ok(self.current.gamma.into()),
            ConfigPath::DensityScale => Ok(self.current.density_scale.into()),
            ConfigPath::TonemapMode => Ok(self.current.tonemap_mode.into()),
            ConfigPath::TonemapCurve => Ok(self.current.tonemap_curve.clone().into()),
            ConfigPath::UseCurve => Ok(self.current.use_curve.into()),

            // Color
            ConfigPath::ColorMode => Ok(self.current.color_mode.into()),
            ConfigPath::PaletteIndex => Ok((self.current.palette_index as u32).into()),
            ConfigPath::Palette(p) => Ok(ConfigValue::Palette((**p).clone())),
            ConfigPath::SpeedFactor => Ok(self.current.speed_factor.into()),
            ConfigPath::BackgroundColor => Ok(self.current.background_color.into()),

            // Rendering settings
            ConfigPath::HistogramColorScale => Ok(self.current.histogram_color_scale.into()),
            ConfigPath::LowDensitySmoothing => Ok(self.current.low_density_smoothing.into()),
            ConfigPath::DensityCompressionStrength => {
                Ok(self.current.density_compression_strength.into())
            }
            ConfigPath::BlendFactor => Ok(self.current.blend_factor.into()),
            ConfigPath::TargetIterationsPerPixel => {
                Ok(self.current.target_iterations_per_pixel.into())
            }
            ConfigPath::MaxIterations => Ok(self.current.max_iterations.into()),
            ConfigPath::DeterministicRng => Ok(self.current.deterministic_rng.into()),

            // Transforms
            ConfigPath::TransformCount => {
                Ok((self.current.flame.transforms.len() as u32).into())
            }
            ConfigPath::TransformWeight { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.weight.into())
            }
            ConfigPath::TransformColor { index, component } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match component {
                    ColorComponent::R => xform.color[0],
                    ColorComponent::G => xform.color[1],
                    ColorComponent::B => xform.color[2],
                };
                Ok(value.into())
            }
            ConfigPath::TransformColorSpeed { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.color_speed.into())
            }
            ConfigPath::TransformAffine { index, param } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => xform.a,
                    AffineParam::B => xform.b,
                    AffineParam::C => xform.c,
                    AffineParam::D => xform.d,
                    AffineParam::E => xform.e,
                    AffineParam::F => xform.f,
                    AffineParam::G => xform.g,
                };
                Ok(value.into())
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight = xform.variations.get(variation).copied().unwrap_or(0.0);
                Ok(weight.into())
            }
            ConfigPath::TransformVariationParam {
                index,
                variation,
                param,
            } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let key = format!("{}.{}", variation, param);
                let value = xform.variation_params.get(&key).copied().unwrap_or(0.0);
                Ok(value.into())
            }

            // Flame
            ConfigPath::RenderMode => Ok(self.current.flame.render_mode.into()),
            ConfigPath::ProjectionType => Ok(self.current.flame.projection.into()),
        }
    }

    /// Set value in config by path
    fn set_value(&mut self, path: &ConfigPath, value: ConfigValue) -> Result<(), ConfigError> {
        match path {
            // View
            ConfigPath::Zoom => {
                self.current.zoom = value.try_into()?;
            }
            ConfigPath::PanX => {
                self.current.pan_x = value.try_into()?;
            }
            ConfigPath::PanY => {
                self.current.pan_y = value.try_into()?;
            }
            ConfigPath::Rotation => {
                self.current.rotation = value.try_into()?;
            }
            ConfigPath::CameraRotationX => {
                self.current.camera_rotation_x = value.try_into()?;
            }
            ConfigPath::CameraRotationY => {
                self.current.camera_rotation_y = value.try_into()?;
            }

            // Tone mapping
            ConfigPath::Exposure => {
                self.current.exposure = value.try_into()?;
            }
            ConfigPath::Gamma => {
                self.current.gamma = value.try_into()?;
            }
            ConfigPath::DensityScale => {
                self.current.density_scale = value.try_into()?;
            }
            ConfigPath::TonemapMode => {
                self.current.tonemap_mode = value.try_into()?;
            }
            ConfigPath::TonemapCurve => {
                self.current.tonemap_curve = value.try_into()?;
            }
            ConfigPath::UseCurve => {
                self.current.use_curve = value.try_into()?;
            }

            // Color
            ConfigPath::ColorMode => {
                self.current.color_mode = value.try_into()?;
            }
            ConfigPath::PaletteIndex => {
                let idx: u32 = value.try_into()?;
                self.current.palette_index = idx as usize;
            }
            ConfigPath::Palette(p) => {
                if let ConfigValue::Palette(palette) = value {
                    // Update embedded palette data
                    self.current.palette = Some(palette);
                }
            }
            ConfigPath::SpeedFactor => {
                self.current.speed_factor = value.try_into()?;
            }
            ConfigPath::BackgroundColor => {
                self.current.background_color = value.try_into()?;
            }

            // Rendering settings
            ConfigPath::HistogramColorScale => {
                self.current.histogram_color_scale = value.try_into()?;
            }
            ConfigPath::LowDensitySmoothing => {
                self.current.low_density_smoothing = value.try_into()?;
            }
            ConfigPath::DensityCompressionStrength => {
                self.current.density_compression_strength = value.try_into()?;
            }
            ConfigPath::BlendFactor => {
                self.current.blend_factor = value.try_into()?;
            }
            ConfigPath::TargetIterationsPerPixel => {
                self.current.target_iterations_per_pixel = value.try_into()?;
            }
            ConfigPath::MaxIterations => {
                self.current.max_iterations = value.try_into()?;
            }
            ConfigPath::DeterministicRng => {
                self.current.deterministic_rng = value.try_into()?;
            }

            // Transforms
            ConfigPath::TransformCount => {
                // Can't directly set count - must add/remove transforms
                return Err(ConfigError::ReadOnlyParameter);
            }
            ConfigPath::TransformWeight { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.weight = value.try_into()?;
            }
            ConfigPath::TransformColor { index, component } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                match component {
                    ColorComponent::R => xform.color[0] = new_value,
                    ColorComponent::G => xform.color[1] = new_value,
                    ColorComponent::B => xform.color[2] = new_value,
                }
            }
            ConfigPath::TransformColorSpeed { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.color_speed = value.try_into()?;
            }
            ConfigPath::TransformAffine { index, param } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.a = new_value,
                    AffineParam::B => xform.b = new_value,
                    AffineParam::C => xform.c = new_value,
                    AffineParam::D => xform.d = new_value,
                    AffineParam::E => xform.e = new_value,
                    AffineParam::F => xform.f = new_value,
                    AffineParam::G => xform.g = new_value,
                }
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight: f32 = value.try_into()?;
                if weight == 0.0 {
                    xform.variations.remove(variation);
                } else {
                    xform.variations.insert(variation.clone(), weight);
                }
            }
            ConfigPath::TransformVariationParam {
                index,
                variation,
                param,
            } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                let key = format!("{}.{}", variation, param);
                xform.variation_params.insert(key, new_value);
            }

            // Flame
            ConfigPath::RenderMode => {
                self.current.flame.render_mode = value.try_into()?;
            }
            ConfigPath::ProjectionType => {
                self.current.flame.projection = value.try_into()?;
            }
        }

        Ok(())
    }

    /// Get current config (read-only)
    pub fn config(&self) -> &FractalConfig {
        &self.current
    }

    /// Get mutable config (for operations that need it - use sparingly!)
    pub fn config_mut(&mut self) -> &mut FractalConfig {
        &mut self.current
    }

    /// Get undo stack (for displaying in undo window)
    pub fn undo_history(&self) -> &[ConfigChange] {
        &self.undo_stack
    }

    /// Get redo stack
    pub fn redo_history(&self) -> &[ConfigChange] {
        &self.redo_stack
    }

    /// Check if can undo
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if can redo
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    TypeMismatch,
    InvalidIndex,
    EmptyUndoStack,
    EmptyRedoStack,
    ReadOnlyParameter,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConfigError::TypeMismatch => write!(f, "Config value type mismatch"),
            ConfigError::InvalidIndex => write!(f, "Invalid transform index"),
            ConfigError::EmptyUndoStack => write!(f, "Nothing to undo"),
            ConfigError::EmptyRedoStack => write!(f, "Nothing to redo"),
            ConfigError::ReadOnlyParameter => write!(f, "Parameter is read-only"),
        }
    }
}

impl std::error::Error for ConfigError {}

// TryFrom implementations for extracting values from ConfigValue
impl TryFrom<ConfigValue> for f32 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Float(f) => Ok(f),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for i32 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Int(i) => Ok(i),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for u32 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::UInt(u) => Ok(u),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for u64 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::UInt64(u) => Ok(u),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for bool {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Bool(b) => Ok(b),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for [f32; 3] {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ColorRgb(c) => Ok(c),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::palette::ColorMode;
use crate::scene::transforms::{RenderMode, ProjectionType};

impl TryFrom<ConfigValue> for ToneMapMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ToneMapMode(m) => Ok(m),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for ColorMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ColorMode(m) => Ok(m),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for RenderMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::RenderMode(m) => Ok(m),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for ProjectionType {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ProjectionType(p) => Ok(p),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for ToneCurve {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ToneCurve(c) => Ok(c),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::fractal_config::FractalConfig;

    #[test]
    fn test_get_set_exposure() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // Get initial value
        let value = manager.get_value(&ConfigPath::Exposure).unwrap();
        assert!(value.approx_eq(&ConfigValue::Float(1.0)));

        // Set new value
        manager
            .set_value(&ConfigPath::Exposure, 2.0.into())
            .unwrap();
        assert_eq!(manager.current.exposure, 2.0);
    }

    #[test]
    fn test_update_param_lazy() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // First lazy update - should capture
        let update1 = manager
            .update_param(ConfigPath::Exposure, 2.0.into(), true)
            .unwrap();
        assert_eq!(update1, UpdateType::ToneMappingOnly);
        assert_eq!(manager.undo_stack.len(), 1);

        // Immediate second update - should NOT capture (throttled)
        let update2 = manager
            .update_param(ConfigPath::Exposure, 3.0.into(), true)
            .unwrap();
        assert_eq!(update2, UpdateType::ToneMappingOnly);
        assert_eq!(manager.undo_stack.len(), 1); // Still 1!
    }

    #[test]
    fn test_undo_redo() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // Make change
        manager
            .update_param(ConfigPath::Exposure, 2.0.into(), false)
            .unwrap();
        assert_eq!(manager.current.exposure, 2.0);

        // Undo
        manager.undo().unwrap();
        assert_eq!(manager.current.exposure, 1.0);

        // Redo
        manager.redo().unwrap();
        assert_eq!(manager.current.exposure, 2.0);
    }

    #[test]
    fn test_batch_update() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        let changes = vec![
            (ConfigPath::Zoom, ConfigValue::Float(2.0)),
            (ConfigPath::PanX, ConfigValue::Float(1.0)),
            (ConfigPath::PanY, ConfigValue::Float(-1.0)),
        ];

        let update = manager
            .update_batch(changes, "Reset View".to_string(), false)
            .unwrap();

        assert_eq!(update, UpdateType::ViewOnly);
        assert_eq!(manager.undo_stack.len(), 1);
        assert_eq!(manager.undo_stack[0].deltas.len(), 3);
        assert_eq!(manager.undo_stack[0].description, "Reset View");
    }
}
