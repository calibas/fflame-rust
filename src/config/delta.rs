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
    DofFocusDistance,
    DofBlurStrength,
    FogStrength,
    FogStart,

    // ===== Tone mapping (no iteration reset needed) =====
    Exposure,
    Gamma,
    GammaThreshold,
    Brightness,
    Vibrancy,
    WhiteLevel,
    Saturation,
    HueShift,
    AlphaBlendLow,
    AlphaBlendHigh,
    DensityScale,
    TonemapMode,
    HighlightMode,
    TonemapCurve,
    UseCurve,
    // Levels controls (density-to-opacity mapping)
    LevelsEnabled,
    LevelsLow,
    LevelsHigh,
    LevelsGamma,

    // ===== Color (no iteration reset, just color buffer update) =====
    ColorMode,
    PathMapStyle,  // Prefix or Suffix coloring for PathMap mode
    PathCaptureMode,  // FirstHit, FirstAfterBurnIn, or LastHit
    PathTrackingMode,  // First (first 32 iterations) or Recent (rolling window of 32 most recent)
    PaletteIndex,
    Palette, // Embedded palette data (custom palettes)
    PaletteRotation,
    PaletteSize, // Palette texture resolution (256-4096)
    PaletteSqueeze, // Palette squeeze: 1.0 = normal, >1 = repeat, <1 = portion
    PaletteSqueezeMode, // Linear vs Geometric squeeze algorithm
    PaletteSqueezeFalloff, // Geometric squeeze octave ratio (typically ~0.5)
    PaletteLogStrength, // Exponential redistribution of the squeezed palette
    PaletteReverse, // Flip the resulting palette lookup table (toggle)
    SpeedFactor,
    BackgroundColor,
    BackgroundColorR, // Separate R component for animation
    BackgroundColorG, // Separate G component for animation
    BackgroundColorB, // Separate B component for animation

    // ===== Rendering settings (affects iteration speed/quality) =====
    BlendFactor,
    UseDynamicBlend,
    MaxIterations,
    DeterministicRng,

    // ===== Transform-level changes (require iteration reset) =====
    TransformCount,
    TransformWeight { index: usize },
    TransformColor { index: usize },
    TransformColorSpeed { index: usize },
    TransformOpacity { index: usize },
    TransformDirectColor { index: usize },
    TransformAffine { index: usize, param: AffineParam },
    TransformVariation { index: usize, variation: String },
    TransformVariationParam {
        index: usize,
        variation: String,
        param: String,
    },
    /// Post-affine enabled flag for a transform
    TransformPostAffineEnabled { index: usize },
    /// Post-affine transformation parameter for a transform
    TransformPostAffine { index: usize, param: AffineParam },
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

    // High-level ops on the post-affine half of a normal-pool
    // transform. Same decomposition as the pre-affine versions
    // above, applied to post_a..post_f.
    TransformPostAffineOriginX { index: usize },
    TransformPostAffineOriginY { index: usize },
    TransformPostAffineRotation { index: usize },
    TransformPostAffineScale { index: usize },

    // ===== Linked Transform pool (require iteration reset) =====
    // Linked transforms run sequentially after a normal transform's
    // variations and feed forward into the next iteration. Pool members
    // are referenced by index; multiple normals can share via
    // attachment lists. See per-transform-linked-and-final.md.
    LinkedTransformAffine { index: usize, param: AffineParam },
    LinkedTransformPostAffineEnabled { index: usize },
    LinkedTransformPostAffine { index: usize, param: AffineParam },
    LinkedTransformVariation { index: usize, variation: String },
    LinkedTransformVariationParam {
        index: usize,
        variation: String,
        param: String,
    },
    // High-level pre-affine ops on a linked-pool transform.
    LinkedTransformOriginX { index: usize },
    LinkedTransformOriginY { index: usize },
    LinkedTransformRotation { index: usize },
    LinkedTransformScale { index: usize },
    // High-level post-affine ops on a linked-pool transform.
    LinkedTransformPostAffineOriginX { index: usize },
    LinkedTransformPostAffineOriginY { index: usize },
    LinkedTransformPostAffineRotation { index: usize },
    LinkedTransformPostAffineScale { index: usize },

    // ===== Final Transform pool (require iteration reset) =====
    // Final transforms run sequentially after the Linked chain to
    // shape the plotted point only (output not fed forward — pure
    // filter). Pool members referenced by index.
    //
    // Legacy `FinalTransform*` variants (no index) lived here before
    // the per-pool model — they routed to `final_transforms[0]` and
    // existed to keep older animation tracks loadable. Removed in
    // Phase 9 of the unified-render-pipeline branch; the migration
    // shim in `from_string_key` keeps any saved tracks targeting
    // the legacy `FinalTransform.{field}` strings working by
    // mapping them to index 0.
    FinalTransformAffine { index: usize, param: AffineParam },
    FinalTransformPostAffineEnabled { index: usize },
    FinalTransformPostAffine { index: usize, param: AffineParam },
    FinalTransformVariation { index: usize, variation: String },
    FinalTransformVariationParam {
        index: usize,
        variation: String,
        param: String,
    },
    // High-level pre-affine ops on a final-pool transform.
    FinalTransformOriginX { index: usize },
    FinalTransformOriginY { index: usize },
    FinalTransformRotation { index: usize },
    FinalTransformScale { index: usize },
    // High-level post-affine ops on a final-pool transform.
    FinalTransformPostAffineOriginX { index: usize },
    FinalTransformPostAffineOriginY { index: usize },
    FinalTransformPostAffineRotation { index: usize },
    FinalTransformPostAffineScale { index: usize },

    // ===== Flame-level (require iteration reset) =====
    RenderMode,
    PerspectiveStrength,
    /// Xaos (chaos) weight for transition from src transform to dst transform
    /// Modifies the probability of selecting dst when coming from src
    Xaos { src: usize, dst: usize },
    /// Solo transform index (0-indexed). When Some(n), only transform n is active,
    /// all others effectively have weight 0. Used for debugging.
    /// Matches Apophysis XML: soloxform="N"
    SoloTransform,

    // ===== Effects (post-processing, no iteration reset needed) =====
    /// Enable/disable a density effect
    DensityEffectEnabled { index: usize },
    /// Parameter value for a density effect
    DensityEffectParam { index: usize, param: String },
    /// Enable/disable a color effect
    ColorEffectEnabled { index: usize },
    /// Parameter value for a color effect
    ColorEffectParam { index: usize, param: String },
    /// Add a new color effect
    AddColorEffect { effect_type: String },
    /// Remove a color effect by index
    RemoveColorEffect { index: usize },
    /// Add a new density effect
    AddDensityEffect { effect_type: String },
    /// Remove a density effect by index
    RemoveDensityEffect { index: usize },

    // ===== System Settings (device-specific, not tracked for undo) =====
    SystemIterationsPerThread,
    SystemBurnIn,
    SystemVsyncEnabled,
    SystemTargetFps,
    SystemExportWidth,
    SystemExportHeight,
    SystemLanguage,
    SystemShowHelpOnStartup,
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

/// Identifies a specific transform across the three pools.
/// Used by reusable UI render fns to construct the right ConfigPath
/// variant and read from the right pool when displaying / editing
/// affine, post-affine, variations, and variation_params.
///
/// See `docs/projects/per-transform-linked-and-final.md`.
/// Which transform pool a path-or-action targets.
/// Use `TransformRef` when an index is also needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TransformKind {
    Normal,
    Linked,
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TransformRef {
    Normal(usize),
    Linked(usize),
    Final(usize),
}

impl TransformKind {
    pub fn at(self, index: usize) -> TransformRef {
        match self {
            TransformKind::Normal => TransformRef::Normal(index),
            TransformKind::Linked => TransformRef::Linked(index),
            TransformKind::Final => TransformRef::Final(index),
        }
    }
}

impl TransformRef {
    pub fn index(&self) -> usize {
        match self {
            TransformRef::Normal(i) | TransformRef::Linked(i) | TransformRef::Final(i) => *i,
        }
    }

    pub fn kind(&self) -> TransformKind {
        match self {
            TransformRef::Normal(_) => TransformKind::Normal,
            TransformRef::Linked(_) => TransformKind::Linked,
            TransformRef::Final(_) => TransformKind::Final,
        }
    }

    /// Short tag used to disambiguate egui id_salts and log strings across pools.
    pub fn pool_kind(&self) -> &'static str {
        match self {
            TransformRef::Normal(_) => "normal",
            TransformRef::Linked(_) => "linked",
            TransformRef::Final(_) => "final",
        }
    }

    /// Look up the corresponding transform in the flame.
    pub fn get<'a>(&self, flame: &'a crate::scene::transforms::Flame)
        -> Option<&'a crate::scene::transforms::Transform>
    {
        match self {
            TransformRef::Normal(i) => flame.transforms.get(*i),
            TransformRef::Linked(i) => flame.linked_transforms.get(*i),
            TransformRef::Final(i) => flame.final_transforms.get(*i),
        }
    }

    /// Mutable lookup of the corresponding transform in the flame.
    pub fn get_mut<'a>(&self, flame: &'a mut crate::scene::transforms::Flame)
        -> Option<&'a mut crate::scene::transforms::Transform>
    {
        match self {
            TransformRef::Normal(i) => flame.transforms.get_mut(*i),
            TransformRef::Linked(i) => flame.linked_transforms.get_mut(*i),
            TransformRef::Final(i) => flame.final_transforms.get_mut(*i),
        }
    }

    pub fn affine_path(&self, param: AffineParam) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformAffine { index: *i, param },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformAffine { index: *i, param },
            TransformRef::Final(i) => ConfigPath::FinalTransformAffine { index: *i, param },
        }
    }

    pub fn post_affine_enabled_path(&self) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformPostAffineEnabled { index: *i },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformPostAffineEnabled { index: *i },
            TransformRef::Final(i) => ConfigPath::FinalTransformPostAffineEnabled { index: *i },
        }
    }

    pub fn post_affine_path(&self, param: AffineParam) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformPostAffine { index: *i, param },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformPostAffine { index: *i, param },
            TransformRef::Final(i) => ConfigPath::FinalTransformPostAffine { index: *i, param },
        }
    }

    pub fn variation_path(&self, variation: String) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformVariation { index: *i, variation },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformVariation { index: *i, variation },
            TransformRef::Final(i) => ConfigPath::FinalTransformVariation { index: *i, variation },
        }
    }

    pub fn variation_param_path(&self, variation: String, param: String) -> ConfigPath {
        match self {
            TransformRef::Normal(i) => ConfigPath::TransformVariationParam {
                index: *i, variation, param,
            },
            TransformRef::Linked(i) => ConfigPath::LinkedTransformVariationParam {
                index: *i, variation, param,
            },
            TransformRef::Final(i) => ConfigPath::FinalTransformVariationParam {
                index: *i, variation, param,
            },
        }
    }
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
            ConfigPath::DofFocusDistance => write!(f, "DOF Focus Distance"),
            ConfigPath::DofBlurStrength => write!(f, "DOF Blur Strength"),
            ConfigPath::FogStrength => write!(f, "Fog Strength"),
            ConfigPath::FogStart => write!(f, "Fog Start"),

            // Tone mapping
            ConfigPath::Exposure => write!(f, "Exposure"),
            ConfigPath::Gamma => write!(f, "Gamma"),
            ConfigPath::GammaThreshold => write!(f, "Gamma Threshold"),
            ConfigPath::Brightness => write!(f, "Brightness"),
            ConfigPath::Vibrancy => write!(f, "Vibrancy"),
            ConfigPath::WhiteLevel => write!(f, "Highlights"),
            ConfigPath::Saturation => write!(f, "Saturation"),
            ConfigPath::HueShift => write!(f, "Hue Shift"),
            ConfigPath::AlphaBlendLow => write!(f, "Alpha Blend Low"),
            ConfigPath::AlphaBlendHigh => write!(f, "Alpha Blend High"),
            ConfigPath::DensityScale => write!(f, "Density Scale"),
            ConfigPath::TonemapMode => write!(f, "Tonemap Mode"),
            ConfigPath::HighlightMode => write!(f, "Highlight Mode"),
            ConfigPath::TonemapCurve => write!(f, "Tone Curve"),
            ConfigPath::UseCurve => write!(f, "Use Tone Curve"),
            ConfigPath::LevelsEnabled => write!(f, "Levels Enabled"),
            ConfigPath::LevelsLow => write!(f, "Levels Low"),
            ConfigPath::LevelsHigh => write!(f, "Levels High"),
            ConfigPath::LevelsGamma => write!(f, "Levels Midtones"),

            // Color
            ConfigPath::ColorMode => write!(f, "Color Mode"),
            ConfigPath::PathMapStyle => write!(f, "PathMap Style"),
            ConfigPath::PathCaptureMode => write!(f, "PathMap Capture Mode"),
            ConfigPath::PathTrackingMode => write!(f, "PathMap Tracking Mode"),
            ConfigPath::PaletteIndex => write!(f, "Palette"),
            ConfigPath::Palette => write!(f, "Palette Data"),
            ConfigPath::PaletteRotation => write!(f, "Palette Rotation"),
            ConfigPath::PaletteSize => write!(f, "Palette Size"),
            ConfigPath::PaletteSqueeze => write!(f, "Palette Squeeze"),
            ConfigPath::PaletteSqueezeMode => write!(f, "Palette Squeeze Mode"),
            ConfigPath::PaletteSqueezeFalloff => write!(f, "Palette Squeeze Falloff"),
            ConfigPath::PaletteLogStrength => write!(f, "Palette Log Strength"),
            ConfigPath::PaletteReverse => write!(f, "Palette Reverse"),
            ConfigPath::SpeedFactor => write!(f, "Speed Blend Factor"),
            ConfigPath::BackgroundColor => write!(f, "Background Color"),
            ConfigPath::BackgroundColorR => write!(f, "Background Red"),
            ConfigPath::BackgroundColorG => write!(f, "Background Green"),
            ConfigPath::BackgroundColorB => write!(f, "Background Blue"),

            // Rendering
            ConfigPath::BlendFactor => write!(f, "Blend Factor"),
            ConfigPath::UseDynamicBlend => write!(f, "Use Dynamic Blend"),
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
            ConfigPath::TransformDirectColor { index } => {
                write!(f, "Transform {} → Direct Color", index + 1)
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
            ConfigPath::TransformPostAffineEnabled { index } => {
                write!(f, "Transform {} → Post-Affine Enabled", index + 1)
            }
            ConfigPath::TransformPostAffineOriginX { index } => {
                write!(f, "Transform {} → Post-Affine Origin X", index + 1)
            }
            ConfigPath::TransformPostAffineOriginY { index } => {
                write!(f, "Transform {} → Post-Affine Origin Y", index + 1)
            }
            ConfigPath::TransformPostAffineRotation { index } => {
                write!(f, "Transform {} → Post-Affine Rotation", index + 1)
            }
            ConfigPath::TransformPostAffineScale { index } => {
                write!(f, "Transform {} → Post-Affine Scale", index + 1)
            }
            ConfigPath::TransformPostAffine { index, param } => {
                write!(f, "Transform {} → Post-Affine {:?}", index + 1, param)
            }

            // Linked Transform pool
            ConfigPath::LinkedTransformAffine { index, param } => {
                write!(f, "Linked Transform {} → Affine {:?}", index + 1, param)
            }
            ConfigPath::LinkedTransformPostAffineEnabled { index } => {
                write!(f, "Linked Transform {} → Post-Affine Enabled", index + 1)
            }
            ConfigPath::LinkedTransformPostAffine { index, param } => {
                write!(f, "Linked Transform {} → Post-Affine {:?}", index + 1, param)
            }
            ConfigPath::LinkedTransformVariation { index, variation } => {
                write!(f, "Linked Transform {} → {} variation", index + 1, variation)
            }
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => {
                write!(f, "Linked Transform {} → {} → {}", index + 1, variation, param)
            }
            ConfigPath::LinkedTransformOriginX { index } => {
                write!(f, "Linked Transform {} → Origin X", index + 1)
            }
            ConfigPath::LinkedTransformOriginY { index } => {
                write!(f, "Linked Transform {} → Origin Y", index + 1)
            }
            ConfigPath::LinkedTransformRotation { index } => {
                write!(f, "Linked Transform {} → Rotation", index + 1)
            }
            ConfigPath::LinkedTransformScale { index } => {
                write!(f, "Linked Transform {} → Scale", index + 1)
            }
            ConfigPath::LinkedTransformPostAffineOriginX { index } => {
                write!(f, "Linked Transform {} → Post-Affine Origin X", index + 1)
            }
            ConfigPath::LinkedTransformPostAffineOriginY { index } => {
                write!(f, "Linked Transform {} → Post-Affine Origin Y", index + 1)
            }
            ConfigPath::LinkedTransformPostAffineRotation { index } => {
                write!(f, "Linked Transform {} → Post-Affine Rotation", index + 1)
            }
            ConfigPath::LinkedTransformPostAffineScale { index } => {
                write!(f, "Linked Transform {} → Post-Affine Scale", index + 1)
            }

            // Final Transform pool
            ConfigPath::FinalTransformAffine { index, param } => {
                write!(f, "Final Transform {} → Affine {:?}", index + 1, param)
            }
            ConfigPath::FinalTransformPostAffineEnabled { index } => {
                write!(f, "Final Transform {} → Post-Affine Enabled", index + 1)
            }
            ConfigPath::FinalTransformPostAffine { index, param } => {
                write!(f, "Final Transform {} → Post-Affine {:?}", index + 1, param)
            }
            ConfigPath::FinalTransformVariation { index, variation } => {
                write!(f, "Final Transform {} → {} variation", index + 1, variation)
            }
            ConfigPath::FinalTransformVariationParam { index, variation, param } => {
                write!(f, "Final Transform {} → {} → {}", index + 1, variation, param)
            }
            ConfigPath::FinalTransformOriginX { index } => {
                write!(f, "Final Transform {} → Origin X", index + 1)
            }
            ConfigPath::FinalTransformOriginY { index } => {
                write!(f, "Final Transform {} → Origin Y", index + 1)
            }
            ConfigPath::FinalTransformRotation { index } => {
                write!(f, "Final Transform {} → Rotation", index + 1)
            }
            ConfigPath::FinalTransformScale { index } => {
                write!(f, "Final Transform {} → Scale", index + 1)
            }
            ConfigPath::FinalTransformPostAffineOriginX { index } => {
                write!(f, "Final Transform {} → Post-Affine Origin X", index + 1)
            }
            ConfigPath::FinalTransformPostAffineOriginY { index } => {
                write!(f, "Final Transform {} → Post-Affine Origin Y", index + 1)
            }
            ConfigPath::FinalTransformPostAffineRotation { index } => {
                write!(f, "Final Transform {} → Post-Affine Rotation", index + 1)
            }
            ConfigPath::FinalTransformPostAffineScale { index } => {
                write!(f, "Final Transform {} → Post-Affine Scale", index + 1)
            }

            // Flame
            ConfigPath::RenderMode => write!(f, "Render Mode"),
            ConfigPath::PerspectiveStrength => write!(f, "Perspective Strength"),
            ConfigPath::Xaos { src, dst } => {
                write!(f, "Xaos {} → {}", src + 1, dst + 1)
            }
            ConfigPath::SoloTransform => write!(f, "Solo Transform"),

            // Effects
            ConfigPath::DensityEffectEnabled { index } => {
                write!(f, "Density Effect {} → Enabled", index + 1)
            }
            ConfigPath::DensityEffectParam { index, param } => {
                write!(f, "Density Effect {} → {}", index + 1, param)
            }
            ConfigPath::ColorEffectEnabled { index } => {
                write!(f, "Color Effect {} → Enabled", index + 1)
            }
            ConfigPath::ColorEffectParam { index, param } => {
                write!(f, "Color Effect {} → {}", index + 1, param)
            }
            ConfigPath::AddColorEffect { effect_type } => {
                write!(f, "Add Color Effect: {}", effect_type)
            }
            ConfigPath::RemoveColorEffect { index } => {
                write!(f, "Remove Color Effect {}", index + 1)
            }
            ConfigPath::AddDensityEffect { effect_type } => {
                write!(f, "Add Density Effect: {}", effect_type)
            }
            ConfigPath::RemoveDensityEffect { index } => {
                write!(f, "Remove Density Effect {}", index + 1)
            }

            // System Settings
            ConfigPath::SystemIterationsPerThread => write!(f, "System: Iterations Per Thread"),
            ConfigPath::SystemBurnIn => write!(f, "System: Burn-in Iterations"),
            ConfigPath::SystemVsyncEnabled => write!(f, "System: VSync Enabled"),
            ConfigPath::SystemTargetFps => write!(f, "System: Target FPS"),
            ConfigPath::SystemExportWidth => write!(f, "System: Export Width"),
            ConfigPath::SystemExportHeight => write!(f, "System: Export Height"),
            ConfigPath::SystemLanguage => write!(f, "System: Language"),
            ConfigPath::SystemShowHelpOnStartup => write!(f, "System: Show Help On Startup"),
        }
    }
}

/// Represents an i18n key with optional parameters for translation
#[derive(Debug, Clone)]
pub struct I18nKey {
    /// The translation key (e.g., "history.param.zoom")
    pub key: String,
    /// Optional parameters for interpolation (e.g., index, variation name)
    pub params: Vec<(String, String)>,
}

impl I18nKey {
    /// Create a simple key with no parameters
    pub fn simple(key: &str) -> Self {
        Self {
            key: key.to_string(),
            params: Vec::new(),
        }
    }

    /// Create a key with parameters
    pub fn with_params(key: &str, params: Vec<(&str, String)>) -> Self {
        Self {
            key: key.to_string(),
            params: params.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }

    /// Serialize to a string format for storage in history descriptions
    /// Format: `key` or `key|param1=value1|param2=value2`
    pub fn to_serialized(&self) -> String {
        if self.params.is_empty() {
            self.key.clone()
        } else {
            let params_str = self.params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("|");
            format!("{}|{}", self.key, params_str)
        }
    }
}

impl ConfigPath {
    /// Convert to an i18n key for translation
    /// Returns an I18nKey struct with the key and any parameters needed for interpolation
    pub fn to_i18n_key(&self) -> I18nKey {
        match self {
            // View
            ConfigPath::Zoom => I18nKey::simple("history.param.zoom"),
            ConfigPath::Pan => I18nKey::simple("history.param.pan"),
            ConfigPath::PanX => I18nKey::simple("history.param.pan_x"),
            ConfigPath::PanY => I18nKey::simple("history.param.pan_y"),
            ConfigPath::Rotation => I18nKey::simple("history.param.rotation"),
            ConfigPath::CameraRotationX => I18nKey::simple("history.param.camera_pitch"),
            ConfigPath::CameraRotationY => I18nKey::simple("history.param.camera_yaw"),
            ConfigPath::CameraZ => I18nKey::simple("history.param.camera_z"),
            ConfigPath::DofFocusDistance => I18nKey::simple("history.param.dof_focus_distance"),
            ConfigPath::DofBlurStrength => I18nKey::simple("history.param.dof_blur_strength"),
            ConfigPath::FogStrength => I18nKey::simple("history.param.fog_strength"),
            ConfigPath::FogStart => I18nKey::simple("history.param.fog_start"),

            // Tone mapping
            ConfigPath::Exposure => I18nKey::simple("history.param.exposure"),
            ConfigPath::Gamma => I18nKey::simple("history.param.gamma"),
            ConfigPath::GammaThreshold => I18nKey::simple("history.param.gamma_threshold"),
            ConfigPath::Brightness => I18nKey::simple("history.param.brightness"),
            ConfigPath::Vibrancy => I18nKey::simple("history.param.vibrancy"),
            ConfigPath::WhiteLevel => I18nKey::simple("history.param.white_level"),
            ConfigPath::Saturation => I18nKey::simple("history.param.saturation"),
            ConfigPath::HueShift => I18nKey::simple("history.param.hue_shift"),
            ConfigPath::AlphaBlendLow => I18nKey::simple("history.param.alpha_blend_low"),
            ConfigPath::AlphaBlendHigh => I18nKey::simple("history.param.alpha_blend_high"),
            ConfigPath::DensityScale => I18nKey::simple("history.param.density_scale"),
            ConfigPath::TonemapMode => I18nKey::simple("history.param.tonemap_mode"),
            ConfigPath::HighlightMode => I18nKey::simple("history.param.highlight_mode"),
            ConfigPath::TonemapCurve => I18nKey::simple("history.param.tone_curve"),
            ConfigPath::UseCurve => I18nKey::simple("history.param.use_tone_curve"),
            ConfigPath::LevelsEnabled => I18nKey::simple("history.param.levels_enabled"),
            ConfigPath::LevelsLow => I18nKey::simple("history.param.levels_low"),
            ConfigPath::LevelsHigh => I18nKey::simple("history.param.levels_high"),
            ConfigPath::LevelsGamma => I18nKey::simple("history.param.levels_midtones"),

            // Color
            ConfigPath::ColorMode => I18nKey::simple("history.param.color_mode"),
            ConfigPath::PathMapStyle => I18nKey::simple("history.param.pathmap_style"),
            ConfigPath::PathCaptureMode => I18nKey::simple("history.param.pathmap_capture_mode"),
            ConfigPath::PathTrackingMode => I18nKey::simple("history.param.pathmap_tracking_mode"),
            ConfigPath::PaletteIndex => I18nKey::simple("history.param.palette"),
            ConfigPath::Palette => I18nKey::simple("history.param.palette_data"),
            ConfigPath::PaletteRotation => I18nKey::simple("history.param.palette_rotation"),
            ConfigPath::PaletteSize => I18nKey::simple("history.param.palette_size"),
            ConfigPath::PaletteSqueeze => I18nKey::simple("history.param.palette_squeeze"),
            ConfigPath::PaletteSqueezeMode => I18nKey::simple("history.param.palette_squeeze_mode"),
            ConfigPath::PaletteSqueezeFalloff => I18nKey::simple("history.param.palette_squeeze_falloff"),
            ConfigPath::PaletteLogStrength => I18nKey::simple("history.param.palette_log_strength"),
            ConfigPath::PaletteReverse => I18nKey::simple("history.param.palette_reverse"),
            ConfigPath::SpeedFactor => I18nKey::simple("history.param.speed_blend_factor"),
            ConfigPath::BackgroundColor => I18nKey::simple("history.param.background_color"),
            ConfigPath::BackgroundColorR => I18nKey::simple("history.param.background_red"),
            ConfigPath::BackgroundColorG => I18nKey::simple("history.param.background_green"),
            ConfigPath::BackgroundColorB => I18nKey::simple("history.param.background_blue"),

            // Rendering
            ConfigPath::BlendFactor => I18nKey::simple("history.param.blend_factor"),
            ConfigPath::UseDynamicBlend => I18nKey::simple("history.param.use_dynamic_blend"),
            ConfigPath::MaxIterations => I18nKey::simple("history.param.max_iterations"),
            ConfigPath::DeterministicRng => I18nKey::simple("history.param.deterministic_rng"),

            // Transforms
            ConfigPath::TransformCount => I18nKey::simple("history.param.transform_count"),
            ConfigPath::TransformWeight { index } => I18nKey::with_params(
                "history.param.transform_weight",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformColor { index } => I18nKey::with_params(
                "history.param.transform_color",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformColorSpeed { index } => I18nKey::with_params(
                "history.param.transform_color_speed",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformOpacity { index } => I18nKey::with_params(
                "history.param.transform_opacity",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformDirectColor { index } => I18nKey::with_params(
                "history.param.transform_direct_color",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformAffine { index, param } => I18nKey::with_params(
                "history.param.transform_affine",
                vec![
                    ("index", (index + 1).to_string()),
                    ("param", format!("{:?}", param)),
                ],
            ),
            ConfigPath::TransformVariation { index, variation } => I18nKey::with_params(
                "history.param.transform_variation",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                ],
            ),
            ConfigPath::TransformVariationParam { index, variation, param } => I18nKey::with_params(
                "history.param.transform_variation_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::TransformOriginX { index } => I18nKey::with_params(
                "history.param.transform_origin_x",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformOriginY { index } => I18nKey::with_params(
                "history.param.transform_origin_y",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformRotation { index } => I18nKey::with_params(
                "history.param.transform_rotation",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformScale { index } => I18nKey::with_params(
                "history.param.transform_scale",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffineEnabled { index } => I18nKey::with_params(
                "history.param.transform_post_affine_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffine { index, param } => I18nKey::with_params(
                "history.param.transform_post_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            // Reuse the pre-affine high-level i18n keys for the
            // post-affine ones — the UI distinguishes by the
            // "Post-Affine" label that surrounds the value.
            ConfigPath::TransformPostAffineOriginX { index } => I18nKey::with_params(
                "history.param.transform_origin_x",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffineOriginY { index } => I18nKey::with_params(
                "history.param.transform_origin_y",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffineRotation { index } => I18nKey::with_params(
                "history.param.transform_rotation",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::TransformPostAffineScale { index } => I18nKey::with_params(
                "history.param.transform_scale",
                vec![("index", (index + 1).to_string())],
            ),

            // Linked Transform pool
            ConfigPath::LinkedTransformAffine { index, param } => I18nKey::with_params(
                "history.param.transform_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            ConfigPath::LinkedTransformPostAffineEnabled { index } => I18nKey::with_params(
                "history.param.transform_post_affine_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::LinkedTransformPostAffine { index, param } => I18nKey::with_params(
                "history.param.transform_post_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            ConfigPath::LinkedTransformVariation { index, variation } => I18nKey::with_params(
                "history.param.transform_variation",
                vec![("index", (index + 1).to_string()), ("variation", variation.clone())],
            ),
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => I18nKey::with_params(
                "history.param.transform_variation_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::LinkedTransformOriginX { index }
            | ConfigPath::LinkedTransformPostAffineOriginX { index } => I18nKey::with_params(
                "history.param.transform_origin_x",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::LinkedTransformOriginY { index }
            | ConfigPath::LinkedTransformPostAffineOriginY { index } => I18nKey::with_params(
                "history.param.transform_origin_y",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::LinkedTransformRotation { index }
            | ConfigPath::LinkedTransformPostAffineRotation { index } => I18nKey::with_params(
                "history.param.transform_rotation",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::LinkedTransformScale { index }
            | ConfigPath::LinkedTransformPostAffineScale { index } => I18nKey::with_params(
                "history.param.transform_scale",
                vec![("index", (index + 1).to_string())],
            ),

            // Final Transform pool — reuses transform_* i18n keys; the
            // user-visible "Linked"/"Final" distinction comes from the
            // panel header.
            ConfigPath::FinalTransformAffine { index, param } => I18nKey::with_params(
                "history.param.transform_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            ConfigPath::FinalTransformPostAffineEnabled { index } => I18nKey::with_params(
                "history.param.transform_post_affine_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::FinalTransformPostAffine { index, param } => I18nKey::with_params(
                "history.param.transform_post_affine",
                vec![("index", (index + 1).to_string()), ("param", format!("{:?}", param))],
            ),
            ConfigPath::FinalTransformVariation { index, variation } => I18nKey::with_params(
                "history.param.transform_variation",
                vec![("index", (index + 1).to_string()), ("variation", variation.clone())],
            ),
            ConfigPath::FinalTransformVariationParam { index, variation, param } => I18nKey::with_params(
                "history.param.transform_variation_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("variation", variation.clone()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::FinalTransformOriginX { index }
            | ConfigPath::FinalTransformPostAffineOriginX { index } => I18nKey::with_params(
                "history.param.transform_origin_x",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::FinalTransformOriginY { index }
            | ConfigPath::FinalTransformPostAffineOriginY { index } => I18nKey::with_params(
                "history.param.transform_origin_y",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::FinalTransformRotation { index }
            | ConfigPath::FinalTransformPostAffineRotation { index } => I18nKey::with_params(
                "history.param.transform_rotation",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::FinalTransformScale { index }
            | ConfigPath::FinalTransformPostAffineScale { index } => I18nKey::with_params(
                "history.param.transform_scale",
                vec![("index", (index + 1).to_string())],
            ),

            // Flame
            ConfigPath::RenderMode => I18nKey::simple("history.param.render_mode"),
            ConfigPath::PerspectiveStrength => I18nKey::simple("history.param.perspective_strength"),
            ConfigPath::Xaos { src, dst } => I18nKey::with_params(
                "history.param.xaos",
                vec![
                    ("src", (src + 1).to_string()),
                    ("dst", (dst + 1).to_string()),
                ],
            ),
            ConfigPath::SoloTransform => I18nKey::simple("history.param.solo_transform"),

            // Effects
            ConfigPath::DensityEffectEnabled { index } => I18nKey::with_params(
                "history.param.density_effect_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::DensityEffectParam { index, param } => I18nKey::with_params(
                "history.param.density_effect_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::ColorEffectEnabled { index } => I18nKey::with_params(
                "history.param.color_effect_enabled",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::ColorEffectParam { index, param } => I18nKey::with_params(
                "history.param.color_effect_param",
                vec![
                    ("index", (index + 1).to_string()),
                    ("param", param.clone()),
                ],
            ),
            ConfigPath::AddColorEffect { effect_type } => I18nKey::with_params(
                "history.param.add_color_effect",
                vec![("effect_type", effect_type.clone())],
            ),
            ConfigPath::RemoveColorEffect { index } => I18nKey::with_params(
                "history.param.remove_color_effect",
                vec![("index", (index + 1).to_string())],
            ),
            ConfigPath::AddDensityEffect { effect_type } => I18nKey::with_params(
                "history.param.add_density_effect",
                vec![("effect_type", effect_type.clone())],
            ),
            ConfigPath::RemoveDensityEffect { index } => I18nKey::with_params(
                "history.param.remove_density_effect",
                vec![("index", (index + 1).to_string())],
            ),

            // System Settings
            ConfigPath::SystemIterationsPerThread => I18nKey::simple("history.param.system_iterations_per_thread"),
            ConfigPath::SystemBurnIn => I18nKey::simple("history.param.system_burn_in"),
            ConfigPath::SystemVsyncEnabled => I18nKey::simple("history.param.system_vsync_enabled"),
            ConfigPath::SystemTargetFps => I18nKey::simple("history.param.system_target_fps"),
            ConfigPath::SystemExportWidth => I18nKey::simple("history.param.system_export_width"),
            ConfigPath::SystemExportHeight => I18nKey::simple("history.param.system_export_height"),
            ConfigPath::SystemLanguage => I18nKey::simple("history.param.system_language"),
            ConfigPath::SystemShowHelpOnStartup => I18nKey::simple("history.param.system_show_help_on_startup"),
        }
    }
}

/// A value that can be stored in FractalConfig
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// Unit value for operations that don't need a value (e.g., Add/Remove)
    Unit,
    Float(f32),
    Int(i32),
    UInt(u32),
    UInt64(u64),
    Bool(bool),
    String(String),
    Vec2(f32, f32),  // For pan coordinates and other 2D values
    ColorRgb([f32; 3]),
    ToneMapMode(ToneMapMode),
    HighlightMode(crate::scene::tonemap::HighlightMode),
    ColorMode(ColorMode),
    PathMapStyle(PathMapStyle),
    PathCaptureMode(PathCaptureMode),
    PathTrackingMode(PathTrackingMode),
    RenderMode(RenderMode),
    ToneCurve(ToneCurve),
    Palette(Palette),
    SqueezeMode(crate::scene::palette::SqueezeMode),
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
            (ConfigValue::HighlightMode(a), ConfigValue::HighlightMode(b)) => a == b,
            (ConfigValue::ColorMode(a), ConfigValue::ColorMode(b)) => a == b,
            (ConfigValue::SqueezeMode(a), ConfigValue::SqueezeMode(b)) => a == b,
            (ConfigValue::RenderMode(a), ConfigValue::RenderMode(b)) => a == b,
            // For complex types, do shallow comparison or always return false
            _ => false,
        }
    }
}

impl Display for ConfigValue {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ConfigValue::Unit => write!(f, "()"),
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
            ConfigValue::HighlightMode(m) => write!(f, "{:?}", m),
            ConfigValue::ColorMode(m) => write!(f, "{:?}", m),
            ConfigValue::SqueezeMode(m) => write!(f, "{:?}", m),
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
impl From<()> for ConfigValue {
    fn from(_: ()) -> Self {
        ConfigValue::Unit
    }
}

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

impl From<crate::scene::tonemap::HighlightMode> for ConfigValue {
    fn from(v: crate::scene::tonemap::HighlightMode) -> Self {
        ConfigValue::HighlightMode(v)
    }
}

impl From<ColorMode> for ConfigValue {
    fn from(v: ColorMode) -> Self {
        ConfigValue::ColorMode(v)
    }
}

impl From<crate::scene::palette::SqueezeMode> for ConfigValue {
    fn from(v: crate::scene::palette::SqueezeMode) -> Self {
        ConfigValue::SqueezeMode(v)
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

    /// Human-readable description using i18n key
    /// Returns serialized i18n key with params - UI translates via translate_description()
    /// Format: `key` or `key|param1=value1|param2=value2`
    pub fn description(&self) -> String {
        self.path.to_i18n_key().to_serialized()
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
    /// Undo: remove at index + restore xaos, Redo: insert at index
    AddTransform {
        index: usize,
        transform: crate::scene::transforms::Transform,
        /// Xaos matrix state before the add (restored on undo)
        xaos_before: Option<Vec<Vec<f32>>>,
        /// If Some, this was a clone — duplicates source's xaos relationships on apply/redo
        clone_from: Option<usize>,
    },

    /// Transform deleted
    /// Undo: re-insert at index + restore xaos, Redo: remove at index
    DeleteTransform {
        index: usize,
        transform: crate::scene::transforms::Transform,
        /// Xaos matrix state before the delete (restored on undo)
        xaos_before: Option<Vec<Vec<f32>>>,
    },

    /// Transform modified (affine edit, triangle editor, etc.)
    /// Stores before/after states for complete restoration. `kind`
    /// selects which pool the `index` indexes into — Triangle Editor
    /// drives all three (Normal / Linked / Final) and the apply path
    /// dispatches accordingly.
    /// Undo: restore before, Redo: restore after
    ModifyTransform {
        kind: TransformKind,
        index: usize,
        before: crate::scene::transforms::Transform,
        after: crate::scene::transforms::Transform,
    },

    /// Color effect added
    /// Undo: remove at index, Redo: insert at index
    AddColorEffect {
        index: usize,
        effect: crate::effects::EffectInstance,
    },

    /// Color effect deleted
    /// Undo: re-insert at index, Redo: remove at index
    DeleteColorEffect {
        index: usize,
        effect: crate::effects::EffectInstance,
    },

    /// Density effect added
    /// Undo: remove at index, Redo: insert at index
    AddDensityEffect {
        index: usize,
        effect: crate::effects::EffectInstance,
    },

    /// Density effect deleted
    /// Undo: re-insert at index, Redo: remove at index
    DeleteDensityEffect {
        index: usize,
        effect: crate::effects::EffectInstance,
    },

    /// Color effect moved (reordered)
    /// Undo: move from to_index back to from_index
    /// Redo: move from from_index to to_index
    MoveColorEffect { from_index: usize, to_index: usize },

    /// Density effect moved (reordered)
    /// Undo: move from to_index back to from_index
    /// Redo: move from from_index to to_index
    MoveDensityEffect { from_index: usize, to_index: usize },

    /// Subflame added.
    /// Undo: swap editing_target to `target_before`, remove subflame at `index`.
    /// Redo: swap to Main, append subflame at `index` (always end of list at
    /// time of add per the add-only-on-Main gate).
    /// `flame` is the *full* added Flame so redo recreates byte-for-byte
    /// even after intervening edits to the rest of the config.
    AddSubflame {
        index: usize,
        flame: crate::scene::transforms::Flame,
        /// Editing context at the moment of the add. Always Main today
        /// (the public API gates add on Main), but stored for future
        /// flexibility and so the undo can confidently restore context.
        target_before: super::manager::EditingTarget,
    },

    /// Subflame deleted.
    /// Undo: re-insert `flame` at `index`, swap editing_target to `target_before`.
    /// Redo: silent-swap to Main, remove subflame at `index`.
    /// `flame` holds the entire deleted Flame so undo restores it
    /// exactly — every transform, variation, parameter, even nested
    /// state — independent of any subsequent edits.
    DeleteSubflame {
        index: usize,
        flame: crate::scene::transforms::Flame,
        /// Editing context before the delete. Can be Main or any
        /// Subflame{i} (delete_subflame auto-swaps to Main internally,
        /// so we capture the pre-swap state here).
        target_before: super::manager::EditingTarget,
    },

    /// Editing target swapped (user clicked Main / Subflame N in the
    /// Subflames panel).
    /// Undo: swap to `before`. Redo: swap to `after`.
    /// No state-data change — only the editing context.
    SwapTarget {
        before: super::manager::EditingTarget,
        after: super::manager::EditingTarget,
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
    /// Which flame this change was made against. Undo/redo silently
    /// swaps the editing context to match this target before applying
    /// the inverse delta or snapshot. Without this tag, an edit made
    /// while on Subflame{N} would be replayed against whatever flame
    /// happens to be swapped in at undo time, silently corrupting
    /// state. Stamped by `ConfigManager::push_undo` from the manager's
    /// current `editing_target`.
    pub target: super::manager::EditingTarget,
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
            // Stamped by push_undo with the manager's editing_target —
            // this default only matters for any direct ConfigChange use
            // outside the manager (e.g., tests).
            target: super::manager::EditingTarget::Main,
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
            // Stamped by push_undo with the manager's editing_target —
            // this default only matters for any direct ConfigChange use
            // outside the manager (e.g., tests).
            target: super::manager::EditingTarget::Main,
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
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create add transform snapshot
    /// Stores the added transform and xaos state for efficient undo/redo
    pub fn add_transform_snapshot(
        index: usize,
        transform: crate::scene::transforms::Transform,
        xaos_before: Option<Vec<Vec<f32>>>,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddTransform { index, transform, xaos_before, clone_from: None }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create clone transform snapshot
    /// Like add, but duplicates the source transform's xaos relationships
    pub fn clone_transform_snapshot(
        index: usize,
        transform: crate::scene::transforms::Transform,
        xaos_before: Option<Vec<Vec<f32>>>,
        clone_from: usize,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddTransform { index, transform, xaos_before, clone_from: Some(clone_from) }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create delete transform snapshot
    /// Stores the deleted transform and xaos state for efficient undo/redo
    pub fn delete_transform_snapshot(
        index: usize,
        transform: crate::scene::transforms::Transform,
        xaos_before: Option<Vec<Vec<f32>>>,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteTransform { index, transform, xaos_before }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create modify transform snapshot
    /// Stores before/after transform states for complete restoration.
    /// `kind` selects which pool the `index` indexes into.
    pub fn modify_transform_snapshot(
        kind: TransformKind,
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
            snapshot: Some(SnapshotData::ModifyTransform { kind, index, before, after }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create add color effect snapshot
    pub fn add_color_effect_snapshot(
        index: usize,
        effect: crate::effects::EffectInstance,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddColorEffect { index, effect }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create delete color effect snapshot
    pub fn delete_color_effect_snapshot(
        index: usize,
        effect: crate::effects::EffectInstance,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteColorEffect { index, effect }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create add density effect snapshot
    pub fn add_density_effect_snapshot(
        index: usize,
        effect: crate::effects::EffectInstance,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddDensityEffect { index, effect }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create delete density effect snapshot
    pub fn delete_density_effect_snapshot(
        index: usize,
        effect: crate::effects::EffectInstance,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteDensityEffect { index, effect }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create a color effect move change (reorder)
    pub fn move_color_effect_snapshot(
        from_index: usize,
        to_index: usize,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::MoveColorEffect { from_index, to_index }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Create a density effect move change (reorder)
    pub fn move_density_effect_snapshot(
        from_index: usize,
        to_index: usize,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::MoveDensityEffect { from_index, to_index }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Subflame added.
    pub fn add_subflame_snapshot(
        index: usize,
        flame: crate::scene::transforms::Flame,
        target_before: super::manager::EditingTarget,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::AddSubflame { index, flame, target_before }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Subflame deleted. `flame` must hold the FULL removed Flame so
    /// undo restores it byte-for-byte.
    pub fn delete_subflame_snapshot(
        index: usize,
        flame: crate::scene::transforms::Flame,
        target_before: super::manager::EditingTarget,
        description: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description,
            snapshot: Some(SnapshotData::DeleteSubflame { index, flame, target_before }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
        }
    }

    /// Editing target switched (Main ↔ Subflame{N}).
    pub fn swap_target_snapshot(
        before: super::manager::EditingTarget,
        after: super::manager::EditingTarget,
    ) -> Self {
        let now = Instant::now();
        Self {
            deltas: vec![],
            timestamp: now,
            description: format!("Switch editing target ({:?} → {:?})", before, after),
            snapshot: Some(SnapshotData::SwapTarget { before, after }),
            last_update_time: now,
            target: super::manager::EditingTarget::Main,
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
            target: super::manager::EditingTarget::Main,
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

            // DOF and fog changes affect pixel colors at write time, need iteration reset
            ConfigPath::DofFocusDistance
            | ConfigPath::DofBlurStrength
            | ConfigPath::FogStrength
            | ConfigPath::FogStart => UpdateType::IterationReset,

            // Tone mapping - re-run tonemap shader
            ConfigPath::Exposure
            | ConfigPath::Gamma
            | ConfigPath::GammaThreshold
            | ConfigPath::Brightness
            | ConfigPath::Vibrancy
            | ConfigPath::WhiteLevel
            | ConfigPath::Saturation
            | ConfigPath::HueShift
            | ConfigPath::AlphaBlendLow
            | ConfigPath::AlphaBlendHigh
            | ConfigPath::DensityScale
            | ConfigPath::TonemapMode
            | ConfigPath::HighlightMode
            | ConfigPath::TonemapCurve
            | ConfigPath::UseCurve
            | ConfigPath::BackgroundColor
            | ConfigPath::BackgroundColorR
            | ConfigPath::BackgroundColorG
            | ConfigPath::BackgroundColorB
            | ConfigPath::LevelsEnabled
            | ConfigPath::LevelsLow
            | ConfigPath::LevelsHigh
            | ConfigPath::LevelsGamma => UpdateType::ToneMappingOnly,

            // Color parameters - re-run accumulation with new colors
            ConfigPath::ColorMode
            | ConfigPath::PaletteIndex
            | ConfigPath::Palette
            | ConfigPath::PaletteRotation
            | ConfigPath::PaletteSize
            | ConfigPath::PaletteSqueeze
            | ConfigPath::PaletteSqueezeMode
            | ConfigPath::PaletteSqueezeFalloff
            | ConfigPath::PaletteLogStrength
            | ConfigPath::PaletteReverse
            | ConfigPath::SpeedFactor
            // PathMapStyle affects color computation in compute shader, needs accumulation reset
            | ConfigPath::PathMapStyle => UpdateType::ColorOnly,

            // PathCaptureMode affects path buffer capture logic in compute shader
            ConfigPath::PathCaptureMode => UpdateType::IterationReset,

            // PathTrackingMode affects path tracking logic in compute shader
            ConfigPath::PathTrackingMode => UpdateType::IterationReset,

            // Rendering settings - affect iteration behavior
            ConfigPath::BlendFactor
            | ConfigPath::UseDynamicBlend => UpdateType::IterationReset,

            // Transform/flame changes - full reset
            ConfigPath::TransformCount
            | ConfigPath::TransformWeight { .. }
            | ConfigPath::TransformColor { .. }
            | ConfigPath::TransformColorSpeed { .. }
            | ConfigPath::TransformOpacity { .. }
            | ConfigPath::TransformDirectColor { .. }
            | ConfigPath::TransformAffine { .. }
            | ConfigPath::TransformPostAffineEnabled { .. }
            | ConfigPath::TransformPostAffine { .. }
            | ConfigPath::TransformVariation { .. }
            | ConfigPath::TransformVariationParam { .. }
            | ConfigPath::TransformOriginX { .. }
            | ConfigPath::TransformOriginY { .. }
            | ConfigPath::TransformRotation { .. }
            | ConfigPath::TransformScale { .. }
            | ConfigPath::TransformPostAffineOriginX { .. }
            | ConfigPath::TransformPostAffineOriginY { .. }
            | ConfigPath::TransformPostAffineRotation { .. }
            | ConfigPath::TransformPostAffineScale { .. }
            | ConfigPath::LinkedTransformAffine { .. }
            | ConfigPath::LinkedTransformPostAffineEnabled { .. }
            | ConfigPath::LinkedTransformPostAffine { .. }
            | ConfigPath::LinkedTransformVariation { .. }
            | ConfigPath::LinkedTransformVariationParam { .. }
            | ConfigPath::LinkedTransformOriginX { .. }
            | ConfigPath::LinkedTransformOriginY { .. }
            | ConfigPath::LinkedTransformRotation { .. }
            | ConfigPath::LinkedTransformScale { .. }
            | ConfigPath::LinkedTransformPostAffineOriginX { .. }
            | ConfigPath::LinkedTransformPostAffineOriginY { .. }
            | ConfigPath::LinkedTransformPostAffineRotation { .. }
            | ConfigPath::LinkedTransformPostAffineScale { .. }
            | ConfigPath::FinalTransformAffine { .. }
            | ConfigPath::FinalTransformPostAffineEnabled { .. }
            | ConfigPath::FinalTransformPostAffine { .. }
            | ConfigPath::FinalTransformVariation { .. }
            | ConfigPath::FinalTransformVariationParam { .. }
            | ConfigPath::FinalTransformOriginX { .. }
            | ConfigPath::FinalTransformOriginY { .. }
            | ConfigPath::FinalTransformRotation { .. }
            | ConfigPath::FinalTransformScale { .. }
            | ConfigPath::FinalTransformPostAffineOriginX { .. }
            | ConfigPath::FinalTransformPostAffineOriginY { .. }
            | ConfigPath::FinalTransformPostAffineRotation { .. }
            | ConfigPath::FinalTransformPostAffineScale { .. }
            | ConfigPath::RenderMode
            | ConfigPath::PerspectiveStrength
            | ConfigPath::Xaos { .. }
            | ConfigPath::SoloTransform
            | ConfigPath::MaxIterations
            | ConfigPath::DeterministicRng => UpdateType::IterationReset,

            // Effects (post-processing, just need tonemap re-run)
            ConfigPath::DensityEffectEnabled { .. }
            | ConfigPath::DensityEffectParam { .. }
            | ConfigPath::ColorEffectEnabled { .. }
            | ConfigPath::ColorEffectParam { .. }
            | ConfigPath::AddColorEffect { .. }
            | ConfigPath::RemoveColorEffect { .. }
            | ConfigPath::AddDensityEffect { .. }
            | ConfigPath::RemoveDensityEffect { .. } => UpdateType::ToneMappingOnly,

            // System Settings
            ConfigPath::SystemIterationsPerThread | ConfigPath::SystemBurnIn => UpdateType::IterationReset,
            ConfigPath::SystemVsyncEnabled | ConfigPath::SystemTargetFps => UpdateType::ViewOnly,
            ConfigPath::SystemExportWidth | ConfigPath::SystemExportHeight | ConfigPath::SystemLanguage | ConfigPath::SystemShowHelpOnStartup => UpdateType::None,
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
            ConfigPath::DofFocusDistance => "DofFocusDistance".to_string(),
            ConfigPath::DofBlurStrength => "DofBlurStrength".to_string(),
            ConfigPath::FogStrength => "FogStrength".to_string(),
            ConfigPath::FogStart => "FogStart".to_string(),

            // Tone mapping
            ConfigPath::Exposure => "Exposure".to_string(),
            ConfigPath::Gamma => "Gamma".to_string(),
            ConfigPath::GammaThreshold => "GammaThreshold".to_string(),
            ConfigPath::Brightness => "Brightness".to_string(),
            ConfigPath::Vibrancy => "Vibrancy".to_string(),
            ConfigPath::WhiteLevel => "WhiteLevel".to_string(),
            ConfigPath::Saturation => "Saturation".to_string(),
            ConfigPath::HueShift => "HueShift".to_string(),
            ConfigPath::AlphaBlendLow => "AlphaBlendLow".to_string(),
            ConfigPath::AlphaBlendHigh => "AlphaBlendHigh".to_string(),
            ConfigPath::DensityScale => "DensityScale".to_string(),
            ConfigPath::TonemapMode => "TonemapMode".to_string(),
            ConfigPath::HighlightMode => "HighlightMode".to_string(),
            ConfigPath::TonemapCurve => "TonemapCurve".to_string(),
            ConfigPath::UseCurve => "UseCurve".to_string(),
            ConfigPath::LevelsEnabled => "LevelsEnabled".to_string(),
            ConfigPath::LevelsLow => "LevelsLow".to_string(),
            ConfigPath::LevelsHigh => "LevelsHigh".to_string(),
            ConfigPath::LevelsGamma => "LevelsGamma".to_string(),

            // Color
            ConfigPath::ColorMode => "ColorMode".to_string(),
            ConfigPath::PathMapStyle => "PathMapStyle".to_string(),
            ConfigPath::PathCaptureMode => "PathCaptureMode".to_string(),
            ConfigPath::PathTrackingMode => "PathTrackingMode".to_string(),
            ConfigPath::PaletteIndex => "PaletteIndex".to_string(),
            ConfigPath::Palette => "Palette".to_string(),
            ConfigPath::PaletteRotation => "PaletteRotation".to_string(),
            ConfigPath::PaletteSize => "PaletteSize".to_string(),
            ConfigPath::PaletteSqueeze => "PaletteSqueeze".to_string(),
            ConfigPath::PaletteSqueezeMode => "PaletteSqueezeMode".to_string(),
            ConfigPath::PaletteSqueezeFalloff => "PaletteSqueezeFalloff".to_string(),
            ConfigPath::PaletteLogStrength => "PaletteLogStrength".to_string(),
            ConfigPath::PaletteReverse => "PaletteReverse".to_string(),
            ConfigPath::SpeedFactor => "SpeedFactor".to_string(),
            ConfigPath::BackgroundColor => "BackgroundColor".to_string(),
            ConfigPath::BackgroundColorR => "BackgroundColorR".to_string(),
            ConfigPath::BackgroundColorG => "BackgroundColorG".to_string(),
            ConfigPath::BackgroundColorB => "BackgroundColorB".to_string(),

            // Rendering
            ConfigPath::BlendFactor => "BlendFactor".to_string(),
            ConfigPath::UseDynamicBlend => "UseDynamicBlend".to_string(),
            ConfigPath::MaxIterations => "MaxIterations".to_string(),
            ConfigPath::DeterministicRng => "DeterministicRng".to_string(),

            // Transforms
            ConfigPath::TransformCount => "TransformCount".to_string(),
            ConfigPath::TransformWeight { index } => format!("Transform.{}.Weight", index),
            ConfigPath::TransformColor { index } => format!("Transform.{}.Color", index),
            ConfigPath::TransformColorSpeed { index } => format!("Transform.{}.ColorSpeed", index),
            ConfigPath::TransformOpacity { index } => format!("Transform.{}.Opacity", index),
            ConfigPath::TransformDirectColor { index } => format!("Transform.{}.DirectColor", index),
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
            ConfigPath::TransformPostAffineEnabled { index } => format!("Transform.{}.PostAffineEnabled", index),
            ConfigPath::TransformPostAffine { index, param } => {
                format!("Transform.{}.PostAffine.{}", index, param.to_char())
            }
            ConfigPath::TransformPostAffineOriginX { index } => format!("Transform.{}.PostAffineOriginX", index),
            ConfigPath::TransformPostAffineOriginY { index } => format!("Transform.{}.PostAffineOriginY", index),
            ConfigPath::TransformPostAffineRotation { index } => format!("Transform.{}.PostAffineRotation", index),
            ConfigPath::TransformPostAffineScale { index } => format!("Transform.{}.PostAffineScale", index),

            // Linked Transform pool
            ConfigPath::LinkedTransformAffine { index, param } => {
                format!("LinkedTransform.{}.Affine.{}", index, param.to_char())
            }
            ConfigPath::LinkedTransformPostAffineEnabled { index } => {
                format!("LinkedTransform.{}.PostAffineEnabled", index)
            }
            ConfigPath::LinkedTransformPostAffine { index, param } => {
                format!("LinkedTransform.{}.PostAffine.{}", index, param.to_char())
            }
            ConfigPath::LinkedTransformVariation { index, variation } => {
                format!("LinkedTransform.{}.Variation.{}", index, variation)
            }
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => {
                format!("LinkedTransform.{}.VariationParam.{}.{}", index, variation, param)
            }
            ConfigPath::LinkedTransformOriginX { index } => format!("LinkedTransform.{}.OriginX", index),
            ConfigPath::LinkedTransformOriginY { index } => format!("LinkedTransform.{}.OriginY", index),
            ConfigPath::LinkedTransformRotation { index } => format!("LinkedTransform.{}.Rotation", index),
            ConfigPath::LinkedTransformScale { index } => format!("LinkedTransform.{}.Scale", index),
            ConfigPath::LinkedTransformPostAffineOriginX { index } => format!("LinkedTransform.{}.PostAffineOriginX", index),
            ConfigPath::LinkedTransformPostAffineOriginY { index } => format!("LinkedTransform.{}.PostAffineOriginY", index),
            ConfigPath::LinkedTransformPostAffineRotation { index } => format!("LinkedTransform.{}.PostAffineRotation", index),
            ConfigPath::LinkedTransformPostAffineScale { index } => format!("LinkedTransform.{}.PostAffineScale", index),

            // Final Transform pool. Emits the new `FinalTransform.{index}.{field}`
            // format. The parser in `from_string_key` also accepts the
            // historical `PoolFinalTransform.{index}.{field}` form (saved by
            // the prior branch) and the legacy `FinalTransform.{field}`
            // (no index, single-final) form for backward compat.
            ConfigPath::FinalTransformAffine { index, param } => {
                format!("FinalTransform.{}.Affine.{}", index, param.to_char())
            }
            ConfigPath::FinalTransformPostAffineEnabled { index } => {
                format!("FinalTransform.{}.PostAffineEnabled", index)
            }
            ConfigPath::FinalTransformPostAffine { index, param } => {
                format!("FinalTransform.{}.PostAffine.{}", index, param.to_char())
            }
            ConfigPath::FinalTransformVariation { index, variation } => {
                format!("FinalTransform.{}.Variation.{}", index, variation)
            }
            ConfigPath::FinalTransformVariationParam { index, variation, param } => {
                format!("FinalTransform.{}.VariationParam.{}.{}", index, variation, param)
            }
            ConfigPath::FinalTransformOriginX { index } => format!("FinalTransform.{}.OriginX", index),
            ConfigPath::FinalTransformOriginY { index } => format!("FinalTransform.{}.OriginY", index),
            ConfigPath::FinalTransformRotation { index } => format!("FinalTransform.{}.Rotation", index),
            ConfigPath::FinalTransformScale { index } => format!("FinalTransform.{}.Scale", index),
            ConfigPath::FinalTransformPostAffineOriginX { index } => format!("FinalTransform.{}.PostAffineOriginX", index),
            ConfigPath::FinalTransformPostAffineOriginY { index } => format!("FinalTransform.{}.PostAffineOriginY", index),
            ConfigPath::FinalTransformPostAffineRotation { index } => format!("FinalTransform.{}.PostAffineRotation", index),
            ConfigPath::FinalTransformPostAffineScale { index } => format!("FinalTransform.{}.PostAffineScale", index),

            // Flame
            ConfigPath::RenderMode => "RenderMode".to_string(),
            ConfigPath::PerspectiveStrength => "PerspectiveStrength".to_string(),
            ConfigPath::Xaos { src, dst } => format!("Xaos.{}.{}", src, dst),
            ConfigPath::SoloTransform => "SoloTransform".to_string(),

            // Effects
            ConfigPath::DensityEffectEnabled { index } => format!("DensityEffect.{}.Enabled", index),
            ConfigPath::DensityEffectParam { index, param } => format!("DensityEffect.{}.{}", index, param),
            ConfigPath::ColorEffectEnabled { index } => format!("ColorEffect.{}.Enabled", index),
            ConfigPath::ColorEffectParam { index, param } => format!("ColorEffect.{}.{}", index, param),
            ConfigPath::AddColorEffect { effect_type } => format!("ColorEffect.Add.{}", effect_type),
            ConfigPath::RemoveColorEffect { index } => format!("ColorEffect.Remove.{}", index),
            ConfigPath::AddDensityEffect { effect_type } => format!("DensityEffect.Add.{}", effect_type),
            ConfigPath::RemoveDensityEffect { index } => format!("DensityEffect.Remove.{}", index),

            // System Settings (not typically animated, but included for completeness)
            ConfigPath::SystemIterationsPerThread => "System.IterationsPerThread".to_string(),
            ConfigPath::SystemBurnIn => "System.BurnIn".to_string(),
            ConfigPath::SystemVsyncEnabled => "System.VsyncEnabled".to_string(),
            ConfigPath::SystemTargetFps => "System.TargetFps".to_string(),
            ConfigPath::SystemExportWidth => "System.ExportWidth".to_string(),
            ConfigPath::SystemExportHeight => "System.ExportHeight".to_string(),
            ConfigPath::SystemLanguage => "System.Language".to_string(),
            ConfigPath::SystemShowHelpOnStartup => "System.ShowHelpOnStartup".to_string(),
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
            "DofFocusDistance" => return Some(ConfigPath::DofFocusDistance),
            "DofBlurStrength" => return Some(ConfigPath::DofBlurStrength),
            "FogStrength" => return Some(ConfigPath::FogStrength),
            "FogStart" => return Some(ConfigPath::FogStart),

            // Tone mapping
            "Exposure" => return Some(ConfigPath::Exposure),
            "Gamma" => return Some(ConfigPath::Gamma),
            "GammaThreshold" => return Some(ConfigPath::GammaThreshold),
            "Brightness" => return Some(ConfigPath::Brightness),
            "Vibrancy" => return Some(ConfigPath::Vibrancy),
            "WhiteLevel" => return Some(ConfigPath::WhiteLevel),
            "Saturation" => return Some(ConfigPath::Saturation),
            "HueShift" => return Some(ConfigPath::HueShift),
            "AlphaBlendLow" => return Some(ConfigPath::AlphaBlendLow),
            "AlphaBlendHigh" => return Some(ConfigPath::AlphaBlendHigh),
            "DensityScale" => return Some(ConfigPath::DensityScale),
            "TonemapMode" => return Some(ConfigPath::TonemapMode),
            "HighlightMode" => return Some(ConfigPath::HighlightMode),
            "TonemapCurve" => return Some(ConfigPath::TonemapCurve),
            "UseCurve" => return Some(ConfigPath::UseCurve),
            "LevelsEnabled" => return Some(ConfigPath::LevelsEnabled),
            "LevelsLow" => return Some(ConfigPath::LevelsLow),
            "LevelsHigh" => return Some(ConfigPath::LevelsHigh),
            "LevelsGamma" => return Some(ConfigPath::LevelsGamma),

            // Color
            "ColorMode" => return Some(ConfigPath::ColorMode),
            "PathMapStyle" => return Some(ConfigPath::PathMapStyle),
            "PathCaptureMode" => return Some(ConfigPath::PathCaptureMode),
            "PathTrackingMode" => return Some(ConfigPath::PathTrackingMode),
            "PaletteIndex" => return Some(ConfigPath::PaletteIndex),
            "Palette" => return Some(ConfigPath::Palette),
            "PaletteRotation" => return Some(ConfigPath::PaletteRotation),
            "PaletteSize" => return Some(ConfigPath::PaletteSize),
            "PaletteSqueeze" => return Some(ConfigPath::PaletteSqueeze),
            "PaletteSqueezeMode" => return Some(ConfigPath::PaletteSqueezeMode),
            "PaletteSqueezeFalloff" => return Some(ConfigPath::PaletteSqueezeFalloff),
            "PaletteLogStrength" => return Some(ConfigPath::PaletteLogStrength),
            "PaletteReverse" => return Some(ConfigPath::PaletteReverse),
            "SpeedFactor" => return Some(ConfigPath::SpeedFactor),
            "BackgroundColor" => return Some(ConfigPath::BackgroundColor),
            "BackgroundColorR" => return Some(ConfigPath::BackgroundColorR),
            "BackgroundColorG" => return Some(ConfigPath::BackgroundColorG),
            "BackgroundColorB" => return Some(ConfigPath::BackgroundColorB),

            // Rendering
            "BlendFactor" => return Some(ConfigPath::BlendFactor),
            "UseDynamicBlend" => return Some(ConfigPath::UseDynamicBlend),
            "MaxIterations" => return Some(ConfigPath::MaxIterations),
            "DeterministicRng" => return Some(ConfigPath::DeterministicRng),

            // Flame
            "TransformCount" => return Some(ConfigPath::TransformCount),
            "RenderMode" => return Some(ConfigPath::RenderMode),
            "PerspectiveStrength" => return Some(ConfigPath::PerspectiveStrength),
            "SoloTransform" => return Some(ConfigPath::SoloTransform),

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
                "DirectColor" => return Some(ConfigPath::TransformDirectColor { index }),
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
                "PostAffineEnabled" => return Some(ConfigPath::TransformPostAffineEnabled { index }),
                "PostAffine" if parts.len() == 4 => {
                    let param = AffineParam::from_char(parts[3].chars().next()?)?;
                    return Some(ConfigPath::TransformPostAffine { index, param });
                }
                "OriginX" => return Some(ConfigPath::TransformOriginX { index }),
                "OriginY" => return Some(ConfigPath::TransformOriginY { index }),
                "Rotation" => return Some(ConfigPath::TransformRotation { index }),
                "Scale" => return Some(ConfigPath::TransformScale { index }),
                "PostAffineOriginX" => return Some(ConfigPath::TransformPostAffineOriginX { index }),
                "PostAffineOriginY" => return Some(ConfigPath::TransformPostAffineOriginY { index }),
                "PostAffineRotation" => return Some(ConfigPath::TransformPostAffineRotation { index }),
                "PostAffineScale" => return Some(ConfigPath::TransformPostAffineScale { index }),
                _ => {}
            }
        }

        // LinkedTransform pool paths: LinkedTransform.{index}.{field}...
        // (mirrors Transform.{index}.{field}... but indexes into
        // flame.linked_transforms instead of flame.transforms.)
        if parts.len() >= 3 && parts[0] == "LinkedTransform" {
            let index: usize = parts[1].parse().ok()?;
            match parts[2] {
                "Affine" if parts.len() == 4 => {
                    let param = AffineParam::from_char(parts[3].chars().next()?)?;
                    return Some(ConfigPath::LinkedTransformAffine { index, param });
                }
                "PostAffineEnabled" => {
                    return Some(ConfigPath::LinkedTransformPostAffineEnabled { index });
                }
                "PostAffine" if parts.len() == 4 => {
                    let param = AffineParam::from_char(parts[3].chars().next()?)?;
                    return Some(ConfigPath::LinkedTransformPostAffine { index, param });
                }
                "Variation" if parts.len() == 4 => {
                    return Some(ConfigPath::LinkedTransformVariation {
                        index,
                        variation: parts[3].to_string(),
                    });
                }
                "VariationParam" if parts.len() == 5 => {
                    return Some(ConfigPath::LinkedTransformVariationParam {
                        index,
                        variation: parts[3].to_string(),
                        param: parts[4].to_string(),
                    });
                }
                "OriginX" => return Some(ConfigPath::LinkedTransformOriginX { index }),
                "OriginY" => return Some(ConfigPath::LinkedTransformOriginY { index }),
                "Rotation" => return Some(ConfigPath::LinkedTransformRotation { index }),
                "Scale" => return Some(ConfigPath::LinkedTransformScale { index }),
                "PostAffineOriginX" => return Some(ConfigPath::LinkedTransformPostAffineOriginX { index }),
                "PostAffineOriginY" => return Some(ConfigPath::LinkedTransformPostAffineOriginY { index }),
                "PostAffineRotation" => return Some(ConfigPath::LinkedTransformPostAffineRotation { index }),
                "PostAffineScale" => return Some(ConfigPath::LinkedTransformPostAffineScale { index }),
                _ => {}
            }
        }

        // FinalTransform pool paths.
        //
        // Three string formats are accepted; all map to the indexed
        // FinalTransform* enum variants.
        //   1. New canonical:  `FinalTransform.{index}.{field}...`
        //   2. Historical:     `PoolFinalTransform.{index}.{field}...`
        //                      (emitted by the prior branch before the
        //                      Phase 9 rename; saved animation tracks
        //                      from that period still resolve.)
        //   3. Legacy single:  `FinalTransform.{field}...` (no index)
        //                      — back when flame had a single Final
        //                      transform. Mapped to index 0 since the
        //                      legacy variants routed to
        //                      `final_transforms[0]`.
        //
        // Legacy keys with no indexed counterpart (`FinalTransform.Enabled`,
        // `FinalTransform.OriginX|OriginY|Rotation|Scale`) parse as
        // `None` — old animation tracks targeting them become no-ops on
        // load, since those UI helpers were tied to a single-final model
        // that no longer exists.
        if parts.len() >= 3 && (parts[0] == "FinalTransform" || parts[0] == "PoolFinalTransform") {
            if let Ok(index) = parts[1].parse::<usize>() {
                match parts[2] {
                    "Affine" if parts.len() == 4 => {
                        let param = AffineParam::from_char(parts[3].chars().next()?)?;
                        return Some(ConfigPath::FinalTransformAffine { index, param });
                    }
                    "PostAffineEnabled" => {
                        return Some(ConfigPath::FinalTransformPostAffineEnabled { index });
                    }
                    "PostAffine" if parts.len() == 4 => {
                        let param = AffineParam::from_char(parts[3].chars().next()?)?;
                        return Some(ConfigPath::FinalTransformPostAffine { index, param });
                    }
                    "Variation" if parts.len() == 4 => {
                        return Some(ConfigPath::FinalTransformVariation {
                            index,
                            variation: parts[3].to_string(),
                        });
                    }
                    "VariationParam" if parts.len() == 5 => {
                        return Some(ConfigPath::FinalTransformVariationParam {
                            index,
                            variation: parts[3].to_string(),
                            param: parts[4].to_string(),
                        });
                    }
                    "OriginX" => return Some(ConfigPath::FinalTransformOriginX { index }),
                    "OriginY" => return Some(ConfigPath::FinalTransformOriginY { index }),
                    "Rotation" => return Some(ConfigPath::FinalTransformRotation { index }),
                    "Scale" => return Some(ConfigPath::FinalTransformScale { index }),
                    "PostAffineOriginX" => return Some(ConfigPath::FinalTransformPostAffineOriginX { index }),
                    "PostAffineOriginY" => return Some(ConfigPath::FinalTransformPostAffineOriginY { index }),
                    "PostAffineRotation" => return Some(ConfigPath::FinalTransformPostAffineRotation { index }),
                    "PostAffineScale" => return Some(ConfigPath::FinalTransformPostAffineScale { index }),
                    _ => {}
                }
            }
        }
        // Legacy single-final migration: `FinalTransform.{field}...`
        // with parts[1] not numeric → route to index 0.
        if parts.len() >= 2 && parts[0] == "FinalTransform" {
            match parts[1] {
                "Affine" if parts.len() == 3 => {
                    let param = AffineParam::from_char(parts[2].chars().next()?)?;
                    return Some(ConfigPath::FinalTransformAffine { index: 0, param });
                }
                "Variation" if parts.len() == 3 => {
                    return Some(ConfigPath::FinalTransformVariation {
                        index: 0,
                        variation: parts[2].to_string(),
                    });
                }
                "VariationParam" if parts.len() == 4 => {
                    return Some(ConfigPath::FinalTransformVariationParam {
                        index: 0,
                        variation: parts[2].to_string(),
                        param: parts[3].to_string(),
                    });
                }
                "PostAffineEnabled" => {
                    return Some(ConfigPath::FinalTransformPostAffineEnabled { index: 0 });
                }
                "PostAffine" if parts.len() == 3 => {
                    let param = AffineParam::from_char(parts[2].chars().next()?)?;
                    return Some(ConfigPath::FinalTransformPostAffine { index: 0, param });
                }
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
                "ShowHelpOnStartup" => return Some(ConfigPath::SystemShowHelpOnStartup),
                _ => {}
            }
        }

        // Effect paths: DensityEffect.{index}.{Enabled|param} or ColorEffect.{index}.{Enabled|param}
        if parts.len() == 3 && parts[0] == "DensityEffect" {
            if let Ok(index) = parts[1].parse::<usize>() {
                if parts[2] == "Enabled" {
                    return Some(ConfigPath::DensityEffectEnabled { index });
                } else {
                    return Some(ConfigPath::DensityEffectParam { index, param: parts[2].to_string() });
                }
            }
        }

        if parts.len() == 3 && parts[0] == "ColorEffect" {
            if let Ok(index) = parts[1].parse::<usize>() {
                if parts[2] == "Enabled" {
                    return Some(ConfigPath::ColorEffectEnabled { index });
                } else {
                    return Some(ConfigPath::ColorEffectParam { index, param: parts[2].to_string() });
                }
            }
        }

        // Xaos paths: Xaos.{src}.{dst}
        if parts.len() == 3 && parts[0] == "Xaos" {
            if let (Ok(src), Ok(dst)) = (parts[1].parse::<usize>(), parts[2].parse::<usize>()) {
                return Some(ConfigPath::Xaos { src, dst });
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
        | ConfigPath::DofFocusDistance
        | ConfigPath::DofBlurStrength
        | ConfigPath::FogStrength
        | ConfigPath::FogStart
        | ConfigPath::Exposure
        | ConfigPath::Gamma
        | ConfigPath::GammaThreshold
        | ConfigPath::Brightness
        | ConfigPath::Vibrancy
        | ConfigPath::WhiteLevel
        | ConfigPath::Saturation
        | ConfigPath::HueShift
        | ConfigPath::AlphaBlendLow
        | ConfigPath::AlphaBlendHigh
        | ConfigPath::DensityScale
        | ConfigPath::PaletteRotation
        | ConfigPath::PaletteSize
        | ConfigPath::PaletteSqueeze
        | ConfigPath::PaletteSqueezeFalloff
        | ConfigPath::PaletteLogStrength
        | ConfigPath::SpeedFactor
        | ConfigPath::BackgroundColorR
        | ConfigPath::BackgroundColorG
        | ConfigPath::BackgroundColorB
        | ConfigPath::BlendFactor
        | ConfigPath::PerspectiveStrength
        | ConfigPath::TransformWeight { .. }
        | ConfigPath::TransformColor { .. }
        | ConfigPath::TransformColorSpeed { .. }
        | ConfigPath::TransformOpacity { .. }
        | ConfigPath::TransformDirectColor { .. }
        | ConfigPath::TransformAffine { .. }
        | ConfigPath::TransformPostAffine { .. }
        | ConfigPath::TransformVariation { .. }
        | ConfigPath::TransformVariationParam { .. }
        | ConfigPath::TransformOriginX { .. }
        | ConfigPath::TransformOriginY { .. }
        | ConfigPath::TransformRotation { .. }
        | ConfigPath::TransformScale { .. }
        | ConfigPath::TransformPostAffineOriginX { .. }
        | ConfigPath::TransformPostAffineOriginY { .. }
        | ConfigPath::TransformPostAffineRotation { .. }
        | ConfigPath::TransformPostAffineScale { .. }
        | ConfigPath::LinkedTransformAffine { .. }
        | ConfigPath::LinkedTransformPostAffine { .. }
        | ConfigPath::LinkedTransformVariation { .. }
        | ConfigPath::LinkedTransformVariationParam { .. }
        | ConfigPath::LinkedTransformOriginX { .. }
        | ConfigPath::LinkedTransformOriginY { .. }
        | ConfigPath::LinkedTransformRotation { .. }
        | ConfigPath::LinkedTransformScale { .. }
        | ConfigPath::LinkedTransformPostAffineOriginX { .. }
        | ConfigPath::LinkedTransformPostAffineOriginY { .. }
        | ConfigPath::LinkedTransformPostAffineRotation { .. }
        | ConfigPath::LinkedTransformPostAffineScale { .. }
        | ConfigPath::FinalTransformAffine { .. }
        | ConfigPath::FinalTransformPostAffine { .. }
        | ConfigPath::FinalTransformVariation { .. }
        | ConfigPath::FinalTransformVariationParam { .. }
        | ConfigPath::FinalTransformOriginX { .. }
        | ConfigPath::FinalTransformOriginY { .. }
        | ConfigPath::FinalTransformRotation { .. }
        | ConfigPath::FinalTransformScale { .. }
        | ConfigPath::FinalTransformPostAffineOriginX { .. }
        | ConfigPath::FinalTransformPostAffineOriginY { .. }
        | ConfigPath::FinalTransformPostAffineRotation { .. }
        | ConfigPath::FinalTransformPostAffineScale { .. }
        | ConfigPath::Xaos { .. }
        | ConfigPath::SystemTargetFps
        | ConfigPath::LevelsLow
        | ConfigPath::LevelsHigh
        | ConfigPath::LevelsGamma => {
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
        | ConfigPath::LevelsEnabled
        | ConfigPath::UseDynamicBlend
        | ConfigPath::DeterministicRng
        | ConfigPath::PaletteReverse
        | ConfigPath::TransformPostAffineEnabled { .. }
        | ConfigPath::LinkedTransformPostAffineEnabled { .. }
        | ConfigPath::FinalTransformPostAffineEnabled { .. }
        | ConfigPath::SystemVsyncEnabled
        | ConfigPath::SystemShowHelpOnStartup => {
            json.as_bool().map(ConfigValue::Bool)
        }

        // UInt parameters
        ConfigPath::PaletteIndex
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

        // Optional usize as Int (-1 = None, 0+ = Some(index))
        ConfigPath::SoloTransform => {
            json.as_i64().map(|i| ConfigValue::Int(i as i32))
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

        ConfigPath::HighlightMode => {
            use crate::scene::tonemap::HighlightMode;
            if let Some(s) = json.as_str() {
                match s {
                    "Clip" => Some(ConfigValue::HighlightMode(HighlightMode::Clip)),
                    "MaxNorm" => Some(ConfigValue::HighlightMode(HighlightMode::MaxNorm)),
                    "Reinhard" => Some(ConfigValue::HighlightMode(HighlightMode::Reinhard)),
                    "Filmic" => Some(ConfigValue::HighlightMode(HighlightMode::Filmic)),
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

        ConfigPath::PaletteSqueezeMode => {
            if let Some(s) = json.as_str() {
                use crate::scene::palette::SqueezeMode;
                match s {
                    "Linear" => Some(ConfigValue::SqueezeMode(SqueezeMode::Linear)),
                    "Geometric" => Some(ConfigValue::SqueezeMode(SqueezeMode::Geometric)),
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

        // Effect enabled flags (bool)
        ConfigPath::DensityEffectEnabled { .. }
        | ConfigPath::ColorEffectEnabled { .. } => {
            json.as_bool().map(ConfigValue::Bool)
        }

        // Effect parameters (float)
        ConfigPath::DensityEffectParam { .. }
        | ConfigPath::ColorEffectParam { .. } => {
            json.as_f64().map(|f| ConfigValue::Float(f as f32))
        }

        // Complex types not supported for animation (yet)
        ConfigPath::TonemapCurve | ConfigPath::Palette => None,

        // Add/Remove operations not animatable
        ConfigPath::AddColorEffect { .. }
        | ConfigPath::RemoveColorEffect { .. }
        | ConfigPath::AddDensityEffect { .. }
        | ConfigPath::RemoveDensityEffect { .. } => None,
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

    /// Phase 9 migration shim: legacy `FinalTransform.{field}...` keys
    /// (no index — emitted before the per-pool model) and the prior
    /// branch's `PoolFinalTransform.{index}.{field}...` keys must both
    /// parse into the new indexed `FinalTransform*` variants. Old
    /// animation tracks targeting either format keep working without
    /// a manual migration step.
    #[test]
    fn legacy_final_transform_keys_migrate_to_indexed() {
        // Legacy single-final keys → index 0
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.Affine.e"),
            Some(ConfigPath::FinalTransformAffine { index: 0, param: AffineParam::E })
        );
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.Variation.spherical"),
            Some(ConfigPath::FinalTransformVariation {
                index: 0, variation: "spherical".to_string()
            })
        );
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.VariationParam.julian.power"),
            Some(ConfigPath::FinalTransformVariationParam {
                index: 0, variation: "julian".to_string(), param: "power".to_string(),
            })
        );
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.PostAffineEnabled"),
            Some(ConfigPath::FinalTransformPostAffineEnabled { index: 0 })
        );
        assert_eq!(
            ConfigPath::from_string_key("FinalTransform.PostAffine.a"),
            Some(ConfigPath::FinalTransformPostAffine { index: 0, param: AffineParam::A })
        );

        // Legacy keys with no indexed counterpart parse as None — old
        // tracks targeting the singular UI helpers become no-ops.
        assert_eq!(ConfigPath::from_string_key("FinalTransform.Enabled"), None);
        assert_eq!(ConfigPath::from_string_key("FinalTransform.OriginX"), None);
        assert_eq!(ConfigPath::from_string_key("FinalTransform.OriginY"), None);
        assert_eq!(ConfigPath::from_string_key("FinalTransform.Rotation"), None);
        assert_eq!(ConfigPath::from_string_key("FinalTransform.Scale"), None);

        // Prior-branch PoolFinalTransform keys → same indexed variants.
        assert_eq!(
            ConfigPath::from_string_key("PoolFinalTransform.2.Affine.b"),
            Some(ConfigPath::FinalTransformAffine { index: 2, param: AffineParam::B })
        );
        assert_eq!(
            ConfigPath::from_string_key("PoolFinalTransform.0.PostAffineEnabled"),
            Some(ConfigPath::FinalTransformPostAffineEnabled { index: 0 })
        );
    }

    /// Comprehensive roundtrip enforcement: every ConfigPath variant
    /// that's reachable as an animation target MUST roundtrip cleanly
    /// through to_string_key → from_string_key. Adding a new variant
    /// without updating both directions is a silent break of the
    /// animation contract — animation tracks targeting that variant
    /// will fail to parse on load and silently no-op.
    ///
    /// Action-only variants (AddColorEffect, RemoveColorEffect,
    /// AddDensityEffect, RemoveDensityEffect) are intentionally
    /// excluded — they're one-shot UI actions, never animation
    /// targets.
    #[test]
    fn test_config_path_string_roundtrip_complete() {
        let paths = vec![
            // View
            ConfigPath::Zoom,
            ConfigPath::Pan,
            ConfigPath::PanX,
            ConfigPath::PanY,
            ConfigPath::Rotation,
            ConfigPath::CameraRotationX,
            ConfigPath::CameraRotationY,
            ConfigPath::CameraZ,
            ConfigPath::DofFocusDistance,
            ConfigPath::DofBlurStrength,
            ConfigPath::FogStrength,
            ConfigPath::FogStart,

            // Tone mapping
            ConfigPath::Exposure,
            ConfigPath::Gamma,
            ConfigPath::GammaThreshold,
            ConfigPath::Brightness,
            ConfigPath::Vibrancy,
            ConfigPath::WhiteLevel,
            ConfigPath::Saturation,
            ConfigPath::HueShift,
            ConfigPath::AlphaBlendLow,
            ConfigPath::AlphaBlendHigh,
            ConfigPath::DensityScale,
            ConfigPath::TonemapMode,
            ConfigPath::HighlightMode,
            ConfigPath::TonemapCurve,
            ConfigPath::UseCurve,
            ConfigPath::LevelsEnabled,
            ConfigPath::LevelsLow,
            ConfigPath::LevelsHigh,
            ConfigPath::LevelsGamma,

            // Color
            ConfigPath::ColorMode,
            ConfigPath::PathMapStyle,
            ConfigPath::PathCaptureMode,
            ConfigPath::PathTrackingMode,
            ConfigPath::PaletteIndex,
            ConfigPath::Palette,
            ConfigPath::PaletteRotation,
            ConfigPath::PaletteSize,
            ConfigPath::PaletteSqueeze,
            ConfigPath::PaletteSqueezeMode,
            ConfigPath::PaletteSqueezeFalloff,
            ConfigPath::PaletteLogStrength,
            ConfigPath::PaletteReverse,
            ConfigPath::SpeedFactor,
            ConfigPath::BackgroundColor,
            ConfigPath::BackgroundColorR,
            ConfigPath::BackgroundColorG,
            ConfigPath::BackgroundColorB,

            // Rendering
            ConfigPath::BlendFactor,
            ConfigPath::UseDynamicBlend,
            ConfigPath::MaxIterations,
            ConfigPath::DeterministicRng,

            // Transform pool (Normal)
            ConfigPath::TransformCount,
            ConfigPath::TransformWeight { index: 0 },
            ConfigPath::TransformColor { index: 1 },
            ConfigPath::TransformColorSpeed { index: 2 },
            ConfigPath::TransformOpacity { index: 0 },
            ConfigPath::TransformDirectColor { index: 0 },
            ConfigPath::TransformAffine { index: 0, param: AffineParam::A },
            ConfigPath::TransformAffine { index: 5, param: AffineParam::G },
            ConfigPath::TransformVariation { index: 0, variation: "linear".to_string() },
            ConfigPath::TransformVariationParam {
                index: 0, variation: "julian".to_string(), param: "power".to_string(),
            },
            ConfigPath::TransformOriginX { index: 0 },
            ConfigPath::TransformOriginY { index: 0 },
            ConfigPath::TransformRotation { index: 0 },
            ConfigPath::TransformScale { index: 0 },
            ConfigPath::TransformPostAffineEnabled { index: 0 },
            ConfigPath::TransformPostAffine { index: 0, param: AffineParam::F },

            // Linked + Final pools (covered separately by
            // test_config_path_string_roundtrip_pool_transforms — included
            // again here to keep this test comprehensive).
            ConfigPath::LinkedTransformAffine { index: 0, param: AffineParam::A },
            ConfigPath::LinkedTransformPostAffineEnabled { index: 0 },
            ConfigPath::LinkedTransformPostAffine { index: 0, param: AffineParam::F },
            ConfigPath::LinkedTransformVariation {
                index: 0, variation: "spherical".to_string(),
            },
            ConfigPath::LinkedTransformVariationParam {
                index: 0, variation: "julian".to_string(), param: "power".to_string(),
            },
            ConfigPath::FinalTransformAffine { index: 0, param: AffineParam::B },
            ConfigPath::FinalTransformPostAffineEnabled { index: 0 },
            ConfigPath::FinalTransformPostAffine { index: 0, param: AffineParam::E },
            ConfigPath::FinalTransformVariation {
                index: 0, variation: "bipolar".to_string(),
            },
            ConfigPath::FinalTransformVariationParam {
                index: 0, variation: "bipolar".to_string(), param: "shift".to_string(),
            },

            // Flame
            ConfigPath::RenderMode,
            ConfigPath::PerspectiveStrength,
            ConfigPath::Xaos { src: 0, dst: 1 },
            ConfigPath::Xaos { src: 3, dst: 7 },
            ConfigPath::SoloTransform,

            // Effects
            ConfigPath::DensityEffectEnabled { index: 0 },
            ConfigPath::DensityEffectParam { index: 0, param: "strength".to_string() },
            ConfigPath::ColorEffectEnabled { index: 0 },
            ConfigPath::ColorEffectParam { index: 0, param: "amount".to_string() },

            // System
            ConfigPath::SystemIterationsPerThread,
            ConfigPath::SystemBurnIn,
            ConfigPath::SystemVsyncEnabled,
            ConfigPath::SystemTargetFps,
            ConfigPath::SystemExportWidth,
            ConfigPath::SystemExportHeight,
            ConfigPath::SystemLanguage,
            ConfigPath::SystemShowHelpOnStartup,
        ];

        let mut failed: Vec<String> = Vec::new();
        for path in paths {
            let key = path.to_string_key();
            match ConfigPath::from_string_key(&key) {
                Some(parsed) if parsed == path => { /* ok */ }
                Some(parsed) => {
                    failed.push(format!(
                        "key={:?} → parsed={:?} but expected={:?}",
                        key, parsed, path
                    ));
                }
                None => {
                    failed.push(format!("key={:?} (from {:?}) failed to parse", key, path));
                }
            }
        }

        assert!(
            failed.is_empty(),
            "ConfigPath roundtrip failures:\n  {}",
            failed.join("\n  "),
        );
    }

    #[test]
    fn test_config_path_string_roundtrip_pool_transforms() {
        // Linked + Final pool paths — animation tracks save these as
        // strings and re-parse on load. Must roundtrip cleanly or
        // animation silently no-ops on those pool members.
        let paths = vec![
            ConfigPath::LinkedTransformAffine { index: 0, param: AffineParam::A },
            ConfigPath::LinkedTransformAffine { index: 7, param: AffineParam::G },
            ConfigPath::LinkedTransformPostAffineEnabled { index: 2 },
            ConfigPath::LinkedTransformPostAffine { index: 1, param: AffineParam::F },
            ConfigPath::LinkedTransformVariation {
                index: 3,
                variation: "spherical".to_string(),
            },
            ConfigPath::LinkedTransformVariationParam {
                index: 0,
                variation: "julian".to_string(),
                param: "power".to_string(),
            },
            ConfigPath::FinalTransformAffine { index: 0, param: AffineParam::B },
            ConfigPath::FinalTransformPostAffineEnabled { index: 4 },
            ConfigPath::FinalTransformPostAffine { index: 1, param: AffineParam::E },
            ConfigPath::FinalTransformVariation {
                index: 2,
                variation: "bipolar".to_string(),
            },
            ConfigPath::FinalTransformVariationParam {
                index: 0,
                variation: "bipolar".to_string(),
                param: "shift".to_string(),
            },
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
