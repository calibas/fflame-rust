/// Delta-based configuration change tracking
///
/// This module defines the core types for tracking configuration changes:
/// - ConfigPath: Identifies any parameter in FractalConfig
/// - ConfigValue: Type-safe container for parameter values
/// - ConfigDelta: Records a single parameter change (old → new)
/// - ConfigChange: Batch of deltas (single undo point)
/// - UpdateType: What kind of update is needed for a change

use crate::scene::palette::Palette;
use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::palette::ColorMode;
use crate::scene::transforms::{RenderMode, ProjectionType};
use std::fmt::{self, Display, Formatter};
use web_time::Instant;

/// Identifies a specific parameter in the configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigPath {
    // ===== View parameters (no fractal recalc needed) =====
    Zoom,
    Pan,  // Combined PanX and PanY into single Vec2 value
    Rotation,
    CameraRotationX,
    CameraRotationY,
    CameraZ,

    // ===== Tone mapping (no iteration reset needed) =====
    Exposure,
    Gamma,
    GammaThreshold,
    Brightness,
    Vibrancy,
    Saturation,
    HueShift,
    ValueScale,
    DensityScale,
    TonemapMode,
    TonemapCurve,
    UseCurve,

    // ===== Color (no iteration reset, just color buffer update) =====
    ColorMode,
    PaletteIndex,
    Palette, // Embedded palette data (custom palettes)
    PaletteRotation,
    SpeedFactor,
    BackgroundColor,

    // ===== Rendering settings (affects iteration speed/quality) =====
    HistogramColorScale,
    LowDensitySmoothing,
    DensityCompressionStrength,
    BlendFactor,
    UseDynamicBlend,
    TargetIterationsPerPixel,
    IterationsPerThread,
    SpeedMultiplier,
    MaxIterations,
    DeterministicRng,

    // ===== Transform-level changes (require iteration reset) =====
    TransformCount,
    TransformWeight { index: usize },
    TransformColor { index: usize },
    TransformColorSpeed { index: usize },
    TransformOpacity { index: usize },
    TransformAffine { index: usize, param: AffineParam },
    TransformVariation { index: usize, variation: String },
    TransformVariationParam {
        index: usize,
        variation: String,
        param: String,
    },

    // ===== Final Transform (require iteration reset) =====
    FinalTransformEnabled,
    FinalTransformAffine { param: AffineParam },
    FinalTransformColor,
    FinalTransformColorSpeed,
    FinalTransformVariation { variation: String },
    FinalTransformVariationParam {
        variation: String,
        param: String,
    },

    // ===== Flame-level (require iteration reset) =====
    RenderMode,
    ProjectionType,
}

/// Affine transformation parameter (a, b, c, d, e, f, g)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AffineParam {
    A,
    B,
    C,
    D,
    E,
    F,
    G, // Z offset for 3D mode
}

impl Display for ConfigPath {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            // View
            ConfigPath::Zoom => write!(f, "Zoom"),
            ConfigPath::Pan => write!(f, "Pan"),
            ConfigPath::Rotation => write!(f, "Rotation"),
            ConfigPath::CameraRotationX => write!(f, "Camera Pitch"),
            ConfigPath::CameraRotationY => write!(f, "Camera Yaw"),
            ConfigPath::CameraZ => write!(f, "Camera Z"),

            // Tone mapping
            ConfigPath::Exposure => write!(f, "Exposure"),
            ConfigPath::Gamma => write!(f, "Gamma"),
            ConfigPath::GammaThreshold => write!(f, "Gamma Threshold"),
            ConfigPath::Brightness => write!(f, "Brightness"),
            ConfigPath::Vibrancy => write!(f, "Vibrancy"),
            ConfigPath::Saturation => write!(f, "Saturation"),
            ConfigPath::HueShift => write!(f, "Hue Shift"),
            ConfigPath::ValueScale => write!(f, "Value Scale"),
            ConfigPath::DensityScale => write!(f, "Density Scale"),
            ConfigPath::TonemapMode => write!(f, "Tonemap Mode"),
            ConfigPath::TonemapCurve => write!(f, "Tone Curve"),
            ConfigPath::UseCurve => write!(f, "Use Tone Curve"),

            // Color
            ConfigPath::ColorMode => write!(f, "Color Mode"),
            ConfigPath::PaletteIndex => write!(f, "Palette"),
            ConfigPath::Palette => write!(f, "Palette Data"),
            ConfigPath::PaletteRotation => write!(f, "Palette Rotation"),
            ConfigPath::SpeedFactor => write!(f, "Speed Blend Factor"),
            ConfigPath::BackgroundColor => write!(f, "Background Color"),

            // Rendering
            ConfigPath::HistogramColorScale => write!(f, "Histogram Color Scale"),
            ConfigPath::LowDensitySmoothing => write!(f, "Low-Density Smoothing"),
            ConfigPath::DensityCompressionStrength => write!(f, "Density Compression"),
            ConfigPath::BlendFactor => write!(f, "Blend Factor"),
            ConfigPath::UseDynamicBlend => write!(f, "Use Dynamic Blend"),
            ConfigPath::TargetIterationsPerPixel => write!(f, "Target Iterations Per Pixel"),
            ConfigPath::IterationsPerThread => write!(f, "Iterations Per Thread"),
            ConfigPath::SpeedMultiplier => write!(f, "Speed Multiplier"),
            ConfigPath::MaxIterations => write!(f, "Max Iterations"),
            ConfigPath::DeterministicRng => write!(f, "Deterministic RNG"),

            // Transforms
            ConfigPath::TransformCount => write!(f, "Transform Count"),
            ConfigPath::TransformWeight { index } => {
                write!(f, "Transform {} → Weight", index + 1)
            }
            ConfigPath::TransformColor { index } => {
                write!(f, "Transform {} → Color", index + 1)
            }
            ConfigPath::TransformColorSpeed { index } => {
                write!(f, "Transform {} → Color Speed", index + 1)
            }
            ConfigPath::TransformOpacity { index } => {
                write!(f, "Transform {} → Opacity", index + 1)
            }
            ConfigPath::TransformAffine { index, param } => {
                write!(f, "Transform {} → Affine {:?}", index + 1, param)
            }
            ConfigPath::TransformVariation { index, variation } => {
                write!(f, "Transform {} → {} variation", index + 1, variation)
            }
            ConfigPath::TransformVariationParam {
                index,
                variation,
                param,
            } => {
                write!(
                    f,
                    "Transform {} → {} → {}",
                    index + 1,
                    variation,
                    param
                )
            }

            // Final Transform
            ConfigPath::FinalTransformEnabled => write!(f, "Final Transform → Enabled"),
            ConfigPath::FinalTransformAffine { param } => {
                write!(f, "Final Transform → Affine {:?}", param)
            }
            ConfigPath::FinalTransformColor => write!(f, "Final Transform → Color"),
            ConfigPath::FinalTransformColorSpeed => write!(f, "Final Transform → Color Speed"),
            ConfigPath::FinalTransformVariation { variation } => {
                write!(f, "Final Transform → {} variation", variation)
            }
            ConfigPath::FinalTransformVariationParam { variation, param } => {
                write!(f, "Final Transform → {} → {}", variation, param)
            }

            // Flame
            ConfigPath::RenderMode => write!(f, "Render Mode"),
            ConfigPath::ProjectionType => write!(f, "Projection Type"),
        }
    }
}

/// A value that can be stored in FractalConfig
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    Float(f32),
    Int(i32),
    UInt(u32),
    UInt64(u64),
    Bool(bool),
    String(String),
    Vec2(f32, f32),  // For pan coordinates and other 2D values
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
        const EPSILON: f32 = 1e-6;

        match (self, other) {
            (ConfigValue::Float(a), ConfigValue::Float(b)) => (a - b).abs() < EPSILON,
            (ConfigValue::Vec2(x1, y1), ConfigValue::Vec2(x2, y2)) => {
                (x1 - x2).abs() < EPSILON && (y1 - y2).abs() < EPSILON
            }
            (ConfigValue::ColorRgb(a), ConfigValue::ColorRgb(b)) => a
                .iter()
                .zip(b.iter())
                .all(|(x, y)| (x - y).abs() < EPSILON),
            (ConfigValue::Int(a), ConfigValue::Int(b)) => a == b,
            (ConfigValue::UInt(a), ConfigValue::UInt(b)) => a == b,
            (ConfigValue::UInt64(a), ConfigValue::UInt64(b)) => a == b,
            (ConfigValue::Bool(a), ConfigValue::Bool(b)) => a == b,
            (ConfigValue::String(a), ConfigValue::String(b)) => a == b,
            (ConfigValue::ToneMapMode(a), ConfigValue::ToneMapMode(b)) => a == b,
            (ConfigValue::ColorMode(a), ConfigValue::ColorMode(b)) => a == b,
            (ConfigValue::RenderMode(a), ConfigValue::RenderMode(b)) => a == b,
            (ConfigValue::ProjectionType(a), ConfigValue::ProjectionType(b)) => a == b,
            // For complex types, do shallow comparison or always return false
            _ => false,
        }
    }
}

impl Display for ConfigValue {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ConfigValue::Float(v) => write!(f, "{:.3}", v),
            ConfigValue::Int(v) => write!(f, "{}", v),
            ConfigValue::UInt(v) => write!(f, "{}", v),
            ConfigValue::UInt64(v) => write!(f, "{}", v),
            ConfigValue::Bool(v) => write!(f, "{}", v),
            ConfigValue::String(v) => write!(f, "{}", v),
            ConfigValue::Vec2(x, y) => write!(f, "({:.3}, {:.3})", x, y),
            ConfigValue::ColorRgb([r, g, b]) => {
                write!(f, "RGB({:.2}, {:.2}, {:.2})", r, g, b)
            }
            ConfigValue::ToneMapMode(m) => write!(f, "{:?}", m),
            ConfigValue::ColorMode(m) => write!(f, "{:?}", m),
            ConfigValue::RenderMode(m) => write!(f, "{:?}", m),
            ConfigValue::ProjectionType(p) => write!(f, "{:?}", p),
            ConfigValue::ToneCurve(curve) => {
                write!(f, "[Tone Curve: {} pts: {:?}]",
                    curve.points.len(),
                    curve.points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
            }
            ConfigValue::Palette(p) => write!(f, "{}", p.name),
        }
    }
}

// Conversion traits: From basic types to ConfigValue
impl From<f32> for ConfigValue {
    fn from(v: f32) -> Self {
        ConfigValue::Float(v)
    }
}

impl From<i32> for ConfigValue {
    fn from(v: i32) -> Self {
        ConfigValue::Int(v)
    }
}

impl From<u32> for ConfigValue {
    fn from(v: u32) -> Self {
        ConfigValue::UInt(v)
    }
}

impl From<u64> for ConfigValue {
    fn from(v: u64) -> Self {
        ConfigValue::UInt64(v)
    }
}

impl From<bool> for ConfigValue {
    fn from(v: bool) -> Self {
        ConfigValue::Bool(v)
    }
}

impl From<String> for ConfigValue {
    fn from(v: String) -> Self {
        ConfigValue::String(v)
    }
}

impl From<&str> for ConfigValue {
    fn from(v: &str) -> Self {
        ConfigValue::String(v.to_string())
    }
}

impl From<(f32, f32)> for ConfigValue {
    fn from((x, y): (f32, f32)) -> Self {
        ConfigValue::Vec2(x, y)
    }
}

impl From<[f32; 3]> for ConfigValue {
    fn from(v: [f32; 3]) -> Self {
        ConfigValue::ColorRgb(v)
    }
}

impl From<ToneMapMode> for ConfigValue {
    fn from(v: ToneMapMode) -> Self {
        ConfigValue::ToneMapMode(v)
    }
}

impl From<ColorMode> for ConfigValue {
    fn from(v: ColorMode) -> Self {
        ConfigValue::ColorMode(v)
    }
}

impl From<RenderMode> for ConfigValue {
    fn from(v: RenderMode) -> Self {
        ConfigValue::RenderMode(v)
    }
}

impl From<ProjectionType> for ConfigValue {
    fn from(v: ProjectionType) -> Self {
        ConfigValue::ProjectionType(v)
    }
}

impl From<ToneCurve> for ConfigValue {
    fn from(v: ToneCurve) -> Self {
        ConfigValue::ToneCurve(v)
    }
}

impl From<Palette> for ConfigValue {
    fn from(v: Palette) -> Self {
        ConfigValue::Palette(v)
    }
}

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

/// Specialized snapshot data for structural changes
/// Stores only what's needed (before/after states) for efficient undo/redo
#[derive(Debug, Clone)]
pub enum SnapshotData {
    /// Full config replacement (preset loading, file import)
    /// Stores both before and after states for bidirectional undo/redo
    FullConfig {
        before: Box<super::fractal_config::FractalConfig>,
        after: Box<super::fractal_config::FractalConfig>,
    },

    /// Transform added
    /// Undo: remove at index, Redo: insert at index
    AddTransform {
        index: usize,
        transform: crate::scene::transforms::Transform,
    },

    /// Transform deleted
    /// Undo: re-insert at index, Redo: remove at index
    DeleteTransform {
        index: usize,
        transform: crate::scene::transforms::Transform,
    },

    /// Transform modified (affine edit, triangle editor, etc.)
    /// Stores before/after states for complete restoration
    /// Undo: restore before, Redo: restore after
    ModifyTransform {
        index: usize,
        before: crate::scene::transforms::Transform,
        after: crate::scene::transforms::Transform,
    },
}

/// A batch of related changes (single undo point)
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub deltas: Vec<ConfigDelta>,
    pub timestamp: Instant,
    pub description: String,
    /// Snapshot data for structural changes
    /// When Some: use snapshot logic for undo/redo (bidirectional or specialized)
    /// When None: use deltas for undo/redo
    pub snapshot: Option<SnapshotData>,
    /// Last update time for coalescing sequence
    /// When coalescing: timestamp = first change, last_update_time = most recent change
    /// This allows checking both inactivity (time since last update) and total span (time since first)
    pub last_update_time: Instant,
}

impl ConfigChange {
    /// Create from single delta
    pub fn single(delta: ConfigDelta) -> Self {
        let timestamp = delta.timestamp;
        let description = delta.description();
        Self {
            deltas: vec![delta],
            timestamp,
            description,
            snapshot: None,
            last_update_time: timestamp,  // Initially same as timestamp
        }
    }

    /// Create from multiple deltas with custom description
    pub fn batch(deltas: Vec<ConfigDelta>, description: String) -> Self {
        let timestamp = deltas
            .first()
            .map(|d| d.timestamp)
            .unwrap_or_else(Instant::now);
        Self {
            deltas,
            timestamp,
            description,
            snapshot: None,
            last_update_time: timestamp,  // Initially same as timestamp
        }
    }

    /// Create full config snapshot (preset loading, file import)
    /// Stores both before and after states for bidirectional undo/redo
    pub fn full_config_snapshot(
        before: super::fractal_config::FractalConfig,
        after: super::fractal_config::FractalConfig,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::FullConfig {
                before: Box::new(before),
                after: Box::new(after),
            }),
            last_update_time: now,
        }
    }

    /// Create add transform snapshot
    /// Stores the added transform for efficient undo/redo
    pub fn add_transform_snapshot(
        index: usize,
        transform: crate::scene::transforms::Transform,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddTransform { index, transform }),
            last_update_time: now,
        }
    }

    /// Create delete transform snapshot
    /// Stores the deleted transform for efficient undo/redo
    pub fn delete_transform_snapshot(
        index: usize,
        transform: crate::scene::transforms::Transform,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteTransform { index, transform }),
            last_update_time: now,
        }
    }

    /// Create modify transform snapshot
    /// Stores before/after transform states for complete restoration
    pub fn modify_transform_snapshot(
        index: usize,
        before: crate::scene::transforms::Transform,
        after: crate::scene::transforms::Transform,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::ModifyTransform { index, before, after }),
            last_update_time: now,
        }
    }

    /// Invert change (for undo)
    pub fn invert(&self) -> Self {
        let now = Instant::now();
        Self {
            deltas: self.deltas.iter().rev().map(|d| d.invert()).collect(),
            timestamp: now,
            description: format!("Undo: {}", self.description),
            snapshot: self.snapshot.clone(),
            last_update_time: now,
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

/// What kind of update is needed for a change
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdateType {
    None,            // No update needed
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
            | ConfigPath::Pan
            | ConfigPath::Rotation
            | ConfigPath::CameraRotationX
            | ConfigPath::CameraRotationY
            | ConfigPath::CameraZ => UpdateType::ViewOnly,

            // Tone mapping - re-run tonemap shader
            ConfigPath::Exposure
            | ConfigPath::Gamma
            | ConfigPath::GammaThreshold
            | ConfigPath::Brightness
            | ConfigPath::Vibrancy
            | ConfigPath::Saturation
            | ConfigPath::HueShift
            | ConfigPath::ValueScale
            | ConfigPath::DensityScale
            | ConfigPath::TonemapMode
            | ConfigPath::TonemapCurve
            | ConfigPath::UseCurve
            | ConfigPath::BackgroundColor => UpdateType::ToneMappingOnly,

            // Color parameters - re-run accumulation with new colors
            ConfigPath::ColorMode
            | ConfigPath::PaletteIndex
            | ConfigPath::Palette
            | ConfigPath::PaletteRotation
            | ConfigPath::SpeedFactor => UpdateType::ColorOnly,

            // Rendering settings - affect iteration behavior
            ConfigPath::HistogramColorScale
            | ConfigPath::LowDensitySmoothing
            | ConfigPath::DensityCompressionStrength
            | ConfigPath::BlendFactor
            | ConfigPath::UseDynamicBlend
            | ConfigPath::TargetIterationsPerPixel
            | ConfigPath::IterationsPerThread
            | ConfigPath::SpeedMultiplier => UpdateType::IterationReset,

            // Transform/flame changes - full reset
            ConfigPath::TransformCount
            | ConfigPath::TransformWeight { .. }
            | ConfigPath::TransformColor { .. }
            | ConfigPath::TransformColorSpeed { .. }
            | ConfigPath::TransformOpacity { .. }
            | ConfigPath::TransformAffine { .. }
            | ConfigPath::TransformVariation { .. }
            | ConfigPath::TransformVariationParam { .. }
            | ConfigPath::FinalTransformEnabled
            | ConfigPath::FinalTransformAffine { .. }
            | ConfigPath::FinalTransformColor
            | ConfigPath::FinalTransformColorSpeed
            | ConfigPath::FinalTransformVariation { .. }
            | ConfigPath::FinalTransformVariationParam { .. }
            | ConfigPath::RenderMode
            | ConfigPath::ProjectionType
            | ConfigPath::MaxIterations
            | ConfigPath::DeterministicRng => UpdateType::IterationReset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_creation_and_inversion() {
        let delta = ConfigDelta::new(
            ConfigPath::Exposure,
            1.0.into(),
            0.5.into(),
        );

        // Check path (can't use == without PartialEq, but we can check via Display)
        assert_eq!(format!("{}", delta.path), "Exposure");
        assert!(delta.old_value.approx_eq(&ConfigValue::Float(1.0)));
        assert!(delta.new_value.approx_eq(&ConfigValue::Float(0.5)));

        let inverted = delta.invert();
        assert!(inverted.old_value.approx_eq(&ConfigValue::Float(0.5)));
        assert!(inverted.new_value.approx_eq(&ConfigValue::Float(1.0)));
    }

    #[test]
    fn test_config_value_approx_eq() {
        let v1 = ConfigValue::Float(1.0);
        let v2 = ConfigValue::Float(1.0 + 1e-7);
        let v3 = ConfigValue::Float(1.0 + 1e-5);

        assert!(v1.approx_eq(&v2)); // Within epsilon
        assert!(!v1.approx_eq(&v3)); // Outside epsilon
    }

    #[test]
    fn test_update_type_merge() {
        assert_eq!(
            UpdateType::ViewOnly.merge(UpdateType::ToneMappingOnly),
            UpdateType::ToneMappingOnly
        );
        assert_eq!(
            UpdateType::ToneMappingOnly.merge(UpdateType::IterationReset),
            UpdateType::IterationReset
        );
        assert_eq!(
            UpdateType::None.merge(UpdateType::ViewOnly),
            UpdateType::ViewOnly
        );
    }

    #[test]
    fn test_config_path_update_type() {
        assert!(matches!(ConfigPath::Exposure.update_type(), UpdateType::ToneMappingOnly));
        assert!(matches!(ConfigPath::Zoom.update_type(), UpdateType::ViewOnly));
        assert!(matches!(
            ConfigPath::TransformVariation {
                index: 0,
                variation: "linear".to_string()
            }
            .update_type(),
            UpdateType::IterationReset
        ));
    }

    #[test]
    fn test_config_change_batch() {
        let deltas = vec![
            ConfigDelta::new(ConfigPath::Zoom, 1.0.into(), 2.0.into()),
            ConfigDelta::new(ConfigPath::PanX, 0.0.into(), 1.0.into()),
        ];

        let change = ConfigChange::batch(deltas, "Reset View".to_string());

        assert_eq!(change.deltas.len(), 2);
        assert_eq!(change.description, "Reset View");
        assert_eq!(change.update_type(), UpdateType::ViewOnly);
    }
}
