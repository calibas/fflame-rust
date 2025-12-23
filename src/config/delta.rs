/// Delta-based configuration change tracking
///
/// This module defines the core types for tracking configuration changes:
/// - ConfigPath: Identifies any parameter in FractalConfig
/// - ConfigValue: Type-safe container for parameter values
/// - ConfigDelta: Records a single parameter change (old → new)
/// - ConfigChange: Batch of deltas (single undo point)
/// - UpdateType: What kind of update is needed for a change

use crate::scene::palette::{Palette, ColorMode, PathCaptureMode, PathMapStyle, PathTrackingMode};
use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::transforms::RenderMode;
use std::fmt::{self, Display, Formatter};
use web_time::Instant;

/// Identifies a specific parameter in the configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigPath {
    // ===== View parameters (no fractal recalc needed) =====
    Zoom,
    Pan,  // Combined PanX and PanY into single Vec2 value
    PanX, // Separate X component for animation
    PanY, // Separate Y component for animation
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
    AlphaBlendLow,
    AlphaBlendHigh,
    DensityScale,
    TonemapMode,
    TonemapCurve,
    UseCurve,

    // ===== Color (no iteration reset, just color buffer update) =====
    ColorMode,
    PathMapStyle,  // Prefix or Suffix coloring for PathMap mode
    PathCaptureMode,  // FirstHit, FirstAfterBurnIn, or LastHit
    PathTrackingMode,  // First (first 32 iterations) or Recent (rolling window of 32 most recent)
    PaletteIndex,
    Palette, // Embedded palette data (custom palettes)
    PaletteRotation,
    SpeedFactor,
    BackgroundColor,
    BackgroundColorR, // Separate R component for animation
    BackgroundColorG, // Separate G component for animation
    BackgroundColorB, // Separate B component for animation

    // ===== Rendering settings (affects iteration speed/quality) =====
    HistogramColorScale,
    LowDensitySmoothing,
    DensityCompressionStrength,
    BlendFactor,
    UseDynamicBlend,
    TargetIterationsPerPixel,
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
    /// High-level transform origin X (translate X)
    /// Computed from affine: origin_x = e
    TransformOriginX { index: usize },
    /// High-level transform origin Y (translate Y)
    /// Computed from affine: origin_y = -f (Apophysis convention)
    TransformOriginY { index: usize },
    /// High-level transform rotation (angle in radians)
    /// Computed from: atan2(b, a)
    TransformRotation { index: usize },
    /// High-level transform scale (uniform scaling)
    /// Computed from: sqrt(a² + b²)
    TransformScale { index: usize },

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
    /// High-level final transform origin X (translate X)
    FinalTransformOriginX,
    /// High-level final transform origin Y (translate Y)
    FinalTransformOriginY,
    /// High-level final transform rotation (angle in radians)
    FinalTransformRotation,
    /// High-level final transform scale (uniform scaling)
    FinalTransformScale,

    // ===== Flame-level (require iteration reset) =====
    RenderMode,
    PerspectiveStrength,

    // ===== System Settings (device-specific, not tracked for undo) =====
    SystemIterationsPerThread,
    SystemBurnIn,
    SystemVsyncEnabled,
    SystemTargetFps,
    SystemExportWidth,
    SystemExportHeight,
    SystemLanguage,
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
            ConfigPath::PanX => write!(f, "Pan X"),
            ConfigPath::PanY => write!(f, "Pan Y"),
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
            ConfigPath::AlphaBlendLow => write!(f, "Alpha Blend Low"),
            ConfigPath::AlphaBlendHigh => write!(f, "Alpha Blend High"),
            ConfigPath::DensityScale => write!(f, "Density Scale"),
            ConfigPath::TonemapMode => write!(f, "Tonemap Mode"),
            ConfigPath::TonemapCurve => write!(f, "Tone Curve"),
            ConfigPath::UseCurve => write!(f, "Use Tone Curve"),

            // Color
            ConfigPath::ColorMode => write!(f, "Color Mode"),
            ConfigPath::PathMapStyle => write!(f, "PathMap Style"),
            ConfigPath::PathCaptureMode => write!(f, "PathMap Capture Mode"),
            ConfigPath::PathTrackingMode => write!(f, "PathMap Tracking Mode"),
            ConfigPath::PaletteIndex => write!(f, "Palette"),
            ConfigPath::Palette => write!(f, "Palette Data"),
            ConfigPath::PaletteRotation => write!(f, "Palette Rotation"),
            ConfigPath::SpeedFactor => write!(f, "Speed Blend Factor"),
            ConfigPath::BackgroundColor => write!(f, "Background Color"),
            ConfigPath::BackgroundColorR => write!(f, "Background Red"),
            ConfigPath::BackgroundColorG => write!(f, "Background Green"),
            ConfigPath::BackgroundColorB => write!(f, "Background Blue"),

            // Rendering
            ConfigPath::HistogramColorScale => write!(f, "Histogram Color Scale"),
            ConfigPath::LowDensitySmoothing => write!(f, "Low-Density Smoothing"),
            ConfigPath::DensityCompressionStrength => write!(f, "Density Compression"),
            ConfigPath::BlendFactor => write!(f, "Blend Factor"),
            ConfigPath::UseDynamicBlend => write!(f, "Use Dynamic Blend"),
            ConfigPath::TargetIterationsPerPixel => write!(f, "Target Iterations Per Pixel"),
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
            ConfigPath::TransformOriginX { index } => {
                write!(f, "Transform {} → Origin X", index + 1)
            }
            ConfigPath::TransformOriginY { index } => {
                write!(f, "Transform {} → Origin Y", index + 1)
            }
            ConfigPath::TransformRotation { index } => {
                write!(f, "Transform {} → Rotation", index + 1)
            }
            ConfigPath::TransformScale { index } => {
                write!(f, "Transform {} → Scale", index + 1)
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
            ConfigPath::FinalTransformOriginX => write!(f, "Final Transform → Origin X"),
            ConfigPath::FinalTransformOriginY => write!(f, "Final Transform → Origin Y"),
            ConfigPath::FinalTransformRotation => write!(f, "Final Transform → Rotation"),
            ConfigPath::FinalTransformScale => write!(f, "Final Transform → Scale"),

            // Flame
            ConfigPath::RenderMode => write!(f, "Render Mode"),
            ConfigPath::PerspectiveStrength => write!(f, "Perspective Strength"),

            // System Settings
            ConfigPath::SystemIterationsPerThread => write!(f, "System: Iterations Per Thread"),
            ConfigPath::SystemBurnIn => write!(f, "System: Burn-in Iterations"),
            ConfigPath::SystemVsyncEnabled => write!(f, "System: VSync Enabled"),
            ConfigPath::SystemTargetFps => write!(f, "System: Target FPS"),
            ConfigPath::SystemExportWidth => write!(f, "System: Export Width"),
            ConfigPath::SystemExportHeight => write!(f, "System: Export Height"),
            ConfigPath::SystemLanguage => write!(f, "System: Language"),
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
    PathMapStyle(PathMapStyle),
    PathCaptureMode(PathCaptureMode),
    PathTrackingMode(PathTrackingMode),
    RenderMode(RenderMode),
    ToneCurve(ToneCurve),
    Palette(Palette),
}

impl ConfigValue {
    /// Check if two values are approximately equal (for floats)
    pub fn approx_eq(&self, other: &Self) -> bool {
        const EPSILON_F32: f32 = 1e-6;

        match (self, other) {
            (ConfigValue::Float(a), ConfigValue::Float(b)) => (a - b).abs() < EPSILON_F32,
            (ConfigValue::Vec2(x1, y1), ConfigValue::Vec2(x2, y2)) => {
                (x1 - x2).abs() < EPSILON_F32 && (y1 - y2).abs() < EPSILON_F32
            }
            (ConfigValue::ColorRgb(a), ConfigValue::ColorRgb(b)) => a
                .iter()
                .zip(b.iter())
                .all(|(x, y)| (x - y).abs() < EPSILON_F32),
            (ConfigValue::Int(a), ConfigValue::Int(b)) => a == b,
            (ConfigValue::UInt(a), ConfigValue::UInt(b)) => a == b,
            (ConfigValue::UInt64(a), ConfigValue::UInt64(b)) => a == b,
            (ConfigValue::Bool(a), ConfigValue::Bool(b)) => a == b,
            (ConfigValue::String(a), ConfigValue::String(b)) => a == b,
            (ConfigValue::ToneMapMode(a), ConfigValue::ToneMapMode(b)) => a == b,
            (ConfigValue::ColorMode(a), ConfigValue::ColorMode(b)) => a == b,
            (ConfigValue::RenderMode(a), ConfigValue::RenderMode(b)) => a == b,
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
            ConfigValue::PathMapStyle(m) => write!(f, "{:?}", m),
            ConfigValue::PathCaptureMode(m) => write!(f, "{:?}", m),
            ConfigValue::PathTrackingMode(m) => write!(f, "{:?}", m),
            ConfigValue::RenderMode(m) => write!(f, "{:?}", m),
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

impl From<PathMapStyle> for ConfigValue {
    fn from(v: PathMapStyle) -> Self {
        ConfigValue::PathMapStyle(v)
    }
}

impl From<PathCaptureMode> for ConfigValue {
    fn from(v: PathCaptureMode) -> Self {
        ConfigValue::PathCaptureMode(v)
    }
}

impl From<PathTrackingMode> for ConfigValue {
    fn from(v: PathTrackingMode) -> Self {
        ConfigValue::PathTrackingMode(v)
    }
}

impl From<RenderMode> for ConfigValue {
    fn from(v: RenderMode) -> Self {
        ConfigValue::RenderMode(v)
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
            | ConfigPath::PanX
            | ConfigPath::PanY
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
            | ConfigPath::AlphaBlendLow
            | ConfigPath::AlphaBlendHigh
            | ConfigPath::DensityScale
            | ConfigPath::TonemapMode
            | ConfigPath::TonemapCurve
            | ConfigPath::UseCurve
            | ConfigPath::BackgroundColor
            | ConfigPath::BackgroundColorR
            | ConfigPath::BackgroundColorG
            | ConfigPath::BackgroundColorB => UpdateType::ToneMappingOnly,

            // Color parameters - re-run accumulation with new colors
            ConfigPath::ColorMode
            | ConfigPath::PaletteIndex
            | ConfigPath::Palette
            | ConfigPath::PaletteRotation
            | ConfigPath::SpeedFactor
            // PathMapStyle affects color computation in compute shader, needs accumulation reset
            | ConfigPath::PathMapStyle => UpdateType::ColorOnly,

            // PathCaptureMode affects path buffer capture logic in compute shader
            ConfigPath::PathCaptureMode => UpdateType::IterationReset,

            // PathTrackingMode affects path tracking logic in compute shader
            ConfigPath::PathTrackingMode => UpdateType::IterationReset,

            // Rendering settings - affect iteration behavior
            ConfigPath::HistogramColorScale
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
            | ConfigPath::TransformOpacity { .. }
            | ConfigPath::TransformAffine { .. }
            | ConfigPath::TransformVariation { .. }
            | ConfigPath::TransformVariationParam { .. }
            | ConfigPath::TransformOriginX { .. }
            | ConfigPath::TransformOriginY { .. }
            | ConfigPath::TransformRotation { .. }
            | ConfigPath::TransformScale { .. }
            | ConfigPath::FinalTransformEnabled
            | ConfigPath::FinalTransformAffine { .. }
            | ConfigPath::FinalTransformColor
            | ConfigPath::FinalTransformColorSpeed
            | ConfigPath::FinalTransformVariation { .. }
            | ConfigPath::FinalTransformVariationParam { .. }
            | ConfigPath::FinalTransformOriginX
            | ConfigPath::FinalTransformOriginY
            | ConfigPath::FinalTransformRotation
            | ConfigPath::FinalTransformScale
            | ConfigPath::RenderMode
            | ConfigPath::PerspectiveStrength
            | ConfigPath::MaxIterations
            | ConfigPath::DeterministicRng => UpdateType::IterationReset,

            // System Settings
            ConfigPath::SystemIterationsPerThread | ConfigPath::SystemBurnIn => UpdateType::IterationReset,
            ConfigPath::SystemVsyncEnabled | ConfigPath::SystemTargetFps => UpdateType::ViewOnly,
            ConfigPath::SystemExportWidth | ConfigPath::SystemExportHeight | ConfigPath::SystemLanguage => UpdateType::None,
        }
    }

    /// Serialize ConfigPath to a stable string key for animation files
    ///
    /// Format examples:
    /// - Simple: "Zoom", "Exposure", "Pan"
    /// - Transform: "Transform.0.Weight", "Transform.0.Affine.A"
    /// - Variation: "Transform.0.Variation.linear", "Transform.0.VariationParam.julian.power"
    pub fn to_string_key(&self) -> String {
        match self {
            // View
            ConfigPath::Zoom => "Zoom".to_string(),
            ConfigPath::Pan => "Pan".to_string(),
            ConfigPath::PanX => "PanX".to_string(),
            ConfigPath::PanY => "PanY".to_string(),
            ConfigPath::Rotation => "Rotation".to_string(),
            ConfigPath::CameraRotationX => "CameraRotationX".to_string(),
            ConfigPath::CameraRotationY => "CameraRotationY".to_string(),
            ConfigPath::CameraZ => "CameraZ".to_string(),

            // Tone mapping
            ConfigPath::Exposure => "Exposure".to_string(),
            ConfigPath::Gamma => "Gamma".to_string(),
            ConfigPath::GammaThreshold => "GammaThreshold".to_string(),
            ConfigPath::Brightness => "Brightness".to_string(),
            ConfigPath::Vibrancy => "Vibrancy".to_string(),
            ConfigPath::Saturation => "Saturation".to_string(),
            ConfigPath::HueShift => "HueShift".to_string(),
            ConfigPath::ValueScale => "ValueScale".to_string(),
            ConfigPath::AlphaBlendLow => "AlphaBlendLow".to_string(),
            ConfigPath::AlphaBlendHigh => "AlphaBlendHigh".to_string(),
            ConfigPath::DensityScale => "DensityScale".to_string(),
            ConfigPath::TonemapMode => "TonemapMode".to_string(),
            ConfigPath::TonemapCurve => "TonemapCurve".to_string(),
            ConfigPath::UseCurve => "UseCurve".to_string(),

            // Color
            ConfigPath::ColorMode => "ColorMode".to_string(),
            ConfigPath::PathMapStyle => "PathMapStyle".to_string(),
            ConfigPath::PathCaptureMode => "PathCaptureMode".to_string(),
            ConfigPath::PathTrackingMode => "PathTrackingMode".to_string(),
            ConfigPath::PaletteIndex => "PaletteIndex".to_string(),
            ConfigPath::Palette => "Palette".to_string(),
            ConfigPath::PaletteRotation => "PaletteRotation".to_string(),
            ConfigPath::SpeedFactor => "SpeedFactor".to_string(),
            ConfigPath::BackgroundColor => "BackgroundColor".to_string(),
            ConfigPath::BackgroundColorR => "BackgroundColorR".to_string(),
            ConfigPath::BackgroundColorG => "BackgroundColorG".to_string(),
            ConfigPath::BackgroundColorB => "BackgroundColorB".to_string(),

            // Rendering
            ConfigPath::HistogramColorScale => "HistogramColorScale".to_string(),
            ConfigPath::LowDensitySmoothing => "LowDensitySmoothing".to_string(),
            ConfigPath::DensityCompressionStrength => "DensityCompressionStrength".to_string(),
            ConfigPath::BlendFactor => "BlendFactor".to_string(),
            ConfigPath::UseDynamicBlend => "UseDynamicBlend".to_string(),
            ConfigPath::TargetIterationsPerPixel => "TargetIterationsPerPixel".to_string(),
            ConfigPath::MaxIterations => "MaxIterations".to_string(),
            ConfigPath::DeterministicRng => "DeterministicRng".to_string(),

            // Transforms
            ConfigPath::TransformCount => "TransformCount".to_string(),
            ConfigPath::TransformWeight { index } => format!("Transform.{}.Weight", index),
            ConfigPath::TransformColor { index } => format!("Transform.{}.Color", index),
            ConfigPath::TransformColorSpeed { index } => format!("Transform.{}.ColorSpeed", index),
            ConfigPath::TransformOpacity { index } => format!("Transform.{}.Opacity", index),
            ConfigPath::TransformAffine { index, param } => {
                format!("Transform.{}.Affine.{}", index, param.to_char())
            }
            ConfigPath::TransformVariation { index, variation } => {
                format!("Transform.{}.Variation.{}", index, variation)
            }
            ConfigPath::TransformVariationParam { index, variation, param } => {
                format!("Transform.{}.VariationParam.{}.{}", index, variation, param)
            }
            ConfigPath::TransformOriginX { index } => format!("Transform.{}.OriginX", index),
            ConfigPath::TransformOriginY { index } => format!("Transform.{}.OriginY", index),
            ConfigPath::TransformRotation { index } => format!("Transform.{}.Rotation", index),
            ConfigPath::TransformScale { index } => format!("Transform.{}.Scale", index),

            // Final Transform
            ConfigPath::FinalTransformEnabled => "FinalTransform.Enabled".to_string(),
            ConfigPath::FinalTransformAffine { param } => {
                format!("FinalTransform.Affine.{}", param.to_char())
            }
            ConfigPath::FinalTransformColor => "FinalTransform.Color".to_string(),
            ConfigPath::FinalTransformColorSpeed => "FinalTransform.ColorSpeed".to_string(),
            ConfigPath::FinalTransformVariation { variation } => {
                format!("FinalTransform.Variation.{}", variation)
            }
            ConfigPath::FinalTransformVariationParam { variation, param } => {
                format!("FinalTransform.VariationParam.{}.{}", variation, param)
            }
            ConfigPath::FinalTransformOriginX => "FinalTransform.OriginX".to_string(),
            ConfigPath::FinalTransformOriginY => "FinalTransform.OriginY".to_string(),
            ConfigPath::FinalTransformRotation => "FinalTransform.Rotation".to_string(),
            ConfigPath::FinalTransformScale => "FinalTransform.Scale".to_string(),

            // Flame
            ConfigPath::RenderMode => "RenderMode".to_string(),
            ConfigPath::PerspectiveStrength => "PerspectiveStrength".to_string(),

            // System Settings (not typically animated, but included for completeness)
            ConfigPath::SystemIterationsPerThread => "System.IterationsPerThread".to_string(),
            ConfigPath::SystemBurnIn => "System.BurnIn".to_string(),
            ConfigPath::SystemVsyncEnabled => "System.VsyncEnabled".to_string(),
            ConfigPath::SystemTargetFps => "System.TargetFps".to_string(),
            ConfigPath::SystemExportWidth => "System.ExportWidth".to_string(),
            ConfigPath::SystemExportHeight => "System.ExportHeight".to_string(),
            ConfigPath::SystemLanguage => "System.Language".to_string(),
        }
    }

    /// Parse ConfigPath from string key (inverse of to_string_key)
    ///
    /// Returns None if the string doesn't match a valid path format
    pub fn from_string_key(s: &str) -> Option<Self> {
        // Simple paths (no dots)
        match s {
            // View
            "Zoom" => return Some(ConfigPath::Zoom),
            "Pan" => return Some(ConfigPath::Pan),
            "PanX" => return Some(ConfigPath::PanX),
            "PanY" => return Some(ConfigPath::PanY),
            "Rotation" => return Some(ConfigPath::Rotation),
            "CameraRotationX" => return Some(ConfigPath::CameraRotationX),
            "CameraRotationY" => return Some(ConfigPath::CameraRotationY),
            "CameraZ" => return Some(ConfigPath::CameraZ),

            // Tone mapping
            "Exposure" => return Some(ConfigPath::Exposure),
            "Gamma" => return Some(ConfigPath::Gamma),
            "GammaThreshold" => return Some(ConfigPath::GammaThreshold),
            "Brightness" => return Some(ConfigPath::Brightness),
            "Vibrancy" => return Some(ConfigPath::Vibrancy),
            "Saturation" => return Some(ConfigPath::Saturation),
            "HueShift" => return Some(ConfigPath::HueShift),
            "ValueScale" => return Some(ConfigPath::ValueScale),
            "AlphaBlendLow" => return Some(ConfigPath::AlphaBlendLow),
            "AlphaBlendHigh" => return Some(ConfigPath::AlphaBlendHigh),
            "DensityScale" => return Some(ConfigPath::DensityScale),
            "TonemapMode" => return Some(ConfigPath::TonemapMode),
            "TonemapCurve" => return Some(ConfigPath::TonemapCurve),
            "UseCurve" => return Some(ConfigPath::UseCurve),

            // Color
            "ColorMode" => return Some(ConfigPath::ColorMode),
            "PaletteIndex" => return Some(ConfigPath::PaletteIndex),
            "Palette" => return Some(ConfigPath::Palette),
            "PaletteRotation" => return Some(ConfigPath::PaletteRotation),
            "SpeedFactor" => return Some(ConfigPath::SpeedFactor),
            "BackgroundColor" => return Some(ConfigPath::BackgroundColor),
            "BackgroundColorR" => return Some(ConfigPath::BackgroundColorR),
            "BackgroundColorG" => return Some(ConfigPath::BackgroundColorG),
            "BackgroundColorB" => return Some(ConfigPath::BackgroundColorB),

            // Rendering
            "HistogramColorScale" => return Some(ConfigPath::HistogramColorScale),
            "LowDensitySmoothing" => return Some(ConfigPath::LowDensitySmoothing),
            "DensityCompressionStrength" => return Some(ConfigPath::DensityCompressionStrength),
            "BlendFactor" => return Some(ConfigPath::BlendFactor),
            "UseDynamicBlend" => return Some(ConfigPath::UseDynamicBlend),
            "TargetIterationsPerPixel" => return Some(ConfigPath::TargetIterationsPerPixel),
            "MaxIterations" => return Some(ConfigPath::MaxIterations),
            "DeterministicRng" => return Some(ConfigPath::DeterministicRng),

            // Flame
            "TransformCount" => return Some(ConfigPath::TransformCount),
            "RenderMode" => return Some(ConfigPath::RenderMode),
            "PerspectiveStrength" => return Some(ConfigPath::PerspectiveStrength),

            _ => {}
        }

        // Parse compound paths with dots
        let parts: Vec<&str> = s.split('.').collect();

        // Transform paths: Transform.{index}.{field}...
        if parts.len() >= 3 && parts[0] == "Transform" {
            let index: usize = parts[1].parse().ok()?;

            match parts[2] {
                "Weight" => return Some(ConfigPath::TransformWeight { index }),
                "Color" => return Some(ConfigPath::TransformColor { index }),
                "ColorSpeed" => return Some(ConfigPath::TransformColorSpeed { index }),
                "Opacity" => return Some(ConfigPath::TransformOpacity { index }),
                "Affine" if parts.len() == 4 => {
                    let param = AffineParam::from_char(parts[3].chars().next()?)?;
                    return Some(ConfigPath::TransformAffine { index, param });
                }
                "Variation" if parts.len() == 4 => {
                    return Some(ConfigPath::TransformVariation {
                        index,
                        variation: parts[3].to_string(),
                    });
                }
                "VariationParam" if parts.len() == 5 => {
                    return Some(ConfigPath::TransformVariationParam {
                        index,
                        variation: parts[3].to_string(),
                        param: parts[4].to_string(),
                    });
                }
                "OriginX" => return Some(ConfigPath::TransformOriginX { index }),
                "OriginY" => return Some(ConfigPath::TransformOriginY { index }),
                "Rotation" => return Some(ConfigPath::TransformRotation { index }),
                "Scale" => return Some(ConfigPath::TransformScale { index }),
                _ => {}
            }
        }

        // FinalTransform paths
        if parts.len() >= 2 && parts[0] == "FinalTransform" {
            match parts[1] {
                "Enabled" => return Some(ConfigPath::FinalTransformEnabled),
                "Color" => return Some(ConfigPath::FinalTransformColor),
                "ColorSpeed" => return Some(ConfigPath::FinalTransformColorSpeed),
                "Affine" if parts.len() == 3 => {
                    let param = AffineParam::from_char(parts[2].chars().next()?)?;
                    return Some(ConfigPath::FinalTransformAffine { param });
                }
                "Variation" if parts.len() == 3 => {
                    return Some(ConfigPath::FinalTransformVariation {
                        variation: parts[2].to_string(),
                    });
                }
                "VariationParam" if parts.len() == 4 => {
                    return Some(ConfigPath::FinalTransformVariationParam {
                        variation: parts[2].to_string(),
                        param: parts[3].to_string(),
                    });
                }
                "OriginX" => return Some(ConfigPath::FinalTransformOriginX),
                "OriginY" => return Some(ConfigPath::FinalTransformOriginY),
                "Rotation" => return Some(ConfigPath::FinalTransformRotation),
                "Scale" => return Some(ConfigPath::FinalTransformScale),
                _ => {}
            }
        }

        // System paths
        if parts.len() == 2 && parts[0] == "System" {
            match parts[1] {
                "IterationsPerThread" => return Some(ConfigPath::SystemIterationsPerThread),
                "BurnIn" => return Some(ConfigPath::SystemBurnIn),
                "VsyncEnabled" => return Some(ConfigPath::SystemVsyncEnabled),
                "TargetFps" => return Some(ConfigPath::SystemTargetFps),
                "ExportWidth" => return Some(ConfigPath::SystemExportWidth),
                "ExportHeight" => return Some(ConfigPath::SystemExportHeight),
                "Language" => return Some(ConfigPath::SystemLanguage),
                _ => {}
            }
        }

        None
    }
}

impl AffineParam {
    /// Convert to single character representation
    pub fn to_char(&self) -> char {
        match self {
            AffineParam::A => 'A',
            AffineParam::B => 'B',
            AffineParam::C => 'C',
            AffineParam::D => 'D',
            AffineParam::E => 'E',
            AffineParam::F => 'F',
            AffineParam::G => 'G',
        }
    }

    /// Parse from single character
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'A' | 'a' => Some(AffineParam::A),
            'B' | 'b' => Some(AffineParam::B),
            'C' | 'c' => Some(AffineParam::C),
            'D' | 'd' => Some(AffineParam::D),
            'E' | 'e' => Some(AffineParam::E),
            'F' | 'f' => Some(AffineParam::F),
            'G' | 'g' => Some(AffineParam::G),
            _ => None,
        }
    }
}

/// Convert JSON value to ConfigValue based on the expected type for a ConfigPath
///
/// This is used by the animation system to convert interpolated JSON values
/// back to strongly-typed ConfigValues for the ConfigManager.
///
/// # Arguments
/// * `json` - The JSON value to convert
/// * `path` - The ConfigPath that determines the expected type
///
/// # Returns
/// * `Some(ConfigValue)` on successful conversion
/// * `None` if the JSON value cannot be converted to the expected type
pub fn json_to_config_value(json: &serde_json::Value, path: &ConfigPath) -> Option<ConfigValue> {
    use serde_json::Value;

    match path {
        // Float parameters
        ConfigPath::Zoom
        | ConfigPath::PanX
        | ConfigPath::PanY
        | ConfigPath::Rotation
        | ConfigPath::CameraRotationX
        | ConfigPath::CameraRotationY
        | ConfigPath::CameraZ
        | ConfigPath::Exposure
        | ConfigPath::Gamma
        | ConfigPath::GammaThreshold
        | ConfigPath::Brightness
        | ConfigPath::Vibrancy
        | ConfigPath::Saturation
        | ConfigPath::HueShift
        | ConfigPath::ValueScale
        | ConfigPath::AlphaBlendLow
        | ConfigPath::AlphaBlendHigh
        | ConfigPath::DensityScale
        | ConfigPath::PaletteRotation
        | ConfigPath::SpeedFactor
        | ConfigPath::BackgroundColorR
        | ConfigPath::BackgroundColorG
        | ConfigPath::BackgroundColorB
        | ConfigPath::HistogramColorScale
        | ConfigPath::LowDensitySmoothing
        | ConfigPath::DensityCompressionStrength
        | ConfigPath::BlendFactor
        | ConfigPath::PerspectiveStrength
        | ConfigPath::TransformWeight { .. }
        | ConfigPath::TransformColor { .. }
        | ConfigPath::TransformColorSpeed { .. }
        | ConfigPath::TransformOpacity { .. }
        | ConfigPath::TransformAffine { .. }
        | ConfigPath::TransformVariation { .. }
        | ConfigPath::TransformVariationParam { .. }
        | ConfigPath::TransformOriginX { .. }
        | ConfigPath::TransformOriginY { .. }
        | ConfigPath::TransformRotation { .. }
        | ConfigPath::TransformScale { .. }
        | ConfigPath::FinalTransformAffine { .. }
        | ConfigPath::FinalTransformColor
        | ConfigPath::FinalTransformColorSpeed
        | ConfigPath::FinalTransformVariation { .. }
        | ConfigPath::FinalTransformVariationParam { .. }
        | ConfigPath::FinalTransformOriginX
        | ConfigPath::FinalTransformOriginY
        | ConfigPath::FinalTransformRotation
        | ConfigPath::FinalTransformScale
        | ConfigPath::SystemTargetFps => {
            json.as_f64().map(|f| ConfigValue::Float(f as f32))
        }

        // Vec2 (pan coordinates)
        ConfigPath::Pan => {
            if let Value::Array(arr) = json {
                if arr.len() == 2 {
                    let x = arr[0].as_f64()? as f32;
                    let y = arr[1].as_f64()? as f32;
                    return Some(ConfigValue::Vec2(x, y));
                }
            }
            None
        }

        // RGB color
        ConfigPath::BackgroundColor => {
            if let Value::Array(arr) = json {
                if arr.len() == 3 {
                    let r = arr[0].as_f64()? as f32;
                    let g = arr[1].as_f64()? as f32;
                    let b = arr[2].as_f64()? as f32;
                    return Some(ConfigValue::ColorRgb([r, g, b]));
                }
            }
            None
        }

        // Boolean parameters
        ConfigPath::UseCurve
        | ConfigPath::UseDynamicBlend
        | ConfigPath::DeterministicRng
        | ConfigPath::FinalTransformEnabled
        | ConfigPath::SystemVsyncEnabled => {
            json.as_bool().map(ConfigValue::Bool)
        }

        // UInt parameters
        ConfigPath::PaletteIndex
        | ConfigPath::TargetIterationsPerPixel
        | ConfigPath::TransformCount
        | ConfigPath::SystemIterationsPerThread
        | ConfigPath::SystemBurnIn
        | ConfigPath::SystemExportWidth
        | ConfigPath::SystemExportHeight => {
            json.as_u64().map(|u| ConfigValue::UInt(u as u32))
        }

        // UInt64 parameters
        ConfigPath::MaxIterations => {
            json.as_u64().map(ConfigValue::UInt64)
        }

        // String parameters
        ConfigPath::SystemLanguage => {
            json.as_str().map(|s| ConfigValue::String(s.to_string()))
        }

        // Enum types (need string parsing)
        ConfigPath::TonemapMode => {
            if let Some(s) = json.as_str() {
                match s {
                    "Linear" => Some(ConfigValue::ToneMapMode(ToneMapMode::Linear)),
                    "Logarithmic" => Some(ConfigValue::ToneMapMode(ToneMapMode::Logarithmic)),
                    _ => None,
                }
            } else {
                None
            }
        }

        ConfigPath::ColorMode => {
            if let Some(s) = json.as_str() {
                match s {
                    "Palette" => Some(ConfigValue::ColorMode(ColorMode::Palette)),
                    "Speed" => Some(ConfigValue::ColorMode(ColorMode::Speed)),
                    "PathMap" => Some(ConfigValue::ColorMode(ColorMode::PathMap)),
                    _ => None,
                }
            } else {
                None
            }
        }

        ConfigPath::PathMapStyle => {
            if let Some(s) = json.as_str() {
                match s {
                    "Prefix" => Some(ConfigValue::PathMapStyle(PathMapStyle::Prefix)),
                    "Suffix" => Some(ConfigValue::PathMapStyle(PathMapStyle::Suffix)),
                    "PrefixDistinct" => Some(ConfigValue::PathMapStyle(PathMapStyle::PrefixDistinct)),
                    "SuffixDistinct" => Some(ConfigValue::PathMapStyle(PathMapStyle::SuffixDistinct)),
                    // Backward compatibility with old config files
                    "Similar" => Some(ConfigValue::PathMapStyle(PathMapStyle::Prefix)),
                    "Distinct" => Some(ConfigValue::PathMapStyle(PathMapStyle::PrefixDistinct)),
                    "ScrambledPrefix" => Some(ConfigValue::PathMapStyle(PathMapStyle::PrefixDistinct)),
                    "ScrambledSuffix" => Some(ConfigValue::PathMapStyle(PathMapStyle::SuffixDistinct)),
                    _ => None,
                }
            } else {
                None
            }
        }

        ConfigPath::PathCaptureMode => {
            if let Some(s) = json.as_str() {
                match s {
                    "FirstHit" => Some(ConfigValue::PathCaptureMode(PathCaptureMode::FirstHit)),
                    "FirstAfterBurnIn" => Some(ConfigValue::PathCaptureMode(PathCaptureMode::FirstAfterBurnIn)),
                    "LastHit" => Some(ConfigValue::PathCaptureMode(PathCaptureMode::LastHit)),
                    _ => None,
                }
            } else {
                None
            }
        }

        ConfigPath::PathTrackingMode => {
            if let Some(s) = json.as_str() {
                match s {
                    "First" => Some(ConfigValue::PathTrackingMode(PathTrackingMode::First)),
                    "Recent" => Some(ConfigValue::PathTrackingMode(PathTrackingMode::Recent)),
                    _ => None,
                }
            } else {
                None
            }
        }

        ConfigPath::RenderMode => {
            if let Some(s) = json.as_str() {
                match s {
                    "TwoD" => Some(ConfigValue::RenderMode(RenderMode::TwoD)),
                    "ThreeD" => Some(ConfigValue::RenderMode(RenderMode::ThreeD)),
                    _ => None,
                }
            } else {
                None
            }
        }

        // Complex types not supported for animation (yet)
        ConfigPath::TonemapCurve | ConfigPath::Palette => None,
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
            ConfigDelta::new(ConfigPath::Pan, (0.0, 0.0).into(), (1.0, 0.0).into()),
        ];

        let change = ConfigChange::batch(deltas, "Reset View".to_string());

        assert_eq!(change.deltas.len(), 2);
        assert_eq!(change.description, "Reset View");
        assert_eq!(change.update_type(), UpdateType::ViewOnly);
    }

    #[test]
    fn test_config_path_string_roundtrip_simple() {
        // Test simple paths
        let paths = vec![
            ConfigPath::Zoom,
            ConfigPath::Pan,
            ConfigPath::Rotation,
            ConfigPath::Exposure,
            ConfigPath::Gamma,
            ConfigPath::Brightness,
            ConfigPath::ColorMode,
            ConfigPath::RenderMode,
        ];

        for path in paths {
            let key = path.to_string_key();
            let parsed = ConfigPath::from_string_key(&key);
            assert_eq!(parsed, Some(path.clone()), "Failed roundtrip for {:?}", path);
        }
    }

    #[test]
    fn test_config_path_string_roundtrip_transform() {
        // Test transform paths
        let paths = vec![
            ConfigPath::TransformWeight { index: 0 },
            ConfigPath::TransformWeight { index: 5 },
            ConfigPath::TransformColor { index: 2 },
            ConfigPath::TransformAffine { index: 1, param: AffineParam::A },
            ConfigPath::TransformAffine { index: 3, param: AffineParam::G },
            ConfigPath::TransformVariation { index: 0, variation: "linear".to_string() },
            ConfigPath::TransformVariation { index: 2, variation: "sinusoidal".to_string() },
            ConfigPath::TransformVariationParam {
                index: 0,
                variation: "julian".to_string(),
                param: "power".to_string(),
            },
        ];

        for path in paths {
            let key = path.to_string_key();
            let parsed = ConfigPath::from_string_key(&key);
            assert_eq!(parsed, Some(path.clone()), "Failed roundtrip for key: {}", key);
        }
    }

    #[test]
    fn test_config_path_string_roundtrip_transform_operations() {
        // Test high-level transform operation paths (origin, rotation, scale)
        let paths = vec![
            ConfigPath::TransformOriginX { index: 0 },
            ConfigPath::TransformOriginX { index: 3 },
            ConfigPath::TransformOriginY { index: 0 },
            ConfigPath::TransformOriginY { index: 5 },
            ConfigPath::TransformRotation { index: 1 },
            ConfigPath::TransformRotation { index: 2 },
            ConfigPath::TransformScale { index: 0 },
            ConfigPath::TransformScale { index: 4 },
        ];

        for path in paths {
            let key = path.to_string_key();
            let parsed = ConfigPath::from_string_key(&key);
            assert_eq!(parsed, Some(path.clone()), "Failed roundtrip for key: {}", key);
        }
    }

    #[test]
    fn test_config_path_string_roundtrip_final_transform() {
        let paths = vec![
            ConfigPath::FinalTransformEnabled,
            ConfigPath::FinalTransformAffine { param: AffineParam::E },
            ConfigPath::FinalTransformColor,
            ConfigPath::FinalTransformVariation { variation: "spherical".to_string() },
        ];

        for path in paths {
            let key = path.to_string_key();
            let parsed = ConfigPath::from_string_key(&key);
            assert_eq!(parsed, Some(path.clone()), "Failed roundtrip for key: {}", key);
        }
    }

    #[test]
    fn test_affine_param_char_roundtrip() {
        let params = vec![
            AffineParam::A,
            AffineParam::B,
            AffineParam::C,
            AffineParam::D,
            AffineParam::E,
            AffineParam::F,
            AffineParam::G,
        ];

        for param in params {
            let c = param.to_char();
            let parsed = AffineParam::from_char(c);
            assert_eq!(parsed, Some(param));
        }
    }
}
